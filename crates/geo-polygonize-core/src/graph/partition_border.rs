use crate::options::{ExecutionPolicy, ZOptions, ZPolicy};
use crate::types::{Coord3D, PartitionFaceRef};
use crate::utils::canonical_coordinate_bits;
use geo_types::Rect;
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

/// One exact intersection between a local arrangement edge and a declared
/// partition-boundary segment.
///
/// The parameter is measured from the edge's original start coordinate. A
/// corner can therefore produce two records with the same point and parameter
/// while retaining both boundary sides for deterministic classification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PartitionBoundaryIntersection {
    pub(crate) side: PartitionBorderSide,
    pub(crate) t: f64,
    pub(crate) point: Coord3D,
}

#[derive(Clone, Copy)]
struct PartitionBoundarySideLine {
    side: PartitionBorderSide,
    boundary_coordinate: f64,
    tangent_min: f64,
    tangent_max: f64,
    axis_start: f64,
    axis_end: f64,
    tangent_start: f64,
    tangent_end: f64,
}

fn push_boundary_intersection(
    intersections: &mut Vec<PartitionBoundaryIntersection>,
    boundary: PartitionBoundarySideLine,
    mut t: f64,
    start: Coord3D,
    end: Coord3D,
) {
    if !t.is_finite() || !(0.0..=1.0).contains(&t) {
        return;
    }
    t = t.clamp(0.0, 1.0);

    let mut point = Coord3D::new(
        start.x + (end.x - start.x) * t,
        start.y + (end.y - start.y) * t,
        start.z + (end.z - start.z) * t,
    );
    match boundary.side {
        PartitionBorderSide::MinX | PartitionBorderSide::MaxX => {
            point.x = boundary.boundary_coordinate;
        }
        PartitionBorderSide::MinY | PartitionBorderSide::MaxY => {
            point.y = boundary.boundary_coordinate;
        }
    }
    let tangent = match boundary.side {
        PartitionBorderSide::MinX | PartitionBorderSide::MaxX => point.y,
        PartitionBorderSide::MinY | PartitionBorderSide::MaxY => point.x,
    };
    if tangent < boundary.tangent_min || tangent > boundary.tangent_max {
        return;
    }

    let point_key = [
        canonical_coordinate_bits(point.x),
        canonical_coordinate_bits(point.y),
    ];
    if intersections.iter().any(|intersection| {
        intersection.side == boundary.side
            && [
                canonical_coordinate_bits(intersection.point.x),
                canonical_coordinate_bits(intersection.point.y),
            ] == point_key
    }) {
        return;
    }
    intersections.push(PartitionBoundaryIntersection {
        side: boundary.side,
        t,
        point,
    });
}

fn append_boundary_side_intersections(
    intersections: &mut Vec<PartitionBoundaryIntersection>,
    boundary: PartitionBoundarySideLine,
    start: Coord3D,
    end: Coord3D,
) {
    let axis_start_on_boundary = canonical_coordinate_bits(boundary.axis_start)
        == canonical_coordinate_bits(boundary.boundary_coordinate);
    let axis_end_on_boundary = canonical_coordinate_bits(boundary.axis_end)
        == canonical_coordinate_bits(boundary.boundary_coordinate);

    if axis_start_on_boundary && axis_end_on_boundary {
        // A collinear edge can extend beyond the finite partition side. Add
        // the side endpoints as breakpoints so the in-border span becomes a
        // physical graph edge after splitting.
        push_boundary_intersection(intersections, boundary, 0.0, start, end);
        push_boundary_intersection(intersections, boundary, 1.0, start, end);
        if boundary.tangent_start != boundary.tangent_end {
            for tangent in [boundary.tangent_min, boundary.tangent_max] {
                let t = (tangent - boundary.tangent_start)
                    / (boundary.tangent_end - boundary.tangent_start);
                push_boundary_intersection(intersections, boundary, t, start, end);
            }
        }
        return;
    }

    if boundary.axis_start == boundary.axis_end {
        return;
    }
    let t = (boundary.boundary_coordinate - boundary.axis_start)
        / (boundary.axis_end - boundary.axis_start);
    push_boundary_intersection(intersections, boundary, t, start, end);
}

/// Returns the exact partition-boundary events for one live graph edge.
///
/// The graph has already passed the configured precision/noding pipeline, so
/// this helper does not snap or infer from polygon envelopes. It only uses the
/// exact rectangle coordinates supplied by the tile partition. Events are
/// sorted along the directed edge, with a stable side order at corners.
pub(crate) fn partition_boundary_intersections(
    start: Coord3D,
    end: Coord3D,
    bbox: Rect<f64>,
) -> Vec<PartitionBoundaryIntersection> {
    let min = bbox.min();
    let max = bbox.max();
    let mut intersections = Vec::with_capacity(8);

    append_boundary_side_intersections(
        &mut intersections,
        PartitionBoundarySideLine {
            side: PartitionBorderSide::MinX,
            boundary_coordinate: min.x,
            tangent_min: min.y,
            tangent_max: max.y,
            axis_start: start.x,
            axis_end: end.x,
            tangent_start: start.y,
            tangent_end: end.y,
        },
        start,
        end,
    );
    append_boundary_side_intersections(
        &mut intersections,
        PartitionBoundarySideLine {
            side: PartitionBorderSide::MaxX,
            boundary_coordinate: max.x,
            tangent_min: min.y,
            tangent_max: max.y,
            axis_start: start.x,
            axis_end: end.x,
            tangent_start: start.y,
            tangent_end: end.y,
        },
        start,
        end,
    );
    append_boundary_side_intersections(
        &mut intersections,
        PartitionBoundarySideLine {
            side: PartitionBorderSide::MinY,
            boundary_coordinate: min.y,
            tangent_min: min.x,
            tangent_max: max.x,
            axis_start: start.y,
            axis_end: end.y,
            tangent_start: start.x,
            tangent_end: end.x,
        },
        start,
        end,
    );
    append_boundary_side_intersections(
        &mut intersections,
        PartitionBoundarySideLine {
            side: PartitionBorderSide::MaxY,
            boundary_coordinate: max.y,
            tangent_min: min.x,
            tangent_max: max.x,
            axis_start: start.y,
            axis_end: end.y,
            tangent_start: start.x,
            tangent_end: end.x,
        },
        start,
        end,
    );

    intersections.sort_unstable_by(|left, right| {
        left.t
            .total_cmp(&right.t)
            .then_with(|| left.side.cmp(&right.side))
            .then_with(|| {
                canonical_coordinate_bits(left.point.x)
                    .cmp(&canonical_coordinate_bits(right.point.x))
            })
            .then_with(|| {
                canonical_coordinate_bits(left.point.y)
                    .cmp(&canonical_coordinate_bits(right.point.y))
            })
    });
    intersections
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
    /// Component identity remains available even when this directed edge is
    /// on the unbounded side and has no local face ID.
    pub component_id: usize,
    pub local_dir_edge_id: DirEdgeId,
    /// Component-local face ID retained for existing debug consumers.
    pub face_id: Option<FaceId>,
    pub(crate) face_ref: Option<PartitionFaceRef>,
    /// Face-walk successor from the local arrangement, when a qualified face
    /// was assigned. This is evidence for a future global face plan, not a
    /// global `next` link.
    pub(crate) local_face_successor: Option<DirEdgeId>,
    /// Whether the local face containing this directed edge is unbounded.
    pub(crate) local_face_is_unbounded: bool,
    pub source_line_ids: Vec<u32>,
    /// The deterministic representative source ID carried by the local edge.
    /// This is the first ID in the sorted source set, or `None` for synthetic
    /// observations without source provenance.
    pub representative_line_id: Option<u32>,
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
        let representative_line_id = source_line_ids.first().copied();
        Some(Self {
            edge_key,
            from,
            to,
            from_z_bits: canonical_coordinate_bits(start.z),
            to_z_bits: canonical_coordinate_bits(end.z),
            side,
            partition_id,
            component_id: face_ref.map_or(0, |face_ref| face_ref.component_id),
            local_dir_edge_id,
            face_id: face_ref.map(|face_ref| face_ref.face_id),
            face_ref,
            local_face_successor: None,
            local_face_is_unbounded: false,
            source_line_ids,
            representative_line_id,
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
    pub edge_key: PartitionBorderEdgeKey,
}

impl PartitionBorderHalfEdge {
    pub fn observation_id(&self) -> PartitionBorderObservationId {
        PartitionBorderObservationId {
            partition_id: self.partition_id,
            local_dir_edge_id: self.local_dir_edge_id,
            edge_key: self.edge_key,
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
    /// Representative IDs retained separately for the two local observations.
    /// A future stitcher may reconcile them only after applying its explicit
    /// representative policy.
    pub forward_representative_line_id: Option<u32>,
    pub reverse_representative_line_id: Option<u32>,
    /// Distinct Z candidates at `twin.edge_key.endpoints().0`, in bit order.
    /// A length greater than one is an explicit conflict, not a hidden choice.
    pub start_z_bits: Vec<u64>,
    /// Distinct Z candidates at `twin.edge_key.endpoints().1`, in bit order.
    /// A length greater than one is an explicit conflict, not a hidden choice.
    pub end_z_bits: Vec<u64>,
}

/// One face-qualified twin link that is safe to carry into a future global
/// arrangement. The local observations and their payload remain immutable;
/// this relation does not rewrite either partition's local adjacency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderFaceTwin {
    pub(crate) twin: PartitionBorderTwin,
    pub(crate) forward_face_ref: PartitionFaceRef,
    pub(crate) reverse_face_ref: PartitionFaceRef,
    pub(crate) payload: PartitionBorderTwinPayload,
}

/// Evidence from attempting to apply exact declared-adjacency twins to
/// qualified local faces.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderTwinApplicationStats {
    pub(crate) candidate_twin_count: usize,
    pub(crate) applied_twin_count: usize,
    pub(crate) missing_face_ref_count: usize,
    pub(crate) invalid_face_ref_count: usize,
}

/// Canonical evidence for one border node after all physical observations
/// have been grouped by exact XY identity. The selected Z value is retained
/// as a policy decision, while every candidate and contributing identity
/// remains available for validation and future global graph construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderNodePayload {
    pub(crate) key: PartitionBorderNodeKey,
    pub(crate) observation_ids: Vec<PartitionBorderObservationId>,
    pub(crate) source_line_ids: Vec<u32>,
    pub(crate) representative_line_ids: Vec<u32>,
    pub(crate) face_refs: Vec<PartitionFaceRef>,
    pub(crate) z_bits: Vec<u64>,
    pub(crate) selected_z_bits: u64,
    pub(crate) selected_z_policy: ZPolicy,
    pub(crate) conflict_tolerance_bits: u64,
    pub(crate) z_conflict: bool,
}

/// Counts from canonical border-node reconciliation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderNodeReconciliationStats {
    pub(crate) node_count: usize,
    pub(crate) z_conflict_count: usize,
}

