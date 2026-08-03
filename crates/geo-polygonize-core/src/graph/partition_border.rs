use crate::types::{Coord3D, PartitionFaceRef};
use crate::utils::canonical_coordinate_bits;
use std::collections::{BTreeMap, BTreeSet};

use super::planar_graph::{DirEdgeId, FaceId};

/// A partition side carried as observation metadata, not part of the global
/// node or edge identity. A corner can therefore be shared by two sides.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PartitionBorderSide {
    MinX,
    MaxX,
    MinY,
    MaxY,
}

impl PartitionBorderSide {
    fn coordinate_index(self) -> usize {
        match self {
            Self::MinX | Self::MaxX => 0,
            Self::MinY | Self::MaxY => 1,
        }
    }

    fn is_complementary_to(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::MinX, Self::MaxX)
                | (Self::MaxX, Self::MinX)
                | (Self::MinY, Self::MaxY)
                | (Self::MaxY, Self::MinY)
        )
    }
}

/// Exact 2D identity of a node on a partition border.
///
/// Z is deliberately excluded from the key. Adjacent partitions must match
/// the same topology even when their local Z decisions differ; Z candidates
/// are retained as node observations for a later reconciliation step.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PartitionBorderNodeKey {
    xy_bits: [u64; 2],
}

impl PartitionBorderNodeKey {
    pub fn from_coord(coord: Coord3D) -> Self {
        Self {
            xy_bits: [
                canonical_coordinate_bits(coord.x),
                canonical_coordinate_bits(coord.y),
            ],
        }
    }

    pub fn xy_bits(self) -> [u64; 2] {
        self.xy_bits
    }
}

/// Canonical, undirected identity of a partition-border edge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PartitionBorderEdgeKey {
    start: PartitionBorderNodeKey,
    end: PartitionBorderNodeKey,
}

impl PartitionBorderEdgeKey {
    pub fn new(start: PartitionBorderNodeKey, end: PartitionBorderNodeKey) -> Option<Self> {
        (start != end).then(|| {
            if start <= end {
                Self { start, end }
            } else {
                Self {
                    start: end,
                    end: start,
                }
            }
        })
    }

    pub fn endpoints(self) -> (PartitionBorderNodeKey, PartitionBorderNodeKey) {
        (self.start, self.end)
    }
}

/// One directed local observation of a canonical partition-border edge.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PartitionBorderHalfEdge {
    pub edge_key: PartitionBorderEdgeKey,
    pub from: PartitionBorderNodeKey,
    pub to: PartitionBorderNodeKey,
    pub from_z_bits: u64,
    pub to_z_bits: u64,
    pub side: PartitionBorderSide,
    pub partition_id: usize,
    pub local_dir_edge_id: DirEdgeId,
    /// Component-local face ID retained for existing debug consumers.
    pub face_id: Option<FaceId>,
    pub(crate) face_ref: Option<PartitionFaceRef>,
    pub source_line_ids: Vec<u32>,
}

