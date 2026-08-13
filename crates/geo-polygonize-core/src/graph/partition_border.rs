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

/// One active directed edge captured from a processed tile-local component.
/// Local directed-edge IDs remain qualified by partition and component so the
/// future global graph can remap them without relying on tile iteration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderLocalDirectedEdge {
    pub(crate) local_dir_edge_id: DirEdgeId,
    pub(crate) symmetric_local_dir_edge_id: DirEdgeId,
    pub(crate) local_face_successor: Option<DirEdgeId>,
    pub(crate) from: PartitionBorderNodeKey,
    pub(crate) to: PartitionBorderNodeKey,
    pub(crate) from_z_bits: u64,
    pub(crate) to_z_bits: u64,
    pub(crate) edge_key: PartitionBorderEdgeKey,
    pub(crate) face_ref: Option<PartitionFaceRef>,
    pub(crate) local_face_is_unbounded: bool,
    pub(crate) source_line_ids: Vec<u32>,
}

/// Processed local face-edge lineage retained for a future global topology.
/// It contains only active local arrangement edges; no global links are
/// written while the snapshot is captured.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderLocalFaceGraph {
    pub(crate) partition_id: usize,
    pub(crate) component_id: usize,
    pub(crate) directed_edges: Vec<PartitionBorderLocalDirectedEdge>,
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

/// One deterministic global edge slot backed by an active tile-local edge.
/// The local symmetric and successor identities are remapped into this slot
/// space, while declared cross-border twins remain explicit side metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceEdge {
    pub(crate) global_dir_edge_id: usize,
    pub(crate) partition_id: usize,
    pub(crate) component_id: usize,
    pub(crate) local_dir_edge_id: DirEdgeId,
    pub(crate) symmetric_global_dir_edge_id: usize,
    pub(crate) local_face_successor_global_dir_edge_id: Option<usize>,
    pub(crate) cross_border_twin_global_dir_edge_id: Option<usize>,
    pub(crate) from_global_node_id: Option<usize>,
    pub(crate) to_global_node_id: Option<usize>,
    pub(crate) from: PartitionBorderNodeKey,
    pub(crate) to: PartitionBorderNodeKey,
    pub(crate) from_z_bits: u64,
    pub(crate) to_z_bits: u64,
    pub(crate) edge_key: PartitionBorderEdgeKey,
    pub(crate) face_ref: Option<PartitionFaceRef>,
    pub(crate) local_face_is_unbounded: bool,
    pub(crate) source_line_ids: Vec<u32>,
}

/// Counts from remapping active local face-edge lineage into deterministic
/// global edge slots. No global node, twin, successor, or face ID is mutated.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceEdgeMapStats {
    pub(crate) local_graph_count: usize,
    pub(crate) component_count: usize,
    pub(crate) directed_edge_count: usize,
    pub(crate) local_successor_count: usize,
    pub(crate) mapped_observation_count: usize,
    pub(crate) mapped_twin_count: usize,
    pub(crate) unmapped_twin_count: usize,
    pub(crate) edge_map_ready: bool,
}

/// One deterministic global node slot backed by active local face-edge
/// endpoints. The slot retains every contributing source, face, observation,
/// and endpoint-Z candidate; it does not rewrite local or tiled topology.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceNode {
    pub(crate) global_node_id: usize,
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
    pub(crate) incident_global_dir_edge_ids: Vec<usize>,
}

/// Counts from canonical global face-node reconciliation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceNodeReconciliationStats {
    pub(crate) edge_count: usize,
    pub(crate) node_count: usize,
    pub(crate) endpoint_count: usize,
    pub(crate) mapped_observation_count: usize,
    pub(crate) unmapped_observation_count: usize,
    pub(crate) z_candidate_count: usize,
    pub(crate) z_conflict_count: usize,
    pub(crate) node_map_ready: bool,
}

#[derive(Default)]
struct GlobalFaceNodeEvidence {
    observation_ids: BTreeSet<PartitionBorderObservationId>,
    source_line_ids: BTreeSet<u32>,
    representative_line_ids: BTreeSet<u32>,
    face_refs: BTreeSet<PartitionFaceRef>,
    z_bits: BTreeSet<u64>,
    z_candidates: Vec<(u32, PartitionBorderObservationId, u64)>,
    incident_global_dir_edge_ids: BTreeSet<usize>,
}

/// One deterministic candidate global successor cycle expressed in global
/// edge-slot space. The links are retained for a future topology mutation;
/// neither local `next` nor any production graph link is written here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceNextApplicationPlan {
    pub(crate) component_index: usize,
    pub(crate) global_dir_edge_ids: Vec<usize>,
    pub(crate) successor_global_dir_edge_ids: Vec<usize>,
    pub(crate) closed: bool,
    pub(crate) node_continuous: bool,
}

/// Counts from mapping validated global-face mutation cycles into global edge
/// slots. `application_ready` means only that this retained boundary plan is
/// exact and node-continuous; it is not permission to mutate topology.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceNextApplicationStats {
    pub(crate) component_count: usize,
    pub(crate) plan_count: usize,
    pub(crate) candidate_link_count: usize,
    pub(crate) mapped_edge_count: usize,
    pub(crate) mapped_twin_count: usize,
    pub(crate) unmapped_observation_count: usize,
    pub(crate) incomplete_plan_count: usize,
    pub(crate) node_discontinuity_count: usize,
    pub(crate) application_ready: bool,
}

/// A detached full directed-edge successor candidate assembled from local
/// successors and any validated global boundary overrides. It is deliberately
/// separate from the production graph, so cycle validation cannot mutate a
/// local or tiled `next` link by accident.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalTopologyCandidate {
    pub(crate) next_global_dir_edge_ids: Vec<Option<usize>>,
    pub(crate) cycle_start_global_dir_edge_ids: Vec<usize>,
}

/// Counts the one atomic commit of detached global successor links. Face IDs
/// remain a separate roadmap slice; no local face identity or output is
/// changed by this mutation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalTopologyMutationStats {
    pub(crate) edge_count: usize,
    pub(crate) applied_next_count: usize,
    pub(crate) mutation_ready: bool,
    pub(crate) applied: bool,
}

/// Counts the atomic commit of deterministic candidate face IDs onto detached
/// global successor cycles. Local face IDs and output payloads remain private.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdMutationStats {
    pub(crate) candidate_cycle_count: usize,
    pub(crate) applied_face_id_count: usize,
    pub(crate) unbounded_face_id_count: usize,
    pub(crate) mutation_ready: bool,
    pub(crate) applied: bool,
}

/// Counts the atomic promotion of the uniquely proven unbounded face onto
/// detached global identity state. Local face identities and output payloads
/// remain private until their own promotion proofs exist.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalUnboundedFaceMutationStats {
    pub(crate) candidate_cycle_count: usize,
    pub(crate) candidate_unbounded_face_id_count: usize,
    pub(crate) applied_unbounded_face_id: Option<usize>,
    pub(crate) applied_cycle_start_global_dir_edge_id: Option<usize>,
    pub(crate) mutation_ready: bool,
    pub(crate) applied: bool,
}

/// Counts detached per-edge face identity materialization after successor,
/// cycle, face-ID, and unbounded-face commits. This remains private evidence;
/// local face IDs and tiled output are untouched.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdentityMaterializationStats {
    pub(crate) edge_count: usize,
    pub(crate) cycle_count: usize,
    pub(crate) assigned_edge_count: usize,
    pub(crate) missing_face_id_count: usize,
    pub(crate) duplicate_edge_count: usize,
    pub(crate) invalid_cycle_count: usize,
    pub(crate) unbounded_edge_count: usize,
    pub(crate) materialization_ready: bool,
}

/// Counts the final detached global face-identity invariant check. This
/// cross-checks the per-edge face map against committed cycles, successor
/// continuity, reciprocal twins, source lineage, and retained walk/Euler
/// evidence without mutating any graph or output state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdentityInvariantStats {
    pub(crate) edge_count: usize,
    pub(crate) cycle_count: usize,
    pub(crate) mapped_face_id_edge_count: usize,
    pub(crate) face_id_set_count: usize,
    pub(crate) missing_face_id_count: usize,
    pub(crate) cycle_face_mismatch_count: usize,
    pub(crate) successor_discontinuity_count: usize,
    pub(crate) source_incomplete_edge_count: usize,
    pub(crate) twin_count: usize,
    pub(crate) twin_mapping_mismatch_count: usize,
    pub(crate) face_walk_ready: bool,
    pub(crate) euler_evidence_ready: bool,
    pub(crate) invariants_ready: bool,
}

/// Counts the final detached global-next lineage integration check. This
/// proves that the committed successor permutation agrees with local face
/// successors, retained boundary overrides, per-edge face identity, and
/// cross-partition face-qualified twins without mutating topology or output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalNextLineageIntegrationStats {
    pub(crate) edge_count: usize,
    pub(crate) cycle_count: usize,
    pub(crate) local_successor_count: usize,
    pub(crate) override_count: usize,
    pub(crate) integrated_successor_count: usize,
    pub(crate) missing_candidate_successor_count: usize,
    pub(crate) local_lineage_mismatch_count: usize,
    pub(crate) override_lineage_mismatch_count: usize,
    pub(crate) application_plan_link_count: usize,
    pub(crate) unrepresented_application_link_count: usize,
    pub(crate) committed_next_edge_count: usize,
    pub(crate) committed_next_mismatch_count: usize,
    pub(crate) twin_count: usize,
    pub(crate) twin_lineage_mismatch_count: usize,
    pub(crate) identity_ready: bool,
    pub(crate) integration_ready: bool,
}

/// Counts detached cycle-to-face lineage integration. Every closed candidate
/// cycle must map to exactly one candidate face plan whose observation slots,
/// qualified local faces, and unbounded marker agree; this remains evidence
/// only and never promotes topology or output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalCycleFaceLineageStats {
    pub(crate) edge_count: usize,
    pub(crate) cycle_count: usize,
    pub(crate) plan_count: usize,
    pub(crate) closed_cycle_count: usize,
    pub(crate) mapped_cycle_count: usize,
    pub(crate) incomplete_cycle_count: usize,
    pub(crate) invalid_cycle_count: usize,
    pub(crate) missing_face_id_count: usize,
    pub(crate) duplicate_face_id_plan_count: usize,
    pub(crate) unmapped_plan_count: usize,
    pub(crate) cycle_plan_mismatch_count: usize,
    pub(crate) cycle_face_ref_mismatch_count: usize,
    pub(crate) duplicate_plan_face_ref_count: usize,
    pub(crate) observation_lineage_mismatch_count: usize,
    pub(crate) unbounded_lineage_mismatch_count: usize,
    pub(crate) identity_ready: bool,
    pub(crate) next_lineage_ready: bool,
    pub(crate) lineage_ready: bool,
}

/// Counts the detached evidence required before a future global face
/// promotion. This is a cross-check after cycle-to-face lineage: it does not
/// replace the pre-mutation gate and never promotes topology or output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalCycleFacePromotionGateStats {
    pub(crate) edge_count: usize,
    pub(crate) cycle_count: usize,
    pub(crate) plan_count: usize,
    pub(crate) component_count: usize,
    pub(crate) face_count: usize,
    pub(crate) covered_face_edge_count: usize,
    pub(crate) candidate_unbounded_face_id_count: usize,
    pub(crate) mapped_unbounded_cycle_count: usize,
    pub(crate) lineage_ready: bool,
    pub(crate) component_coverage_ready: bool,
    pub(crate) unbounded_face_application_ready: bool,
    pub(crate) edge_count_mismatch_count: usize,
    pub(crate) cycle_count_mismatch_count: usize,
    pub(crate) plan_count_mismatch_count: usize,
    pub(crate) face_count_mismatch_count: usize,
    pub(crate) unbounded_marker_mismatch_count: usize,
    pub(crate) gate_ready: bool,
}

/// Counts detached face-cycle payload lineage after the promotion gate. Each
/// edge must still agree with its source observation and both endpoint nodes;
/// this remains evidence only and never promotes stitched output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFacePayloadLineageStats {
    pub(crate) edge_count: usize,
    pub(crate) cycle_count: usize,
    pub(crate) plan_count: usize,
    pub(crate) checked_edge_count: usize,
    pub(crate) checked_cycle_count: usize,
    pub(crate) missing_face_id_count: usize,
    pub(crate) missing_plan_count: usize,
    pub(crate) missing_observation_count: usize,
    pub(crate) source_incomplete_edge_count: usize,
    pub(crate) source_lineage_mismatch_count: usize,
    pub(crate) z_lineage_mismatch_count: usize,
    pub(crate) face_lineage_mismatch_count: usize,
    pub(crate) node_lineage_mismatch_count: usize,
    pub(crate) lineage_ready: bool,
}

/// Counts from validating a detached global directed-edge topology candidate.
/// Readiness requires complete application evidence, one predecessor and one
/// successor per edge, endpoint continuity, and closed cycles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalTopologyCandidateStats {
    pub(crate) edge_count: usize,
    pub(crate) local_successor_count: usize,
    pub(crate) global_override_count: usize,
    pub(crate) assigned_next_count: usize,
    pub(crate) unassigned_next_count: usize,
    pub(crate) cycle_count: usize,
    pub(crate) closed_cycle_edge_count: usize,
    pub(crate) predecessor_conflict_count: usize,
    pub(crate) node_discontinuity_count: usize,
    pub(crate) incomplete_application_plan_count: usize,
    pub(crate) candidate_ready: bool,
}

/// Counts from the final evidence gate before a future global topology
/// mutation. This gate proves that detached successor links and every
/// cross-border twin are still backed by declared adjacency; it writes no
/// production topology.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalTopologyApplicationGateStats {
    pub(crate) edge_count: usize,
    pub(crate) candidate_successor_count: usize,
    pub(crate) declared_adjacency_count: usize,
    pub(crate) applied_twin_count: usize,
    pub(crate) mapped_twin_count: usize,
    pub(crate) unmapped_twin_count: usize,
    pub(crate) invalid_twin_count: usize,
    pub(crate) predecessor_conflict_count: usize,
    pub(crate) node_discontinuity_count: usize,
    pub(crate) application_ready: bool,
}

/// Counts from validating deterministic global-component coverage over the
/// detached edge-slot candidate. This is component evidence only; it does not
/// assign global face IDs or mutate topology.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalComponentCoverageStats {
    pub(crate) component_count: usize,
    pub(crate) face_count: usize,
    pub(crate) edge_count: usize,
    pub(crate) face_edge_count: usize,
    pub(crate) covered_face_edge_count: usize,
    pub(crate) uncovered_face_edge_count: usize,
    pub(crate) duplicate_face_count: usize,
    pub(crate) duplicate_twin_edge_count: usize,
    pub(crate) coverage_ready: bool,
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

/// Counts the fail-closed bridge between canonical border-node payloads and
/// active global face-node slots. Canonical-only nodes are retained as valid
/// evidence because they may belong to dangles or other non-face-qualified
/// observations; every active global node must still be a payload-consistent
/// projection of its canonical node.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderCanonicalNodeValidationStats {
    pub(crate) canonical_node_count: usize,
    pub(crate) global_node_count: usize,
    pub(crate) mapped_global_node_count: usize,
    pub(crate) canonical_only_node_count: usize,
    pub(crate) source_set_mismatch_count: usize,
    pub(crate) representative_set_mismatch_count: usize,
    pub(crate) face_set_mismatch_count: usize,
    pub(crate) z_candidate_mismatch_count: usize,
    pub(crate) selected_z_mismatch_count: usize,
    pub(crate) z_policy_mismatch_count: usize,
    pub(crate) z_conflict_mismatch_count: usize,
    pub(crate) edge_endpoint_mismatch_count: usize,
    pub(crate) invalid_global_node_id_count: usize,
    pub(crate) reconciliation_ready: bool,
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

/// Counts from applying the conservative unbounded-face proof to detached
/// candidate face-ID evidence. This gate identifies one candidate only; it
/// does not write a global face identity or mutate topology.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalUnboundedFaceApplicationStats {
    pub(crate) face_count: usize,
    pub(crate) candidate_cycle_count: usize,
    pub(crate) local_unbounded_face_count: usize,
    pub(crate) candidate_unbounded_face_id_count: usize,
    pub(crate) mapped_unbounded_cycle_count: usize,
    pub(crate) missing_unbounded_face_id_count: usize,
    pub(crate) duplicate_unbounded_face_id_count: usize,
    pub(crate) proof_ready: bool,
    pub(crate) application_ready: bool,
}

/// Combined fail-closed evidence before any future global topology mutation.
/// `gate_ready` is a proof-boundary result only; it does not authorize writes
/// to local `next`, global edge slots, face IDs, or tiled output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalTopologyMutationGateStats {
    pub(crate) edge_count: usize,
    pub(crate) component_count: usize,
    pub(crate) face_count: usize,
    pub(crate) candidate_cycle_count: usize,
    pub(crate) applied_twin_count: usize,
    pub(crate) mapped_twin_count: usize,
    pub(crate) source_complete_twin_count: usize,
    pub(crate) closed_face_count: usize,
    pub(crate) euler_boundary_lhs: i64,
    pub(crate) euler_boundary_rhs: i64,
    pub(crate) topology_application_ready: bool,
    pub(crate) component_coverage_ready: bool,
    pub(crate) face_id_application_ready: bool,
    pub(crate) unbounded_face_application_ready: bool,
    pub(crate) face_walk_ready: bool,
    pub(crate) euler_evidence_ready: bool,
    pub(crate) gate_ready: bool,
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

/// One retained global-next assignment set derived from a validated identity
/// cycle. The pair vectors are evidence for a future global topology mutation;
/// no local or global directed-edge link is written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceNextMutationPlan {
    pub(crate) component_index: usize,
    pub(crate) boundary_observation_ids: Vec<PartitionBorderObservationId>,
    pub(crate) successor_observation_ids: Vec<PartitionBorderObservationId>,
    pub(crate) closed: bool,
}

/// Counts from the fail-closed global-next mutation gate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceNextMutationPlanStats {
    pub(crate) component_count: usize,
    pub(crate) boundary_observation_count: usize,
    pub(crate) plan_count: usize,
    pub(crate) candidate_link_count: usize,
    pub(crate) ready_component_count: usize,
    pub(crate) incomplete_component_count: usize,
    pub(crate) mutation_ready: bool,
}

/// One deterministic candidate global face identity derived from a validated
/// boundary mutation cycle. The ID is evidence for a future complete global
/// arrangement; it is never written into local observations or tiled output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdPlan {
    pub(crate) candidate_global_face_id: Option<usize>,
    pub(crate) component_index: usize,
    pub(crate) boundary_observation_ids: Vec<PartitionBorderObservationId>,
    pub(crate) face_refs: Vec<PartitionFaceRef>,
    pub(crate) local_unbounded_face_count: usize,
    pub(crate) closed: bool,
}

/// Counts from assigning deterministic candidate IDs to closed boundary
/// cycles. `assignment_ready` is deliberately narrower than global topology
/// readiness: it says only that the retained boundary cycles are complete.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdPlanStats {
    pub(crate) component_count: usize,
    pub(crate) candidate_cycle_count: usize,
    pub(crate) assigned_face_count: usize,
    pub(crate) boundary_observation_count: usize,
    pub(crate) unbounded_candidate_count: usize,
    pub(crate) incomplete_plan_count: usize,
    pub(crate) assignment_ready: bool,
}