/// One deterministic connected component of qualified partition-border face
/// evidence. This is retained for a future global arrangement; it does not
/// rewrite local face IDs, adjacency, or tiled output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalComponent {
    pub(crate) component_index: usize,
    pub(crate) face_refs: Vec<PartitionFaceRef>,
    pub(crate) border_node_keys: Vec<PartitionBorderNodeKey>,
    pub(crate) twin_edge_keys: Vec<PartitionBorderEdgeKey>,
}

/// Counts from deterministic global component reconciliation evidence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalComponentReconciliationStats {
    pub(crate) component_count: usize,
    pub(crate) face_count: usize,
    pub(crate) linked_face_count: usize,
    pub(crate) twin_link_count: usize,
}

/// One qualified local border half-edge prepared for a future global face
/// walk. The successor remains in the local directed-edge identity space;
/// this record does not rewrite it into a global arrangement.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PartitionBorderFaceBoundaryCandidate {
    pub(crate) observation_id: PartitionBorderObservationId,
    pub(crate) edge_key: PartitionBorderEdgeKey,
    pub(crate) face_ref: PartitionFaceRef,
    pub(crate) local_dir_edge_id: DirEdgeId,
    pub(crate) local_face_successor: DirEdgeId,
    pub(crate) local_face_is_unbounded: bool,
}

/// Deterministic boundary evidence grouped by qualified local face identity.
/// Cross-partition twin edges are retained separately so a later stitcher can
/// validate transitions before assigning global `next` links or face IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFacePlan {
    pub(crate) face_ref: PartitionFaceRef,
    pub(crate) candidates: Vec<PartitionBorderFaceBoundaryCandidate>,
    pub(crate) twin_edge_keys: Vec<PartitionBorderEdgeKey>,
    pub(crate) local_face_is_unbounded: bool,
}

/// Counts from the retained global face-boundary plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFacePlanStats {
    pub(crate) face_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) missing_successor_count: usize,
    pub(crate) unbounded_face_count: usize,
    pub(crate) linked_face_count: usize,
}

/// Counts from validating the retained global face-boundary plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFacePlanValidationStats {
    pub(crate) face_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) twin_link_count: usize,
    pub(crate) unbounded_face_count: usize,
}