impl PartitionBorderHalfEdge {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        partition_id: usize,
        local_dir_edge_id: DirEdgeId,
        face_id: Option<FaceId>,
        side: PartitionBorderSide,
        start: Coord3D,
        end: Coord3D,
        source_line_ids: impl IntoIterator<Item = u32>,
    ) -> Option<Self> {
        Self::new_with_face_ref(
            partition_id,
            local_dir_edge_id,
            face_id.map(|face_id| PartitionFaceRef {
                partition_id,
                component_id: 0,
                face_id,
            }),
            side,
            start,
            end,
            source_line_ids,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_face_ref(
        partition_id: usize,
        local_dir_edge_id: DirEdgeId,
        face_ref: Option<PartitionFaceRef>,
        side: PartitionBorderSide,
        start: Coord3D,
        end: Coord3D,
        source_line_ids: impl IntoIterator<Item = u32>,
    ) -> Option<Self> {
        let from = PartitionBorderNodeKey::from_coord(start);
        let to = PartitionBorderNodeKey::from_coord(end);
        let edge_key = PartitionBorderEdgeKey::new(from, to)?;
        let mut source_line_ids = source_line_ids.into_iter().collect::<Vec<_>>();
        source_line_ids.sort_unstable();
        source_line_ids.dedup();
        Some(Self {
            edge_key,
            from,
            to,
            from_z_bits: canonical_coordinate_bits(start.z),
            to_z_bits: canonical_coordinate_bits(end.z),
            side,
            partition_id,
            local_dir_edge_id,
            face_id: face_ref.map(|face_ref| face_ref.face_id),
            face_ref,
            source_line_ids,
        })
    }

    fn z_at(&self, point: PartitionBorderNodeKey, tangent_index: usize) -> u64 {
        if point == self.from {
            return self.from_z_bits;
        }
        if point == self.to {
            return self.to_z_bits;
        }
        let start = f64::from_bits(self.from.xy_bits()[tangent_index]);
        let end = f64::from_bits(self.to.xy_bits()[tangent_index]);
        let position = f64::from_bits(point.xy_bits()[tangent_index]);
        let fraction = (position - start) / (end - start);
        canonical_coordinate_bits(
            f64::from_bits(self.from_z_bits)
                + (f64::from_bits(self.to_z_bits) - f64::from_bits(self.from_z_bits)) * fraction,
        )
    }
}

/// Declared shared border between two neighboring partitions.
///
/// The exact border coordinate is part of the relationship, so geometrically
/// coincident linework cannot match without a shared partition boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PartitionBorderAdjacency {
    first_partition_id: usize,
    first_side: PartitionBorderSide,
    second_partition_id: usize,
    second_side: PartitionBorderSide,
    coordinate_bits: u64,
}

impl PartitionBorderAdjacency {
    pub fn new(
        first_partition_id: usize,
        first_side: PartitionBorderSide,
        second_partition_id: usize,
        second_side: PartitionBorderSide,
        coordinate: f64,
    ) -> crate::Result<Self> {
        if first_partition_id == second_partition_id || !first_side.is_complementary_to(second_side)
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "partition border adjacency must join distinct complementary sides"
                    .to_string(),
            });
        }
        let (first_partition_id, first_side, second_partition_id, second_side) =
            if first_partition_id < second_partition_id {
                (
                    first_partition_id,
                    first_side,
                    second_partition_id,
                    second_side,
                )
            } else {
                (
                    second_partition_id,
                    second_side,
                    first_partition_id,
                    first_side,
                )
            };
        Ok(Self {
            first_partition_id,
            first_side,
            second_partition_id,
            second_side,
            coordinate_bits: canonical_coordinate_bits(coordinate),
        })
    }

    fn matches(self, first: &PartitionBorderHalfEdge, second: &PartitionBorderHalfEdge) -> bool {
        self.matches_observation(first, self.first_partition_id, self.first_side)
            && self.matches_observation(second, self.second_partition_id, self.second_side)
            || self.matches_observation(first, self.second_partition_id, self.second_side)
                && self.matches_observation(second, self.first_partition_id, self.first_side)
    }

    fn matches_observation(
        self,
        observation: &PartitionBorderHalfEdge,
        partition_id: usize,
        side: PartitionBorderSide,
    ) -> bool {
        let coordinate_index = side.coordinate_index();
        observation.partition_id == partition_id
            && observation.side == side
            && observation.from.xy_bits()[coordinate_index] == self.coordinate_bits
            && observation.to.xy_bits()[coordinate_index] == self.coordinate_bits
    }
}

/// Stable identity of one local directed border observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PartitionBorderObservationId {
    pub partition_id: usize,
    pub local_dir_edge_id: DirEdgeId,
}

impl PartitionBorderHalfEdge {
    pub fn observation_id(&self) -> PartitionBorderObservationId {
        PartitionBorderObservationId {
            partition_id: self.partition_id,
            local_dir_edge_id: self.local_dir_edge_id,
        }
    }
}

/// An unambiguous opposite-direction pair observed in two partitions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PartitionBorderTwin {
    pub edge_key: PartitionBorderEdgeKey,
    /// Observation whose direction follows `edge_key.endpoints()`.
    pub forward: PartitionBorderObservationId,
    /// Observation whose direction reverses `edge_key.endpoints()`.
    pub reverse: PartitionBorderObservationId,
}

