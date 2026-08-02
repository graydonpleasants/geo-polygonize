use crate::types::Coord3D;
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
    pub face_id: Option<FaceId>,
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
            face_id,
            source_line_ids,
        })
    }
}

/// Deterministic partition-border observations ready for future twin matching.
///
/// The graph stores canonical undirected edge buckets while retaining each
/// local directed observation. This makes reversal and insertion order
/// irrelevant to lookup without discarding direction, provenance, or Z data.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PartitionBorderGraph {
    nodes: BTreeMap<PartitionBorderNodeKey, BTreeSet<u64>>,
    edges: BTreeMap<PartitionBorderEdgeKey, BTreeSet<PartitionBorderHalfEdge>>,
}

impl PartitionBorderGraph {
    pub fn insert(&mut self, half_edge: PartitionBorderHalfEdge) {
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
            .insert(half_edge);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
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
        graph.insert(second);
        graph.insert(first);

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
}