/// Deterministic evidence for the declared-adjacency twin boundary.
///
/// Only observations covered by a declared partition adjacency contribute to
/// normalized or matched counts. Unrelated coincident observations therefore
/// cannot become twins by geometry alone.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PartitionBorderReconciliationStats {
    pub declared_adjacency_count: usize,
    pub normalized_edge_count: usize,
    pub matched_twin_count: usize,
    pub unmatched_edge_count: usize,
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
    applied_face_twins: Vec<PartitionBorderFaceTwin>,
    reconciled_nodes: Vec<PartitionBorderNodePayload>,
    global_components: Vec<PartitionBorderGlobalComponent>,
    global_face_plans: Vec<PartitionBorderGlobalFacePlan>,
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
                    "partition border observation ({}, {}, {:?}) conflicts with prior payload",
                    observation_id.partition_id,
                    observation_id.local_dir_edge_id,
                    observation_id.edge_key
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
        self.applied_face_twins.clear();
        self.reconciled_nodes.clear();
        self.global_components.clear();
        self.global_face_plans.clear();
        Ok(())
    }

    pub fn declare_adjacency(&mut self, adjacency: PartitionBorderAdjacency) {
        self.adjacencies.insert(adjacency);
        self.applied_face_twins.clear();
        self.reconciled_nodes.clear();
        self.global_components.clear();
        self.global_face_plans.clear();
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

    /// Reports the conservative declared-adjacency reconciliation boundary
    /// without mutating observations or choosing a Z/provenance policy.
    pub fn reconciliation_stats(&self) -> PartitionBorderReconciliationStats {
        let edges = self.normalized_edges();
        let matched_twin_count = self.twin_pairs_from_edges(&edges).len();
        PartitionBorderReconciliationStats {
            declared_adjacency_count: self.adjacencies.len(),
            normalized_edge_count: edges.len(),
            matched_twin_count,
            unmatched_edge_count: edges.len().saturating_sub(matched_twin_count),
        }
    }

    fn twin_payload_from_edges(
        &self,
        edges: &BTreeMap<PartitionBorderEdgeKey, BTreeSet<PartitionBorderHalfEdge>>,
        twin: PartitionBorderTwin,
    ) -> Option<PartitionBorderTwinPayload> {
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
            forward_representative_line_id: forward.representative_line_id,
            reverse_representative_line_id: reverse.representative_line_id,
            start_z_bits,
            end_z_bits,
        })
    }

    /// Applies only exact declared-adjacency twins whose two observations
    /// carry qualified, partition-matching local face references. The
    /// resulting links are retained on this exported graph for later global
    /// arrangement work; no local adjacency, output polygon, or Z policy is
    /// mutated here.
    pub(crate) fn apply_unambiguous_face_twins(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderTwinApplicationStats> {
        execution_policy.check_cancelled("partition_border_twin_application")?;
        let edges = self.normalized_edges();
        let twins = self.twin_pairs_from_edges(&edges);
        execution_policy.check(
            "partition_border_twin_applications",
            execution_policy.max_graph_edges,
            twins.len(),
        )?;
        let mut stats = PartitionBorderTwinApplicationStats {
            candidate_twin_count: twins.len(),
            ..Default::default()
        };
        let mut applied_face_twins = Vec::new();

        for (twin_index, twin) in twins.into_iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_twin_application", twin_index)?;
            let Some(observations) = edges.get(&twin.edge_key) else {
                stats.invalid_face_ref_count += 1;
                continue;
            };
            let Some(forward) = observations
                .iter()
                .find(|observation| observation.observation_id() == twin.forward)
            else {
                stats.invalid_face_ref_count += 1;
                continue;
            };
            let Some(reverse) = observations
                .iter()
                .find(|observation| observation.observation_id() == twin.reverse)
            else {
                stats.invalid_face_ref_count += 1;
                continue;
            };

            let (Some(forward_face_ref), Some(reverse_face_ref)) =
                (forward.face_ref, reverse.face_ref)
            else {
                stats.missing_face_ref_count += 1;
                continue;
            };
            if forward_face_ref.partition_id != forward.partition_id
                || reverse_face_ref.partition_id != reverse.partition_id
                || forward_face_ref.partition_id == reverse_face_ref.partition_id
            {
                stats.invalid_face_ref_count += 1;
                continue;
            }
            let Some(payload) = self.twin_payload_from_edges(&edges, twin) else {
                stats.invalid_face_ref_count += 1;
                continue;
            };
            applied_face_twins.push(PartitionBorderFaceTwin {
                twin,
                forward_face_ref,
                reverse_face_ref,
                payload,
            });
        }
        stats.applied_twin_count = applied_face_twins.len();
        self.applied_face_twins = applied_face_twins;
        self.global_components.clear();
        self.global_face_plans.clear();
        Ok(stats)
    }

    pub(crate) fn applied_face_twins(&self) -> &[PartitionBorderFaceTwin] {
        &self.applied_face_twins
    }

    /// Reconciles every exact border node without mutating observations or
    /// local topology. All source, representative, face, and Z evidence is
    /// retained. Non-ignored policies choose the first candidate under the
    /// existing representative/source ordering used by untiled graph
    /// construction; conflict detection remains explicit and
    /// `ErrorOnConflict` fails before the plan is committed.
    pub(crate) fn reconcile_border_nodes(
        &mut self,
        z_options: ZOptions,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderNodeReconciliationStats> {
        execution_policy.check_cancelled("partition_border_node_reconciliation")?;
        execution_policy.check(
            "partition_border_nodes",
            execution_policy.max_graph_nodes,
            self.nodes.len(),
        )?;

        let mut evidence = BTreeMap::<
            PartitionBorderNodeKey,
            (
                BTreeSet<PartitionBorderObservationId>,
                BTreeSet<u32>,
                BTreeSet<u32>,
                BTreeSet<PartitionFaceRef>,
                BTreeSet<u64>,
                Vec<(u32, PartitionBorderObservationId, u64)>,
            ),
        >::new();
        for (observation_index, observation) in self.observations.values().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_node_reconciliation", observation_index)?;
            let observation_id = observation.observation_id();
            for (key, z_bits) in [
                (observation.from, observation.from_z_bits),
                (observation.to, observation.to_z_bits),
            ] {
                let entry = evidence.entry(key).or_default();
                entry.0.insert(observation_id);
                entry.1.extend(observation.source_line_ids.iter().copied());
                if let Some(representative_line_id) = observation.representative_line_id {
                    entry.2.insert(representative_line_id);
                }
                if let Some(face_ref) = observation.face_ref {
                    entry.3.insert(face_ref);
                }
                entry.4.insert(z_bits);
                entry.5.push((
                    observation.representative_line_id.unwrap_or(u32::MAX),
                    observation_id,
                    z_bits,
                ));
            }
        }

        let mut reconciled_nodes = Vec::with_capacity(evidence.len());
        let mut stats = PartitionBorderNodeReconciliationStats {
            node_count: evidence.len(),
            ..Default::default()
        };
        for (
            node_index,
            (
                key,
                (
                    observation_ids,
                    source_line_ids,
                    representative_line_ids,
                    face_refs,
                    z_bits,
                    mut z_candidates,
                ),
            ),
        ) in evidence.into_iter().enumerate()
        {
            execution_policy
                .check_cancelled_every("partition_border_node_reconciliation", node_index)?;
            let mut z_bits = z_bits.into_iter().collect::<Vec<_>>();
            z_bits.sort_unstable_by(|left, right| {
                f64::from_bits(*left)
                    .total_cmp(&f64::from_bits(*right))
                    .then(left.cmp(right))
            });
            let min_z = z_bits
                .first()
                .map(|bits| f64::from_bits(*bits))
                .unwrap_or(0.0);
            let max_z = z_bits
                .last()
                .map(|bits| f64::from_bits(*bits))
                .unwrap_or(0.0);
            let z_conflict = max_z - min_z > z_options.conflict_tolerance;
            if z_conflict {
                stats.z_conflict_count += 1;
            }
            if z_conflict && matches!(z_options.policy, ZPolicy::ErrorOnConflict) {
                return Err(crate::PolygonizeError::ZConflict {
                    x: f64::from_bits(key.xy_bits()[0]),
                    y: f64::from_bits(key.xy_bits()[1]),
                    line_ids: source_line_ids.into_iter().collect(),
                });
            }
            let selected_z_bits = if matches!(z_options.policy, ZPolicy::Ignore) {
                canonical_coordinate_bits(0.0)
            } else {
                z_candidates.sort_unstable_by(|left, right| {
                    left.0
                        .cmp(&right.0)
                        .then(left.1.cmp(&right.1))
                        .then(f64::from_bits(left.2).total_cmp(&f64::from_bits(right.2)))
                        .then(left.2.cmp(&right.2))
                });
                z_candidates
                    .first()
                    .map(|candidate| candidate.2)
                    .unwrap_or_else(|| canonical_coordinate_bits(0.0))
            };
            reconciled_nodes.push(PartitionBorderNodePayload {
                key,
                observation_ids: observation_ids.into_iter().collect(),
                source_line_ids: source_line_ids.into_iter().collect(),
                representative_line_ids: representative_line_ids.into_iter().collect(),
                face_refs: face_refs.into_iter().collect(),
                z_bits,
                selected_z_bits,
                selected_z_policy: z_options.policy,
                conflict_tolerance_bits: canonical_coordinate_bits(z_options.conflict_tolerance),
                z_conflict,
            });
        }
        self.reconciled_nodes = reconciled_nodes;
        self.global_components.clear();
        self.global_face_plans.clear();
        Ok(stats)
    }

    pub(crate) fn reconciled_border_nodes(&self) -> &[PartitionBorderNodePayload] {
        &self.reconciled_nodes
    }

    /// Reconciles qualified face references into deterministic connected
    /// components using only retained exact twin links. Every face observed
    /// at a reconciled border node is included, so unlinked faces remain
    /// explicit singleton components instead of being silently dropped.
    /// Components are retained as evidence only; no local or tiled topology
    /// is mutated.
    pub(crate) fn reconcile_global_components(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalComponentReconciliationStats> {
        execution_policy.check_cancelled("partition_border_global_components")?;
        let mut face_set = BTreeSet::new();
        for (node_index, node) in self.reconciled_nodes.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_components", node_index)?;
            face_set.extend(node.face_refs.iter().copied());
        }
        face_set.extend(
            self.applied_face_twins
                .iter()
                .flat_map(|twin| [twin.forward_face_ref, twin.reverse_face_ref]),
        );
        execution_policy.check(
            "partition_border_global_faces",
            execution_policy.max_graph_nodes,
            face_set.len(),
        )?;
        execution_policy.check(
            "partition_border_global_links",
            execution_policy.max_graph_edges,
            self.applied_face_twins.len(),
        )?;

        let faces = face_set.into_iter().collect::<Vec<_>>();
        let face_indices = faces
            .iter()
            .enumerate()
            .map(|(index, face_ref)| (*face_ref, index))
            .collect::<BTreeMap<_, _>>();
        let mut parents = (0..faces.len()).collect::<Vec<_>>();
        fn find(parents: &mut [usize], mut index: usize) -> usize {
            while parents[index] != index {
                parents[index] = parents[parents[index]];
                index = parents[index];
            }
            index
        }
        let mut linked_faces = BTreeSet::new();
        for (twin_index, twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_components", twin_index)?;
            let Some(&forward_index) = face_indices.get(&twin.forward_face_ref) else {
                continue;
            };
            let Some(&reverse_index) = face_indices.get(&twin.reverse_face_ref) else {
                continue;
            };
            linked_faces.insert(twin.forward_face_ref);
            linked_faces.insert(twin.reverse_face_ref);
            let forward_root = find(&mut parents, forward_index);
            let reverse_root = find(&mut parents, reverse_index);
            if forward_root != reverse_root {
                let (root, child) = if forward_root < reverse_root {
                    (forward_root, reverse_root)
                } else {
                    (reverse_root, forward_root)
                };
                parents[child] = root;
            }
        }

        let mut groups = BTreeMap::<
            usize,
            (
                BTreeSet<PartitionFaceRef>,
                BTreeSet<PartitionBorderNodeKey>,
                BTreeSet<PartitionBorderEdgeKey>,
            ),
        >::new();
        for (face_index, face_ref) in faces.iter().copied().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_components", face_index)?;
            groups
                .entry(find(&mut parents, face_index))
                .or_default()
                .0
                .insert(face_ref);
        }
        for (node_index, node) in self.reconciled_nodes.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_components", node_index)?;
            let mut roots = BTreeSet::new();
            for face_ref in &node.face_refs {
                if let Some(&face_index) = face_indices.get(face_ref) {
                    roots.insert(find(&mut parents, face_index));
                }
            }
            for root in roots {
                groups.entry(root).or_default().1.insert(node.key);
            }
        }
        for (twin_index, twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_components", twin_index)?;
            let Some(&face_index) = face_indices.get(&twin.forward_face_ref) else {
                continue;
            };
            groups
                .entry(find(&mut parents, face_index))
                .or_default()
                .2
                .insert(twin.twin.edge_key);
        }

        let global_components = groups
            .into_iter()
            .enumerate()
            .map(
                |(component_index, (_root, (face_refs, border_node_keys, twin_edge_keys)))| {
                    PartitionBorderGlobalComponent {
                        component_index,
                        face_refs: face_refs.into_iter().collect(),
                        border_node_keys: border_node_keys.into_iter().collect(),
                        twin_edge_keys: twin_edge_keys.into_iter().collect(),
                    }
                },
            )
            .collect::<Vec<_>>();
        let stats = PartitionBorderGlobalComponentReconciliationStats {
            component_count: global_components.len(),
            face_count: faces.len(),
            linked_face_count: linked_faces.len(),
            twin_link_count: self.applied_face_twins.len(),
        };
        self.global_components = global_components;
        self.global_face_plans.clear();
        Ok(stats)
    }

    pub(crate) fn global_components(&self) -> &[PartitionBorderGlobalComponent] {
        &self.global_components
    }

    /// Builds deterministic face-boundary evidence from qualified local
    /// observations and their local face-walk successors. Missing successors
    /// are reported and excluded from the plan; no local `next`, face ID, or
    /// tiled output is mutated.
    pub(crate) fn reconcile_global_face_plans(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFacePlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_plan")?;

        let mut candidates = Vec::new();
        let mut missing_successor_count = 0;
        let mut face_unbounded = BTreeMap::<PartitionFaceRef, bool>::new();
        for (observation_index, observation) in self.observations.values().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_plan", observation_index)?;
            let Some(face_ref) = observation.face_ref else {
                continue;
            };
            face_unbounded
                .entry(face_ref)
                .and_modify(|is_unbounded| *is_unbounded |= observation.local_face_is_unbounded)
                .or_insert(observation.local_face_is_unbounded);
            let Some(local_face_successor) = observation.local_face_successor else {
                missing_successor_count += 1;
                continue;
            };
            candidates.push(PartitionBorderFaceBoundaryCandidate {
                observation_id: observation.observation_id(),
                edge_key: observation.edge_key,
                face_ref,
                local_dir_edge_id: observation.local_dir_edge_id,
                local_face_successor,
                local_face_is_unbounded: observation.local_face_is_unbounded,
            });
        }
        execution_policy.check(
            "partition_border_global_face_candidates",
            execution_policy.max_graph_edges,
            candidates.len(),
        )?;

        let mut face_groups = BTreeMap::<
            PartitionFaceRef,
            (
                BTreeSet<PartitionBorderFaceBoundaryCandidate>,
                BTreeSet<PartitionBorderEdgeKey>,
                bool,
            ),
        >::new();
        for candidate in candidates {
            execution_policy
                .check_cancelled_every("partition_border_global_face_plan", face_groups.len())?;
            let group = face_groups.entry(candidate.face_ref).or_default();
            group.0.insert(candidate);
            group.2 |= candidate.local_face_is_unbounded;
        }

        for (face_ref, is_unbounded) in face_unbounded {
            face_groups.entry(face_ref).or_default().2 |= is_unbounded;
        }

        for (twin_index, twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_plan", twin_index)?;
            face_groups
                .entry(twin.forward_face_ref)
                .or_default()
                .1
                .insert(twin.twin.edge_key);
            face_groups
                .entry(twin.reverse_face_ref)
                .or_default()
                .1
                .insert(twin.twin.edge_key);
        }

        execution_policy.check(
            "partition_border_global_faces",
            execution_policy.max_graph_nodes,
            face_groups.len(),
        )?;
        let mut global_face_plans = Vec::with_capacity(face_groups.len());
        let mut unbounded_face_count = 0;
        let mut linked_face_count = 0;
        for (face_ref, (candidates, twin_edge_keys, local_face_is_unbounded)) in face_groups {
            if local_face_is_unbounded {
                unbounded_face_count += 1;
            }
            if !twin_edge_keys.is_empty() {
                linked_face_count += 1;
            }
            global_face_plans.push(PartitionBorderGlobalFacePlan {
                face_ref,
                candidates: candidates.into_iter().collect(),
                twin_edge_keys: twin_edge_keys.into_iter().collect(),
                local_face_is_unbounded,
            });
        }
        let stats = PartitionBorderGlobalFacePlanStats {
            face_count: global_face_plans.len(),
            candidate_count: global_face_plans
                .iter()
                .map(|plan| plan.candidates.len())
                .sum(),
            missing_successor_count,
            unbounded_face_count,
            linked_face_count,
        };
        self.global_face_plans = global_face_plans;
        Ok(stats)
    }

    pub(crate) fn global_face_plans(&self) -> &[PartitionBorderGlobalFacePlan] {
        &self.global_face_plans
    }

    /// Validates retained face-boundary evidence without assigning global
    /// `next` links or face IDs. Every candidate must still resolve to its
    /// immutable observation, and every retained twin edge must connect
    /// exactly two qualified face plans.
    pub(crate) fn validate_global_face_plans(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFacePlanValidationStats> {
        execution_policy.check_cancelled("partition_border_global_face_validation")?;
        let candidate_count = self
            .global_face_plans
            .iter()
            .map(|plan| plan.candidates.len())
            .sum::<usize>();
        execution_policy.check(
            "partition_border_global_face_validation_faces",
            execution_policy.max_graph_nodes,
            self.global_face_plans.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_validation_candidates",
            execution_policy.max_graph_edges,
            candidate_count,
        )?;
        execution_policy.check(
            "partition_border_global_face_validation_twins",
            execution_policy.max_graph_edges,
            self.applied_face_twins.len(),
        )?;

        let mut observed_face_unbounded = BTreeMap::<PartitionFaceRef, bool>::new();
        for (observation_index, observation) in self.observations.values().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_validation_observations",
                observation_index,
            )?;
            let Some(face_ref) = observation.face_ref else {
                continue;
            };
            observed_face_unbounded
                .entry(face_ref)
                .and_modify(|is_unbounded| *is_unbounded |= observation.local_face_is_unbounded)
                .or_insert(observation.local_face_is_unbounded);
        }

        let mut plan_indices = BTreeMap::<PartitionFaceRef, usize>::new();
        let mut candidate_observation_ids = BTreeSet::<PartitionBorderObservationId>::new();
        let mut unbounded_face_count = 0;
        let mut previous_face_ref = None;
        for (plan_index, plan) in self.global_face_plans.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_validation", plan_index)?;
            if previous_face_ref.is_some_and(|previous| previous >= plan.face_ref) {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face plans are not strictly ordered at face {:?}",
                        plan.face_ref
                    ),
                });
            }
            previous_face_ref = Some(plan.face_ref);
            if plan_indices.insert(plan.face_ref, plan_index).is_some() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!("global face plan {:?} is duplicated", plan.face_ref),
                });
            }
            let Some(&observed_is_unbounded) = observed_face_unbounded.get(&plan.face_ref) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face plan {:?} has no qualified observation",
                        plan.face_ref
                    ),
                });
            };
            if plan.local_face_is_unbounded != observed_is_unbounded {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face plan {:?} has inconsistent unbounded-face evidence",
                        plan.face_ref
                    ),
                });
            }
            if plan.local_face_is_unbounded {
                unbounded_face_count += 1;
            }

            let mut previous_candidate = None;
            let mut previous_twin_edge_key = None;
            for (candidate_index, candidate) in plan.candidates.iter().enumerate() {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_validation_candidates",
                    candidate_index,
                )?;
                if previous_candidate.is_some_and(|previous| previous >= *candidate) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face plan {:?} candidates are not strictly ordered",
                            plan.face_ref
                        ),
                    });
                }
                previous_candidate = Some(*candidate);
                if !candidate_observation_ids.insert(candidate.observation_id) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face boundary observation {:?} is duplicated",
                            candidate.observation_id
                        ),
                    });
                }
                let Some(observation) = self.observations.get(&candidate.observation_id) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face boundary observation {:?} is missing",
                            candidate.observation_id
                        ),
                    });
                };
                if observation.edge_key != candidate.edge_key
                    || candidate.face_ref != plan.face_ref
                    || observation.face_ref != Some(candidate.face_ref)
                    || observation.local_dir_edge_id != candidate.local_dir_edge_id
                    || observation.local_face_successor != Some(candidate.local_face_successor)
                    || observation.local_face_is_unbounded != candidate.local_face_is_unbounded
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face boundary candidate {:?} disagrees with its observation",
                            candidate.observation_id
                        ),
                    });
                }
            }
            for edge_key in &plan.twin_edge_keys {
                if previous_twin_edge_key.is_some_and(|previous| previous >= *edge_key) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face plan {:?} twin edges are not strictly ordered",
                            plan.face_ref
                        ),
                    });
                }
                previous_twin_edge_key = Some(*edge_key);
            }
        }

        let mut twin_faces = BTreeMap::<PartitionBorderEdgeKey, BTreeSet<PartitionFaceRef>>::new();
        for (twin_index, applied_twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_validation_twins",
                twin_index,
            )?;
            if applied_twin.forward_face_ref == applied_twin.reverse_face_ref {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin {:?} joins a face to itself",
                        applied_twin.twin.edge_key
                    ),
                });
            }
            let Some(forward) = self.observations.get(&applied_twin.twin.forward) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin {:?} has a missing forward observation",
                        applied_twin.twin.edge_key
                    ),
                });
            };
            let Some(reverse) = self.observations.get(&applied_twin.twin.reverse) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin {:?} has a missing reverse observation",
                        applied_twin.twin.edge_key
                    ),
                });
            };
            if forward.observation_id() != applied_twin.twin.forward
                || reverse.observation_id() != applied_twin.twin.reverse
                || forward.edge_key != applied_twin.twin.edge_key
                || reverse.edge_key != applied_twin.twin.edge_key
                || forward.face_ref != Some(applied_twin.forward_face_ref)
                || reverse.face_ref != Some(applied_twin.reverse_face_ref)
                || forward.from != reverse.to
                || forward.to != reverse.from
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin {:?} disagrees with its observations",
                        applied_twin.twin.edge_key
                    ),
                });
            }
            let faces = twin_faces.entry(applied_twin.twin.edge_key).or_default();
            if !faces.insert(applied_twin.forward_face_ref)
                || !faces.insert(applied_twin.reverse_face_ref)
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin edge {:?} is duplicated",
                        applied_twin.twin.edge_key
                    ),
                });
            }
        }

        for plan in &self.global_face_plans {
            for edge_key in &plan.twin_edge_keys {
                let Some(faces) = twin_faces.get(edge_key) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face plan {:?} references an unapplied twin edge {:?}",
                            plan.face_ref, edge_key
                        ),
                    });
                };
                if !faces.contains(&plan.face_ref) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face plan {:?} does not belong to twin edge {:?}",
                            plan.face_ref, edge_key
                        ),
                    });
                }
            }
        }
        for (edge_key, faces) in &twin_faces {
            if faces.len() != 2 {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin edge {:?} connects {} face plans",
                        edge_key,
                        faces.len()
                    ),
                });
            }
            for face_ref in faces {
                let Some(&plan_index) = plan_indices.get(face_ref) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face twin edge {:?} references missing face plan {:?}",
                            edge_key, face_ref
                        ),
                    });
                };
                if !self.global_face_plans[plan_index]
                    .twin_edge_keys
                    .contains(edge_key)
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face twin edge {:?} is absent from face plan {:?}",
                            edge_key, face_ref
                        ),
                    });
                }
            }
        }

        Ok(PartitionBorderGlobalFacePlanValidationStats {
            face_count: self.global_face_plans.len(),
            candidate_count,
            twin_link_count: twin_faces.len(),
            unbounded_face_count,
        })
    }

    /// Merges source IDs and retains every distinct Z candidate for each
    /// canonical endpoint. No Z conflict policy is applied here.
    pub fn reconcile_twin_payloads(&self) -> Vec<PartitionBorderTwinPayload> {
        let edges = self.normalized_edges();
        self.twin_pairs_from_edges(&edges)
            .into_iter()
            .filter_map(|twin| self.twin_payload_from_edges(&edges, twin))
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
    use crate::CancellationToken;

    fn coord(x: f64, y: f64, z: f64) -> Coord3D {
        Coord3D::new(x, y, z)
    }

    fn unit_bbox() -> Rect<f64> {
        Rect::new(
            geo_types::Coord { x: 0.0, y: 0.0 },
            geo_types::Coord { x: 10.0, y: 10.0 },
        )
    }

    #[test]
    fn boundary_intersections_cover_crossings_endpoints_and_corners() {
        let bbox = unit_bbox();
        let vertical =
            partition_boundary_intersections(coord(-1.0, 2.0, 0.0), coord(11.0, 2.0, 12.0), bbox);
        assert_eq!(
            vertical
                .iter()
                .map(|intersection| (
                    intersection.side,
                    intersection.point.x,
                    intersection.point.y
                ))
                .collect::<Vec<_>>(),
            vec![
                (PartitionBorderSide::MinX, 0.0, 2.0),
                (PartitionBorderSide::MaxX, 10.0, 2.0),
            ]
        );
        assert!(vertical[0].t < vertical[1].t);

        let horizontal =
            partition_boundary_intersections(coord(2.0, -1.0, 0.0), coord(2.0, 11.0, 12.0), bbox);
        assert_eq!(
            horizontal
                .iter()
                .map(|intersection| (
                    intersection.side,
                    intersection.point.x,
                    intersection.point.y
                ))
                .collect::<Vec<_>>(),
            vec![
                (PartitionBorderSide::MinY, 2.0, 0.0),
                (PartitionBorderSide::MaxY, 2.0, 10.0),
            ]
        );

        let endpoint =
            partition_boundary_intersections(coord(0.0, 2.0, 3.0), coord(5.0, 3.0, 8.0), bbox);
        assert_eq!(endpoint.len(), 1);
        assert_eq!(endpoint[0].side, PartitionBorderSide::MinX);
        assert_eq!(endpoint[0].t, 0.0);
        assert_eq!(endpoint[0].point.z, 3.0);

        let corner =
            partition_boundary_intersections(coord(-1.0, -1.0, 0.0), coord(11.0, 11.0, 12.0), bbox);
        assert_eq!(corner.len(), 4);
        assert_eq!(
            corner
                .iter()
                .filter(|intersection| intersection.t == corner[0].t)
                .map(|intersection| intersection.side)
                .collect::<Vec<_>>(),
            vec![PartitionBorderSide::MinX, PartitionBorderSide::MinY]
        );
        assert_eq!(
            corner
                .iter()
                .filter(|intersection| intersection.t == corner[2].t)
                .map(|intersection| intersection.side)
                .collect::<Vec<_>>(),
            vec![PartitionBorderSide::MaxX, PartitionBorderSide::MaxY]
        );
    }

    #[test]
    fn boundary_intersections_split_collinear_edges_at_finite_side_endpoints() {
        let intersections = partition_boundary_intersections(
            coord(-2.0, 0.0, 1.0),
            coord(12.0, 0.0, 15.0),
            unit_bbox(),
        );
        let min_y = intersections
            .iter()
            .filter(|intersection| intersection.side == PartitionBorderSide::MinY)
            .collect::<Vec<_>>();
        assert_eq!(min_y.len(), 2);
        assert_eq!(min_y[0].point, coord(0.0, 0.0, 3.0));
        assert_eq!(min_y[1].point, coord(10.0, 0.0, 13.0));
        assert!(min_y[0].t < min_y[1].t);
    }

    #[test]
    fn boundary_intersections_normalize_signed_zero_and_reverse_deterministically() {
        let bbox = Rect::new(
            geo_types::Coord { x: -0.0, y: -1.0 },
            geo_types::Coord { x: 1.0, y: 1.0 },
        );
        let forward =
            partition_boundary_intersections(coord(-1.0, -0.0, 0.0), coord(1.0, 0.0, 2.0), bbox);
        let reverse =
            partition_boundary_intersections(coord(1.0, 0.0, 2.0), coord(-1.0, -0.0, 0.0), bbox);
        let forward_min_x = forward
            .iter()
            .find(|intersection| intersection.side == PartitionBorderSide::MinX)
            .unwrap();
        let reverse_min_x = reverse
            .iter()
            .find(|intersection| intersection.side == PartitionBorderSide::MinX)
            .unwrap();
        assert_eq!(canonical_coordinate_bits(forward_min_x.point.x), 0);
        assert_eq!(forward_min_x.point, reverse_min_x.point);
        assert_eq!(forward_min_x.t + reverse_min_x.t, 1.0);
    }

    fn face(partition_id: usize, component_id: usize, face_id: usize) -> PartitionFaceRef {
        PartitionFaceRef {
            partition_id,
            component_id,
            face_id,
        }
    }

    fn exact_face_twin_graph() -> PartitionBorderGraph {
        let forward = PartitionBorderHalfEdge::new_with_face_ref(
            1,
            7,
            Some(face(1, 4, 9)),
            PartitionBorderSide::MinY,
            coord(0.0, 0.0, 1.0),
            coord(2.0, 0.0, 2.0),
            [8],
        )
        .unwrap();
        let reverse = PartitionBorderHalfEdge::new_with_face_ref(
            2,
            9,
            Some(face(2, 6, 11)),
            PartitionBorderSide::MaxY,
            coord(2.0, 0.0, 20.0),
            coord(0.0, 0.0, 10.0),
            [7],
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
        graph.insert(reverse).unwrap();
        graph.insert(forward).unwrap();
        graph
    }

    fn exact_face_twin_graph_with_successors() -> PartitionBorderGraph {
        let mut graph = exact_face_twin_graph();
        for observation in graph.observations.values_mut() {
            observation.local_face_successor = Some(observation.local_dir_edge_id + 100);
            observation.local_face_is_unbounded = false;
        }
        graph
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
        assert_eq!(
            observations.iter().next().unwrap().representative_line_id,
            Some(2)
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
        assert!(graph
            .insert(conflict)
            .unwrap_err()
            .to_string()
            .contains("partition border observation (4, 7, "));
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn atomic_observation_identity_allows_distinct_spans_from_one_local_edge_id() {
        let first = PartitionBorderHalfEdge::new(
            4,
            7,
            Some(3),
            PartitionBorderSide::MinY,
            coord(0.0, 0.0, 1.0),
            coord(1.0, 0.0, 2.0),
            [11],
        )
        .unwrap();
        let second = PartitionBorderHalfEdge::new(
            4,
            7,
            Some(3),
            PartitionBorderSide::MinY,
            coord(1.0, 0.0, 2.0),
            coord(2.0, 0.0, 3.0),
            [11],
        )
        .unwrap();
        assert_ne!(first.observation_id(), second.observation_id());

        let mut graph = PartitionBorderGraph::default();
        graph.insert(first).unwrap();
        graph.insert(second).unwrap();
        assert_eq!(graph.edge_count(), 2);
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
                    edge_key: key,
                },
                reverse: PartitionBorderObservationId {
                    partition_id: 2,
                    local_dir_edge_id: 9,
                    edge_key: key,
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
        let low_key = low.edge_key;
        let high_key = high.edge_key;

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
                        edge_key: low_key,
                    },
                    PartitionBorderObservationId {
                        partition_id: 2,
                        local_dir_edge_id: 10,
                        edge_key: low_key,
                    },
                ),
                (
                    PartitionBorderObservationId {
                        partition_id: 1,
                        local_dir_edge_id: 7,
                        edge_key: high_key,
                    },
                    PartitionBorderObservationId {
                        partition_id: 2,
                        local_dir_edge_id: 9,
                        edge_key: high_key,
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
        assert_eq!(
            graph.reconciliation_stats(),
            PartitionBorderReconciliationStats {
                declared_adjacency_count: 1,
                normalized_edge_count: 0,
                matched_twin_count: 0,
                unmatched_edge_count: 0,
            }
        );
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
            graph.reconciliation_stats(),
            PartitionBorderReconciliationStats {
                declared_adjacency_count: 1,
                normalized_edge_count: 1,
                matched_twin_count: 1,
                unmatched_edge_count: 0,
            }
        );

        assert_eq!(
            graph.reconcile_twin_payloads(),
            vec![PartitionBorderTwinPayload {
                twin,
                source_line_ids: vec![2, 4, 8],
                forward_representative_line_id: Some(2),
                reverse_representative_line_id: Some(4),
                start_z_bits: vec![0, 1.0f64.to_bits()],
                end_z_bits: vec![2.0f64.to_bits()],
            }]
        );
    }

    #[test]
    fn face_twin_application_retains_qualified_faces_and_payload_lineage() {
        let forward = PartitionBorderHalfEdge::new_with_face_ref(
            1,
            7,
            Some(face(1, 4, 9)),
            PartitionBorderSide::MinY,
            coord(0.0, 0.0, 1.0),
            coord(2.0, 0.0, 2.0),
            [8, 3],
        )
        .unwrap();
        let reverse = PartitionBorderHalfEdge::new_with_face_ref(
            2,
            9,
            Some(face(2, 6, 11)),
            PartitionBorderSide::MaxY,
            coord(2.0, 0.0, 20.0),
            coord(0.0, 0.0, 10.0),
            [7, 3],
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

        let stats = graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderTwinApplicationStats {
                candidate_twin_count: 1,
                applied_twin_count: 1,
                missing_face_ref_count: 0,
                invalid_face_ref_count: 0,
            }
        );
        assert_eq!(graph.applied_face_twins().len(), 1);
        let application = &graph.applied_face_twins()[0];
        assert_eq!(application.twin, twin);
        assert_eq!(application.forward_face_ref, face(1, 4, 9));
        assert_eq!(application.reverse_face_ref, face(2, 6, 11));
        assert_eq!(application.payload.source_line_ids, vec![3, 7, 8]);
        assert_eq!(application.payload.forward_representative_line_id, Some(3));
        assert_eq!(application.payload.reverse_representative_line_id, Some(3));
        assert_eq!(
            application.payload.start_z_bits,
            vec![1.0f64.to_bits(), 10.0f64.to_bits()]
        );
        assert_eq!(
            application.payload.end_z_bits,
            vec![2.0f64.to_bits(), 20.0f64.to_bits()]
        );
    }

    #[test]
    fn face_twin_application_declines_missing_face_identity_without_mutating_observations() {
        let forward = PartitionBorderHalfEdge::new_with_face_ref(
            1,
            7,
            None,
            PartitionBorderSide::MinY,
            coord(0.0, 0.0, 1.0),
            coord(2.0, 0.0, 2.0),
            [8],
        )
        .unwrap();
        let reverse = PartitionBorderHalfEdge::new_with_face_ref(
            2,
            9,
            Some(face(2, 6, 11)),
            PartitionBorderSide::MaxY,
            coord(2.0, 0.0, 20.0),
            coord(0.0, 0.0, 10.0),
            [7],
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
        graph.insert(forward).unwrap();
        graph.insert(reverse).unwrap();
        let before = graph.clone();

        let stats = graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.candidate_twin_count, 1);
        assert_eq!(stats.applied_twin_count, 0);
        assert_eq!(stats.missing_face_ref_count, 1);
        assert_eq!(stats.invalid_face_ref_count, 0);
        assert!(graph.applied_face_twins().is_empty());
        assert_eq!(graph.observations, before.observations);
        assert_eq!(graph.edges, before.edges);
    }

    #[test]
    fn face_twin_application_declines_malformed_face_partition_identity() {
        let forward = PartitionBorderHalfEdge::new_with_face_ref(
            1,
            7,
            Some(face(99, 4, 9)),
            PartitionBorderSide::MinY,
            coord(0.0, 0.0, 1.0),
            coord(2.0, 0.0, 2.0),
            [8],
        )
        .unwrap();
        let reverse = PartitionBorderHalfEdge::new_with_face_ref(
            2,
            9,
            Some(face(2, 6, 11)),
            PartitionBorderSide::MaxY,
            coord(2.0, 0.0, 20.0),
            coord(0.0, 0.0, 10.0),
            [7],
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
        graph.insert(reverse).unwrap();
        graph.insert(forward).unwrap();

        let stats = graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.candidate_twin_count, 1);
        assert_eq!(stats.applied_twin_count, 0);
        assert_eq!(stats.missing_face_ref_count, 0);
        assert_eq!(stats.invalid_face_ref_count, 1);
        assert!(graph.applied_face_twins().is_empty());
    }

    #[test]
    fn face_twin_application_is_bounded_and_cancellable_before_mutation() {
        let mut limited = exact_face_twin_graph();
        let before = limited.clone();
        let error = limited
            .apply_unambiguous_face_twins(&ExecutionPolicy {
                max_graph_edges: Some(0),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 1,
            } if stage == "partition_border_twin_applications"
        ));
        assert_eq!(limited, before);

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_twin_graph();
        let before = cancelled.clone();
        let error = cancelled
            .apply_unambiguous_face_twins(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_twin_application"
        ));
        assert_eq!(cancelled, before);
    }

    #[test]
    fn border_node_reconciliation_unions_identity_and_retains_z_candidates() {
        let mut graph = exact_face_twin_graph();
        let stats = graph
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();

        assert_eq!(
            stats,
            PartitionBorderNodeReconciliationStats {
                node_count: 2,
                z_conflict_count: 2,
            }
        );
        assert_eq!(graph.reconciled_border_nodes().len(), 2);
        let start_key = PartitionBorderNodeKey::from_coord(coord(0.0, 0.0, 0.0));
        let start = graph
            .reconciled_border_nodes()
            .iter()
            .find(|node| node.key == start_key)
            .unwrap();
        assert_eq!(start.source_line_ids, vec![7, 8]);
        assert_eq!(start.representative_line_ids, vec![7, 8]);
        assert_eq!(start.face_refs, vec![face(1, 4, 9), face(2, 6, 11)]);
        assert_eq!(start.observation_ids.len(), 2);
        assert_eq!(start.z_bits, vec![1.0f64.to_bits(), 10.0f64.to_bits()]);
        assert_eq!(start.selected_z_bits, 10.0f64.to_bits());
        assert_eq!(start.selected_z_policy, ZPolicy::InterpolateAlongEdge);
        assert_eq!(start.conflict_tolerance_bits, 0);
        assert!(start.z_conflict);
    }

    #[test]
    fn border_node_reconciliation_applies_ignore_and_fails_error_on_conflict_closed() {
        let mut ignored = exact_face_twin_graph();
        ignored
            .reconcile_border_nodes(
                ZOptions {
                    policy: ZPolicy::Ignore,
                    ..Default::default()
                },
                &ExecutionPolicy::default(),
            )
            .unwrap();
        assert!(ignored
            .reconciled_border_nodes()
            .iter()
            .all(|node| node.selected_z_bits == 0));
        assert!(ignored
            .reconciled_border_nodes()
            .iter()
            .all(|node| node.z_conflict));

        let mut error_on_conflict = exact_face_twin_graph();
        let before = error_on_conflict.clone();
        let error = error_on_conflict
            .reconcile_border_nodes(
                ZOptions {
                    policy: ZPolicy::ErrorOnConflict,
                    ..Default::default()
                },
                &ExecutionPolicy::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ZConflict {
                x,
                y,
                line_ids,
            } if x == 0.0 && y == 0.0 && line_ids == vec![7, 8]
        ));
        assert_eq!(error_on_conflict, before);
        assert!(error_on_conflict.reconciled_border_nodes().is_empty());
    }

    #[test]
    fn border_node_reconciliation_is_bounded_and_cancellable_before_mutation() {
        let mut limited = exact_face_twin_graph();
        let before = limited.clone();
        let error = limited
            .reconcile_border_nodes(
                ZOptions::default(),
                &ExecutionPolicy {
                    max_graph_nodes: Some(1),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            } if stage == "partition_border_nodes"
        ));
        assert_eq!(limited, before);

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_twin_graph();
        let before = cancelled.clone();
        let error = cancelled
            .reconcile_border_nodes(
                ZOptions::default(),
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_node_reconciliation"
        ));
        assert_eq!(cancelled, before);
    }

    #[test]
    fn global_component_reconciliation_unions_qualified_faces_deterministically() {
        let mut graph = exact_face_twin_graph();
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let stats = graph
            .reconcile_global_components(&ExecutionPolicy::default())
            .unwrap();

        assert_eq!(
            stats,
            PartitionBorderGlobalComponentReconciliationStats {
                component_count: 1,
                face_count: 2,
                linked_face_count: 2,
                twin_link_count: 1,
            }
        );
        assert_eq!(graph.global_components().len(), 1);
        let component = &graph.global_components()[0];
        assert_eq!(component.component_index, 0);
        assert_eq!(component.face_refs, vec![face(1, 4, 9), face(2, 6, 11)]);
        assert_eq!(component.border_node_keys.len(), 2);
        assert_eq!(
            component.twin_edge_keys,
            vec![PartitionBorderEdgeKey::new(
                PartitionBorderNodeKey::from_coord(coord(0.0, 0.0, 0.0)),
                PartitionBorderNodeKey::from_coord(coord(2.0, 0.0, 0.0)),
            )
            .unwrap(),]
        );

        let mut reordered = PartitionBorderGraph::default();
        for adjacency in graph.adjacencies.iter().copied() {
            reordered.declare_adjacency(adjacency);
        }
        for observation in graph.observations.values().rev().cloned() {
            reordered.insert(observation).unwrap();
        }
        reordered
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_components(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(reordered.global_components(), graph.global_components());

        let mut singletons = exact_face_twin_graph();
        singletons
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let singleton_stats = singletons
            .reconcile_global_components(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(singleton_stats.component_count, 2);
        assert_eq!(singleton_stats.linked_face_count, 0);
        assert!(singletons
            .global_components()
            .iter()
            .all(|component| component.twin_edge_keys.is_empty()));
    }

    #[test]
    fn global_component_reconciliation_is_bounded_and_cancellable_before_mutation() {
        let mut limited = exact_face_twin_graph();
        limited
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        limited
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let before = limited.clone();
        let error = limited
            .reconcile_global_components(&ExecutionPolicy {
                max_graph_nodes: Some(1),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            } if stage == "partition_border_global_faces"
        ));
        assert_eq!(limited, before);

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_twin_graph();
        let before = cancelled.clone();
        let error = cancelled
            .reconcile_global_components(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_components"
        ));
        assert_eq!(cancelled, before);
    }

    #[test]
    fn global_face_plan_retains_local_successors_and_twin_edges_deterministically() {
        let mut graph = exact_face_twin_graph_with_successors();
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        let stats = graph
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();

        assert_eq!(
            stats,
            PartitionBorderGlobalFacePlanStats {
                face_count: 2,
                candidate_count: 2,
                missing_successor_count: 0,
                unbounded_face_count: 0,
                linked_face_count: 2,
            }
        );
        assert_eq!(graph.global_face_plans().len(), 2);
        assert!(graph.global_face_plans().iter().all(|plan| {
            plan.candidates.len() == 1
                && plan.twin_edge_keys.len() == 1
                && plan.candidates[0].local_face_successor >= 100
                && !plan.local_face_is_unbounded
        }));

        let mut reordered = PartitionBorderGraph::default();
        for adjacency in graph.adjacencies.iter().copied() {
            reordered.declare_adjacency(adjacency);
        }
        for observation in graph.observations.values().rev().cloned() {
            reordered.insert(observation).unwrap();
        }
        reordered
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(reordered.global_face_plans(), graph.global_face_plans());
    }

    #[test]
    fn global_face_plan_reports_missing_successors_and_fails_closed_on_limits() {
        let mut missing = exact_face_twin_graph();
        missing
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        let stats = missing
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.face_count, 2);
        assert_eq!(stats.candidate_count, 0);
        assert_eq!(stats.missing_successor_count, 2);
        assert_eq!(stats.linked_face_count, 2);
        assert!(missing
            .global_face_plans()
            .iter()
            .all(|plan| plan.candidates.is_empty()));

        for observation in missing.observations.values_mut() {
            observation.local_face_is_unbounded = true;
        }
        let stats = missing
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.unbounded_face_count, 2);
        assert!(missing
            .global_face_plans()
            .iter()
            .all(|plan| plan.local_face_is_unbounded));

        let mut limited = exact_face_twin_graph_with_successors();
        let before = limited.clone();
        let error = limited
            .reconcile_global_face_plans(&ExecutionPolicy {
                max_graph_nodes: Some(1),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            } if stage == "partition_border_global_faces"
        ));
        assert_eq!(limited, before);

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_twin_graph_with_successors();
        let before = cancelled.clone();
        let error = cancelled
            .reconcile_global_face_plans(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_plan"
        ));
        assert_eq!(cancelled, before);
    }

    #[test]
    fn global_face_plan_validation_is_deterministic_and_preserves_graph_state() {
        let mut graph = exact_face_twin_graph_with_successors();
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = graph.clone();
        let stats = graph
            .validate_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFacePlanValidationStats {
                face_count: 2,
                candidate_count: 2,
                twin_link_count: 1,
                unbounded_face_count: 0,
            }
        );
        assert_eq!(graph, before);
        assert_eq!(
            graph
                .validate_global_face_plans(&ExecutionPolicy::default())
                .unwrap(),
            stats
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn global_face_plan_validation_rejects_lineage_and_twin_corruption() {
        let mut candidate_corruption = exact_face_twin_graph_with_successors();
        candidate_corruption
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        candidate_corruption
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        candidate_corruption.global_face_plans[0].candidates[0].local_face_successor += 1;
        let before = candidate_corruption.clone();
        let error = candidate_corruption
            .validate_global_face_plans(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("disagrees with its observation")
        ));
        assert_eq!(candidate_corruption, before);

        let mut twin_corruption = exact_face_twin_graph_with_successors();
        twin_corruption
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        twin_corruption
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        twin_corruption.global_face_plans[0].twin_edge_keys.clear();
        let before = twin_corruption.clone();
        let error = twin_corruption
            .validate_global_face_plans(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("absent from face plan")
        ));
        assert_eq!(twin_corruption, before);
    }

    #[test]
    fn global_face_plan_validation_is_bounded_and_cancellable_before_mutation() {
        let mut limited_faces = exact_face_twin_graph_with_successors();
        limited_faces
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        limited_faces
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = limited_faces.clone();
        let error = limited_faces
            .validate_global_face_plans(&ExecutionPolicy {
                max_graph_nodes: Some(1),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            } if stage == "partition_border_global_face_validation_faces"
        ));
        assert_eq!(limited_faces, before);

        let mut limited_candidates = exact_face_twin_graph_with_successors();
        limited_candidates
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        limited_candidates
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = limited_candidates.clone();
        let error = limited_candidates
            .validate_global_face_plans(&ExecutionPolicy {
                max_graph_edges: Some(1),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            } if stage == "partition_border_global_face_validation_candidates"
        ));
        assert_eq!(limited_candidates, before);

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_twin_graph_with_successors();
        cancelled
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        cancelled
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = cancelled.clone();
        let error = cancelled
            .validate_global_face_plans(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_validation"
        ));
        assert_eq!(cancelled, before);
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