/// Provenance and Z evidence merged from one unambiguous partition-border twin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionBorderTwinPayload {
    pub twin: PartitionBorderTwin,
    pub source_line_ids: Vec<u32>,
    /// Distinct Z candidates at `twin.edge_key.endpoints().0`, in bit order.
    /// A length greater than one is an explicit conflict, not a hidden choice.
    pub start_z_bits: Vec<u64>,
    /// Distinct Z candidates at `twin.edge_key.endpoints().1`, in bit order.
    /// A length greater than one is an explicit conflict, not a hidden choice.
    pub end_z_bits: Vec<u64>,
}

/// Deterministic partition-border observations ready for twin reconciliation.
///
/// The graph stores canonical undirected edge buckets while retaining each
/// local directed observation. This makes reversal and insertion order
/// irrelevant to lookup without discarding direction, provenance, or Z data.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PartitionBorderGraph {
    nodes: BTreeMap<PartitionBorderNodeKey, BTreeSet<u64>>,
    observations: BTreeMap<PartitionBorderObservationId, PartitionBorderHalfEdge>,
    adjacencies: BTreeSet<PartitionBorderAdjacency>,
    edges: BTreeMap<PartitionBorderEdgeKey, BTreeSet<PartitionBorderHalfEdge>>,
}

