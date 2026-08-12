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
    /// The first retained border half-edge reached by following this local
    /// face cycle, when the boundary continuation can be resolved.
    pub(crate) local_face_boundary_successor: Option<PartitionBorderObservationId>,
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
            local_face_boundary_successor: None,
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

/// Deterministic source, representative, and Z payload merged across the
/// reconciled border nodes of one retained global component. This is boundary
/// payload evidence only; it does not choose a global face or rewrite a node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalComponentPayload {
    pub(crate) component_index: usize,
    pub(crate) face_refs: Vec<PartitionFaceRef>,
    pub(crate) border_node_keys: Vec<PartitionBorderNodeKey>,
    pub(crate) source_line_ids: Vec<u32>,
    pub(crate) representative_line_ids: Vec<u32>,
    pub(crate) z_bits: Vec<u64>,
    pub(crate) selected_z_bits: Vec<(PartitionBorderNodeKey, u64)>,
    pub(crate) selected_z_policy: ZPolicy,
    pub(crate) z_conflict_node_count: usize,
}

/// Counts from retaining deterministic global-component border payloads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalComponentPayloadStats {
    pub(crate) component_count: usize,
    pub(crate) source_line_count: usize,
    pub(crate) representative_line_count: usize,
    pub(crate) z_candidate_count: usize,
    pub(crate) selected_z_node_count: usize,
    pub(crate) z_conflict_node_count: usize,
    pub(crate) z_conflict_component_count: usize,
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
    pub(crate) local_face_boundary_successor: Option<PartitionBorderObservationId>,
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
    pub(crate) missing_boundary_successor_count: usize,
}

/// Counts from validating the retained global face-boundary plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFacePlanValidationStats {
    pub(crate) face_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) twin_link_count: usize,
    pub(crate) unbounded_face_count: usize,
}

/// Counts from the mutation gate for local face-boundary transitions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceMutationGateStats {
    pub(crate) face_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) boundary_transition_count: usize,
    pub(crate) missing_boundary_successor_count: usize,
    pub(crate) mutation_ready_face_count: usize,
}

/// One deterministic local boundary cycle retained as input to a future
/// global face mutation. The ordered observation IDs are evidence only; this
/// record does not assign global directed-edge links or face IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceTransitionPlan {
    pub(crate) face_ref: PartitionFaceRef,
    pub(crate) boundary_observation_ids: Vec<PartitionBorderObservationId>,
    pub(crate) twin_edge_keys: Vec<PartitionBorderEdgeKey>,
    pub(crate) local_face_is_unbounded: bool,
    pub(crate) closed: bool,
}

/// Counts from deterministic local boundary-transition plan materialization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceTransitionPlanStats {
    pub(crate) face_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) boundary_transition_count: usize,
    pub(crate) missing_boundary_successor_count: usize,
    pub(crate) closed_face_count: usize,
    pub(crate) incomplete_face_count: usize,
}

/// One declared face-qualified twin positioned inside the two deterministic
/// local transition plans. This is a bridge for a future global walk; it does
/// not assign a global successor or face identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PartitionBorderGlobalFaceTwinTransition {
    pub(crate) edge_key: PartitionBorderEdgeKey,
    pub(crate) forward_face_ref: PartitionFaceRef,
    pub(crate) reverse_face_ref: PartitionFaceRef,
    pub(crate) forward_observation_id: PartitionBorderObservationId,
    pub(crate) reverse_observation_id: PartitionBorderObservationId,
    pub(crate) forward_cycle_index: usize,
    pub(crate) reverse_cycle_index: usize,
    pub(crate) forward_cycle_closed: bool,
    pub(crate) reverse_cycle_closed: bool,
}

/// Counts from positioning declared face twins in ordered local cycles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceTwinTransitionStats {
    pub(crate) face_count: usize,
    pub(crate) transition_count: usize,
    pub(crate) applied_twin_count: usize,
    pub(crate) mapped_twin_count: usize,
    pub(crate) unmapped_twin_count: usize,
    pub(crate) mutation_ready_twin_count: usize,
}

/// Counts from validating the retained global face-walk evidence.
///
/// `face_adjacency_cycle_rank` describes cycles in the retained face/twin
/// connectivity graph. It is not the planar arrangement Euler characteristic;
/// that proof remains gated on a future global topology construction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceWalkInvariantStats {
    pub(crate) face_count: usize,
    pub(crate) transition_count: usize,
    pub(crate) closed_face_count: usize,
    pub(crate) applied_twin_count: usize,
    pub(crate) mapped_twin_count: usize,
    pub(crate) unmapped_twin_count: usize,
    pub(crate) mutation_ready_twin_count: usize,
    pub(crate) component_count: usize,
    pub(crate) unbounded_face_count: usize,
    pub(crate) unbounded_component_count: usize,
    pub(crate) source_complete_twin_count: usize,
    pub(crate) face_adjacency_cycle_rank: usize,
}

/// Evidence for the conservative single-marker global-unbounded-face proof
/// gate. A `proof_ready` result is intentionally narrower than the full
/// global face-identification problem: multiple local unbounded markers remain
/// unresolved rather than being merged by assumption.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalUnboundedFaceProofStats {
    pub(crate) face_count: usize,
    pub(crate) local_unbounded_face_count: usize,
    pub(crate) unbounded_component_count: usize,
    pub(crate) closed_unbounded_face_count: usize,
    pub(crate) unbounded_face_twin_count: usize,
    pub(crate) unbounded_face_unmapped_twin_count: usize,
    pub(crate) unbounded_face_not_ready_twin_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) proof_ready: bool,
}

/// Counts from an Euler witness over retained partition-border evidence.
///
/// The witness deliberately names its measurements as boundary-only values:
/// the exported graph does not yet contain the complete interior arrangement,
/// so `boundary_euler_consistent` is diagnostic evidence and is not a planar
/// Euler proof or permission to assign global face IDs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceEulerWitnessStats {
    pub(crate) component_count: usize,
    pub(crate) transition_face_count: usize,
    pub(crate) closed_boundary_cycle_count: usize,
    pub(crate) boundary_vertex_count: usize,
    pub(crate) boundary_edge_count: usize,
    pub(crate) cross_component_edge_count: usize,
    pub(crate) boundary_euler_lhs: i64,
    pub(crate) boundary_euler_rhs: i64,
    pub(crate) boundary_euler_consistent: bool,
}

/// One retained candidate for splicing two local face cycles across a mapped
/// partition-border twin. The predecessor/successor identities remain in the
/// local observation space; no global `next` link or face ID is assigned.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PartitionBorderGlobalFaceNextCandidate {
    pub(crate) component_index: usize,
    pub(crate) edge_key: PartitionBorderEdgeKey,
    pub(crate) forward_face_ref: PartitionFaceRef,
    pub(crate) reverse_face_ref: PartitionFaceRef,
    pub(crate) forward_observation_id: PartitionBorderObservationId,
    pub(crate) reverse_observation_id: PartitionBorderObservationId,
    pub(crate) forward_predecessor: Option<PartitionBorderObservationId>,
    pub(crate) reverse_predecessor: Option<PartitionBorderObservationId>,
    pub(crate) forward_successor: Option<PartitionBorderObservationId>,
    pub(crate) reverse_successor: Option<PartitionBorderObservationId>,
    pub(crate) forward_global_successor: Option<PartitionBorderObservationId>,
    pub(crate) reverse_global_successor: Option<PartitionBorderObservationId>,
    pub(crate) ready: bool,
}

/// Counts from retaining global-next splice candidates without applying them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceNextCandidateStats {
    pub(crate) component_count: usize,
    pub(crate) twin_candidate_count: usize,
    pub(crate) ready_candidate_count: usize,
    pub(crate) incomplete_candidate_count: usize,
    pub(crate) global_successor_count: usize,
}

/// One retained candidate global face cycle assembled from local transitions
/// and prospective cross-border successors. The ordered observations and face
/// references are evidence only; no global face ID is assigned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdentityPlan {
    pub(crate) component_index: usize,
    pub(crate) boundary_observation_ids: Vec<PartitionBorderObservationId>,
    pub(crate) face_refs: Vec<PartitionFaceRef>,
    pub(crate) closed: bool,
}