/// Counts from validating that deterministic candidate face IDs map one-to-one
/// onto the detached candidate's closed cycles. This is evidence only; it does
/// not write IDs or global `next` links.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PartitionBorderGlobalFaceIdApplicationStats {
    pub(crate) component_count: usize,
    pub(crate) candidate_cycle_count: usize,
    pub(crate) assigned_face_count: usize,
    pub(crate) candidate_cycle_start_count: usize,
    pub(crate) mapped_cycle_count: usize,
    pub(crate) unmapped_plan_count: usize,
    pub(crate) duplicate_face_id_count: usize,
    pub(crate) non_contiguous_face_id_count: usize,
    pub(crate) application_ready: bool,
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
    local_face_graphs: Vec<PartitionBorderLocalFaceGraph>,
    applied_face_twins: Vec<PartitionBorderFaceTwin>,
    global_face_edge_map: Vec<PartitionBorderGlobalFaceEdge>,
    global_face_nodes: Vec<PartitionBorderGlobalFaceNode>,
    reconciled_nodes: Vec<PartitionBorderNodePayload>,
    global_components: Vec<PartitionBorderGlobalComponent>,
    global_component_payloads: Vec<PartitionBorderGlobalComponentPayload>,
    global_face_plans: Vec<PartitionBorderGlobalFacePlan>,
    global_face_transitions: Vec<PartitionBorderGlobalFaceTransitionPlan>,
    global_face_twin_transitions: Vec<PartitionBorderGlobalFaceTwinTransition>,
    global_face_next_candidates: Vec<PartitionBorderGlobalFaceNextCandidate>,
    global_face_identity_plans: Vec<PartitionBorderGlobalFaceIdentityPlan>,
    global_face_next_mutation_plans: Vec<PartitionBorderGlobalFaceNextMutationPlan>,
    global_face_id_plans: Vec<PartitionBorderGlobalFaceIdPlan>,
    global_face_next_application_plans: Vec<PartitionBorderGlobalFaceNextApplicationPlan>,
    global_topology_candidate: Option<PartitionBorderGlobalTopologyCandidate>,
    global_next_global_dir_edge_ids: Vec<Option<usize>>,
    global_face_id_by_cycle_start: Vec<Option<usize>>,
    global_face_id_by_global_dir_edge_id: Vec<Option<usize>>,
    global_unbounded_face_id_by_cycle_start: Option<(usize, usize)>,
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
        self.local_face_graphs.clear();
        self.applied_face_twins.clear();
        self.reconciled_nodes.clear();
        self.global_components.clear();
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        self.global_next_global_dir_edge_ids.clear();
        self.global_face_id_by_cycle_start.clear();
        self.global_face_id_by_global_dir_edge_id.clear();
        self.global_unbounded_face_id_by_cycle_start = None;
        self.global_face_edge_map.clear();
        self.global_face_nodes.clear();
        Ok(())
    }

    pub(crate) fn insert_local_face_graph(
        &mut self,
        local_face_graph: PartitionBorderLocalFaceGraph,
    ) -> crate::Result<()> {
        if let Some(existing) = self.local_face_graphs.iter().find(|existing| {
            existing.partition_id == local_face_graph.partition_id
                && existing.component_id == local_face_graph.component_id
        }) {
            if existing == &local_face_graph {
                return Ok(());
            }
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "partition local face graph ({}, {}) conflicts with prior snapshot",
                    local_face_graph.partition_id, local_face_graph.component_id
                ),
            });
        }
        self.local_face_graphs.push(local_face_graph);
        self.local_face_graphs
            .sort_unstable_by_key(|graph| (graph.partition_id, graph.component_id));
        self.applied_face_twins.clear();
        self.global_components.clear();
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        self.global_face_id_by_cycle_start.clear();
        self.global_face_id_by_global_dir_edge_id.clear();
        self.global_unbounded_face_id_by_cycle_start = None;
        self.global_face_edge_map.clear();
        self.global_face_nodes.clear();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn local_face_graphs(&self) -> &[PartitionBorderLocalFaceGraph] {
        &self.local_face_graphs
    }

    pub fn declare_adjacency(&mut self, adjacency: PartitionBorderAdjacency) {
        self.adjacencies.insert(adjacency);
        self.global_face_edge_map.clear();
        self.global_face_nodes.clear();
        self.applied_face_twins.clear();
        self.reconciled_nodes.clear();
        self.global_components.clear();
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        self.global_face_id_by_cycle_start.clear();
        self.global_face_id_by_global_dir_edge_id.clear();
        self.global_unbounded_face_id_by_cycle_start = None;
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
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        self.global_face_edge_map.clear();
        self.global_face_nodes.clear();
        Ok(stats)
    }

    pub(crate) fn applied_face_twins(&self) -> &[PartitionBorderFaceTwin] {
        &self.applied_face_twins
    }

    /// Remaps active tile-local face-edge lineage into deterministic global
    /// edge slots and positions every available declared face twin in that
    /// slot space. Missing local snapshots remain explicitly unmapped; any
    /// malformed snapshot lineage fails closed before the map is committed.
    pub(crate) fn reconcile_global_face_edge_map(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceEdgeMapStats> {
        execution_policy.check_cancelled("partition_border_global_face_edge_map")?;
        execution_policy.check(
            "partition_border_global_face_edge_map_components",
            execution_policy.max_graph_nodes,
            self.local_face_graphs.len(),
        )?;
        let directed_edge_count = self
            .local_face_graphs
            .iter()
            .try_fold(0usize, |count, graph| {
                count.checked_add(graph.directed_edges.len())
            })
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face edge map directed-edge count overflow".to_string(),
            })?;
        execution_policy.check(
            "partition_border_global_face_edge_map_edges",
            execution_policy.max_graph_edges,
            directed_edge_count,
        )?;

        let mut edge_records = self
            .local_face_graphs
            .iter()
            .flat_map(|graph| {
                graph
                    .directed_edges
                    .iter()
                    .map(move |edge| (graph.partition_id, graph.component_id, edge))
            })
            .collect::<Vec<_>>();
        edge_records.sort_unstable_by_key(|(partition_id, component_id, edge)| {
            (*partition_id, *component_id, edge.local_dir_edge_id)
        });

        let mut global_index_by_local = BTreeMap::<(usize, usize, DirEdgeId), usize>::new();
        for (global_dir_edge_id, (partition_id, component_id, edge)) in
            edge_records.iter().enumerate()
        {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_edge_map",
                global_dir_edge_id,
            )?;
            if global_index_by_local
                .insert(
                    (*partition_id, *component_id, edge.local_dir_edge_id),
                    global_dir_edge_id,
                )
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face edge map local directed edge ({}, {}, {}) is duplicated",
                        partition_id, component_id, edge.local_dir_edge_id
                    ),
                });
            }
        }

        let mut global_edges = Vec::with_capacity(edge_records.len());
        let mut local_successor_count = 0usize;
        for (global_dir_edge_id, (partition_id, component_id, edge)) in
            edge_records.iter().enumerate()
        {
            let symmetric_global_dir_edge_id = *global_index_by_local
                .get(&(
                    *partition_id,
                    *component_id,
                    edge.symmetric_local_dir_edge_id,
                ))
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face edge map edge {} has no symmetric local edge",
                        global_dir_edge_id
                    ),
                })?;
            let symmetric = edge_records
                .get(symmetric_global_dir_edge_id)
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face edge map edge {} has invalid symmetric slot {}",
                        global_dir_edge_id, symmetric_global_dir_edge_id
                    ),
                })?
                .2;
            if symmetric.edge_key != edge.edge_key
                || symmetric.from != edge.to
                || symmetric.to != edge.from
                || symmetric.from_z_bits != edge.to_z_bits
                || symmetric.to_z_bits != edge.from_z_bits
                || symmetric.source_line_ids != edge.source_line_ids
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face edge map edge {} disagrees with symmetric geometry",
                        global_dir_edge_id
                    ),
                });
            }
            let local_face_successor_global_dir_edge_id = edge
                .local_face_successor
                .map(|local_face_successor| {
                    global_index_by_local
                        .get(&(*partition_id, *component_id, local_face_successor))
                        .copied()
                        .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face edge map edge {} has no local face successor",
                                global_dir_edge_id
                            ),
                        })
                })
                .transpose()?;
            if local_face_successor_global_dir_edge_id.is_some() {
                local_successor_count += 1;
            }
            global_edges.push(PartitionBorderGlobalFaceEdge {
                global_dir_edge_id,
                partition_id: *partition_id,
                component_id: *component_id,
                local_dir_edge_id: edge.local_dir_edge_id,
                symmetric_global_dir_edge_id,
                local_face_successor_global_dir_edge_id,
                cross_border_twin_global_dir_edge_id: None,
                from_global_node_id: None,
                to_global_node_id: None,
                from: edge.from,
                to: edge.to,
                from_z_bits: edge.from_z_bits,
                to_z_bits: edge.to_z_bits,
                edge_key: edge.edge_key,
                face_ref: edge.face_ref,
                local_face_is_unbounded: edge.local_face_is_unbounded,
                source_line_ids: edge.source_line_ids.clone(),
            });
        }

        let mut observation_to_global = BTreeMap::<PartitionBorderObservationId, usize>::new();
        let mut mapped_observation_count = 0usize;
        for (observation_index, observation) in self.observations.values().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_edge_map_observations",
                observation_index,
            )?;
            let Some(face_ref) = observation.face_ref else {
                continue;
            };
            let local_key = (
                observation.partition_id,
                face_ref.component_id,
                observation.local_dir_edge_id,
            );
            let Some(&global_dir_edge_id) = global_index_by_local.get(&local_key) else {
                continue;
            };
            let edge = &global_edges[global_dir_edge_id];
            let expected_successor = observation
                .local_face_successor
                .map(|local_successor| {
                    global_index_by_local
                        .get(&(
                            observation.partition_id,
                            face_ref.component_id,
                            local_successor,
                        ))
                        .copied()
                        .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face edge map observation {:?} has no local successor lineage",
                                observation.observation_id()
                            ),
                        })
                })
                .transpose()?;
            if edge.edge_key != observation.edge_key
                || edge.from != observation.from
                || edge.to != observation.to
                || edge.from_z_bits != observation.from_z_bits
                || edge.to_z_bits != observation.to_z_bits
                || edge.source_line_ids != observation.source_line_ids
                || edge.face_ref != Some(face_ref)
                || edge.local_face_successor_global_dir_edge_id != expected_successor
                || edge.local_face_is_unbounded != observation.local_face_is_unbounded
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face edge map observation {:?} disagrees with local lineage",
                        observation.observation_id()
                    ),
                });
            }
            observation_to_global.insert(observation.observation_id(), global_dir_edge_id);
            mapped_observation_count += 1;
        }

        let mut mapped_twin_count = 0usize;
        let mut unmapped_twin_count = 0usize;
        for (twin_index, twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_edge_map_twins", twin_index)?;
            let Some(&forward_global_dir_edge_id) = observation_to_global.get(&twin.twin.forward)
            else {
                unmapped_twin_count += 1;
                continue;
            };
            let Some(&reverse_global_dir_edge_id) = observation_to_global.get(&twin.twin.reverse)
            else {
                unmapped_twin_count += 1;
                continue;
            };
            if forward_global_dir_edge_id == reverse_global_dir_edge_id
                || global_edges[forward_global_dir_edge_id].edge_key != twin.twin.edge_key
                || global_edges[reverse_global_dir_edge_id].edge_key != twin.twin.edge_key
                || global_edges[forward_global_dir_edge_id].partition_id
                    == global_edges[reverse_global_dir_edge_id].partition_id
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face edge map twin {} has invalid cross-border lineage",
                        twin_index
                    ),
                });
            }
            for (from, to) in [
                (forward_global_dir_edge_id, reverse_global_dir_edge_id),
                (reverse_global_dir_edge_id, forward_global_dir_edge_id),
            ] {
                if let Some(existing) = global_edges[from].cross_border_twin_global_dir_edge_id {
                    if existing != to {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face edge map edge {} has conflicting cross-border twins {} and {}",
                                from, existing, to
                            ),
                        });
                    }
                } else {
                    global_edges[from].cross_border_twin_global_dir_edge_id = Some(to);
                }
            }
            mapped_twin_count += 1;
        }

        let stats = PartitionBorderGlobalFaceEdgeMapStats {
            local_graph_count: self.local_face_graphs.len(),
            component_count: self.local_face_graphs.len(),
            directed_edge_count,
            local_successor_count,
            mapped_observation_count,
            mapped_twin_count,
            unmapped_twin_count,
            edge_map_ready: unmapped_twin_count == 0,
        };
        self.global_face_edge_map = global_edges;
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        self.global_face_nodes.clear();
        Ok(stats)
    }

    /// Validates that each detached closed successor cycle maps exactly once
    /// to a candidate global face plan. Observation slots, qualified local
    /// face references, and local-unbounded markers must agree with the
    /// mapped plan; incomplete cycles remain evidence rather than becoming a
    /// topology error.
    pub(crate) fn validate_global_cycle_face_lineage(
        &self,
        execution_policy: &ExecutionPolicy,
        identity: PartitionBorderGlobalFaceIdentityInvariantStats,
        next_lineage: PartitionBorderGlobalNextLineageIntegrationStats,
    ) -> crate::Result<PartitionBorderGlobalCycleFaceLineageStats> {
        execution_policy.check_cancelled("partition_border_global_cycle_face_lineage")?;
        let edge_count = self.global_face_edge_map.len();
        execution_policy.check(
            "partition_border_global_cycle_face_lineage_edges",
            execution_policy.max_graph_edges,
            edge_count,
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global cycle face lineage has no detached topology candidate".to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != edge_count {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global cycle face lineage successor length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    edge_count
                ),
            });
        }
        execution_policy.check(
            "partition_border_global_cycle_face_lineage_cycles",
            execution_policy.max_graph_nodes,
            candidate.cycle_start_global_dir_edge_ids.len(),
        )?;
        execution_policy.check(
            "partition_border_global_cycle_face_lineage_plans",
            execution_policy.max_graph_nodes,
            self.global_face_id_plans.len(),
        )?;

        let mut stats = PartitionBorderGlobalCycleFaceLineageStats {
            edge_count,
            cycle_count: candidate.cycle_start_global_dir_edge_ids.len(),
            plan_count: self.global_face_id_plans.len(),
            identity_ready: identity.invariants_ready,
            next_lineage_ready: next_lineage.integration_ready,
            ..Default::default()
        };
        let mut edge_slot_by_observation = BTreeMap::<PartitionBorderObservationId, usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_cycle_face_lineage_edges",
                edge_index,
            )?;
            let observation_id = PartitionBorderObservationId {
                partition_id: edge.partition_id,
                local_dir_edge_id: edge.local_dir_edge_id,
                edge_key: edge.edge_key,
            };
            if edge_slot_by_observation
                .insert(observation_id, edge_index)
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global cycle face lineage duplicates observation {:?}",
                        observation_id
                    ),
                });
            }
        }

        let mut plan_by_face_id = BTreeMap::<usize, usize>::new();
        for (plan_index, plan) in self.global_face_id_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_cycle_face_lineage_plans",
                plan_index,
            )?;
            let Some(face_id) = plan.candidate_global_face_id else {
                continue;
            };
            if plan_by_face_id.insert(face_id, plan_index).is_some() {
                stats.duplicate_face_id_plan_count += 1;
            }
            let unique_face_refs = plan.face_refs.iter().copied().collect::<BTreeSet<_>>();
            stats.duplicate_plan_face_ref_count +=
                plan.face_refs.len().saturating_sub(unique_face_refs.len());
        }

        let mut used_plans = BTreeSet::new();
        for (cycle_index, &start) in candidate.cycle_start_global_dir_edge_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_cycle_face_lineage_cycles",
                cycle_index,
            )?;
            if start >= edge_count {
                stats.invalid_cycle_count += 1;
                continue;
            }
            let mut cycle_edges = BTreeSet::new();
            let mut current = start;
            let mut closed = true;
            loop {
                execution_policy.check_cancelled_every(
                    "partition_border_global_cycle_face_lineage_cycle_edges",
                    cycle_edges.len(),
                )?;
                if !cycle_edges.insert(current) {
                    if current != start {
                        stats.invalid_cycle_count += 1;
                        closed = false;
                    }
                    break;
                }
                let Some(successor) = candidate.next_global_dir_edge_ids[current] else {
                    stats.incomplete_cycle_count += 1;
                    closed = false;
                    break;
                };
                if successor >= edge_count {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global cycle face lineage successor {} exceeds {} edges",
                            successor, edge_count
                        ),
                    });
                }
                current = successor;
            }
            if !closed {
                continue;
            }
            stats.closed_cycle_count += 1;
            let Some(face_id) = self
                .global_face_id_by_cycle_start
                .get(cycle_index)
                .copied()
                .flatten()
            else {
                stats.missing_face_id_count += 1;
                continue;
            };
            let Some(&plan_index) = plan_by_face_id.get(&face_id) else {
                stats.cycle_plan_mismatch_count += 1;
                continue;
            };
            if !used_plans.insert(plan_index) {
                stats.cycle_plan_mismatch_count += 1;
                continue;
            }
            let plan = &self.global_face_id_plans[plan_index];
            let mut plan_edges = BTreeSet::new();
            for observation_id in &plan.boundary_observation_ids {
                let Some(&edge_index) = edge_slot_by_observation.get(observation_id) else {
                    stats.observation_lineage_mismatch_count += 1;
                    continue;
                };
                plan_edges.insert(edge_index);
            }
            if plan_edges != cycle_edges || !plan.closed {
                stats.cycle_plan_mismatch_count += 1;
            }

            let cycle_face_refs = cycle_edges
                .iter()
                .filter_map(|&edge_index| self.global_face_edge_map[edge_index].face_ref)
                .collect::<BTreeSet<_>>();
            let plan_face_refs = plan.face_refs.iter().copied().collect::<BTreeSet<_>>();
            if cycle_face_refs != plan_face_refs {
                stats.cycle_face_ref_mismatch_count += 1;
            }
            let cycle_unbounded_count = cycle_edges
                .iter()
                .filter(|&&edge_index| {
                    self.global_face_edge_map[edge_index].local_face_is_unbounded
                })
                .count();
            if cycle_unbounded_count != plan.local_unbounded_face_count {
                stats.unbounded_lineage_mismatch_count += 1;
            }
            stats.mapped_cycle_count += 1;
        }
        stats.unmapped_plan_count = self
            .global_face_id_plans
            .iter()
            .enumerate()
            .filter(|(plan_index, plan)| {
                plan.candidate_global_face_id.is_some() && !used_plans.contains(plan_index)
            })
            .count();
        stats.lineage_ready = stats.identity_ready
            && stats.next_lineage_ready
            && stats.closed_cycle_count == stats.cycle_count
            && stats.mapped_cycle_count == stats.cycle_count
            && stats.plan_count == stats.cycle_count
            && stats.incomplete_cycle_count == 0
            && stats.invalid_cycle_count == 0
            && stats.missing_face_id_count == 0
            && stats.duplicate_face_id_plan_count == 0
            && stats.unmapped_plan_count == 0
            && stats.cycle_plan_mismatch_count == 0
            && stats.cycle_face_ref_mismatch_count == 0
            && stats.duplicate_plan_face_ref_count == 0
            && stats.observation_lineage_mismatch_count == 0
            && stats.unbounded_lineage_mismatch_count == 0;
        Ok(stats)
    }

    /// Cross-checks cycle-to-face lineage against complete component coverage
    /// and the conservative exactly-one-unbounded application proof. This is
    /// a detached promotion boundary only; it does not mutate any topology or
    /// output state.
    pub(crate) fn validate_global_cycle_face_promotion_gate(
        &self,
        execution_policy: &ExecutionPolicy,
        cycle_face_lineage: PartitionBorderGlobalCycleFaceLineageStats,
        component_coverage: PartitionBorderGlobalComponentCoverageStats,
        unbounded_face_application: PartitionBorderGlobalUnboundedFaceApplicationStats,
    ) -> crate::Result<PartitionBorderGlobalCycleFacePromotionGateStats> {
        execution_policy.check_cancelled("partition_border_global_cycle_face_promotion_gate")?;
        let edge_count = self.global_face_edge_map.len();
        execution_policy.check(
            "partition_border_global_cycle_face_promotion_gate_edges",
            execution_policy.max_graph_edges,
            edge_count,
        )?;
        execution_policy.check(
            "partition_border_global_cycle_face_promotion_gate_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;

        let edge_count_mismatch_count = usize::from(cycle_face_lineage.edge_count != edge_count)
            + usize::from(component_coverage.edge_count != edge_count);
        let cycle_count_mismatch_count = usize::from(
            cycle_face_lineage.cycle_count != unbounded_face_application.candidate_cycle_count,
        );
        let plan_count_mismatch_count =
            usize::from(cycle_face_lineage.plan_count != cycle_face_lineage.cycle_count)
                + usize::from(
                    cycle_face_lineage.plan_count
                        != unbounded_face_application.candidate_cycle_count,
                );
        let face_count_mismatch_count =
            usize::from(component_coverage.face_count != unbounded_face_application.face_count);
        let unbounded_marker_mismatch_count =
            usize::from(unbounded_face_application.candidate_unbounded_face_id_count != 1)
                + usize::from(unbounded_face_application.mapped_unbounded_cycle_count != 1);
        let gate_ready = cycle_face_lineage.lineage_ready
            && component_coverage.coverage_ready
            && unbounded_face_application.application_ready
            && edge_count_mismatch_count == 0
            && cycle_count_mismatch_count == 0
            && plan_count_mismatch_count == 0
            && face_count_mismatch_count == 0
            && unbounded_marker_mismatch_count == 0;

        Ok(PartitionBorderGlobalCycleFacePromotionGateStats {
            edge_count,
            cycle_count: cycle_face_lineage.cycle_count,
            plan_count: cycle_face_lineage.plan_count,
            component_count: component_coverage.component_count,
            face_count: component_coverage.face_count,
            covered_face_edge_count: component_coverage.covered_face_edge_count,
            candidate_unbounded_face_id_count: unbounded_face_application
                .candidate_unbounded_face_id_count,
            mapped_unbounded_cycle_count: unbounded_face_application.mapped_unbounded_cycle_count,
            lineage_ready: cycle_face_lineage.lineage_ready,
            component_coverage_ready: component_coverage.coverage_ready,
            unbounded_face_application_ready: unbounded_face_application.application_ready,
            edge_count_mismatch_count,
            cycle_count_mismatch_count,
            plan_count_mismatch_count,
            face_count_mismatch_count,
            unbounded_marker_mismatch_count,
            gate_ready,
        })
    }

    /// Cross-checks every gate-ready detached face edge against its retained
    /// observation, source/Z/face payload, and reconciled endpoint nodes.
    /// Cycle plans are checked as well so no face can become ready with a
    /// missing boundary observation. This never mutates graph or output state.
    pub(crate) fn validate_global_face_payload_lineage(
        &self,
        execution_policy: &ExecutionPolicy,
        promotion_gate: PartitionBorderGlobalCycleFacePromotionGateStats,
    ) -> crate::Result<PartitionBorderGlobalFacePayloadLineageStats> {
        execution_policy.check_cancelled("partition_border_global_face_payload_lineage")?;
        let edge_count = self.global_face_edge_map.len();
        execution_policy.check(
            "partition_border_global_face_payload_lineage_edges",
            execution_policy.max_graph_edges,
            edge_count,
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face payload lineage has no detached topology candidate"
                    .to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != edge_count {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face payload lineage successor length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    edge_count
                ),
            });
        }
        let cycle_count = candidate.cycle_start_global_dir_edge_ids.len();
        execution_policy.check(
            "partition_border_global_face_payload_lineage_cycles",
            execution_policy.max_graph_nodes,
            cycle_count,
        )?;

        let mut stats = PartitionBorderGlobalFacePayloadLineageStats {
            edge_count,
            cycle_count,
            plan_count: self.global_face_id_plans.len(),
            ..Default::default()
        };
        let mut edge_index_by_observation = BTreeMap::<PartitionBorderObservationId, usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_payload_lineage_edges",
                edge_index,
            )?;
            let observation_id = PartitionBorderObservationId {
                partition_id: edge.partition_id,
                local_dir_edge_id: edge.local_dir_edge_id,
                edge_key: edge.edge_key,
            };
            if edge_index_by_observation
                .insert(observation_id, edge_index)
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face payload lineage duplicates observation {:?}",
                        observation_id
                    ),
                });
            }
            stats.checked_edge_count += 1;
            if edge.source_line_ids.is_empty() {
                stats.source_incomplete_edge_count += 1;
            }
            let local_lineage = self.local_face_graphs.iter().find_map(|graph| {
                if graph.partition_id == edge.partition_id
                    && graph.component_id == edge.component_id
                {
                    graph
                        .directed_edges
                        .iter()
                        .find(|local_edge| local_edge.local_dir_edge_id == edge.local_dir_edge_id)
                } else {
                    None
                }
            });
            if let Some(local_edge) = local_lineage {
                if edge.source_line_ids != local_edge.source_line_ids {
                    stats.source_lineage_mismatch_count += 1;
                }
                if edge.from_z_bits != local_edge.from_z_bits
                    || edge.to_z_bits != local_edge.to_z_bits
                {
                    stats.z_lineage_mismatch_count += 1;
                }
                if edge.from != local_edge.from
                    || edge.to != local_edge.to
                    || edge.edge_key != local_edge.edge_key
                    || edge.face_ref != local_edge.face_ref
                    || edge.local_face_is_unbounded != local_edge.local_face_is_unbounded
                {
                    stats.face_lineage_mismatch_count += 1;
                }
            } else if let Some(observation) = self.observations.get(&observation_id) {
                if edge.source_line_ids != observation.source_line_ids {
                    stats.source_lineage_mismatch_count += 1;
                }
                if edge.from_z_bits != observation.from_z_bits
                    || edge.to_z_bits != observation.to_z_bits
                {
                    stats.z_lineage_mismatch_count += 1;
                }
                if edge.from != observation.from
                    || edge.to != observation.to
                    || edge.edge_key != observation.edge_key
                    || edge.face_ref != observation.face_ref
                    || edge.local_face_is_unbounded != observation.local_face_is_unbounded
                {
                    stats.face_lineage_mismatch_count += 1;
                }
            } else {
                stats.missing_observation_count += 1;
            }

            for (node_id, key, z_bits) in [
                (edge.from_global_node_id, edge.from, edge.from_z_bits),
                (edge.to_global_node_id, edge.to, edge.to_z_bits),
            ] {
                let Some(node_id) = node_id else {
                    stats.node_lineage_mismatch_count += 1;
                    continue;
                };
                let Some(node) = self.global_face_nodes.get(node_id) else {
                    stats.node_lineage_mismatch_count += 1;
                    continue;
                };
                if node.global_node_id != node_id
                    || node.key != key
                    || !edge
                        .source_line_ids
                        .iter()
                        .all(|source_line_id| node.source_line_ids.contains(source_line_id))
                    || !node.z_bits.contains(&z_bits)
                    || edge
                        .face_ref
                        .is_some_and(|face_ref| !node.face_refs.contains(&face_ref))
                {
                    stats.node_lineage_mismatch_count += 1;
                }
            }
        }

        let mut plan_by_face_id = BTreeMap::<usize, usize>::new();
        for (plan_index, plan) in self.global_face_id_plans.iter().enumerate() {
            if let Some(face_id) = plan.candidate_global_face_id {
                if plan_by_face_id.insert(face_id, plan_index).is_some() {
                    stats.face_lineage_mismatch_count += 1;
                }
            }
        }
        for (cycle_index, &start) in candidate.cycle_start_global_dir_edge_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_payload_lineage_cycles",
                cycle_index,
            )?;
            if start >= edge_count {
                stats.face_lineage_mismatch_count += 1;
                continue;
            }
            let Some(face_id) = self
                .global_face_id_by_cycle_start
                .get(cycle_index)
                .copied()
                .flatten()
            else {
                stats.missing_face_id_count += 1;
                continue;
            };
            let Some(&plan_index) = plan_by_face_id.get(&face_id) else {
                stats.missing_plan_count += 1;
                continue;
            };
            let plan = &self.global_face_id_plans[plan_index];
            let mut plan_edges = BTreeSet::new();
            for observation_id in &plan.boundary_observation_ids {
                let Some(&edge_index) = edge_index_by_observation.get(observation_id) else {
                    stats.missing_observation_count += 1;
                    continue;
                };
                plan_edges.insert(edge_index);
            }
            let mut cycle_edges = BTreeSet::new();
            let mut current = start;
            let mut closed = true;
            loop {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_payload_lineage_cycle_edges",
                    cycle_edges.len(),
                )?;
                if !cycle_edges.insert(current) {
                    if current != start {
                        closed = false;
                    }
                    break;
                }
                let Some(successor) = candidate
                    .next_global_dir_edge_ids
                    .get(current)
                    .copied()
                    .flatten()
                else {
                    closed = false;
                    break;
                };
                if successor >= edge_count {
                    closed = false;
                    break;
                }
                current = successor;
            }
            if !closed || plan_edges != cycle_edges {
                stats.face_lineage_mismatch_count += 1;
            } else {
                stats.checked_cycle_count += 1;
            }
        }

        stats.lineage_ready = promotion_gate.gate_ready
            && stats.checked_edge_count == edge_count
            && stats.checked_cycle_count == cycle_count
            && stats.missing_face_id_count == 0
            && stats.missing_plan_count == 0
            && stats.missing_observation_count == 0
            && stats.source_incomplete_edge_count == 0
            && stats.source_lineage_mismatch_count == 0
            && stats.z_lineage_mismatch_count == 0
            && stats.face_lineage_mismatch_count == 0
            && stats.node_lineage_mismatch_count == 0;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_edge_map(&self) -> &[PartitionBorderGlobalFaceEdge] {
        &self.global_face_edge_map
    }

    /// Reconciles every endpoint in the active global face-edge map into a
    /// deterministic XY node slot. Edge, observation, face, provenance, and
    /// Z payloads are merged before any node or edge slot is committed.
    pub(crate) fn reconcile_global_face_nodes(
        &mut self,
        z_options: ZOptions,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceNodeReconciliationStats> {
        execution_policy.check_cancelled("partition_border_global_face_nodes")?;
        let expected_edge_count = self
            .local_face_graphs
            .iter()
            .try_fold(0usize, |count, graph| {
                count.checked_add(graph.directed_edges.len())
            })
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face node edge count overflow".to_string(),
            })?;
        if expected_edge_count != self.global_face_edge_map.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face node reconciliation requires the complete edge map: expected {}, found {}",
                    expected_edge_count,
                    self.global_face_edge_map.len()
                ),
            });
        }
        execution_policy.check(
            "partition_border_global_face_nodes_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        let endpoint_count = self
            .global_face_edge_map
            .len()
            .checked_mul(2)
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face node endpoint count overflow".to_string(),
            })?;

        let mut edge_index_by_local = BTreeMap::<(usize, usize, DirEdgeId), usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_nodes_edges", edge_index)?;
            if edge_index_by_local
                .insert(
                    (edge.partition_id, edge.component_id, edge.local_dir_edge_id),
                    edge_index,
                )
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face node edge identity ({}, {}, {}) is duplicated",
                        edge.partition_id, edge.component_id, edge.local_dir_edge_id
                    ),
                });
            }
        }

        let mut evidence = BTreeMap::<PartitionBorderNodeKey, GlobalFaceNodeEvidence>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_nodes_payload", edge_index)?;
            let edge_observation_id = PartitionBorderObservationId {
                partition_id: edge.partition_id,
                local_dir_edge_id: edge.local_dir_edge_id,
                edge_key: edge.edge_key,
            };
            let representative_line_id = edge.source_line_ids.first().copied();
            for (key, z_bits) in [(edge.from, edge.from_z_bits), (edge.to, edge.to_z_bits)] {
                let node = evidence.entry(key).or_default();
                node.source_line_ids
                    .extend(edge.source_line_ids.iter().copied());
                if let Some(representative_line_id) = representative_line_id {
                    node.representative_line_ids.insert(representative_line_id);
                }
                if let Some(face_ref) = edge.face_ref {
                    node.face_refs.insert(face_ref);
                }
                node.z_bits.insert(z_bits);
                node.z_candidates.push((
                    representative_line_id.unwrap_or(u32::MAX),
                    edge_observation_id,
                    z_bits,
                ));
                node.incident_global_dir_edge_ids
                    .insert(edge.global_dir_edge_id);
            }
        }

        let mut mapped_observation_count = 0usize;
        let mut unmapped_observation_count = 0usize;
        for (observation_index, observation) in self.observations.values().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_nodes_observations",
                observation_index,
            )?;
            let component_id = observation
                .face_ref
                .map_or(observation.component_id, |face_ref| face_ref.component_id);
            let local_key = (
                observation.partition_id,
                component_id,
                observation.local_dir_edge_id,
            );
            let Some(&global_dir_edge_id) = edge_index_by_local.get(&local_key) else {
                unmapped_observation_count += 1;
                continue;
            };
            let edge = &self.global_face_edge_map[global_dir_edge_id];
            if edge.edge_key != observation.edge_key
                || edge.from != observation.from
                || edge.to != observation.to
                || edge.from_z_bits != observation.from_z_bits
                || edge.to_z_bits != observation.to_z_bits
                || edge.source_line_ids != observation.source_line_ids
                || edge.face_ref != observation.face_ref
                || edge.local_face_is_unbounded != observation.local_face_is_unbounded
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face node observation {:?} disagrees with edge lineage",
                        observation.observation_id()
                    ),
                });
            }
            let observation_id = observation.observation_id();
            let representative_line_id = observation.representative_line_id;
            for (key, z_bits) in [
                (observation.from, observation.from_z_bits),
                (observation.to, observation.to_z_bits),
            ] {
                let node = evidence.entry(key).or_default();
                node.observation_ids.insert(observation_id);
                node.source_line_ids
                    .extend(observation.source_line_ids.iter().copied());
                if let Some(representative_line_id) = representative_line_id {
                    node.representative_line_ids.insert(representative_line_id);
                }
                if let Some(face_ref) = observation.face_ref {
                    node.face_refs.insert(face_ref);
                }
                node.z_bits.insert(z_bits);
                node.z_candidates.push((
                    representative_line_id.unwrap_or(u32::MAX),
                    observation_id,
                    z_bits,
                ));
                node.incident_global_dir_edge_ids.insert(global_dir_edge_id);
            }
            mapped_observation_count += 1;
        }

        execution_policy.check(
            "partition_border_global_face_nodes",
            execution_policy.max_graph_nodes,
            evidence.len(),
        )?;
        let mut global_nodes = Vec::with_capacity(evidence.len());
        let mut node_index_by_key = BTreeMap::<PartitionBorderNodeKey, usize>::new();
        let mut z_candidate_count = 0usize;
        let mut z_conflict_count = 0usize;
        for (
            global_node_id,
            (
                key,
                GlobalFaceNodeEvidence {
                    observation_ids,
                    source_line_ids,
                    representative_line_ids,
                    face_refs,
                    z_bits,
                    mut z_candidates,
                    incident_global_dir_edge_ids,
                },
            ),
        ) in evidence.into_iter().enumerate()
        {
            execution_policy
                .check_cancelled_every("partition_border_global_face_nodes", global_node_id)?;
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
                z_conflict_count += 1;
            }
            if z_conflict && matches!(z_options.policy, ZPolicy::ErrorOnConflict) {
                return Err(crate::PolygonizeError::ZConflict {
                    x: f64::from_bits(key.xy_bits()[0]),
                    y: f64::from_bits(key.xy_bits()[1]),
                    line_ids: source_line_ids.iter().copied().collect(),
                });
            }
            z_candidate_count += z_bits.len();
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
            node_index_by_key.insert(key, global_node_id);
            global_nodes.push(PartitionBorderGlobalFaceNode {
                global_node_id,
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
                incident_global_dir_edge_ids: incident_global_dir_edge_ids.into_iter().collect(),
            });
        }

        let mut mapped_edges = self.global_face_edge_map.clone();
        for (edge_index, edge) in mapped_edges.iter_mut().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_nodes_edges", edge_index)?;
            edge.from_global_node_id =
                Some(*node_index_by_key.get(&edge.from).ok_or_else(|| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face node edge {} has no source node slot",
                            edge.global_dir_edge_id
                        ),
                    }
                })?);
            edge.to_global_node_id = Some(*node_index_by_key.get(&edge.to).ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face node edge {} has no destination node slot",
                        edge.global_dir_edge_id
                    ),
                }
            })?);
        }

        let stats = PartitionBorderGlobalFaceNodeReconciliationStats {
            edge_count: mapped_edges.len(),
            node_count: global_nodes.len(),
            endpoint_count,
            mapped_observation_count,
            unmapped_observation_count,
            z_candidate_count,
            z_conflict_count,
            node_map_ready: unmapped_observation_count == 0,
        };
        self.global_face_edge_map = mapped_edges;
        self.global_face_nodes = global_nodes;
        self.global_components.clear();
        self.global_component_payloads.clear();
        self.global_face_plans.clear();
        self.global_face_transitions.clear();
        self.global_face_twin_transitions.clear();
        self.global_face_next_candidates.clear();
        self.global_face_identity_plans.clear();
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_nodes(&self) -> &[PartitionBorderGlobalFaceNode] {
        &self.global_face_nodes
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
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        Ok(stats)
    }

    pub(crate) fn reconciled_border_nodes(&self) -> &[PartitionBorderNodePayload] {
        &self.reconciled_nodes
    }

    /// Validates that every active global face-node slot is a consistent
    /// projection of the canonical border-node payload at the same XY key.
    /// Canonical-only nodes remain allowed because not every physical border
    /// observation is face-qualified. This method is evidence only and does
    /// not mutate nodes, edges, observations, topology, or output.
    pub(crate) fn validate_canonical_border_nodes(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderCanonicalNodeValidationStats> {
        execution_policy.check_cancelled("partition_border_canonical_node_validation")?;
        execution_policy.check(
            "partition_border_canonical_node_validation_nodes",
            execution_policy.max_graph_nodes,
            self.reconciled_nodes.len(),
        )?;
        execution_policy.check(
            "partition_border_canonical_node_validation_global_nodes",
            execution_policy.max_graph_nodes,
            self.global_face_nodes.len(),
        )?;
        execution_policy.check(
            "partition_border_canonical_node_validation_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;

        let canonical_by_key = self
            .reconciled_nodes
            .iter()
            .map(|node| (node.key, node))
            .collect::<BTreeMap<_, _>>();
        if canonical_by_key.len() != self.reconciled_nodes.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "canonical border-node reconciliation contains duplicate keys".to_string(),
            });
        }

        let mut stats = PartitionBorderCanonicalNodeValidationStats {
            canonical_node_count: self.reconciled_nodes.len(),
            global_node_count: self.global_face_nodes.len(),
            canonical_only_node_count: self
                .reconciled_nodes
                .iter()
                .filter(|node| {
                    !self
                        .global_face_nodes
                        .iter()
                        .any(|global| global.key == node.key)
                })
                .count(),
            ..Default::default()
        };
        let set_is_subset = |left: &[u32], right: &[u32]| {
            left.iter().all(|value| right.binary_search(value).is_ok())
        };
        let face_set_is_subset = |left: &[PartitionFaceRef], right: &[PartitionFaceRef]| {
            left.iter().all(|value| right.binary_search(value).is_ok())
        };
        let z_set_is_subset = |left: &[u64], right: &[u64]| {
            left.iter().all(|value| right.binary_search(value).is_ok())
        };

        for (global_index, global_node) in self.global_face_nodes.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_canonical_node_validation_nodes",
                global_index,
            )?;
            if global_node.global_node_id != global_index
                || !canonical_by_key.contains_key(&global_node.key)
            {
                stats.invalid_global_node_id_count += 1;
                continue;
            }
            let canonical = canonical_by_key[&global_node.key];
            stats.mapped_global_node_count += 1;
            if !set_is_subset(&global_node.source_line_ids, &canonical.source_line_ids) {
                stats.source_set_mismatch_count += 1;
            }
            if !set_is_subset(
                &global_node.representative_line_ids,
                &canonical.representative_line_ids,
            ) {
                stats.representative_set_mismatch_count += 1;
            }
            let mut canonical_face_refs = canonical.face_refs.clone();
            canonical_face_refs.extend(
                self.global_face_edge_map
                    .iter()
                    .filter(|edge| edge.from == global_node.key || edge.to == global_node.key)
                    .filter_map(|edge| edge.face_ref),
            );
            canonical_face_refs.sort_unstable();
            canonical_face_refs.dedup();
            if !face_set_is_subset(&global_node.face_refs, &canonical_face_refs) {
                stats.face_set_mismatch_count += 1;
            }
            if !z_set_is_subset(&global_node.z_bits, &canonical.z_bits) {
                stats.z_candidate_mismatch_count += 1;
            }
            if global_node.selected_z_bits != canonical.selected_z_bits {
                stats.selected_z_mismatch_count += 1;
            }
            if global_node.selected_z_policy != canonical.selected_z_policy
                || global_node.conflict_tolerance_bits != canonical.conflict_tolerance_bits
            {
                stats.z_policy_mismatch_count += 1;
            }
            if global_node.z_conflict != canonical.z_conflict {
                stats.z_conflict_mismatch_count += 1;
            }
        }

        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_canonical_node_validation_edges",
                edge_index,
            )?;
            for (key, node_id) in [
                (edge.from, edge.from_global_node_id),
                (edge.to, edge.to_global_node_id),
            ] {
                let valid = node_id
                    .and_then(|node_id| self.global_face_nodes.get(node_id))
                    .is_some_and(|node| node.key == key);
                if !valid {
                    stats.edge_endpoint_mismatch_count += 1;
                }
            }
        }

        stats.reconciliation_ready = stats.mapped_global_node_count == stats.global_node_count
            && stats.source_set_mismatch_count == 0
            && stats.representative_set_mismatch_count == 0
            && stats.face_set_mismatch_count == 0
            && stats.z_candidate_mismatch_count == 0
            && stats.selected_z_mismatch_count == 0
            && stats.z_policy_mismatch_count == 0
            && stats.z_conflict_mismatch_count == 0
            && stats.edge_endpoint_mismatch_count == 0
            && stats.invalid_global_node_id_count == 0;
        Ok(stats)
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
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        Ok(stats)
    }

    pub(crate) fn global_components(&self) -> &[PartitionBorderGlobalComponent] {
        &self.global_components
    }

    /// Validates that the deterministic global components cover every
    /// face-qualified edge in the detached candidate exactly once. Missing
    /// face lineage remains explicit evidence and prevents readiness; no
    /// component, face, or topology identity is rewritten.
    pub(crate) fn validate_global_component_coverage(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalComponentCoverageStats> {
        execution_policy.check_cancelled("partition_border_global_component_coverage")?;
        execution_policy.check(
            "partition_border_global_component_coverage_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        execution_policy.check(
            "partition_border_global_component_coverage_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global component coverage has no detached topology candidate".to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != self.global_face_edge_map.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global component coverage candidate length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    self.global_face_edge_map.len()
                ),
            });
        }

        let mut component_by_face = BTreeMap::<PartitionFaceRef, usize>::new();
        let mut duplicate_face_count = 0usize;
        let mut face_count = 0usize;
        for (component_index, component) in self.global_components.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_component_coverage_components",
                component_index,
            )?;
            if component.component_index != component_index {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global component coverage index mismatch: expected {}, got {}",
                        component_index, component.component_index
                    ),
                });
            }
            for face_ref in &component.face_refs {
                face_count += 1;
                if component_by_face
                    .insert(*face_ref, component_index)
                    .is_some()
                {
                    duplicate_face_count += 1;
                }
            }
        }

        let mut twin_edge_owner = BTreeMap::<PartitionBorderEdgeKey, usize>::new();
        let mut duplicate_twin_edge_count = 0usize;
        for (component_index, component) in self.global_components.iter().enumerate() {
            for edge_key in &component.twin_edge_keys {
                if twin_edge_owner.insert(*edge_key, component_index).is_some() {
                    duplicate_twin_edge_count += 1;
                }
            }
        }

        let mut face_edge_count = 0usize;
        let mut covered_face_edge_count = 0usize;
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_component_coverage_edges",
                edge_index,
            )?;
            let Some(face_ref) = edge.face_ref else {
                continue;
            };
            face_edge_count += 1;
            if component_by_face.contains_key(&face_ref) {
                covered_face_edge_count += 1;
            }
        }
        let uncovered_face_edge_count = face_edge_count - covered_face_edge_count;
        let coverage_ready = duplicate_face_count == 0
            && duplicate_twin_edge_count == 0
            && uncovered_face_edge_count == 0
            && face_count > 0;
        Ok(PartitionBorderGlobalComponentCoverageStats {
            component_count: self.global_components.len(),
            face_count,
            edge_count: self.global_face_edge_map.len(),
            face_edge_count,
            covered_face_edge_count,
            uncovered_face_edge_count,
            duplicate_face_count,
            duplicate_twin_edge_count,
            coverage_ready,
        })
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
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
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
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
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
        self.global_face_next_mutation_plans.clear();
        self.global_face_id_plans.clear();
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
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

    /// Retains the exact prospective global-next assignments from validated
    /// identity cycles. This is a mutation gate only: it does not write any
    /// local or global directed-edge links.
    #[cfg(test)]
    pub(crate) fn reconcile_global_face_next_mutation_plans(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceNextMutationPlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_next_mutation_plans")?;
        let walk = self.validate_global_face_walk_invariants(execution_policy)?;
        if self.global_face_identity_plans.is_empty() && !self.global_components.is_empty() {
            self.reconcile_global_face_next_candidates_with_walk(execution_policy, walk)?;
            self.reconcile_global_face_identity_plans_with_walk(execution_policy, walk)?;
        }
        self.reconcile_global_face_next_mutation_plans_with_walk(execution_policy, walk)
    }

    pub(crate) fn reconcile_global_face_next_mutation_plans_with_walk(
        &mut self,
        execution_policy: &ExecutionPolicy,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
    ) -> crate::Result<PartitionBorderGlobalFaceNextMutationPlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_next_mutation_plans")?;
        execution_policy.check(
            "partition_border_global_face_next_mutation_plans_faces",
            execution_policy.max_graph_nodes,
            self.global_face_transitions.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_next_mutation_plans_observations",
            execution_policy.max_graph_edges,
            walk.transition_count,
        )?;
        execution_policy.check(
            "partition_border_global_face_next_mutation_plans_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        if walk.face_count != self.global_face_transitions.len()
            || walk.mapped_twin_count != self.global_face_twin_transitions.len()
            || self
                .global_face_identity_plans
                .iter()
                .any(|plan| plan.component_index >= self.global_components.len())
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face next mutation plan evidence mismatch".to_string(),
            });
        }

        let mut plans = Vec::with_capacity(self.global_face_identity_plans.len());
        let mut seen_components = BTreeSet::new();
        let mut seen_observations = BTreeSet::new();
        let mut incomplete_components = BTreeSet::new();
        let mut successor_by_observation =
            BTreeMap::<PartitionBorderObservationId, PartitionBorderObservationId>::new();
        let mut boundary_observation_count = 0usize;
        let mut candidate_link_count = 0usize;
        for (plan_index, identity_plan) in self.global_face_identity_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_next_mutation_plans",
                plan_index,
            )?;
            seen_components.insert(identity_plan.component_index);
            let mut cycle_observations = BTreeSet::new();
            for observation_id in &identity_plan.boundary_observation_ids {
                if !cycle_observations.insert(*observation_id)
                    || !seen_observations.insert(*observation_id)
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face next mutation plan observation {:?} is duplicated",
                            observation_id
                        ),
                    });
                }
            }
            boundary_observation_count = boundary_observation_count
                .checked_add(identity_plan.boundary_observation_ids.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global face next mutation plan observation count overflow".to_string(),
                })?;
            if !identity_plan.closed {
                incomplete_components.insert(identity_plan.component_index);
                plans.push(PartitionBorderGlobalFaceNextMutationPlan {
                    component_index: identity_plan.component_index,
                    boundary_observation_ids: identity_plan.boundary_observation_ids.clone(),
                    successor_observation_ids: Vec::new(),
                    closed: false,
                });
                continue;
            }
            if identity_plan.boundary_observation_ids.is_empty() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next mutation plan component {} has invalid cycle lengths",
                        identity_plan.component_index
                    ),
                });
            }
            for (cycle_index, predecessor) in
                identity_plan.boundary_observation_ids.iter().enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_next_mutation_plan_links",
                    cycle_index,
                )?;
                let successor = identity_plan.boundary_observation_ids
                    [(cycle_index + 1) % identity_plan.boundary_observation_ids.len()];
                if !cycle_observations.contains(&successor) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face next mutation plan component {} disagrees with its identity cycle",
                            identity_plan.component_index
                        ),
                    });
                }
                if successor_by_observation
                    .insert(*predecessor, successor)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face next mutation plan predecessor {:?} is duplicated",
                            predecessor
                        ),
                    });
                }
            }
            candidate_link_count = candidate_link_count
                .checked_add(identity_plan.boundary_observation_ids.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global face next mutation plan link count overflow".to_string(),
                })?;
            plans.push(PartitionBorderGlobalFaceNextMutationPlan {
                component_index: identity_plan.component_index,
                boundary_observation_ids: identity_plan.boundary_observation_ids.clone(),
                successor_observation_ids: (0..identity_plan.boundary_observation_ids.len())
                    .map(|cycle_index| {
                        identity_plan.boundary_observation_ids
                            [(cycle_index + 1) % identity_plan.boundary_observation_ids.len()]
                    })
                    .collect(),
                closed: true,
            });
        }

        if seen_components.len() != self.global_components.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face next mutation plan omits components: planned={}, components={}",
                    seen_components.len(),
                    self.global_components.len()
                ),
            });
        }
        let ready_component_count = self
            .global_components
            .len()
            .saturating_sub(incomplete_components.len());
        let stats = PartitionBorderGlobalFaceNextMutationPlanStats {
            component_count: self.global_components.len(),
            boundary_observation_count,
            plan_count: plans.len(),
            candidate_link_count,
            ready_component_count,
            incomplete_component_count: incomplete_components.len(),
            mutation_ready: incomplete_components.is_empty(),
        };
        self.global_face_next_mutation_plans = plans;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_next_mutation_plans(
        &self,
    ) -> &[PartitionBorderGlobalFaceNextMutationPlan] {
        &self.global_face_next_mutation_plans
    }

    /// Assigns deterministic candidate global face IDs to validated closed
    /// boundary cycles. Incomplete cycles are retained with no ID, and this
    /// plan never writes IDs into local observations, global topology, or
    /// tiled output.
    #[cfg(test)]
    pub(crate) fn reconcile_global_face_id_plans(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceIdPlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_id_plans")?;
        let walk = self.validate_global_face_walk_invariants(execution_policy)?;
        if self.global_face_next_mutation_plans.is_empty() && !self.global_components.is_empty() {
            self.reconcile_global_face_next_candidates_with_walk(execution_policy, walk)?;
            self.reconcile_global_face_identity_plans_with_walk(execution_policy, walk)?;
            self.reconcile_global_face_next_mutation_plans_with_walk(execution_policy, walk)?;
        }
        self.reconcile_global_face_id_plans_with_walk(execution_policy, walk)
    }

    pub(crate) fn reconcile_global_face_id_plans_with_walk(
        &mut self,
        execution_policy: &ExecutionPolicy,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
    ) -> crate::Result<PartitionBorderGlobalFaceIdPlanStats> {
        execution_policy.check_cancelled("partition_border_global_face_id_plans")?;
        execution_policy.check(
            "partition_border_global_face_id_plans_faces",
            execution_policy.max_graph_nodes,
            self.global_face_identity_plans.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_id_plans_observations",
            execution_policy.max_graph_edges,
            walk.transition_count,
        )?;
        execution_policy.check(
            "partition_border_global_face_id_plans_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        if walk.face_count != self.global_face_transitions.len()
            || self.global_face_identity_plans.len() != self.global_face_next_mutation_plans.len()
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face ID plan evidence mismatch".to_string(),
            });
        }

        let mut transition_unbounded_by_face = BTreeMap::<PartitionFaceRef, usize>::new();
        for (transition_index, transition) in self.global_face_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_plans_transitions",
                transition_index,
            )?;
            if transition_unbounded_by_face
                .insert(
                    transition.face_ref,
                    usize::from(transition.local_face_is_unbounded),
                )
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID plan transition {:?} is duplicated",
                        transition.face_ref
                    ),
                });
            }
        }

        let mut plans = Vec::with_capacity(self.global_face_identity_plans.len());
        let mut seen_components = BTreeSet::new();
        let mut seen_observations = BTreeSet::new();
        let mut boundary_observation_count = 0usize;
        let mut incomplete_plan_count = 0usize;
        let mut unbounded_candidate_count = 0usize;
        for (plan_index, identity_plan) in self.global_face_identity_plans.iter().enumerate() {
            execution_policy
                .check_cancelled_every("partition_border_global_face_id_plans", plan_index)?;
            if identity_plan.component_index >= self.global_components.len() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID plan {} references missing component {}",
                        plan_index, identity_plan.component_index
                    ),
                });
            }
            seen_components.insert(identity_plan.component_index);
            let mutation_plan = &self.global_face_next_mutation_plans[plan_index];
            if mutation_plan.component_index != identity_plan.component_index
                || mutation_plan.boundary_observation_ids != identity_plan.boundary_observation_ids
                || mutation_plan.closed != identity_plan.closed
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID plan {} disagrees with its mutation plan",
                        plan_index
                    ),
                });
            }
            for observation_id in &identity_plan.boundary_observation_ids {
                if !seen_observations.insert(*observation_id) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face ID plan observation {:?} is duplicated",
                            observation_id
                        ),
                    });
                }
            }
            boundary_observation_count = boundary_observation_count
                .checked_add(identity_plan.boundary_observation_ids.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "global face ID plan observation count overflow".to_string(),
                })?;
            let local_unbounded_face_count = identity_plan
                .face_refs
                .iter()
                .map(|face_ref| transition_unbounded_by_face.get(face_ref).copied())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID plan {} references a face without a transition",
                        plan_index
                    ),
                })?
                .into_iter()
                .sum();
            if local_unbounded_face_count > 0 && identity_plan.closed {
                unbounded_candidate_count += 1;
            }
            if !identity_plan.closed {
                incomplete_plan_count += 1;
            }
            plans.push(PartitionBorderGlobalFaceIdPlan {
                candidate_global_face_id: None,
                component_index: identity_plan.component_index,
                boundary_observation_ids: identity_plan.boundary_observation_ids.clone(),
                face_refs: identity_plan.face_refs.clone(),
                local_unbounded_face_count,
                closed: identity_plan.closed,
            });
        }
        if seen_components.len() != self.global_components.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face ID plan omits components: planned={}, components={}",
                    seen_components.len(),
                    self.global_components.len()
                ),
            });
        }

        let mut closed_plan_indices = plans
            .iter()
            .enumerate()
            .filter_map(|(index, plan)| plan.closed.then_some(index))
            .collect::<Vec<_>>();
        closed_plan_indices.sort_unstable_by(|left, right| {
            plans[*left]
                .component_index
                .cmp(&plans[*right].component_index)
                .then_with(|| {
                    plans[*left]
                        .boundary_observation_ids
                        .cmp(&plans[*right].boundary_observation_ids)
                })
                .then_with(|| plans[*left].face_refs.cmp(&plans[*right].face_refs))
        });
        for (candidate_global_face_id, plan_index) in
            closed_plan_indices.iter().copied().enumerate()
        {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_plans_cycles",
                candidate_global_face_id,
            )?;
            plans[plan_index].candidate_global_face_id = Some(candidate_global_face_id);
        }

        let stats = PartitionBorderGlobalFaceIdPlanStats {
            component_count: self.global_components.len(),
            candidate_cycle_count: plans.len(),
            assigned_face_count: closed_plan_indices.len(),
            boundary_observation_count,
            unbounded_candidate_count,
            incomplete_plan_count,
            assignment_ready: incomplete_plan_count == 0,
        };
        self.global_face_id_plans = plans;
        self.global_face_next_application_plans.clear();
        self.global_topology_candidate = None;
        self.global_next_global_dir_edge_ids.clear();
        self.global_face_id_by_cycle_start.clear();
        self.global_face_id_by_global_dir_edge_id.clear();
        self.global_unbounded_face_id_by_cycle_start = None;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_id_plans(&self) -> &[PartitionBorderGlobalFaceIdPlan] {
        &self.global_face_id_plans
    }

    /// Validates that candidate global face IDs are a contiguous permutation of
    /// the detached candidate's closed cycles. Each retained boundary plan must
    /// map to exactly one candidate cycle by its observation identities; no
    /// topology or output state is changed.
    pub(crate) fn validate_global_face_id_application(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceIdApplicationStats> {
        execution_policy.check_cancelled("partition_border_global_face_id_application")?;
        execution_policy.check(
            "partition_border_global_face_id_application_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_id_application_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face ID application has no detached topology candidate".to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != self.global_face_edge_map.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face ID application candidate length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    self.global_face_edge_map.len()
                ),
            });
        }

        let mut edge_slot_by_observation = BTreeMap::<PartitionBorderObservationId, usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_application_edges",
                edge_index,
            )?;
            for observation in self.observations.values().filter(|observation| {
                observation.partition_id == edge.partition_id
                    && observation.local_dir_edge_id == edge.local_dir_edge_id
                    && observation.edge_key == edge.edge_key
                    && observation.from == edge.from
                    && observation.to == edge.to
                    && observation.from_z_bits == edge.from_z_bits
                    && observation.to_z_bits == edge.to_z_bits
                    && observation.source_line_ids == edge.source_line_ids
                    && observation.face_ref == edge.face_ref
                    && observation.local_face_is_unbounded == edge.local_face_is_unbounded
            }) {
                if edge_slot_by_observation
                    .insert(observation.observation_id(), edge_index)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face ID application observation {:?} is mapped more than once",
                            observation.observation_id()
                        ),
                    });
                }
            }
        }

        let mut candidate_cycles = Vec::<BTreeSet<usize>>::new();
        let mut cycle_starts = BTreeSet::new();
        for (start_index, &start) in candidate.cycle_start_global_dir_edge_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_application_cycles",
                start_index,
            )?;
            if start >= candidate.next_global_dir_edge_ids.len() || !cycle_starts.insert(start) {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID application candidate has invalid or duplicate cycle start {}",
                        start
                    ),
                });
            }
            let mut cycle = BTreeSet::new();
            let mut current = start;
            loop {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_id_application_cycle_edges",
                    cycle.len(),
                )?;
                if !cycle.insert(current) {
                    if current != start {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face ID application candidate cycle from {} repeats at {}",
                                start, current
                            ),
                        });
                    }
                    break;
                }
                let Some(successor) = candidate.next_global_dir_edge_ids[current] else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face ID application candidate cycle from {} is incomplete",
                            start
                        ),
                    });
                };
                if successor >= candidate.next_global_dir_edge_ids.len() {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face ID application candidate cycle from {} references {} outside {} slots",
                            start,
                            successor,
                            candidate.next_global_dir_edge_ids.len()
                        ),
                    });
                }
                current = successor;
            }
            candidate_cycles.push(cycle);
        }

        let mut assigned_ids = BTreeSet::new();
        let mut assigned_face_count = 0usize;
        let mut duplicate_face_id_count = 0usize;
        let mut mapped_cycle_count = 0usize;
        let mut unmapped_plan_count = 0usize;
        let mut used_cycles = BTreeSet::new();
        for (plan_index, plan) in self.global_face_id_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_application_plans",
                plan_index,
            )?;
            let Some(face_id) = plan.candidate_global_face_id else {
                unmapped_plan_count += 1;
                continue;
            };
            assigned_face_count += 1;
            if !assigned_ids.insert(face_id) {
                duplicate_face_id_count += 1;
            }
            let mut plan_edges = BTreeSet::new();
            for observation_id in &plan.boundary_observation_ids {
                let Some(&edge_index) = edge_slot_by_observation.get(observation_id) else {
                    unmapped_plan_count += 1;
                    plan_edges.clear();
                    break;
                };
                plan_edges.insert(edge_index);
            }
            if plan_edges.is_empty() {
                continue;
            }
            let Some((cycle_index, cycle)) =
                candidate_cycles
                    .iter()
                    .enumerate()
                    .find(|(cycle_index, cycle)| {
                        **cycle == plan_edges && !used_cycles.contains(cycle_index)
                    })
            else {
                unmapped_plan_count += 1;
                continue;
            };
            used_cycles.insert(cycle_index);
            if cycle == &plan_edges {
                mapped_cycle_count += 1;
            }
        }
        let expected_ids = (0..assigned_face_count).collect::<BTreeSet<_>>();
        let non_contiguous_face_id_count = expected_ids.difference(&assigned_ids).count();
        let application_ready = !self.global_components.is_empty()
            && self.global_face_id_plans.len() == candidate_cycles.len()
            && assigned_face_count == candidate_cycles.len()
            && mapped_cycle_count == candidate_cycles.len()
            && unmapped_plan_count == 0
            && duplicate_face_id_count == 0
            && non_contiguous_face_id_count == 0;
        Ok(PartitionBorderGlobalFaceIdApplicationStats {
            component_count: self.global_components.len(),
            candidate_cycle_count: self.global_face_id_plans.len(),
            assigned_face_count,
            candidate_cycle_start_count: candidate_cycles.len(),
            mapped_cycle_count,
            unmapped_plan_count,
            duplicate_face_id_count,
            non_contiguous_face_id_count,
            application_ready,
        })
    }

    /// Maps validated global-face mutation cycles into the captured global
    /// edge and node slots. This is the last evidence step before a future
    /// topology mutation: every candidate link is checked for unique
    /// predecessor/successor ownership, endpoint-node continuity, and
    /// cross-border twin reversal, but no `next` link is written.
    pub(crate) fn reconcile_global_face_next_application_plans(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceNextApplicationStats> {
        execution_policy.check_cancelled("partition_border_global_face_next_application")?;
        execution_policy.check(
            "partition_border_global_face_next_application_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_next_application_nodes",
            execution_policy.max_graph_nodes,
            self.global_face_nodes.len(),
        )?;
        execution_policy.check(
            "partition_border_global_face_next_application_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        if self.global_face_next_mutation_plans.len() != self.global_components.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face next application plan count mismatch: plans={}, components={}",
                    self.global_face_next_mutation_plans.len(),
                    self.global_components.len()
                ),
            });
        }

        let mut edge_slot_by_local = BTreeMap::<(usize, usize, DirEdgeId), usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_next_application_edges",
                edge_index,
            )?;
            if edge
                .from_global_node_id
                .is_none_or(|node| node >= self.global_face_nodes.len())
                || edge
                    .to_global_node_id
                    .is_none_or(|node| node >= self.global_face_nodes.len())
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next application edge {} has an invalid node slot",
                        edge.global_dir_edge_id
                    ),
                });
            }
            if edge_slot_by_local
                .insert(
                    (edge.partition_id, edge.component_id, edge.local_dir_edge_id),
                    edge_index,
                )
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next application edge identity ({}, {}, {}) is duplicated",
                        edge.partition_id, edge.component_id, edge.local_dir_edge_id
                    ),
                });
            }
        }

        let mut edge_slot_by_observation = BTreeMap::<PartitionBorderObservationId, usize>::new();
        let mut unmapped_observation_count = 0usize;
        for (observation_index, observation) in self.observations.values().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_next_application_observations",
                observation_index,
            )?;
            let component_id = observation
                .face_ref
                .map_or(observation.component_id, |face_ref| face_ref.component_id);
            let local_key = (
                observation.partition_id,
                component_id,
                observation.local_dir_edge_id,
            );
            let Some(&global_dir_edge_id) = edge_slot_by_local.get(&local_key) else {
                unmapped_observation_count += 1;
                continue;
            };
            let edge = &self.global_face_edge_map[global_dir_edge_id];
            if edge.edge_key != observation.edge_key
                || edge.from != observation.from
                || edge.to != observation.to
                || edge.from_z_bits != observation.from_z_bits
                || edge.to_z_bits != observation.to_z_bits
                || edge.source_line_ids != observation.source_line_ids
                || edge.face_ref != observation.face_ref
                || edge.local_face_is_unbounded != observation.local_face_is_unbounded
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next application observation {:?} disagrees with edge lineage",
                        observation.observation_id()
                    ),
                });
            }
            edge_slot_by_observation.insert(observation.observation_id(), global_dir_edge_id);
        }

        let mut mapped_twin_count = 0usize;
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            let Some(twin_index) = edge.cross_border_twin_global_dir_edge_id else {
                continue;
            };
            let twin = self.global_face_edge_map.get(twin_index).ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next application edge {} has invalid twin slot {}",
                        edge_index, twin_index
                    ),
                }
            })?;
            if twin.cross_border_twin_global_dir_edge_id != Some(edge_index)
                || edge.from_global_node_id != twin.to_global_node_id
                || edge.to_global_node_id != twin.from_global_node_id
                || edge.partition_id == twin.partition_id
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next application twin {} does not reverse edge {}",
                        twin_index, edge_index
                    ),
                });
            }
            if edge_index < twin_index {
                mapped_twin_count += 1;
            }
        }

        let mut plans = Vec::with_capacity(self.global_face_next_mutation_plans.len());
        let mut next_by_edge = BTreeMap::<usize, usize>::new();
        let mut predecessor_by_edge = BTreeMap::<usize, usize>::new();
        let mut candidate_link_count = 0usize;
        let mut incomplete_plan_count = 0usize;
        let mut node_discontinuity_count = 0usize;
        for (plan_index, mutation_plan) in self.global_face_next_mutation_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_next_application_plans",
                plan_index,
            )?;
            if mutation_plan.component_index >= self.global_components.len()
                || (mutation_plan.closed
                    && mutation_plan.boundary_observation_ids.len()
                        != mutation_plan.successor_observation_ids.len())
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face next application plan {} has invalid component or cycle lengths",
                        plan_index
                    ),
                });
            }
            let mut global_dir_edge_ids =
                Vec::with_capacity(mutation_plan.boundary_observation_ids.len());
            let mut unique_edges = BTreeSet::new();
            let mut complete = mutation_plan.closed;
            for (observation_index, observation_id) in mutation_plan
                .boundary_observation_ids
                .iter()
                .copied()
                .enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_next_application_observations",
                    observation_index,
                )?;
                let Some(&global_dir_edge_id) = edge_slot_by_observation.get(&observation_id)
                else {
                    complete = false;
                    continue;
                };
                if !unique_edges.insert(global_dir_edge_id) {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face next application plan {} repeats edge {}",
                            plan_index, global_dir_edge_id
                        ),
                    });
                }
                global_dir_edge_ids.push(global_dir_edge_id);
            }
            let mut successor_global_dir_edge_ids = Vec::new();
            let mut unique_successor_edges = BTreeSet::new();
            if complete {
                for (link_index, successor_observation_id) in mutation_plan
                    .successor_observation_ids
                    .iter()
                    .copied()
                    .enumerate()
                {
                    execution_policy.check_cancelled_every(
                        "partition_border_global_face_next_application_links",
                        link_index,
                    )?;
                    let Some(&successor_global_dir_edge_id) =
                        edge_slot_by_observation.get(&successor_observation_id)
                    else {
                        complete = false;
                        break;
                    };
                    if !unique_successor_edges.insert(successor_global_dir_edge_id) {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face next application plan {} repeats successor edge {}",
                                plan_index, successor_global_dir_edge_id
                            ),
                        });
                    }
                    successor_global_dir_edge_ids.push(successor_global_dir_edge_id);
                }
            }
            let mut plan_next_by_edge = BTreeMap::<usize, usize>::new();
            let mut plan_predecessor_by_edge = BTreeMap::<usize, usize>::new();
            let mut node_continuous = complete;
            if complete {
                for (link_index, (&global_dir_edge_id, &successor_global_dir_edge_id)) in
                    global_dir_edge_ids
                        .iter()
                        .zip(&successor_global_dir_edge_ids)
                        .enumerate()
                {
                    execution_policy.check_cancelled_every(
                        "partition_border_global_face_next_application_continuity",
                        link_index,
                    )?;
                    let edge = &self.global_face_edge_map[global_dir_edge_id];
                    let successor = &self.global_face_edge_map[successor_global_dir_edge_id];
                    if edge.to_global_node_id != successor.from_global_node_id {
                        node_continuous = false;
                        node_discontinuity_count += 1;
                        continue;
                    }
                    if plan_next_by_edge
                        .insert(global_dir_edge_id, successor_global_dir_edge_id)
                        .is_some()
                        || plan_predecessor_by_edge
                            .insert(successor_global_dir_edge_id, global_dir_edge_id)
                            .is_some()
                    {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face next application plan {} conflicts at edge {}",
                                plan_index, global_dir_edge_id
                            ),
                        });
                    }
                }
            }
            if !complete || !node_continuous {
                incomplete_plan_count += 1;
                successor_global_dir_edge_ids.clear();
            } else {
                for (&global_dir_edge_id, &successor_global_dir_edge_id) in &plan_next_by_edge {
                    if next_by_edge
                        .insert(global_dir_edge_id, successor_global_dir_edge_id)
                        .is_some()
                        || predecessor_by_edge
                            .insert(successor_global_dir_edge_id, global_dir_edge_id)
                            .is_some()
                    {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face next application plan {} conflicts at edge {}",
                                plan_index, global_dir_edge_id
                            ),
                        });
                    }
                }
                candidate_link_count = candidate_link_count
                    .checked_add(successor_global_dir_edge_ids.len())
                    .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                        reason: "global face next application link count overflow".to_string(),
                    })?;
            }
            plans.push(PartitionBorderGlobalFaceNextApplicationPlan {
                component_index: mutation_plan.component_index,
                global_dir_edge_ids,
                successor_global_dir_edge_ids,
                closed: complete && node_continuous,
                node_continuous,
            });
        }

        let stats = PartitionBorderGlobalFaceNextApplicationStats {
            component_count: self.global_components.len(),
            plan_count: plans.len(),
            candidate_link_count,
            mapped_edge_count: self.global_face_edge_map.len(),
            mapped_twin_count,
            unmapped_observation_count,
            incomplete_plan_count,
            node_discontinuity_count,
            application_ready: incomplete_plan_count == 0
                && unmapped_observation_count == 0
                && node_discontinuity_count == 0,
        };
        self.global_face_next_application_plans = plans;
        self.global_topology_candidate = None;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_next_application_plans(
        &self,
    ) -> &[PartitionBorderGlobalFaceNextApplicationPlan] {
        &self.global_face_next_application_plans
    }

    /// Builds a detached full directed-edge successor candidate from the
    /// captured local successors and closed global boundary overrides. The
    /// candidate is validated as a one-in/one-out cycle system, but no
    /// production `next` link is written.
    pub(crate) fn reconcile_global_topology_candidate(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalTopologyCandidateStats> {
        execution_policy.check_cancelled("partition_border_global_topology_candidate")?;
        execution_policy.check(
            "partition_border_global_topology_candidate_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        execution_policy.check(
            "partition_border_global_topology_candidate_nodes",
            execution_policy.max_graph_nodes,
            self.global_face_nodes.len(),
        )?;
        if self.global_face_next_application_plans.len() != self.global_components.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global topology candidate application plan count mismatch: plans={}, components={}",
                    self.global_face_next_application_plans.len(),
                    self.global_components.len()
                ),
            });
        }

        let edge_count = self.global_face_edge_map.len();
        let mut next_global_dir_edge_ids = Vec::with_capacity(edge_count);
        let mut local_successor_count = 0usize;
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_topology_candidate_edges",
                edge_index,
            )?;
            if edge.global_dir_edge_id != edge_index {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global topology candidate edge slot {} has declared ID {}",
                        edge_index, edge.global_dir_edge_id
                    ),
                });
            }
            if let Some(successor) = edge.local_face_successor_global_dir_edge_id {
                if successor >= edge_count {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global topology candidate edge {} has invalid local successor {}",
                            edge_index, successor
                        ),
                    });
                }
                local_successor_count += 1;
            }
            next_global_dir_edge_ids.push(edge.local_face_successor_global_dir_edge_id);
        }

        let mut global_override_edges = BTreeSet::new();
        let mut global_override_by_edge = BTreeMap::<usize, usize>::new();
        let mut incomplete_application_plan_count = 0usize;
        for (plan_index, plan) in self.global_face_next_application_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_topology_candidate_plans",
                plan_index,
            )?;
            if !plan.closed || !plan.node_continuous {
                incomplete_application_plan_count += 1;
                continue;
            }
            if plan.global_dir_edge_ids.len() != plan.successor_global_dir_edge_ids.len() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global topology candidate plan {} has mismatched successor lengths",
                        plan_index
                    ),
                });
            }
            let mut plan_links = Vec::with_capacity(plan.global_dir_edge_ids.len());
            let mut valid_plan = true;
            for (link_index, (&edge_index, &successor_index)) in plan
                .global_dir_edge_ids
                .iter()
                .zip(&plan.successor_global_dir_edge_ids)
                .enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_topology_candidate_links",
                    link_index,
                )?;
                if edge_index >= edge_count || successor_index >= edge_count {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global topology candidate plan {} references edge {} -> {} outside {} slots",
                            plan_index, edge_index, successor_index, edge_count
                        ),
                    });
                }
                let edge = &self.global_face_edge_map[edge_index];
                let successor = &self.global_face_edge_map[successor_index];
                if edge.to_global_node_id != successor.from_global_node_id {
                    valid_plan = false;
                    continue;
                }
                plan_links.push((edge_index, successor_index));
            }
            if !valid_plan {
                incomplete_application_plan_count += 1;
                continue;
            }
            for (edge_index, successor_index) in plan_links {
                if let Some(existing) = global_override_by_edge.get(&edge_index).copied() {
                    if existing != successor_index {
                        return Err(crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global topology candidate plan {} conflicts at edge {}: {} vs {}",
                                plan_index, edge_index, existing, successor_index
                            ),
                        });
                    }
                }
                global_override_by_edge.insert(edge_index, successor_index);
                next_global_dir_edge_ids[edge_index] = Some(successor_index);
                global_override_edges.insert(edge_index);
            }
        }

        let mut predecessor_by_edge = BTreeMap::<usize, usize>::new();
        let mut assigned_next_count = 0usize;
        let mut unassigned_next_count = 0usize;
        let mut predecessor_conflict_count = 0usize;
        let mut node_discontinuity_count = 0usize;
        for (edge_index, successor_index) in next_global_dir_edge_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_topology_candidate_validation",
                edge_index,
            )?;
            let Some(successor_index) = successor_index else {
                unassigned_next_count += 1;
                continue;
            };
            assigned_next_count += 1;
            let edge = &self.global_face_edge_map[edge_index];
            let successor = &self.global_face_edge_map[*successor_index];
            if edge.to_global_node_id != successor.from_global_node_id {
                node_discontinuity_count += 1;
            }
            if predecessor_by_edge
                .insert(*successor_index, edge_index)
                .is_some()
            {
                predecessor_conflict_count += 1;
            }
        }

        let mut state = vec![0u8; edge_count];
        let mut cycle_start_global_dir_edge_ids = Vec::new();
        let mut cycle_count = 0usize;
        let mut closed_cycle_edge_count = 0usize;
        for start in 0..edge_count {
            if state[start] != 0 {
                continue;
            }
            let mut path = Vec::new();
            let mut path_positions = BTreeMap::<usize, usize>::new();
            let mut current = start;
            loop {
                execution_policy.check_cancelled_every(
                    "partition_border_global_topology_candidate_cycles",
                    path.len(),
                )?;
                if let Some(&cycle_position) = path_positions.get(&current) {
                    cycle_count += 1;
                    cycle_start_global_dir_edge_ids.push(current);
                    closed_cycle_edge_count += path.len() - cycle_position;
                    break;
                }
                if state[current] != 0 {
                    break;
                }
                path_positions.insert(current, path.len());
                path.push(current);
                state[current] = 1;
                let Some(successor) = next_global_dir_edge_ids[current] else {
                    break;
                };
                current = successor;
            }
            for edge_index in path {
                state[edge_index] = 2;
            }
        }

        let candidate_ready = incomplete_application_plan_count == 0
            && unassigned_next_count == 0
            && predecessor_conflict_count == 0
            && node_discontinuity_count == 0
            && closed_cycle_edge_count == edge_count;
        let stats = PartitionBorderGlobalTopologyCandidateStats {
            edge_count,
            local_successor_count,
            global_override_count: global_override_edges.len(),
            assigned_next_count,
            unassigned_next_count,
            cycle_count,
            closed_cycle_edge_count,
            predecessor_conflict_count,
            node_discontinuity_count,
            incomplete_application_plan_count,
            candidate_ready,
        };
        self.global_next_global_dir_edge_ids.clear();
        self.global_face_id_by_cycle_start.clear();
        self.global_face_id_by_global_dir_edge_id.clear();
        self.global_unbounded_face_id_by_cycle_start = None;
        self.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids,
            cycle_start_global_dir_edge_ids,
        });
        Ok(stats)
    }

    /// Commits a validated candidate into a detached global successor buffer.
    /// This is the first atomic mutation step, but it intentionally does not
    /// rewrite local half-edge links, local face IDs, or tiled output.
    pub(crate) fn apply_global_topology_candidate_with_gate(
        &mut self,
        execution_policy: &ExecutionPolicy,
        mutation_gate: PartitionBorderGlobalTopologyMutationGateStats,
        candidate: PartitionBorderGlobalTopologyCandidateStats,
    ) -> crate::Result<PartitionBorderGlobalTopologyMutationStats> {
        execution_policy.check_cancelled("partition_border_global_topology_mutation")?;
        execution_policy.check(
            "partition_border_global_topology_mutation_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        let Some(detached_candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global topology mutation has no detached candidate".to_string(),
            });
        };
        if detached_candidate.next_global_dir_edge_ids.len() != self.global_face_edge_map.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global topology mutation candidate length mismatch".to_string(),
            });
        }
        let mutation_ready = mutation_gate.gate_ready && candidate.candidate_ready;
        if !mutation_ready {
            return Ok(PartitionBorderGlobalTopologyMutationStats {
                edge_count: self.global_face_edge_map.len(),
                applied_next_count: 0,
                mutation_ready,
                applied: false,
            });
        }
        let next_global_dir_edge_ids = detached_candidate.next_global_dir_edge_ids.clone();
        let applied_next_count = next_global_dir_edge_ids.iter().flatten().count();
        self.global_next_global_dir_edge_ids = next_global_dir_edge_ids;
        Ok(PartitionBorderGlobalTopologyMutationStats {
            edge_count: self.global_face_edge_map.len(),
            applied_next_count,
            mutation_ready: true,
            applied: true,
        })
    }

    /// Commits deterministic candidate face IDs to detached cycle starts only
    /// after successor mutation, ID application, and unique-unbounded evidence
    /// are all ready. No local face ID or output payload is written.
    pub(crate) fn apply_global_face_ids_with_evidence(
        &mut self,
        execution_policy: &ExecutionPolicy,
        topology_mutation: PartitionBorderGlobalTopologyMutationStats,
        face_id_application: PartitionBorderGlobalFaceIdApplicationStats,
        unbounded_face_application: PartitionBorderGlobalUnboundedFaceApplicationStats,
    ) -> crate::Result<PartitionBorderGlobalFaceIdMutationStats> {
        execution_policy.check_cancelled("partition_border_global_face_id_mutation")?;
        let candidate_cycle_count = self
            .global_topology_candidate
            .as_ref()
            .map_or(self.global_face_id_plans.len(), |candidate| {
                candidate.cycle_start_global_dir_edge_ids.len()
            });
        execution_policy.check(
            "partition_border_global_face_id_mutation_cycles",
            execution_policy.max_graph_nodes,
            candidate_cycle_count,
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face ID mutation has no detached topology candidate".to_string(),
            });
        };
        let candidate_next = candidate.next_global_dir_edge_ids.clone();
        let cycle_starts = candidate.cycle_start_global_dir_edge_ids.clone();
        let candidate_cycle_count = cycle_starts.len();
        let mutation_ready = topology_mutation.applied
            && topology_mutation.mutation_ready
            && face_id_application.application_ready
            && unbounded_face_application.application_ready;
        if !mutation_ready {
            return Ok(PartitionBorderGlobalFaceIdMutationStats {
                candidate_cycle_count,
                mutation_ready: false,
                ..Default::default()
            });
        }

        let mut edge_slot_by_observation = BTreeMap::<PartitionBorderObservationId, usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_mutation_edges",
                edge_index,
            )?;
            for observation in self.observations.values().filter(|observation| {
                observation.partition_id == edge.partition_id
                    && observation.local_dir_edge_id == edge.local_dir_edge_id
                    && observation.edge_key == edge.edge_key
                    && observation.from == edge.from
                    && observation.to == edge.to
                    && observation.from_z_bits == edge.from_z_bits
                    && observation.to_z_bits == edge.to_z_bits
                    && observation.source_line_ids == edge.source_line_ids
                    && observation.face_ref == edge.face_ref
                    && observation.local_face_is_unbounded == edge.local_face_is_unbounded
            }) {
                if edge_slot_by_observation
                    .insert(observation.observation_id(), edge_index)
                    .is_some()
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global face ID mutation observation {:?} is mapped more than once",
                            observation.observation_id()
                        ),
                    });
                }
            }
        }

        let mut cycles = Vec::<(usize, BTreeSet<usize>)>::with_capacity(candidate_cycle_count);
        for (cycle_index, start) in cycle_starts.iter().copied().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_mutation_cycles",
                cycle_index,
            )?;
            if start >= candidate_next.len() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID mutation cycle start {} exceeds {} edges",
                        start,
                        candidate_next.len()
                    ),
                });
            }
            let mut cycle = BTreeSet::new();
            let mut current = start;
            loop {
                if !cycle.insert(current) {
                    if current == start {
                        break;
                    }
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: "global face ID mutation cycle is not closed at its start"
                            .to_string(),
                    });
                }
                if cycle.len() > candidate_next.len() {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: "global face ID mutation cycle exceeds edge count".to_string(),
                    });
                }
                let Some(successor) = candidate_next[current] else {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: "global face ID mutation cycle has no successor".to_string(),
                    });
                };
                if successor >= candidate_next.len() {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: "global face ID mutation successor exceeds edge count".to_string(),
                    });
                }
                current = successor;
            }
            cycles.push((start, cycle));
        }

        if self.global_face_id_plans.len() != cycles.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face ID mutation plan/cycle mismatch: plans={}, cycles={}",
                    self.global_face_id_plans.len(),
                    cycles.len()
                ),
            });
        }
        let mut ids_by_cycle_start = vec![None; cycles.len()];
        let mut used_cycles = BTreeSet::new();
        let mut used_ids = BTreeSet::new();
        let mut unbounded_face_id_count = 0usize;
        for (plan_index, plan) in self.global_face_id_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_id_mutation_plans",
                plan_index,
            )?;
            let face_id = plan.candidate_global_face_id.ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID mutation plan {} has no candidate ID",
                        plan_index
                    ),
                }
            })?;
            if !used_ids.insert(face_id) {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID mutation duplicates candidate ID {}",
                        face_id
                    ),
                });
            }
            let plan_edges = plan
                .boundary_observation_ids
                .iter()
                .map(|observation_id| {
                    edge_slot_by_observation
                        .get(observation_id)
                        .copied()
                        .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                            reason: format!(
                                "global face ID mutation cannot map observation {:?}",
                                observation_id
                            ),
                        })
                })
                .collect::<crate::Result<BTreeSet<_>>>()?;
            let (cycle_index, (cycle_start, cycle)) = cycles
                .iter()
                .enumerate()
                .find(|(cycle_index, (_, cycle))| {
                    !used_cycles.contains(cycle_index) && *cycle == plan_edges
                })
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global face ID mutation plan {} does not map to one cycle",
                        plan_index
                    ),
                })?;
            used_cycles.insert(cycle_index);
            ids_by_cycle_start[cycle_index] = Some(face_id);
            if plan.local_unbounded_face_count > 0 {
                unbounded_face_id_count += 1;
            }
            debug_assert!(cycle.contains(cycle_start));
        }
        if used_ids != (0..used_ids.len()).collect::<BTreeSet<_>>() || unbounded_face_id_count != 1
        {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face ID mutation IDs are not contiguous or uniquely unbounded"
                    .to_string(),
            });
        }
        let applied_face_id_count = ids_by_cycle_start.iter().flatten().count();
        self.global_face_id_by_cycle_start = ids_by_cycle_start;
        Ok(PartitionBorderGlobalFaceIdMutationStats {
            candidate_cycle_count,
            applied_face_id_count,
            unbounded_face_id_count,
            mutation_ready: true,
            applied: true,
        })
    }

    /// Promotes the uniquely proven unbounded face onto detached identity
    /// state after detached successors and deterministic face IDs are both
    /// committed. This never changes local face IDs, local links, or output.
    pub(crate) fn apply_global_unbounded_face_with_evidence(
        &mut self,
        execution_policy: &ExecutionPolicy,
        topology_mutation: PartitionBorderGlobalTopologyMutationStats,
        face_id_mutation: PartitionBorderGlobalFaceIdMutationStats,
        unbounded_face_application: PartitionBorderGlobalUnboundedFaceApplicationStats,
    ) -> crate::Result<PartitionBorderGlobalUnboundedFaceMutationStats> {
        execution_policy.check_cancelled("partition_border_global_unbounded_face_mutation")?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global unbounded face mutation has no detached topology candidate"
                    .to_string(),
            });
        };
        let candidate_cycle_count = candidate.cycle_start_global_dir_edge_ids.len();
        execution_policy.check(
            "partition_border_global_unbounded_face_mutation_cycles",
            execution_policy.max_graph_nodes,
            candidate_cycle_count,
        )?;
        let candidate_unbounded_face_id_count =
            unbounded_face_application.candidate_unbounded_face_id_count;
        let mutation_ready = topology_mutation.applied
            && topology_mutation.mutation_ready
            && face_id_mutation.applied
            && face_id_mutation.mutation_ready
            && unbounded_face_application.application_ready
            && candidate_cycle_count == face_id_mutation.candidate_cycle_count
            && face_id_mutation.applied_face_id_count == candidate_cycle_count
            && candidate_unbounded_face_id_count == 1;
        if !mutation_ready {
            return Ok(PartitionBorderGlobalUnboundedFaceMutationStats {
                candidate_cycle_count,
                candidate_unbounded_face_id_count,
                ..Default::default()
            });
        }

        let unbounded_plans = self
            .global_face_id_plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| plan.local_unbounded_face_count > 0)
            .collect::<Vec<_>>();
        if unbounded_plans.len() != 1 {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global unbounded face mutation expected one marked plan, found {}",
                    unbounded_plans.len()
                ),
            });
        }
        let (_, unbounded_plan) = unbounded_plans[0];
        let unbounded_face_id = unbounded_plan.candidate_global_face_id.ok_or_else(|| {
            crate::PolygonizeError::InternalInvariantViolation {
                reason: "global unbounded face mutation plan has no candidate face ID".to_string(),
            }
        })?;
        let cycle_index = self
            .global_face_id_by_cycle_start
            .iter()
            .position(|face_id| *face_id == Some(unbounded_face_id))
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global unbounded face mutation cannot map candidate face ID {}",
                    unbounded_face_id
                ),
            })?;
        let cycle_start = candidate
            .cycle_start_global_dir_edge_ids
            .get(cycle_index)
            .copied()
            .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global unbounded face mutation cycle index {} exceeds candidate starts",
                    cycle_index
                ),
            })?;
        self.global_unbounded_face_id_by_cycle_start = Some((unbounded_face_id, cycle_start));
        Ok(PartitionBorderGlobalUnboundedFaceMutationStats {
            candidate_cycle_count,
            candidate_unbounded_face_id_count,
            applied_unbounded_face_id: Some(unbounded_face_id),
            applied_cycle_start_global_dir_edge_id: Some(cycle_start),
            mutation_ready: true,
            applied: true,
        })
    }

    /// Materializes a detached per-edge face-ID map from the committed global
    /// successor cycles. Every edge must belong to exactly one closed cycle,
    /// every cycle must have one committed ID, and the committed unbounded
    /// identity must point into that map. No local graph or output state is
    /// modified.
    pub(crate) fn materialize_global_face_identity(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalFaceIdentityMaterializationStats> {
        execution_policy
            .check_cancelled("partition_border_global_face_identity_materialization")?;
        let edge_count = self.global_face_edge_map.len();
        execution_policy.check(
            "partition_border_global_face_identity_materialization_edges",
            execution_policy.max_graph_edges,
            edge_count,
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face identity materialization has no detached topology candidate"
                    .to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != edge_count {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face identity materialization successor length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    edge_count
                ),
            });
        }
        let cycle_count = candidate.cycle_start_global_dir_edge_ids.len();
        execution_policy.check(
            "partition_border_global_face_identity_materialization_cycles",
            execution_policy.max_graph_nodes,
            cycle_count,
        )?;
        let unbounded_cycle = self.global_unbounded_face_id_by_cycle_start;
        let mut edge_face_ids = vec![None; edge_count];
        let mut stats = PartitionBorderGlobalFaceIdentityMaterializationStats {
            edge_count,
            cycle_count,
            ..Default::default()
        };
        let mut starts = BTreeSet::new();
        for (cycle_index, &start) in candidate.cycle_start_global_dir_edge_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_materialization_cycles",
                cycle_index,
            )?;
            if start >= edge_count || !starts.insert(start) {
                stats.invalid_cycle_count += 1;
                continue;
            }
            let Some(face_id) = self
                .global_face_id_by_cycle_start
                .get(cycle_index)
                .copied()
                .flatten()
            else {
                stats.missing_face_id_count += 1;
                continue;
            };
            let is_unbounded_cycle =
                unbounded_cycle.is_some_and(|(unbounded_id, unbounded_start)| {
                    unbounded_id == face_id && unbounded_start == start
                });
            let mut visited = BTreeSet::new();
            let mut current = start;
            let mut closed = true;
            loop {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_identity_materialization_edges",
                    visited.len(),
                )?;
                if !visited.insert(current) {
                    if current != start {
                        closed = false;
                    }
                    break;
                }
                if current >= edge_count {
                    closed = false;
                    break;
                }
                if edge_face_ids[current].is_some() {
                    stats.duplicate_edge_count += 1;
                    closed = false;
                    break;
                }
                edge_face_ids[current] = Some(face_id);
                stats.assigned_edge_count += 1;
                if is_unbounded_cycle {
                    stats.unbounded_edge_count += 1;
                }
                let Some(successor) = candidate.next_global_dir_edge_ids[current] else {
                    closed = false;
                    break;
                };
                if successor >= edge_count {
                    closed = false;
                    break;
                }
                current = successor;
            }
            if !closed {
                stats.invalid_cycle_count += 1;
            }
        }
        stats.materialization_ready = stats.assigned_edge_count == edge_count
            && stats.cycle_count == self.global_face_id_by_cycle_start.len()
            && stats.missing_face_id_count == 0
            && stats.duplicate_edge_count == 0
            && stats.invalid_cycle_count == 0
            && unbounded_cycle.is_some()
            && stats.unbounded_edge_count > 0;
        if stats.materialization_ready {
            self.global_face_id_by_global_dir_edge_id = edge_face_ids;
        }
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_face_id_by_global_dir_edge_id(&self) -> &[Option<usize>] {
        &self.global_face_id_by_global_dir_edge_id
    }

    /// Validates the detached per-edge face identity against all retained
    /// global evidence. This is a proof boundary only: it does not promote
    /// `next`, face IDs, unbounded identity, or any payload into local/output
    /// topology.
    pub(crate) fn validate_global_face_identity_invariants(
        &self,
        execution_policy: &ExecutionPolicy,
        materialization: PartitionBorderGlobalFaceIdentityMaterializationStats,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
        euler: PartitionBorderGlobalFaceEulerWitnessStats,
    ) -> crate::Result<PartitionBorderGlobalFaceIdentityInvariantStats> {
        execution_policy.check_cancelled("partition_border_global_face_identity_invariants")?;
        execution_policy.check(
            "partition_border_global_face_identity_invariants_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global face identity invariants have no detached topology candidate"
                    .to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != self.global_face_edge_map.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global face identity invariants successor length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    self.global_face_edge_map.len()
                ),
            });
        }
        execution_policy.check(
            "partition_border_global_face_identity_invariants_cycles",
            execution_policy.max_graph_nodes,
            candidate.cycle_start_global_dir_edge_ids.len(),
        )?;

        let edge_count = self.global_face_edge_map.len();
        let cycle_count = candidate.cycle_start_global_dir_edge_ids.len();
        let face_ids = &self.global_face_id_by_global_dir_edge_id;
        let mut stats = PartitionBorderGlobalFaceIdentityInvariantStats {
            edge_count,
            cycle_count,
            ..Default::default()
        };
        let mut face_id_set = BTreeSet::new();
        for (edge_index, face_id) in face_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_invariants_edges",
                edge_index,
            )?;
            let Some(face_id) = face_id else {
                stats.missing_face_id_count += 1;
                continue;
            };
            stats.mapped_face_id_edge_count += 1;
            face_id_set.insert(*face_id);
        }
        if face_ids.len() != edge_count {
            stats.missing_face_id_count = stats
                .missing_face_id_count
                .saturating_add(edge_count.saturating_sub(face_ids.len()));
        }
        stats.face_id_set_count = face_id_set.len();

        for (cycle_index, &start) in candidate.cycle_start_global_dir_edge_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_invariants_cycles",
                cycle_index,
            )?;
            let expected_face_id = self
                .global_face_id_by_cycle_start
                .get(cycle_index)
                .copied()
                .flatten();
            let Some(expected_face_id) = expected_face_id else {
                stats.missing_face_id_count += 1;
                continue;
            };
            if start >= edge_count {
                stats.cycle_face_mismatch_count += 1;
                continue;
            }
            let mut visited = BTreeSet::new();
            let mut current = start;
            loop {
                execution_policy.check_cancelled_every(
                    "partition_border_global_face_identity_invariants_cycle_edges",
                    visited.len(),
                )?;
                if !visited.insert(current) {
                    if current != start {
                        stats.cycle_face_mismatch_count += 1;
                    }
                    break;
                }
                if current >= edge_count
                    || face_ids.get(current).copied().flatten() != Some(expected_face_id)
                {
                    stats.cycle_face_mismatch_count += 1;
                    break;
                }
                let Some(successor) = candidate.next_global_dir_edge_ids[current] else {
                    stats.cycle_face_mismatch_count += 1;
                    break;
                };
                if successor >= edge_count {
                    stats.cycle_face_mismatch_count += 1;
                    break;
                }
                current = successor;
            }
        }

        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_face_identity_invariants_successors",
                edge_index,
            )?;
            if edge.source_line_ids.is_empty() {
                stats.source_incomplete_edge_count += 1;
            }
            let Some(successor_index) = candidate.next_global_dir_edge_ids[edge_index] else {
                stats.successor_discontinuity_count += 1;
                continue;
            };
            let Some(successor) = self.global_face_edge_map.get(successor_index) else {
                stats.successor_discontinuity_count += 1;
                continue;
            };
            if edge.to_global_node_id != successor.from_global_node_id
                || face_ids.get(edge_index).copied().flatten()
                    != face_ids.get(successor_index).copied().flatten()
            {
                stats.successor_discontinuity_count += 1;
            }
        }

        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            let Some(twin_index) = edge.cross_border_twin_global_dir_edge_id else {
                continue;
            };
            if edge_index > twin_index {
                continue;
            }
            stats.twin_count += 1;
            let reciprocal = self
                .global_face_edge_map
                .get(twin_index)
                .is_some_and(|twin| twin.cross_border_twin_global_dir_edge_id == Some(edge_index));
            if !reciprocal
                || face_ids.get(edge_index).copied().flatten().is_none()
                || face_ids.get(twin_index).copied().flatten().is_none()
            {
                stats.twin_mapping_mismatch_count += 1;
            }
        }

        stats.face_walk_ready = walk.face_count > 0
            && walk.closed_face_count == walk.face_count
            && walk.unmapped_twin_count == 0
            && walk.mapped_twin_count == walk.applied_twin_count
            && walk.source_complete_twin_count == walk.applied_twin_count
            && walk.unbounded_face_count == 1
            && walk.unbounded_component_count == 1;
        stats.euler_evidence_ready = euler.boundary_euler_consistent;
        stats.invariants_ready = materialization.materialization_ready
            && stats.mapped_face_id_edge_count == edge_count
            && stats.face_id_set_count == cycle_count
            && stats.missing_face_id_count == 0
            && stats.cycle_face_mismatch_count == 0
            && stats.successor_discontinuity_count == 0
            && stats.source_incomplete_edge_count == 0
            && stats.twin_mapping_mismatch_count == 0
            && stats.face_walk_ready
            && stats.euler_evidence_ready;
        Ok(stats)
    }

    /// Validates that the detached successor permutation is the exact
    /// integration of local face successors and retained cross-border
    /// boundary overrides. The check also maps every retained face-qualified
    /// twin back to its reciprocal global edge slots and verifies that the
    /// committed detached successor buffer agrees with the candidate. This is
    /// evidence only; local topology, public face IDs, and tiled output remain
    /// untouched.
    pub(crate) fn validate_global_next_lineage_integration(
        &self,
        execution_policy: &ExecutionPolicy,
        identity: PartitionBorderGlobalFaceIdentityInvariantStats,
    ) -> crate::Result<PartitionBorderGlobalNextLineageIntegrationStats> {
        execution_policy.check_cancelled("partition_border_global_next_lineage_integration")?;
        let edge_count = self.global_face_edge_map.len();
        execution_policy.check(
            "partition_border_global_next_lineage_integration_edges",
            execution_policy.max_graph_edges,
            edge_count,
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global next lineage integration has no detached topology candidate"
                    .to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != edge_count {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global next lineage integration successor length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    edge_count
                ),
            });
        }
        execution_policy.check(
            "partition_border_global_next_lineage_integration_cycles",
            execution_policy.max_graph_nodes,
            candidate.cycle_start_global_dir_edge_ids.len(),
        )?;

        let mut stats = PartitionBorderGlobalNextLineageIntegrationStats {
            edge_count,
            cycle_count: candidate.cycle_start_global_dir_edge_ids.len(),
            identity_ready: identity.invariants_ready,
            ..Default::default()
        };
        let mut expected_next = self
            .global_face_edge_map
            .iter()
            .map(|edge| {
                if edge.local_face_successor_global_dir_edge_id.is_some() {
                    stats.local_successor_count += 1;
                }
                edge.local_face_successor_global_dir_edge_id
            })
            .collect::<Vec<_>>();
        let mut override_edges = BTreeSet::new();
        let mut override_by_edge = BTreeMap::<usize, usize>::new();
        let mut application_links = BTreeSet::<(usize, usize)>::new();
        for (plan_index, plan) in self.global_face_next_application_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_next_lineage_integration_plans",
                plan_index,
            )?;
            if !plan.closed || !plan.node_continuous {
                continue;
            }
            if plan.global_dir_edge_ids.len() != plan.successor_global_dir_edge_ids.len() {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global next lineage integration plan {} has mismatched successor lengths",
                        plan_index
                    ),
                });
            }
            for (link_index, (&edge_index, &successor_index)) in plan
                .global_dir_edge_ids
                .iter()
                .zip(&plan.successor_global_dir_edge_ids)
                .enumerate()
            {
                execution_policy.check_cancelled_every(
                    "partition_border_global_next_lineage_integration_links",
                    link_index,
                )?;
                if edge_index >= edge_count || successor_index >= edge_count {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global next lineage integration plan {} references edge {} -> {} outside {} slots",
                            plan_index, edge_index, successor_index, edge_count
                        ),
                    });
                }
                if self.global_face_edge_map[edge_index].to_global_node_id
                    != self.global_face_edge_map[successor_index].from_global_node_id
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global next lineage integration plan {} is not node-continuous at edge {}",
                            plan_index, edge_index
                        ),
                    });
                }
                if override_by_edge
                    .insert(edge_index, successor_index)
                    .is_some_and(|existing| existing != successor_index)
                {
                    return Err(crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "global next lineage integration conflicts at edge {}",
                            edge_index
                        ),
                    });
                }
                expected_next[edge_index] = Some(successor_index);
                override_edges.insert(edge_index);
                application_links.insert((edge_index, successor_index));
            }
        }
        stats.override_count = override_edges.len();
        stats.application_plan_link_count = application_links.len();

        for (edge_index, (&expected, &actual)) in expected_next
            .iter()
            .zip(&candidate.next_global_dir_edge_ids)
            .enumerate()
        {
            execution_policy.check_cancelled_every(
                "partition_border_global_next_lineage_integration_edges",
                edge_index,
            )?;
            match (expected, actual) {
                (Some(expected), Some(actual)) if expected == actual => {
                    stats.integrated_successor_count += 1;
                }
                (Some(_), None) => stats.missing_candidate_successor_count += 1,
                (Some(_), Some(_)) if override_edges.contains(&edge_index) => {
                    stats.override_lineage_mismatch_count += 1;
                }
                _ => stats.local_lineage_mismatch_count += 1,
            }
        }
        stats.unrepresented_application_link_count = application_links
            .iter()
            .filter(|&&(edge_index, successor_index)| {
                candidate.next_global_dir_edge_ids.get(edge_index).copied()
                    != Some(Some(successor_index))
            })
            .count();

        stats.committed_next_edge_count = self
            .global_next_global_dir_edge_ids
            .iter()
            .flatten()
            .count();
        if self.global_next_global_dir_edge_ids.len() != edge_count {
            stats.committed_next_mismatch_count = edge_count;
        } else {
            stats.committed_next_mismatch_count = self
                .global_next_global_dir_edge_ids
                .iter()
                .zip(&candidate.next_global_dir_edge_ids)
                .filter(|(committed, candidate)| committed != candidate)
                .count();
        }

        let mut edge_slot_by_local = BTreeMap::<(usize, usize, DirEdgeId), usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            if edge_slot_by_local
                .insert(
                    (edge.partition_id, edge.component_id, edge.local_dir_edge_id),
                    edge_index,
                )
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global next lineage integration duplicates local edge identity at {}",
                        edge_index
                    ),
                });
            }
        }
        let mut edge_slot_by_observation = BTreeMap::<PartitionBorderObservationId, usize>::new();
        for observation in self.observations.values() {
            let component_id = observation
                .face_ref
                .map_or(observation.component_id, |face_ref| face_ref.component_id);
            let Some(&edge_index) = edge_slot_by_local.get(&(
                observation.partition_id,
                component_id,
                observation.local_dir_edge_id,
            )) else {
                continue;
            };
            let edge = &self.global_face_edge_map[edge_index];
            if edge.edge_key != observation.edge_key || edge.face_ref != observation.face_ref {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global next lineage integration observation {:?} disagrees with edge lineage",
                        observation.observation_id()
                    ),
                });
            }
            if edge_slot_by_observation
                .insert(observation.observation_id(), edge_index)
                .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global next lineage integration duplicates observation {:?}",
                        observation.observation_id()
                    ),
                });
            }
        }
        for (twin_index, twin) in self.global_face_twin_transitions.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_next_lineage_integration_twins",
                twin_index,
            )?;
            stats.twin_count += 1;
            let Some(&forward_index) = edge_slot_by_observation.get(&twin.forward_observation_id)
            else {
                stats.twin_lineage_mismatch_count += 1;
                continue;
            };
            let Some(&reverse_index) = edge_slot_by_observation.get(&twin.reverse_observation_id)
            else {
                stats.twin_lineage_mismatch_count += 1;
                continue;
            };
            let forward = &self.global_face_edge_map[forward_index];
            let reverse = &self.global_face_edge_map[reverse_index];
            if forward.face_ref != Some(twin.forward_face_ref)
                || reverse.face_ref != Some(twin.reverse_face_ref)
                || forward.cross_border_twin_global_dir_edge_id != Some(reverse_index)
                || reverse.cross_border_twin_global_dir_edge_id != Some(forward_index)
            {
                stats.twin_lineage_mismatch_count += 1;
            }
        }

        stats.integration_ready = stats.identity_ready
            && stats.integrated_successor_count == edge_count
            && stats.missing_candidate_successor_count == 0
            && stats.local_lineage_mismatch_count == 0
            && stats.override_lineage_mismatch_count == 0
            && stats.unrepresented_application_link_count == 0
            && stats.committed_next_edge_count == edge_count
            && stats.committed_next_mismatch_count == 0
            && stats.twin_lineage_mismatch_count == 0;
        Ok(stats)
    }

    #[cfg(test)]
    pub(crate) fn global_topology_candidate(
        &self,
    ) -> Option<&PartitionBorderGlobalTopologyCandidate> {
        self.global_topology_candidate.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn global_next_global_dir_edge_ids(&self) -> &[Option<usize>] {
        &self.global_next_global_dir_edge_ids
    }

    /// Validates the detached candidate against declared-adjacency twin
    /// evidence immediately before any future global topology mutation. The
    /// gate is deliberately observational: it never rewrites edge, twin, or
    /// local arrangement links.
    pub(crate) fn validate_global_topology_application_gate(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalTopologyApplicationGateStats> {
        execution_policy.check_cancelled("partition_border_global_topology_application_gate")?;
        execution_policy.check(
            "partition_border_global_topology_application_gate_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        execution_policy.check(
            "partition_border_global_topology_application_gate_twins",
            execution_policy.max_graph_edges,
            self.applied_face_twins.len(),
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global topology application gate has no detached candidate".to_string(),
            });
        };
        let edge_count = self.global_face_edge_map.len();
        if candidate.next_global_dir_edge_ids.len() != edge_count {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global topology application gate candidate length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    edge_count
                ),
            });
        }

        let mut edge_slot_by_local = BTreeMap::<(usize, usize, DirEdgeId), usize>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_topology_application_gate_edges",
                edge_index,
            )?;
            if edge.global_dir_edge_id != edge_index
                || edge_slot_by_local
                    .insert(
                        (edge.partition_id, edge.component_id, edge.local_dir_edge_id),
                        edge_index,
                    )
                    .is_some()
            {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global topology application gate has invalid edge slot {}",
                        edge_index
                    ),
                });
            }
        }

        let mut edge_slot_by_observation = BTreeMap::<PartitionBorderObservationId, usize>::new();
        for (observation_index, observation) in self.observations.values().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_topology_application_gate_observations",
                observation_index,
            )?;
            let component_id = observation
                .face_ref
                .map_or(observation.component_id, |face_ref| face_ref.component_id);
            if let Some(&edge_index) = edge_slot_by_local.get(&(
                observation.partition_id,
                component_id,
                observation.local_dir_edge_id,
            )) {
                edge_slot_by_observation.insert(observation.observation_id(), edge_index);
            }
        }

        let mut candidate_successor_count = 0usize;
        let mut predecessor_by_edge = BTreeMap::<usize, usize>::new();
        let mut predecessor_conflict_count = 0usize;
        let mut node_discontinuity_count = 0usize;
        for (edge_index, successor) in candidate.next_global_dir_edge_ids.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_topology_application_gate_successors",
                edge_index,
            )?;
            let Some(successor_index) = successor else {
                continue;
            };
            if *successor_index >= edge_count {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global topology application gate edge {} has invalid successor {}",
                        edge_index, successor_index
                    ),
                });
            }
            candidate_successor_count += 1;
            let edge = &self.global_face_edge_map[edge_index];
            let successor_edge = &self.global_face_edge_map[*successor_index];
            if edge.to_global_node_id != successor_edge.from_global_node_id {
                node_discontinuity_count += 1;
            }
            if predecessor_by_edge
                .insert(*successor_index, edge_index)
                .is_some()
            {
                predecessor_conflict_count += 1;
            }
        }

        let mut mapped_twin_pairs = BTreeSet::<(usize, usize)>::new();
        let mut invalid_twin_count = 0usize;
        for (twin_index, applied_twin) in self.applied_face_twins.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_topology_application_gate_twins",
                twin_index,
            )?;
            let Some(forward) = self.observations.get(&applied_twin.twin.forward) else {
                invalid_twin_count += 1;
                continue;
            };
            let Some(reverse) = self.observations.get(&applied_twin.twin.reverse) else {
                invalid_twin_count += 1;
                continue;
            };
            if !self
                .adjacencies
                .iter()
                .any(|adjacency| adjacency.matches(forward, reverse))
            {
                invalid_twin_count += 1;
                continue;
            }
            let Some(&forward_index) = edge_slot_by_observation.get(&applied_twin.twin.forward)
            else {
                continue;
            };
            let Some(&reverse_index) = edge_slot_by_observation.get(&applied_twin.twin.reverse)
            else {
                continue;
            };
            let forward_edge = &self.global_face_edge_map[forward_index];
            let reverse_edge = &self.global_face_edge_map[reverse_index];
            if forward_edge.cross_border_twin_global_dir_edge_id != Some(reverse_index)
                || reverse_edge.cross_border_twin_global_dir_edge_id != Some(forward_index)
                || forward_edge.partition_id == reverse_edge.partition_id
            {
                invalid_twin_count += 1;
                continue;
            }
            mapped_twin_pairs.insert(if forward_index < reverse_index {
                (forward_index, reverse_index)
            } else {
                (reverse_index, forward_index)
            });
        }

        let mut edge_map_twin_pairs = BTreeSet::<(usize, usize)>::new();
        for (edge_index, edge) in self.global_face_edge_map.iter().enumerate() {
            let Some(twin_index) = edge.cross_border_twin_global_dir_edge_id else {
                continue;
            };
            if twin_index >= edge_count {
                return Err(crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "global topology application gate edge {} has invalid twin {}",
                        edge_index, twin_index
                    ),
                });
            }
            edge_map_twin_pairs.insert(if edge_index < twin_index {
                (edge_index, twin_index)
            } else {
                (twin_index, edge_index)
            });
        }
        let unmapped_twin_count = edge_map_twin_pairs.difference(&mapped_twin_pairs).count();
        let application_ready = candidate_successor_count == edge_count
            && predecessor_by_edge.len() == edge_count
            && predecessor_conflict_count == 0
            && node_discontinuity_count == 0
            && invalid_twin_count == 0
            && unmapped_twin_count == 0
            && mapped_twin_pairs.len() == edge_map_twin_pairs.len();

        Ok(PartitionBorderGlobalTopologyApplicationGateStats {
            edge_count,
            candidate_successor_count,
            declared_adjacency_count: self.adjacencies.len(),
            applied_twin_count: self.applied_face_twins.len(),
            mapped_twin_count: mapped_twin_pairs.len(),
            unmapped_twin_count,
            invalid_twin_count,
            predecessor_conflict_count,
            node_discontinuity_count,
            application_ready,
        })
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

    /// Validates that exactly one local-unbounded face proof is represented by
    /// one mapped candidate global face ID and one detached candidate cycle.
    /// This is the final unbounded-face evidence gate before any future global
    /// face identity mutation; it never changes the graph.
    #[cfg(test)]
    pub(crate) fn validate_global_unbounded_face_application(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<PartitionBorderGlobalUnboundedFaceApplicationStats> {
        execution_policy.check_cancelled("partition_border_global_unbounded_face_application")?;
        let walk = self.validate_global_face_walk_invariants(execution_policy)?;
        let proof = self.validate_global_unbounded_face_proof_with_walk(execution_policy, walk)?;
        let face_id_application = self.validate_global_face_id_application(execution_policy)?;
        self.validate_global_unbounded_face_application_with_evidence(
            execution_policy,
            proof,
            face_id_application,
        )
    }

    pub(crate) fn validate_global_unbounded_face_application_with_evidence(
        &self,
        execution_policy: &ExecutionPolicy,
        proof: PartitionBorderGlobalUnboundedFaceProofStats,
        face_id_application: PartitionBorderGlobalFaceIdApplicationStats,
    ) -> crate::Result<PartitionBorderGlobalUnboundedFaceApplicationStats> {
        execution_policy.check_cancelled("partition_border_global_unbounded_face_application")?;
        execution_policy.check(
            "partition_border_global_unbounded_face_application_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global unbounded face application has no detached topology candidate"
                    .to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != self.global_face_edge_map.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global unbounded face application candidate length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    self.global_face_edge_map.len()
                ),
            });
        }
        let candidate_cycle_count = candidate.cycle_start_global_dir_edge_ids.len();
        let local_unbounded_face_count = self
            .global_face_plans
            .iter()
            .filter(|plan| plan.local_face_is_unbounded)
            .count();
        let mut unbounded_face_ids = BTreeSet::new();
        let mut candidate_unbounded_face_id_count = 0usize;
        for (plan_index, plan) in self.global_face_id_plans.iter().enumerate() {
            execution_policy.check_cancelled_every(
                "partition_border_global_unbounded_face_application_plans",
                plan_index,
            )?;
            if plan.local_unbounded_face_count == 0 {
                continue;
            }
            if let Some(face_id) = plan.candidate_global_face_id {
                candidate_unbounded_face_id_count += 1;
                unbounded_face_ids.insert(face_id);
            }
        }
        let duplicate_unbounded_face_id_count =
            candidate_unbounded_face_id_count.saturating_sub(unbounded_face_ids.len());
        let missing_unbounded_face_id_count =
            local_unbounded_face_count.saturating_sub(candidate_unbounded_face_id_count);
        let mapped_unbounded_cycle_count = if face_id_application.application_ready {
            candidate_unbounded_face_id_count
        } else {
            0
        };
        let application_ready = proof.proof_ready
            && face_id_application.application_ready
            && face_id_application.candidate_cycle_count == self.global_face_id_plans.len()
            && face_id_application.candidate_cycle_count == candidate_cycle_count
            && candidate_cycle_count == face_id_application.candidate_cycle_start_count
            && local_unbounded_face_count == 1
            && candidate_unbounded_face_id_count == 1
            && mapped_unbounded_cycle_count == 1
            && missing_unbounded_face_id_count == 0
            && duplicate_unbounded_face_id_count == 0;
        Ok(PartitionBorderGlobalUnboundedFaceApplicationStats {
            face_count: proof.face_count,
            candidate_cycle_count,
            local_unbounded_face_count,
            candidate_unbounded_face_id_count,
            mapped_unbounded_cycle_count,
            missing_unbounded_face_id_count,
            duplicate_unbounded_face_id_count,
            proof_ready: proof.proof_ready,
            application_ready,
        })
    }

    /// Combines every retained detached-topology proof boundary into one
    /// deterministic readiness result. This remains evidence only: no graph
    /// identity, successor, face ID, or output field is mutated.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_global_topology_mutation_gate_with_evidence(
        &self,
        execution_policy: &ExecutionPolicy,
        topology_application: PartitionBorderGlobalTopologyApplicationGateStats,
        component_coverage: PartitionBorderGlobalComponentCoverageStats,
        face_id_application: PartitionBorderGlobalFaceIdApplicationStats,
        unbounded_face_application: PartitionBorderGlobalUnboundedFaceApplicationStats,
        walk: PartitionBorderGlobalFaceWalkInvariantStats,
        euler: PartitionBorderGlobalFaceEulerWitnessStats,
    ) -> crate::Result<PartitionBorderGlobalTopologyMutationGateStats> {
        execution_policy.check_cancelled("partition_border_global_topology_mutation_gate")?;
        execution_policy.check(
            "partition_border_global_topology_mutation_gate_edges",
            execution_policy.max_graph_edges,
            self.global_face_edge_map.len(),
        )?;
        execution_policy.check(
            "partition_border_global_topology_mutation_gate_components",
            execution_policy.max_graph_nodes,
            self.global_components.len(),
        )?;
        let Some(candidate) = self.global_topology_candidate.as_ref() else {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: "global topology mutation gate has no detached topology candidate"
                    .to_string(),
            });
        };
        if candidate.next_global_dir_edge_ids.len() != self.global_face_edge_map.len() {
            return Err(crate::PolygonizeError::InternalInvariantViolation {
                reason: format!(
                    "global topology mutation gate candidate length mismatch: candidate={}, edges={}",
                    candidate.next_global_dir_edge_ids.len(),
                    self.global_face_edge_map.len()
                ),
            });
        }
        let face_walk_ready = walk.face_count > 0
            && walk.closed_face_count == walk.face_count
            && walk.unmapped_twin_count == 0
            && walk.mapped_twin_count == walk.applied_twin_count
            && walk.source_complete_twin_count == walk.applied_twin_count
            && walk.unbounded_face_count == 1
            && walk.unbounded_component_count == 1;
        let euler_evidence_ready = euler.boundary_euler_consistent;
        let gate_ready = topology_application.application_ready
            && component_coverage.coverage_ready
            && face_id_application.application_ready
            && unbounded_face_application.application_ready
            && face_walk_ready
            && euler_evidence_ready;
        Ok(PartitionBorderGlobalTopologyMutationGateStats {
            edge_count: self.global_face_edge_map.len(),
            component_count: self.global_components.len(),
            face_count: walk.face_count,
            candidate_cycle_count: candidate.cycle_start_global_dir_edge_ids.len(),
            applied_twin_count: walk.applied_twin_count,
            mapped_twin_count: walk.mapped_twin_count,
            source_complete_twin_count: walk.source_complete_twin_count,
            closed_face_count: walk.closed_face_count,
            euler_boundary_lhs: euler.boundary_euler_lhs,
            euler_boundary_rhs: euler.boundary_euler_rhs,
            topology_application_ready: topology_application.application_ready,
            component_coverage_ready: component_coverage.coverage_ready,
            face_id_application_ready: face_id_application.application_ready,
            unbounded_face_application_ready: unbounded_face_application.application_ready,
            face_walk_ready,
            euler_evidence_ready,
            gate_ready,
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

    fn local_face_graph(
        partition_id: usize,
        component_id: usize,
        first_local_dir_edge_id: DirEdgeId,
        forward_face_id: usize,
        from: Coord3D,
        to: Coord3D,
        source_line_id: u32,
    ) -> PartitionBorderLocalFaceGraph {
        let from_key = PartitionBorderNodeKey::from_coord(from);
        let to_key = PartitionBorderNodeKey::from_coord(to);
        let edge_key = PartitionBorderEdgeKey::new(from_key, to_key).unwrap();
        let reverse_edge_key = edge_key;
        let reverse_local_dir_edge_id = first_local_dir_edge_id + 1;
        PartitionBorderLocalFaceGraph {
            partition_id,
            component_id,
            directed_edges: vec![
                PartitionBorderLocalDirectedEdge {
                    local_dir_edge_id: first_local_dir_edge_id,
                    symmetric_local_dir_edge_id: reverse_local_dir_edge_id,
                    local_face_successor: Some(first_local_dir_edge_id),
                    from: from_key,
                    to: to_key,
                    from_z_bits: canonical_coordinate_bits(from.z),
                    to_z_bits: canonical_coordinate_bits(to.z),
                    edge_key,
                    face_ref: Some(face(partition_id, component_id, forward_face_id)),
                    local_face_is_unbounded: false,
                    source_line_ids: vec![source_line_id],
                },
                PartitionBorderLocalDirectedEdge {
                    local_dir_edge_id: reverse_local_dir_edge_id,
                    symmetric_local_dir_edge_id: first_local_dir_edge_id,
                    local_face_successor: Some(reverse_local_dir_edge_id),
                    from: to_key,
                    to: from_key,
                    from_z_bits: canonical_coordinate_bits(to.z),
                    to_z_bits: canonical_coordinate_bits(from.z),
                    edge_key: reverse_edge_key,
                    face_ref: Some(face(partition_id, component_id, forward_face_id + 1)),
                    local_face_is_unbounded: false,
                    source_line_ids: vec![source_line_id],
                },
            ],
        }
    }

    fn exact_face_edge_map_graph(reverse_snapshot_order: bool) -> PartitionBorderGraph {
        let mut graph = exact_face_twin_graph();
        for observation in graph.observations.values_mut() {
            observation.local_face_successor = Some(observation.local_dir_edge_id);
            observation.local_face_is_unbounded = false;
        }
        let first = local_face_graph(1, 4, 7, 9, coord(0.0, 0.0, 1.0), coord(2.0, 0.0, 2.0), 8);
        let second = local_face_graph(2, 6, 9, 11, coord(2.0, 0.0, 20.0), coord(0.0, 0.0, 10.0), 7);
        if reverse_snapshot_order {
            graph.insert_local_face_graph(second).unwrap();
            graph.insert_local_face_graph(first).unwrap();
        } else {
            graph.insert_local_face_graph(first).unwrap();
            graph.insert_local_face_graph(second).unwrap();
        }
        graph
    }

    fn exact_face_next_application_graph(reverse_snapshot_order: bool) -> PartitionBorderGraph {
        let mut graph = exact_face_edge_map_graph(reverse_snapshot_order);
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let forward = graph
            .observations
            .values()
            .find(|observation| observation.partition_id == 1)
            .unwrap()
            .observation_id();
        let reverse = graph
            .observations
            .values()
            .find(|observation| observation.partition_id == 2)
            .unwrap()
            .observation_id();
        graph.global_components = vec![PartitionBorderGlobalComponent {
            component_index: 0,
            face_refs: vec![
                face(1, 4, 9),
                face(1, 4, 10),
                face(2, 6, 11),
                face(2, 6, 12),
            ],
            border_node_keys: vec![
                PartitionBorderNodeKey::from_coord(coord(0.0, 0.0, 0.0)),
                PartitionBorderNodeKey::from_coord(coord(2.0, 0.0, 0.0)),
            ],
            twin_edge_keys: vec![graph.observations.get(&forward).unwrap().edge_key],
        }];
        graph.global_face_next_mutation_plans = vec![PartitionBorderGlobalFaceNextMutationPlan {
            component_index: 0,
            boundary_observation_ids: vec![forward, reverse],
            successor_observation_ids: vec![reverse, forward],
            closed: true,
        }];
        graph
    }

    fn exact_global_topology_candidate_graph() -> PartitionBorderGraph {
        let mut graph = exact_face_next_application_graph(false);
        graph.global_face_edge_map[0].local_face_successor_global_dir_edge_id = Some(1);
        graph.global_face_edge_map[1].local_face_successor_global_dir_edge_id = Some(3);
        graph.global_face_edge_map[2].local_face_successor_global_dir_edge_id = Some(0);
        graph.global_face_edge_map[3].local_face_successor_global_dir_edge_id = Some(1);
        graph
            .reconcile_global_face_next_application_plans(&ExecutionPolicy::default())
            .unwrap();
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
    fn global_face_edge_map_is_deterministic_and_maps_face_twins() {
        let mut graph = exact_face_edge_map_graph(false);
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        let stats = graph
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();

        assert_eq!(stats.local_graph_count, 2);
        assert_eq!(stats.component_count, 2);
        assert_eq!(stats.directed_edge_count, 4);
        assert_eq!(stats.local_successor_count, 4);
        assert_eq!(stats.mapped_observation_count, 2);
        assert_eq!(stats.mapped_twin_count, 1);
        assert_eq!(stats.unmapped_twin_count, 0);
        assert!(stats.edge_map_ready);
        assert_eq!(
            graph
                .global_face_edge_map
                .iter()
                .map(|edge| (
                    edge.partition_id,
                    edge.component_id,
                    edge.local_dir_edge_id,
                    edge.symmetric_global_dir_edge_id,
                    edge.local_face_successor_global_dir_edge_id,
                    edge.cross_border_twin_global_dir_edge_id,
                    edge.from_z_bits,
                    edge.to_z_bits,
                    edge.source_line_ids.clone(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    1,
                    4,
                    7,
                    1,
                    Some(0),
                    Some(2),
                    1.0f64.to_bits(),
                    2.0f64.to_bits(),
                    vec![8]
                ),
                (
                    1,
                    4,
                    8,
                    0,
                    Some(1),
                    None,
                    2.0f64.to_bits(),
                    1.0f64.to_bits(),
                    vec![8]
                ),
                (
                    2,
                    6,
                    9,
                    3,
                    Some(2),
                    Some(0),
                    20.0f64.to_bits(),
                    10.0f64.to_bits(),
                    vec![7]
                ),
                (
                    2,
                    6,
                    10,
                    2,
                    Some(3),
                    None,
                    10.0f64.to_bits(),
                    20.0f64.to_bits(),
                    vec![7]
                ),
            ]
        );

        let mut reordered = exact_face_edge_map_graph(true);
        reordered
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(graph.global_face_edge_map, reordered.global_face_edge_map);
    }

    #[test]
    fn global_face_edge_map_is_atomic_and_bounded() {
        let mut malformed = exact_face_edge_map_graph(false);
        malformed
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        malformed
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        let before = malformed.global_face_edge_map.clone();
        malformed.local_face_graphs[0].directed_edges[0].symmetric_local_dir_edge_id = 999;
        assert!(matches!(
            malformed.reconcile_global_face_edge_map(&ExecutionPolicy::default()),
            Err(crate::PolygonizeError::InternalInvariantViolation { .. })
        ));
        assert_eq!(malformed.global_face_edge_map, before);

        let mut limited = exact_face_edge_map_graph(false);
        let error = limited
            .reconcile_global_face_edge_map(&ExecutionPolicy {
                max_graph_edges: Some(3),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 3,
                observed: 4,
            } if stage == "partition_border_global_face_edge_map_edges"
        ));
        assert!(limited.global_face_edge_map.is_empty());

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_edge_map_graph(false);
        let error = cancelled
            .reconcile_global_face_edge_map(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_edge_map"
        ));
        assert!(cancelled.global_face_edge_map.is_empty());
    }

    #[test]
    fn global_face_nodes_are_deterministic_and_retain_endpoint_payloads() {
        let mut graph = exact_face_edge_map_graph(false);
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        let stats = graph
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();

        assert_eq!(stats.edge_count, 4);
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.endpoint_count, 8);
        assert_eq!(stats.mapped_observation_count, 2);
        assert_eq!(stats.unmapped_observation_count, 0);
        assert_eq!(stats.z_candidate_count, 4);
        assert_eq!(stats.z_conflict_count, 2);
        assert!(stats.node_map_ready);
        assert_eq!(
            graph
                .global_face_nodes
                .iter()
                .map(|node| (
                    node.global_node_id,
                    node.key,
                    node.source_line_ids.clone(),
                    node.representative_line_ids.clone(),
                    node.z_bits.clone(),
                    node.incident_global_dir_edge_ids.clone(),
                    node.z_conflict,
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    0,
                    PartitionBorderNodeKey::from_coord(coord(0.0, 0.0, 0.0)),
                    vec![7, 8],
                    vec![7, 8],
                    vec![1.0f64.to_bits(), 10.0f64.to_bits()],
                    vec![0, 1, 2, 3],
                    true,
                ),
                (
                    1,
                    PartitionBorderNodeKey::from_coord(coord(2.0, 0.0, 0.0)),
                    vec![7, 8],
                    vec![7, 8],
                    vec![2.0f64.to_bits(), 20.0f64.to_bits()],
                    vec![0, 1, 2, 3],
                    true,
                ),
            ]
        );
        assert!(graph.global_face_edge_map.iter().all(|edge| {
            edge.from_global_node_id.is_some() && edge.to_global_node_id.is_some()
        }));

        let mut reordered = exact_face_edge_map_graph(true);
        reordered
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        reordered
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        assert_eq!(graph.global_face_nodes, reordered.global_face_nodes);
        assert_eq!(graph.global_face_edge_map, reordered.global_face_edge_map);
    }

    #[test]
    fn global_face_nodes_are_atomic_and_bounded() {
        let mut malformed = exact_face_edge_map_graph(false);
        malformed
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        malformed
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        malformed
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let before_nodes = malformed.global_face_nodes.clone();
        let before_edges = malformed.global_face_edge_map.clone();
        let observation_id = malformed.observations.keys().next().copied().unwrap();
        malformed
            .observations
            .get_mut(&observation_id)
            .unwrap()
            .source_line_ids = vec![99];
        assert!(matches!(
            malformed.reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default()),
            Err(crate::PolygonizeError::InternalInvariantViolation { .. })
        ));
        assert_eq!(malformed.global_face_nodes, before_nodes);
        assert_eq!(malformed.global_face_edge_map, before_edges);

        let mut z_conflict = exact_face_edge_map_graph(false);
        z_conflict
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        z_conflict
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        let error = z_conflict
            .reconcile_global_face_nodes(
                ZOptions {
                    policy: ZPolicy::ErrorOnConflict,
                    ..ZOptions::default()
                },
                &ExecutionPolicy::default(),
            )
            .unwrap_err();
        assert!(matches!(error, crate::PolygonizeError::ZConflict { .. }));
        assert!(z_conflict.global_face_nodes.is_empty());
        assert!(z_conflict
            .global_face_edge_map
            .iter()
            .all(|edge| edge.from_global_node_id.is_none() && edge.to_global_node_id.is_none()));

        let mut limited = exact_face_edge_map_graph(false);
        limited
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        limited
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .reconcile_global_face_nodes(
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
            } if stage == "partition_border_global_face_nodes"
        ));
        assert!(limited.global_face_nodes.is_empty());

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_edge_map_graph(false);
        cancelled
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        cancelled
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .reconcile_global_face_nodes(
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
                if stage == "partition_border_global_face_nodes"
        ));
        assert!(cancelled.global_face_nodes.is_empty());
    }

    #[test]
    fn canonical_border_nodes_validate_active_global_payloads_without_mutation() {
        let mut graph = exact_face_edge_map_graph(false);
        graph
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        graph
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let before = graph.clone();
        let stats = graph
            .validate_canonical_border_nodes(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderCanonicalNodeValidationStats {
                canonical_node_count: 2,
                global_node_count: 2,
                mapped_global_node_count: 2,
                canonical_only_node_count: 0,
                reconciliation_ready: true,
                ..Default::default()
            }
        );
        assert_eq!(graph, before);

        graph.global_face_nodes[0].selected_z_bits = 0;
        let stats = graph
            .validate_canonical_border_nodes(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.selected_z_mismatch_count, 1);
        assert!(!stats.reconciliation_ready);
    }

    #[test]
    fn canonical_border_node_validation_is_bounded_and_cancellable() {
        let mut limited = exact_face_edge_map_graph(false);
        limited
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        limited
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        limited
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        limited
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let before = limited.clone();
        let error = limited
            .validate_canonical_border_nodes(&ExecutionPolicy {
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
            } if stage == "partition_border_canonical_node_validation_nodes"
        ));
        assert_eq!(limited, before);

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_edge_map_graph(false);
        cancelled
            .apply_unambiguous_face_twins(&ExecutionPolicy::default())
            .unwrap();
        cancelled
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        cancelled
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        cancelled
            .reconcile_border_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .validate_canonical_border_nodes(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_canonical_node_validation"
        ));
    }

    #[test]
    fn global_face_next_application_maps_cycles_and_twins_deterministically() {
        let mut graph = exact_face_next_application_graph(false);
        let stats = graph
            .reconcile_global_face_next_application_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceNextApplicationStats {
                component_count: 1,
                plan_count: 1,
                candidate_link_count: 2,
                mapped_edge_count: 4,
                mapped_twin_count: 1,
                unmapped_observation_count: 0,
                incomplete_plan_count: 0,
                node_discontinuity_count: 0,
                application_ready: true,
            }
        );
        assert_eq!(
            graph.global_face_next_application_plans(),
            &[PartitionBorderGlobalFaceNextApplicationPlan {
                component_index: 0,
                global_dir_edge_ids: vec![0, 2],
                successor_global_dir_edge_ids: vec![2, 0],
                closed: true,
                node_continuous: true,
            }]
        );

        let mut reordered = exact_face_next_application_graph(true);
        reordered
            .reconcile_global_face_next_application_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            graph.global_face_next_application_plans(),
            reordered.global_face_next_application_plans()
        );
    }

    #[test]
    fn global_face_next_application_is_atomic_and_bounded() {
        let mut malformed = exact_face_next_application_graph(false);
        malformed
            .reconcile_global_face_next_application_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = malformed.global_face_next_application_plans.clone();
        malformed.global_face_next_mutation_plans[0].boundary_observation_ids[1] =
            malformed.global_face_next_mutation_plans[0].boundary_observation_ids[0];
        assert!(matches!(
            malformed.reconcile_global_face_next_application_plans(&ExecutionPolicy::default()),
            Err(crate::PolygonizeError::InternalInvariantViolation { .. })
        ));
        assert_eq!(malformed.global_face_next_application_plans, before);

        let mut limited = exact_face_next_application_graph(false);
        let error = limited
            .reconcile_global_face_next_application_plans(&ExecutionPolicy {
                max_graph_edges: Some(3),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 3,
                observed: 4,
            } if stage == "partition_border_global_face_next_application_edges"
        ));
        assert!(limited.global_face_next_application_plans.is_empty());

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_face_next_application_graph(false);
        let error = cancelled
            .reconcile_global_face_next_application_plans(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_next_application"
        ));
        assert!(cancelled.global_face_next_application_plans.is_empty());
    }

    #[test]
    fn global_topology_candidate_materializes_closed_cycles_without_mutation() {
        let mut graph = exact_global_topology_candidate_graph();
        let before_edges = graph.global_face_edge_map.clone();
        let stats = graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalTopologyCandidateStats {
                edge_count: 4,
                local_successor_count: 4,
                global_override_count: 2,
                assigned_next_count: 4,
                unassigned_next_count: 0,
                cycle_count: 2,
                closed_cycle_edge_count: 4,
                predecessor_conflict_count: 0,
                node_discontinuity_count: 0,
                incomplete_application_plan_count: 0,
                candidate_ready: true,
            }
        );
        assert_eq!(
            graph.global_topology_candidate(),
            Some(&PartitionBorderGlobalTopologyCandidate {
                next_global_dir_edge_ids: vec![Some(2), Some(3), Some(0), Some(1)],
                cycle_start_global_dir_edge_ids: vec![0, 1],
            })
        );
        assert_eq!(graph.global_face_edge_map, before_edges);
    }

    #[test]
    fn global_topology_candidate_is_atomic_bounded_and_fail_closed() {
        let mut malformed = exact_global_topology_candidate_graph();
        malformed
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let before = malformed.global_topology_candidate.clone();
        malformed.global_face_edge_map[0].local_face_successor_global_dir_edge_id = Some(99);
        assert!(matches!(
            malformed.reconcile_global_topology_candidate(&ExecutionPolicy::default()),
            Err(crate::PolygonizeError::InternalInvariantViolation { .. })
        ));
        assert_eq!(malformed.global_topology_candidate, before);

        let mut limited = exact_global_topology_candidate_graph();
        let error = limited
            .reconcile_global_topology_candidate(&ExecutionPolicy {
                max_graph_edges: Some(3),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 3,
                observed: 4,
            } if stage == "partition_border_global_topology_candidate_edges"
        ));
        assert!(limited.global_topology_candidate.is_none());

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        let error = cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_topology_candidate"
        ));
        assert!(cancelled.global_topology_candidate.is_none());

        let mut incomplete = exact_global_topology_candidate_graph();
        incomplete.global_face_next_application_plans[0].closed = false;
        let stats = incomplete
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.incomplete_application_plan_count, 1);
        assert!(!stats.candidate_ready);
        assert!(incomplete.global_topology_candidate().is_some());
    }

    #[test]
    fn global_topology_application_gate_requires_declared_twins_and_complete_candidate() {
        let mut graph = exact_global_topology_candidate_graph();
        graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let stats = graph
            .validate_global_topology_application_gate(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalTopologyApplicationGateStats {
                edge_count: 4,
                candidate_successor_count: 4,
                declared_adjacency_count: 1,
                applied_twin_count: 1,
                mapped_twin_count: 1,
                unmapped_twin_count: 0,
                invalid_twin_count: 0,
                predecessor_conflict_count: 0,
                node_discontinuity_count: 0,
                application_ready: true,
            }
        );

        graph.adjacencies.clear();
        let stats = graph
            .validate_global_topology_application_gate(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.invalid_twin_count, 1);
        assert_eq!(stats.unmapped_twin_count, 1);
        assert!(!stats.application_ready);

        let mut incomplete = exact_global_topology_candidate_graph();
        incomplete
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        incomplete
            .global_topology_candidate
            .as_mut()
            .unwrap()
            .next_global_dir_edge_ids[0] = None;
        let stats = incomplete
            .validate_global_topology_application_gate(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.candidate_successor_count, 3);
        assert!(!stats.application_ready);
    }

    #[test]
    fn global_topology_application_gate_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .validate_global_topology_application_gate(&ExecutionPolicy {
                max_graph_edges: Some(3),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 3,
                observed: 4,
            } if stage == "partition_border_global_topology_application_gate_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .validate_global_topology_application_gate(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_topology_application_gate"
        ));
    }

    #[test]
    fn global_component_coverage_is_complete_and_deterministic() {
        let mut graph = exact_global_topology_candidate_graph();
        graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let stats = graph
            .validate_global_component_coverage(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalComponentCoverageStats {
                component_count: 1,
                face_count: 4,
                edge_count: 4,
                face_edge_count: 4,
                covered_face_edge_count: 4,
                uncovered_face_edge_count: 0,
                duplicate_face_count: 0,
                duplicate_twin_edge_count: 0,
                coverage_ready: true,
            }
        );

        graph.global_components[0].face_refs.push(face(1, 4, 9));
        let stats = graph
            .validate_global_component_coverage(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.duplicate_face_count, 1);
        assert!(!stats.coverage_ready);
    }

    #[test]
    fn global_component_coverage_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .validate_global_component_coverage(&ExecutionPolicy {
                max_graph_edges: Some(3),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 3,
                observed: 4,
            } if stage == "partition_border_global_component_coverage_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .validate_global_component_coverage(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_component_coverage"
        ));
    }

    #[test]
    fn global_face_id_application_maps_candidate_cycles_deterministically() {
        let mut graph = exact_global_topology_candidate_graph();
        graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        for edge_index in [1usize, 3usize] {
            let edge = graph.global_face_edge_map[edge_index].clone();
            let start = Coord3D::new(
                f64::from_bits(edge.from.xy_bits()[0]),
                f64::from_bits(edge.from.xy_bits()[1]),
                f64::from_bits(edge.from_z_bits),
            );
            let end = Coord3D::new(
                f64::from_bits(edge.to.xy_bits()[0]),
                f64::from_bits(edge.to.xy_bits()[1]),
                f64::from_bits(edge.to_z_bits),
            );
            let observation = PartitionBorderHalfEdge::new_with_face_ref(
                edge.partition_id,
                edge.local_dir_edge_id,
                edge.face_ref,
                PartitionBorderSide::MinY,
                start,
                end,
                edge.source_line_ids,
            )
            .unwrap();
            graph
                .observations
                .insert(observation.observation_id(), observation);
        }
        let observations = graph
            .global_face_edge_map
            .iter()
            .map(|edge| {
                graph
                    .observations
                    .values()
                    .find(|observation| {
                        observation.partition_id == edge.partition_id
                            && observation.local_dir_edge_id == edge.local_dir_edge_id
                            && observation.edge_key == edge.edge_key
                            && observation.from == edge.from
                            && observation.to == edge.to
                            && observation.from_z_bits == edge.from_z_bits
                            && observation.to_z_bits == edge.to_z_bits
                            && observation.source_line_ids == edge.source_line_ids
                            && observation.face_ref == edge.face_ref
                            && observation.local_face_is_unbounded == edge.local_face_is_unbounded
                    })
                    .unwrap()
                    .observation_id()
            })
            .collect::<Vec<_>>();
        for (candidate_global_face_id, cycle) in
            [[0usize, 2usize], [1usize, 3usize]].into_iter().enumerate()
        {
            graph
                .global_face_id_plans
                .push(PartitionBorderGlobalFaceIdPlan {
                    candidate_global_face_id: Some(candidate_global_face_id),
                    component_index: 0,
                    boundary_observation_ids: cycle
                        .iter()
                        .map(|edge_index| observations[*edge_index])
                        .collect(),
                    face_refs: cycle
                        .iter()
                        .filter_map(|edge_index| graph.global_face_edge_map[*edge_index].face_ref)
                        .collect(),
                    local_unbounded_face_count: 0,
                    closed: true,
                });
        }

        assert_eq!(
            graph
                .validate_global_face_id_application(&ExecutionPolicy::default())
                .unwrap(),
            PartitionBorderGlobalFaceIdApplicationStats {
                component_count: 1,
                candidate_cycle_count: 2,
                assigned_face_count: 2,
                candidate_cycle_start_count: 2,
                mapped_cycle_count: 2,
                unmapped_plan_count: 0,
                duplicate_face_id_count: 0,
                non_contiguous_face_id_count: 0,
                application_ready: true,
            }
        );

        graph.global_face_id_plans[1].candidate_global_face_id = Some(0);
        let stats = graph
            .validate_global_face_id_application(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.duplicate_face_id_count, 1);
        assert!(!stats.application_ready);
    }

    #[test]
    fn global_face_id_application_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .validate_global_face_id_application(&ExecutionPolicy {
                max_graph_edges: Some(3),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 3,
                observed: 4,
            } if stage == "partition_border_global_face_id_application_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .validate_global_face_id_application(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_id_application"
        ));
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
    fn global_face_next_mutation_plans_are_atomic_and_require_closed_cycles() {
        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let mut complete = graph.clone();
        let before = complete.clone();
        let stats = complete
            .reconcile_global_face_next_mutation_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceNextMutationPlanStats {
                component_count: 1,
                boundary_observation_count: 2,
                plan_count: 1,
                candidate_link_count: 2,
                ready_component_count: 1,
                incomplete_component_count: 0,
                mutation_ready: true,
            }
        );
        let mutation_plan = complete
            .global_face_next_mutation_plans()
            .first()
            .expect("global next mutation plan");
        assert!(mutation_plan.closed);
        assert_eq!(
            mutation_plan.boundary_observation_ids.len(),
            mutation_plan.successor_observation_ids.len()
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

        let mut incomplete = prepared_global_face_walk_graph(false, [false, false]);
        let stats = incomplete
            .reconcile_global_face_next_mutation_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.component_count, 1);
        assert_eq!(stats.boundary_observation_count, 2);
        assert_eq!(stats.plan_count, 1);
        assert_eq!(stats.candidate_link_count, 0);
        assert_eq!(stats.ready_component_count, 0);
        assert_eq!(stats.incomplete_component_count, 1);
        assert!(!stats.mutation_ready);
        assert!(!incomplete.global_face_next_mutation_plans()[0].closed);

        let mut malformed = graph.clone();
        malformed
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap();
        malformed.global_face_identity_plans[0].boundary_observation_ids[1] =
            malformed.global_face_identity_plans[0].boundary_observation_ids[0];
        let before = malformed.clone();
        let error = malformed
            .reconcile_global_face_next_mutation_plans(&ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("observation") && reason.contains("duplicated")
        ));
        assert_eq!(malformed, before);
        assert!(malformed.global_face_next_mutation_plans().is_empty());

        let walk = graph
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        let error = graph
            .clone()
            .reconcile_global_face_next_mutation_plans_with_walk(
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
            } if stage == "partition_border_global_face_next_mutation_plans_faces"
        ));
    }

    #[test]
    fn global_face_id_plans_assign_only_closed_boundary_cycles() {
        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let mut complete = graph.clone();
        let stats = complete
            .reconcile_global_face_id_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceIdPlanStats {
                component_count: 1,
                candidate_cycle_count: 1,
                assigned_face_count: 1,
                boundary_observation_count: 2,
                unbounded_candidate_count: 0,
                incomplete_plan_count: 0,
                assignment_ready: true,
            }
        );
        let plan = complete
            .global_face_id_plans()
            .first()
            .expect("global face ID plan");
        assert_eq!(plan.candidate_global_face_id, Some(0));
        assert!(plan.closed);
        assert_eq!(plan.local_unbounded_face_count, 0);
        assert_eq!(complete.global_face_plans(), graph.global_face_plans());

        let mut incomplete = prepared_global_face_walk_graph(false, [false, false]);
        let stats = incomplete
            .reconcile_global_face_id_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.candidate_cycle_count, 1);
        assert_eq!(stats.assigned_face_count, 0);
        assert_eq!(stats.boundary_observation_count, 2);
        assert_eq!(stats.incomplete_plan_count, 1);
        assert!(!stats.assignment_ready);
        assert_eq!(
            incomplete.global_face_id_plans()[0].candidate_global_face_id,
            None
        );
        assert!(!incomplete.global_face_id_plans()[0].closed);

        let unbounded = prepared_global_face_walk_graph(true, [true, false]);
        let mut unbounded = unbounded;
        let stats = unbounded
            .reconcile_global_face_id_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.unbounded_candidate_count, 1);
        assert!(unbounded.global_face_id_plans()[0].closed);
        assert_eq!(
            unbounded.global_face_id_plans()[0].local_unbounded_face_count,
            1
        );
    }

    #[test]
    fn global_face_id_plans_are_atomic_deterministic_and_bounded() {
        let graph = prepared_global_face_walk_graph(true, [false, false]);
        let walk = graph
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        let mut ready = graph.clone();
        ready
            .reconcile_global_face_next_candidates(&ExecutionPolicy::default())
            .unwrap();
        ready
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap();
        ready
            .reconcile_global_face_next_mutation_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = ready.clone();
        let error = ready
            .reconcile_global_face_id_plans_with_walk(
                &ExecutionPolicy {
                    max_graph_nodes: Some(0),
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
            } if stage == "partition_border_global_face_id_plans_faces"
        ));
        assert_eq!(ready, before);

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .clone()
            .reconcile_global_face_id_plans_with_walk(
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
                if stage == "partition_border_global_face_id_plans"
        ));

        let mut malformed = graph.clone();
        malformed
            .reconcile_global_face_id_plans(&ExecutionPolicy::default())
            .unwrap();
        let before = malformed.clone();
        malformed.global_face_next_mutation_plans[0].closed = false;
        let error = malformed
            .reconcile_global_face_id_plans_with_walk(&ExecutionPolicy::default(), walk)
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::InternalInvariantViolation { ref reason }
                if reason.contains("disagrees")
        ));
        assert_eq!(
            malformed.global_face_id_plans(),
            before.global_face_id_plans()
        );
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
    fn global_unbounded_face_application_requires_one_mapped_candidate() {
        let mut graph = exact_global_topology_candidate_graph();
        graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        graph.global_face_plans = vec![PartitionBorderGlobalFacePlan {
            face_ref: face(1, 4, 9),
            candidates: Vec::new(),
            twin_edge_keys: Vec::new(),
            local_face_is_unbounded: true,
        }];
        graph.global_face_id_plans = vec![
            PartitionBorderGlobalFaceIdPlan {
                candidate_global_face_id: Some(0),
                component_index: 0,
                boundary_observation_ids: Vec::new(),
                face_refs: vec![face(1, 4, 9)],
                local_unbounded_face_count: 1,
                closed: true,
            },
            PartitionBorderGlobalFaceIdPlan {
                candidate_global_face_id: Some(1),
                component_index: 0,
                boundary_observation_ids: Vec::new(),
                face_refs: vec![face(1, 4, 10)],
                local_unbounded_face_count: 0,
                closed: true,
            },
        ];
        let stats = graph
            .validate_global_unbounded_face_application_with_evidence(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalUnboundedFaceProofStats {
                    face_count: 4,
                    local_unbounded_face_count: 1,
                    unbounded_component_count: 1,
                    closed_unbounded_face_count: 1,
                    unbounded_face_twin_count: 0,
                    unbounded_face_unmapped_twin_count: 0,
                    unbounded_face_not_ready_twin_count: 0,
                    candidate_count: 1,
                    proof_ready: true,
                },
                PartitionBorderGlobalFaceIdApplicationStats {
                    component_count: 1,
                    candidate_cycle_count: 2,
                    assigned_face_count: 2,
                    candidate_cycle_start_count: 2,
                    mapped_cycle_count: 1,
                    unmapped_plan_count: 0,
                    duplicate_face_id_count: 0,
                    non_contiguous_face_id_count: 0,
                    application_ready: true,
                },
            )
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalUnboundedFaceApplicationStats {
                face_count: 4,
                candidate_cycle_count: 2,
                local_unbounded_face_count: 1,
                candidate_unbounded_face_id_count: 1,
                mapped_unbounded_cycle_count: 1,
                missing_unbounded_face_id_count: 0,
                duplicate_unbounded_face_id_count: 0,
                proof_ready: true,
                application_ready: true,
            }
        );

        graph.global_face_id_plans[0].candidate_global_face_id = None;
        let stats = graph
            .validate_global_unbounded_face_application_with_evidence(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalUnboundedFaceProofStats {
                    face_count: 4,
                    local_unbounded_face_count: 1,
                    unbounded_component_count: 1,
                    closed_unbounded_face_count: 1,
                    candidate_count: 1,
                    proof_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdApplicationStats {
                    candidate_cycle_count: 2,
                    assigned_face_count: 2,
                    candidate_cycle_start_count: 2,
                    application_ready: false,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(stats.missing_unbounded_face_id_count, 1);
        assert!(!stats.application_ready);
    }

    #[test]
    fn global_unbounded_face_application_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .validate_global_unbounded_face_application_with_evidence(
                &ExecutionPolicy {
                    max_graph_nodes: Some(0),
                    ..Default::default()
                },
                PartitionBorderGlobalUnboundedFaceProofStats::default(),
                PartitionBorderGlobalFaceIdApplicationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 1,
            } if stage == "partition_border_global_unbounded_face_application_components"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .validate_global_unbounded_face_application_with_evidence(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                PartitionBorderGlobalUnboundedFaceProofStats::default(),
                PartitionBorderGlobalFaceIdApplicationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_unbounded_face_application"
        ));
    }

    #[test]
    fn global_topology_mutation_gate_combines_evidence_without_mutation() {
        let mut graph = exact_global_topology_candidate_graph();
        graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let before = graph.clone();
        let stats = graph
            .validate_global_topology_mutation_gate_with_evidence(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalTopologyApplicationGateStats {
                    application_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalComponentCoverageStats {
                    coverage_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdApplicationStats {
                    application_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalUnboundedFaceApplicationStats {
                    application_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceWalkInvariantStats {
                    face_count: 2,
                    closed_face_count: 2,
                    applied_twin_count: 2,
                    mapped_twin_count: 2,
                    source_complete_twin_count: 2,
                    unbounded_face_count: 1,
                    unbounded_component_count: 1,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceEulerWitnessStats {
                    boundary_euler_consistent: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(stats.edge_count, 4);
        assert_eq!(stats.component_count, 1);
        assert_eq!(stats.candidate_cycle_count, 2);
        assert!(stats.face_walk_ready);
        assert!(stats.euler_evidence_ready);
        assert!(stats.gate_ready);
        assert_eq!(graph, before);
    }

    #[test]
    fn global_topology_mutation_gate_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .validate_global_topology_mutation_gate_with_evidence(
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
                PartitionBorderGlobalTopologyApplicationGateStats::default(),
                PartitionBorderGlobalComponentCoverageStats::default(),
                PartitionBorderGlobalFaceIdApplicationStats::default(),
                PartitionBorderGlobalUnboundedFaceApplicationStats::default(),
                PartitionBorderGlobalFaceWalkInvariantStats::default(),
                PartitionBorderGlobalFaceEulerWitnessStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 4,
            } if stage == "partition_border_global_topology_mutation_gate_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .validate_global_topology_mutation_gate_with_evidence(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                PartitionBorderGlobalTopologyApplicationGateStats::default(),
                PartitionBorderGlobalComponentCoverageStats::default(),
                PartitionBorderGlobalFaceIdApplicationStats::default(),
                PartitionBorderGlobalUnboundedFaceApplicationStats::default(),
                PartitionBorderGlobalFaceWalkInvariantStats::default(),
                PartitionBorderGlobalFaceEulerWitnessStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_topology_mutation_gate"
        ));
    }

    #[test]
    fn global_topology_mutation_commits_only_after_the_gate() {
        let mut graph = exact_global_topology_candidate_graph();
        let candidate = graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let not_ready = graph
            .apply_global_topology_candidate_with_gate(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalTopologyMutationGateStats::default(),
                candidate,
            )
            .unwrap();
        assert!(!not_ready.applied);
        assert!(graph.global_next_global_dir_edge_ids().is_empty());

        let candidate = graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let applied = graph
            .apply_global_topology_candidate_with_gate(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalTopologyMutationGateStats {
                    gate_ready: true,
                    ..Default::default()
                },
                candidate,
            )
            .unwrap();
        assert!(applied.applied);
        assert_eq!(applied.applied_next_count, 4);
        assert_eq!(graph.global_next_global_dir_edge_ids().len(), 4);
        assert!(graph
            .global_next_global_dir_edge_ids()
            .iter()
            .all(Option::is_some));
    }

    #[test]
    fn global_face_id_mutation_maps_closed_cycles_without_local_writes() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: None,
                to_global_node_id: None,
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(1), Some(0)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        graph.global_face_id_plans = vec![PartitionBorderGlobalFaceIdPlan {
            candidate_global_face_id: Some(0),
            component_index: 0,
            boundary_observation_ids: observations
                .iter()
                .map(PartitionBorderHalfEdge::observation_id)
                .collect(),
            face_refs: Vec::new(),
            local_unbounded_face_count: 1,
            closed: true,
        }];
        let observations_before = graph.observations.clone();
        let stats = graph
            .apply_global_face_ids_with_evidence(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalTopologyMutationStats {
                    applied: true,
                    mutation_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdApplicationStats {
                    application_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalUnboundedFaceApplicationStats {
                    application_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(stats.candidate_cycle_count, 1);
        assert_eq!(stats.applied_face_id_count, 1);
        assert_eq!(stats.unbounded_face_id_count, 1);
        assert!(stats.mutation_ready);
        assert!(stats.applied);
        assert_eq!(graph.observations, observations_before);
        assert_eq!(graph.global_face_id_by_cycle_start.len(), 1);
        assert!(graph
            .global_face_id_by_cycle_start
            .iter()
            .all(Option::is_some));
        let unbounded_stats = graph
            .apply_global_unbounded_face_with_evidence(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalTopologyMutationStats {
                    applied: true,
                    mutation_ready: true,
                    ..Default::default()
                },
                stats,
                PartitionBorderGlobalUnboundedFaceApplicationStats {
                    candidate_unbounded_face_id_count: 1,
                    application_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(unbounded_stats.candidate_cycle_count, 1);
        assert_eq!(unbounded_stats.candidate_unbounded_face_id_count, 1);
        assert_eq!(unbounded_stats.applied_unbounded_face_id, Some(0));
        assert_eq!(
            unbounded_stats.applied_cycle_start_global_dir_edge_id,
            Some(0)
        );
        assert!(unbounded_stats.mutation_ready);
        assert!(unbounded_stats.applied);
        assert_eq!(graph.global_unbounded_face_id_by_cycle_start, Some((0, 0)));
    }

    #[test]
    fn global_face_id_mutation_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .apply_global_face_ids_with_evidence(
                &ExecutionPolicy {
                    max_graph_nodes: Some(0),
                    ..Default::default()
                },
                PartitionBorderGlobalTopologyMutationStats::default(),
                PartitionBorderGlobalFaceIdApplicationStats::default(),
                PartitionBorderGlobalUnboundedFaceApplicationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 2,
            } if stage == "partition_border_global_face_id_mutation_cycles"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .apply_global_face_ids_with_evidence(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                PartitionBorderGlobalTopologyMutationStats::default(),
                PartitionBorderGlobalFaceIdApplicationStats::default(),
                PartitionBorderGlobalUnboundedFaceApplicationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_id_mutation"
        ));
    }

    #[test]
    fn global_face_identity_materialization_maps_each_detached_edge_without_local_writes() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: None,
                to_global_node_id: None,
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(1), Some(0)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        graph.global_face_id_by_cycle_start = vec![Some(0)];
        graph.global_unbounded_face_id_by_cycle_start = Some((0, 0));
        let observations_before = graph.observations.clone();
        let stats = graph
            .materialize_global_face_identity(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFaceIdentityMaterializationStats {
                edge_count: 2,
                cycle_count: 1,
                assigned_edge_count: 2,
                unbounded_edge_count: 2,
                materialization_ready: true,
                ..Default::default()
            }
        );
        assert_eq!(
            graph.global_face_id_by_global_dir_edge_id(),
            &[Some(0), Some(0)]
        );
        assert_eq!(graph.observations, observations_before);

        graph.global_face_id_by_cycle_start.clear();
        graph.global_face_id_by_global_dir_edge_id.clear();
        let stats = graph
            .materialize_global_face_identity(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(stats.missing_face_id_count, 1);
        assert!(!stats.materialization_ready);
        assert!(graph.global_face_id_by_global_dir_edge_id().is_empty());
    }

    #[test]
    fn global_face_identity_materialization_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited.global_face_id_by_cycle_start = vec![Some(0), Some(1)];
        limited.global_unbounded_face_id_by_cycle_start = Some((0, 0));
        let error = limited
            .materialize_global_face_identity(&ExecutionPolicy {
                max_graph_edges: Some(0),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 4,
            } if stage == "partition_border_global_face_identity_materialization_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled.global_face_id_by_cycle_start = vec![Some(0), Some(1)];
        cancelled.global_unbounded_face_id_by_cycle_start = Some((0, 0));
        let error = cancelled
            .materialize_global_face_identity(&ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_identity_materialization"
        ));
    }

    #[test]
    fn global_face_identity_invariants_cross_check_detached_evidence() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: None,
                to_global_node_id: None,
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(1), Some(0)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        graph.global_face_id_by_cycle_start = vec![Some(0)];
        graph.global_face_id_by_global_dir_edge_id = vec![Some(0), Some(0)];
        let stats = graph
            .validate_global_face_identity_invariants(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalFaceIdentityMaterializationStats {
                    edge_count: 2,
                    cycle_count: 1,
                    assigned_edge_count: 2,
                    unbounded_edge_count: 2,
                    materialization_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceWalkInvariantStats {
                    face_count: 1,
                    closed_face_count: 1,
                    applied_twin_count: 1,
                    mapped_twin_count: 1,
                    source_complete_twin_count: 1,
                    unbounded_face_count: 1,
                    unbounded_component_count: 1,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceEulerWitnessStats {
                    boundary_euler_consistent: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(stats.invariants_ready);
        assert_eq!(stats.mapped_face_id_edge_count, 2);
        assert_eq!(stats.twin_count, 1);
        assert_eq!(stats.twin_mapping_mismatch_count, 0);
        assert_eq!(
            graph.global_face_id_by_global_dir_edge_id,
            vec![Some(0), Some(0)]
        );

        graph.global_face_id_by_global_dir_edge_id = vec![Some(0), Some(1)];
        let stats = graph
            .validate_global_face_identity_invariants(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalFaceIdentityMaterializationStats {
                    edge_count: 2,
                    cycle_count: 1,
                    assigned_edge_count: 2,
                    unbounded_edge_count: 2,
                    materialization_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceWalkInvariantStats {
                    face_count: 1,
                    closed_face_count: 1,
                    applied_twin_count: 1,
                    mapped_twin_count: 1,
                    source_complete_twin_count: 1,
                    unbounded_face_count: 1,
                    unbounded_component_count: 1,
                    ..Default::default()
                },
                PartitionBorderGlobalFaceEulerWitnessStats {
                    boundary_euler_consistent: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!stats.invariants_ready);
        assert!(stats.cycle_face_mismatch_count > 0);
        assert!(stats.successor_discontinuity_count > 0);
    }

    #[test]
    fn global_face_identity_invariants_are_bounded_and_cancellable() {
        let mut graph = exact_global_topology_candidate_graph();
        graph.global_face_id_by_cycle_start = vec![Some(0), Some(1)];
        graph.global_face_id_by_global_dir_edge_id = vec![Some(0); 4];
        let error = graph
            .validate_global_face_identity_invariants(
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdentityMaterializationStats::default(),
                PartitionBorderGlobalFaceWalkInvariantStats::default(),
                PartitionBorderGlobalFaceEulerWitnessStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 4,
            } if stage == "partition_border_global_face_identity_invariants_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .validate_global_face_identity_invariants(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdentityMaterializationStats::default(),
                PartitionBorderGlobalFaceWalkInvariantStats::default(),
                PartitionBorderGlobalFaceEulerWitnessStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_identity_invariants"
        ));
    }

    #[test]
    fn global_next_lineage_integration_matches_overrides_and_face_twins() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: Some(edge_index),
                to_global_node_id: Some(1 - edge_index),
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(1), Some(0)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        graph.global_next_global_dir_edge_ids = vec![Some(1), Some(0)];
        graph.global_face_next_application_plans =
            vec![PartitionBorderGlobalFaceNextApplicationPlan {
                component_index: 0,
                global_dir_edge_ids: vec![0, 1],
                successor_global_dir_edge_ids: vec![1, 0],
                closed: true,
                node_continuous: true,
            }];
        graph.global_face_twin_transitions = vec![PartitionBorderGlobalFaceTwinTransition {
            edge_key: observations[0].edge_key,
            forward_face_ref: observations[0].face_ref.unwrap(),
            reverse_face_ref: observations[1].face_ref.unwrap(),
            forward_observation_id: observations[0].observation_id(),
            reverse_observation_id: observations[1].observation_id(),
            forward_cycle_index: 0,
            reverse_cycle_index: 0,
            forward_cycle_closed: true,
            reverse_cycle_closed: true,
        }];
        let before = graph.clone();
        let stats = graph
            .validate_global_next_lineage_integration(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalFaceIdentityInvariantStats {
                    invariants_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalNextLineageIntegrationStats {
                edge_count: 2,
                cycle_count: 1,
                override_count: 2,
                integrated_successor_count: 2,
                application_plan_link_count: 2,
                committed_next_edge_count: 2,
                twin_count: 1,
                identity_ready: true,
                integration_ready: true,
                ..Default::default()
            }
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn global_next_lineage_integration_rejects_successor_drift() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: Some(edge_index),
                to_global_node_id: Some(1 - edge_index),
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(0), Some(1)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        graph.global_next_global_dir_edge_ids = vec![Some(0), Some(1)];
        graph.global_face_next_application_plans =
            vec![PartitionBorderGlobalFaceNextApplicationPlan {
                component_index: 0,
                global_dir_edge_ids: vec![0, 1],
                successor_global_dir_edge_ids: vec![1, 0],
                closed: true,
                node_continuous: true,
            }];
        let stats = graph
            .validate_global_next_lineage_integration(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalFaceIdentityInvariantStats {
                    invariants_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!stats.integration_ready);
        assert_eq!(stats.override_lineage_mismatch_count, 2);
        assert_eq!(stats.committed_next_mismatch_count, 0);
    }

    #[test]
    fn global_next_lineage_integration_is_bounded_and_cancellable() {
        let mut graph = exact_global_topology_candidate_graph();
        graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        graph.global_next_global_dir_edge_ids = graph
            .global_topology_candidate()
            .unwrap()
            .next_global_dir_edge_ids
            .clone();
        let error = graph
            .validate_global_next_lineage_integration(
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdentityInvariantStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 4,
            } if stage == "partition_border_global_next_lineage_integration_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .validate_global_next_lineage_integration(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdentityInvariantStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_next_lineage_integration"
        ));
    }

    #[test]
    fn global_cycle_face_lineage_maps_cycle_and_face_payload_without_mutation() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: Some(edge_index),
                to_global_node_id: Some(1 - edge_index),
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(1), Some(0)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        graph.global_face_id_by_cycle_start = vec![Some(0)];
        graph.global_face_id_plans = vec![PartitionBorderGlobalFaceIdPlan {
            candidate_global_face_id: Some(0),
            component_index: 0,
            boundary_observation_ids: observations
                .iter()
                .map(PartitionBorderHalfEdge::observation_id)
                .collect(),
            face_refs: observations
                .iter()
                .filter_map(|observation| observation.face_ref)
                .collect(),
            local_unbounded_face_count: 0,
            closed: true,
        }];
        let before = graph.clone();
        let stats = graph
            .validate_global_cycle_face_lineage(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalFaceIdentityInvariantStats {
                    invariants_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalNextLineageIntegrationStats {
                    integration_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalCycleFaceLineageStats {
                edge_count: 2,
                cycle_count: 1,
                plan_count: 1,
                closed_cycle_count: 1,
                mapped_cycle_count: 1,
                identity_ready: true,
                next_lineage_ready: true,
                lineage_ready: true,
                ..Default::default()
            }
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn global_cycle_face_lineage_rejects_face_and_observation_drift() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: Some(edge_index),
                to_global_node_id: Some(1 - edge_index),
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(1), Some(0)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        graph.global_face_id_by_cycle_start = vec![Some(0)];
        graph.global_face_id_plans = vec![PartitionBorderGlobalFaceIdPlan {
            candidate_global_face_id: Some(0),
            component_index: 0,
            boundary_observation_ids: vec![observations[0].observation_id()],
            face_refs: vec![observations[0].face_ref.unwrap()],
            local_unbounded_face_count: 0,
            closed: true,
        }];
        let stats = graph
            .validate_global_cycle_face_lineage(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalFaceIdentityInvariantStats {
                    invariants_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalNextLineageIntegrationStats {
                    integration_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!stats.lineage_ready);
        assert_eq!(stats.cycle_plan_mismatch_count, 1);
        assert_eq!(stats.cycle_face_ref_mismatch_count, 1);
    }

    #[test]
    fn global_cycle_face_lineage_is_bounded_and_cancellable() {
        let mut graph = exact_face_twin_graph();
        let observations = graph.observations.values().cloned().collect::<Vec<_>>();
        graph.global_face_edge_map = observations
            .iter()
            .enumerate()
            .map(|(edge_index, observation)| PartitionBorderGlobalFaceEdge {
                global_dir_edge_id: edge_index,
                partition_id: observation.partition_id,
                component_id: observation.face_ref.unwrap().component_id,
                local_dir_edge_id: observation.local_dir_edge_id,
                symmetric_global_dir_edge_id: 1 - edge_index,
                local_face_successor_global_dir_edge_id: None,
                cross_border_twin_global_dir_edge_id: Some(1 - edge_index),
                from_global_node_id: None,
                to_global_node_id: None,
                from: observation.from,
                to: observation.to,
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                edge_key: observation.edge_key,
                face_ref: observation.face_ref,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                source_line_ids: observation.source_line_ids.clone(),
            })
            .collect();
        graph.global_topology_candidate = Some(PartitionBorderGlobalTopologyCandidate {
            next_global_dir_edge_ids: vec![Some(1), Some(0)],
            cycle_start_global_dir_edge_ids: vec![0],
        });
        let error = graph
            .validate_global_cycle_face_lineage(
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdentityInvariantStats::default(),
                PartitionBorderGlobalNextLineageIntegrationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 2,
            } if stage == "partition_border_global_cycle_face_lineage_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .validate_global_cycle_face_lineage(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                PartitionBorderGlobalFaceIdentityInvariantStats::default(),
                PartitionBorderGlobalNextLineageIntegrationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_cycle_face_lineage"
        ));
    }

    #[test]
    fn global_cycle_face_promotion_gate_requires_matching_detached_evidence() {
        let graph = exact_global_topology_candidate_graph();
        let before = graph.clone();
        let stats = graph
            .validate_global_cycle_face_promotion_gate(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalCycleFaceLineageStats {
                    edge_count: 4,
                    cycle_count: 1,
                    plan_count: 1,
                    lineage_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalComponentCoverageStats {
                    component_count: 1,
                    face_count: 2,
                    edge_count: 4,
                    face_edge_count: 4,
                    covered_face_edge_count: 4,
                    coverage_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalUnboundedFaceApplicationStats {
                    face_count: 2,
                    candidate_cycle_count: 1,
                    local_unbounded_face_count: 1,
                    candidate_unbounded_face_id_count: 1,
                    mapped_unbounded_cycle_count: 1,
                    proof_ready: true,
                    application_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalCycleFacePromotionGateStats {
                edge_count: 4,
                cycle_count: 1,
                plan_count: 1,
                component_count: 1,
                face_count: 2,
                covered_face_edge_count: 4,
                candidate_unbounded_face_id_count: 1,
                mapped_unbounded_cycle_count: 1,
                lineage_ready: true,
                component_coverage_ready: true,
                unbounded_face_application_ready: true,
                gate_ready: true,
                ..Default::default()
            }
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn global_cycle_face_promotion_gate_rejects_count_and_marker_drift() {
        let graph = exact_global_topology_candidate_graph();
        let stats = graph
            .validate_global_cycle_face_promotion_gate(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalCycleFaceLineageStats {
                    edge_count: 3,
                    cycle_count: 2,
                    plan_count: 1,
                    lineage_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalComponentCoverageStats {
                    component_count: 1,
                    face_count: 3,
                    edge_count: 3,
                    coverage_ready: true,
                    ..Default::default()
                },
                PartitionBorderGlobalUnboundedFaceApplicationStats {
                    face_count: 2,
                    candidate_cycle_count: 1,
                    candidate_unbounded_face_id_count: 2,
                    mapped_unbounded_cycle_count: 0,
                    application_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!stats.gate_ready);
        assert_eq!(stats.edge_count_mismatch_count, 2);
        assert_eq!(stats.cycle_count_mismatch_count, 1);
        assert_eq!(stats.plan_count_mismatch_count, 1);
        assert_eq!(stats.face_count_mismatch_count, 1);
        assert_eq!(stats.unbounded_marker_mismatch_count, 2);
    }

    #[test]
    fn global_cycle_face_promotion_gate_is_bounded_and_cancellable() {
        let graph = exact_global_topology_candidate_graph();
        let inputs = (
            PartitionBorderGlobalCycleFaceLineageStats {
                edge_count: 4,
                cycle_count: 1,
                plan_count: 1,
                lineage_ready: true,
                ..Default::default()
            },
            PartitionBorderGlobalComponentCoverageStats {
                component_count: 1,
                face_count: 2,
                edge_count: 4,
                covered_face_edge_count: 4,
                coverage_ready: true,
                ..Default::default()
            },
            PartitionBorderGlobalUnboundedFaceApplicationStats {
                face_count: 2,
                candidate_cycle_count: 1,
                local_unbounded_face_count: 1,
                candidate_unbounded_face_id_count: 1,
                mapped_unbounded_cycle_count: 1,
                proof_ready: true,
                application_ready: true,
                ..Default::default()
            },
        );
        let error = graph
            .validate_global_cycle_face_promotion_gate(
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
                inputs.0,
                inputs.1,
                inputs.2,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 4,
            } if stage == "partition_border_global_cycle_face_promotion_gate_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .validate_global_cycle_face_promotion_gate(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                inputs.0,
                inputs.1,
                inputs.2,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_cycle_face_promotion_gate"
        ));
    }

    fn prepared_global_face_payload_lineage_graph() -> PartitionBorderGraph {
        let mut graph = exact_global_topology_candidate_graph();
        graph
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let candidate = graph.global_topology_candidate.clone().unwrap();
        graph.global_face_id_by_cycle_start = (0..candidate.cycle_start_global_dir_edge_ids.len())
            .map(Some)
            .collect();
        graph.global_face_id_plans = candidate
            .cycle_start_global_dir_edge_ids
            .iter()
            .enumerate()
            .map(|(face_id, &start)| {
                let mut boundary_observation_ids = Vec::new();
                let mut face_refs = BTreeSet::new();
                let mut local_unbounded_face_count = 0;
                let mut current = start;
                loop {
                    let edge = &graph.global_face_edge_map[current];
                    boundary_observation_ids.push(PartitionBorderObservationId {
                        partition_id: edge.partition_id,
                        local_dir_edge_id: edge.local_dir_edge_id,
                        edge_key: edge.edge_key,
                    });
                    if let Some(face_ref) = edge.face_ref {
                        face_refs.insert(face_ref);
                    }
                    local_unbounded_face_count += usize::from(edge.local_face_is_unbounded);
                    current = candidate.next_global_dir_edge_ids[current].unwrap();
                    if current == start {
                        break;
                    }
                }
                PartitionBorderGlobalFaceIdPlan {
                    candidate_global_face_id: Some(face_id),
                    component_index: 0,
                    boundary_observation_ids,
                    face_refs: face_refs.into_iter().collect(),
                    local_unbounded_face_count,
                    closed: true,
                }
            })
            .collect();
        graph
    }

    #[test]
    fn global_face_payload_lineage_preserves_source_z_face_and_node_payloads() {
        let graph = prepared_global_face_payload_lineage_graph();
        let before = graph.clone();
        let stats = graph
            .validate_global_face_payload_lineage(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalCycleFacePromotionGateStats {
                    edge_count: 4,
                    cycle_count: 2,
                    plan_count: 2,
                    gate_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            stats,
            PartitionBorderGlobalFacePayloadLineageStats {
                edge_count: 4,
                cycle_count: 2,
                plan_count: 2,
                checked_edge_count: 4,
                checked_cycle_count: 2,
                lineage_ready: true,
                ..Default::default()
            }
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn global_face_payload_lineage_rejects_provenance_z_and_node_drift() {
        let mut graph = prepared_global_face_payload_lineage_graph();
        graph.global_face_edge_map[0].source_line_ids.push(99);
        graph.global_face_edge_map[0].from_z_bits = 77;
        graph.global_face_nodes[0].observation_ids.clear();
        let stats = graph
            .validate_global_face_payload_lineage(
                &ExecutionPolicy::default(),
                PartitionBorderGlobalCycleFacePromotionGateStats {
                    edge_count: 4,
                    cycle_count: 2,
                    plan_count: 2,
                    gate_ready: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!stats.lineage_ready);
        assert_eq!(stats.source_lineage_mismatch_count, 1);
        assert_eq!(stats.z_lineage_mismatch_count, 1);
        assert!(stats.node_lineage_mismatch_count > 0);
    }

    #[test]
    fn global_face_payload_lineage_is_bounded_and_cancellable() {
        let graph = prepared_global_face_payload_lineage_graph();
        let gate = PartitionBorderGlobalCycleFacePromotionGateStats {
            edge_count: 4,
            cycle_count: 2,
            plan_count: 2,
            gate_ready: true,
            ..Default::default()
        };
        let error = graph
            .validate_global_face_payload_lineage(
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
                gate,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 4,
            } if stage == "partition_border_global_face_payload_lineage_edges"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let error = graph
            .validate_global_face_payload_lineage(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                gate,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_face_payload_lineage"
        ));
    }

    #[test]
    fn global_unbounded_face_mutation_is_bounded_and_cancellable() {
        let mut limited = exact_global_topology_candidate_graph();
        limited
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = limited
            .apply_global_unbounded_face_with_evidence(
                &ExecutionPolicy {
                    max_graph_nodes: Some(0),
                    ..Default::default()
                },
                PartitionBorderGlobalTopologyMutationStats::default(),
                PartitionBorderGlobalFaceIdMutationStats::default(),
                PartitionBorderGlobalUnboundedFaceApplicationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 2,
            } if stage == "partition_border_global_unbounded_face_mutation_cycles"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let mut cancelled = exact_global_topology_candidate_graph();
        cancelled
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let error = cancelled
            .apply_global_unbounded_face_with_evidence(
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                },
                PartitionBorderGlobalTopologyMutationStats::default(),
                PartitionBorderGlobalFaceIdMutationStats::default(),
                PartitionBorderGlobalUnboundedFaceApplicationStats::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            crate::PolygonizeError::Cancelled { ref stage }
                if stage == "partition_border_global_unbounded_face_mutation"
        ));
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