impl PartitionBorderGraph {
    pub fn insert(&mut self, half_edge: PartitionBorderHalfEdge) -> crate::Result<()> {
        let observation_id = half_edge.observation_id();
        if let Some(existing) = self.observations.get(&observation_id) {
            if existing == &half_edge {
                return Ok(());
            }
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "partition border observation ({}, {}) conflicts with prior payload",
                    observation_id.partition_id, observation_id.local_dir_edge_id
                ),
            });
        }
        let from = half_edge.from;
        let to = half_edge.to;
        self.nodes
            .entry(from)
            .or_default()
            .insert(half_edge.from_z_bits);
        self.nodes
            .entry(to)
            .or_default()
            .insert(half_edge.to_z_bits);
        self.edges
            .entry(half_edge.edge_key)
            .or_default()
            .insert(half_edge.clone());
        self.observations.insert(observation_id, half_edge);
        Ok(())
    }

    pub fn declare_adjacency(&mut self, adjacency: PartitionBorderAdjacency) {
        self.adjacencies.insert(adjacency);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    fn normalized_edges(
        &self,
    ) -> BTreeMap<PartitionBorderEdgeKey, BTreeSet<PartitionBorderHalfEdge>> {
        let mut edges =
            BTreeMap::<PartitionBorderEdgeKey, BTreeSet<PartitionBorderHalfEdge>>::new();
        for &adjacency in &self.adjacencies {
            let observations = self
                .observations
                .values()
                .filter(|observation| {
                    adjacency.matches_observation(
                        observation,
                        adjacency.first_partition_id,
                        adjacency.first_side,
                    ) || adjacency.matches_observation(
                        observation,
                        adjacency.second_partition_id,
                        adjacency.second_side,
                    )
                })
                .collect::<Vec<_>>();
            let coordinate_index = adjacency.first_side.coordinate_index();
            let tangent_index = 1 - coordinate_index;
            let mut breakpoints = observations
                .iter()
                .flat_map(|observation| [observation.from, observation.to])
                .collect::<Vec<_>>();
            breakpoints.sort_unstable_by(|left, right| {
                f64::from_bits(left.xy_bits()[tangent_index])
                    .total_cmp(&f64::from_bits(right.xy_bits()[tangent_index]))
            });
            breakpoints.dedup();

            for observation in observations {
                let start = f64::from_bits(observation.from.xy_bits()[tangent_index]);
                let end = f64::from_bits(observation.to.xy_bits()[tangent_index]);
                let mut points = breakpoints
                    .iter()
                    .copied()
                    .filter(|point| {
                        let value = f64::from_bits(point.xy_bits()[tangent_index]);
                        (start.total_cmp(&value).is_le() && value.total_cmp(&end).is_le())
                            || (end.total_cmp(&value).is_le() && value.total_cmp(&start).is_le())
                    })
                    .collect::<Vec<_>>();
                if start.total_cmp(&end).is_gt() {
                    points.reverse();
                }
                for pair in points.windows(2) {
                    let Some(edge_key) = PartitionBorderEdgeKey::new(pair[0], pair[1]) else {
                        continue;
                    };
                    let mut normalized = observation.clone();
                    normalized.edge_key = edge_key;
                    normalized.from = pair[0];
                    normalized.to = pair[1];
                    normalized.from_z_bits = observation.z_at(pair[0], tangent_index);
                    normalized.to_z_bits = observation.z_at(pair[1], tangent_index);
                    edges.entry(edge_key).or_default().insert(normalized);
                }
            }
        }
        edges
    }

    fn twin_pairs_from_edges(
        &self,
        edges: &BTreeMap<PartitionBorderEdgeKey, BTreeSet<PartitionBorderHalfEdge>>,
    ) -> Vec<PartitionBorderTwin> {
        edges
            .iter()
            .filter_map(|(&edge_key, observations)| {
                let mut observations = observations.iter();
                let first = observations.next()?;
                let second = observations.next()?;
                if observations.next().is_some()
                    || first.partition_id == second.partition_id
                    || !self
                        .adjacencies
                        .iter()
                        .any(|adjacency| adjacency.matches(first, second))
                {
                    return None;
                }
                let (start, end) = edge_key.endpoints();
                let first_is_forward = first.from == start && first.to == end;
                let second_is_forward = second.from == start && second.to == end;
                let first_is_reverse = first.from == end && first.to == start;
                let second_is_reverse = second.from == end && second.to == start;
                if first_is_forward && second_is_reverse {
                    Some(PartitionBorderTwin {
                        edge_key,
                        forward: first.observation_id(),
                        reverse: second.observation_id(),
                    })
                } else if second_is_forward && first_is_reverse {
                    Some(PartitionBorderTwin {
                        edge_key,
                        forward: second.observation_id(),
                        reverse: first.observation_id(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Matches only exactly-two-observation buckets with opposite directions
    /// on one declared partition border. Ambiguous, same-partition, or
    /// unrelated-partition buckets remain unmatched for later reconciliation.
    pub fn twin_pairs(&self) -> Vec<PartitionBorderTwin> {
        self.twin_pairs_from_edges(&self.normalized_edges())
    }

    /// Merges source IDs and retains every distinct Z candidate for each
    /// canonical endpoint. No Z conflict policy is applied here.
    pub fn reconcile_twin_payloads(&self) -> Vec<PartitionBorderTwinPayload> {
        let edges = self.normalized_edges();
        self.twin_pairs_from_edges(&edges)
            .into_iter()
            .filter_map(|twin| {
                let observations = edges.get(&twin.edge_key)?;
                let forward = observations
                    .iter()
                    .find(|observation| observation.observation_id() == twin.forward)?;
                let reverse = observations
                    .iter()
                    .find(|observation| observation.observation_id() == twin.reverse)?;

                let mut source_line_ids = forward
                    .source_line_ids
                    .iter()
                    .chain(&reverse.source_line_ids)
                    .copied()
                    .collect::<Vec<_>>();
                source_line_ids.sort_unstable();
                source_line_ids.dedup();

                let mut start_z_bits = vec![forward.from_z_bits, reverse.to_z_bits];
                start_z_bits.sort_unstable();
                start_z_bits.dedup();
                let mut end_z_bits = vec![forward.to_z_bits, reverse.from_z_bits];
                end_z_bits.sort_unstable();
                end_z_bits.dedup();

                Some(PartitionBorderTwinPayload {
                    twin,
                    source_line_ids,
                    start_z_bits,
                    end_z_bits,
                })
            })
            .collect()
    }

    pub fn node_z_bits(&self, key: PartitionBorderNodeKey) -> Option<&BTreeSet<u64>> {
        self.nodes.get(&key)
    }

    pub fn edge_observations(
        &self,
        key: PartitionBorderEdgeKey,
    ) -> Option<&BTreeSet<PartitionBorderHalfEdge>> {
        self.edges.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::PlanarGraph;
    use crate::types::Line3D;

    fn coord(x: f64, y: f64, z: f64) -> Coord3D {
        Coord3D::new(x, y, z)
    }

    fn face(partition_id: usize, component_id: usize, face_id: usize) -> PartitionFaceRef {
        PartitionFaceRef {
            partition_id,
            component_id,
            face_id,
        }
    }

    #[test]
    fn node_keys_normalize_signed_zero_but_preserve_z_outside_identity() {
        assert_eq!(
            PartitionBorderNodeKey::from_coord(coord(-0.0, 1.0, 4.0)),
            PartitionBorderNodeKey::from_coord(coord(0.0, 1.0, -4.0))
        );
        assert_eq!(
            PartitionBorderNodeKey::from_coord(coord(0.0, 1.0, 4.0)).xy_bits(),
            [0, 1.0f64.to_bits()]
        );
    }

    #[test]
    fn edge_keys_are_reversal_invariant_and_round_trip_endpoints() {
        let start = PartitionBorderNodeKey::from_coord(coord(2.0, 0.0, 1.0));
        let end = PartitionBorderNodeKey::from_coord(coord(0.0, 0.0, 2.0));
        let forward = PartitionBorderEdgeKey::new(start, end).unwrap();
        let reverse = PartitionBorderEdgeKey::new(end, start).unwrap();

        assert_eq!(forward, reverse);
        assert_eq!(forward.endpoints(), (end, start));
        assert!(PartitionBorderEdgeKey::new(start, start).is_none());
    }

    #[test]
    fn graph_deduplicates_keys_and_retains_local_observations() {
        let start = coord(-0.0, 0.0, 1.0);
        let end = coord(2.0, 0.0, 2.0);
        let first = PartitionBorderHalfEdge::new(
            1,
            7,
            Some(3),
            PartitionBorderSide::MinY,
            start,
            end,
            [4, 2, 4],
        )
        .unwrap();
        let second = PartitionBorderHalfEdge::new(
            2,
            9,
            Some(5),
            PartitionBorderSide::MaxX,
            end,
            coord(0.0, 0.0, 3.0),
            [2, 8],
        )
        .unwrap();
        let key = first.edge_key;
        let start_key = first.from;

        let mut graph = PartitionBorderGraph::default();
        graph.insert(second).unwrap();
        graph.insert(first).unwrap();

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(
            graph.node_z_bits(start_key).unwrap(),
            &BTreeSet::from([1.0f64.to_bits(), 3.0f64.to_bits()])
        );
        let observations = graph.edge_observations(key).unwrap();
        assert_eq!(observations.len(), 2);
        assert_eq!(
            observations.iter().next().unwrap().source_line_ids,
            vec![2, 4]
        );
    }

    #[test]
    fn degenerate_half_edges_are_not_inserted() {
        assert!(PartitionBorderHalfEdge::new(
            0,
            0,
            None,
            PartitionBorderSide::MinX,
            coord(1.0, 1.0, 0.0),
            coord(1.0, 1.0, 9.0),
            [],
        )
        .is_none());
    }

    #[test]
    fn planar_graph_transfers_directed_identity_into_border_observations() {
        let mut planar = PlanarGraph::new();
        planar.add_line(Line3D::new(coord(0.0, 0.0, 1.0), coord(2.0, 0.0, 2.0), 11));

        let forward = planar
            .partition_border_half_edge(4, 0, PartitionBorderSide::MinY)
            .unwrap();
        let reverse = planar
            .partition_border_half_edge(4, 1, PartitionBorderSide::MaxY)
            .unwrap();
        assert_eq!(forward.edge_key, reverse.edge_key);
        assert_eq!(forward.from, reverse.to);
        assert_eq!(forward.to, reverse.from);
        assert_eq!(forward.source_line_ids, vec![11]);
        assert!(planar.remove_line_by_id(11));
        assert!(planar
            .partition_border_half_edge(4, 0, PartitionBorderSide::MinY)
            .is_none());
    }

    #[test]
    fn face_refs_distinguish_components_with_the_same_local_face_id() {
        let first = PartitionBorderHalfEdge::new_with_face_ref(
            4,
            0,
            Some(face(4, 0, 0)),
            PartitionBorderSide::MinY,
            coord(0.0, 0.0, 0.0),
            coord(1.0, 0.0, 0.0),
            [],
        )
        .unwrap();
        let second = PartitionBorderHalfEdge::new_with_face_ref(
            4,
            1,
            Some(face(4, 1, 0)),
            PartitionBorderSide::MinY,
            coord(2.0, 0.0, 0.0),
            coord(3.0, 0.0, 0.0),
            [],
        )
        .unwrap();

        assert_eq!(first.face_id, second.face_id);
        assert_ne!(first.face_ref, second.face_ref);
    }

    #[test]
    fn insert_rejects_a_conflicting_observation_id() {
        let observation = PartitionBorderHalfEdge::new(
            4,
            7,
            Some(3),
            PartitionBorderSide::MinY,
            coord(0.0, 0.0, 1.0),
            coord(1.0, 0.0, 2.0),
            [11],
        )
        .unwrap();
        let mut conflict = observation.clone();
        conflict.source_line_ids = vec![12];

        let mut graph = PartitionBorderGraph::default();
        graph.insert(observation).unwrap();
        assert_eq!(
            graph.insert(conflict).unwrap_err().to_string(),
            "Internal invariant violation: partition border observation (4, 7) conflicts with prior payload"
        );
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn twin_matching_requires_a_declared_adjacent_partition_border() {
        let start = coord(0.0, 0.0, 1.0);
        let end = coord(2.0, 0.0, 2.0);
        let forward =
            PartitionBorderHalfEdge::new(1, 7, Some(3), PartitionBorderSide::MinY, start, end, [4])
                .unwrap();
        let reverse =
            PartitionBorderHalfEdge::new(2, 9, Some(5), PartitionBorderSide::MaxY, end, start, [8])
                .unwrap();
        let key = forward.edge_key;

        let mut graph = PartitionBorderGraph::default();
        graph.declare_adjacency(
            PartitionBorderAdjacency::new(
                1,
                PartitionBorderSide::MinY,
                2,
                PartitionBorderSide::MaxY,
                0.0,
            )
            .unwrap(),
        );
        graph.insert(reverse).unwrap();
        graph.insert(forward).unwrap();

        assert_eq!(
            graph.twin_pairs(),
            vec![PartitionBorderTwin {
                edge_key: key,
                forward: PartitionBorderObservationId {
                    partition_id: 1,
                    local_dir_edge_id: 7,
                },
                reverse: PartitionBorderObservationId {
                    partition_id: 2,
                    local_dir_edge_id: 9,
                },
            }]
        );
    }

    #[test]
    fn twin_matching_normalizes_one_to_two_reversed_duplicate_observations() {
        let start = coord(0.0, 0.0, 0.0);
        let end = coord(2.0, 0.0, 20.0);
        let long =
            PartitionBorderHalfEdge::new(1, 7, Some(3), PartitionBorderSide::MinY, start, end, [1])
                .unwrap();
        let high = PartitionBorderHalfEdge::new(
            2,
            9,
            Some(5),
            PartitionBorderSide::MaxY,
            coord(2.0, 0.0, 50.0),
            coord(1.0, 0.0, 60.0),
            [2],
        )
        .unwrap();
        let low = PartitionBorderHalfEdge::new(
            2,
            10,
            Some(5),
            PartitionBorderSide::MaxY,
            coord(1.0, 0.0, 30.0),
            coord(0.0, 0.0, 40.0),
            [3],
        )
        .unwrap();

        let mut graph = PartitionBorderGraph::default();
        graph.declare_adjacency(
            PartitionBorderAdjacency::new(
                1,
                PartitionBorderSide::MinY,
                2,
                PartitionBorderSide::MaxY,
                0.0,
            )
            .unwrap(),
        );
        graph.insert(long.clone()).unwrap();
        graph.insert(long).unwrap();
        graph.insert(high).unwrap();
        graph.insert(low).unwrap();

        assert_eq!(
            graph
                .twin_pairs()
                .iter()
                .map(|twin| (twin.forward, twin.reverse))
                .collect::<Vec<_>>(),
            vec![
                (
                    PartitionBorderObservationId {
                        partition_id: 1,
                        local_dir_edge_id: 7,
                    },
                    PartitionBorderObservationId {
                        partition_id: 2,
                        local_dir_edge_id: 10,
                    },
                ),
                (
                    PartitionBorderObservationId {
                        partition_id: 1,
                        local_dir_edge_id: 7,
                    },
                    PartitionBorderObservationId {
                        partition_id: 2,
                        local_dir_edge_id: 9,
                    },
                ),
            ]
        );
        assert_eq!(
            graph
                .reconcile_twin_payloads()
                .into_iter()
                .map(|payload| (
                    payload.source_line_ids,
                    payload.start_z_bits,
                    payload.end_z_bits
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    vec![1, 3],
                    vec![0, 40.0f64.to_bits()],
                    vec![10.0f64.to_bits(), 30.0f64.to_bits()],
                ),
                (
                    vec![1, 2],
                    vec![10.0f64.to_bits(), 60.0f64.to_bits()],
                    vec![20.0f64.to_bits(), 50.0f64.to_bits()],
                ),
            ]
        );
    }

    #[test]
    fn twin_matching_rejects_unrelated_coincident_partition_borders() {
        let start = coord(0.0, 0.0, 1.0);
        let end = coord(2.0, 0.0, 2.0);
        let forward =
            PartitionBorderHalfEdge::new(3, 7, Some(3), PartitionBorderSide::MinY, start, end, [4])
                .unwrap();
        let reverse =
            PartitionBorderHalfEdge::new(4, 9, Some(5), PartitionBorderSide::MaxY, end, start, [8])
                .unwrap();

        let mut graph = PartitionBorderGraph::default();
        graph.declare_adjacency(
            PartitionBorderAdjacency::new(
                1,
                PartitionBorderSide::MinY,
                2,
                PartitionBorderSide::MaxY,
                0.0,
            )
            .unwrap(),
        );
        graph.insert(reverse).unwrap();
        graph.insert(forward).unwrap();

        assert!(graph.twin_pairs().is_empty());
    }

    #[test]
    fn twin_payload_reconciliation_unions_sources_and_retains_z_conflicts() {
        let forward = PartitionBorderHalfEdge::new(
            1,
            7,
            Some(3),
            PartitionBorderSide::MinY,
            coord(-0.0, 0.0, 1.0),
            coord(2.0, 0.0, 2.0),
            [4, 2, 4],
        )
        .unwrap();
        let reverse = PartitionBorderHalfEdge::new(
            2,
            9,
            Some(5),
            PartitionBorderSide::MaxY,
            coord(2.0, 0.0, 2.0),
            coord(0.0, 0.0, -0.0),
            [8, 4],
        )
        .unwrap();
        let twin = PartitionBorderTwin {
            edge_key: forward.edge_key,
            forward: forward.observation_id(),
            reverse: reverse.observation_id(),
        };

        let mut graph = PartitionBorderGraph::default();
        graph.declare_adjacency(
            PartitionBorderAdjacency::new(
                1,
                PartitionBorderSide::MinY,
                2,
                PartitionBorderSide::MaxY,
                0.0,
            )
            .unwrap(),
        );
        graph.insert(reverse).unwrap();
        graph.insert(forward).unwrap();

        assert_eq!(
            graph.reconcile_twin_payloads(),
            vec![PartitionBorderTwinPayload {
                twin,
                source_line_ids: vec![2, 4, 8],
                start_z_bits: vec![0, 1.0f64.to_bits()],
                end_z_bits: vec![2.0f64.to_bits()],
            }]
        );
    }

    #[test]
    fn twin_matching_leaves_same_partition_and_ambiguous_buckets_unmatched() {
        let start = coord(0.0, 0.0, 0.0);
        let end = coord(1.0, 0.0, 0.0);
        let mut same_partition = PartitionBorderGraph::default();
        same_partition
            .insert(
                PartitionBorderHalfEdge::new(1, 1, None, PartitionBorderSide::MaxX, start, end, [])
                    .unwrap(),
            )
            .unwrap();
        same_partition
            .insert(
                PartitionBorderHalfEdge::new(1, 2, None, PartitionBorderSide::MinX, end, start, [])
                    .unwrap(),
            )
            .unwrap();
        assert!(same_partition.twin_pairs().is_empty());

        let mut ambiguous = same_partition.clone();
        ambiguous
            .insert(
                PartitionBorderHalfEdge::new(2, 3, None, PartitionBorderSide::MinX, end, start, [])
                    .unwrap(),
            )
            .unwrap();
        assert!(ambiguous.twin_pairs().is_empty());
    }
}