/// Counts from retaining boundary-only global face identity candidates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdentityPlanStats {
    pub(crate) component_count: usize,
    pub(crate) boundary_observation_count: usize,
    pub(crate) candidate_cycle_count: usize,
    pub(crate) closed_cycle_count: usize,
    pub(crate) incomplete_component_count: usize,
    pub(crate) non_permutation_component_count: usize,
    pub(crate) permutation_ready: bool,
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
    global_component_payloads: Vec<PartitionBorderGlobalComponentPayload>,
    global_face_plans: Vec<PartitionBorderGlobalFacePlan>,
    global_face_transitions: Vec<PartitionBorderGlobalFaceTransitionPlan>,
    global_face_twin_transitions: Vec<PartitionBorderGlobalFaceTwinTransition>,
    global_face_next_candidates: Vec<PartitionBorderGlobalFaceNextCandidate>,
    global_face_identity_plans: Vec<PartitionBorderGlobalFaceIdentityPlan>,
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
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        Ok(())
    }

    pub fn declare_adjacency(&mut self, adjacency: PartitionBorderAdjacency) {
        self.adjacencies.insert(adjacency);
        self.applied_face_twins.clear();
        self.reconciled_nodes.clear();
        self.global_components.clear();
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
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
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
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
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
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
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        Ok(stats)
    }

    pub(crate) fn global_components(&self) -> &[PartitionBorderGlobalComponent] {
        &self.global_components
    }

    /// Retains deterministic component-level border payload merges after
    /// canonical node reconciliation. Every source, representative, and Z
    /// candidate is unioned by component while each node's selected Z decision
    /// remains addressable. This is evidence only; no global node or face
    /// payload is written.
    pub(crate) fn reconcile_global_component_payloads(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalComponentPayloadStats> {
        execution_policy.check_cancelled("partition_border_global_component_payloads")?;
        execution_policy.check(
            "partition_border_global_component_payload_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        execution_policy.check(
            "partition_border_global_component_payload_nodes",
            execution_policy.max_graph_nodes,
            self.reconciled_nodes.len(),
        )?;

        let mut node_by_key =
            BTreeMap::<PartitionBorderNodeKey, &PartitionBorderNodePayload>::new();
        for (node_index, node) in self.reconciled_nodes.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_component_payload_nodes",
                node_index,
            )?;
            if node_by_key.insert(node.key, node).is_some() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!("global component payload node {:?} is duplicated", node.key),
                });
            }
        }

        let mut payloads = Vec::with_capacity(self.global_components.len());
        let mut stats = PartitionBorderGlobalComponentPayloadStats {
            component_count: self.global_components.len(),
            ..Default::default()
        };
        for (component_position, component) in self.global_components.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_component_payloads",
                component_position,
            )?;
            if component.component_index != component_position {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global component payload index mismatch: expected {}, got {}",
                        component_position, component.component_index
                    ),
                });
            }
            if component.border_node_keys.is_empty() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global component payload {} has no border nodes",
                        component.component_index
                    ),
                });
            }

            let mut source_line_ids = BTreeSet::new();
            let mut representative_line_ids = BTreeSet::new();
            let mut z_bits = BTreeSet::new();
            let mut selected_z_bits = Vec::with_capacity(component.border_node_keys.len());
            let mut selected_z_policy = None;
            let mut z_conflict_node_count = 0usize;
            let mut previous_node_key = None;
            for (node_position, node_key) in component.border_node_keys.iter().copied().enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_component_payload_nodes",
                    node_position,
                )?;
                if previous_node_key.is_some_and(|previous| previous >= node_key) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global component payload {} nodes are not strictly ordered",
                            component.component_index
                        ),
                    });
                }
                previous_node_key = Some(node_key);
                let Some(node) = node_by_key.get(&node_key) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global component payload node {:?} is unreconciled",
                            node_key
                        ),
                    });
                };
                source_line_ids.extend(node.source_line_ids.iter().copied());
                representative_line_ids.extend(node.representative_line_ids.iter().copied());
                z_bits.extend(node.z_bits.iter().copied());
                selected_z_bits.push((node.key, node.selected_z_bits));
                match selected_z_policy {
                    Some(policy) if policy != node.selected_z_policy => {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global component payload {} mixes Z policies",
                                component.component_index
                            ),
                        });
                    }
                    None => selected_z_policy = Some(node.selected_z_policy),
                    Some(_) => {}
                }
                if node.z_conflict {
                    z_conflict_node_count += 1;
                }
            }
            let selected_z_policy = selected_z_policy.ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global component payload {} has no selected Z policy",
                        component.component_index
                    ),
                }
            })?;
            let source_line_ids = source_line_ids.into_iter().collect::<Vec<_>>();
            let representative_line_ids = representative_line_ids.into_iter().collect::<Vec<_>>();
            let z_bits = z_bits.into_iter().collect::<Vec<_>>();
            stats.source_line_count = stats
                .source_line_count
                .checked_add(source_line_ids.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global component payload source count overflow".to_string(),
                })?;
            stats.representative_line_count = stats
                .representative_line_count
                .checked_add(representative_line_ids.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global component payload representative count overflow".to_string(),
                })?;
            stats.z_candidate_count = stats
                .z_candidate_count
                .checked_add(z_bits.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global component payload Z candidate count overflow".to_string(),
                })?;
            stats.selected_z_node_count = stats
                .selected_z_node_count
                .checked_add(selected_z_bits.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global component payload selected Z count overflow".to_string(),
                })?;
            stats.z_conflict_node_count = stats
                .z_conflict_node_count
                .checked_add(z_conflict_node_count)
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global component payload Z conflict count overflow".to_string(),
                })?;
            if z_conflict_node_count > 0 {
                stats.z_conflict_component_count = stats
                    .z_conflict_component_count
                    .checked_add(1)
                    .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                        reason: "global component payload Z conflict component count overflow"
                            .to_string(),
                    })?;
            }
            payloads.push(PartitionBorderGlobalComponentPayload {
                component_index: component.component_index,
                face_refs: component.face_refs.clone(),
                border_node_keys: component.border_node_keys.clone(),
                source_line_ids,
                representative_line_ids,
                z_bits,
                selected_z_bits,
                selected_z_policy,
                z_conflict_node_count,
            });
        }

        self.global_component_payloads = payloads;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_component_payloads(&self) -> &[PartitionBorderGlobalComponentPayload] {
        &self.global_component_payloads
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
        let mut missing_boundary_successor_count = 0;
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
            if observation.local_face_boundary_successor.is_none() {
                missing_boundary_successor_count += 1;
            }
            candidates.push(PartitionBorderFaceBoundaryCandidate {
                observation_id: observation.observation_id(),
                edge_key: observation.edge_key,
                face_ref,
                local_dir_edge_id: observation.local_dir_edge_id,
                local_face_successor,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                local_face_boundary_successor: observation.local_face_boundary_successor,
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
            missing_boundary_successor_count,
        };
        self.global_face_plans = global_face_plans;
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
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
                    || observation.local_face_boundary_successor
                        != candidate.local_face_boundary_successor
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

    /// Checks whether each retained local face can be reduced to one closed
    /// boundary-transition cycle. This is a mutation gate only: it retains no
    /// global `next` links and does not alter local topology or tiled output.
    /// Incomplete local evidence is reported as not ready; contradictory
    /// identity or face lineage fails closed.
    pub(crate) fn validate_global_face_mutation_gate(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceMutationGateStats> {
        let validation = self.validate_global_face_plans(execution_policy)?;
        execution_policy.check_cancelled("partition_border_global_face_mutation_gate")?;

        let mut candidate_faces = BTreeMap::<PartitionBorderObservationId, PartitionFaceRef>::new();
        for plan in &self.global_face_plans {
            for candidate in &plan.candidates {
                candidate_faces.insert(candidate.observation_id, plan.face_ref);
            }
        }

        let mut boundary_transition_count = 0;
        let mut missing_boundary_successor_count = 0;
        let mut mutation_ready_face_count = 0;
        for (plan_index, plan) in self.global_face_plans.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_mutation_gate", plan_index)?;
            let mut transitions =
                BTreeMap::<PartitionBorderObservationId, PartitionBorderObservationId>::new();
            let mut complete = !plan.candidates.is_empty();
            for (candidate_index, candidate) in plan.candidates.iter().enumerate() {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_mutation_gate_candidates",
                    candidate_index,
                )?;
                let Some(boundary_successor) = candidate.local_face_boundary_successor else {
                    missing_boundary_successor_count += 1;
                    complete = false;
                    continue;
                };
                let Some(successor_observation) = self.observations.get(&boundary_successor) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face boundary successor {:?} is missing",
                            boundary_successor
                        ),
                    });
                };
                if successor_observation.face_ref != Some(plan.face_ref) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face boundary successor {:?} crosses face {:?}",
                            boundary_successor, plan.face_ref
                        ),
                    });
                }
                match candidate_faces.get(&boundary_successor) {
                    Some(&successor_face_ref) if successor_face_ref == plan.face_ref => {
                        transitions.insert(candidate.observation_id, boundary_successor);
                        boundary_transition_count += 1;
                    }
                    Some(&successor_face_ref) => {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face boundary successor {:?} is assigned to face {:?}",
                                boundary_successor, successor_face_ref
                            ),
                        });
                    }
                    None => {
                        missing_boundary_successor_count += 1;
                        complete = false;
                    }
                }
            }
            if !complete || transitions.len() != plan.candidates.len() {
                continue;
            }

            let start = plan.candidates[0].observation_id;
            let mut visited = BTreeSet::new();
            let mut current = start;
            loop {
                if !visited.insert(current) {
                    if current == start && visited.len() == plan.candidates.len() {
                        mutation_ready_face_count += 1;
                    }
                    break;
                }
                let Some(&next) = transitions.get(&current) else {
                    break;
                };
                current = next;
                if visited.len() > plan.candidates.len() {
                    break;
                }
            }
        }

        Ok(PartitionBorderGlobalFaceMutationGateStats {
            face_count: validation.face_count,
            candidate_count: validation.candidate_count,
            boundary_transition_count,
            missing_boundary_successor_count,
            mutation_ready_face_count,
        })
    }

    /// Materializes deterministic ordered local boundary-transition plans
    /// after the mutation gate passes. Incomplete faces are retained with
    /// their sorted candidate identities and `closed == false`; no global
    /// `next` links or face IDs are assigned.
    pub(crate) fn reconcile_global_face_transitions(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceTransitionPlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_transition_plan")?;
        let gate = self.validate_global_face_mutation_gate(execution_policy)?;
        execution_policy.check(
            "partition_border_global_face_transition_faces",
            execution_policy.max_graph_nodes,
            self.global_face_plans.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_transition_edges",
            execution_policy.max_graph_edges,
            gate.candidate_count,
        )?;

        let mut candidate_faces = BTreeMap::<PartitionBorderObservationId, PartitionFaceRef>::new();
        for plan in &self.global_face_plans {
            for candidate in &plan.candidates {
                candidate_faces.insert(candidate.observation_id, plan.face_ref);
            }
        }

        let mut transition_plans = Vec::with_capacity(self.global_face_plans.len());
        let mut closed_face_count = 0;
        let mut incomplete_face_count = 0;
        for (plan_index, plan) in self.global_face_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_transition_plan",
                plan_index,
            )?;
            let mut successors =
                BTreeMap::<PartitionBorderObservationId, PartitionBorderObservationId>::new();
            let mut complete = !plan.candidates.is_empty();
            for (candidate_index, candidate) in plan.candidates.iter().enumerate() {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_transition_candidates",
                    candidate_index,
                )?;
                let Some(boundary_successor) = candidate.local_face_boundary_successor else {
                    complete = false;
                    continue;
                };
                let Some(successor_observation) = self.observations.get(&boundary_successor) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face transition successor {:?} is missing",
                            boundary_successor
                        ),
                    });
                };
                if successor_observation.face_ref != Some(plan.face_ref) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face transition successor {:?} crosses face {:?}",
                            boundary_successor, plan.face_ref
                        ),
                    });
                }
                if candidate_faces.get(&boundary_successor) == Some(&plan.face_ref) {
                    successors.insert(candidate.observation_id, boundary_successor);
                } else {
                    complete = false;
                }
            }

            let mut ordered_observation_ids = Vec::with_capacity(plan.candidates.len());
            if complete && successors.len() == plan.candidates.len() {
                let start = plan.candidates[0].observation_id;
                let mut visited = BTreeSet::new();
                let mut current = start;
                loop {
                    if !visited.insert(current) {
                        if current == start && visited.len() == plan.candidates.len() {
                            break;
                        }
                        complete = false;
                        break;
                    }
                    ordered_observation_ids.push(current);
                    let Some(&next) = successors.get(&current) else {
                        complete = false;
                        break;
                    };
                    current = next;
                    if ordered_observation_ids.len() > plan.candidates.len() {
                        complete = false;
                        break;
                    }
                }
            }
            if !complete || ordered_observation_ids.len() != plan.candidates.len() {
                complete = false;
                incomplete_face_count += 1;
                ordered_observation_ids = plan
                    .candidates
                    .iter()
                    .map(|candidate| candidate.observation_id)
                    .collect();
            } else {
                closed_face_count += 1;
            }
            transition_plans.push(PartitionBorderGlobalFaceTransitionPlan {
                face_ref: plan.face_ref,
                boundary_observation_ids: ordered_observation_ids,
                twin_edge_keys: plan.twin_edge_keys.clone(),
                local_face_is_unbounded: plan.local_face_is_unbounded,
                closed: complete,
            });
        }

        let stats = PartitionBorderGlobalFaceTransitionPlanStats {
            face_count: transition_plans.len(),
            candidate_count: gate.candidate_count,
            boundary_transition_count: gate.boundary_transition_count,
            missing_boundary_successor_count: gate.missing_boundary_successor_count,
            closed_face_count,
            incomplete_face_count,
        };
        self.global_face_transitions = transition_plans;
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        Ok(stats)
    }

    pub(crate) fn global_face_transitions(&self) -> &[PartitionBorderGlobalFaceTransitionPlan] {
        &self.global_face_transitions
    }

    /// Positions declared face-qualified twins in the ordered local face
    /// cycles. A twin missing from an incomplete cycle is reported rather than
    /// guessed; no global successor or face identity is assigned.
    pub(crate) fn reconcile_global_face_twin_transitions(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceTwinTransitionStats> {
        execution_policy.check_cancelled("partition_border_global_face_twin_transitions")?;
        execution_policy.check(
            "partition_border_global_face_twin_transition_faces",
            execution_policy.max_graph_nodes,
            self.global_face_transitions.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_twin_transition_edges",
            execution_policy.max_graph_edges,
            self.applied_face_twins.len(),
        )?;

        let mut positions =
            BTreeMap::<PartitionBorderObservationId, (PartitionFaceRef, usize, bool)>::new();
        let mut transition_count = 0usize;
        for (plan_index, plan) in self.global_face_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_twin_transitions",
                plan_index,
            )?;
            transition_count = transition_count
                .checked_add(plan.boundary_observation_ids.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global face transition count overflow".to_string(),
                })?;
            for (cycle_index, observation_id) in
                plan.boundary_observation_ids.iter().copied().enumerate()
            {
                let Some(observation) = self.observations.get(&observation_id) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face transition observation {:?} is missing",
                            observation_id
                        ),
                    });
                };
                if observation.observation_id() != observation_id
                    || observation.face_ref != Some(plan.face_ref)
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face transition observation {:?} disagrees with face {:?}",
                            observation_id, plan.face_ref
                        ),
                    });
                }
                if positions
                    .insert(observation_id, (plan.face_ref, cycle_index, plan.closed))
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face transition observation {:?} is duplicated",
                            observation_id
                        ),
                    });
                }
            }
        }

        let mut links = BTreeSet::new();
        let mut unmapped_twin_count = 0;
        let mut mutation_ready_twin_count = 0;
        for (twin_index, applied_twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_twin_transitions",
                twin_index,
            )?;
            let Some(&(forward_face_ref, forward_cycle_index, forward_cycle_closed)) =
                positions.get(&applied_twin.twin.forward)
            else {
                unmapped_twin_count += 1;
                continue;
            };
            let Some(&(reverse_face_ref, reverse_cycle_index, reverse_cycle_closed)) =
                positions.get(&applied_twin.twin.reverse)
            else {
                unmapped_twin_count += 1;
                continue;
            };
            if forward_face_ref != applied_twin.forward_face_ref
                || reverse_face_ref != applied_twin.reverse_face_ref
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin {:?} is positioned in the wrong face cycles",
                        applied_twin.twin.edge_key
                    ),
                });
            }
            let link = PartitionBorderGlobalFaceTwinTransition {
                edge_key: applied_twin.twin.edge_key,
                forward_face_ref,
                reverse_face_ref,
                forward_observation_id: applied_twin.twin.forward,
                reverse_observation_id: applied_twin.twin.reverse,
                forward_cycle_index,
                reverse_cycle_index,
                forward_cycle_closed,
                reverse_cycle_closed,
            };
            if !links.insert(link) {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face twin transition {:?} is duplicated",
                        applied_twin.twin.edge_key
                    ),
                });
            }
            if forward_cycle_closed && reverse_cycle_closed {
                mutation_ready_twin_count += 1;
            }
        }

        let stats = PartitionBorderGlobalFaceTwinTransitionStats {
            face_count: self.global_face_transitions.len(),
            transition_count,
            applied_twin_count: self.applied_face_twins.len(),
            mapped_twin_count: links.len(),
            unmapped_twin_count,
            mutation_ready_twin_count,
        };
        self.global_face_twin_transitions = links.into_iter().collect();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        Ok(stats)
    }

    pub(crate) fn global_face_twin_transitions(
        &self,
    ) -> &[PartitionBorderGlobalFaceTwinTransition] {
        &self.global_face_twin_transitions
    }

    /// Retains deterministic cross-tile successor splice candidates from
    /// mapped twins and local cycle positions. Incomplete cycles remain
    /// explicit candidates with `ready == false`; conflicting successor
    /// assignments fail closed before the candidate vector is committed.
    #[cfg(test)]
    pub(crate) fn reconcile_global_face_next_candidates(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceNextCandidateStats> {
        execution_policy.check_cancelled("partition_border_global_face_next_candidates")?;
        let walk = self.validate_global_face_walk_invariants(execution_policy)?;
        self.reconcile_global_face_next_candidates_with_walk(execution_policy, walk)
    }

    pub(crate) fn reconcile_global_face_next_candidates_with_walk(
        &mut self,
        execution_policy: &ExecutionPolicy,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
    ) -> crate::Result<PartitionBorderGlobalFaceNextCandidateStats> {
        execution_policy.check_cancelled("partition_border_global_face_next_candidates")?;
        execution_policy.check(
            "partition_border_global_face_next_candidates_faces",
            execution_policy.max_graph_nodes,
            self.global_face_transitions.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_next_candidates_twins",
            execution_policy.max_graph_edges,
            self.global_face_twin_transitions.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_next_candidates_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        if walk.face_count != self.global_face_transitions.len()
            || walk.mapped_twin_count != self.global_face_twin_transitions.len()
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face next candidate walk mismatch: faces={}, walk_faces={}, twins={}, walk_twins={}",
                    self.global_face_transitions.len(),
                    walk.face_count,
                    self.global_face_twin_transitions.len(),
                    walk.mapped_twin_count
                ),
            });
        }

        let mut component_by_face = BTreeMap::<PartitionFaceRef, usize>::new();
        for (component_position, component) in self.global_components.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_next_candidates_components",
                component_position,
            )?;
            for face_ref in &component.face_refs {
                if component_by_face
                    .insert(*face_ref, component_position)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face next candidate face {:?} belongs to multiple components",
                            face_ref
                        ),
                    });
                }
            }
        }

        let mut transition_by_face = BTreeMap::<PartitionFaceRef, usize>::new();
        for (transition_index, transition) in self.global_face_transitions.iter().enumerate() {
            if transition_by_face
                .insert(transition.face_ref, transition_index)
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next candidate transition {:?} is duplicated",
                        transition.face_ref
                    ),
                });
            }
        }

        let mut candidates = Vec::with_capacity(self.global_face_twin_transitions.len());
        let mut global_successors =
            BTreeMap::<PartitionBorderObservationId, PartitionBorderObservationId>::new();
        let mut ready_candidate_count = 0usize;
        let mut incomplete_candidate_count = 0usize;
        for (twin_index, link) in self.global_face_twin_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_next_candidates",
                twin_index,
            )?;
            let Some(&component_index) = component_by_face.get(&link.forward_face_ref) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next candidate twin {:?} has no component",
                        link.edge_key
                    ),
                });
            };
            if component_by_face.get(&link.reverse_face_ref) != Some(&component_index) {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next candidate twin {:?} crosses components",
                        link.edge_key
                    ),
                });
            }
            let Some(&forward_transition_index) = transition_by_face.get(&link.forward_face_ref)
            else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next candidate twin {:?} has no forward transition",
                        link.edge_key
                    ),
                });
            };
            let Some(&reverse_transition_index) = transition_by_face.get(&link.reverse_face_ref)
            else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next candidate twin {:?} has no reverse transition",
                        link.edge_key
                    ),
                });
            };
            let forward_transition = &self.global_face_transitions[forward_transition_index];
            let reverse_transition = &self.global_face_transitions[reverse_transition_index];
            let cycle_position = |transition: &PartitionBorderGlobalFaceTransitionPlan,
                                  observation_id: PartitionBorderObservationId,
                                  cycle_index: usize|
             -> crate::Result<
                Option<(PartitionBorderObservationId, PartitionBorderObservationId)>,
            > {
                if !transition.closed {
                    return Ok(None);
                }
                let Some(&position_observation_id) =
                    transition.boundary_observation_ids.get(cycle_index)
                else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face next candidate twin {:?} has an invalid cycle position",
                            link.edge_key
                        ),
                    });
                };
                if position_observation_id != observation_id
                    || transition.boundary_observation_ids.is_empty()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face next candidate twin {:?} disagrees with its cycle position",
                            link.edge_key
                        ),
                    });
                }
                let cycle_len = transition.boundary_observation_ids.len();
                let predecessor_index = (cycle_index + cycle_len - 1) % cycle_len;
                let successor_index = (cycle_index + 1) % cycle_len;
                Ok(Some((
                    transition.boundary_observation_ids[predecessor_index],
                    transition.boundary_observation_ids[successor_index],
                )))
            };
            let forward_position = cycle_position(
                forward_transition,
                link.forward_observation_id,
                link.forward_cycle_index,
            )?;
            let reverse_position = cycle_position(
                reverse_transition,
                link.reverse_observation_id,
                link.reverse_cycle_index,
            )?;
            let ready = forward_position.is_some() && reverse_position.is_some();
            let (
                forward_predecessor,
                forward_successor,
                reverse_predecessor,
                reverse_successor,
                forward_global_successor,
                reverse_global_successor,
            ) = match (forward_position, reverse_position) {
                (
                    Some((forward_predecessor, forward_successor)),
                    Some((reverse_predecessor, reverse_successor)),
                ) => {
                    let assignments = [
                        (forward_predecessor, reverse_successor),
                        (reverse_predecessor, forward_successor),
                    ];
                    for (predecessor, successor) in assignments {
                        if let Some(existing) = global_successors.insert(predecessor, successor) {
                            if existing != successor {
                                return Err(crate::PolygonizeError::InternalInvariantViolation {
                                    reason: format!(
                                        "global face next candidate predecessor {:?} has conflicting successors {:?} and {:?}",
                                        predecessor, existing, successor
                                    ),
                                });
                            }
                        }
                    }
                    (
                        Some(forward_predecessor),
                        Some(forward_successor),
                        Some(reverse_predecessor),
                        Some(reverse_successor),
                        Some(reverse_successor),
                        Some(forward_successor),
                    )
                }
                (forward_position, reverse_position) => {
                    incomplete_candidate_count += 1;
                    (
                        forward_position.map(|(predecessor, _successor)| predecessor),
                        forward_position.map(|(_predecessor, successor)| successor),
                        reverse_position.map(|(predecessor, _successor)| predecessor),
                        reverse_position.map(|(_predecessor, successor)| successor),
                        None,
                        None,
                    )
                }
            };
            if ready {
                ready_candidate_count += 1;
            }
            candidates.push(PartitionBorderGlobalFaceNextCandidate {
                component_index,
                edge_key: link.edge_key,
                forward_face_ref: link.forward_face_ref,
                reverse_face_ref: link.reverse_face_ref,
                forward_observation_id: link.forward_observation_id,
                reverse_observation_id: link.reverse_observation_id,
                forward_predecessor,
                reverse_predecessor,
                forward_successor,
                reverse_successor,
                forward_global_successor,
                reverse_global_successor,
                ready,
            });
        }

        let stats = PartitionBorderGlobalFaceNextCandidateStats {
            component_count: self.global_components.len(),
            twin_candidate_count: candidates.len(),
            ready_candidate_count,
            incomplete_candidate_count,
            global_successor_count: global_successors.len(),
        };
        self.global_face_next_candidates = candidates;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_next_candidates(&self) -> &[PartitionBorderGlobalFaceNextCandidate] {
        &self.global_face_next_candidates
    }

    /// Retains boundary-only global face identity candidates from the local
    /// transition cycles and prospective cross-border successors. The wrapper
    /// validates the preceding evidence first; no global face ID is assigned.
    #[cfg(test)]
    pub(crate) fn reconcile_global_face_identity_plans(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceIdentityPlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_identity_plans")?;
        let walk = self.validate_global_face_walk_invariants(execution_policy)?;
        if self.global_face_next_candidates.len() != self.global_face_twin_transitions.len() {
            self.reconcile_global_face_next_candidates_with_walk(execution_policy, walk)?;
        }
        self.reconcile_global_face_identity_plans_with_walk(execution_policy, walk)
    }

    pub(crate) fn reconcile_global_face_identity_plans_with_walk(
        &mut self,
        execution_policy: &ExecutionPolicy,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
    ) -> crate::Result<PartitionBorderGlobalFaceIdentityPlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_identity_plans")?;
        execution_policy.check(
            "partition_border_global_face_identity_plans_faces",
            execution_policy.max_graph_nodes,
            self.global_face_transitions.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_identity_plans_observations",
            execution_policy.max_graph_edges,
            walk.transition_count,
        )?;
        execution_policy.check(
            "partition_border_global_face_identity_plans_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        if walk.face_count != self.global_face_transitions.len()
            || walk.mapped_twin_count != self.global_face_twin_transitions.len()
            || self.global_face_next_candidates.len() != self.global_face_twin_transitions.len()
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face identity plan evidence mismatch: faces={}, walk_faces={}, twins={}, walk_twins={}, next_candidates={}",
                    self.global_face_transitions.len(),
                    walk.face_count,
                    self.global_face_twin_transitions.len(),
                    walk.mapped_twin_count,
                    self.global_face_next_candidates.len()
                ),
            });
        }

        let mut component_by_face = BTreeMap::<PartitionFaceRef, usize>::new();
        for (component_index, component) in self.global_components.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_plans_components",
                component_index,
            )?;
            for face_ref in &component.face_refs {
                if component_by_face
                    .insert(*face_ref, component_index)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face identity plan face {:?} belongs to multiple components",
                            face_ref
                        ),
                    });
                }
            }
        }

        let mut face_by_observation =
            BTreeMap::<PartitionBorderObservationId, PartitionFaceRef>::new();
        let mut observations_by_component =
            BTreeMap::<usize, BTreeSet<PartitionBorderObservationId>>::new();
        let mut face_refs_by_component = BTreeMap::<usize, BTreeSet<PartitionFaceRef>>::new();
        let mut successor_by_observation =
            BTreeMap::<PartitionBorderObservationId, PartitionBorderObservationId>::new();
        let mut incomplete_components = BTreeSet::new();
        for (transition_index, transition) in self.global_face_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_plans_transitions",
                transition_index,
            )?;
            let Some(&component_index) = component_by_face.get(&transition.face_ref) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face identity plan transition {:?} has no component",
                        transition.face_ref
                    ),
                });
            };
            face_refs_by_component
                .entry(component_index)
                .or_default()
                .insert(transition.face_ref);
            let observations = observations_by_component
                .entry(component_index)
                .or_default();
            for observation_id in transition.boundary_observation_ids.iter().copied() {
                if face_by_observation
                    .insert(observation_id, transition.face_ref)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face identity plan observation {:?} is duplicated",
                            observation_id
                        ),
                    });
                }
                observations.insert(observation_id);
            }
            if !transition.closed {
                incomplete_components.insert(component_index);
                continue;
            }
            if transition.boundary_observation_ids.is_empty() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face identity plan transition {:?} is closed but empty",
                        transition.face_ref
                    ),
                });
            }
            let cycle_len = transition.boundary_observation_ids.len();
            for (cycle_index, observation_id) in transition
                .boundary_observation_ids
                .iter()
                .copied()
                .enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_identity_plans_transition_observations",
                    cycle_index,
                )?;
                let successor = transition.boundary_observation_ids[(cycle_index + 1) % cycle_len];
                if successor_by_observation
                    .insert(observation_id, successor)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face identity plan observation {:?} has duplicate local successors",
                            observation_id
                        ),
                    });
                }
            }
        }

        let mut global_successor_overrides =
            BTreeMap::<PartitionBorderObservationId, PartitionBorderObservationId>::new();
        for (candidate_index, candidate) in self.global_face_next_candidates.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_plans_candidates",
                candidate_index,
            )?;
            if !candidate.ready {
                if candidate.forward_global_successor.is_some()
                    || candidate.reverse_global_successor.is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face identity plan candidate {:?} is incomplete but has a global successor",
                            candidate.edge_key
                        ),
                    });
                }
                continue;
            }
            let Some(forward_predecessor) = candidate.forward_predecessor else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face identity plan candidate {:?} lacks a forward predecessor",
                        candidate.edge_key
                    ),
                });
            };
            let Some(reverse_predecessor) = candidate.reverse_predecessor else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face identity plan candidate {:?} lacks a reverse predecessor",
                        candidate.edge_key
                    ),
                });
            };
            let Some(forward_successor) = candidate.forward_global_successor else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face identity plan candidate {:?} lacks a forward global successor",
                        candidate.edge_key
                    ),
                });
            };
            let Some(reverse_successor) = candidate.reverse_global_successor else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face identity plan candidate {:?} lacks a reverse global successor",
                        candidate.edge_key
                    ),
                });
            };
            for (predecessor, successor) in [
                (forward_predecessor, forward_successor),
                (reverse_predecessor, reverse_successor),
            ] {
                if !successor_by_observation.contains_key(&predecessor) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face identity plan candidate {:?} overrides an absent predecessor {:?}",
                            candidate.edge_key, predecessor
                        ),
                    });
                }
                if let Some(existing) = global_successor_overrides.insert(predecessor, successor) {
                    if existing != successor {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face identity plan predecessor {:?} has conflicting successors {:?} and {:?}",
                                predecessor, existing, successor
                            ),
                        });
                    }
                }
            }
        }
        for (predecessor, successor) in global_successor_overrides {
            successor_by_observation.insert(predecessor, successor);
        }

        let mut plans = Vec::new();
        let mut closed_cycle_count = 0usize;
        let mut non_permutation_component_count = 0usize;
        for component_index in 0..self.global_components.len() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_plans_cycles",
                component_index,
            )?;
            let observations = observations_by_component
                .get(&component_index)
                .cloned()
                .unwrap_or_default();
            let face_refs = face_refs_by_component
                .get(&component_index)
                .cloned()
                .unwrap_or_default();
            if observations.is_empty() || incomplete_components.contains(&component_index) {
                incomplete_components.insert(component_index);
                plans.push(PartitionBorderGlobalFaceIdentityPlan {
                    component_index,
                    boundary_observation_ids: observations.into_iter().collect(),
                    face_refs: face_refs.into_iter().collect(),
                    closed: false,
                });
                continue;
            }

            let mut incoming_count = BTreeMap::<PartitionBorderObservationId, usize>::new();
            let mut non_permutation = false;
            for observation_id in &observations {
                let Some(&successor) = successor_by_observation.get(observation_id) else {
                    non_permutation = true;
                    continue;
                };
                if !observations.contains(&successor) {
                    non_permutation = true;
                    continue;
                }
                *incoming_count.entry(successor).or_default() += 1;
            }
            if incoming_count.len() != observations.len()
                || observations
                    .iter()
                    .any(|observation_id| incoming_count.get(observation_id) != Some(&1))
            {
                non_permutation = true;
            }
            if non_permutation {
                non_permutation_component_count += 1;
                plans.push(PartitionBorderGlobalFaceIdentityPlan {
                    component_index,
                    boundary_observation_ids: observations.into_iter().collect(),
                    face_refs: face_refs.into_iter().collect(),
                    closed: false,
                });
                continue;
            }

            let mut remaining = observations.clone();
            let mut component_cycles = Vec::new();
            while let Some(&start) = remaining.first() {
                let mut cycle = Vec::new();
                let mut current = start;
                loop {
                    if !remaining.remove(&current) {
                        non_permutation = true;
                        break;
                    }
                    cycle.push(current);
                    let Some(&successor) = successor_by_observation.get(&current) else {
                        non_permutation = true;
                        break;
                    };
                    if successor == start {
                        break;
                    }
                    if !remaining.contains(&successor) {
                        non_permutation = true;
                        break;
                    }
                    current = successor;
                }
                if non_permutation {
                    break;
                }
                component_cycles.push(cycle);
            }
            if non_permutation {
                non_permutation_component_count += 1;
                plans.push(PartitionBorderGlobalFaceIdentityPlan {
                    component_index,
                    boundary_observation_ids: observations.into_iter().collect(),
                    face_refs: face_refs.into_iter().collect(),
                    closed: false,
                });
                continue;
            }
            for cycle in component_cycles {
                let cycle_face_refs = cycle
                    .iter()
                    .filter_map(|observation_id| face_by_observation.get(observation_id))
                    .copied()
                    .collect::<BTreeSet<_>>();
                plans.push(PartitionBorderGlobalFaceIdentityPlan {
                    component_index,
                    boundary_observation_ids: cycle,
                    face_refs: cycle_face_refs.into_iter().collect(),
                    closed: true,
                });
                closed_cycle_count += 1;
            }
        }

        let boundary_observation_count =
            observations_by_component.values().map(BTreeSet::len).sum();
        let stats = PartitionBorderGlobalFaceIdentityPlanStats {
            component_count: self.global_components.len(),
            boundary_observation_count,
            candidate_cycle_count: plans.len(),
            closed_cycle_count,
            incomplete_component_count: incomplete_components.len(),
            non_permutation_component_count,
            permutation_ready: incomplete_components.is_empty()
                && non_permutation_component_count == 0,
        };
        self.global_face_identity_plans = plans;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_identity_plans(&self) -> &[PartitionBorderGlobalFaceIdentityPlan] {
        &self.global_face_identity_plans
    }

    /// Validates the retained face-walk, twin, payload, and face-adjacency
    /// evidence before any global topology mutation. Closed local cycles must
    /// preserve their declared successor identities, every mapped twin must
    /// point into the declared cycle positions, and every payload must still
    /// agree with its immutable observations and reconciled endpoint nodes.
    /// Incomplete cycles remain explicit evidence; local unbounded markers are
    /// counted but are not promoted to a global unbounded-face identity.
    pub(crate) fn validate_global_face_walk_invariants(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceWalkInvariantStats> {
        execution_policy.check_cancelled("partition_border_global_face_walk_invariants")?;
        execution_policy.check(
            "partition_border_global_face_walk_faces",
            execution_policy.max_graph_nodes,
            self.global_face_plans.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_walk_transitions",
            execution_policy.max_graph_edges,
            self.global_face_transitions
                .iter()
                .map(|plan| plan.boundary_observation_ids.len())
                .sum(),
        )?;
        execution_policy.check(
            "partition_border_global_face_walk_twins",
            execution_policy.max_graph_edges,
            self.applied_face_twins.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_walk_mapped_twins",
            execution_policy.max_graph_edges,
            self.global_face_twin_transitions.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_walk_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_walk_nodes",
            execution_policy.max_graph_nodes,
            self.reconciled_nodes.len(),
        )?;

        let face_validation = self.validate_global_face_plans(execution_policy)?;
        if self.global_face_plans.len() != self.global_face_transitions.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face walk plan count mismatch: plans={}, transitions={}",
                    self.global_face_plans.len(),
                    self.global_face_transitions.len()
                ),
            });
        }
        if !self.global_face_plans.is_empty() && self.global_components.is_empty() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face walk evidence has no reconciled components".to_string(),
            });
        }

        let mut plan_indices = BTreeMap::<PartitionFaceRef, usize>::new();
        let mut candidates_by_face = BTreeMap::<
            PartitionFaceRef,
            BTreeMap<PartitionBorderObservationId, PartitionBorderFaceBoundaryCandidate>,
        >::new();
        let mut unbounded_face_count = 0usize;
        for (plan_index, plan) in self.global_face_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_walk_invariants",
                plan_index,
            )?;
            if plan_indices.insert(plan.face_ref, plan_index).is_some() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!("global face walk plan {:?} is duplicated", plan.face_ref),
                });
            }
            if plan.local_face_is_unbounded {
                unbounded_face_count += 1;
            }
            let mut candidates = BTreeMap::new();
            for candidate in &plan.candidates {
                if candidates
                    .insert(candidate.observation_id, *candidate)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk candidate {:?} is duplicated",
                            candidate.observation_id
                        ),
                    });
                }
            }
            candidates_by_face.insert(plan.face_ref, candidates);
        }
        if face_validation.face_count != plan_indices.len()
            || face_validation.unbounded_face_count != unbounded_face_count
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face walk validation counts disagree with face plans".to_string(),
            });
        }

        let mut transition_indices = BTreeMap::<PartitionFaceRef, usize>::new();
        let mut transition_count = 0usize;
        let mut closed_face_count = 0usize;
        let mut previous_transition_face_ref = None;
        for (transition_index, transition) in self.global_face_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_walk_transitions",
                transition_index,
            )?;
            if previous_transition_face_ref.is_some_and(|previous| previous >= transition.face_ref)
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk transitions are not strictly ordered at face {:?}",
                        transition.face_ref
                    ),
                });
            }
            previous_transition_face_ref = Some(transition.face_ref);
            let Some(&plan_index) = plan_indices.get(&transition.face_ref) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk transition {:?} has no face plan",
                        transition.face_ref
                    ),
                });
            };
            if transition_indices
                .insert(transition.face_ref, transition_index)
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk transition {:?} is duplicated",
                        transition.face_ref
                    ),
                });
            }
            let plan = &self.global_face_plans[plan_index];
            if transition.local_face_is_unbounded != plan.local_face_is_unbounded
                || transition.twin_edge_keys != plan.twin_edge_keys
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk transition {:?} disagrees with its face plan",
                        transition.face_ref
                    ),
                });
            }
            transition_count = transition_count
                .checked_add(transition.boundary_observation_ids.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global face walk transition count overflow".to_string(),
                })?;
            let Some(candidates) = candidates_by_face.get(&transition.face_ref) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk transition {:?} has no candidates",
                        transition.face_ref
                    ),
                });
            };
            let mut observed_ids = BTreeSet::new();
            for (cycle_index, observation_id) in transition
                .boundary_observation_ids
                .iter()
                .copied()
                .enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_walk_transition_observations",
                    cycle_index,
                )?;
                if !observed_ids.insert(observation_id) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk transition {:?} reuses observation {:?}",
                            transition.face_ref, observation_id
                        ),
                    });
                }
                if !candidates.contains_key(&observation_id) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk transition {:?} references an absent candidate {:?}",
                            transition.face_ref, observation_id
                        ),
                    });
                }
            }
            if observed_ids.len() != candidates.len() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk transition {:?} omits candidates: listed={}, candidates={}",
                        transition.face_ref,
                        observed_ids.len(),
                        candidates.len()
                    ),
                });
            }
            if transition.closed {
                if transition.boundary_observation_ids.is_empty() {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk transition {:?} is closed but empty",
                            transition.face_ref
                        ),
                    });
                }
                for (cycle_index, observation_id) in transition
                    .boundary_observation_ids
                    .iter()
                    .copied()
                    .enumerate()
                {
                    let next_index = (cycle_index + 1) % transition.boundary_observation_ids.len();
                    let next_observation_id = transition.boundary_observation_ids[next_index];
                    let candidate = candidates
                        .get(&observation_id)
                        .expect("candidate identity was checked above");
                    if candidate.local_face_boundary_successor != Some(next_observation_id) {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face walk transition {:?} successor mismatch at observation {:?}",
                                transition.face_ref, observation_id
                            ),
                        });
                    }
                }
                closed_face_count += 1;
            }
        }
        if transition_indices.len() != plan_indices.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face walk transition coverage mismatch: plans={}, transitions={}",
                    plan_indices.len(),
                    transition_indices.len()
                ),
            });
        }

        let mut node_by_key =
            BTreeMap::<PartitionBorderNodeKey, &PartitionBorderNodePayload>::new();
        for (node_index, node) in self.reconciled_nodes.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_walk_nodes", node_index)?;
            if node_by_key.insert(node.key, node).is_some() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk reconciled node {:?} is duplicated",
                        node.key
                    ),
                });
            }
        }

        let mut component_face_owner = BTreeMap::<PartitionFaceRef, usize>::new();
        let mut component_edge_owner = BTreeMap::<PartitionBorderEdgeKey, usize>::new();
        let mut face_adjacency_cycle_rank = 0usize;
        let mut unbounded_component_count = 0usize;
        for (component_position, component) in self.global_components.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_walk_components",
                component_position,
            )?;
            if component.component_index != component_position {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk component index mismatch: expected {}, got {}",
                        component_position, component.component_index
                    ),
                });
            }
            if component.face_refs.is_empty() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk component {} has no face refs",
                        component.component_index
                    ),
                });
            }
            let mut local_faces = BTreeSet::new();
            let mut previous_face_ref = None;
            for face_ref in &component.face_refs {
                if previous_face_ref.is_some_and(|previous| previous >= *face_ref) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk component {} faces are not strictly ordered",
                            component.component_index
                        ),
                    });
                }
                previous_face_ref = Some(*face_ref);
                if !local_faces.insert(*face_ref) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk component {} repeats face {:?}",
                            component.component_index, face_ref
                        ),
                    });
                }
                if component_face_owner
                    .insert(*face_ref, component.component_index)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk face {:?} belongs to multiple components",
                            face_ref
                        ),
                    });
                }
            }
            let mut local_edges = BTreeSet::new();
            let mut previous_edge_key = None;
            for edge_key in &component.twin_edge_keys {
                if previous_edge_key.is_some_and(|previous| previous >= *edge_key) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk component {} twin edges are not strictly ordered",
                            component.component_index
                        ),
                    });
                }
                previous_edge_key = Some(*edge_key);
                if !local_edges.insert(*edge_key) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk component {} repeats twin edge {:?}",
                            component.component_index, edge_key
                        ),
                    });
                }
                if component_edge_owner
                    .insert(*edge_key, component.component_index)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk twin edge {:?} belongs to multiple components",
                            edge_key
                        ),
                    });
                }
            }
            let required_edge_count = component.face_refs.len().saturating_sub(1);
            let component_cycle_rank = local_edges
                .len()
                .checked_sub(required_edge_count)
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk component {} is disconnected: faces={}, twin_edges={}",
                        component.component_index,
                        component.face_refs.len(),
                        local_edges.len()
                    ),
                })?;
            face_adjacency_cycle_rank = face_adjacency_cycle_rank
                .checked_add(component_cycle_rank)
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global face adjacency cycle-rank overflow".to_string(),
                })?;
            if component.face_refs.iter().any(|face_ref| {
                plan_indices.get(face_ref).is_some_and(|&plan_index| {
                    self.global_face_plans[plan_index].local_face_is_unbounded
                })
            }) {
                unbounded_component_count += 1;
            }
        }
        if component_face_owner.len() != plan_indices.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face walk component coverage mismatch: faces={}, owned={}",
                    plan_indices.len(),
                    component_face_owner.len()
                ),
            });
        }
        if component_edge_owner.len() != self.applied_face_twins.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face walk component twin coverage mismatch: applied={}, owned={}",
                    self.applied_face_twins.len(),
                    component_edge_owner.len()
                ),
            });
        }

        let mut applied_by_edge =
            BTreeMap::<PartitionBorderEdgeKey, &PartitionBorderFaceTwin>::new();
        for (twin_index, applied_twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_walk_twins", twin_index)?;
            if applied_by_edge
                .insert(applied_twin.twin.edge_key, applied_twin)
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk twin edge {:?} is duplicated",
                        applied_twin.twin.edge_key
                    ),
                });
            }
            let Some(&component_index) = component_edge_owner.get(&applied_twin.twin.edge_key)
            else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk twin edge {:?} has no component",
                        applied_twin.twin.edge_key
                    ),
                });
            };
            if component_face_owner.get(&applied_twin.forward_face_ref) != Some(&component_index)
                || component_face_owner.get(&applied_twin.reverse_face_ref)
                    != Some(&component_index)
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk twin edge {:?} crosses components",
                        applied_twin.twin.edge_key
                    ),
                });
            }
        }

        let mut mapped_by_edge =
            BTreeMap::<PartitionBorderEdgeKey, &PartitionBorderGlobalFaceTwinTransition>::new();
        for (link_index, link) in self.global_face_twin_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_walk_mapped_twins",
                link_index,
            )?;
            let Some(applied_twin) = applied_by_edge.get(&link.edge_key) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk link {:?} has no applied twin",
                        link.edge_key
                    ),
                });
            };
            if mapped_by_edge.insert(link.edge_key, link).is_some()
                || link.forward_face_ref != applied_twin.forward_face_ref
                || link.reverse_face_ref != applied_twin.reverse_face_ref
                || link.forward_observation_id != applied_twin.twin.forward
                || link.reverse_observation_id != applied_twin.twin.reverse
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk link {:?} disagrees with its applied twin",
                        link.edge_key
                    ),
                });
            }
            let Some(&forward_transition_index) = transition_indices.get(&link.forward_face_ref)
            else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk link {:?} has no forward transition",
                        link.edge_key
                    ),
                });
            };
            let Some(&reverse_transition_index) = transition_indices.get(&link.reverse_face_ref)
            else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk link {:?} has no reverse transition",
                        link.edge_key
                    ),
                });
            };
            let forward_transition = &self.global_face_transitions[forward_transition_index];
            let reverse_transition = &self.global_face_transitions[reverse_transition_index];
            if forward_transition
                .boundary_observation_ids
                .get(link.forward_cycle_index)
                != Some(&link.forward_observation_id)
                || reverse_transition
                    .boundary_observation_ids
                    .get(link.reverse_cycle_index)
                    != Some(&link.reverse_observation_id)
                || link.forward_cycle_closed != forward_transition.closed
                || link.reverse_cycle_closed != reverse_transition.closed
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk link {:?} has invalid cycle position",
                        link.edge_key
                    ),
                });
            }
        }

        let mut source_complete_twin_count = 0usize;
        let mut mutation_ready_twin_count = 0usize;
        for (twin_index, applied_twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_walk_sources", twin_index)?;
            let forward_id = applied_twin.twin.forward;
            let reverse_id = applied_twin.twin.reverse;
            let forward = self
                .observations
                .get(&forward_id)
                .expect("validated face twin forward observation");
            let reverse = self
                .observations
                .get(&reverse_id)
                .expect("validated face twin reverse observation");
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
            if applied_twin.payload.twin != applied_twin.twin
                || applied_twin.payload.source_line_ids != source_line_ids
                || applied_twin.payload.forward_representative_line_id
                    != forward.representative_line_id
                || applied_twin.payload.reverse_representative_line_id
                    != reverse.representative_line_id
                || applied_twin.payload.start_z_bits != start_z_bits
                || applied_twin.payload.end_z_bits != end_z_bits
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face walk twin {:?} payload disagrees with observations",
                        applied_twin.twin.edge_key
                    ),
                });
            }
            for (node_key, observation_ids, z_bits) in [
                (
                    forward.from,
                    [forward_id, reverse_id],
                    [forward.from_z_bits, reverse.to_z_bits],
                ),
                (
                    forward.to,
                    [forward_id, reverse_id],
                    [forward.to_z_bits, reverse.from_z_bits],
                ),
            ] {
                let Some(node) = node_by_key.get(&node_key) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk twin {:?} endpoint node {:?} is unreconciled",
                            applied_twin.twin.edge_key, node_key
                        ),
                    });
                };
                if observation_ids
                    .iter()
                    .any(|observation_id| !node.observation_ids.contains(observation_id))
                    || source_line_ids
                        .iter()
                        .any(|source_line_id| !node.source_line_ids.contains(source_line_id))
                    || z_bits.iter().any(|z_bit| !node.z_bits.contains(z_bit))
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face walk twin {:?} endpoint node {:?} loses payload lineage",
                            applied_twin.twin.edge_key, node_key
                        ),
                    });
                }
            }
            source_complete_twin_count += 1;
            if let Some(link) = mapped_by_edge.get(&applied_twin.twin.edge_key) {
                if link.forward_cycle_closed && link.reverse_cycle_closed {
                    mutation_ready_twin_count += 1;
                }
            }
        }
        let mapped_twin_count = mapped_by_edge.len();
        if mapped_twin_count > self.applied_face_twins.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face walk mapped twin count exceeds applied count: mapped={}, applied={}",
                    mapped_twin_count,
                    self.applied_face_twins.len()
                ),
            });
        }

        Ok(PartitionBorderGlobalFaceWalkInvariantStats {
            face_count: plan_indices.len(),
            transition_count,
            closed_face_count,
            applied_twin_count: self.applied_face_twins.len(),
            mapped_twin_count,
            unmapped_twin_count: self.applied_face_twins.len() - mapped_twin_count,
            mutation_ready_twin_count,
            component_count: self.global_components.len(),
            unbounded_face_count,
            unbounded_component_count,
            source_complete_twin_count,
            face_adjacency_cycle_rank,
        })
    }

    /// Builds a deterministic Euler witness from the retained border cycles.
    ///
    /// This intentionally does not claim planar completeness: each measured
    /// vertex and edge is still limited to exported partition-border evidence.
    /// A mismatch remains a diagnostic boundary, while a match does not permit
    /// global topology mutation or face identity assignment.
    #[cfg(test)]
    pub(crate) fn validate_global_face_euler_witness(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceEulerWitnessStats> {
        execution_policy.check_cancelled("partition_border_global_face_euler_witness")?;
        let walk = self.validate_global_face_walk_invariants(execution_policy)?;
        self.validate_global_face_euler_witness_with_walk(execution_policy, walk)
    }

    pub(crate) fn validate_global_face_euler_witness_with_walk(
        &self,
        execution_policy: &ExecutionPolicy,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
    ) -> crate::Result<PartitionBorderGlobalFaceEulerWitnessStats> {
        execution_policy.check_cancelled("partition_border_global_face_euler_witness")?;
        execution_policy.check(
            "partition_border_global_face_euler_witness_faces",
            execution_policy.max_graph_nodes,
            self.global_face_transitions.len(),
        )?;
        let transition_edge_count = self
            .global_face_transitions
            .iter()
            .try_fold(0usize, |count, transition| {
                count.checked_add(transition.boundary_observation_ids.len())
            })
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face Euler witness transition count overflow".to_string(),
            })?;
        execution_policy.check(
            "partition_border_global_face_euler_witness_edges",
            execution_policy.max_graph_edges,
            transition_edge_count,
        )?;
        execution_policy.check(
            "partition_border_global_face_euler_witness_nodes",
            execution_policy.max_graph_nodes,
            self.reconciled_nodes.len(),
        )?;
        if walk.face_count != self.global_face_transitions.len()
            || walk.transition_count != transition_edge_count
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face Euler witness walk mismatch: faces={}, transitions={}, walk_faces={}, walk_transitions={}",
                    self.global_face_transitions.len(),
                    transition_edge_count,
                    walk.face_count,
                    walk.transition_count
                ),
            });
        }

        let mut component_by_face = BTreeMap::<PartitionFaceRef, usize>::new();
        for (component_position, component) in self.global_components.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_euler_witness_components",
                component_position,
            )?;
            for face_ref in &component.face_refs {
                if component_by_face
                    .insert(*face_ref, component_position)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face Euler witness face {:?} belongs to multiple components",
                            face_ref
                        ),
                    });
                }
            }
        }

        let mut node_keys = BTreeSet::new();
        for (node_index, node) in self.reconciled_nodes.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_euler_witness_nodes",
                node_index,
            )?;
            if !node_keys.insert(node.key) {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face Euler witness reconciled node {:?} is duplicated",
                        node.key
                    ),
                });
            }
        }

        let mut boundary_vertices = BTreeSet::new();
        let mut boundary_edges = BTreeSet::new();
        let mut edge_component = BTreeMap::<PartitionBorderEdgeKey, usize>::new();
        let mut cross_component_edges = BTreeSet::new();
        let mut closed_boundary_cycle_count = 0usize;
        for (transition_index, transition) in self.global_face_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_euler_witness",
                transition_index,
            )?;
            let Some(&component_index) = component_by_face.get(&transition.face_ref) else {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face Euler witness transition {:?} has no component",
                        transition.face_ref
                    ),
                });
            };
            if transition.closed {
                closed_boundary_cycle_count = closed_boundary_cycle_count
                    .checked_add(1)
                    .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                        reason: "global face Euler witness cycle count overflow".to_string(),
                    })?;
            }
            let component = &self.global_components[component_index];
            for (observation_index, observation_id) in transition
                .boundary_observation_ids
                .iter()
                .copied()
                .enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_euler_witness_edges",
                    observation_index,
                )?;
                let Some(observation) = self.observations.get(&observation_id) else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face Euler witness observation {:?} is missing",
                            observation_id
                        ),
                    });
                };
                if observation.face_ref != Some(transition.face_ref) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face Euler witness observation {:?} crosses face {:?}",
                            observation_id, transition.face_ref
                        ),
                    });
                }
                if !node_keys.contains(&observation.from) || !node_keys.contains(&observation.to) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face Euler witness observation {:?} has unreconciled endpoints",
                            observation_id
                        ),
                    });
                }
                if !component.border_node_keys.contains(&observation.from)
                    || !component.border_node_keys.contains(&observation.to)
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face Euler witness observation {:?} leaves component {}",
                            observation_id, component_index
                        ),
                    });
                }
                if let Some(previous_component) =
                    edge_component.insert(observation.edge_key, component_index)
                {
                    if previous_component != component_index {
                        cross_component_edges.insert(observation.edge_key);
                    }
                }
                boundary_edges.insert(observation.edge_key);
                boundary_vertices.insert(observation.from);
                boundary_vertices.insert(observation.to);
            }
        }
        if closed_boundary_cycle_count != walk.closed_face_count {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face Euler witness closed-cycle mismatch: cycles={}, walk={}",
                    closed_boundary_cycle_count, walk.closed_face_count
                ),
            });
        }

        let boundary_vertex_count = boundary_vertices.len();
        let boundary_edge_count = boundary_edges.len();
        let cross_component_edge_count = cross_component_edges.len();
        let to_i64 = |label: &str, value: usize| {
            i64::try_from(value).map_err(|_| crate::PolygonizeError::InternalInvariantViolation {
                reason: format!("global face Euler witness {label} count overflows i64"),
            })
        };
        let boundary_vertex_count_i64 = to_i64("vertex", boundary_vertex_count)?;
        let boundary_edge_count_i64 = to_i64("edge", boundary_edge_count)?;
        let closed_boundary_cycle_count_i64 = to_i64("cycle", closed_boundary_cycle_count)?;
        let boundary_euler_lhs = boundary_vertex_count_i64
            .checked_sub(boundary_edge_count_i64)
            .and_then(|value| value.checked_add(closed_boundary_cycle_count_i64))
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face Euler witness lhs overflow".to_string(),
            })?;
        let boundary_euler_rhs = to_i64("component", self.global_components.len())?
            .checked_add(1)
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face Euler witness rhs overflow".to_string(),
            })?;

        Ok(PartitionBorderGlobalFaceEulerWitnessStats {
            component_count: self.global_components.len(),
            transition_face_count: self.global_face_transitions.len(),
            closed_boundary_cycle_count,
            boundary_vertex_count,
            boundary_edge_count,
            cross_component_edge_count,
            boundary_euler_lhs,
            boundary_euler_rhs,
            boundary_euler_consistent: cross_component_edge_count == 0
                && boundary_euler_lhs == boundary_euler_rhs,
        })
    }

    /// Applies a conservative proof boundary to local-unbounded evidence.
    /// Exactly one local unbounded marker is a candidate only when every face
    /// cycle is closed, every border twin is mapped, and every unbounded-face
    /// twin is mutation-ready. Multiple local markers, including markers that
    /// share one retained connected component, remain unresolved evidence.
    #[cfg(test)]
    pub(crate) fn validate_global_unbounded_face_proof(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalUnboundedFaceProofStats> {
        execution_policy.check_cancelled("partition_border_global_unbounded_face_proof")?;
        let walk = self.validate_global_face_walk_invariants(execution_policy)?;
        self.validate_global_unbounded_face_proof_with_walk(execution_policy, walk)
    }

    pub(crate) fn validate_global_unbounded_face_proof_with_walk(
        &self,
        execution_policy: &ExecutionPolicy,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
    ) -> crate::Result<PartitionBorderGlobalUnboundedFaceProofStats> {
        execution_policy.check_cancelled("partition_border_global_unbounded_face_proof")?;
        let unbounded_faces = self
            .global_face_plans
            .iter()
            .filter(|plan| plan.local_face_is_unbounded)
            .map(|plan| plan.face_ref)
            .collect::<BTreeSet<_>>();
        let local_unbounded_face_count = unbounded_faces.len();
        let closed_unbounded_face_count = self
            .global_face_transitions
            .iter()
            .filter(|transition| transition.local_face_is_unbounded && transition.closed)
            .count();
        let mapped_twin_links = self
            .global_face_twin_transitions
            .iter()
            .map(|link| (link.edge_key, link))
            .collect::<BTreeMap<_, _>>();
        let mut unbounded_face_twin_count = 0usize;
        let mut unbounded_face_unmapped_twin_count = 0usize;
        let mut unbounded_face_not_ready_twin_count = 0usize;
        for (twin_index, twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_unbounded_face_proof_twins",
                twin_index,
            )?;
            if !unbounded_faces.contains(&twin.forward_face_ref)
                && !unbounded_faces.contains(&twin.reverse_face_ref)
            {
                continue;
            }
            unbounded_face_twin_count += 1;
            let Some(link) = mapped_twin_links.get(&twin.twin.edge_key) else {
                unbounded_face_unmapped_twin_count += 1;
                continue;
            };
            if !(link.forward_cycle_closed && link.reverse_cycle_closed) {
                unbounded_face_not_ready_twin_count += 1;
            }
        }
        let candidate_count = usize::from(local_unbounded_face_count == 1);
        let proof_ready = candidate_count == 1
            && walk.face_count > 0
            && walk.closed_face_count == walk.face_count
            && closed_unbounded_face_count == 1
            && walk.unbounded_component_count == 1
            && walk.unmapped_twin_count == 0
            && walk.source_complete_twin_count == walk.applied_twin_count
            && unbounded_face_unmapped_twin_count == 0
            && unbounded_face_not_ready_twin_count == 0;
        debug_assert!(unbounded_face_unmapped_twin_count == 0 || !proof_ready);

        Ok(PartitionBorderGlobalUnboundedFaceProofStats {
            face_count: walk.face_count,
            local_unbounded_face_count,
            unbounded_component_count: walk.unbounded_component_count,
            closed_unbounded_face_count,
            unbounded_face_twin_count,
            unbounded_face_unmapped_twin_count,
            unbounded_face_not_ready_twin_count,
            candidate_count,
            proof_ready,
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

    fn prepared_global_face_walk_graph(closed: bool, unbounded: [bool; 2]) -> PartitionBorderGraph {
        let mut graph = exact_face_twin_graph_with_successors();
        for (observation_index, observation) in graph.observations.values_mut().enumerate() {
            observation.local_face_is_unbounded = unbounded[observation_index];
            if closed {
                observation.local_face_boundary_successor = Some(observation.observation_id());
            }
        }
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_components(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_twin_transitions(&ExecutionPolicy::default())
            .unwrap();
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
    fn global_component_payloads_union_provenance_and_z_deterministically() {
        let mut graph = exact_face_twin_graph();
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_components(&ExecutionPolicy::default())
            .unwrap();
        let stats = graph
            .reconcile_global_component_payloads(&ExecutionPolicy::default())
            .unwrap();

        assert_eq!(
            stats,
            PartitionBorderGlobalComponentPayloadStats {
                component_count: 1,
                source_line_count: 2,
                representative_line_count: 2,
                z_candidate_count: 4,
                selected_z_node_count: 2,
                z_conflict_node_count: 2,
                z_conflict_component_count: 1,
            }
        );
        assert_eq!(graph.global_component_payloads().len(), 1);
        let payload = &graph.global_component_payloads()[0];
        assert_eq!(payload.component_index, 0);
        assert_eq!(payload.face_refs, vec![face(1, 4, 9), face(2, 6, 11)]);
        assert_eq!(
            payload.border_node_keys,
            graph.global_components()[0].border_node_keys
        );
        assert_eq!(payload.source_line_ids, vec![7, 8]);
        assert_eq!(payload.representative_line_ids, vec![7, 8]);
        assert_eq!(
            payload.z_bits,
            vec![
                1.0f64.to_bits(),
                2.0f64.to_bits(),
                10.0f64.to_bits(),
                20.0f64.to_bits(),
            ]
        );
        assert_eq!(payload.selected_z_policy, ZPolicy::InterpolateAlongEdge);
        assert_eq!(payload.z_conflict_node_count, 2);
        assert_eq!(
            payload.selected_z_bits,
            vec![
                (
                    PartitionBorderNodeKey::from_coord(coord(0.0, 0.0, 0.0)),
                    10.0f64.to_bits(),
                ),
                (
                    PartitionBorderNodeKey::from_coord(coord(2.0, 0.0, 0.0)),
                    20.0f64.to_bits(),
                ),
            ]
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
        reordered
            .reconcile_global_component_payloads(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            reordered.global_component_payloads(),
            graph.global_component_payloads()
        );
    }

    #[test]
    fn global_component_payloads_are_bounded_cancellable_and_atomic() {
        let mut limited = exact_face_twin_graph();
        limited
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        limited
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        limited
            .reconcile_global_components(&ExecutionPolicy::default())
            .unwrap();
        let before = limited.clone();
        let error = limited
            .reconcile_global_component_payloads(&ExecutionPolicy {
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
            } if stage == "partition_border_global_component_payload_nodes"
        ));
        assert_eq!(limited, before);

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = before.clone();
        let error = cancelled
            .reconcile_global_component_payloads(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_component_payloads"
        ));
        assert_eq!(cancelled, before);

        let mut malformed = before.clone();
        malformed.global_components[0].border_node_keys[0] =
            PartitionBorderNodeKey::from_coord(coord(99.0, 99.0, 0.0));
        let error = malformed
            .reconcile_global_component_payloads(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("is unreconciled")
        ));
        assert!(malformed.global_component_payloads().is_empty());
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
                missing_boundary_successor_count: 2,
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
        assert_eq!(stats.missing_boundary_successor_count, 0);
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
    fn global_face_mutation_gate_reports_incomplete_and_closed_boundary_cycles() {
        let mut incomplete = exact_face_twin_graph_with_successors();
        incomplete
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        incomplete
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = incomplete.clone();
        assert_eq!(
            incomplete
                .validate_global_face_mutation_gate(&ExecutionPolicy::default())
                .unwrap(),
            PartitionBorderGlobalFaceMutationGateStats {
                face_count: 2,
                candidate_count: 2,
                boundary_transition_count: 0,
                missing_boundary_successor_count: 2,
                mutation_ready_face_count: 0,
            }
        );
        assert_eq!(incomplete, before);

        let mut complete = exact_face_twin_graph_with_successors();
        let observation_ids = complete.observations.keys().copied().collect::<Vec<_>>();
        for observation_id in observation_ids {
            complete
                .observations
                .get_mut(&observation_id)
                .expect("observation identity")
                .local_face_boundary_successor = Some(observation_id);
        }
        complete
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        complete
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            complete
                .validate_global_face_mutation_gate(&ExecutionPolicy::default())
                .unwrap(),
            PartitionBorderGlobalFaceMutationGateStats {
                face_count: 2,
                candidate_count: 2,
                boundary_transition_count: 2,
                missing_boundary_successor_count: 0,
                mutation_ready_face_count: 2,
            }
        );
        let first_id = complete.observations.keys().next().copied().unwrap();
        let second_id = complete.observations.keys().nth(1).copied().unwrap();
        complete
            .observations
            .get_mut(&first_id)
            .unwrap()
            .local_face_boundary_successor = Some(second_id);
        complete
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let error = complete
            .validate_global_face_mutation_gate(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("crosses face")
        ));
    }

    #[test]
    fn global_face_transition_plans_are_ordered_equivalent_and_fail_closed() {
        let mut incomplete = exact_face_twin_graph_with_successors();
        incomplete
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        incomplete
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let stats = incomplete
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceTransitionPlanStats {
                face_count: 2,
                candidate_count: 2,
                boundary_transition_count: 0,
                missing_boundary_successor_count: 2,
                closed_face_count: 0,
                incomplete_face_count: 2,
            }
        );
        assert!(incomplete
            .global_face_transitions()
            .iter()
            .all(|plan| !plan.closed && plan.boundary_observation_ids.len() == 1));

        let mut complete = exact_face_twin_graph_with_successors();
        let observation_ids = complete.observations.keys().copied().collect::<Vec<_>>();
        for observation_id in observation_ids {
            complete
                .observations
                .get_mut(&observation_id)
                .expect("observation identity")
                .local_face_boundary_successor = Some(observation_id);
        }
        complete
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        complete
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        let stats = complete
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.closed_face_count, 2);
        assert_eq!(stats.incomplete_face_count, 0);
        assert!(complete
            .global_face_transitions()
            .iter()
            .all(|plan| plan.closed && plan.boundary_observation_ids.len() == 1));
        let mut reordered = PartitionBorderGraph::default();
        for adjacency in complete.adjacencies.iter().copied() {
            reordered.declare_adjacency(adjacency);
        }
        for observation in complete.observations.values().rev().cloned() {
            reordered.insert(observation).unwrap();
        }
        reordered
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            reordered.global_face_transitions(),
            complete.global_face_transitions()
        );

        complete.global_face_plans[0].candidates[0].local_face_boundary_successor =
            Some(complete.global_face_plans[1].candidates[0].observation_id);
        let before = complete.clone();
        let error = complete
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("disagrees with its observation")
        ));
        assert_eq!(complete, before);
    }

    #[test]
    fn global_face_twin_transitions_position_declared_twins_deterministically() {
        let mut incomplete = exact_face_twin_graph_with_successors();
        incomplete
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        incomplete
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        incomplete
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        let stats = incomplete
            .reconcile_global_face_twin_transitions(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceTwinTransitionStats {
                face_count: 2,
                transition_count: 2,
                applied_twin_count: 1,
                mapped_twin_count: 1,
                unmapped_twin_count: 0,
                mutation_ready_twin_count: 0,
            }
        );
        assert_eq!(incomplete.global_face_twin_transitions().len(), 1);

        let mut complete = exact_face_twin_graph_with_successors();
        let observation_ids = complete.observations.keys().copied().collect::<Vec<_>>();
        for observation_id in observation_ids {
            complete
                .observations
                .get_mut(&observation_id)
                .expect("observation identity")
                .local_face_boundary_successor = Some(observation_id);
        }
        complete
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        complete
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        complete
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        let stats = complete
            .reconcile_global_face_twin_transitions(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.mutation_ready_twin_count, 1);

        let mut reordered = PartitionBorderGraph::default();
        for adjacency in complete.adjacencies.iter().copied() {
            reordered.declare_adjacency(adjacency);
        }
        for observation in complete.observations.values().rev().cloned() {
            reordered.insert(observation).unwrap();
        }
        reordered
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_twin_transitions(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            reordered.global_face_twin_transitions(),
            complete.global_face_twin_transitions()
        );

        let reverse_id = complete.observations.keys().nth(1).copied().unwrap();
        complete.global_face_transitions[0].boundary_observation_ids[0] = reverse_id;
        let before = complete.clone();
        let error = complete
            .reconcile_global_face_twin_transitions(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("disagrees with face")
        ));
        assert_eq!(complete, before);
    }

    #[test]
    fn global_face_walk_invariants_validate_cycles_payload_and_connectivity() {
        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let before = graph.clone();
        let stats = graph
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceWalkInvariantStats {
                face_count: 2,
                transition_count: 2,
                closed_face_count: 2,
                applied_twin_count: 1,
                mapped_twin_count: 1,
                unmapped_twin_count: 0,
                mutation_ready_twin_count: 1,
                component_count: 1,
                unbounded_face_count: 0,
                unbounded_component_count: 0,
                source_complete_twin_count: 1,
                face_adjacency_cycle_rank: 0,
            }
        );
        assert_eq!(graph, before);

        let incomplete = prepared_global_face_walk_graph(false, [false, false]);
        let stats = incomplete
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.closed_face_count, 0);
        assert_eq!(stats.mutation_ready_twin_count, 0);
        assert_eq!(stats.mapped_twin_count, 1);
        assert_eq!(stats.source_complete_twin_count, 1);

        let unbounded = prepared_global_face_walk_graph(true, [true, true]);
        let stats = unbounded
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.unbounded_face_count, 2);
        assert_eq!(stats.unbounded_component_count, 1);
    }

    #[test]
    fn global_face_walk_invariants_fail_closed_on_cycle_payload_and_limits() {
        let mut cycle_corruption = prepared_global_face_walk_graph(true, [false, false]);
        let reverse_id = cycle_corruption
            .observations
            .keys()
            .nth(1)
            .copied()
            .unwrap();
        cycle_corruption.global_face_transitions[0].boundary_observation_ids[0] = reverse_id;
        let before = cycle_corruption.clone();
        let error = cycle_corruption
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("absent candidate")
        ));
        assert_eq!(cycle_corruption, before);

        let mut payload_corruption = prepared_global_face_walk_graph(true, [false, false]);
        payload_corruption.applied_face_twins[0]
            .payload
            .source_line_ids
            .push(99);
        let before = payload_corruption.clone();
        let error = payload_corruption
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("payload disagrees")
        ));
        assert_eq!(payload_corruption, before);

        let limited = prepared_global_face_walk_graph(true, [false, false]);
        let error = limited
            .validate_global_face_walk_invariants(&ExecutionPolicy {
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
            } if stage == "partition_border_global_face_walk_faces"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let cancelled = prepared_global_face_walk_graph(true, [false, false])
            .validate_global_face_walk_invariants(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            cancelled,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_walk_invariants"
        ));
    }

    #[test]
    fn global_face_euler_witness_is_explicitly_border_only_and_fail_closed() {
        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let stats = graph
            .validate_global_face_euler_witness(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceEulerWitnessStats {
                component_count: 1,
                transition_face_count: 2,
                closed_boundary_cycle_count: 2,
                boundary_vertex_count: 2,
                boundary_edge_count: 1,
                cross_component_edge_count: 0,
                boundary_euler_lhs: 3,
                boundary_euler_rhs: 2,
                boundary_euler_consistent: false,
            }
        );

        let incomplete = prepared_global_face_walk_graph(false, [false, false]);
        let stats = incomplete
            .validate_global_face_euler_witness(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.closed_boundary_cycle_count, 0);
        assert_eq!(stats.boundary_euler_lhs, 1);
        assert!(!stats.boundary_euler_consistent);

        let walk = graph
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        let error = graph
            .validate_global_face_euler_witness_with_walk(
                &ExecutionPolicy {
                    max_graph_nodes: Some(1),
                    ..Default::default()
                },
                walk,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            } if stage == "partition_border_global_face_euler_witness_faces"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .validate_global_face_euler_witness_with_walk(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                walk,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_euler_witness"
        ));

        let mut malformed = graph.clone();
        malformed.global_face_transitions[0].boundary_observation_ids[0] =
            PartitionBorderObservationId {
                partition_id: 99,
                local_dir_edge_id: 99,
                edge_key: malformed.global_face_transitions[0].boundary_observation_ids[0].edge_key,
            };
        let error = malformed
            .validate_global_face_euler_witness(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("absent candidate")
        ));
    }

    #[test]
    fn global_face_next_candidates_are_deterministic_and_non_mutating() {
        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let stats = graph
            .clone()
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceNextCandidateStats {
                component_count: 1,
                twin_candidate_count: 1,
                ready_candidate_count: 1,
                incomplete_candidate_count: 0,
                global_successor_count: 2,
            }
        );

        let mut complete = graph.clone();
        let before = complete.clone();
        let stats = complete
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.ready_candidate_count, 1);
        assert_ne!(complete, before);
        let candidate = complete
            .global_face_next_candidates()
            .first()
            .copied()
            .expect("global next candidate");
        assert!(candidate.ready);
        assert_eq!(candidate.component_index, 0);
        assert_eq!(
            candidate.forward_predecessor,
            Some(candidate.forward_observation_id)
        );
        assert_eq!(
            candidate.forward_successor,
            Some(candidate.forward_observation_id)
        );
        assert_eq!(
            candidate.reverse_predecessor,
            Some(candidate.reverse_observation_id)
        );
        assert_eq!(
            candidate.reverse_successor,
            Some(candidate.reverse_observation_id)
        );
        assert_eq!(
            candidate.forward_global_successor,
            Some(candidate.reverse_observation_id)
        );
        assert_eq!(
            candidate.reverse_global_successor,
            Some(candidate.forward_observation_id)
        );
        assert_eq!(complete.global_face_plans(), before.global_face_plans());
        assert_eq!(
            complete.global_face_transitions(),
            before.global_face_transitions()
        );
        assert_eq!(
            complete.global_face_twin_transitions(),
            before.global_face_twin_transitions()
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
        reordered
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_twin_transitions(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            reordered.global_face_next_candidates(),
            complete.global_face_next_candidates()
        );
    }

    #[test]
    fn global_face_next_candidates_preserve_incomplete_cycles_and_fail_closed() {
        let mut incomplete = prepared_global_face_walk_graph(false, [false, false]);
        let stats = incomplete
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceNextCandidateStats {
                component_count: 1,
                twin_candidate_count: 1,
                ready_candidate_count: 0,
                incomplete_candidate_count: 1,
                global_successor_count: 0,
            }
        );
        let candidate = incomplete
            .global_face_next_candidates()
            .first()
            .copied()
            .expect("incomplete global next candidate");
        assert!(!candidate.ready);
        assert_eq!(candidate.forward_predecessor, None);
        assert_eq!(candidate.forward_successor, None);
        assert_eq!(candidate.reverse_predecessor, None);
        assert_eq!(candidate.reverse_successor, None);
        assert_eq!(candidate.forward_global_successor, None);
        assert_eq!(candidate.reverse_global_successor, None);

        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let walk = graph
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        let error = graph
            .clone()
            .reconcile_global_face_next_candidates_with_walk(
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
                walk,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 1,
            } if stage == "partition_border_global_face_next_candidates_twins"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .clone()
            .reconcile_global_face_next_candidates_with_walk(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                walk,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_next_candidates"
        ));

        let mut malformed = graph.clone();
        malformed.global_face_twin_transitions[0].forward_cycle_index += 1;
        let before = malformed.clone();
        let error = malformed
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("cycle")
        ));
        assert_eq!(malformed, before);
        assert!(malformed.global_face_next_candidates().is_empty());
    }

    #[test]
    fn global_face_identity_plans_are_deterministic_and_boundary_only() {
        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let mut complete = graph.clone();
        let stats = complete
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceIdentityPlanStats {
                component_count: 1,
                boundary_observation_count: 2,
                candidate_cycle_count: 1,
                closed_cycle_count: 1,
                incomplete_component_count: 0,
                non_permutation_component_count: 0,
                permutation_ready: true,
            }
        );
        let plan = complete
            .global_face_identity_plans()
            .first()
            .expect("global face identity candidate");
        assert!(plan.closed);
        assert_eq!(plan.component_index, 0);
        assert_eq!(plan.boundary_observation_ids.len(), 2);
        assert_eq!(plan.face_refs.len(), 2);
        assert_eq!(complete.global_face_plans(), graph.global_face_plans());
        assert_eq!(
            complete.global_face_transitions(),
            graph.global_face_transitions()
        );
        assert_eq!(
            complete.global_face_twin_transitions(),
            graph.global_face_twin_transitions()
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
        reordered
            .reconcile_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_transitions(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_twin_transitions(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            reordered.global_face_identity_plans(),
            complete.global_face_identity_plans()
        );
    }

    #[test]
    fn global_face_identity_plans_fail_closed_for_incomplete_and_non_permutation_walks() {
        let mut incomplete = prepared_global_face_walk_graph(false, [false, false]);
        let stats = incomplete
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.boundary_observation_count, 2);
        assert_eq!(stats.candidate_cycle_count, 1);
        assert_eq!(stats.closed_cycle_count, 0);
        assert_eq!(stats.incomplete_component_count, 1);
        assert_eq!(stats.non_permutation_component_count, 0);
        assert!(!stats.permutation_ready);
        assert!(!incomplete.global_face_identity_plans()[0].closed);

        let mut non_permutation = prepared_global_face_walk_graph(true, [false, false]);
        non_permutation
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap();
        let candidate = non_permutation.global_face_next_candidates()[0];
        non_permutation.global_face_next_candidates[0].forward_global_successor =
            Some(PartitionBorderObservationId {
                partition_id: 99,
                local_dir_edge_id: 99,
                edge_key: candidate.edge_key,
            });
        let stats = non_permutation
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.incomplete_component_count, 0);
        assert_eq!(stats.non_permutation_component_count, 1);
        assert!(!stats.permutation_ready);
        assert!(!non_permutation.global_face_identity_plans()[0].closed);

        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let walk = graph
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        let error = graph
            .clone()
            .reconcile_global_face_identity_plans_with_walk(
                &ExecutionPolicy {
                    max_graph_nodes: Some(1),
                    ..Default::default()
                },
                walk,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            } if stage == "partition_border_global_face_identity_plans_faces"
        ));

        let mut malformed = prepared_global_face_walk_graph(true, [false, false]);
        malformed
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap();
        malformed.global_face_next_candidates[0].forward_global_successor = None;
        let before = malformed.clone();
        let error = malformed
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("forward global successor")
        ));
        assert_eq!(malformed, before);
        assert!(malformed.global_face_identity_plans().is_empty());
    }

    #[test]
    fn global_unbounded_face_proof_is_conservative_about_marker_multiplicity() {
        let single = prepared_global_face_walk_graph(true, [true, false]);
        assert_eq!(
            single
                .validate_global_unbounded_face_proof(&ExecutionPolicy::default())
                .unwrap(),
            PartitionBorderGlobalUnboundedFaceProofStats {
                face_count: 2,
                local_unbounded_face_count: 1,
                unbounded_component_count: 1,
                closed_unbounded_face_count: 1,
                unbounded_face_twin_count: 1,
                unbounded_face_unmapped_twin_count: 0,
                unbounded_face_not_ready_twin_count: 0,
                candidate_count: 1,
                proof_ready: true,
            }
        );

        let multiple = prepared_global_face_walk_graph(true, [true, true]);
        let stats = multiple
            .validate_global_unbounded_face_proof(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.local_unbounded_face_count, 2);
        assert_eq!(stats.unbounded_component_count, 1);
        assert_eq!(stats.candidate_count, 0);
        assert!(!stats.proof_ready);

        let incomplete = prepared_global_face_walk_graph(false, [true, false]);
        let stats = incomplete
            .validate_global_unbounded_face_proof(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.candidate_count, 1);
        assert_eq!(stats.closed_unbounded_face_count, 0);
        assert_eq!(stats.unbounded_face_not_ready_twin_count, 1);
        assert!(!stats.proof_ready);
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
