use crate::diagnostics::ExecutionWorkTracker;
use crate::graph::partition_border::{
    PartitionBorderAdjacency, PartitionBorderGraph, PartitionBorderHalfEdge,
    PartitionBorderLocalFaceGraph, PartitionBorderObservationId, PartitionBorderSide,
};
use crate::index::{IndexedEnvelope, RStarBackend};
use crate::noding::hot_pixel::HotPixelNoder;
use crate::noding::snap::SnapNoder;
use crate::options::{DedupPolicy, ExecutionPolicy, NodingGuarantee, TileOwnershipPolicy};
use crate::polygonizer::{apply_determinism, canonicalize_ring, PartitionNodedSegment};
use crate::trace::{
    TopologyTraceV1, TraceByteLimitsV1, TraceCaptureBudget, TraceLevelV1, TraceRecorderV1,
    TraceStageV1,
};
use crate::types::{Coord3D, Line3D, Polygon3D, PolygonProvenance};
use crate::utils::canonical_coordinate_bits;
use crate::{PolygonizeError, Polygonizer, PolygonizerOptions, PolygonizerResult, Result};
use geo::algorithm::line_intersection::line_intersection;
use geo::bounding_rect::BoundingRect;
use geo::intersects::Intersects;
use geo::InteriorPoint;
use geo_types::{Coord, Geometry, Line, LineString, Point, Rect};
#[cfg(feature = "parallel")]
use rayon::{prelude::*, ThreadPoolBuilder};
use rstar::AABB;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use thiserror::Error;

fn canonical_ring_key(ring: &[Coord3D]) -> Vec<[u64; 3]> {
    let key = |mut ring: Vec<Coord3D>| {
        canonicalize_ring(&mut ring, None);
        ring.into_iter()
            .map(|coord| {
                [
                    canonical_coordinate_bits(coord.x),
                    canonical_coordinate_bits(coord.y),
                    canonical_coordinate_bits(coord.z),
                ]
            })
            .collect::<Vec<_>>()
    };
    key(ring.to_vec()).min(key(ring.iter().rev().copied().collect()))
}

fn canonical_polygon_key(poly: &Polygon3D) -> (Vec<[u64; 3]>, Vec<Vec<[u64; 3]>>) {
    let mut interiors = poly
        .interiors
        .iter()
        .map(|ring| canonical_ring_key(ring))
        .collect::<Vec<_>>();
    interiors.sort_unstable();
    (canonical_ring_key(&poly.exterior), interiors)
}

type CanonicalPolygonOutputKey = (
    (Vec<[u64; 3]>, Vec<Vec<[u64; 3]>>),
    Vec<u32>,
    Option<(Vec<u64>, Option<String>)>,
);

const PARTITION_SNAPSHOT_V1_SCHEMA_VERSION: u32 = 11;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionSourceSegmentV1 {
    pub(crate) geometry_index: usize,
    pub(crate) segment_index: usize,
    pub(crate) start: crate::fingerprint::CoordinateFingerprintV1,
    pub(crate) end: crate::fingerprint::CoordinateFingerprintV1,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionNodedSegmentV1 {
    pub(crate) start: crate::fingerprint::CoordinateFingerprintV1,
    pub(crate) end: crate::fingerprint::CoordinateFingerprintV1,
    pub(crate) source_line_ids: Vec<u32>,
    pub(crate) representative_line_id: Option<u32>,
}

fn partition_source_segments(
    geometries: &[(&Geometry<f64>, Option<Rect<f64>>)],
    selected_input_geometry_indices: &[usize],
) -> Result<Vec<PartitionSourceSegmentV1>> {
    let mut segments = Vec::new();
    for &geometry_index in selected_input_geometry_indices {
        let geometry = geometries
            .get(geometry_index)
            .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                reason: "partition snapshot source geometry index is missing".to_string(),
            })?
            .0;
        for (segment_index, segment) in crate::polygonizer::extract_geometry_segments(geometry)
            .into_iter()
            .enumerate()
        {
            segments.push(PartitionSourceSegmentV1 {
                geometry_index,
                segment_index,
                start: crate::fingerprint::coordinate_fingerprint(segment.start)?,
                end: crate::fingerprint::coordinate_fingerprint(segment.end)?,
            });
        }
    }
    Ok(segments)
}

fn partition_noded_segments(
    segments: &[PartitionNodedSegment],
) -> Result<Vec<PartitionNodedSegmentV1>> {
    let mut noded_segments = segments
        .iter()
        .map(|segment| {
            let start_key = [
                canonical_coordinate_bits(segment.line.start.x),
                canonical_coordinate_bits(segment.line.start.y),
                canonical_coordinate_bits(segment.line.start.z),
            ];
            let end_key = [
                canonical_coordinate_bits(segment.line.end.x),
                canonical_coordinate_bits(segment.line.end.y),
                canonical_coordinate_bits(segment.line.end.z),
            ];
            let (start, end) = if start_key <= end_key {
                (segment.line.start, segment.line.end)
            } else {
                (segment.line.end, segment.line.start)
            };
            let mut source_line_ids = segment.source_line_ids.clone();
            source_line_ids.sort_unstable();
            source_line_ids.dedup();
            Ok(PartitionNodedSegmentV1 {
                start: crate::fingerprint::coordinate_fingerprint(start)?,
                end: crate::fingerprint::coordinate_fingerprint(end)?,
                representative_line_id: Some(segment.representative_line_id),
                source_line_ids,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    noded_segments.sort_unstable();
    noded_segments.dedup();
    Ok(noded_segments)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionAtomicObservationV1 {
    pub(crate) from_xy_bits: [u64; 2],
    pub(crate) to_xy_bits: [u64; 2],
    pub(crate) from_z_bits: u64,
    pub(crate) to_z_bits: u64,
    pub(crate) side: u8,
    pub(crate) component_id: usize,
    pub(crate) source_line_ids: Vec<u32>,
    pub(crate) representative_line_id: Option<u32>,
    pub(crate) face_ref: Option<[usize; 3]>,
    pub(crate) local_face_successor: Option<usize>,
    pub(crate) local_face_is_unbounded: bool,
    pub(crate) local_face_boundary_successor: Option<PartitionBorderObservationIdV1>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionBorderObservationIdV1 {
    pub(crate) partition_id: usize,
    pub(crate) local_dir_edge_id: usize,
    pub(crate) edge_start_xy_bits: [u64; 2],
    pub(crate) edge_end_xy_bits: [u64; 2],
}

fn partition_border_observation_id(
    observation_id: PartitionBorderObservationId,
) -> PartitionBorderObservationIdV1 {
    let (start, end) = observation_id.edge_key.endpoints();
    PartitionBorderObservationIdV1 {
        partition_id: observation_id.partition_id,
        local_dir_edge_id: observation_id.local_dir_edge_id,
        edge_start_xy_bits: start.xy_bits(),
        edge_end_xy_bits: end.xy_bits(),
    }
}

fn partition_border_side_code(side: PartitionBorderSide) -> u8 {
    match side {
        PartitionBorderSide::MinX => 0,
        PartitionBorderSide::MaxX => 1,
        PartitionBorderSide::MinY => 2,
        PartitionBorderSide::MaxY => 3,
    }
}

fn partition_atomic_observations(
    border_observations: &[PartitionBorderHalfEdge],
) -> Vec<PartitionAtomicObservationV1> {
    let mut observations = border_observations
        .iter()
        .map(|observation| {
            let mut source_line_ids = observation.source_line_ids.clone();
            source_line_ids.sort_unstable();
            source_line_ids.dedup();
            PartitionAtomicObservationV1 {
                from_xy_bits: observation.from.xy_bits(),
                to_xy_bits: observation.to.xy_bits(),
                from_z_bits: observation.from_z_bits,
                to_z_bits: observation.to_z_bits,
                side: partition_border_side_code(observation.side),
                component_id: observation.component_id,
                source_line_ids,
                representative_line_id: observation.representative_line_id,
                face_ref: observation
                    .face_ref
                    .map(|face| [face.partition_id, face.component_id, face.face_id]),
                local_face_successor: observation.local_face_successor,
                local_face_is_unbounded: observation.local_face_is_unbounded,
                local_face_boundary_successor: observation
                    .local_face_boundary_successor
                    .map(partition_border_observation_id),
            }
        })
        .collect::<Vec<_>>();
    observations.sort_unstable();
    observations.dedup();
    observations
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct PartitionBoundaryNodingEvidenceV1 {
    pub(crate) added_node_count: usize,
    pub(crate) added_edge_count: usize,
    pub(crate) split_event_count: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionLocalFaceEdgeV1 {
    pub(crate) local_dir_edge_id: usize,
    pub(crate) symmetric_local_dir_edge_id: usize,
    pub(crate) local_face_successor: Option<usize>,
    pub(crate) from_xy_bits: [u64; 2],
    pub(crate) to_xy_bits: [u64; 2],
    pub(crate) from_z_bits: u64,
    pub(crate) to_z_bits: u64,
    pub(crate) face_ref: Option<[usize; 3]>,
    pub(crate) local_face_is_unbounded: bool,
    pub(crate) source_line_ids: Vec<u32>,
    pub(crate) representative_line_id: Option<u32>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionLocalNodeV1 {
    pub(crate) xy_bits: [u64; 2],
    pub(crate) z_bits: Vec<u64>,
    pub(crate) outgoing_local_dir_edge_ids: Vec<usize>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionLocalFaceGraphV1 {
    pub(crate) partition_id: usize,
    pub(crate) component_id: usize,
    pub(crate) nodes: Vec<PartitionLocalNodeV1>,
    pub(crate) directed_edges: Vec<PartitionLocalFaceEdgeV1>,
}

fn partition_local_nodes(edges: &[PartitionLocalFaceEdgeV1]) -> Vec<PartitionLocalNodeV1> {
    let mut nodes = BTreeMap::<[u64; 2], PartitionLocalNodeV1>::new();
    for edge in edges {
        let from = nodes
            .entry(edge.from_xy_bits)
            .or_insert_with(|| PartitionLocalNodeV1 {
                xy_bits: edge.from_xy_bits,
                z_bits: Vec::new(),
                outgoing_local_dir_edge_ids: Vec::new(),
            });
        from.z_bits.push(edge.from_z_bits);
        from.outgoing_local_dir_edge_ids
            .push(edge.local_dir_edge_id);

        let to = nodes
            .entry(edge.to_xy_bits)
            .or_insert_with(|| PartitionLocalNodeV1 {
                xy_bits: edge.to_xy_bits,
                z_bits: Vec::new(),
                outgoing_local_dir_edge_ids: Vec::new(),
            });
        to.z_bits.push(edge.to_z_bits);
    }
    for node in nodes.values_mut() {
        node.z_bits.sort_unstable();
        node.z_bits.dedup();
        node.outgoing_local_dir_edge_ids.sort_unstable();
        node.outgoing_local_dir_edge_ids.dedup();
    }
    nodes.into_values().collect()
}

fn partition_local_face_graphs(
    local_face_graphs: &[PartitionBorderLocalFaceGraph],
) -> Vec<PartitionLocalFaceGraphV1> {
    let mut graphs = local_face_graphs
        .iter()
        .map(|graph| {
            let mut directed_edges = graph
                .directed_edges
                .iter()
                .map(|edge| {
                    let mut source_line_ids = edge.source_line_ids.clone();
                    source_line_ids.sort_unstable();
                    source_line_ids.dedup();
                    PartitionLocalFaceEdgeV1 {
                        local_dir_edge_id: edge.local_dir_edge_id,
                        symmetric_local_dir_edge_id: edge.symmetric_local_dir_edge_id,
                        local_face_successor: edge.local_face_successor,
                        from_xy_bits: edge.from.xy_bits(),
                        to_xy_bits: edge.to.xy_bits(),
                        from_z_bits: edge.from_z_bits,
                        to_z_bits: edge.to_z_bits,
                        face_ref: edge
                            .face_ref
                            .map(|face| [face.partition_id, face.component_id, face.face_id]),
                        local_face_is_unbounded: edge.local_face_is_unbounded,
                        representative_line_id: source_line_ids.first().copied(),
                        source_line_ids,
                    }
                })
                .collect::<Vec<_>>();
            directed_edges.sort_unstable();
            directed_edges.dedup();
            PartitionLocalFaceGraphV1 {
                partition_id: graph.partition_id,
                component_id: graph.component_id,
                nodes: partition_local_nodes(&directed_edges),
                directed_edges,
            }
        })
        .collect::<Vec<_>>();
    graphs.sort_unstable();
    graphs.dedup();
    graphs
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionBoundaryNodeV1 {
    pub(crate) xy_bits: [u64; 2],
    pub(crate) z_bits: Vec<u64>,
    pub(crate) source_line_ids: Vec<u32>,
    pub(crate) representative_line_ids: Vec<u32>,
    pub(crate) face_refs: Vec<[usize; 3]>,
    pub(crate) incident_observation_count: usize,
}

fn partition_boundary_nodes(
    border_observations: &[PartitionBorderHalfEdge],
) -> Vec<PartitionBoundaryNodeV1> {
    let mut nodes = BTreeMap::<[u64; 2], PartitionBoundaryNodeV1>::new();
    for observation in border_observations {
        for (key, z_bits) in [
            (observation.from.xy_bits(), observation.from_z_bits),
            (observation.to.xy_bits(), observation.to_z_bits),
        ] {
            let node = nodes.entry(key).or_insert_with(|| PartitionBoundaryNodeV1 {
                xy_bits: key,
                z_bits: Vec::new(),
                source_line_ids: Vec::new(),
                representative_line_ids: Vec::new(),
                face_refs: Vec::new(),
                incident_observation_count: 0,
            });
            node.z_bits.push(z_bits);
            node.source_line_ids
                .extend(observation.source_line_ids.iter().copied());
            if let Some(representative_line_id) = observation.representative_line_id {
                node.representative_line_ids.push(representative_line_id);
            }
            if let Some(face_ref) = observation.face_ref {
                node.face_refs.push([
                    face_ref.partition_id,
                    face_ref.component_id,
                    face_ref.face_id,
                ]);
            }
            node.incident_observation_count += 1;
        }
    }
    let mut nodes = nodes.into_values().collect::<Vec<_>>();
    for node in &mut nodes {
        node.z_bits.sort_unstable();
        node.z_bits.dedup();
        node.source_line_ids.sort_unstable();
        node.source_line_ids.dedup();
        node.representative_line_ids.sort_unstable();
        node.representative_line_ids.dedup();
        node.face_refs.sort_unstable();
        node.face_refs.dedup();
    }
    nodes
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct PartitionNonPolygonEvidenceV1 {
    pub(crate) dangles: Vec<Vec<[u64; 3]>>,
    pub(crate) cut_edges: Vec<Vec<[u64; 3]>>,
    pub(crate) invalid_rings: Vec<Vec<[u64; 3]>>,
}

fn partition_non_polygon_evidence(result: &PolygonizerResult) -> PartitionNonPolygonEvidenceV1 {
    let mut dangles = result
        .dangles
        .iter()
        .map(|line| canonical_open_line_key(line))
        .collect::<Vec<_>>();
    let mut cut_edges = result
        .cut_edges
        .iter()
        .map(|line| canonical_open_line_key(line))
        .collect::<Vec<_>>();
    let mut invalid_rings = result
        .invalid_rings
        .iter()
        .map(|ring| canonical_ring_key(ring))
        .collect::<Vec<_>>();
    dangles.sort_unstable();
    cut_edges.sort_unstable();
    invalid_rings.sort_unstable();
    PartitionNonPolygonEvidenceV1 {
        dangles,
        cut_edges,
        invalid_rings,
    }
}

fn partition_snapshot_diff_field<T: Serialize>(
    path: &str,
    expected: &T,
    actual: &T,
) -> crate::fingerprint::FingerprintDiffV1 {
    crate::fingerprint::FingerprintDiffV1 {
        path: path.to_string(),
        expected: serde_json::to_value(expected).expect("snapshot field serializes"),
        actual: serde_json::to_value(actual).expect("snapshot field serializes"),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct PartitionSnapshotV1 {
    pub(crate) schema_version: u32,
    pub(crate) partition_id: usize,
    pub(crate) tile_min: crate::fingerprint::CoordinateFingerprintV1,
    pub(crate) tile_max: crate::fingerprint::CoordinateFingerprintV1,
    pub(crate) selected_input_geometry_indices: Vec<usize>,
    pub(crate) selected_source_segments: Vec<PartitionSourceSegmentV1>,
    pub(crate) local_noded_segments: Vec<PartitionNodedSegmentV1>,
    pub(crate) boundary_noded_segments: Vec<PartitionNodedSegmentV1>,
    pub(crate) boundary_noding: PartitionBoundaryNodingEvidenceV1,
    pub(crate) atomic_observations: Vec<PartitionAtomicObservationV1>,
    pub(crate) local_face_graphs: Vec<PartitionLocalFaceGraphV1>,
    pub(crate) boundary_nodes: Vec<PartitionBoundaryNodeV1>,
    pub(crate) non_polygon: PartitionNonPolygonEvidenceV1,
    pub(crate) topology: crate::fingerprint::TopologyFingerprintV1,
}

impl PartitionSnapshotV1 {
    #[allow(clippy::too_many_arguments)]
    fn from_result(
        partition_id: usize,
        tile_bbox: Rect<f64>,
        mut selected_input_geometry_indices: Vec<usize>,
        mut selected_source_segments: Vec<PartitionSourceSegmentV1>,
        boundary_noding_stats: crate::graph::planar_graph::PartitionBoundaryNodingStats,
        noded_segments: &[PartitionNodedSegment],
        boundary_noded_segments: &[PartitionNodedSegment],
        border_observations: &[PartitionBorderHalfEdge],
        local_face_graphs: &[PartitionBorderLocalFaceGraph],
        result: &PolygonizerResult,
        options: &PolygonizerOptions,
    ) -> Result<Self> {
        selected_input_geometry_indices.sort_unstable();
        selected_input_geometry_indices.dedup();
        selected_source_segments.sort_unstable();
        selected_source_segments.dedup();
        let coordinate = |coord: Coord<f64>| {
            crate::fingerprint::coordinate_fingerprint(Coord3D::new(coord.x, coord.y, 0.0))
        };
        Ok(Self {
            schema_version: PARTITION_SNAPSHOT_V1_SCHEMA_VERSION,
            partition_id,
            tile_min: coordinate(tile_bbox.min())?,
            tile_max: coordinate(tile_bbox.max())?,
            selected_input_geometry_indices,
            selected_source_segments,
            local_noded_segments: partition_noded_segments(noded_segments)?,
            boundary_noded_segments: partition_noded_segments(boundary_noded_segments)?,
            boundary_noding: PartitionBoundaryNodingEvidenceV1 {
                added_node_count: boundary_noding_stats.added_node_count,
                added_edge_count: boundary_noding_stats.added_edge_count,
                split_event_count: boundary_noding_stats.split_event_count,
            },
            atomic_observations: partition_atomic_observations(border_observations),
            local_face_graphs: partition_local_face_graphs(local_face_graphs),
            boundary_nodes: partition_boundary_nodes(border_observations),
            non_polygon: partition_non_polygon_evidence(result),
            topology: crate::fingerprint::TopologyFingerprintV1::try_from_result(result, options)?,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn fingerprint_sha256(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self).expect("snapshot serializes"))
        )
    }

    #[allow(dead_code)]
    pub(crate) fn diff(&self, actual: &Self) -> Option<crate::fingerprint::FingerprintDiffV1> {
        if self.schema_version != actual.schema_version {
            return Some(partition_snapshot_diff_field(
                "$.schema_version",
                &self.schema_version,
                &actual.schema_version,
            ));
        }
        if self.partition_id != actual.partition_id {
            return Some(partition_snapshot_diff_field(
                "$.partition_id",
                &self.partition_id,
                &actual.partition_id,
            ));
        }
        if self.tile_min != actual.tile_min {
            return Some(partition_snapshot_diff_field(
                "$.tile_min",
                &self.tile_min,
                &actual.tile_min,
            ));
        }
        if self.tile_max != actual.tile_max {
            return Some(partition_snapshot_diff_field(
                "$.tile_max",
                &self.tile_max,
                &actual.tile_max,
            ));
        }
        if self.selected_input_geometry_indices != actual.selected_input_geometry_indices {
            return Some(partition_snapshot_diff_field(
                "$.selected_input_geometry_indices",
                &self.selected_input_geometry_indices,
                &actual.selected_input_geometry_indices,
            ));
        }
        if self.selected_source_segments != actual.selected_source_segments {
            return Some(partition_snapshot_diff_field(
                "$.selected_source_segments",
                &self.selected_source_segments,
                &actual.selected_source_segments,
            ));
        }
        if self.local_noded_segments != actual.local_noded_segments {
            return Some(partition_snapshot_diff_field(
                "$.local_noded_segments",
                &self.local_noded_segments,
                &actual.local_noded_segments,
            ));
        }
        if self.boundary_noded_segments != actual.boundary_noded_segments {
            return Some(partition_snapshot_diff_field(
                "$.boundary_noded_segments",
                &self.boundary_noded_segments,
                &actual.boundary_noded_segments,
            ));
        }
        if self.boundary_noding.added_node_count != actual.boundary_noding.added_node_count {
            return Some(partition_snapshot_diff_field(
                "$.boundary_noding.added_node_count",
                &self.boundary_noding.added_node_count,
                &actual.boundary_noding.added_node_count,
            ));
        }
        if self.boundary_noding.added_edge_count != actual.boundary_noding.added_edge_count {
            return Some(partition_snapshot_diff_field(
                "$.boundary_noding.added_edge_count",
                &self.boundary_noding.added_edge_count,
                &actual.boundary_noding.added_edge_count,
            ));
        }
        if self.boundary_noding.split_event_count != actual.boundary_noding.split_event_count {
            return Some(partition_snapshot_diff_field(
                "$.boundary_noding.split_event_count",
                &self.boundary_noding.split_event_count,
                &actual.boundary_noding.split_event_count,
            ));
        }
        if self.atomic_observations != actual.atomic_observations {
            return Some(partition_snapshot_diff_field(
                "$.atomic_observations",
                &self.atomic_observations,
                &actual.atomic_observations,
            ));
        }
        if self.local_face_graphs != actual.local_face_graphs {
            return Some(partition_snapshot_diff_field(
                "$.local_face_graphs",
                &self.local_face_graphs,
                &actual.local_face_graphs,
            ));
        }
        if self.boundary_nodes != actual.boundary_nodes {
            return Some(partition_snapshot_diff_field(
                "$.boundary_nodes",
                &self.boundary_nodes,
                &actual.boundary_nodes,
            ));
        }
        if self.non_polygon != actual.non_polygon {
            return Some(partition_snapshot_diff_field(
                "$.non_polygon",
                &self.non_polygon,
                &actual.non_polygon,
            ));
        }
        self.topology.diff(&actual.topology).map(|mut diff| {
            diff.path = if diff.path == "$" {
                "$.topology".to_string()
            } else {
                format!("$.topology{}", &diff.path[1..])
            };
            diff
        })
    }
}

fn canonical_polygon_output_key(poly: &Polygon3D) -> CanonicalPolygonOutputKey {
    let mut source_line_ids = poly.boundary_source_line_ids.clone();
    source_line_ids.sort_unstable();
    source_line_ids.dedup();
    let provenance = poly.provenance.as_ref().map(|provenance| {
        let mut boundary_line_ids = provenance.boundary_line_ids.clone();
        boundary_line_ids.sort_unstable();
        boundary_line_ids.dedup();
        (boundary_line_ids, provenance.input_profile_id.clone())
    });
    (canonical_polygon_key(poly), source_line_ids, provenance)
}

fn canonical_open_line_key(line: &[Coord3D]) -> Vec<[u64; 3]> {
    let forward = line
        .iter()
        .map(|coord| {
            [
                canonical_coordinate_bits(coord.x),
                canonical_coordinate_bits(coord.y),
                canonical_coordinate_bits(coord.z),
            ]
        })
        .collect::<Vec<_>>();
    let mut reverse = forward.clone();
    reverse.reverse();
    forward.min(reverse)
}

fn canonical_polygon_output_keys(
    polygons: &[Polygon3D],
    execution_policy: &ExecutionPolicy,
) -> Result<Vec<CanonicalPolygonOutputKey>> {
    let mut keys = Vec::with_capacity(polygons.len());
    for (index, polygon) in polygons.iter().enumerate() {
        execution_policy.check_cancelled_every("tiled_untiled_equivalence_polygons", index)?;
        keys.push(canonical_polygon_output_key(polygon));
    }
    keys.sort_unstable();
    Ok(keys)
}

fn canonical_line_output_keys(
    lines: &[Vec<Coord3D>],
    closed: bool,
    execution_policy: &ExecutionPolicy,
    stage: &'static str,
) -> Result<Vec<Vec<[u64; 3]>>> {
    let mut keys = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        execution_policy.check_cancelled_every(stage, index)?;
        keys.push(if closed {
            canonical_ring_key(line)
        } else {
            canonical_open_line_key(line)
        });
    }
    keys.sort_unstable();
    Ok(keys)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TiledUntiledEquivalenceStats {
    checked: bool,
    ready: bool,
    mismatch_count: usize,
}

fn compare_stitched_output_with_untiled(
    stitched_output: Option<&TiledStitchedOutput>,
    geometries: &[(&Geometry<f64>, Option<Rect<f64>>)],
    options: &PolygonizerOptions,
    execution_policy: &ExecutionPolicy,
) -> Result<TiledUntiledEquivalenceStats> {
    let Some(stitched_output) = stitched_output else {
        return Ok(TiledUntiledEquivalenceStats::default());
    };
    execution_policy.check_cancelled("tiled_untiled_equivalence")?;
    let mut polygonizer =
        Polygonizer::with_options(options.clone()).with_execution_policy(execution_policy.clone());
    for (geometry, _) in geometries {
        polygonizer.add_borrowed_geometry(geometry);
    }
    let untiled = polygonizer.polygonize()?;
    let polygon_mismatch =
        canonical_polygon_output_keys(&stitched_output.polygons, execution_policy)?
            != canonical_polygon_output_keys(&untiled.polygons, execution_policy)?;
    let dangle_mismatch = canonical_line_output_keys(
        &stitched_output.dangles,
        false,
        execution_policy,
        "tiled_untiled_equivalence_dangles",
    )? != canonical_line_output_keys(
        &untiled.dangles,
        false,
        execution_policy,
        "tiled_untiled_equivalence_dangles",
    )?;
    let cut_edge_mismatch = canonical_line_output_keys(
        &stitched_output.cut_edges,
        false,
        execution_policy,
        "tiled_untiled_equivalence_cut_edges",
    )? != canonical_line_output_keys(
        &untiled.cut_edges,
        false,
        execution_policy,
        "tiled_untiled_equivalence_cut_edges",
    )?;
    let invalid_ring_mismatch = canonical_line_output_keys(
        &stitched_output.invalid_rings,
        true,
        execution_policy,
        "tiled_untiled_equivalence_invalid_rings",
    )? != canonical_line_output_keys(
        &untiled.invalid_rings,
        true,
        execution_policy,
        "tiled_untiled_equivalence_invalid_rings",
    )?;
    let mismatch_count = usize::from(polygon_mismatch)
        .saturating_add(usize::from(dangle_mismatch))
        .saturating_add(usize::from(cut_edge_mismatch))
        .saturating_add(usize::from(invalid_ring_mismatch));
    Ok(TiledUntiledEquivalenceStats {
        checked: true,
        ready: mismatch_count == 0,
        mismatch_count,
    })
}

fn coord3d_from_bits(bits: [u64; 3]) -> Coord3D {
    Coord3D::new(
        f64::from_bits(bits[0]),
        f64::from_bits(bits[1]),
        f64::from_bits(bits[2]),
    )
}

fn promote_global_private_extraction(
    partition_border_graph: &PartitionBorderGraph,
    options: &PolygonizerOptions,
    execution_policy: &ExecutionPolicy,
) -> Result<Option<TiledStitchedOutput>> {
    let Some(extraction) = partition_border_graph.global_private_extraction() else {
        return Ok(None);
    };
    execution_policy.check_cancelled("tiled_stitched_output")?;
    let mut polygons = Vec::with_capacity(extraction.ring_payloads.len());
    let mut output_polygon_count = 0;
    let mut output_coordinate_count = 0;
    for (payload_index, payload) in extraction.ring_payloads.iter().enumerate() {
        execution_policy.check_cancelled_every("tiled_stitched_output_polygons", payload_index)?;
        let exterior = payload
            .exterior_coords
            .iter()
            .copied()
            .map(coord3d_from_bits)
            .collect::<Vec<_>>();
        let interiors = payload
            .hole_coords
            .iter()
            .map(|hole| hole.iter().copied().map(coord3d_from_bits).collect())
            .collect::<Vec<Vec<_>>>();
        let mut polygon = Polygon3D::new(exterior, interiors, vec![], vec![]);
        polygon.set_boundary_source_line_ids(payload.source_line_ids.clone());
        if options.provenance.enabled {
            let boundary_line_ids = if options.provenance.include_boundary_line_ids {
                payload
                    .source_line_ids
                    .iter()
                    .copied()
                    .filter(|line_id| *line_id != 0)
                    .map(u64::from)
                    .collect()
            } else {
                Vec::new()
            };
            polygon.provenance = Some(PolygonProvenance {
                boundary_line_ids,
                input_profile_id: options.input_profile_id.clone(),
            });
        }
        let area = polygon.unsigned_area_2d();
        if area.is_finite()
            && area > 0.0
            && options
                .output_filter
                .minimum_face_area
                .is_none_or(|minimum| area >= minimum)
        {
            account_polygon_output(
                execution_policy,
                &mut output_polygon_count,
                &mut output_coordinate_count,
                &polygon,
            )?;
            polygons.push(polygon);
        }
    }

    let mut dangles = Vec::new();
    let mut cut_edges = Vec::new();
    let mut invalid_rings = Vec::new();
    if !options.extract_only_polygonal {
        for (payload_index, payload) in extraction.non_polygon_payloads.iter().enumerate() {
            execution_policy
                .check_cancelled_every("tiled_stitched_output_non_polygon", payload_index)?;
            let coordinates = payload
                .coordinate_bits
                .iter()
                .copied()
                .map(coord3d_from_bits)
                .collect::<Vec<_>>();
            match payload.kind {
                crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::Dangle => {
                    dangles.push(coordinates)
                }
                crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::CutEdge => {
                    cut_edges.push(coordinates)
                }
                crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::InvalidRing => {
                    invalid_rings.push(coordinates)
                }
            }
        }
    }
    Ok(Some(TiledStitchedOutput {
        polygons,
        dangles,
        cut_edges,
        invalid_rings,
    }))
}

/// An internal buffered-tile boundary reached by an owned face.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileBoundarySide {
    MinX,
    MaxX,
    MinY,
    MaxY,
}

/// Evidence that an owned face extends to an unresolved buffered-tile boundary.
#[derive(Clone, Debug)]
pub struct TileCoverageIssue {
    pub polygon_index: usize,
    pub polygon_bbox: Rect<f64>,
    pub unresolved_sides: Vec<TileBoundarySide>,
    pub representative_source_line_ids: Vec<u32>,
    /// Complete source IDs when aggregate provenance was requested.
    pub aggregate_source_line_ids: Vec<u32>,
    /// Whether `aggregate_source_line_ids` contains the complete boundary source set.
    pub aggregate_source_line_ids_complete: bool,
}

/// A reconstructed face whose selected ownership point falls outside the
/// configured ownership domain while its envelope overlaps that domain.
///
/// This is definite evidence that the face cannot be owned by any generated
/// tile. It does not clip the face or infer how an application wants to handle
/// input outside the ownership domain.
#[derive(Clone, Debug)]
pub struct TileOwnershipDomainIssue {
    pub polygon_index: usize,
    pub polygon_bbox: Rect<f64>,
    pub ownership_point: Coord3D,
}

/// Input geometry that reaches an internal buffered-tile boundary.
///
/// This is conservative evidence that topology may continue through linework
/// outside the halo, including when no local face was reconstructed.
#[derive(Clone, Debug)]
pub struct TileInputBoundaryIssue {
    pub input_geometry_index: usize,
    pub geometry_bbox: Rect<f64>,
    pub unresolved_sides: Vec<TileBoundarySide>,
}

/// How separate input geometries in an excluded component are connected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileComponentConnection {
    ExactEndpoint,
    SegmentIntersection,
    PreSnap,
    FixedGrid,
}

/// A transformed-connected input component not fully observed in a tile halo.
///
/// The component envelope intersects the buffered tile, but its member geometry
/// envelopes are not fully observed there. This is conservative evidence, not
/// proof that the component contains a face.
#[derive(Clone, Debug)]
pub struct TileExcludedComponentIssue {
    pub input_geometry_indices: Vec<usize>,
    pub component_bbox: Rect<f64>,
    pub connection: TileComponentConnection,
}

/// Deterministic bounded halo growth for unresolved tiles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRetryPolicy {
    pub max_attempts: usize,
    pub buffer_increment: f64,
    pub max_buffer: f64,
}

/// Limits for work owned by one experimental tiled polygonization call.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TileExecutionPolicy {
    pub max_tiles: Option<usize>,
    pub max_input_geometries: Option<usize>,
    pub max_tile_geometry_assignments: Option<usize>,
    pub max_retry_attempts_total: Option<usize>,
    pub max_fallback_regions: Option<usize>,
    pub max_parallel_tiles: Option<usize>,
}

/// Result of one larger-halo retry for a tile.
#[derive(Clone, Debug, PartialEq)]
pub struct TileRetryAttempt {
    pub attempt: usize,
    pub buffer: f64,
    pub unresolved_owned_polygon_count: usize,
    pub unresolved_input_geometry_count: usize,
    pub unresolved_component_count: usize,
    pub unresolved_ownership_domain_count: usize,
    pub resolved: bool,
}

/// Observed work and topology output for one tile.
#[derive(Debug)]
pub struct TileReport {
    pub tile_bbox: Rect<f64>,
    /// Geometries whose bounds intersected the buffered tile.
    pub input_geometry_count: usize,
    /// Polygons produced before tile ownership filtering.
    pub polygon_count: usize,
    pub owned_polygon_count: usize,
    pub dangle_count: usize,
    pub cut_edge_count: usize,
    pub invalid_ring_count: usize,
    /// Definite halo insufficiency observed for owned faces in this tile.
    pub coverage_issues: Vec<TileCoverageIssue>,
    /// Reconstructed faces that cannot be owned by any tile in the domain.
    pub ownership_domain_issues: Vec<TileOwnershipDomainIssue>,
    /// Inputs that may connect to linework beyond this tile's halo.
    pub input_boundary_issues: Vec<TileInputBoundaryIssue>,
    /// Transformed-connected components not fully observed in this tile's halo.
    pub excluded_component_issues: Vec<TileExcludedComponentIssue>,
    pub retry_attempts: Vec<TileRetryAttempt>,
    pub retry_exhausted: bool,
}

/// Outcome of resolving the observed coverage evidence for one tiled call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileCoverageResolution {
    pub observed_issue_count: usize,
    pub resolved_issue_count: usize,
    pub unresolved_issue_count: usize,
    pub resolution: TileCoverageResolutionKind,
    /// Deterministic indexes of component-fallback regions that contributed to
    /// the resolution. Untiled fallback does not use region indexes.
    pub recovered_region_ids: Vec<usize>,
}

/// Classification of the tiled coverage-resolution ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileCoverageResolutionKind {
    NoIssues,
    ComponentFallback,
    UntiledFallback,
    Partial,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum TileCoverageIssueKind {
    OwnedFace,
    OwnershipDomain,
    InputBoundary,
    ExcludedComponent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct TileCoverageIssueId {
    tile_index: usize,
    kind: TileCoverageIssueKind,
    issue_index: usize,
}

/// Counts from merging and deduplicating owned tile polygons.
///
/// These counts do not certify that the configured buffer was sufficient.
#[derive(Debug)]
pub struct StitchingReport {
    pub merged_polygon_count: usize,
    pub duplicate_polygon_count: usize,
    pub output_polygon_count: usize,
    /// Polygon count retained from tile-local ownership before fallback merge.
    pub retained_tile_polygon_count: usize,
    /// Number of indexed components recovered by component or region fallback.
    pub component_fallback_count: usize,
    /// Polygon count produced by component fallback before final deduplication.
    pub component_fallback_polygon_count: usize,
    /// Number of retained tile polygons replaced by region fallback.
    pub component_fallback_replaced_polygon_count: usize,
    /// Whether component fallback was enabled and attempted for unresolved output.
    pub component_fallback_attempted: bool,
    /// Why an attempted component fallback was declined, when it was not safe.
    pub component_fallback_decline_reason: Option<&'static str>,
    pub unresolved_tile_count: usize,
    pub unresolved_owned_polygon_count: usize,
    pub unresolved_ownership_domain_tile_count: usize,
    pub unresolved_ownership_domain_count: usize,
    pub unresolved_input_tile_count: usize,
    /// Input-boundary issue instances across tiles; a geometry may occur more than once.
    pub unresolved_input_geometry_count: usize,
    pub unresolved_component_tile_count: usize,
    /// Excluded-component issue instances across tiles; a component may occur more than once.
    pub unresolved_component_count: usize,
    pub retried_tile_count: usize,
    pub retry_attempt_count: usize,
    pub retry_exhausted_tile_count: usize,
    /// Number of declared geometric adjacencies used for border matching.
    pub partition_border_adjacency_count: usize,
    /// Number of normalized border edge buckets covered by those adjacencies.
    pub partition_border_normalized_edge_count: usize,
    /// Number of unambiguous opposite-direction border pairs.
    pub partition_border_twin_count: usize,
    /// Normalized border buckets conservatively left unmatched.
    pub partition_border_unmatched_edge_count: usize,
    /// Canonical border nodes reconciled across all exported observations.
    pub partition_border_reconciled_node_count: usize,
    /// Reconciled border nodes whose Z candidates exceed the configured
    /// conflict tolerance.
    pub partition_border_node_z_conflict_count: usize,
    /// Canonical border nodes available to the detached global face graph.
    pub partition_border_canonical_node_count: usize,
    /// Active global face-node slots checked against canonical payloads.
    pub partition_border_canonical_global_node_count: usize,
    /// Active global face-node slots with a canonical payload match.
    pub partition_border_canonical_mapped_global_node_count: usize,
    /// Canonical-only nodes retained for non-face-qualified evidence.
    pub partition_border_canonical_only_node_count: usize,
    /// Whether active global face-node payloads reconcile with canonical nodes.
    pub partition_border_canonical_node_reconciliation_ready: bool,
    /// Deterministic connected components of qualified border-face evidence.
    pub partition_border_global_component_count: usize,
    /// Retained deterministic payload plans for qualified border components.
    pub partition_border_global_component_payload_count: usize,
    /// Source IDs retained in component-level border payload plans.
    pub partition_border_global_component_payload_source_line_count: usize,
    /// Representative IDs retained in component-level border payload plans.
    pub partition_border_global_component_payload_representative_line_count: usize,
    /// Distinct endpoint Z candidates retained in component-level payload plans.
    pub partition_border_global_component_payload_z_candidate_count: usize,
    /// Selected endpoint Z decisions retained for component-level payload plans.
    pub partition_border_global_component_payload_selected_z_node_count: usize,
    /// Reconciled nodes with explicit Z conflicts in component payload plans.
    pub partition_border_global_component_payload_z_conflict_node_count: usize,
    /// Components containing at least one explicit Z conflict.
    pub partition_border_global_component_payload_z_conflict_component_count: usize,
    /// Qualified face references included in the retained global plan.
    pub partition_border_global_face_count: usize,
    /// Face references participating in at least one retained twin link.
    pub partition_border_global_linked_face_count: usize,
    /// Qualified local faces retained in the deterministic face-walk plan.
    pub partition_border_global_face_plan_count: usize,
    /// Local border half-edges retained as face-walk candidates.
    pub partition_border_global_face_candidate_count: usize,
    /// Qualified observations that lacked a local face-walk successor.
    pub partition_border_global_face_missing_successor_count: usize,
    /// Local faces marked unbounded by their source arrangement.
    pub partition_border_global_unbounded_face_count: usize,
    /// Planned faces that participate in at least one retained twin edge.
    pub partition_border_global_face_linked_count: usize,
    /// Qualified observations whose local face boundary continuation was not
    /// resolved to another retained border observation.
    pub partition_border_global_face_missing_boundary_successor_count: usize,
    /// Face plans that passed immutable identity and twin-link validation.
    pub partition_border_global_face_validated_count: usize,
    /// Face-boundary candidates that passed observation-lineage validation.
    pub partition_border_global_face_validated_candidate_count: usize,
    /// Retained twin links whose two face-plan endpoints were validated.
    pub partition_border_global_face_validated_twin_count: usize,
    /// Validated face plans marked unbounded by their source arrangements.
    pub partition_border_global_face_validated_unbounded_count: usize,
    /// Face-boundary transitions resolved for the mutation gate.
    pub partition_border_global_face_boundary_transition_count: usize,
    /// Face-boundary transitions that remain incomplete for mutation.
    pub partition_border_global_face_mutation_missing_successor_count: usize,
    /// Face plans whose retained transitions form one closed local cycle.
    pub partition_border_global_face_mutation_ready_count: usize,
    /// Face-boundary transitions retained in deterministic cycle order.
    pub partition_border_global_face_transition_count: usize,
    /// Face plans with a complete closed transition cycle.
    pub partition_border_global_face_transition_closed_count: usize,
    /// Face plans retained with incomplete transition evidence.
    pub partition_border_global_face_transition_incomplete_count: usize,
    /// Declared face-qualified twins positioned in local transition cycles.
    pub partition_border_global_face_twin_transition_count: usize,
    /// Declared twins whose two local cycles are mutation-ready.
    pub partition_border_global_face_twin_transition_ready_count: usize,
    /// Declared twins not present in both local transition cycles.
    pub partition_border_global_face_twin_transition_unmapped_count: usize,
    /// Face plans whose retained local walks passed global evidence validation.
    pub partition_border_global_face_walk_validated_count: usize,
    /// Validated face plans whose retained local walks are closed.
    pub partition_border_global_face_walk_closed_count: usize,
    /// Applied twins whose payload and reconciled endpoint lineage passed validation.
    pub partition_border_global_face_walk_source_complete_twin_count: usize,
    /// Components containing at least one locally unbounded face marker.
    pub partition_border_global_face_walk_unbounded_component_count: usize,
    /// Cycle rank of the retained face/twin connectivity graph, not planar Euler.
    pub partition_border_global_face_walk_face_adjacency_cycle_rank: usize,
    /// Face cycles counted by the retained border-only Euler witness.
    pub partition_border_global_face_euler_transition_face_count: usize,
    /// Closed cycles counted by the retained border-only Euler witness.
    pub partition_border_global_face_euler_closed_boundary_cycle_count: usize,
    /// Unique XY vertices in retained border-only Euler evidence.
    pub partition_border_global_face_euler_boundary_vertex_count: usize,
    /// Unique undirected edges in retained border-only Euler evidence.
    pub partition_border_global_face_euler_boundary_edge_count: usize,
    /// Border spans observed in more than one retained face component.
    pub partition_border_global_face_euler_cross_component_edge_count: usize,
    /// V - E + closed cycles for retained border-only evidence.
    pub partition_border_global_face_euler_boundary_lhs: i64,
    /// C + 1 for retained border-only evidence.
    pub partition_border_global_face_euler_boundary_rhs: i64,
    /// Whether the retained border-only arithmetic happens to balance.
    pub partition_border_global_face_euler_boundary_consistent: bool,
    /// Qualified cross-tile twins retained as deterministic global-next candidates.
    pub partition_border_global_face_next_candidate_count: usize,
    /// Candidate twins whose two local cycles provide a complete splice witness.
    pub partition_border_global_face_next_ready_candidate_count: usize,
    /// Candidate twins with incomplete local-cycle evidence.
    pub partition_border_global_face_next_incomplete_candidate_count: usize,
    /// Distinct predecessor-to-successor assignments retained without mutation.
    pub partition_border_global_face_next_global_successor_count: usize,
    /// Boundary-only global face cycle candidates retained from local cycles.
    pub partition_border_global_face_identity_candidate_cycle_count: usize,
    /// Candidate global face cycles whose prospective successor walk is closed.
    pub partition_border_global_face_identity_closed_cycle_count: usize,
    /// Components with incomplete local boundary evidence.
    pub partition_border_global_face_identity_incomplete_component_count: usize,
    /// Components whose prospective successor map is not a permutation.
    pub partition_border_global_face_identity_non_permutation_component_count: usize,
    /// Boundary observations retained in the identity candidate plan.
    pub partition_border_global_face_identity_boundary_observation_count: usize,
    /// Whether all retained boundary evidence forms closed permutation cycles.
    pub partition_border_global_face_identity_permutation_ready: bool,
    /// Boundary-only global-next mutation plans retained from identity cycles.
    pub partition_border_global_face_next_mutation_plan_count: usize,
    /// Prospective global-next links retained without applying them.
    pub partition_border_global_face_next_mutation_candidate_link_count: usize,
    /// Boundary observations covered by the prospective mutation plan.
    pub partition_border_global_face_next_mutation_boundary_observation_count: usize,
    /// Components whose prospective global-next plan is complete.
    pub partition_border_global_face_next_mutation_ready_component_count: usize,
    /// Components whose prospective global-next plan remains incomplete.
    pub partition_border_global_face_next_mutation_incomplete_component_count: usize,
    /// Whether the prospective global-next plan is safe to apply later.
    pub partition_border_global_face_next_mutation_ready: bool,
    /// Boundary-only candidate global face cycles retained from the mutation
    /// plan. These IDs are not assigned to local observations or output.
    pub partition_border_global_face_id_candidate_cycle_count: usize,
    /// Candidate global face IDs assigned to closed boundary cycles.
    pub partition_border_global_face_id_assigned_count: usize,
    /// Boundary observations covered by candidate global face ID plans.
    pub partition_border_global_face_id_boundary_observation_count: usize,
    /// Candidate cycles containing a local unbounded-face marker.
    pub partition_border_global_face_id_unbounded_candidate_count: usize,
    /// Candidate cycles retained without a global ID because their boundary
    /// evidence is incomplete.
    pub partition_border_global_face_id_incomplete_plan_count: usize,
    /// Whether every retained boundary cycle received a candidate ID.
    pub partition_border_global_face_id_assignment_ready: bool,
    /// Boundary face-ID plans mapped one-to-one onto detached candidate cycles.
    pub partition_border_global_face_id_application_candidate_cycle_count: usize,
    /// Candidate face-ID plans with an assigned deterministic ID.
    pub partition_border_global_face_id_application_assigned_face_count: usize,
    /// Closed cycles retained by the detached global topology candidate.
    pub partition_border_global_face_id_application_cycle_start_count: usize,
    /// Face-ID plans whose observations matched a detached candidate cycle.
    pub partition_border_global_face_id_application_mapped_cycle_count: usize,
    /// Face-ID plans that could not map to a detached candidate cycle.
    pub partition_border_global_face_id_application_unmapped_plan_count: usize,
    /// Duplicate candidate face IDs retained by the evidence gate.
    pub partition_border_global_face_id_application_duplicate_face_id_count: usize,
    /// Missing IDs in the expected contiguous candidate ID range.
    pub partition_border_global_face_id_application_non_contiguous_face_id_count: usize,
    /// Whether candidate face IDs are ready to map onto detached cycles.
    pub partition_border_global_face_id_application_ready: bool,
    /// Conservative exactly-one-local-marker unbounded-face candidates.
    pub partition_border_global_unbounded_face_proof_candidate_count: usize,
    /// Whether the conservative unbounded-face proof gate is ready.
    pub partition_border_global_unbounded_face_proof_ready: bool,
    /// Detached candidate cycles checked against the unique unbounded face.
    pub partition_border_global_unbounded_face_application_candidate_cycle_count: usize,
    /// Candidate face IDs carrying the unique local-unbounded marker.
    pub partition_border_global_unbounded_face_application_candidate_unbounded_face_id_count: usize,
    /// Candidate unbounded cycles mapped through the face-ID application gate.
    pub partition_border_global_unbounded_face_application_mapped_unbounded_cycle_count: usize,
    /// Local-unbounded faces without a candidate global face ID.
    pub partition_border_global_unbounded_face_application_missing_unbounded_face_id_count: usize,
    /// Duplicate candidate IDs carrying local-unbounded evidence.
    pub partition_border_global_unbounded_face_application_duplicate_unbounded_face_id_count: usize,
    /// Whether the unique unbounded-face evidence is ready for future mutation.
    pub partition_border_global_unbounded_face_application_ready: bool,
    /// Detached global topology evidence combined before any future mutation.
    pub partition_border_global_topology_mutation_gate_edge_count: usize,
    pub partition_border_global_topology_mutation_gate_component_count: usize,
    pub partition_border_global_topology_mutation_gate_face_count: usize,
    pub partition_border_global_topology_mutation_gate_candidate_cycle_count: usize,
    pub partition_border_global_topology_mutation_gate_applied_twin_count: usize,
    pub partition_border_global_topology_mutation_gate_mapped_twin_count: usize,
    pub partition_border_global_topology_mutation_gate_source_complete_twin_count: usize,
    pub partition_border_global_topology_mutation_gate_closed_face_count: usize,
    pub partition_border_global_topology_mutation_gate_topology_application_ready: bool,
    pub partition_border_global_topology_mutation_gate_component_coverage_ready: bool,
    pub partition_border_global_topology_mutation_gate_face_id_application_ready: bool,
    pub partition_border_global_topology_mutation_gate_unbounded_face_application_ready: bool,
    pub partition_border_global_topology_mutation_gate_face_walk_ready: bool,
    pub partition_border_global_topology_mutation_gate_euler_evidence_ready: bool,
    pub partition_border_global_topology_mutation_gate_ready: bool,
    /// Detached global successor links committed after the complete gate.
    pub partition_border_global_topology_mutation_applied_next_count: usize,
    pub partition_border_global_topology_mutation_ready: bool,
    pub partition_border_global_topology_mutation_applied: bool,
    /// Detached deterministic face IDs committed after successor mutation.
    pub partition_border_global_face_id_mutation_candidate_cycle_count: usize,
    pub partition_border_global_face_id_mutation_applied_face_id_count: usize,
    pub partition_border_global_face_id_mutation_unbounded_face_id_count: usize,
    pub partition_border_global_face_id_mutation_ready: bool,
    pub partition_border_global_face_id_mutation_applied: bool,
    /// Detached identity for the uniquely proven global unbounded face.
    pub partition_border_global_unbounded_face_mutation_candidate_cycle_count: usize,
    pub partition_border_global_unbounded_face_mutation_candidate_unbounded_face_id_count: usize,
    pub partition_border_global_unbounded_face_mutation_applied_unbounded_face_id: Option<usize>,
    pub partition_border_global_unbounded_face_mutation_applied_cycle_start_global_dir_edge_id:
        Option<usize>,
    pub partition_border_global_unbounded_face_mutation_ready: bool,
    pub partition_border_global_unbounded_face_mutation_applied: bool,
    /// Detached per-edge face identities materialized from committed cycles.
    pub partition_border_global_face_identity_edge_count: usize,
    pub partition_border_global_face_identity_cycle_count: usize,
    pub partition_border_global_face_identity_assigned_edge_count: usize,
    pub partition_border_global_face_identity_missing_face_id_count: usize,
    pub partition_border_global_face_identity_invalid_cycle_count: usize,
    pub partition_border_global_face_identity_unbounded_edge_count: usize,
    pub partition_border_global_face_identity_materialization_ready: bool,
    /// Private global edge-topology records materialized from validated
    /// successor and face-ID buffers.
    pub partition_border_global_face_topology_edge_count: usize,
    pub partition_border_global_face_topology_next_link_count: usize,
    pub partition_border_global_face_topology_face_id_count: usize,
    pub partition_border_global_face_topology_missing_next_count: usize,
    pub partition_border_global_face_topology_invalid_next_count: usize,
    pub partition_border_global_face_topology_duplicate_next_count: usize,
    pub partition_border_global_face_topology_node_discontinuity_count: usize,
    pub partition_border_global_face_topology_missing_face_id_count: usize,
    pub partition_border_global_face_topology_non_contiguous_face_id_count: usize,
    pub partition_border_global_face_topology_unbounded_edge_count: usize,
    pub partition_border_global_face_topology_unbounded_face_id_count: usize,
    pub partition_border_global_face_topology_unbounded_cycle_start_count: usize,
    pub partition_border_global_face_topology_missing_unbounded_identity_count: usize,
    pub partition_border_global_face_topology_unbounded_identity_mismatch_count: usize,
    pub partition_border_global_face_topology_evidence_mismatch_count: usize,
    pub partition_border_global_face_topology_unbounded_face_ready: bool,
    pub partition_border_global_face_topology_ready: bool,
    /// Consolidated private twin, cycle, source, Euler, face-walk, and
    /// topology invariant evidence.
    pub partition_border_global_face_invariant_gate_edge_count: usize,
    pub partition_border_global_face_invariant_gate_cycle_count: usize,
    pub partition_border_global_face_invariant_gate_edge_count_mismatch_count: usize,
    pub partition_border_global_face_invariant_gate_cycle_count_mismatch_count: usize,
    pub partition_border_global_face_invariant_gate_twin_mismatch_count: usize,
    pub partition_border_global_face_invariant_gate_cycle_mismatch_count: usize,
    pub partition_border_global_face_invariant_gate_source_mismatch_count: usize,
    pub partition_border_global_face_invariant_gate_face_walk_failure_count: usize,
    pub partition_border_global_face_invariant_gate_euler_failure_count: usize,
    pub partition_border_global_face_invariant_gate_evidence_mismatch_count: usize,
    pub partition_border_global_face_invariant_gate_identity_ready: bool,
    pub partition_border_global_face_invariant_gate_next_lineage_ready: bool,
    pub partition_border_global_face_invariant_gate_cycle_face_lineage_ready: bool,
    pub partition_border_global_face_invariant_gate_payload_lineage_ready: bool,
    pub partition_border_global_face_invariant_gate_geometry_ready: bool,
    pub partition_border_global_face_invariant_gate_topology_ready: bool,
    pub partition_border_global_face_invariant_gate_extraction_gate_ready: bool,
    pub partition_border_global_face_invariant_gate_ready: bool,
    /// Final detached global face-identity invariant evidence.
    pub partition_border_global_face_identity_invariant_twin_count: usize,
    pub partition_border_global_face_identity_invariant_twin_mapping_mismatch_count: usize,
    pub partition_border_global_face_identity_invariant_cycle_face_mismatch_count: usize,
    pub partition_border_global_face_identity_invariant_successor_discontinuity_count: usize,
    pub partition_border_global_face_identity_invariant_source_incomplete_edge_count: usize,
    pub partition_border_global_face_identity_invariant_face_walk_ready: bool,
    pub partition_border_global_face_identity_invariant_euler_ready: bool,
    pub partition_border_global_face_identity_invariants_ready: bool,
    /// Detached global-next integration evidence against local face lineage,
    /// boundary overrides, committed successors, and face-qualified twins.
    pub partition_border_global_next_lineage_integration_edge_count: usize,
    pub partition_border_global_next_lineage_integration_cycle_count: usize,
    pub partition_border_global_next_lineage_integration_local_successor_count: usize,
    pub partition_border_global_next_lineage_integration_override_count: usize,
    pub partition_border_global_next_lineage_integration_successor_count: usize,
    pub partition_border_global_next_lineage_integration_missing_successor_count: usize,
    pub partition_border_global_next_lineage_integration_local_mismatch_count: usize,
    pub partition_border_global_next_lineage_integration_override_mismatch_count: usize,
    pub partition_border_global_next_lineage_integration_plan_link_count: usize,
    pub partition_border_global_next_lineage_integration_unrepresented_link_count: usize,
    pub partition_border_global_next_lineage_integration_committed_next_count: usize,
    pub partition_border_global_next_lineage_integration_committed_next_mismatch_count: usize,
    pub partition_border_global_next_lineage_integration_twin_count: usize,
    pub partition_border_global_next_lineage_integration_twin_mismatch_count: usize,
    pub partition_border_global_next_lineage_integration_identity_ready: bool,
    pub partition_border_global_next_lineage_integration_ready: bool,
    /// Detached candidate cycles mapped back to exact global face-plan
    /// observation and qualified-face lineage.
    pub partition_border_global_cycle_face_lineage_edge_count: usize,
    pub partition_border_global_cycle_face_lineage_cycle_count: usize,
    pub partition_border_global_cycle_face_lineage_plan_count: usize,
    pub partition_border_global_cycle_face_lineage_closed_cycle_count: usize,
    pub partition_border_global_cycle_face_lineage_mapped_cycle_count: usize,
    pub partition_border_global_cycle_face_lineage_incomplete_cycle_count: usize,
    pub partition_border_global_cycle_face_lineage_invalid_cycle_count: usize,
    pub partition_border_global_cycle_face_lineage_missing_face_id_count: usize,
    pub partition_border_global_cycle_face_lineage_duplicate_face_id_plan_count: usize,
    pub partition_border_global_cycle_face_lineage_unmapped_plan_count: usize,
    pub partition_border_global_cycle_face_lineage_cycle_plan_mismatch_count: usize,
    pub partition_border_global_cycle_face_lineage_cycle_face_ref_mismatch_count: usize,
    pub partition_border_global_cycle_face_lineage_duplicate_plan_face_ref_count: usize,
    pub partition_border_global_cycle_face_lineage_observation_mismatch_count: usize,
    pub partition_border_global_cycle_face_lineage_unbounded_mismatch_count: usize,
    pub partition_border_global_cycle_face_lineage_identity_ready: bool,
    pub partition_border_global_cycle_face_lineage_next_ready: bool,
    pub partition_border_global_cycle_face_lineage_ready: bool,
    /// Final detached cross-check before any future global face promotion.
    pub partition_border_global_cycle_face_promotion_gate_edge_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_cycle_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_plan_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_component_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_face_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_covered_face_edge_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_candidate_unbounded_face_id_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_mapped_unbounded_cycle_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_lineage_ready: bool,
    pub partition_border_global_cycle_face_promotion_gate_component_coverage_ready: bool,
    pub partition_border_global_cycle_face_promotion_gate_unbounded_face_application_ready: bool,
    pub partition_border_global_cycle_face_promotion_gate_edge_count_mismatch_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_cycle_count_mismatch_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_plan_count_mismatch_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_face_count_mismatch_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_unbounded_marker_mismatch_count: usize,
    pub partition_border_global_cycle_face_promotion_gate_ready: bool,
    /// Detached face-cycle source, Z, observation, and global-node lineage.
    pub partition_border_global_face_payload_lineage_edge_count: usize,
    pub partition_border_global_face_payload_lineage_cycle_count: usize,
    pub partition_border_global_face_payload_lineage_plan_count: usize,
    pub partition_border_global_face_payload_lineage_checked_edge_count: usize,
    pub partition_border_global_face_payload_lineage_checked_cycle_count: usize,
    pub partition_border_global_face_payload_lineage_missing_face_id_count: usize,
    pub partition_border_global_face_payload_lineage_missing_plan_count: usize,
    pub partition_border_global_face_payload_lineage_missing_observation_count: usize,
    pub partition_border_global_face_payload_lineage_source_incomplete_edge_count: usize,
    pub partition_border_global_face_payload_lineage_source_mismatch_count: usize,
    pub partition_border_global_face_payload_lineage_z_mismatch_count: usize,
    pub partition_border_global_face_payload_lineage_face_mismatch_count: usize,
    pub partition_border_global_face_payload_lineage_node_mismatch_count: usize,
    pub partition_border_global_face_payload_lineage_ready: bool,
    /// Detached cycle winding and containment evidence; no output is promoted.
    pub partition_border_global_face_cycle_geometry_edge_count: usize,
    pub partition_border_global_face_cycle_geometry_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_checked_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_closed_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_missing_node_count: usize,
    pub partition_border_global_face_cycle_geometry_node_discontinuity_count: usize,
    pub partition_border_global_face_cycle_geometry_repeated_edge_count: usize,
    pub partition_border_global_face_cycle_geometry_missing_face_id_count: usize,
    pub partition_border_global_face_cycle_geometry_missing_interior_point_count: usize,
    pub partition_border_global_face_cycle_geometry_degenerate_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_positive_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_negative_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_unbounded_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_unbounded_orientation_mismatch_count: usize,
    pub partition_border_global_face_cycle_geometry_containment_pair_count: usize,
    pub partition_border_global_face_cycle_geometry_contained_cycle_count: usize,
    pub partition_border_global_face_cycle_geometry_nested_opposite_orientation_pair_count: usize,
    pub partition_border_global_face_cycle_geometry_nested_same_orientation_pair_count: usize,
    pub partition_border_global_face_cycle_geometry_edge_pair_count: usize,
    pub partition_border_global_face_cycle_geometry_checked_edge_pair_count: usize,
    pub partition_border_global_face_cycle_geometry_expected_reciprocal_pair_count: usize,
    pub partition_border_global_face_cycle_geometry_proper_crossing_count: usize,
    pub partition_border_global_face_cycle_geometry_endpoint_touch_count: usize,
    pub partition_border_global_face_cycle_geometry_boundary_touch_count: usize,
    pub partition_border_global_face_cycle_geometry_collinear_overlap_count: usize,
    pub partition_border_global_face_cycle_geometry_unexpected_collinear_overlap_count: usize,
    pub partition_border_global_face_cycle_geometry_interaction_ready: bool,
    /// Closed detached cycles that canonicalize into stable coordinate/Z payloads.
    pub partition_border_global_face_cycle_geometry_canonical_ring_count: usize,
    /// Detached cycles whose canonical ring payload was not stable on repeat.
    pub partition_border_global_face_cycle_geometry_canonical_ring_mismatch_count: usize,
    /// Non-adjacent edge intersections within one detached cycle.
    pub partition_border_global_face_cycle_geometry_self_intersection_count: usize,
    /// Detached edges with reciprocal symmetric-edge coverage.
    pub partition_border_global_face_cycle_geometry_reciprocal_edge_count: usize,
    /// Detached edges with missing or non-reciprocal symmetric-edge coverage.
    pub partition_border_global_face_cycle_geometry_reciprocal_edge_mismatch_count: usize,
    /// Whether detached ring payload evidence is ready for a future extraction gate.
    pub partition_border_global_face_cycle_geometry_ring_payload_ready: bool,
    pub partition_border_global_face_cycle_geometry_ready: bool,
    /// Final detached pre-extraction evidence gate; no stitched output is promoted.
    pub partition_border_global_face_extraction_gate_ready: bool,
    pub partition_border_global_face_extraction_gate_edge_count_mismatch_count: usize,
    pub partition_border_global_face_extraction_gate_cycle_count_mismatch_count: usize,
    /// Detached canonical ring payloads retained after the extraction gate.
    pub partition_border_global_face_ring_payload_edge_count: usize,
    pub partition_border_global_face_ring_payload_cycle_count: usize,
    pub partition_border_global_face_ring_payload_materialized_cycle_count: usize,
    pub partition_border_global_face_ring_payload_coordinate_count: usize,
    pub partition_border_global_face_ring_payload_source_line_id_count: usize,
    pub partition_border_global_face_ring_payload_missing_face_id_count: usize,
    pub partition_border_global_face_ring_payload_missing_edge_face_id_count: usize,
    pub partition_border_global_face_ring_payload_invalid_cycle_count: usize,
    pub partition_border_global_face_ring_payload_canonical_ring_mismatch_count: usize,
    pub partition_border_global_face_ring_payload_unbounded_cycle_count: usize,
    pub partition_border_global_face_ring_payload_ready: bool,
    /// Detached ring shell/hole candidate classification evidence.
    pub partition_border_global_face_ring_classification_cycle_count: usize,
    pub partition_border_global_face_ring_classification_classified_cycle_count: usize,
    pub partition_border_global_face_ring_classification_shell_candidate_count: usize,
    pub partition_border_global_face_ring_classification_hole_candidate_count: usize,
    pub partition_border_global_face_ring_classification_unbounded_cycle_count: usize,
    pub partition_border_global_face_ring_classification_containment_pair_count: usize,
    pub partition_border_global_face_ring_classification_contained_cycle_count: usize,
    pub partition_border_global_face_ring_classification_nested_same_orientation_pair_count: usize,
    pub partition_border_global_face_ring_classification_ambiguous_interaction_count: usize,
    pub partition_border_global_face_ring_classification_missing_interior_point_count: usize,
    pub partition_border_global_face_ring_classification_invalid_cycle_count: usize,
    pub partition_border_global_face_ring_classification_evidence_mismatch_count: usize,
    pub partition_border_global_face_ring_classification_ready: bool,
    /// Detached shell-to-hole candidate assembly evidence.
    pub partition_border_global_face_ring_candidate_assembly_cycle_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_shell_candidate_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_hole_candidate_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_assembled_shell_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_assigned_hole_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_unassigned_hole_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_ambiguous_hole_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_evidence_mismatch_count: usize,
    pub partition_border_global_face_ring_candidate_assembly_ready: bool,
    /// Private shell/hole extraction candidates backed by retained payloads.
    pub partition_border_global_face_ring_extraction_candidate_cycle_count: usize,
    pub partition_border_global_face_ring_extraction_candidate_shell_count: usize,
    pub partition_border_global_face_ring_extraction_candidate_hole_count: usize,
    pub partition_border_global_face_ring_extraction_candidate_coordinate_count: usize,
    pub partition_border_global_face_ring_extraction_candidate_source_line_id_count: usize,
    pub partition_border_global_face_ring_extraction_missing_payload_count: usize,
    pub partition_border_global_face_ring_extraction_duplicate_face_id_count: usize,
    pub partition_border_global_face_ring_extraction_duplicate_cycle_start_count: usize,
    pub partition_border_global_face_ring_extraction_duplicate_candidate_count: usize,
    pub partition_border_global_face_ring_extraction_unbounded_payload_count: usize,
    pub partition_border_global_face_ring_extraction_invalid_coordinate_count: usize,
    pub partition_border_global_face_ring_extraction_source_lineage_mismatch_count: usize,
    pub partition_border_global_face_ring_extraction_evidence_mismatch_count: usize,
    pub partition_border_global_face_ring_extraction_ready: bool,
    /// Private stitched shell/hole payload materialization evidence.
    pub partition_border_global_face_ring_extraction_payload_candidate_count: usize,
    pub partition_border_global_face_ring_extraction_payload_materialized_candidate_count: usize,
    pub partition_border_global_face_ring_extraction_payload_shell_coordinate_count: usize,
    pub partition_border_global_face_ring_extraction_payload_hole_coordinate_count: usize,
    pub partition_border_global_face_ring_extraction_payload_source_line_id_count: usize,
    pub partition_border_global_face_ring_extraction_payload_missing_count: usize,
    pub partition_border_global_face_ring_extraction_payload_duplicate_count: usize,
    pub partition_border_global_face_ring_extraction_payload_invalid_count: usize,
    pub partition_border_global_face_ring_extraction_payload_source_lineage_mismatch_count: usize,
    pub partition_border_global_face_ring_extraction_payload_evidence_mismatch_count: usize,
    pub partition_border_global_face_ring_extraction_payload_ready: bool,
    /// Private non-polygon extraction evidence retained from tile-local output.
    pub partition_border_global_non_polygon_extraction_dangle_count: usize,
    pub partition_border_global_non_polygon_extraction_cut_edge_count: usize,
    pub partition_border_global_non_polygon_extraction_invalid_ring_count: usize,
    pub partition_border_global_non_polygon_extraction_coordinate_count: usize,
    pub partition_border_global_non_polygon_extraction_duplicate_payload_count: usize,
    pub partition_border_global_non_polygon_extraction_invalid_coordinate_count: usize,
    pub partition_border_global_non_polygon_extraction_evidence_mismatch_count: usize,
    pub partition_border_global_non_polygon_extraction_ready: bool,
    /// Consolidated private extraction readiness after all detached payloads
    /// and topology records have committed atomically.
    pub partition_border_global_extraction_readiness_edge_count: usize,
    pub partition_border_global_extraction_readiness_topology_edge_count: usize,
    pub partition_border_global_extraction_readiness_candidate_shell_count: usize,
    pub partition_border_global_extraction_readiness_candidate_hole_count: usize,
    pub partition_border_global_extraction_readiness_candidate_coordinate_count: usize,
    pub partition_border_global_extraction_readiness_materialized_candidate_count: usize,
    pub partition_border_global_extraction_readiness_non_polygon_payload_count: usize,
    pub partition_border_global_extraction_readiness_missing_topology_count: usize,
    pub partition_border_global_extraction_readiness_missing_ring_candidate_count: usize,
    pub partition_border_global_extraction_readiness_missing_ring_payload_count: usize,
    pub partition_border_global_extraction_readiness_missing_non_polygon_payload_count: usize,
    pub partition_border_global_extraction_readiness_missing_invariant_gate_count: usize,
    pub partition_border_global_extraction_readiness_evidence_mismatch_count: usize,
    pub partition_border_global_extraction_readiness_invariant_gate_ready: bool,
    pub partition_border_global_extraction_readiness_topology_ready: bool,
    pub partition_border_global_extraction_readiness_ring_candidate_ready: bool,
    pub partition_border_global_extraction_readiness_ring_payload_ready: bool,
    pub partition_border_global_extraction_readiness_non_polygon_payload_ready: bool,
    pub partition_border_global_extraction_readiness_ready: bool,
    /// Private extraction snapshot committed after the complete readiness gate.
    pub partition_border_global_private_extraction_ring_payload_count: usize,
    pub partition_border_global_private_extraction_hole_count: usize,
    pub partition_border_global_private_extraction_dangle_count: usize,
    pub partition_border_global_private_extraction_cut_edge_count: usize,
    pub partition_border_global_private_extraction_invalid_ring_count: usize,
    pub partition_border_global_private_extraction_coordinate_count: usize,
    pub partition_border_global_private_extraction_source_line_id_count: usize,
    pub partition_border_global_private_extraction_missing_ring_payload_count: usize,
    pub partition_border_global_private_extraction_missing_non_polygon_payload_count: usize,
    pub partition_border_global_private_extraction_invalid_ring_payload_count: usize,
    pub partition_border_global_private_extraction_invalid_non_polygon_payload_count: usize,
    pub partition_border_global_private_extraction_evidence_mismatch_count: usize,
    pub partition_border_global_private_extraction_ready: bool,
    /// Whether validated stitched output was exposed in the additive sidecar.
    pub partition_border_global_stitched_output_ready: bool,
    /// Whether the opt-in validated stitched versus untiled comparison ran.
    pub partition_border_global_untiled_equivalence_checked: bool,
    /// Whether every canonical output family matched the untiled result.
    pub partition_border_global_untiled_equivalence_ready: bool,
    /// Number of canonical output families that differed from untiled output.
    pub partition_border_global_untiled_equivalence_mismatch_count: usize,
    /// Unbounded-face candidates whose local cycles are closed.
    pub partition_border_global_unbounded_face_proof_closed_count: usize,
    /// Unbounded-face twins absent from the retained twin-position map.
    pub partition_border_global_unbounded_face_proof_unmapped_twin_count: usize,
    /// Unbounded-face twins whose opposite local cycle is incomplete.
    pub partition_border_global_unbounded_face_proof_not_ready_twin_count: usize,
    /// Exact twin pairs whose observations also carried valid qualified local
    /// face references and were retained as face-level links.
    pub partition_border_face_twin_count: usize,
    /// Exact twin pairs declined because at least one observation had no face
    /// reference.
    pub partition_border_face_twin_missing_face_count: usize,
    /// Exact twin pairs declined because a face reference was malformed or
    /// did not match its observation's partition.
    pub partition_border_face_twin_invalid_face_count: usize,
    /// Processed local face-component snapshots retained for global mapping.
    pub partition_border_global_face_edge_map_local_graph_count: usize,
    /// Active directed edges remapped into deterministic global edge slots.
    pub partition_border_global_face_edge_map_directed_edge_count: usize,
    /// Local face successors successfully remapped into global edge slots.
    pub partition_border_global_face_edge_map_local_successor_count: usize,
    /// Face-qualified border observations mapped to active local edge slots.
    pub partition_border_global_face_edge_map_observation_count: usize,
    /// Applied face twins mapped across two active local edge slots.
    pub partition_border_global_face_edge_map_twin_count: usize,
    /// Applied face twins without complete local edge snapshots.
    pub partition_border_global_face_edge_map_unmapped_twin_count: usize,
    /// Whether every applied face twin mapped to active local edge slots.
    pub partition_border_global_face_edge_map_ready: bool,
    /// Active global face edges covered by canonical global node slots.
    pub partition_border_global_face_node_edge_count: usize,
    /// Deterministic global node slots retained for active face-edge endpoints.
    pub partition_border_global_face_node_count: usize,
    /// Active face-edge endpoints assigned to global node slots.
    pub partition_border_global_face_node_endpoint_count: usize,
    /// Border observations mapped into global node payloads.
    pub partition_border_global_face_node_observation_count: usize,
    /// Border observations without a matching active global edge slot.
    pub partition_border_global_face_node_unmapped_observation_count: usize,
    /// Distinct endpoint-Z candidates retained by global node payloads.
    pub partition_border_global_face_node_z_candidate_count: usize,
    /// Global node slots with Z candidates outside the selected tolerance.
    pub partition_border_global_face_node_z_conflict_count: usize,
    /// Whether every retained observation received global endpoint slots.
    pub partition_border_global_face_node_ready: bool,
    /// Global-face mutation cycles retained in global edge-slot space.
    pub partition_border_global_face_next_application_plan_count: usize,
    /// Candidate global edge-slot successor links retained by those plans.
    pub partition_border_global_face_next_application_link_count: usize,
    /// Active global edge slots covered by the application plan.
    pub partition_border_global_face_next_application_edge_count: usize,
    /// Applied cross-border twins validated in global node-slot space.
    pub partition_border_global_face_next_application_twin_count: usize,
    /// Observations without global edge-slot lineage for application.
    pub partition_border_global_face_next_application_unmapped_observation_count: usize,
    /// Mutation cycles retained incomplete or not node-continuous.
    pub partition_border_global_face_next_application_incomplete_plan_count: usize,
    /// Candidate links whose endpoint node slots were discontinuous.
    pub partition_border_global_face_next_application_node_discontinuity_count: usize,
    /// Whether the retained boundary application plan is exact and continuous.
    pub partition_border_global_face_next_application_ready: bool,
    /// Active global edge slots represented by the detached topology candidate.
    pub partition_border_global_topology_candidate_edge_count: usize,
    /// Local successor links copied into the detached topology candidate.
    pub partition_border_global_topology_candidate_local_successor_count: usize,
    /// Global boundary successor overrides materialized in the candidate.
    pub partition_border_global_topology_candidate_global_override_count: usize,
    /// Directed edges with an assigned candidate successor.
    pub partition_border_global_topology_candidate_assigned_next_count: usize,
    /// Directed edges without a candidate successor.
    pub partition_border_global_topology_candidate_unassigned_next_count: usize,
    /// Closed cycles found in the detached candidate.
    pub partition_border_global_topology_candidate_cycle_count: usize,
    /// Edges covered by closed cycles in the detached candidate.
    pub partition_border_global_topology_candidate_closed_cycle_edge_count: usize,
    /// Candidate successor targets with more than one predecessor.
    pub partition_border_global_topology_candidate_predecessor_conflict_count: usize,
    /// Candidate links whose endpoint node slots are discontinuous.
    pub partition_border_global_topology_candidate_node_discontinuity_count: usize,
    /// Global application plans excluded because they were incomplete.
    pub partition_border_global_topology_candidate_incomplete_application_plan_count: usize,
    /// Whether the detached candidate is a complete cycle system.
    pub partition_border_global_topology_candidate_ready: bool,
    /// Edge slots checked by the final detached-topology application gate.
    pub partition_border_global_topology_application_gate_edge_count: usize,
    /// Candidate successor links checked by the final application gate.
    pub partition_border_global_topology_application_gate_successor_count: usize,
    /// Declared partition adjacencies available to the application gate.
    pub partition_border_global_topology_application_gate_adjacency_count: usize,
    /// Face-qualified twins retained from declared-adjacency application.
    pub partition_border_global_topology_application_gate_applied_twin_count: usize,
    /// Twin pairs mapped into reciprocal global edge slots.
    pub partition_border_global_topology_application_gate_mapped_twin_count: usize,
    /// Global twin pairs without complete applied evidence.
    pub partition_border_global_topology_application_gate_unmapped_twin_count: usize,
    /// Applied twin records rejected by the final adjacency/lineage gate.
    pub partition_border_global_topology_application_gate_invalid_twin_count: usize,
    /// Candidate successor targets with multiple predecessors at the gate.
    pub partition_border_global_topology_application_gate_predecessor_conflict_count: usize,
    /// Candidate links with discontinuous endpoint node slots at the gate.
    pub partition_border_global_topology_application_gate_node_discontinuity_count: usize,
    /// Whether the detached candidate is safe for a future mutation phase.
    pub partition_border_global_topology_application_gate_ready: bool,
    /// Deterministic global components checked against candidate face edges.
    pub partition_border_global_component_coverage_component_count: usize,
    /// Qualified faces retained by global component evidence.
    pub partition_border_global_component_coverage_face_count: usize,
    /// Candidate edge slots checked for component coverage.
    pub partition_border_global_component_coverage_edge_count: usize,
    /// Candidate edges carrying qualified face lineage.
    pub partition_border_global_component_coverage_face_edge_count: usize,
    /// Face-qualified candidate edges covered by one global component.
    pub partition_border_global_component_coverage_covered_face_edge_count: usize,
    /// Face-qualified candidate edges without component coverage.
    pub partition_border_global_component_coverage_uncovered_face_edge_count: usize,
    /// Faces assigned to multiple global components.
    pub partition_border_global_component_coverage_duplicate_face_count: usize,
    /// Twin edge keys assigned to multiple global components.
    pub partition_border_global_component_coverage_duplicate_twin_edge_count: usize,
    /// Whether component coverage is complete and deterministic.
    pub partition_border_global_component_coverage_ready: bool,
    /// Whether indexed components were recovered by component or region fallback.
    /// This is operational metadata; strict validation uses `coverage_resolution`.
    pub component_fallback_used: bool,
    /// Whether the whole-input fallback was selected for execution.
    pub untiled_fallback_attempted: bool,
    /// Whether the whole-input fallback completed and is authoritative.
    pub untiled_fallback_authoritative: bool,
    /// Polygon count produced by an authoritative whole-input fallback.
    pub untiled_fallback_output_polygon_count: usize,
    /// Backward-compatible alias for `untiled_fallback_authoritative`.
    pub untiled_fallback_used: bool,
    pub coverage_resolution: TileCoverageResolution,
}

/// Experimental tiled output with per-tile and merge diagnostics.
#[derive(Debug)]
pub struct TiledPolygonizeResult {
    pub polygons: Vec<Polygon3D>,
    /// Validated stitched output, when the private extraction gate passes.
    /// The existing `polygons` field remains the replicate-and-own tiled
    /// output until full untiled equivalence is proven.
    pub stitched_output: Option<TiledStitchedOutput>,
    pub tile_reports: Vec<TileReport>,
    pub stitching_report: StitchingReport,
    /// Canonical observations captured from physical tile arrangements. A
    /// conservative face-qualified twin-link plan may be retained on this
    /// graph, but tiled output does not build a global arrangement or replace
    /// replicate-and-own polygons with stitched output.
    #[doc(hidden)]
    pub partition_border_graph: PartitionBorderGraph,
    #[doc(hidden)]
    #[allow(dead_code)]
    pub(crate) partition_snapshots: Vec<PartitionSnapshotV1>,
}

/// Experimental stitched output produced from the validated private global
/// extraction snapshot. It is additive and does not replace tiled output.
#[derive(Clone, Debug)]
pub struct TiledStitchedOutput {
    pub polygons: Vec<Polygon3D>,
    pub dangles: Vec<Vec<Coord3D>>,
    pub cut_edges: Vec<Vec<Coord3D>>,
    pub invalid_rings: Vec<Vec<Coord3D>>,
}

/// Coverage contract requested for experimental tiled polygonization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TileCoverageGuarantee {
    /// Return output with any detected coverage issues in the tile reports.
    #[default]
    BestEffort,
    /// Reject output when an owned face reaches an internal buffered-tile boundary.
    ///
    /// This validates reconstructed owned faces only. It cannot detect a region
    /// that is absent because its closing linework fell outside every tile halo.
    ValidateOwnedFaces,
    /// Reject tiled output when owned-face, input-boundary, or excluded
    /// component evidence is present.
    ///
    /// A successful caller-enabled untiled fallback replaces unresolved tiled
    /// output and satisfies this guarantee. Otherwise this validates observed
    /// evidence only; it does not certify connected regions whose geometry
    /// never intersected a tile halo.
    ValidateObservedCoverage,
}

/// Failure from experimental tiled polygonization with coverage validation.
#[derive(Debug, Error)]
pub enum TiledPolygonizeError {
    #[error(transparent)]
    Polygonize(#[from] PolygonizeError),
    #[error(
        "tiled coverage validation failed for {unresolved_owned_polygon_count} owned polygons, {unresolved_ownership_domain_count} ownership-domain faces, {unresolved_input_geometry_count} input boundary instances, and {unresolved_component_count} excluded linework-component instances"
    )]
    CoverageIncomplete {
        unresolved_tile_count: usize,
        unresolved_owned_polygon_count: usize,
        unresolved_ownership_domain_tile_count: usize,
        unresolved_ownership_domain_count: usize,
        unresolved_input_tile_count: usize,
        unresolved_input_geometry_count: usize,
        unresolved_component_tile_count: usize,
        unresolved_component_count: usize,
        retry_attempt_count: usize,
        retry_exhausted_tile_count: usize,
        component_fallback_decline_reason: Option<&'static str>,
        coverage_resolution: Box<TileCoverageResolution>,
        tile_reports: Box<Vec<TileReport>>,
    },
}

/// Experimental tiled output paired with a bounded topology trace.
#[derive(Debug)]
pub struct TracedTiledPolygonizeResultV1 {
    pub result: TiledPolygonizeResult,
    pub trace: TopologyTraceV1,
}

type TileOwnershipDecision = (usize, Option<Coord3D>, bool);
type TileProcessResult = (
    Vec<Polygon3D>,
    TileReport,
    Vec<TileOwnershipDecision>,
    Vec<PartitionBorderHalfEdge>,
    Vec<PartitionBorderLocalFaceGraph>,
    bool,
    crate::graph::planar_graph::PartitionBoundaryNodingStats,
    Vec<Vec<Coord3D>>,
    Vec<Vec<Coord3D>>,
    Vec<Vec<Coord3D>>,
    Vec<PartitionNodedSegment>,
    Vec<PartitionNodedSegment>,
    PartitionSnapshotV1,
);

#[derive(Debug)]
struct InputComponent {
    input_geometry_indices: Vec<usize>,
    bbox: Rect<f64>,
    connection: TileComponentConnection,
}

struct ComponentFallbackResult {
    polygons: Vec<Polygon3D>,
    events: Vec<ComponentFallbackEvent>,
    region_bboxes: Vec<Rect<f64>>,
}

struct ComponentFallbackEvent {
    input_geometry_indices: Vec<usize>,
    output_polygon_count: usize,
    recovered_component_count: usize,
    replaced_retained_polygon_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComponentFallbackDeclineReason {
    NoIndexedComponentEvidence,
    OwnedFaceCoverageEvidence,
    IndexedComponentMissing,
    InputBoundaryOutsideRecoveryRegion,
    NonClosedRecoveryRegion,
    EmptyRecoveryOutput,
}

impl ComponentFallbackDeclineReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::NoIndexedComponentEvidence => "no_indexed_component_evidence",
            Self::OwnedFaceCoverageEvidence => "owned_face_coverage_evidence",
            Self::IndexedComponentMissing => "indexed_component_missing",
            Self::InputBoundaryOutsideRecoveryRegion => "input_boundary_outside_recovery_region",
            Self::NonClosedRecoveryRegion => "non_closed_recovery_region",
            Self::EmptyRecoveryOutput => "empty_recovery_output",
        }
    }
}

enum ComponentFallbackDecision {
    Recovered(ComponentFallbackResult),
    Declined(ComponentFallbackDeclineReason),
}

fn build_coverage_resolution(
    tile_reports: &[TileReport],
    component_fallback_used: bool,
    region_bboxes: &[Rect<f64>],
    untiled_fallback_authoritative: bool,
) -> TileCoverageResolution {
    let mut observed_issue_ids = HashSet::new();
    let mut resolved_issue_ids = HashSet::new();
    let mut record_issue =
        |tile_index: usize, kind: TileCoverageIssueKind, issue_index: usize, bbox: Rect<f64>| {
            let issue_id = TileCoverageIssueId {
                tile_index,
                kind,
                issue_index,
            };
            observed_issue_ids.insert(issue_id);
            let resolved = untiled_fallback_authoritative
                || (component_fallback_used
                    && kind != TileCoverageIssueKind::OwnershipDomain
                    && region_bboxes
                        .iter()
                        .any(|region_bbox| bbox.intersects(region_bbox)));
            if resolved {
                resolved_issue_ids.insert(issue_id);
            }
        };

    for (tile_index, report) in tile_reports.iter().enumerate() {
        for (issue_index, issue) in report.coverage_issues.iter().enumerate() {
            record_issue(
                tile_index,
                TileCoverageIssueKind::OwnedFace,
                issue_index,
                issue.polygon_bbox,
            );
        }
        for (issue_index, issue) in report.ownership_domain_issues.iter().enumerate() {
            record_issue(
                tile_index,
                TileCoverageIssueKind::OwnershipDomain,
                issue_index,
                issue.polygon_bbox,
            );
        }
        for (issue_index, issue) in report.input_boundary_issues.iter().enumerate() {
            record_issue(
                tile_index,
                TileCoverageIssueKind::InputBoundary,
                issue_index,
                issue.geometry_bbox,
            );
        }
        for (issue_index, issue) in report.excluded_component_issues.iter().enumerate() {
            record_issue(
                tile_index,
                TileCoverageIssueKind::ExcludedComponent,
                issue_index,
                issue.component_bbox,
            );
        }
    }

    let observed_issue_count = observed_issue_ids.len();
    let resolved_issue_count = resolved_issue_ids.len();
    let unresolved_issue_count = observed_issue_count - resolved_issue_count;
    let resolution = match (observed_issue_count, resolved_issue_count) {
        (0, _) => TileCoverageResolutionKind::NoIssues,
        (_, 0) => TileCoverageResolutionKind::Unresolved,
        (_, resolved) if resolved == observed_issue_count => {
            if untiled_fallback_authoritative {
                TileCoverageResolutionKind::UntiledFallback
            } else {
                TileCoverageResolutionKind::ComponentFallback
            }
        }
        _ => TileCoverageResolutionKind::Partial,
    };
    TileCoverageResolution {
        observed_issue_count,
        resolved_issue_count,
        unresolved_issue_count,
        resolution,
        recovered_region_ids: if component_fallback_used {
            (0..region_bboxes.len()).collect()
        } else {
            Vec::new()
        },
    }
}

fn account_polygon_output(
    policy: &ExecutionPolicy,
    polygon_count: &mut usize,
    coordinate_count: &mut usize,
    polygon: &Polygon3D,
) -> Result<()> {
    let next_polygon_count = polygon_count.checked_add(1).ok_or_else(|| {
        PolygonizeError::InternalInvariantViolation {
            reason: "tiled output polygon count overflow".to_string(),
        }
    })?;
    let polygon_coordinates = polygon
        .exterior
        .len()
        .checked_add(
            polygon
                .interiors
                .iter()
                .try_fold(0usize, |count, ring| count.checked_add(ring.len()))
                .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                    reason: "tiled polygon coordinate count overflow".to_string(),
                })?,
        )
        .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
            reason: "tiled polygon coordinate count overflow".to_string(),
        })?;
    let next_coordinate_count = coordinate_count
        .checked_add(polygon_coordinates)
        .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
            reason: "tiled output coordinate count overflow".to_string(),
        })?;
    policy.check(
        "output_polygons",
        policy.max_output_polygons,
        next_polygon_count,
    )?;
    policy.check(
        "output_coordinates",
        policy.max_output_coordinates,
        next_coordinate_count,
    )?;
    *polygon_count = next_polygon_count;
    *coordinate_count = next_coordinate_count;
    Ok(())
}

fn merge_duplicate_polygon_provenance(
    retained: &mut Polygon3D,
    duplicate: &Polygon3D,
    profile_conflicted: bool,
) -> bool {
    retained
        .boundary_source_line_ids
        .extend_from_slice(&duplicate.boundary_source_line_ids);
    retained.boundary_source_line_ids.sort_unstable();
    retained.boundary_source_line_ids.dedup();

    match (&mut retained.provenance, &duplicate.provenance) {
        (Some(retained_provenance), Some(duplicate_provenance)) => {
            retained_provenance
                .boundary_line_ids
                .extend_from_slice(&duplicate_provenance.boundary_line_ids);
            retained_provenance.boundary_line_ids.sort_unstable();
            retained_provenance.boundary_line_ids.dedup();

            if !profile_conflicted {
                let conflicting_profiles = matches!(
                    (
                        &retained_provenance.input_profile_id,
                        &duplicate_provenance.input_profile_id
                    ),
                    (Some(retained), Some(duplicate)) if retained != duplicate
                );
                if conflicting_profiles {
                    retained_provenance.input_profile_id = None;
                    return true;
                } else if retained_provenance.input_profile_id.is_none() {
                    retained_provenance.input_profile_id =
                        duplicate_provenance.input_profile_id.clone();
                }
            }
        }
        (None, Some(duplicate_provenance)) if !profile_conflicted => {
            retained.provenance = Some(duplicate_provenance.clone());
        }
        _ => {}
    }

    // The first exact duplicate remains the deterministic representative for
    // per-edge IDs; aggregate source sets above retain all provenance evidence.
    profile_conflicted
}

#[derive(Clone, Copy, Debug)]
struct InputSegment {
    line: Line<f64>,
    geometry_index: usize,
}

fn line_string_segments(
    line: &LineString<f64>,
    geometry_index: usize,
    segments: &mut Vec<InputSegment>,
    execution_policy: &ExecutionPolicy,
) -> Result<()> {
    for line in line.lines() {
        let observed = segments.len().checked_add(1).ok_or_else(|| {
            PolygonizeError::InternalInvariantViolation {
                reason: "tiled component segment counter overflow".to_string(),
            }
        })?;
        execution_policy.check(
            "input_segments",
            execution_policy.max_input_segments,
            observed,
        )?;
        execution_policy.check_cancelled_every("tile_component_preflight", observed)?;
        segments.push(InputSegment {
            line,
            geometry_index,
        });
    }
    Ok(())
}

fn geometry_segments(
    geometry: &Geometry<f64>,
    geometry_index: usize,
    segments: &mut Vec<InputSegment>,
    execution_policy: &ExecutionPolicy,
) -> Result<()> {
    match geometry {
        Geometry::LineString(line) => {
            line_string_segments(line, geometry_index, segments, execution_policy)?
        }
        Geometry::MultiLineString(lines) => {
            for line in lines {
                line_string_segments(line, geometry_index, segments, execution_policy)?;
            }
        }
        Geometry::Polygon(polygon) => {
            line_string_segments(
                polygon.exterior(),
                geometry_index,
                segments,
                execution_policy,
            )?;
            for ring in polygon.interiors() {
                line_string_segments(ring, geometry_index, segments, execution_policy)?;
            }
        }
        Geometry::MultiPolygon(polygons) => {
            for polygon in polygons {
                line_string_segments(
                    polygon.exterior(),
                    geometry_index,
                    segments,
                    execution_policy,
                )?;
                for ring in polygon.interiors() {
                    line_string_segments(ring, geometry_index, segments, execution_policy)?;
                }
            }
        }
        Geometry::GeometryCollection(collection) => {
            for member in collection {
                geometry_segments(member, geometry_index, segments, execution_policy)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn line_string_has_closed_boundary(line: &LineString<f64>) -> bool {
    let Some((first, last)) = line.0.first().zip(line.0.last()) else {
        return false;
    };
    line.0.len() >= 4 && first.x == last.x && first.y == last.y
}

fn geometry_has_closed_boundary(geometry: &Geometry<f64>) -> bool {
    match geometry {
        Geometry::LineString(line) => line_string_has_closed_boundary(line),
        Geometry::MultiLineString(lines) => {
            !lines.0.is_empty() && lines.0.iter().all(line_string_has_closed_boundary)
        }
        Geometry::Polygon(polygon) => {
            line_string_has_closed_boundary(polygon.exterior())
                && polygon
                    .interiors()
                    .iter()
                    .all(line_string_has_closed_boundary)
        }
        Geometry::MultiPolygon(polygons) => {
            !polygons.0.is_empty()
                && polygons.iter().all(|polygon| {
                    line_string_has_closed_boundary(polygon.exterior())
                        && polygon
                            .interiors()
                            .iter()
                            .all(line_string_has_closed_boundary)
                })
        }
        Geometry::GeometryCollection(collection) => {
            !collection.0.is_empty() && collection.0.iter().all(geometry_has_closed_boundary)
        }
        _ => false,
    }
}

fn component_root(parents: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = index;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn join_components(parents: &mut [usize], left: usize, right: usize) {
    let left = component_root(parents, left);
    let right = component_root(parents, right);
    if left != right {
        parents[left.max(right)] = left.min(right);
    }
}

fn expand_rect(rect: &mut Rect<f64>, other: Rect<f64>) {
    *rect = Rect::new(
        Coord {
            x: rect.min().x.min(other.min().x),
            y: rect.min().y.min(other.min().y),
        },
        Coord {
            x: rect.max().x.max(other.max().x),
            y: rect.max().y.max(other.max().y),
        },
    );
}

/// Experimental tiled polygonization.
///
/// Equivalence with untiled output is not guaranteed.
pub struct TiledPolygonizer<'a> {
    bbox: Rect<f64>,
    tile_size: f64,
    buffer: f64, // Overlap buffer to ensure polygons are fully captured
    geometries: Vec<(&'a Geometry<f64>, Option<Rect<f64>>)>,
    ownership_policy: TileOwnershipPolicy,
    dedup_policy: DedupPolicy,
    options: PolygonizerOptions,
    execution_policy: ExecutionPolicy,
    tile_execution_policy: TileExecutionPolicy,
    retry_policy: Option<TileRetryPolicy>,
    component_fallback: bool,
    untiled_fallback: bool,
    untiled_equivalence_check: bool,
}

impl<'a> TiledPolygonizer<'a> {
    pub fn new(bbox: Rect<f64>, tile_size: f64) -> Self {
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        Self {
            bbox,
            tile_size,
            buffer: 0.0,
            geometries: Vec::new(),
            ownership_policy: TileOwnershipPolicy::Centroid,
            dedup_policy: DedupPolicy::KeepAll,
            options,
            execution_policy: ExecutionPolicy::default(),
            tile_execution_policy: TileExecutionPolicy::default(),
            retry_policy: None,
            component_fallback: false,
            untiled_fallback: false,
            untiled_equivalence_check: false,
        }
    }

    pub fn with_buffer(mut self, buffer: f64) -> Self {
        self.buffer = buffer;
        self
    }

    pub fn with_ownership_policy(mut self, policy: TileOwnershipPolicy) -> Self {
        self.ownership_policy = policy;
        self
    }

    pub fn with_dedup_policy(mut self, policy: DedupPolicy) -> Self {
        self.dedup_policy = policy;
        self
    }

    pub fn with_options(mut self, options: PolygonizerOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets non-semantic limits for the component preflight and each tile polygonization.
    pub fn with_execution_policy(mut self, execution_policy: ExecutionPolicy) -> Self {
        self.execution_policy = execution_policy;
        self
    }

    pub fn with_tile_execution_policy(
        mut self,
        tile_execution_policy: TileExecutionPolicy,
    ) -> Self {
        self.tile_execution_policy = tile_execution_policy;
        self
    }

    pub fn with_retry_policy(mut self, retry_policy: TileRetryPolicy) -> Self {
        self.retry_policy = Some(retry_policy);
        self
    }

    /// Enables conservative recovery for excluded components and interacting
    /// retained regions.
    pub fn with_component_fallback(mut self) -> Self {
        self.component_fallback = true;
        self
    }

    /// Replaces unresolved tiled output with one global untiled pass.
    ///
    /// The global pass preserves containment relationships that cannot be
    /// recovered by appending independently polygonized components.
    pub fn with_untiled_fallback(mut self) -> Self {
        self.untiled_fallback = true;
        self
    }

    /// Enables an experimental full canonical comparison of validated
    /// stitched output against one same-options untiled pass.
    pub fn with_untiled_equivalence_check(mut self) -> Self {
        self.untiled_equivalence_check = true;
        self
    }

    pub fn add_geometry(&mut self, geom: &'a Geometry<f64>) {
        let bbox = geom.bounding_rect();
        self.geometries.push((geom, bbox));
    }

    fn buffered_bbox(&self, tile_bbox: Rect<f64>, buffer: f64) -> Rect<f64> {
        Rect::new(
            Coord {
                x: tile_bbox.min().x - buffer,
                y: tile_bbox.min().y - buffer,
            },
            Coord {
                x: tile_bbox.max().x + buffer,
                y: tile_bbox.max().y + buffer,
            },
        )
    }

    #[cfg(test)]
    fn process_one_partition(
        &self,
        partition_id: usize,
        tile_bbox: Rect<f64>,
        buffer: f64,
    ) -> Result<PartitionSnapshotV1> {
        self.execution_policy.check_cancelled("partition_oracle")?;
        let buffered_bbox = self.buffered_bbox(tile_bbox, buffer);
        let mut local_poly = Polygonizer::with_options(self.options.clone())
            .with_execution_policy(self.execution_policy.clone());
        let mut selected_input_geometry_indices = Vec::new();
        for (input_geometry_index, (geometry, bbox)) in self.geometries.iter().enumerate() {
            self.execution_policy
                .check_cancelled_every("partition_oracle", input_geometry_index)?;
            if bbox
                .as_ref()
                .is_some_and(|geometry_bbox| geometry_bbox.intersects(&buffered_bbox))
            {
                local_poly.add_borrowed_geometry(geometry);
                selected_input_geometry_indices.push(input_geometry_index);
            }
        }
        let selected_source_segments =
            partition_source_segments(&self.geometries, &selected_input_geometry_indices)?;
        if selected_input_geometry_indices.is_empty() {
            return PartitionSnapshotV1::from_result(
                partition_id,
                tile_bbox,
                selected_input_geometry_indices,
                selected_source_segments,
                crate::graph::planar_graph::PartitionBoundaryNodingStats::default(),
                &[],
                &[],
                &[],
                &[],
                &PolygonizerResult {
                    polygons: Vec::new(),
                    dangles: Vec::new(),
                    cut_edges: Vec::new(),
                    invalid_rings: Vec::new(),
                    diagnostics: None,
                },
                &self.options,
            );
        }
        let (
            result,
            border_observations,
            local_face_graphs,
            boundary_noding_stats,
            noded_segments,
            boundary_noded_segments,
        ) = local_poly
            .polygonize_with_partition_border_export_and_stats(partition_id, tile_bbox)?;
        PartitionSnapshotV1::from_result(
            partition_id,
            tile_bbox,
            selected_input_geometry_indices,
            selected_source_segments,
            boundary_noding_stats,
            &noded_segments,
            &boundary_noded_segments,
            &border_observations,
            &local_face_graphs,
            &result,
            &self.options,
        )
    }

    fn process_tile(
        &self,
        partition_id: usize,
        tile_bbox: Rect<f64>,
        input_components: &[InputComponent],
        buffer: f64,
        capture_byte_limit: Option<usize>,
    ) -> Result<TileProcessResult> {
        self.execution_policy.check_cancelled("tile_processing")?;
        let mut capture_budget = capture_byte_limit.map(TraceCaptureBudget::new);
        let mut local_poly = Polygonizer::with_options(self.options.clone())
            .with_execution_policy(self.execution_policy.clone());

        // Define buffered bbox
        let buffered_bbox = self.buffered_bbox(tile_bbox, buffer);

        // Filter geometries intersecting the BUFFERED tile
        let mut relevant_lines = 0;
        let mut selected_input_geometry_indices = Vec::new();
        let mut input_boundary_issues = Vec::new();
        for (input_geometry_index, (geom, bbox)) in self.geometries.iter().enumerate() {
            self.execution_policy
                .check_cancelled_every("tile_processing", input_geometry_index)?;
            if let Some(geometry_bbox) = bbox
                .as_ref()
                .filter(|geometry_bbox| geometry_bbox.intersects(&buffered_bbox))
            {
                local_poly.add_borrowed_geometry(geom);
                relevant_lines += 1;
                selected_input_geometry_indices.push(input_geometry_index);
                let unresolved_sides = self.unresolved_sides(*geometry_bbox, buffered_bbox);
                if !unresolved_sides.is_empty() {
                    input_boundary_issues.push(TileInputBoundaryIssue {
                        input_geometry_index,
                        geometry_bbox: *geometry_bbox,
                        unresolved_sides,
                    });
                }
            }
        }
        let selected_source_segments =
            partition_source_segments(&self.geometries, &selected_input_geometry_indices)?;
        let input_boundary_geometry_indices = input_boundary_issues
            .iter()
            .map(|issue| issue.input_geometry_index)
            .collect::<HashSet<_>>();
        let mut excluded_component_issues = Vec::new();
        for (component_index, component) in input_components.iter().enumerate() {
            self.execution_policy
                .check_cancelled_every("tile_processing", component_index)?;
            if !component.bbox.intersects(&buffered_bbox) {
                continue;
            }
            let mut has_unobserved_member = false;
            for (member_index, &index) in component.input_geometry_indices.iter().enumerate() {
                self.execution_policy
                    .check_cancelled_every("tile_processing", member_index)?;
                has_unobserved_member |= self.geometries[index]
                    .1
                    .is_none_or(|bbox| !bbox.intersects(&buffered_bbox));
            }
            if has_unobserved_member
                && component
                    .input_geometry_indices
                    .iter()
                    .all(|index| !input_boundary_geometry_indices.contains(index))
            {
                excluded_component_issues.push(TileExcludedComponentIssue {
                    input_geometry_indices: component.input_geometry_indices.clone(),
                    component_bbox: component.bbox,
                    connection: component.connection,
                });
            }
        }

        let mut report = TileReport {
            tile_bbox,
            input_geometry_count: relevant_lines,
            polygon_count: 0,
            owned_polygon_count: 0,
            dangle_count: 0,
            cut_edge_count: 0,
            invalid_ring_count: 0,
            coverage_issues: Vec::new(),
            ownership_domain_issues: Vec::new(),
            input_boundary_issues,
            excluded_component_issues,
            retry_attempts: Vec::new(),
            retry_exhausted: false,
        };
        if relevant_lines == 0 {
            let empty_result = PolygonizerResult {
                polygons: Vec::new(),
                dangles: Vec::new(),
                cut_edges: Vec::new(),
                invalid_rings: Vec::new(),
                diagnostics: None,
            };
            let snapshot = PartitionSnapshotV1::from_result(
                partition_id,
                tile_bbox,
                selected_input_geometry_indices,
                selected_source_segments,
                crate::graph::planar_graph::PartitionBoundaryNodingStats::default(),
                &[],
                &[],
                &[],
                &[],
                &empty_result,
                &self.options,
            )?;
            return Ok((
                Vec::new(),
                report,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                false,
                crate::graph::planar_graph::PartitionBoundaryNodingStats::default(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                snapshot,
            ));
        }

        // Run polygonization
        let (
            result,
            border_observations,
            local_face_graphs,
            boundary_noding_stats,
            noded_segments,
            boundary_noded_segments,
        ) = local_poly
            .polygonize_with_partition_border_export_and_stats(partition_id, tile_bbox)?;
        let snapshot = PartitionSnapshotV1::from_result(
            partition_id,
            tile_bbox,
            selected_input_geometry_indices,
            selected_source_segments,
            boundary_noding_stats,
            &noded_segments,
            &boundary_noded_segments,
            &border_observations,
            &local_face_graphs,
            &result,
            &self.options,
        )?;
        let dangles = result.dangles;
        let cut_edges = result.cut_edges;
        let invalid_rings = result.invalid_rings;
        report.polygon_count = result.polygons.len();
        report.dangle_count = dangles.len();
        report.cut_edge_count = cut_edges.len();
        report.invalid_ring_count = invalid_rings.len();
        // Ownership check:
        let mut valid_polys = Vec::new();
        let mut ownership_decisions = Vec::new();
        for (polygon_index, poly) in result.polygons.into_iter().enumerate() {
            self.execution_policy
                .check_cancelled_every("tile_processing", polygon_index)?;
            let ownership_point = self.ownership_point(&poly);
            let owned = ownership_point.is_some_and(|c| {
                // Check inclusion [min, max)
                // For the last tile in a row/col, we include the max boundary to cover the full bbox.
                let max_x_inclusive = tile_bbox.max().x >= self.bbox.max().x;
                let max_y_inclusive = tile_bbox.max().y >= self.bbox.max().y;

                let in_x = if max_x_inclusive {
                    c.x() >= tile_bbox.min().x && c.x() <= tile_bbox.max().x
                } else {
                    c.x() >= tile_bbox.min().x && c.x() < tile_bbox.max().x
                };

                let in_y = if max_y_inclusive {
                    c.y() >= tile_bbox.min().y && c.y() <= tile_bbox.max().y
                } else {
                    c.y() >= tile_bbox.min().y && c.y() < tile_bbox.max().y
                };
                in_x && in_y
            });
            if !owned {
                if let Some(ownership_point) = ownership_point {
                    if let Some(polygon_bbox) = Self::polygon_bbox(&poly) {
                        let point_in_domain = ownership_point.x() >= self.bbox.min().x
                            && ownership_point.x() <= self.bbox.max().x
                            && ownership_point.y() >= self.bbox.min().y
                            && ownership_point.y() <= self.bbox.max().y;
                        if !point_in_domain && polygon_bbox.intersects(&self.bbox) {
                            report
                                .ownership_domain_issues
                                .push(TileOwnershipDomainIssue {
                                    polygon_index,
                                    polygon_bbox,
                                    ownership_point: Coord3D::new(
                                        ownership_point.x(),
                                        ownership_point.y(),
                                        0.0,
                                    ),
                                });
                        }
                    }
                }
            }
            if let Some(budget) = capture_budget.as_mut() {
                budget.capture(
                    &mut ownership_decisions,
                    (
                        polygon_index,
                        ownership_point.map(|point| Coord3D::new(point.x(), point.y(), 0.0)),
                        owned,
                    ),
                );
            }
            if owned {
                if let Some(issue) = self.coverage_issue(polygon_index, &poly, buffered_bbox) {
                    report.coverage_issues.push(issue);
                }
                valid_polys.push(poly);
            }
        }
        report.owned_polygon_count = valid_polys.len();
        Ok((
            valid_polys,
            report,
            ownership_decisions,
            border_observations,
            local_face_graphs,
            capture_budget.is_some_and(|budget| budget.truncated()),
            boundary_noding_stats,
            dangles,
            cut_edges,
            invalid_rings,
            noded_segments,
            boundary_noded_segments,
            snapshot,
        ))
    }

    fn report_is_unresolved(report: &TileReport) -> bool {
        !report.coverage_issues.is_empty()
            || !report.ownership_domain_issues.is_empty()
            || !report.input_boundary_issues.is_empty()
            || !report.excluded_component_issues.is_empty()
    }

    fn report_has_retry_evidence(report: &TileReport) -> bool {
        !report.coverage_issues.is_empty()
            || !report.input_boundary_issues.is_empty()
            || !report.excluded_component_issues.is_empty()
    }

    fn try_closed_boundary_fallback(
        &self,
        tile_polygons: &[Vec<Polygon3D>],
        tile_reports: &[TileReport],
    ) -> Result<ComponentFallbackDecision> {
        self.execution_policy
            .check_cancelled("tile_component_fallback")?;
        let Some(mut region_bbox) = tile_reports
            .iter()
            .flat_map(|report| {
                report
                    .coverage_issues
                    .iter()
                    .map(|issue| issue.polygon_bbox)
                    .chain(
                        report
                            .input_boundary_issues
                            .iter()
                            .map(|issue| issue.geometry_bbox),
                    )
            })
            .next()
        else {
            return Ok(ComponentFallbackDecision::Declined(
                ComponentFallbackDeclineReason::NoIndexedComponentEvidence,
            ));
        };
        for bbox in tile_reports.iter().flat_map(|report| {
            report
                .coverage_issues
                .iter()
                .map(|issue| issue.polygon_bbox)
                .chain(
                    report
                        .input_boundary_issues
                        .iter()
                        .map(|issue| issue.geometry_bbox),
                )
        }) {
            expand_rect(&mut region_bbox, bbox);
        }

        let retained_polygon_bboxes = tile_polygons
            .iter()
            .flat_map(|polygons| polygons.iter())
            .filter_map(Self::polygon_bbox)
            .collect::<Vec<_>>();
        let mut selected_geometry_indices = HashSet::new();
        loop {
            self.execution_policy
                .check_cancelled("tile_component_fallback")?;
            let previous_region_bbox = region_bbox;
            for (geometry_index, (geometry, geometry_bbox)) in self.geometries.iter().enumerate() {
                self.execution_policy
                    .check_cancelled_every("tile_component_fallback", geometry_index)?;
                let Some(geometry_bbox) = *geometry_bbox else {
                    continue;
                };
                if selected_geometry_indices.contains(&geometry_index)
                    || !geometry_bbox.intersects(&region_bbox)
                {
                    continue;
                }
                if !geometry_has_closed_boundary(geometry) {
                    return Ok(ComponentFallbackDecision::Declined(
                        ComponentFallbackDeclineReason::NonClosedRecoveryRegion,
                    ));
                }
                selected_geometry_indices.insert(geometry_index);
                expand_rect(&mut region_bbox, geometry_bbox);
            }
            for (polygon_index, polygon_bbox) in retained_polygon_bboxes.iter().enumerate() {
                self.execution_policy
                    .check_cancelled_every("tile_component_fallback", polygon_index)?;
                if polygon_bbox.intersects(&region_bbox) {
                    expand_rect(&mut region_bbox, *polygon_bbox);
                }
            }
            if region_bbox == previous_region_bbox {
                break;
            }
        }

        if selected_geometry_indices.is_empty()
            || tile_reports.iter().any(|report| {
                report
                    .input_boundary_issues
                    .iter()
                    .any(|issue| !selected_geometry_indices.contains(&issue.input_geometry_index))
            })
        {
            return Ok(ComponentFallbackDecision::Declined(
                ComponentFallbackDeclineReason::InputBoundaryOutsideRecoveryRegion,
            ));
        }

        let mut input_geometry_indices = selected_geometry_indices.into_iter().collect::<Vec<_>>();
        input_geometry_indices.sort_unstable();
        self.execution_policy
            .check_cancelled("tile_component_fallback")?;
        let mut polygonizer = Polygonizer::with_options(self.options.clone())
            .with_execution_policy(self.execution_policy.clone());
        for &geometry_index in &input_geometry_indices {
            polygonizer.add_borrowed_geometry(self.geometries[geometry_index].0);
        }
        let polygons = polygonizer.polygonize()?.polygons;
        if polygons.is_empty() {
            return Ok(ComponentFallbackDecision::Declined(
                ComponentFallbackDeclineReason::EmptyRecoveryOutput,
            ));
        }
        let output_polygon_count = polygons.len();
        let replaced_retained_polygon_count = retained_polygon_bboxes
            .iter()
            .filter(|polygon_bbox| polygon_bbox.intersects(&region_bbox))
            .count();
        Ok(ComponentFallbackDecision::Recovered(
            ComponentFallbackResult {
                polygons,
                events: vec![ComponentFallbackEvent {
                    input_geometry_indices,
                    output_polygon_count,
                    recovered_component_count: 1,
                    replaced_retained_polygon_count,
                }],
                region_bboxes: vec![region_bbox],
            },
        ))
    }

    fn try_component_fallback(
        &self,
        tile_polygons: &[Vec<Polygon3D>],
        tile_reports: &[TileReport],
        input_components: &[InputComponent],
    ) -> Result<ComponentFallbackDecision> {
        self.execution_policy
            .check_cancelled("tile_component_fallback")?;
        let mut component_keys = tile_reports
            .iter()
            .flat_map(|report| &report.excluded_component_issues)
            .map(|issue| issue.input_geometry_indices.clone())
            .collect::<HashSet<_>>();
        let input_boundary_geometry_indices = tile_reports
            .iter()
            .flat_map(|report| &report.input_boundary_issues)
            .map(|issue| issue.input_geometry_index)
            .collect::<HashSet<_>>();
        for (component_index, component) in input_components.iter().enumerate() {
            self.execution_policy
                .check_cancelled_every("tile_component_fallback", component_index)?;
            if component
                .input_geometry_indices
                .iter()
                .any(|index| input_boundary_geometry_indices.contains(index))
            {
                component_keys.insert(component.input_geometry_indices.clone());
            }
        }
        let has_coverage_evidence = tile_reports
            .iter()
            .any(|report| !report.coverage_issues.is_empty());
        let has_input_boundary_evidence = tile_reports
            .iter()
            .any(|report| !report.input_boundary_issues.is_empty());
        if component_keys.is_empty() {
            if has_coverage_evidence || has_input_boundary_evidence {
                return self.try_closed_boundary_fallback(tile_polygons, tile_reports);
            }
            return Ok(ComponentFallbackDecision::Declined(
                ComponentFallbackDeclineReason::NoIndexedComponentEvidence,
            ));
        }
        let retained_polygon_bboxes = tile_polygons
            .iter()
            .flat_map(|polygons| polygons.iter())
            .filter_map(Self::polygon_bbox)
            .collect::<Vec<_>>();

        let mut assigned_components = HashSet::new();
        let mut regions = Vec::new();
        let seed_component_count = input_components
            .iter()
            .filter(|component| component_keys.contains(&component.input_geometry_indices))
            .count();
        if seed_component_count != component_keys.len() {
            return Ok(ComponentFallbackDecision::Declined(
                ComponentFallbackDeclineReason::IndexedComponentMissing,
            ));
        }
        for (component_index, seed) in input_components.iter().enumerate() {
            if !component_keys.contains(&seed.input_geometry_indices)
                || !assigned_components.insert(component_index)
            {
                continue;
            }
            let mut region_bbox = seed.bbox;
            let mut region_geometry_indices = seed.input_geometry_indices.clone();
            let mut region_component_count = 1;
            let mut selected_geometry_indices = region_geometry_indices
                .iter()
                .copied()
                .collect::<HashSet<_>>();

            loop {
                self.execution_policy
                    .check_cancelled("tile_component_fallback")?;
                let mut expanded = false;

                for polygon_bbox in &retained_polygon_bboxes {
                    if polygon_bbox.intersects(&region_bbox) {
                        let previous = region_bbox;
                        expand_rect(&mut region_bbox, *polygon_bbox);
                        expanded |= region_bbox != previous;
                    }
                }

                for (geometry_index, (_, geometry_bbox)) in self.geometries.iter().enumerate() {
                    self.execution_policy
                        .check_cancelled_every("tile_component_fallback", geometry_index)?;
                    if selected_geometry_indices.contains(&geometry_index) {
                        continue;
                    }
                    if geometry_bbox.is_some_and(|bbox| bbox.intersects(&region_bbox)) {
                        selected_geometry_indices.insert(geometry_index);
                        region_geometry_indices.push(geometry_index);
                        if let Some(geometry_bbox) = geometry_bbox {
                            expand_rect(&mut region_bbox, *geometry_bbox);
                        }
                        expanded = true;
                    }
                }

                for (candidate_index, candidate) in input_components.iter().enumerate() {
                    self.execution_policy
                        .check_cancelled_every("tile_component_fallback", candidate_index)?;
                    if assigned_components.contains(&candidate_index)
                        || !candidate.bbox.intersects(&region_bbox)
                    {
                        continue;
                    }
                    assigned_components.insert(candidate_index);
                    region_component_count += 1;
                    for &geometry_index in &candidate.input_geometry_indices {
                        if selected_geometry_indices.insert(geometry_index) {
                            region_geometry_indices.push(geometry_index);
                        }
                    }
                    expand_rect(&mut region_bbox, candidate.bbox);
                    expanded = true;
                }

                if !expanded {
                    break;
                }
            }

            region_geometry_indices.sort_unstable();
            regions.push((region_geometry_indices, region_bbox, region_component_count));
        }
        let recoverable_geometry_indices = regions
            .iter()
            .flat_map(|(indices, _, _)| indices.iter().copied())
            .collect::<HashSet<_>>();
        if tile_reports.iter().any(|report| {
            report
                .input_boundary_issues
                .iter()
                .any(|issue| !recoverable_geometry_indices.contains(&issue.input_geometry_index))
        }) {
            return Ok(ComponentFallbackDecision::Declined(
                ComponentFallbackDeclineReason::InputBoundaryOutsideRecoveryRegion,
            ));
        }
        if has_coverage_evidence {
            let mut coverage_region_closed = true;
            'coverage: for report in tile_reports {
                for issue in &report.coverage_issues {
                    self.execution_policy
                        .check_cancelled("tile_component_fallback")?;
                    if !regions
                        .iter()
                        .any(|(_, region_bbox, _)| issue.polygon_bbox.intersects(region_bbox))
                    {
                        coverage_region_closed = false;
                        break 'coverage;
                    }
                }
            }
            if !coverage_region_closed {
                return Ok(ComponentFallbackDecision::Declined(
                    ComponentFallbackDeclineReason::OwnedFaceCoverageEvidence,
                ));
            }
        }

        let mut recovered = Vec::new();
        let mut events = Vec::with_capacity(regions.len());
        let mut region_bboxes = Vec::with_capacity(regions.len());
        for (input_geometry_indices, region_bbox, recovered_component_count) in regions {
            self.execution_policy
                .check_cancelled("tile_component_fallback")?;
            let mut polygonizer = Polygonizer::with_options(self.options.clone())
                .with_execution_policy(self.execution_policy.clone());
            for &geometry_index in &input_geometry_indices {
                polygonizer.add_borrowed_geometry(self.geometries[geometry_index].0);
            }
            let polygons = polygonizer.polygonize()?.polygons;
            if polygons.is_empty() {
                return Ok(ComponentFallbackDecision::Declined(
                    ComponentFallbackDeclineReason::EmptyRecoveryOutput,
                ));
            }
            let replaced_retained_polygon_count = retained_polygon_bboxes
                .iter()
                .filter(|polygon_bbox| polygon_bbox.intersects(&region_bbox))
                .count();
            events.push(ComponentFallbackEvent {
                input_geometry_indices,
                output_polygon_count: polygons.len(),
                recovered_component_count,
                replaced_retained_polygon_count,
            });
            region_bboxes.push(region_bbox);
            recovered.extend(polygons);
        }
        Ok(ComponentFallbackDecision::Recovered(
            ComponentFallbackResult {
                polygons: recovered,
                events,
                region_bboxes,
            },
        ))
    }

    fn polygon_bbox(poly: &Polygon3D) -> Option<Rect<f64>> {
        let first = poly.exterior.first()?;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.x, first.y, first.x, first.y);
        for coordinate in &poly.exterior[1..] {
            min_x = min_x.min(coordinate.x);
            min_y = min_y.min(coordinate.y);
            max_x = max_x.max(coordinate.x);
            max_y = max_y.max(coordinate.y);
        }
        Some(Rect::new(
            Coord { x: min_x, y: min_y },
            Coord { x: max_x, y: max_y },
        ))
    }

    fn merge_fallback_polygons(
        &self,
        tile_polygons: Vec<Vec<Polygon3D>>,
        region_bboxes: &[Rect<f64>],
        component_fallback_polygons: Vec<Polygon3D>,
    ) -> Result<Vec<Polygon3D>> {
        self.execution_policy
            .check_cancelled("tile_fallback_merge")?;
        let mut result = Vec::new();
        let mut output_polygon_count = 0;
        let mut output_coordinate_count = 0;
        let mut work_items = 0;
        for polygons in tile_polygons {
            for polygon in polygons {
                self.execution_policy
                    .check_cancelled_every("tile_fallback_merge", work_items)?;
                work_items = work_items.saturating_add(1);
                let polygon_bbox = Self::polygon_bbox(&polygon);
                let mut replaced = false;
                for region_bbox in region_bboxes {
                    self.execution_policy
                        .check_cancelled_every("tile_fallback_merge", work_items)?;
                    work_items = work_items.saturating_add(1);
                    if polygon_bbox.is_some_and(|polygon_bbox| polygon_bbox.intersects(region_bbox))
                    {
                        replaced = true;
                        break;
                    }
                }
                if !replaced {
                    account_polygon_output(
                        &self.execution_policy,
                        &mut output_polygon_count,
                        &mut output_coordinate_count,
                        &polygon,
                    )?;
                    result.push(polygon);
                }
            }
        }
        for polygon in component_fallback_polygons {
            self.execution_policy
                .check_cancelled_every("tile_fallback_merge", work_items)?;
            work_items = work_items.saturating_add(1);
            account_polygon_output(
                &self.execution_policy,
                &mut output_polygon_count,
                &mut output_coordinate_count,
                &polygon,
            )?;
            result.push(polygon);
        }
        Ok(result)
    }

    fn process_tile_with_retries(
        &self,
        partition_id: usize,
        tile_bbox: Rect<f64>,
        input_components: &[InputComponent],
        capture_byte_limit: Option<usize>,
        retry_attempt_counter: &AtomicUsize,
    ) -> Result<TileProcessResult> {
        let mut buffer = self.buffer;
        let mut result = self.process_tile(
            partition_id,
            tile_bbox,
            input_components,
            buffer,
            capture_byte_limit,
        )?;
        let Some(policy) = self.retry_policy else {
            return Ok(result);
        };
        let mut retry_attempts = Vec::new();
        let mut capture_truncated = result.5;
        for attempt in 1..=policy.max_attempts {
            if !Self::report_has_retry_evidence(&result.1) || buffer >= policy.max_buffer {
                break;
            }
            self.execution_policy.check(
                "tile_retry_attempts",
                self.execution_policy.max_tile_retry_attempts,
                attempt,
            )?;
            let observed_attempts = retry_attempt_counter
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |attempts| {
                    attempts.checked_add(1)
                })
                .map_err(|_| PolygonizeError::InternalInvariantViolation {
                    reason: "tiled retry attempt counter overflow".to_string(),
                })?
                .checked_add(1)
                .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                    reason: "tiled retry attempt counter overflow".to_string(),
                })?;
            self.execution_policy.check(
                "tile_retry_attempts_total",
                self.tile_execution_policy.max_retry_attempts_total,
                observed_attempts,
            )?;
            buffer = (buffer + policy.buffer_increment).min(policy.max_buffer);
            result = self.process_tile(
                partition_id,
                tile_bbox,
                input_components,
                buffer,
                capture_byte_limit,
            )?;
            capture_truncated |= result.5;
            let resolved = !Self::report_is_unresolved(&result.1);
            retry_attempts.push(TileRetryAttempt {
                attempt,
                buffer,
                unresolved_owned_polygon_count: result.1.coverage_issues.len(),
                unresolved_input_geometry_count: result.1.input_boundary_issues.len(),
                unresolved_component_count: result.1.excluded_component_issues.len(),
                unresolved_ownership_domain_count: result.1.ownership_domain_issues.len(),
                resolved,
            });
        }
        result.1.retry_exhausted = Self::report_has_retry_evidence(&result.1);
        result.1.retry_attempts = retry_attempts;
        result.5 = capture_truncated;
        Ok(result)
    }

    fn ownership_point(&self, poly: &Polygon3D) -> Option<Point<f64>> {
        match self.ownership_policy {
            TileOwnershipPolicy::Centroid => poly.centroid_2d(),
            TileOwnershipPolicy::RepresentativePointInsidePolygon => {
                poly.to_polygon_2d().interior_point()
            }
            TileOwnershipPolicy::LexicographicMinVertex => poly
                .exterior
                .iter()
                .min_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)))
                .map(|coord| Point::new(coord.x, coord.y)),
        }
    }

    fn coverage_issue(
        &self,
        polygon_index: usize,
        poly: &Polygon3D,
        buffered_bbox: Rect<f64>,
    ) -> Option<TileCoverageIssue> {
        let first = poly.exterior.first()?;
        let mut min_x = first.x;
        let mut min_y = first.y;
        let mut max_x = first.x;
        let mut max_y = first.y;
        for coordinate in &poly.exterior[1..] {
            min_x = min_x.min(coordinate.x);
            min_y = min_y.min(coordinate.y);
            max_x = max_x.max(coordinate.x);
            max_y = max_y.max(coordinate.y);
        }
        let polygon_bbox = Rect::new(Coord { x: min_x, y: min_y }, Coord { x: max_x, y: max_y });
        let unresolved_sides = self.unresolved_sides(polygon_bbox, buffered_bbox);
        if unresolved_sides.is_empty() {
            return None;
        }
        let mut representative_source_line_ids = poly
            .exterior_ids
            .iter()
            .chain(poly.interiors_ids.iter().flatten())
            .copied()
            .collect::<Vec<_>>();
        representative_source_line_ids.sort_unstable();
        representative_source_line_ids.dedup();
        Some(TileCoverageIssue {
            polygon_index,
            polygon_bbox,
            unresolved_sides,
            representative_source_line_ids,
            aggregate_source_line_ids: poly.boundary_source_line_ids.clone(),
            aggregate_source_line_ids_complete: self.options.provenance.enabled
                && self.options.provenance.include_boundary_line_ids,
        })
    }

    fn unresolved_sides(
        &self,
        geometry_bbox: Rect<f64>,
        buffered_bbox: Rect<f64>,
    ) -> Vec<TileBoundarySide> {
        let mut unresolved_sides = Vec::new();
        if buffered_bbox.min().x > self.bbox.min().x
            && geometry_bbox.min().x <= buffered_bbox.min().x
        {
            unresolved_sides.push(TileBoundarySide::MinX);
        }
        if buffered_bbox.max().x < self.bbox.max().x
            && geometry_bbox.max().x >= buffered_bbox.max().x
        {
            unresolved_sides.push(TileBoundarySide::MaxX);
        }
        if buffered_bbox.min().y > self.bbox.min().y
            && geometry_bbox.min().y <= buffered_bbox.min().y
        {
            unresolved_sides.push(TileBoundarySide::MinY);
        }
        if buffered_bbox.max().y < self.bbox.max().y
            && geometry_bbox.max().y >= buffered_bbox.max().y
        {
            unresolved_sides.push(TileBoundarySide::MaxY);
        }
        unresolved_sides
    }

    fn declare_partition_adjacencies(
        &self,
        graph: &mut PartitionBorderGraph,
        reports: &[TileReport],
    ) -> Result<()> {
        for (first_partition_id, first) in reports.iter().enumerate() {
            for (second_partition_id, second) in
                reports.iter().enumerate().skip(first_partition_id + 1)
            {
                let first_bbox = first.tile_bbox;
                let second_bbox = second.tile_bbox;
                let y_overlap = first_bbox.min().y < second_bbox.max().y
                    && second_bbox.min().y < first_bbox.max().y;
                let x_overlap = first_bbox.min().x < second_bbox.max().x
                    && second_bbox.min().x < first_bbox.max().x;
                let adjacency = if y_overlap && first_bbox.max().x == second_bbox.min().x {
                    Some(PartitionBorderAdjacency::new(
                        first_partition_id,
                        PartitionBorderSide::MaxX,
                        second_partition_id,
                        PartitionBorderSide::MinX,
                        first_bbox.max().x,
                    )?)
                } else if y_overlap && second_bbox.max().x == first_bbox.min().x {
                    Some(PartitionBorderAdjacency::new(
                        first_partition_id,
                        PartitionBorderSide::MinX,
                        second_partition_id,
                        PartitionBorderSide::MaxX,
                        first_bbox.min().x,
                    )?)
                } else if x_overlap && first_bbox.max().y == second_bbox.min().y {
                    Some(PartitionBorderAdjacency::new(
                        first_partition_id,
                        PartitionBorderSide::MaxY,
                        second_partition_id,
                        PartitionBorderSide::MinY,
                        first_bbox.max().y,
                    )?)
                } else if x_overlap && second_bbox.max().y == first_bbox.min().y {
                    Some(PartitionBorderAdjacency::new(
                        first_partition_id,
                        PartitionBorderSide::MinY,
                        second_partition_id,
                        PartitionBorderSide::MaxY,
                        first_bbox.min().y,
                    )?)
                } else {
                    None
                };
                if let Some(adjacency) = adjacency {
                    graph.declare_adjacency(adjacency);
                }
            }
        }
        Ok(())
    }

    fn generate_tiles(&self) -> Result<Vec<Rect<f64>>> {
        let min = self.bbox.min();
        let max = self.bbox.max();
        let width = max.x - min.x;
        let height = max.y - min.y;

        let checked_axis_count = |length: f64| {
            let count = (length / self.tile_size).ceil();
            if !count.is_finite() || count >= usize::MAX as f64 {
                return Err(PolygonizeError::ResourceLimitExceeded {
                    stage: "tile_count".to_string(),
                    limit: usize::MAX - 1,
                    observed: usize::MAX,
                });
            }
            Ok(count as usize)
        };
        let cols = checked_axis_count(width)?;
        let rows = checked_axis_count(height)?;
        let tile_count =
            rows.checked_mul(cols)
                .ok_or_else(|| PolygonizeError::ResourceLimitExceeded {
                    stage: "tile_count".to_string(),
                    limit: self
                        .tile_execution_policy
                        .max_tiles
                        .unwrap_or(usize::MAX - 1),
                    observed: usize::MAX,
                })?;
        self.execution_policy.check(
            "tile_count",
            self.tile_execution_policy.max_tiles,
            tile_count,
        )?;

        let tile_rect = |r: usize, c: usize| {
            let x0 = min.x + c as f64 * self.tile_size;
            let y0 = min.y + r as f64 * self.tile_size;
            let x1 = (x0 + self.tile_size).min(max.x);
            let y1 = (y0 + self.tile_size).min(max.y);
            Rect::new(Coord { x: x0, y: y0 }, Coord { x: x1, y: y1 })
        };
        let mut assignment_count = 0usize;
        if self
            .tile_execution_policy
            .max_tile_geometry_assignments
            .is_some()
        {
            for r in 0..rows {
                for c in 0..cols {
                    self.execution_policy
                        .check_cancelled_every("tile_assignment_preflight", r * cols + c)?;
                    let buffered_bbox = self.buffered_bbox(tile_rect(r, c), self.buffer);
                    let tile_assignments = self
                        .geometries
                        .iter()
                        .filter(|(_, bbox)| {
                            bbox.is_some_and(|geometry_bbox| {
                                geometry_bbox.intersects(&buffered_bbox)
                            })
                        })
                        .count();
                    assignment_count =
                        assignment_count
                            .checked_add(tile_assignments)
                            .ok_or_else(|| PolygonizeError::ResourceLimitExceeded {
                                stage: "tile_geometry_assignments".to_string(),
                                limit: self
                                    .tile_execution_policy
                                    .max_tile_geometry_assignments
                                    .unwrap_or(usize::MAX - 1),
                                observed: usize::MAX,
                            })?;
                    self.execution_policy.check(
                        "tile_geometry_assignments",
                        self.tile_execution_policy.max_tile_geometry_assignments,
                        assignment_count,
                    )?;
                }
            }
        }

        let mut tiles = Vec::new();
        tiles.try_reserve_exact(tile_count).map_err(|_| {
            PolygonizeError::ResourceLimitExceeded {
                stage: "tile_allocation".to_string(),
                limit: tile_count,
                observed: usize::MAX,
            }
        })?;
        for r in 0..rows {
            for c in 0..cols {
                self.execution_policy
                    .check_cancelled_every("tile_generation", r * cols + c)?;
                tiles.push(tile_rect(r, c));
            }
        }
        Ok(tiles)
    }

    fn input_components(&self) -> Result<Vec<InputComponent>> {
        self.execution_policy
            .check_cancelled("tile_component_preflight")?;
        let mut parents = (0..self.geometries.len()).collect::<Vec<_>>();
        let mut endpoint_owners = HashMap::new();
        let mut segments = Vec::new();
        for (geometry_index, (geometry, _)) in self.geometries.iter().enumerate() {
            geometry_segments(
                geometry,
                geometry_index,
                &mut segments,
                &self.execution_policy,
            )?;
        }
        if self.options.pre_snap_tolerance > 0.0 {
            let source_segments = segments;
            let lines = source_segments
                .iter()
                .enumerate()
                .map(|(segment_index, segment)| {
                    let line_id = u32::try_from(segment_index).map_err(|_| {
                        PolygonizeError::InvalidGeometry {
                            reason: "more than u32::MAX tiled component pre-snap segments"
                                .to_string(),
                        }
                    })?;
                    Ok(Line3D::new(
                        segment.line.start.into(),
                        segment.line.end.into(),
                        line_id,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            let (snapped, _) = SnapNoder::pre_snap_to_reference_vertices_with_stats(
                &lines,
                self.options.pre_snap_tolerance,
                self.options.z.policy,
                &self.execution_policy,
            )?;
            segments = snapped
                .into_iter()
                .map(|line| {
                    let source = source_segments.get(line.line_id as usize).ok_or_else(|| {
                        PolygonizeError::InternalInvariantViolation {
                            reason: "tiled pre-snap source segment is missing".to_string(),
                        }
                    })?;
                    Ok(InputSegment {
                        line: line.to_line_2d(),
                        geometry_index: source.geometry_index,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
        }
        let grid_size = self.options.precision_model.grid_size();
        if grid_size > 0.0 {
            let snapper = SnapNoder::new(grid_size)
                .with_snap_strategy(self.options.snap_strategy.clone())
                .with_z_policy(self.options.z.policy);
            for (segment_index, segment) in segments.iter_mut().enumerate() {
                self.execution_policy
                    .check_cancelled_every("tile_component_preflight", segment_index)?;
                let start: Coord3D = segment.line.start.into();
                let end: Coord3D = segment.line.end.into();
                segment.line = Line::new(
                    snapper.snap(start).to_coord_2d(),
                    snapper.snap(end).to_coord_2d(),
                );
            }
        }
        let certified_fixed_precision = matches!(
            self.options.noding.guarantee,
            NodingGuarantee::CertifiedFixedPrecision
        );
        if certified_fixed_precision {
            // Certified fixed precision can add shared hot-pixel vertices to
            // lines whose transformed straight segments do not intersect.
            // Reuse the bounded production noder so component evidence follows
            // the same connectivity contract as untiled polygonization.
            let source_segments = segments;
            let lines = source_segments
                .iter()
                .enumerate()
                .map(|(segment_index, segment)| {
                    let line_id = u32::try_from(segment_index).map_err(|_| {
                        PolygonizeError::InvalidGeometry {
                            reason: "more than u32::MAX tiled certified fixed-grid segments"
                                .to_string(),
                        }
                    })?;
                    Ok(Line3D::new(
                        segment.line.start.into(),
                        segment.line.end.into(),
                        line_id,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            let noded = HotPixelNoder::new(grid_size)?
                .with_z_policy(self.options.z.policy)
                .node_with_execution_policy(lines, &self.execution_policy)?;
            segments = noded
                .into_iter()
                .map(|line| {
                    let source = source_segments.get(line.line_id as usize).ok_or_else(|| {
                        PolygonizeError::InternalInvariantViolation {
                            reason: "tiled certified fixed-grid source segment is missing"
                                .to_string(),
                        }
                    })?;
                    Ok(InputSegment {
                        line: line.to_line_2d(),
                        geometry_index: source.geometry_index,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
        }
        for segment in &segments {
            for endpoint in [segment.line.start, segment.line.end] {
                let key = (
                    canonical_coordinate_bits(endpoint.x),
                    canonical_coordinate_bits(endpoint.y),
                );
                if let Some(previous) = endpoint_owners.insert(key, segment.geometry_index) {
                    join_components(&mut parents, previous, segment.geometry_index);
                }
            }
        }

        let endpoint_roots = (0..self.geometries.len())
            .map(|index| component_root(&mut parents, index))
            .collect::<Vec<_>>();
        let mut intersection_connected = vec![false; self.geometries.len()];
        if self.options.node_input && !certified_fixed_precision {
            let envelopes = segments
                .iter()
                .enumerate()
                .map(|(index, segment)| IndexedEnvelope {
                    aabb: AABB::from_corners(
                        [
                            segment.line.start.x.min(segment.line.end.x),
                            segment.line.start.y.min(segment.line.end.y),
                        ],
                        [
                            segment.line.start.x.max(segment.line.end.x),
                            segment.line.start.y.max(segment.line.end.y),
                        ],
                    ),
                    index,
                })
                .collect();
            let index = RStarBackend::new(envelopes);
            let mut work = ExecutionWorkTracker::new(Some(&self.execution_policy), None);
            for (segment_index, segment) in segments.iter().enumerate() {
                let envelope = AABB::from_corners(
                    [
                        segment.line.start.x.min(segment.line.end.x),
                        segment.line.start.y.min(segment.line.end.y),
                    ],
                    [
                        segment.line.start.x.max(segment.line.end.x),
                        segment.line.start.y.max(segment.line.end.y),
                    ],
                );
                for candidate_index in index.locate_in_envelope_intersecting(&envelope) {
                    if candidate_index <= segment_index {
                        continue;
                    }
                    let candidate = segments[candidate_index];
                    if candidate.geometry_index == segment.geometry_index {
                        work.candidate(false)?;
                        continue;
                    }
                    work.candidate(true)?;
                    if line_intersection(segment.line, candidate.line).is_none() {
                        continue;
                    }
                    if endpoint_roots[segment.geometry_index]
                        != endpoint_roots[candidate.geometry_index]
                    {
                        intersection_connected[segment.geometry_index] = true;
                        intersection_connected[candidate.geometry_index] = true;
                    }
                    join_components(
                        &mut parents,
                        segment.geometry_index,
                        candidate.geometry_index,
                    );
                }
            }
        }

        let mut intersection_roots = HashSet::new();
        for (geometry_index, connected) in intersection_connected.into_iter().enumerate() {
            if connected {
                intersection_roots.insert(component_root(&mut parents, geometry_index));
            }
        }

        let mut members = HashMap::<usize, Vec<usize>>::new();
        for geometry_index in 0..self.geometries.len() {
            let root = component_root(&mut parents, geometry_index);
            members.entry(root).or_default().push(geometry_index);
        }
        let mut components = members
            .into_values()
            .filter(|indices| indices.len() > 1)
            .filter_map(|input_geometry_indices| {
                let root = component_root(&mut parents, input_geometry_indices[0]);
                let mut bounds = input_geometry_indices
                    .iter()
                    .filter_map(|&index| self.geometries[index].1);
                let first = bounds.next()?;
                let bbox = bounds.fold(first, |bbox, next| {
                    Rect::new(
                        Coord {
                            x: bbox.min().x.min(next.min().x),
                            y: bbox.min().y.min(next.min().y),
                        },
                        Coord {
                            x: bbox.max().x.max(next.max().x),
                            y: bbox.max().y.max(next.max().y),
                        },
                    )
                });
                Some(InputComponent {
                    input_geometry_indices,
                    bbox,
                    connection: if self.options.pre_snap_tolerance > 0.0 {
                        TileComponentConnection::PreSnap
                    } else if grid_size > 0.0 {
                        TileComponentConnection::FixedGrid
                    } else if intersection_roots.contains(&root) {
                        TileComponentConnection::SegmentIntersection
                    } else {
                        TileComponentConnection::ExactEndpoint
                    },
                })
            })
            .collect::<Vec<_>>();
        components.sort_unstable_by_key(|component| component.input_geometry_indices[0]);
        Ok(components)
    }

    pub fn polygonize(&self) -> Result<TiledPolygonizeResult> {
        self.polygonize_impl(None)
    }

    pub fn polygonize_with_coverage_guarantee(
        &self,
        guarantee: TileCoverageGuarantee,
    ) -> std::result::Result<TiledPolygonizeResult, TiledPolygonizeError> {
        let result = self.polygonize_impl(None)?;
        let reject = match guarantee {
            TileCoverageGuarantee::BestEffort => false,
            TileCoverageGuarantee::ValidateOwnedFaces => {
                !result.stitching_report.untiled_fallback_authoritative
                    && result.stitching_report.unresolved_owned_polygon_count != 0
            }
            TileCoverageGuarantee::ValidateObservedCoverage => {
                result
                    .stitching_report
                    .coverage_resolution
                    .unresolved_issue_count
                    != 0
            }
        };
        if reject {
            return Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_tile_count: result.stitching_report.unresolved_tile_count,
                unresolved_owned_polygon_count: result
                    .stitching_report
                    .unresolved_owned_polygon_count,
                unresolved_ownership_domain_tile_count: result
                    .stitching_report
                    .unresolved_ownership_domain_tile_count,
                unresolved_ownership_domain_count: result
                    .stitching_report
                    .unresolved_ownership_domain_count,
                unresolved_input_tile_count: result.stitching_report.unresolved_input_tile_count,
                unresolved_input_geometry_count: result
                    .stitching_report
                    .unresolved_input_geometry_count,
                unresolved_component_tile_count: result
                    .stitching_report
                    .unresolved_component_tile_count,
                unresolved_component_count: result.stitching_report.unresolved_component_count,
                retry_attempt_count: result.stitching_report.retry_attempt_count,
                retry_exhausted_tile_count: result.stitching_report.retry_exhausted_tile_count,
                component_fallback_decline_reason: result
                    .stitching_report
                    .component_fallback_decline_reason,
                coverage_resolution: Box::new(result.stitching_report.coverage_resolution.clone()),
                tile_reports: Box::new(result.tile_reports),
            });
        }
        Ok(result)
    }

    pub fn polygonize_with_trace(
        &self,
        level: TraceLevelV1,
        byte_limit: usize,
    ) -> Result<TracedTiledPolygonizeResultV1> {
        self.polygonize_with_trace_limits(level, TraceByteLimitsV1::total(byte_limit))
    }

    pub fn polygonize_with_trace_limits(
        &self,
        level: TraceLevelV1,
        limits: TraceByteLimitsV1,
    ) -> Result<TracedTiledPolygonizeResultV1> {
        let mut trace = TraceRecorderV1::new_with_limits(Some(level), limits, &self.options)
            .expect("trace enabled");
        let result = self.polygonize_impl(Some(&mut trace))?;
        Ok(TracedTiledPolygonizeResultV1 {
            result,
            trace: trace.finish(),
        })
    }

    fn polygonize_impl(
        &self,
        trace: Option<&mut TraceRecorderV1>,
    ) -> Result<TiledPolygonizeResult> {
        self.validate()?;
        let tiles = self.generate_tiles()?;
        let input_components = self.input_components()?;
        self.polygonize_tiles(tiles, &input_components, trace)
    }

    fn polygonize_tiles(
        &self,
        tiles: Vec<Rect<f64>>,
        input_components: &[InputComponent],
        mut trace: Option<&mut TraceRecorderV1>,
    ) -> Result<TiledPolygonizeResult> {
        let retry_attempt_counter = Arc::new(AtomicUsize::new(0));
        let trace_ownership = trace
            .as_ref()
            .is_some_and(|trace| trace.records_stage(TraceStageV1::Output));

        let mut tile_polygons = Vec::with_capacity(tiles.len());
        let mut tile_reports = Vec::with_capacity(tiles.len());
        let mut partition_snapshots = Vec::with_capacity(tiles.len());
        let mut partition_border_graph = PartitionBorderGraph::default();
        let mut partition_border_local_face_graphs = Vec::new();
        if trace_ownership {
            for (tile_index, tile) in tiles.into_iter().enumerate() {
                let capture_byte_limit = trace.as_ref().and_then(|trace| {
                    trace
                        .records_stage(TraceStageV1::Output)
                        .then(|| trace.capture_byte_limit(TraceStageV1::Output))
                });
                let (
                    polygons,
                    report,
                    ownership_decisions,
                    border_observations,
                    local_face_graphs,
                    capture_truncated,
                    boundary_noding_stats,
                    dangles,
                    cut_edges,
                    invalid_rings,
                    _noded_segments,
                    _boundary_noded_segments,
                    snapshot,
                ) = self.process_tile_with_retries(
                    tile_index,
                    tile,
                    input_components,
                    capture_byte_limit,
                    &retry_attempt_counter,
                )?;
                let trace = trace.as_deref_mut().expect("tile trace exists");
                for attempt in &report.retry_attempts {
                    if !trace.record_tile_halo_retry(tile_index, attempt) {
                        break;
                    }
                }
                for issue in &report.excluded_component_issues {
                    let recorded = match issue.connection {
                        TileComponentConnection::ExactEndpoint => {
                            trace.record_tile_excluded_endpoint_component(tile_index, issue)?
                        }
                        TileComponentConnection::SegmentIntersection => {
                            trace.record_tile_excluded_segment_component(tile_index, issue)?
                        }
                        TileComponentConnection::PreSnap => {
                            trace.record_tile_excluded_pre_snap_component(tile_index, issue)?
                        }
                        TileComponentConnection::FixedGrid => {
                            trace.record_tile_excluded_fixed_grid_component(tile_index, issue)?
                        }
                    };
                    if !recorded {
                        break;
                    }
                }
                for issue in &report.input_boundary_issues {
                    if !trace.record_tile_input_boundary(tile_index, issue)? {
                        break;
                    }
                }
                for issue in &report.coverage_issues {
                    if !trace.record_tile_owned_face_boundary(tile_index, issue)? {
                        break;
                    }
                }
                for issue in &report.ownership_domain_issues {
                    if !trace.record_tile_ownership_domain(tile_index, issue)? {
                        break;
                    }
                }
                for (polygon_index, ownership_point, owned) in ownership_decisions {
                    trace.record_tile_ownership(
                        tile_index,
                        polygon_index,
                        ownership_point,
                        owned,
                    )?;
                }
                if capture_truncated {
                    trace.mark_capture_truncated(TraceStageV1::Output);
                }
                trace.record_partition_boundary_noding(tile_index, boundary_noding_stats);
                for observation in border_observations {
                    trace.record_partition_border_observation(&observation);
                    if let Err(error) = partition_border_graph.insert(observation.clone()) {
                        trace.record_partition_border_rejection(&observation, &error.to_string());
                        return Err(error);
                    }
                }
                partition_border_graph.retain_global_non_polygon_extraction_payloads(
                    &self.execution_policy,
                    tile_index,
                    dangles,
                    cut_edges,
                    invalid_rings,
                )?;
                partition_border_local_face_graphs.extend(local_face_graphs);
                tile_polygons.push(polygons);
                tile_reports.push(report);
                partition_snapshots.push(snapshot);
            }
        } else {
            #[cfg(feature = "parallel")]
            let tile_results: Result<Vec<_>> =
                if let Some(max_parallel_tiles) = self.tile_execution_policy.max_parallel_tiles {
                    let pool = ThreadPoolBuilder::new()
                        .num_threads(max_parallel_tiles)
                        .build()
                        .map_err(|error| PolygonizeError::InternalInvariantViolation {
                            reason: format!("failed to create tiled worker pool: {error}"),
                        })?;
                    pool.install(|| {
                        tiles
                            .into_par_iter()
                            .enumerate()
                            .map(|(tile_index, tile)| {
                                self.process_tile_with_retries(
                                    tile_index,
                                    tile,
                                    input_components,
                                    None,
                                    &retry_attempt_counter,
                                )
                            })
                            .collect()
                    })
                } else {
                    tiles
                        .into_par_iter()
                        .enumerate()
                        .map(|(tile_index, tile)| {
                            self.process_tile_with_retries(
                                tile_index,
                                tile,
                                input_components,
                                None,
                                &retry_attempt_counter,
                            )
                        })
                        .collect()
                };
            #[cfg(not(feature = "parallel"))]
            let tile_results: Result<Vec<_>> = tiles
                .into_iter()
                .enumerate()
                .map(|(tile_index, tile)| {
                    self.process_tile_with_retries(
                        tile_index,
                        tile,
                        input_components,
                        None,
                        &retry_attempt_counter,
                    )
                })
                .collect();

            for (partition_id, result) in tile_results?.into_iter().enumerate() {
                let (
                    polygons,
                    report,
                    _,
                    border_observations,
                    local_face_graphs,
                    _,
                    _,
                    dangles,
                    cut_edges,
                    invalid_rings,
                    _,
                    _,
                    snapshot,
                ) = result;
                for observation in border_observations {
                    partition_border_graph.insert(observation)?;
                }
                partition_border_graph.retain_global_non_polygon_extraction_payloads(
                    &self.execution_policy,
                    partition_id,
                    dangles,
                    cut_edges,
                    invalid_rings,
                )?;
                partition_border_local_face_graphs.extend(local_face_graphs);
                tile_polygons.push(polygons);
                tile_reports.push(report);
                partition_snapshots.push(snapshot);
            }
        }
        for local_face_graph in partition_border_local_face_graphs {
            partition_border_graph.insert_local_face_graph(local_face_graph)?;
        }
        self.declare_partition_adjacencies(&mut partition_border_graph, &tile_reports)?;
        let partition_border_reconciliation = partition_border_graph.reconciliation_stats();
        let partition_border_twin_application =
            partition_border_graph.apply_unambiguous_face_twins(&self.execution_policy)?;
        let partition_border_global_face_edge_map =
            partition_border_graph.reconcile_global_face_edge_map(&self.execution_policy)?;
        let partition_border_global_face_nodes = partition_border_graph
            .reconcile_global_face_nodes(self.options.z, &self.execution_policy)?;
        let applied_face_twin_count = partition_border_graph.applied_face_twins().len();
        debug_assert_eq!(
            applied_face_twin_count,
            partition_border_twin_application.applied_twin_count
        );
        let partition_border_node_reconciliation = partition_border_graph
            .reconcile_border_nodes(self.options.z, &self.execution_policy)?;
        let reconciled_border_node_count = partition_border_graph.reconciled_border_nodes().len();
        debug_assert_eq!(
            reconciled_border_node_count,
            partition_border_node_reconciliation.node_count
        );
        let partition_border_canonical_node_validation =
            partition_border_graph.validate_canonical_border_nodes(&self.execution_policy)?;
        let partition_border_global_component_reconciliation =
            partition_border_graph.reconcile_global_components(&self.execution_policy)?;
        let global_component_count = partition_border_graph.global_components().len();
        debug_assert_eq!(
            global_component_count,
            partition_border_global_component_reconciliation.component_count
        );
        let partition_border_global_component_payloads =
            partition_border_graph.reconcile_global_component_payloads(&self.execution_policy)?;
        debug_assert_eq!(
            partition_border_global_component_payloads.component_count,
            global_component_count
        );
        let partition_border_global_face_plan =
            partition_border_graph.reconcile_global_face_plans(&self.execution_policy)?;
        let global_face_plan_count = partition_border_graph.global_face_plans().len();
        debug_assert_eq!(
            global_face_plan_count,
            partition_border_global_face_plan.face_count
        );
        let partition_border_global_face_validation =
            partition_border_graph.validate_global_face_plans(&self.execution_policy)?;
        debug_assert_eq!(
            partition_border_global_face_validation.face_count,
            global_face_plan_count
        );
        let partition_border_global_face_mutation_gate =
            partition_border_graph.validate_global_face_mutation_gate(&self.execution_policy)?;
        debug_assert_eq!(
            partition_border_global_face_mutation_gate.face_count,
            global_face_plan_count
        );
        let partition_border_global_face_transition_plan =
            partition_border_graph.reconcile_global_face_transitions(&self.execution_policy)?;
        let global_face_transition_plan_count =
            partition_border_graph.global_face_transitions().len();
        debug_assert_eq!(
            partition_border_global_face_transition_plan.face_count,
            global_face_transition_plan_count
        );
        let partition_border_global_face_twin_transition = partition_border_graph
            .reconcile_global_face_twin_transitions(&self.execution_policy)?;
        let global_face_twin_transition_count =
            partition_border_graph.global_face_twin_transitions().len();
        debug_assert_eq!(
            partition_border_global_face_twin_transition.mapped_twin_count,
            global_face_twin_transition_count
        );
        let partition_border_global_face_walk_invariants =
            partition_border_graph.validate_global_face_walk_invariants(&self.execution_policy)?;
        debug_assert_eq!(
            partition_border_global_face_walk_invariants.face_count,
            global_face_plan_count
        );
        debug_assert_eq!(
            partition_border_global_face_walk_invariants.mapped_twin_count,
            global_face_twin_transition_count
        );
        let partition_border_global_face_euler_witness = partition_border_graph
            .validate_global_face_euler_witness_with_walk(
                &self.execution_policy,
                partition_border_global_face_walk_invariants,
            )?;
        let partition_border_global_face_next_candidates = partition_border_graph
            .reconcile_global_face_next_candidates_with_walk(
                &self.execution_policy,
                partition_border_global_face_walk_invariants,
            )?;
        let partition_border_global_face_identity_plans = partition_border_graph
            .reconcile_global_face_identity_plans_with_walk(
                &self.execution_policy,
                partition_border_global_face_walk_invariants,
            )?;
        let partition_border_global_face_next_mutation_plans = partition_border_graph
            .reconcile_global_face_next_mutation_plans_with_walk(
                &self.execution_policy,
                partition_border_global_face_walk_invariants,
            )?;
        let partition_border_global_face_id_plans = partition_border_graph
            .reconcile_global_face_id_plans_with_walk(
                &self.execution_policy,
                partition_border_global_face_walk_invariants,
            )?;
        let partition_border_global_face_next_application = partition_border_graph
            .reconcile_global_face_next_application_plans(&self.execution_policy)?;
        let partition_border_global_topology_candidate =
            partition_border_graph.reconcile_global_topology_candidate(&self.execution_policy)?;
        let partition_border_global_topology_application_gate = partition_border_graph
            .validate_global_topology_application_gate(&self.execution_policy)?;
        let partition_border_global_component_coverage =
            partition_border_graph.validate_global_component_coverage(&self.execution_policy)?;
        let partition_border_global_face_id_application =
            partition_border_graph.validate_global_face_id_application(&self.execution_policy)?;
        let partition_border_global_unbounded_face_proof = partition_border_graph
            .validate_global_unbounded_face_proof_with_walk(
                &self.execution_policy,
                partition_border_global_face_walk_invariants,
            )?;
        let partition_border_global_unbounded_face_application = partition_border_graph
            .validate_global_unbounded_face_application_with_evidence(
                &self.execution_policy,
                partition_border_global_unbounded_face_proof,
                partition_border_global_face_id_application,
            )?;
        let partition_border_global_topology_mutation_gate = partition_border_graph
            .validate_global_topology_mutation_gate_with_evidence(
                &self.execution_policy,
                partition_border_global_topology_application_gate,
                partition_border_global_component_coverage,
                partition_border_global_face_id_application,
                partition_border_global_unbounded_face_application,
                partition_border_global_face_walk_invariants,
                partition_border_global_face_euler_witness,
            )?;
        let partition_border_global_topology_mutation = partition_border_graph
            .apply_global_topology_candidate_with_gate(
                &self.execution_policy,
                partition_border_global_topology_mutation_gate,
                partition_border_global_topology_candidate,
            )?;
        let partition_border_global_face_id_mutation = partition_border_graph
            .apply_global_face_ids_with_evidence(
                &self.execution_policy,
                partition_border_global_topology_mutation,
                partition_border_global_face_id_application,
                partition_border_global_unbounded_face_application,
            )?;
        let partition_border_global_unbounded_face_mutation = partition_border_graph
            .apply_global_unbounded_face_with_evidence(
                &self.execution_policy,
                partition_border_global_topology_mutation,
                partition_border_global_face_id_mutation,
                partition_border_global_unbounded_face_application,
            )?;
        let partition_border_global_face_identity_materialization =
            partition_border_graph.materialize_global_face_identity(&self.execution_policy)?;
        let partition_border_global_face_identity_invariants = partition_border_graph
            .validate_global_face_identity_invariants(
                &self.execution_policy,
                partition_border_global_face_identity_materialization,
                partition_border_global_face_walk_invariants,
                partition_border_global_face_euler_witness,
            )?;
        let partition_border_global_next_lineage_integration = partition_border_graph
            .validate_global_next_lineage_integration(
                &self.execution_policy,
                partition_border_global_face_identity_invariants,
            )?;
        let partition_border_global_cycle_face_lineage = partition_border_graph
            .validate_global_cycle_face_lineage(
                &self.execution_policy,
                partition_border_global_face_identity_invariants,
                partition_border_global_next_lineage_integration,
            )?;
        let partition_border_global_cycle_face_promotion_gate = partition_border_graph
            .validate_global_cycle_face_promotion_gate(
                &self.execution_policy,
                partition_border_global_cycle_face_lineage,
                partition_border_global_component_coverage,
                partition_border_global_unbounded_face_application,
            )?;
        let partition_border_global_face_payload_lineage = partition_border_graph
            .validate_global_face_payload_lineage(
                &self.execution_policy,
                partition_border_global_cycle_face_promotion_gate,
            )?;
        let partition_border_global_face_cycle_geometry = partition_border_graph
            .validate_global_face_cycle_geometry(
                &self.execution_policy,
                partition_border_global_face_payload_lineage,
            )?;
        let partition_border_global_face_extraction_gate = partition_border_graph
            .validate_global_face_extraction_gate(
                &self.execution_policy,
                partition_border_global_face_identity_invariants,
                partition_border_global_next_lineage_integration,
                partition_border_global_cycle_face_lineage,
                partition_border_global_cycle_face_promotion_gate,
                partition_border_global_face_payload_lineage,
                partition_border_global_face_cycle_geometry,
            )?;
        let partition_border_global_face_topology = partition_border_graph
            .materialize_global_face_topology(
                &self.execution_policy,
                partition_border_global_face_extraction_gate,
            )?;
        let partition_border_global_face_invariant_gate = partition_border_graph
            .validate_global_face_invariant_gate(
                &self.execution_policy,
                partition_border_global_face_identity_invariants,
                partition_border_global_next_lineage_integration,
                partition_border_global_cycle_face_lineage,
                partition_border_global_face_payload_lineage,
                partition_border_global_face_cycle_geometry,
                partition_border_global_face_extraction_gate,
                partition_border_global_face_topology,
            )?;
        let partition_border_global_face_ring_payloads = partition_border_graph
            .materialize_global_face_ring_payloads(
                &self.execution_policy,
                partition_border_global_face_extraction_gate,
            )?;
        let partition_border_global_face_ring_classification = partition_border_graph
            .classify_global_face_ring_payloads(
                &self.execution_policy,
                partition_border_global_face_ring_payloads,
                partition_border_global_face_cycle_geometry,
            )?;
        let partition_border_global_face_ring_candidate_assembly = partition_border_graph
            .assemble_global_face_ring_candidates(
                &self.execution_policy,
                partition_border_global_face_ring_classification,
            )?;
        let partition_border_global_face_ring_extraction_readiness = partition_border_graph
            .materialize_global_face_ring_extraction_candidates(
                &self.execution_policy,
                partition_border_global_face_ring_payloads,
                partition_border_global_face_ring_classification,
                partition_border_global_face_ring_candidate_assembly,
            )?;
        let partition_border_global_face_ring_extraction_payloads = partition_border_graph
            .materialize_global_face_ring_extraction_payloads(
                &self.execution_policy,
                partition_border_global_face_ring_extraction_readiness,
            )?;
        let partition_border_global_non_polygon_extraction = partition_border_graph
            .materialize_global_non_polygon_extraction_payloads(&self.execution_policy)?;
        let partition_border_global_extraction_readiness = partition_border_graph
            .validate_global_extraction_readiness(
                &self.execution_policy,
                partition_border_global_face_topology,
                partition_border_global_face_ring_extraction_readiness,
                partition_border_global_face_ring_extraction_payloads,
                partition_border_global_non_polygon_extraction,
                partition_border_global_face_invariant_gate,
            )?;
        let partition_border_global_private_extraction = partition_border_graph
            .materialize_global_private_extraction(
                &self.execution_policy,
                partition_border_global_extraction_readiness,
                partition_border_global_face_ring_extraction_payloads,
                partition_border_global_non_polygon_extraction,
            )?;
        let stitched_output = if partition_border_global_private_extraction.extraction_ready {
            promote_global_private_extraction(
                &partition_border_graph,
                &self.options,
                &self.execution_policy,
            )?
        } else {
            None
        };
        let untiled_equivalence = if self.untiled_equivalence_check {
            compare_stitched_output_with_untiled(
                stitched_output.as_ref(),
                &self.geometries,
                &self.options,
                &self.execution_policy,
            )?
        } else {
            TiledUntiledEquivalenceStats::default()
        };
        if let Some(trace) = trace.as_deref_mut() {
            let (
                stitched_polygon_count,
                stitched_dangle_count,
                stitched_cut_edge_count,
                stitched_invalid_ring_count,
            ) = stitched_output.as_ref().map_or((0, 0, 0, 0), |output| {
                (
                    output.polygons.len(),
                    output.dangles.len(),
                    output.cut_edges.len(),
                    output.invalid_rings.len(),
                )
            });
            trace.record_tiled_stitched_output(
                stitched_output.is_some(),
                stitched_polygon_count,
                stitched_dangle_count,
                stitched_cut_edge_count,
                stitched_invalid_ring_count,
            );
            trace.record_tiled_untiled_equivalence(
                untiled_equivalence.checked,
                untiled_equivalence.ready,
                untiled_equivalence.mismatch_count,
            );
            trace.record_partition_border_reconciliation(partition_border_reconciliation);
            trace.record_partition_border_twin_application(partition_border_twin_application);
            trace.record_partition_border_global_face_edge_map(
                partition_border_global_face_edge_map,
            );
            trace.record_partition_border_global_face_nodes(partition_border_global_face_nodes);
            trace.record_partition_border_node_reconciliation(
                partition_border_node_reconciliation,
                self.options.z,
            );
            trace.record_partition_border_canonical_node_validation(
                partition_border_canonical_node_validation,
            );
            trace.record_partition_border_global_component_reconciliation(
                partition_border_global_component_reconciliation,
            );
            trace.record_partition_border_global_component_payloads(
                partition_border_global_component_payloads,
            );
            trace.record_partition_border_global_face_plan(partition_border_global_face_plan);
            trace.record_partition_border_global_face_validation(
                partition_border_global_face_validation,
            );
            trace.record_partition_border_global_face_mutation_gate(
                partition_border_global_face_mutation_gate,
            );
            trace.record_partition_border_global_face_transition_plan(
                partition_border_global_face_transition_plan,
            );
            trace.record_partition_border_global_face_twin_transitions(
                partition_border_global_face_twin_transition,
            );
            trace.record_partition_border_global_face_walk_invariants(
                partition_border_global_face_walk_invariants,
            );
            trace.record_partition_border_global_face_euler_witness(
                partition_border_global_face_euler_witness,
            );
            trace.record_partition_border_global_face_next_candidates(
                partition_border_global_face_next_candidates,
            );
            trace.record_partition_border_global_face_identity_plans(
                partition_border_global_face_identity_plans,
            );
            trace.record_partition_border_global_face_next_mutation_plans(
                partition_border_global_face_next_mutation_plans,
            );
            trace.record_partition_border_global_face_id_plans(
                partition_border_global_face_id_plans,
            );
            trace.record_partition_border_global_face_next_application(
                partition_border_global_face_next_application,
            );
            trace.record_partition_border_global_topology_candidate(
                partition_border_global_topology_candidate,
            );
            trace.record_partition_border_global_topology_application_gate(
                partition_border_global_topology_application_gate,
            );
            trace.record_partition_border_global_component_coverage(
                partition_border_global_component_coverage,
            );
            trace.record_partition_border_global_face_id_application(
                partition_border_global_face_id_application,
            );
            trace.record_partition_border_global_unbounded_face_proof(
                partition_border_global_unbounded_face_proof,
            );
            trace.record_partition_border_global_unbounded_face_application(
                partition_border_global_unbounded_face_application,
            );
            trace.record_partition_border_global_topology_mutation_gate(
                partition_border_global_topology_mutation_gate,
            );
            trace.record_partition_border_global_topology_mutation(
                partition_border_global_topology_mutation,
            );
            trace.record_partition_border_global_face_id_mutation(
                partition_border_global_face_id_mutation,
            );
            trace.record_partition_border_global_unbounded_face_mutation(
                partition_border_global_unbounded_face_mutation,
            );
            trace.record_partition_border_global_face_identity_materialization(
                partition_border_global_face_identity_materialization,
            );
            trace.record_partition_border_global_face_topology(
                partition_border_global_face_topology,
            );
            trace.record_partition_border_global_face_invariant_gate(
                partition_border_global_face_invariant_gate,
            );
            trace.record_partition_border_global_face_identity_invariants(
                partition_border_global_face_identity_invariants,
            );
            trace.record_partition_border_global_next_lineage_integration(
                partition_border_global_next_lineage_integration,
            );
            trace.record_partition_border_global_cycle_face_lineage(
                partition_border_global_cycle_face_lineage,
            );
            trace.record_partition_border_global_cycle_face_promotion_gate(
                partition_border_global_cycle_face_promotion_gate,
            );
            trace.record_partition_border_global_face_payload_lineage(
                partition_border_global_face_payload_lineage,
            );
            trace.record_partition_border_global_face_cycle_geometry(
                partition_border_global_face_cycle_geometry,
            );
            trace.record_partition_border_global_face_extraction_gate(
                partition_border_global_face_extraction_gate,
            );
            trace.record_partition_border_global_face_ring_payloads(
                partition_border_global_face_ring_payloads,
            );
            trace.record_partition_border_global_face_ring_classification(
                partition_border_global_face_ring_classification,
            );
            trace.record_partition_border_global_face_ring_candidate_assembly(
                partition_border_global_face_ring_candidate_assembly,
            );
            trace.record_partition_border_global_face_ring_extraction_readiness(
                partition_border_global_face_ring_extraction_readiness,
            );
            trace.record_partition_border_global_face_ring_extraction_payloads(
                partition_border_global_face_ring_extraction_payloads,
            );
            trace.record_partition_border_global_non_polygon_extraction(
                partition_border_global_non_polygon_extraction,
            );
            trace.record_partition_border_global_extraction_readiness(
                partition_border_global_extraction_readiness,
            );
            trace.record_partition_border_global_private_extraction(
                partition_border_global_private_extraction,
            );
        }
        let unresolved = tile_reports.iter().any(Self::report_is_unresolved);
        let component_fallback_attempted = self.component_fallback && unresolved;
        let (component_fallback, component_fallback_decline_reason) =
            if component_fallback_attempted {
                match self.try_component_fallback(
                    &tile_polygons,
                    &tile_reports,
                    input_components,
                )? {
                    ComponentFallbackDecision::Recovered(result) => {
                        self.execution_policy.check(
                            "tile_fallback_regions",
                            self.tile_execution_policy.max_fallback_regions,
                            result.region_bboxes.len(),
                        )?;
                        (Some(result), None)
                    }
                    ComponentFallbackDecision::Declined(reason) => (None, Some(reason.as_str())),
                }
            } else {
                (None, None)
            };
        let component_fallback_used = component_fallback.is_some();
        let (component_fallback_polygons, component_fallback_events, region_bboxes) =
            component_fallback
                .map(|fallback| (fallback.polygons, fallback.events, fallback.region_bboxes))
                .unwrap_or_default();
        let retained_tile_polygon_count = tile_polygons.iter().map(Vec::len).sum();
        let component_fallback_count = component_fallback_events
            .iter()
            .map(|event| event.recovered_component_count)
            .sum();
        let component_fallback_polygon_count = component_fallback_polygons.len();
        let component_fallback_replaced_polygon_count = component_fallback_events
            .iter()
            .map(|event| event.replaced_retained_polygon_count)
            .sum();
        let untiled_fallback_attempted =
            self.untiled_fallback && unresolved && !component_fallback_used;
        let (result_polygons, untiled_fallback_authoritative): (Vec<Polygon3D>, bool) =
            if untiled_fallback_attempted {
                let mut polygonizer = Polygonizer::with_options(self.options.clone())
                    .with_execution_policy(self.execution_policy.clone());
                for (geometry, _) in &self.geometries {
                    polygonizer.add_borrowed_geometry(geometry);
                }
                let polygons = polygonizer.polygonize()?.polygons;
                let mut polygon_count = 0;
                let mut coordinate_count = 0;
                for polygon in &polygons {
                    account_polygon_output(
                        &self.execution_policy,
                        &mut polygon_count,
                        &mut coordinate_count,
                        polygon,
                    )?;
                }
                (polygons, true)
            } else {
                (
                    self.merge_fallback_polygons(
                        tile_polygons,
                        &region_bboxes,
                        component_fallback_polygons,
                    )?,
                    false,
                )
            };
        let untiled_fallback_output_polygon_count = if untiled_fallback_authoritative {
            result_polygons.len()
        } else {
            0
        };
        let untiled_fallback_used = untiled_fallback_authoritative;
        let coverage_resolution = build_coverage_resolution(
            &tile_reports,
            component_fallback_used,
            &region_bboxes,
            untiled_fallback_authoritative,
        );
        let merged_polygon_count = result_polygons.len();

        let polygons = if untiled_fallback_used {
            result_polygons
        } else {
            self.execution_policy
                .check_cancelled("tile_deduplication")?;
            match self.dedup_policy {
                DedupPolicy::KeepAll => {
                    if let Some(trace) = trace.as_deref_mut() {
                        for polygon_index in 0..result_polygons.len() {
                            self.execution_policy
                                .check_cancelled_every("tile_deduplication", polygon_index)?;
                            trace.record_tile_dedup(polygon_index, true);
                        }
                    }
                    result_polygons
                }
                DedupPolicy::CanonicalRingHash => {
                    let mut unique_polygons = Vec::new();
                    let mut seen = HashMap::new();
                    let mut profile_conflicts = Vec::new();

                    for (polygon_index, poly) in result_polygons.into_iter().enumerate() {
                        self.execution_policy
                            .check_cancelled_every("tile_deduplication", polygon_index)?;
                        let key = canonical_polygon_key(&poly);
                        let retained = if let Some(&retained_index) = seen.get(&key) {
                            profile_conflicts[retained_index] = merge_duplicate_polygon_provenance(
                                &mut unique_polygons[retained_index],
                                &poly,
                                profile_conflicts[retained_index],
                            );
                            false
                        } else {
                            seen.insert(key, unique_polygons.len());
                            unique_polygons.push(poly);
                            profile_conflicts.push(false);
                            true
                        };
                        if let Some(trace) = trace.as_deref_mut() {
                            trace.record_tile_dedup(polygon_index, retained);
                        }
                    }

                    unique_polygons
                }
            }
        };
        let mut dangles = Vec::new();
        let mut cut_edges = Vec::new();
        let mut invalid_rings = Vec::new();
        let polygons = apply_determinism(
            polygons,
            &mut dangles,
            &mut cut_edges,
            &mut invalid_rings,
            &self.options,
            &self.execution_policy,
            None,
        )?;
        let mut output_polygon_count = 0;
        let mut output_coordinate_count = 0;
        for (polygon_index, polygon) in polygons.iter().enumerate() {
            self.execution_policy
                .check_cancelled_every("output_flattening", polygon_index)?;
            account_polygon_output(
                &self.execution_policy,
                &mut output_polygon_count,
                &mut output_coordinate_count,
                polygon,
            )?;
        }
        let output_polygon_count = polygons.len();
        let unresolved_tile_count = tile_reports
            .iter()
            .filter(|report| !report.coverage_issues.is_empty())
            .count();
        let unresolved_owned_polygon_count = tile_reports
            .iter()
            .map(|report| report.coverage_issues.len())
            .sum();
        let unresolved_ownership_domain_tile_count = tile_reports
            .iter()
            .filter(|report| !report.ownership_domain_issues.is_empty())
            .count();
        let unresolved_ownership_domain_count =
            tile_reports.iter().fold(0usize, |total, report| {
                total.saturating_add(report.ownership_domain_issues.len())
            });
        let unresolved_input_tile_count = tile_reports
            .iter()
            .filter(|report| !report.input_boundary_issues.is_empty())
            .count();
        let unresolved_input_geometry_count = tile_reports.iter().fold(0usize, |total, report| {
            total.saturating_add(report.input_boundary_issues.len())
        });
        let unresolved_component_tile_count = tile_reports
            .iter()
            .filter(|report| !report.excluded_component_issues.is_empty())
            .count();
        let unresolved_component_count = tile_reports.iter().fold(0usize, |total, report| {
            total.saturating_add(report.excluded_component_issues.len())
        });
        let retried_tile_count = tile_reports
            .iter()
            .filter(|report| !report.retry_attempts.is_empty())
            .count();
        let retry_attempt_count = tile_reports.iter().fold(0usize, |total, report| {
            total.saturating_add(report.retry_attempts.len())
        });
        let retry_exhausted_tile_count = tile_reports
            .iter()
            .filter(|report| report.retry_exhausted)
            .count();
        let stitched_output_ready = stitched_output.is_some();
        let result = TiledPolygonizeResult {
            polygons,
            stitched_output,
            tile_reports,
            partition_border_graph,
            partition_snapshots,
            stitching_report: StitchingReport {
                merged_polygon_count,
                duplicate_polygon_count: merged_polygon_count - output_polygon_count,
                output_polygon_count,
                retained_tile_polygon_count,
                component_fallback_count,
                component_fallback_polygon_count,
                component_fallback_replaced_polygon_count,
                component_fallback_attempted,
                component_fallback_decline_reason,
                unresolved_tile_count,
                unresolved_owned_polygon_count,
                unresolved_ownership_domain_tile_count,
                unresolved_ownership_domain_count,
                unresolved_input_tile_count,
                unresolved_input_geometry_count,
                unresolved_component_tile_count,
                unresolved_component_count,
                retried_tile_count,
                retry_attempt_count,
                retry_exhausted_tile_count,
                partition_border_adjacency_count: partition_border_reconciliation
                    .declared_adjacency_count,
                partition_border_normalized_edge_count: partition_border_reconciliation
                    .normalized_edge_count,
                partition_border_twin_count: partition_border_reconciliation.matched_twin_count,
                partition_border_unmatched_edge_count: partition_border_reconciliation
                    .unmatched_edge_count,
                partition_border_reconciled_node_count: reconciled_border_node_count,
                partition_border_node_z_conflict_count: partition_border_node_reconciliation
                    .z_conflict_count,
                partition_border_canonical_node_count: partition_border_canonical_node_validation
                    .canonical_node_count,
                partition_border_canonical_global_node_count:
                    partition_border_canonical_node_validation.global_node_count,
                partition_border_canonical_mapped_global_node_count:
                    partition_border_canonical_node_validation.mapped_global_node_count,
                partition_border_canonical_only_node_count:
                    partition_border_canonical_node_validation.canonical_only_node_count,
                partition_border_canonical_node_reconciliation_ready:
                    partition_border_canonical_node_validation.reconciliation_ready,
                partition_border_global_component_count: global_component_count,
                partition_border_global_component_payload_count:
                    partition_border_global_component_payloads.component_count,
                partition_border_global_component_payload_source_line_count:
                    partition_border_global_component_payloads.source_line_count,
                partition_border_global_component_payload_representative_line_count:
                    partition_border_global_component_payloads.representative_line_count,
                partition_border_global_component_payload_z_candidate_count:
                    partition_border_global_component_payloads.z_candidate_count,
                partition_border_global_component_payload_selected_z_node_count:
                    partition_border_global_component_payloads.selected_z_node_count,
                partition_border_global_component_payload_z_conflict_node_count:
                    partition_border_global_component_payloads.z_conflict_node_count,
                partition_border_global_component_payload_z_conflict_component_count:
                    partition_border_global_component_payloads.z_conflict_component_count,
                partition_border_global_face_count:
                    partition_border_global_component_reconciliation.face_count,
                partition_border_global_linked_face_count:
                    partition_border_global_component_reconciliation.linked_face_count,
                partition_border_global_face_plan_count: global_face_plan_count,
                partition_border_global_face_candidate_count: partition_border_global_face_plan
                    .candidate_count,
                partition_border_global_face_missing_successor_count:
                    partition_border_global_face_plan.missing_successor_count,
                partition_border_global_unbounded_face_count: partition_border_global_face_plan
                    .unbounded_face_count,
                partition_border_global_face_linked_count: partition_border_global_face_plan
                    .linked_face_count,
                partition_border_global_face_missing_boundary_successor_count:
                    partition_border_global_face_plan.missing_boundary_successor_count,
                partition_border_global_face_validated_count:
                    partition_border_global_face_validation.face_count,
                partition_border_global_face_validated_candidate_count:
                    partition_border_global_face_validation.candidate_count,
                partition_border_global_face_validated_twin_count:
                    partition_border_global_face_validation.twin_link_count,
                partition_border_global_face_validated_unbounded_count:
                    partition_border_global_face_validation.unbounded_face_count,
                partition_border_global_face_boundary_transition_count:
                    partition_border_global_face_mutation_gate.boundary_transition_count,
                partition_border_global_face_mutation_missing_successor_count:
                    partition_border_global_face_mutation_gate.missing_boundary_successor_count,
                partition_border_global_face_mutation_ready_count:
                    partition_border_global_face_mutation_gate.mutation_ready_face_count,
                partition_border_global_face_transition_count:
                    partition_border_global_face_transition_plan.boundary_transition_count,
                partition_border_global_face_transition_closed_count:
                    partition_border_global_face_transition_plan.closed_face_count,
                partition_border_global_face_transition_incomplete_count:
                    partition_border_global_face_transition_plan.incomplete_face_count,
                partition_border_global_face_twin_transition_count:
                    partition_border_global_face_twin_transition.mapped_twin_count,
                partition_border_global_face_twin_transition_ready_count:
                    partition_border_global_face_twin_transition.mutation_ready_twin_count,
                partition_border_global_face_twin_transition_unmapped_count:
                    partition_border_global_face_twin_transition.unmapped_twin_count,
                partition_border_global_face_walk_validated_count:
                    partition_border_global_face_walk_invariants.face_count,
                partition_border_global_face_walk_closed_count:
                    partition_border_global_face_walk_invariants.closed_face_count,
                partition_border_global_face_walk_source_complete_twin_count:
                    partition_border_global_face_walk_invariants.source_complete_twin_count,
                partition_border_global_face_walk_unbounded_component_count:
                    partition_border_global_face_walk_invariants.unbounded_component_count,
                partition_border_global_face_walk_face_adjacency_cycle_rank:
                    partition_border_global_face_walk_invariants.face_adjacency_cycle_rank,
                partition_border_global_face_euler_transition_face_count:
                    partition_border_global_face_euler_witness.transition_face_count,
                partition_border_global_face_euler_closed_boundary_cycle_count:
                    partition_border_global_face_euler_witness.closed_boundary_cycle_count,
                partition_border_global_face_euler_boundary_vertex_count:
                    partition_border_global_face_euler_witness.boundary_vertex_count,
                partition_border_global_face_euler_boundary_edge_count:
                    partition_border_global_face_euler_witness.boundary_edge_count,
                partition_border_global_face_euler_cross_component_edge_count:
                    partition_border_global_face_euler_witness.cross_component_edge_count,
                partition_border_global_face_euler_boundary_lhs:
                    partition_border_global_face_euler_witness.boundary_euler_lhs,
                partition_border_global_face_euler_boundary_rhs:
                    partition_border_global_face_euler_witness.boundary_euler_rhs,
                partition_border_global_face_euler_boundary_consistent:
                    partition_border_global_face_euler_witness.boundary_euler_consistent,
                partition_border_global_face_next_candidate_count:
                    partition_border_global_face_next_candidates.twin_candidate_count,
                partition_border_global_face_next_ready_candidate_count:
                    partition_border_global_face_next_candidates.ready_candidate_count,
                partition_border_global_face_next_incomplete_candidate_count:
                    partition_border_global_face_next_candidates.incomplete_candidate_count,
                partition_border_global_face_next_global_successor_count:
                    partition_border_global_face_next_candidates.global_successor_count,
                partition_border_global_face_identity_candidate_cycle_count:
                    partition_border_global_face_identity_plans.candidate_cycle_count,
                partition_border_global_face_identity_closed_cycle_count:
                    partition_border_global_face_identity_plans.closed_cycle_count,
                partition_border_global_face_identity_incomplete_component_count:
                    partition_border_global_face_identity_plans.incomplete_component_count,
                partition_border_global_face_identity_non_permutation_component_count:
                    partition_border_global_face_identity_plans.non_permutation_component_count,
                partition_border_global_face_identity_boundary_observation_count:
                    partition_border_global_face_identity_plans.boundary_observation_count,
                partition_border_global_face_identity_permutation_ready:
                    partition_border_global_face_identity_plans.permutation_ready,
                partition_border_global_face_next_mutation_plan_count:
                    partition_border_global_face_next_mutation_plans.plan_count,
                partition_border_global_face_next_mutation_candidate_link_count:
                    partition_border_global_face_next_mutation_plans.candidate_link_count,
                partition_border_global_face_next_mutation_boundary_observation_count:
                    partition_border_global_face_next_mutation_plans.boundary_observation_count,
                partition_border_global_face_next_mutation_ready_component_count:
                    partition_border_global_face_next_mutation_plans.ready_component_count,
                partition_border_global_face_next_mutation_incomplete_component_count:
                    partition_border_global_face_next_mutation_plans.incomplete_component_count,
                partition_border_global_face_next_mutation_ready:
                    partition_border_global_face_next_mutation_plans.mutation_ready,
                partition_border_global_face_id_candidate_cycle_count:
                    partition_border_global_face_id_plans.candidate_cycle_count,
                partition_border_global_face_id_assigned_count:
                    partition_border_global_face_id_plans.assigned_face_count,
                partition_border_global_face_id_boundary_observation_count:
                    partition_border_global_face_id_plans.boundary_observation_count,
                partition_border_global_face_id_unbounded_candidate_count:
                    partition_border_global_face_id_plans.unbounded_candidate_count,
                partition_border_global_face_id_incomplete_plan_count:
                    partition_border_global_face_id_plans.incomplete_plan_count,
                partition_border_global_face_id_assignment_ready:
                    partition_border_global_face_id_plans.assignment_ready,
                partition_border_global_face_id_application_candidate_cycle_count:
                    partition_border_global_face_id_application.candidate_cycle_count,
                partition_border_global_face_id_application_assigned_face_count:
                    partition_border_global_face_id_application.assigned_face_count,
                partition_border_global_face_id_application_cycle_start_count:
                    partition_border_global_face_id_application.candidate_cycle_start_count,
                partition_border_global_face_id_application_mapped_cycle_count:
                    partition_border_global_face_id_application.mapped_cycle_count,
                partition_border_global_face_id_application_unmapped_plan_count:
                    partition_border_global_face_id_application.unmapped_plan_count,
                partition_border_global_face_id_application_duplicate_face_id_count:
                    partition_border_global_face_id_application.duplicate_face_id_count,
                partition_border_global_face_id_application_non_contiguous_face_id_count:
                    partition_border_global_face_id_application.non_contiguous_face_id_count,
                partition_border_global_face_id_application_ready:
                    partition_border_global_face_id_application.application_ready,
                partition_border_global_unbounded_face_proof_candidate_count:
                    partition_border_global_unbounded_face_proof.candidate_count,
                partition_border_global_unbounded_face_proof_ready:
                    partition_border_global_unbounded_face_proof.proof_ready,
                partition_border_global_unbounded_face_application_candidate_cycle_count:
                    partition_border_global_unbounded_face_application.candidate_cycle_count,
                partition_border_global_unbounded_face_application_candidate_unbounded_face_id_count:
                    partition_border_global_unbounded_face_application
                        .candidate_unbounded_face_id_count,
                partition_border_global_unbounded_face_application_mapped_unbounded_cycle_count:
                    partition_border_global_unbounded_face_application
                        .mapped_unbounded_cycle_count,
                partition_border_global_unbounded_face_application_missing_unbounded_face_id_count:
                    partition_border_global_unbounded_face_application
                        .missing_unbounded_face_id_count,
                partition_border_global_unbounded_face_application_duplicate_unbounded_face_id_count:
                    partition_border_global_unbounded_face_application
                        .duplicate_unbounded_face_id_count,
                partition_border_global_unbounded_face_application_ready:
                    partition_border_global_unbounded_face_application.application_ready,
                partition_border_global_topology_mutation_gate_edge_count:
                    partition_border_global_topology_mutation_gate.edge_count,
                partition_border_global_topology_mutation_gate_component_count:
                    partition_border_global_topology_mutation_gate.component_count,
                partition_border_global_topology_mutation_gate_face_count:
                    partition_border_global_topology_mutation_gate.face_count,
                partition_border_global_topology_mutation_gate_candidate_cycle_count:
                    partition_border_global_topology_mutation_gate.candidate_cycle_count,
                partition_border_global_topology_mutation_gate_applied_twin_count:
                    partition_border_global_topology_mutation_gate.applied_twin_count,
                partition_border_global_topology_mutation_gate_mapped_twin_count:
                    partition_border_global_topology_mutation_gate.mapped_twin_count,
                partition_border_global_topology_mutation_gate_source_complete_twin_count:
                    partition_border_global_topology_mutation_gate.source_complete_twin_count,
                partition_border_global_topology_mutation_gate_closed_face_count:
                    partition_border_global_topology_mutation_gate.closed_face_count,
                partition_border_global_topology_mutation_gate_topology_application_ready:
                    partition_border_global_topology_mutation_gate.topology_application_ready,
                partition_border_global_topology_mutation_gate_component_coverage_ready:
                    partition_border_global_topology_mutation_gate.component_coverage_ready,
                partition_border_global_topology_mutation_gate_face_id_application_ready:
                    partition_border_global_topology_mutation_gate.face_id_application_ready,
                partition_border_global_topology_mutation_gate_unbounded_face_application_ready:
                    partition_border_global_topology_mutation_gate
                        .unbounded_face_application_ready,
                partition_border_global_topology_mutation_gate_face_walk_ready:
                    partition_border_global_topology_mutation_gate.face_walk_ready,
                partition_border_global_topology_mutation_gate_euler_evidence_ready:
                    partition_border_global_topology_mutation_gate.euler_evidence_ready,
                partition_border_global_topology_mutation_gate_ready:
                    partition_border_global_topology_mutation_gate.gate_ready,
                partition_border_global_topology_mutation_applied_next_count:
                    partition_border_global_topology_mutation.applied_next_count,
                partition_border_global_topology_mutation_ready:
                    partition_border_global_topology_mutation.mutation_ready,
                partition_border_global_topology_mutation_applied:
                    partition_border_global_topology_mutation.applied,
                partition_border_global_face_id_mutation_candidate_cycle_count:
                    partition_border_global_face_id_mutation.candidate_cycle_count,
                partition_border_global_face_id_mutation_applied_face_id_count:
                    partition_border_global_face_id_mutation.applied_face_id_count,
                partition_border_global_face_id_mutation_unbounded_face_id_count:
                    partition_border_global_face_id_mutation.unbounded_face_id_count,
                partition_border_global_face_id_mutation_ready:
                    partition_border_global_face_id_mutation.mutation_ready,
                partition_border_global_face_id_mutation_applied:
                    partition_border_global_face_id_mutation.applied,
                partition_border_global_unbounded_face_mutation_candidate_cycle_count:
                    partition_border_global_unbounded_face_mutation.candidate_cycle_count,
                partition_border_global_unbounded_face_mutation_candidate_unbounded_face_id_count:
                    partition_border_global_unbounded_face_mutation
                        .candidate_unbounded_face_id_count,
                partition_border_global_unbounded_face_mutation_applied_unbounded_face_id:
                    partition_border_global_unbounded_face_mutation
                        .applied_unbounded_face_id,
                partition_border_global_unbounded_face_mutation_applied_cycle_start_global_dir_edge_id:
                    partition_border_global_unbounded_face_mutation
                        .applied_cycle_start_global_dir_edge_id,
                partition_border_global_unbounded_face_mutation_ready:
                    partition_border_global_unbounded_face_mutation.mutation_ready,
                partition_border_global_unbounded_face_mutation_applied:
                    partition_border_global_unbounded_face_mutation.applied,
                partition_border_global_face_identity_edge_count:
                    partition_border_global_face_identity_materialization.edge_count,
                partition_border_global_face_identity_cycle_count:
                    partition_border_global_face_identity_materialization.cycle_count,
                partition_border_global_face_identity_assigned_edge_count:
                    partition_border_global_face_identity_materialization.assigned_edge_count,
                partition_border_global_face_identity_missing_face_id_count:
                    partition_border_global_face_identity_materialization.missing_face_id_count,
                partition_border_global_face_identity_invalid_cycle_count:
                    partition_border_global_face_identity_materialization.invalid_cycle_count,
                partition_border_global_face_identity_unbounded_edge_count:
                    partition_border_global_face_identity_materialization.unbounded_edge_count,
                partition_border_global_face_identity_materialization_ready:
                    partition_border_global_face_identity_materialization.materialization_ready,
                partition_border_global_face_topology_edge_count:
                    partition_border_global_face_topology.edge_count,
                partition_border_global_face_topology_next_link_count:
                    partition_border_global_face_topology.next_link_count,
                partition_border_global_face_topology_face_id_count:
                    partition_border_global_face_topology.face_id_count,
                partition_border_global_face_topology_missing_next_count:
                    partition_border_global_face_topology.missing_next_count,
                partition_border_global_face_topology_invalid_next_count:
                    partition_border_global_face_topology.invalid_next_count,
                partition_border_global_face_topology_duplicate_next_count:
                    partition_border_global_face_topology.duplicate_next_count,
                partition_border_global_face_topology_node_discontinuity_count:
                    partition_border_global_face_topology.node_discontinuity_count,
                partition_border_global_face_topology_missing_face_id_count:
                    partition_border_global_face_topology.missing_face_id_count,
                partition_border_global_face_topology_non_contiguous_face_id_count:
                    partition_border_global_face_topology.non_contiguous_face_id_count,
                partition_border_global_face_topology_unbounded_edge_count:
                    partition_border_global_face_topology.unbounded_edge_count,
                partition_border_global_face_topology_unbounded_face_id_count:
                    partition_border_global_face_topology.unbounded_face_id_count,
                partition_border_global_face_topology_unbounded_cycle_start_count:
                    partition_border_global_face_topology.unbounded_cycle_start_count,
                partition_border_global_face_topology_missing_unbounded_identity_count:
                    partition_border_global_face_topology.missing_unbounded_identity_count,
                partition_border_global_face_topology_unbounded_identity_mismatch_count:
                    partition_border_global_face_topology.unbounded_identity_mismatch_count,
                partition_border_global_face_topology_evidence_mismatch_count:
                    partition_border_global_face_topology.evidence_mismatch_count,
                partition_border_global_face_topology_unbounded_face_ready:
                    partition_border_global_face_topology.unbounded_face_ready,
                partition_border_global_face_topology_ready:
                    partition_border_global_face_topology.topology_ready,
                partition_border_global_face_invariant_gate_edge_count:
                    partition_border_global_face_invariant_gate.edge_count,
                partition_border_global_face_invariant_gate_cycle_count:
                    partition_border_global_face_invariant_gate.cycle_count,
                partition_border_global_face_invariant_gate_edge_count_mismatch_count:
                    partition_border_global_face_invariant_gate.edge_count_mismatch_count,
                partition_border_global_face_invariant_gate_cycle_count_mismatch_count:
                    partition_border_global_face_invariant_gate.cycle_count_mismatch_count,
                partition_border_global_face_invariant_gate_twin_mismatch_count:
                    partition_border_global_face_invariant_gate.twin_mismatch_count,
                partition_border_global_face_invariant_gate_cycle_mismatch_count:
                    partition_border_global_face_invariant_gate.cycle_mismatch_count,
                partition_border_global_face_invariant_gate_source_mismatch_count:
                    partition_border_global_face_invariant_gate.source_mismatch_count,
                partition_border_global_face_invariant_gate_face_walk_failure_count:
                    partition_border_global_face_invariant_gate.face_walk_failure_count,
                partition_border_global_face_invariant_gate_euler_failure_count:
                    partition_border_global_face_invariant_gate.euler_failure_count,
                partition_border_global_face_invariant_gate_evidence_mismatch_count:
                    partition_border_global_face_invariant_gate.evidence_mismatch_count,
                partition_border_global_face_invariant_gate_identity_ready:
                    partition_border_global_face_invariant_gate.identity_invariants_ready,
                partition_border_global_face_invariant_gate_next_lineage_ready:
                    partition_border_global_face_invariant_gate.next_lineage_ready,
                partition_border_global_face_invariant_gate_cycle_face_lineage_ready:
                    partition_border_global_face_invariant_gate.cycle_face_lineage_ready,
                partition_border_global_face_invariant_gate_payload_lineage_ready:
                    partition_border_global_face_invariant_gate.payload_lineage_ready,
                partition_border_global_face_invariant_gate_geometry_ready:
                    partition_border_global_face_invariant_gate.geometry_ready,
                partition_border_global_face_invariant_gate_topology_ready:
                    partition_border_global_face_invariant_gate.topology_ready,
                partition_border_global_face_invariant_gate_extraction_gate_ready:
                    partition_border_global_face_invariant_gate.extraction_gate_ready,
                partition_border_global_face_invariant_gate_ready:
                    partition_border_global_face_invariant_gate.gate_ready,
                partition_border_global_face_identity_invariant_twin_count:
                    partition_border_global_face_identity_invariants.twin_count,
                partition_border_global_face_identity_invariant_twin_mapping_mismatch_count:
                    partition_border_global_face_identity_invariants.twin_mapping_mismatch_count,
                partition_border_global_face_identity_invariant_cycle_face_mismatch_count:
                    partition_border_global_face_identity_invariants.cycle_face_mismatch_count,
                partition_border_global_face_identity_invariant_successor_discontinuity_count:
                    partition_border_global_face_identity_invariants
                        .successor_discontinuity_count,
                partition_border_global_face_identity_invariant_source_incomplete_edge_count:
                    partition_border_global_face_identity_invariants
                        .source_incomplete_edge_count,
                partition_border_global_face_identity_invariant_face_walk_ready:
                    partition_border_global_face_identity_invariants.face_walk_ready,
                partition_border_global_face_identity_invariant_euler_ready:
                    partition_border_global_face_identity_invariants.euler_evidence_ready,
                partition_border_global_face_identity_invariants_ready:
                    partition_border_global_face_identity_invariants.invariants_ready,
                partition_border_global_next_lineage_integration_edge_count:
                    partition_border_global_next_lineage_integration.edge_count,
                partition_border_global_next_lineage_integration_cycle_count:
                    partition_border_global_next_lineage_integration.cycle_count,
                partition_border_global_next_lineage_integration_local_successor_count:
                    partition_border_global_next_lineage_integration.local_successor_count,
                partition_border_global_next_lineage_integration_override_count:
                    partition_border_global_next_lineage_integration.override_count,
                partition_border_global_next_lineage_integration_successor_count:
                    partition_border_global_next_lineage_integration.integrated_successor_count,
                partition_border_global_next_lineage_integration_missing_successor_count:
                    partition_border_global_next_lineage_integration
                        .missing_candidate_successor_count,
                partition_border_global_next_lineage_integration_local_mismatch_count:
                    partition_border_global_next_lineage_integration.local_lineage_mismatch_count,
                partition_border_global_next_lineage_integration_override_mismatch_count:
                    partition_border_global_next_lineage_integration
                        .override_lineage_mismatch_count,
                partition_border_global_next_lineage_integration_plan_link_count:
                    partition_border_global_next_lineage_integration.application_plan_link_count,
                partition_border_global_next_lineage_integration_unrepresented_link_count:
                    partition_border_global_next_lineage_integration
                        .unrepresented_application_link_count,
                partition_border_global_next_lineage_integration_committed_next_count:
                    partition_border_global_next_lineage_integration.committed_next_edge_count,
                partition_border_global_next_lineage_integration_committed_next_mismatch_count:
                    partition_border_global_next_lineage_integration
                        .committed_next_mismatch_count,
                partition_border_global_next_lineage_integration_twin_count:
                    partition_border_global_next_lineage_integration.twin_count,
                partition_border_global_next_lineage_integration_twin_mismatch_count:
                    partition_border_global_next_lineage_integration
                        .twin_lineage_mismatch_count,
                partition_border_global_next_lineage_integration_identity_ready:
                    partition_border_global_next_lineage_integration.identity_ready,
                partition_border_global_next_lineage_integration_ready:
                    partition_border_global_next_lineage_integration.integration_ready,
                partition_border_global_cycle_face_lineage_edge_count:
                    partition_border_global_cycle_face_lineage.edge_count,
                partition_border_global_cycle_face_lineage_cycle_count:
                    partition_border_global_cycle_face_lineage.cycle_count,
                partition_border_global_cycle_face_lineage_plan_count:
                    partition_border_global_cycle_face_lineage.plan_count,
                partition_border_global_cycle_face_lineage_closed_cycle_count:
                    partition_border_global_cycle_face_lineage.closed_cycle_count,
                partition_border_global_cycle_face_lineage_mapped_cycle_count:
                    partition_border_global_cycle_face_lineage.mapped_cycle_count,
                partition_border_global_cycle_face_lineage_incomplete_cycle_count:
                    partition_border_global_cycle_face_lineage.incomplete_cycle_count,
                partition_border_global_cycle_face_lineage_invalid_cycle_count:
                    partition_border_global_cycle_face_lineage.invalid_cycle_count,
                partition_border_global_cycle_face_lineage_missing_face_id_count:
                    partition_border_global_cycle_face_lineage.missing_face_id_count,
                partition_border_global_cycle_face_lineage_duplicate_face_id_plan_count:
                    partition_border_global_cycle_face_lineage.duplicate_face_id_plan_count,
                partition_border_global_cycle_face_lineage_unmapped_plan_count:
                    partition_border_global_cycle_face_lineage.unmapped_plan_count,
                partition_border_global_cycle_face_lineage_cycle_plan_mismatch_count:
                    partition_border_global_cycle_face_lineage.cycle_plan_mismatch_count,
                partition_border_global_cycle_face_lineage_cycle_face_ref_mismatch_count:
                    partition_border_global_cycle_face_lineage.cycle_face_ref_mismatch_count,
                partition_border_global_cycle_face_lineage_duplicate_plan_face_ref_count:
                    partition_border_global_cycle_face_lineage.duplicate_plan_face_ref_count,
                partition_border_global_cycle_face_lineage_observation_mismatch_count:
                    partition_border_global_cycle_face_lineage
                        .observation_lineage_mismatch_count,
                partition_border_global_cycle_face_lineage_unbounded_mismatch_count:
                    partition_border_global_cycle_face_lineage.unbounded_lineage_mismatch_count,
                partition_border_global_cycle_face_lineage_identity_ready:
                    partition_border_global_cycle_face_lineage.identity_ready,
                partition_border_global_cycle_face_lineage_next_ready:
                    partition_border_global_cycle_face_lineage.next_lineage_ready,
                partition_border_global_cycle_face_lineage_ready:
                    partition_border_global_cycle_face_lineage.lineage_ready,
                partition_border_global_cycle_face_promotion_gate_edge_count:
                    partition_border_global_cycle_face_promotion_gate.edge_count,
                partition_border_global_cycle_face_promotion_gate_cycle_count:
                    partition_border_global_cycle_face_promotion_gate.cycle_count,
                partition_border_global_cycle_face_promotion_gate_plan_count:
                    partition_border_global_cycle_face_promotion_gate.plan_count,
                partition_border_global_cycle_face_promotion_gate_component_count:
                    partition_border_global_cycle_face_promotion_gate.component_count,
                partition_border_global_cycle_face_promotion_gate_face_count:
                    partition_border_global_cycle_face_promotion_gate.face_count,
                partition_border_global_cycle_face_promotion_gate_covered_face_edge_count:
                    partition_border_global_cycle_face_promotion_gate.covered_face_edge_count,
                partition_border_global_cycle_face_promotion_gate_candidate_unbounded_face_id_count:
                    partition_border_global_cycle_face_promotion_gate
                        .candidate_unbounded_face_id_count,
                partition_border_global_cycle_face_promotion_gate_mapped_unbounded_cycle_count:
                    partition_border_global_cycle_face_promotion_gate
                        .mapped_unbounded_cycle_count,
                partition_border_global_cycle_face_promotion_gate_lineage_ready:
                    partition_border_global_cycle_face_promotion_gate.lineage_ready,
                partition_border_global_cycle_face_promotion_gate_component_coverage_ready:
                    partition_border_global_cycle_face_promotion_gate.component_coverage_ready,
                partition_border_global_cycle_face_promotion_gate_unbounded_face_application_ready:
                    partition_border_global_cycle_face_promotion_gate
                        .unbounded_face_application_ready,
                partition_border_global_cycle_face_promotion_gate_edge_count_mismatch_count:
                    partition_border_global_cycle_face_promotion_gate
                        .edge_count_mismatch_count,
                partition_border_global_cycle_face_promotion_gate_cycle_count_mismatch_count:
                    partition_border_global_cycle_face_promotion_gate
                        .cycle_count_mismatch_count,
                partition_border_global_cycle_face_promotion_gate_plan_count_mismatch_count:
                    partition_border_global_cycle_face_promotion_gate
                        .plan_count_mismatch_count,
                partition_border_global_cycle_face_promotion_gate_face_count_mismatch_count:
                    partition_border_global_cycle_face_promotion_gate
                        .face_count_mismatch_count,
                partition_border_global_cycle_face_promotion_gate_unbounded_marker_mismatch_count:
                    partition_border_global_cycle_face_promotion_gate
                        .unbounded_marker_mismatch_count,
                partition_border_global_cycle_face_promotion_gate_ready:
                    partition_border_global_cycle_face_promotion_gate.gate_ready,
                partition_border_global_face_payload_lineage_edge_count:
                    partition_border_global_face_payload_lineage.edge_count,
                partition_border_global_face_payload_lineage_cycle_count:
                    partition_border_global_face_payload_lineage.cycle_count,
                partition_border_global_face_payload_lineage_plan_count:
                    partition_border_global_face_payload_lineage.plan_count,
                partition_border_global_face_payload_lineage_checked_edge_count:
                    partition_border_global_face_payload_lineage.checked_edge_count,
                partition_border_global_face_payload_lineage_checked_cycle_count:
                    partition_border_global_face_payload_lineage.checked_cycle_count,
                partition_border_global_face_payload_lineage_missing_face_id_count:
                    partition_border_global_face_payload_lineage.missing_face_id_count,
                partition_border_global_face_payload_lineage_missing_plan_count:
                    partition_border_global_face_payload_lineage.missing_plan_count,
                partition_border_global_face_payload_lineage_missing_observation_count:
                    partition_border_global_face_payload_lineage.missing_observation_count,
                partition_border_global_face_payload_lineage_source_incomplete_edge_count:
                    partition_border_global_face_payload_lineage.source_incomplete_edge_count,
                partition_border_global_face_payload_lineage_source_mismatch_count:
                    partition_border_global_face_payload_lineage.source_lineage_mismatch_count,
                partition_border_global_face_payload_lineage_z_mismatch_count:
                    partition_border_global_face_payload_lineage.z_lineage_mismatch_count,
                partition_border_global_face_payload_lineage_face_mismatch_count:
                    partition_border_global_face_payload_lineage.face_lineage_mismatch_count,
                partition_border_global_face_payload_lineage_node_mismatch_count:
                    partition_border_global_face_payload_lineage.node_lineage_mismatch_count,
                partition_border_global_face_payload_lineage_ready:
                    partition_border_global_face_payload_lineage.lineage_ready,
                partition_border_global_face_cycle_geometry_edge_count:
                    partition_border_global_face_cycle_geometry.edge_count,
                partition_border_global_face_cycle_geometry_cycle_count:
                    partition_border_global_face_cycle_geometry.cycle_count,
                partition_border_global_face_cycle_geometry_checked_cycle_count:
                    partition_border_global_face_cycle_geometry.checked_cycle_count,
                partition_border_global_face_cycle_geometry_closed_cycle_count:
                    partition_border_global_face_cycle_geometry.closed_cycle_count,
                partition_border_global_face_cycle_geometry_missing_node_count:
                    partition_border_global_face_cycle_geometry.missing_node_count,
                partition_border_global_face_cycle_geometry_node_discontinuity_count:
                    partition_border_global_face_cycle_geometry.node_discontinuity_count,
                partition_border_global_face_cycle_geometry_repeated_edge_count:
                    partition_border_global_face_cycle_geometry.repeated_edge_count,
                partition_border_global_face_cycle_geometry_missing_face_id_count:
                    partition_border_global_face_cycle_geometry.missing_face_id_count,
                partition_border_global_face_cycle_geometry_missing_interior_point_count:
                    partition_border_global_face_cycle_geometry.missing_interior_point_count,
                partition_border_global_face_cycle_geometry_degenerate_cycle_count:
                    partition_border_global_face_cycle_geometry.degenerate_cycle_count,
                partition_border_global_face_cycle_geometry_positive_cycle_count:
                    partition_border_global_face_cycle_geometry.positive_cycle_count,
                partition_border_global_face_cycle_geometry_negative_cycle_count:
                    partition_border_global_face_cycle_geometry.negative_cycle_count,
                partition_border_global_face_cycle_geometry_unbounded_cycle_count:
                    partition_border_global_face_cycle_geometry.unbounded_cycle_count,
                partition_border_global_face_cycle_geometry_unbounded_orientation_mismatch_count:
                    partition_border_global_face_cycle_geometry
                        .unbounded_orientation_mismatch_count,
                partition_border_global_face_cycle_geometry_containment_pair_count:
                    partition_border_global_face_cycle_geometry.containment_pair_count,
                partition_border_global_face_cycle_geometry_contained_cycle_count:
                    partition_border_global_face_cycle_geometry.contained_cycle_count,
                partition_border_global_face_cycle_geometry_nested_opposite_orientation_pair_count:
                    partition_border_global_face_cycle_geometry
                        .nested_opposite_orientation_pair_count,
                partition_border_global_face_cycle_geometry_nested_same_orientation_pair_count:
                    partition_border_global_face_cycle_geometry
                        .nested_same_orientation_pair_count,
                partition_border_global_face_cycle_geometry_edge_pair_count:
                    partition_border_global_face_cycle_geometry.edge_pair_count,
                partition_border_global_face_cycle_geometry_checked_edge_pair_count:
                    partition_border_global_face_cycle_geometry.checked_edge_pair_count,
                partition_border_global_face_cycle_geometry_expected_reciprocal_pair_count:
                    partition_border_global_face_cycle_geometry.expected_reciprocal_pair_count,
                partition_border_global_face_cycle_geometry_proper_crossing_count:
                    partition_border_global_face_cycle_geometry.proper_crossing_count,
                partition_border_global_face_cycle_geometry_endpoint_touch_count:
                    partition_border_global_face_cycle_geometry.endpoint_touch_count,
                partition_border_global_face_cycle_geometry_boundary_touch_count:
                    partition_border_global_face_cycle_geometry.boundary_touch_count,
                partition_border_global_face_cycle_geometry_collinear_overlap_count:
                    partition_border_global_face_cycle_geometry.collinear_overlap_count,
                partition_border_global_face_cycle_geometry_unexpected_collinear_overlap_count:
                    partition_border_global_face_cycle_geometry
                        .unexpected_collinear_overlap_count,
                partition_border_global_face_cycle_geometry_interaction_ready:
                    partition_border_global_face_cycle_geometry.interaction_ready,
                partition_border_global_face_cycle_geometry_canonical_ring_count:
                    partition_border_global_face_cycle_geometry.canonical_ring_count,
                partition_border_global_face_cycle_geometry_canonical_ring_mismatch_count:
                    partition_border_global_face_cycle_geometry.canonical_ring_mismatch_count,
                partition_border_global_face_cycle_geometry_self_intersection_count:
                    partition_border_global_face_cycle_geometry.self_intersection_count,
                partition_border_global_face_cycle_geometry_reciprocal_edge_count:
                    partition_border_global_face_cycle_geometry.reciprocal_edge_count,
                partition_border_global_face_cycle_geometry_reciprocal_edge_mismatch_count:
                    partition_border_global_face_cycle_geometry.reciprocal_edge_mismatch_count,
                partition_border_global_face_cycle_geometry_ring_payload_ready:
                    partition_border_global_face_cycle_geometry.ring_payload_ready,
                partition_border_global_face_cycle_geometry_ready:
                    partition_border_global_face_cycle_geometry.geometry_ready,
                partition_border_global_face_extraction_gate_ready:
                    partition_border_global_face_extraction_gate.extraction_ready,
                partition_border_global_face_extraction_gate_edge_count_mismatch_count:
                    partition_border_global_face_extraction_gate.edge_count_mismatch_count,
                partition_border_global_face_extraction_gate_cycle_count_mismatch_count:
                    partition_border_global_face_extraction_gate.cycle_count_mismatch_count,
                partition_border_global_face_ring_payload_edge_count:
                    partition_border_global_face_ring_payloads.edge_count,
                partition_border_global_face_ring_payload_cycle_count:
                    partition_border_global_face_ring_payloads.cycle_count,
                partition_border_global_face_ring_payload_materialized_cycle_count:
                    partition_border_global_face_ring_payloads.materialized_cycle_count,
                partition_border_global_face_ring_payload_coordinate_count:
                    partition_border_global_face_ring_payloads.coordinate_count,
                partition_border_global_face_ring_payload_source_line_id_count:
                    partition_border_global_face_ring_payloads.source_line_id_count,
                partition_border_global_face_ring_payload_missing_face_id_count:
                    partition_border_global_face_ring_payloads.missing_face_id_count,
                partition_border_global_face_ring_payload_missing_edge_face_id_count:
                    partition_border_global_face_ring_payloads.missing_edge_face_id_count,
                partition_border_global_face_ring_payload_invalid_cycle_count:
                    partition_border_global_face_ring_payloads.invalid_cycle_count,
                partition_border_global_face_ring_payload_canonical_ring_mismatch_count:
                    partition_border_global_face_ring_payloads.canonical_ring_mismatch_count,
                partition_border_global_face_ring_payload_unbounded_cycle_count:
                    partition_border_global_face_ring_payloads.unbounded_cycle_count,
                partition_border_global_face_ring_payload_ready:
                    partition_border_global_face_ring_payloads.materialization_ready,
                partition_border_global_face_ring_classification_cycle_count:
                    partition_border_global_face_ring_classification.cycle_count,
                partition_border_global_face_ring_classification_classified_cycle_count:
                    partition_border_global_face_ring_classification.classified_cycle_count,
                partition_border_global_face_ring_classification_shell_candidate_count:
                    partition_border_global_face_ring_classification.shell_candidate_count,
                partition_border_global_face_ring_classification_hole_candidate_count:
                    partition_border_global_face_ring_classification.hole_candidate_count,
                partition_border_global_face_ring_classification_unbounded_cycle_count:
                    partition_border_global_face_ring_classification.unbounded_cycle_count,
                partition_border_global_face_ring_classification_containment_pair_count:
                    partition_border_global_face_ring_classification.containment_pair_count,
                partition_border_global_face_ring_classification_contained_cycle_count:
                    partition_border_global_face_ring_classification.contained_cycle_count,
                partition_border_global_face_ring_classification_nested_same_orientation_pair_count:
                    partition_border_global_face_ring_classification
                        .nested_same_orientation_pair_count,
                partition_border_global_face_ring_classification_ambiguous_interaction_count:
                    partition_border_global_face_ring_classification.ambiguous_interaction_count,
                partition_border_global_face_ring_classification_missing_interior_point_count:
                    partition_border_global_face_ring_classification.missing_interior_point_count,
                partition_border_global_face_ring_classification_invalid_cycle_count:
                    partition_border_global_face_ring_classification.invalid_cycle_count,
                partition_border_global_face_ring_classification_evidence_mismatch_count:
                    partition_border_global_face_ring_classification.evidence_mismatch_count,
                partition_border_global_face_ring_classification_ready:
                    partition_border_global_face_ring_classification.classification_ready,
                partition_border_global_face_ring_candidate_assembly_cycle_count:
                    partition_border_global_face_ring_candidate_assembly.cycle_count,
                partition_border_global_face_ring_candidate_assembly_shell_candidate_count:
                    partition_border_global_face_ring_candidate_assembly.shell_candidate_count,
                partition_border_global_face_ring_candidate_assembly_hole_candidate_count:
                    partition_border_global_face_ring_candidate_assembly.hole_candidate_count,
                partition_border_global_face_ring_candidate_assembly_assembled_shell_count:
                    partition_border_global_face_ring_candidate_assembly.assembled_shell_count,
                partition_border_global_face_ring_candidate_assembly_assigned_hole_count:
                    partition_border_global_face_ring_candidate_assembly.assigned_hole_count,
                partition_border_global_face_ring_candidate_assembly_unassigned_hole_count:
                    partition_border_global_face_ring_candidate_assembly.unassigned_hole_count,
                partition_border_global_face_ring_candidate_assembly_ambiguous_hole_count:
                    partition_border_global_face_ring_candidate_assembly.ambiguous_hole_count,
                partition_border_global_face_ring_candidate_assembly_evidence_mismatch_count:
                    partition_border_global_face_ring_candidate_assembly.evidence_mismatch_count,
                partition_border_global_face_ring_candidate_assembly_ready:
                    partition_border_global_face_ring_candidate_assembly.candidate_ready,
                partition_border_global_face_ring_extraction_candidate_cycle_count:
                    partition_border_global_face_ring_extraction_readiness.cycle_count,
                partition_border_global_face_ring_extraction_candidate_shell_count:
                    partition_border_global_face_ring_extraction_readiness.candidate_shell_count,
                partition_border_global_face_ring_extraction_candidate_hole_count:
                    partition_border_global_face_ring_extraction_readiness.candidate_hole_count,
                partition_border_global_face_ring_extraction_candidate_coordinate_count:
                    partition_border_global_face_ring_extraction_readiness.candidate_coordinate_count,
                partition_border_global_face_ring_extraction_candidate_source_line_id_count:
                    partition_border_global_face_ring_extraction_readiness
                        .candidate_source_line_id_count,
                partition_border_global_face_ring_extraction_missing_payload_count:
                    partition_border_global_face_ring_extraction_readiness.missing_payload_count,
                partition_border_global_face_ring_extraction_duplicate_face_id_count:
                    partition_border_global_face_ring_extraction_readiness
                        .duplicate_face_id_count,
                partition_border_global_face_ring_extraction_duplicate_cycle_start_count:
                    partition_border_global_face_ring_extraction_readiness
                        .duplicate_cycle_start_count,
                partition_border_global_face_ring_extraction_duplicate_candidate_count:
                    partition_border_global_face_ring_extraction_readiness
                        .duplicate_candidate_count,
                partition_border_global_face_ring_extraction_unbounded_payload_count:
                    partition_border_global_face_ring_extraction_readiness
                        .unbounded_payload_count,
                partition_border_global_face_ring_extraction_invalid_coordinate_count:
                    partition_border_global_face_ring_extraction_readiness
                        .invalid_coordinate_count,
                partition_border_global_face_ring_extraction_source_lineage_mismatch_count:
                    partition_border_global_face_ring_extraction_readiness
                        .source_lineage_mismatch_count,
                partition_border_global_face_ring_extraction_evidence_mismatch_count:
                    partition_border_global_face_ring_extraction_readiness
                        .evidence_mismatch_count,
                partition_border_global_face_ring_extraction_ready:
                    partition_border_global_face_ring_extraction_readiness.candidate_ready,
                partition_border_global_face_ring_extraction_payload_candidate_count:
                    partition_border_global_face_ring_extraction_payloads.candidate_count,
                partition_border_global_face_ring_extraction_payload_materialized_candidate_count:
                    partition_border_global_face_ring_extraction_payloads
                        .materialized_candidate_count,
                partition_border_global_face_ring_extraction_payload_shell_coordinate_count:
                    partition_border_global_face_ring_extraction_payloads.shell_coordinate_count,
                partition_border_global_face_ring_extraction_payload_hole_coordinate_count:
                    partition_border_global_face_ring_extraction_payloads.hole_coordinate_count,
                partition_border_global_face_ring_extraction_payload_source_line_id_count:
                    partition_border_global_face_ring_extraction_payloads.source_line_id_count,
                partition_border_global_face_ring_extraction_payload_missing_count:
                    partition_border_global_face_ring_extraction_payloads.missing_payload_count,
                partition_border_global_face_ring_extraction_payload_duplicate_count:
                    partition_border_global_face_ring_extraction_payloads.duplicate_payload_count,
                partition_border_global_face_ring_extraction_payload_invalid_count:
                    partition_border_global_face_ring_extraction_payloads.invalid_payload_count,
                partition_border_global_face_ring_extraction_payload_source_lineage_mismatch_count:
                    partition_border_global_face_ring_extraction_payloads
                        .source_lineage_mismatch_count,
                partition_border_global_face_ring_extraction_payload_evidence_mismatch_count:
                    partition_border_global_face_ring_extraction_payloads
                        .evidence_mismatch_count,
                partition_border_global_face_ring_extraction_payload_ready:
                    partition_border_global_face_ring_extraction_payloads.payload_ready,
                partition_border_global_non_polygon_extraction_dangle_count:
                    partition_border_global_non_polygon_extraction.dangle_count,
                partition_border_global_non_polygon_extraction_cut_edge_count:
                    partition_border_global_non_polygon_extraction.cut_edge_count,
                partition_border_global_non_polygon_extraction_invalid_ring_count:
                    partition_border_global_non_polygon_extraction.invalid_ring_count,
                partition_border_global_non_polygon_extraction_coordinate_count:
                    partition_border_global_non_polygon_extraction.coordinate_count,
                partition_border_global_non_polygon_extraction_duplicate_payload_count:
                    partition_border_global_non_polygon_extraction.duplicate_payload_count,
                partition_border_global_non_polygon_extraction_invalid_coordinate_count:
                    partition_border_global_non_polygon_extraction.invalid_coordinate_count,
                partition_border_global_non_polygon_extraction_evidence_mismatch_count:
                    partition_border_global_non_polygon_extraction.evidence_mismatch_count,
                partition_border_global_non_polygon_extraction_ready:
                    partition_border_global_non_polygon_extraction.payload_ready,
                partition_border_global_extraction_readiness_edge_count:
                    partition_border_global_extraction_readiness.edge_count,
                partition_border_global_extraction_readiness_topology_edge_count:
                    partition_border_global_extraction_readiness.topology_edge_count,
                partition_border_global_extraction_readiness_candidate_shell_count:
                    partition_border_global_extraction_readiness.candidate_shell_count,
                partition_border_global_extraction_readiness_candidate_hole_count:
                    partition_border_global_extraction_readiness.candidate_hole_count,
                partition_border_global_extraction_readiness_candidate_coordinate_count:
                    partition_border_global_extraction_readiness.candidate_coordinate_count,
                partition_border_global_extraction_readiness_materialized_candidate_count:
                    partition_border_global_extraction_readiness.materialized_candidate_count,
                partition_border_global_extraction_readiness_non_polygon_payload_count:
                    partition_border_global_extraction_readiness.non_polygon_payload_count,
                partition_border_global_extraction_readiness_missing_topology_count:
                    partition_border_global_extraction_readiness.missing_topology_count,
                partition_border_global_extraction_readiness_missing_ring_candidate_count:
                    partition_border_global_extraction_readiness.missing_ring_candidate_count,
                partition_border_global_extraction_readiness_missing_ring_payload_count:
                    partition_border_global_extraction_readiness.missing_ring_payload_count,
                partition_border_global_extraction_readiness_missing_non_polygon_payload_count:
                    partition_border_global_extraction_readiness
                        .missing_non_polygon_payload_count,
                partition_border_global_extraction_readiness_missing_invariant_gate_count:
                    partition_border_global_extraction_readiness.missing_invariant_gate_count,
                partition_border_global_extraction_readiness_evidence_mismatch_count:
                    partition_border_global_extraction_readiness.evidence_mismatch_count,
                partition_border_global_extraction_readiness_invariant_gate_ready:
                    partition_border_global_extraction_readiness.invariant_gate_ready,
                partition_border_global_extraction_readiness_topology_ready:
                    partition_border_global_extraction_readiness.topology_ready,
                partition_border_global_extraction_readiness_ring_candidate_ready:
                    partition_border_global_extraction_readiness.ring_candidate_ready,
                partition_border_global_extraction_readiness_ring_payload_ready:
                    partition_border_global_extraction_readiness.ring_payload_ready,
                partition_border_global_extraction_readiness_non_polygon_payload_ready:
                    partition_border_global_extraction_readiness.non_polygon_payload_ready,
                partition_border_global_extraction_readiness_ready:
                    partition_border_global_extraction_readiness.extraction_ready,
                partition_border_global_private_extraction_ring_payload_count:
                    partition_border_global_private_extraction.ring_payload_count,
                partition_border_global_private_extraction_hole_count:
                    partition_border_global_private_extraction.hole_count,
                partition_border_global_private_extraction_dangle_count:
                    partition_border_global_private_extraction.dangle_count,
                partition_border_global_private_extraction_cut_edge_count:
                    partition_border_global_private_extraction.cut_edge_count,
                partition_border_global_private_extraction_invalid_ring_count:
                    partition_border_global_private_extraction.invalid_ring_count,
                partition_border_global_private_extraction_coordinate_count:
                    partition_border_global_private_extraction.coordinate_count,
                partition_border_global_private_extraction_source_line_id_count:
                    partition_border_global_private_extraction.source_line_id_count,
                partition_border_global_private_extraction_missing_ring_payload_count:
                    partition_border_global_private_extraction.missing_ring_payload_count,
                partition_border_global_private_extraction_missing_non_polygon_payload_count:
                    partition_border_global_private_extraction.missing_non_polygon_payload_count,
                partition_border_global_private_extraction_invalid_ring_payload_count:
                    partition_border_global_private_extraction.invalid_ring_payload_count,
                partition_border_global_private_extraction_invalid_non_polygon_payload_count:
                    partition_border_global_private_extraction.invalid_non_polygon_payload_count,
                partition_border_global_private_extraction_evidence_mismatch_count:
                    partition_border_global_private_extraction.evidence_mismatch_count,
                partition_border_global_private_extraction_ready:
                    partition_border_global_private_extraction.extraction_ready,
                partition_border_global_stitched_output_ready: stitched_output_ready,
                partition_border_global_untiled_equivalence_checked:
                    untiled_equivalence.checked,
                partition_border_global_untiled_equivalence_ready: untiled_equivalence.ready,
                partition_border_global_untiled_equivalence_mismatch_count:
                    untiled_equivalence.mismatch_count,
                partition_border_global_unbounded_face_proof_closed_count:
                    partition_border_global_unbounded_face_proof.closed_unbounded_face_count,
                partition_border_global_unbounded_face_proof_unmapped_twin_count:
                    partition_border_global_unbounded_face_proof.unbounded_face_unmapped_twin_count,
                partition_border_global_unbounded_face_proof_not_ready_twin_count:
                    partition_border_global_unbounded_face_proof.unbounded_face_not_ready_twin_count,
                partition_border_face_twin_count: applied_face_twin_count,
                partition_border_face_twin_missing_face_count: partition_border_twin_application
                    .missing_face_ref_count,
                partition_border_face_twin_invalid_face_count: partition_border_twin_application
                    .invalid_face_ref_count,
                partition_border_global_face_edge_map_local_graph_count:
                    partition_border_global_face_edge_map.local_graph_count,
                partition_border_global_face_edge_map_directed_edge_count:
                    partition_border_global_face_edge_map.directed_edge_count,
                partition_border_global_face_edge_map_local_successor_count:
                    partition_border_global_face_edge_map.local_successor_count,
                partition_border_global_face_edge_map_observation_count:
                    partition_border_global_face_edge_map.mapped_observation_count,
                partition_border_global_face_edge_map_twin_count:
                    partition_border_global_face_edge_map.mapped_twin_count,
                partition_border_global_face_edge_map_unmapped_twin_count:
                    partition_border_global_face_edge_map.unmapped_twin_count,
                partition_border_global_face_edge_map_ready: partition_border_global_face_edge_map
                    .edge_map_ready,
                partition_border_global_face_node_edge_count: partition_border_global_face_nodes
                    .edge_count,
                partition_border_global_face_node_count: partition_border_global_face_nodes
                    .node_count,
                partition_border_global_face_node_endpoint_count:
                    partition_border_global_face_nodes.endpoint_count,
                partition_border_global_face_node_observation_count:
                    partition_border_global_face_nodes.mapped_observation_count,
                partition_border_global_face_node_unmapped_observation_count:
                    partition_border_global_face_nodes.unmapped_observation_count,
                partition_border_global_face_node_z_candidate_count:
                    partition_border_global_face_nodes.z_candidate_count,
                partition_border_global_face_node_z_conflict_count:
                    partition_border_global_face_nodes.z_conflict_count,
                partition_border_global_face_node_ready: partition_border_global_face_nodes
                    .node_map_ready,
                partition_border_global_face_next_application_plan_count:
                    partition_border_global_face_next_application.plan_count,
                partition_border_global_face_next_application_link_count:
                    partition_border_global_face_next_application.candidate_link_count,
                partition_border_global_face_next_application_edge_count:
                    partition_border_global_face_next_application.mapped_edge_count,
                partition_border_global_face_next_application_twin_count:
                    partition_border_global_face_next_application.mapped_twin_count,
                partition_border_global_face_next_application_unmapped_observation_count:
                    partition_border_global_face_next_application.unmapped_observation_count,
                partition_border_global_face_next_application_incomplete_plan_count:
                    partition_border_global_face_next_application.incomplete_plan_count,
                partition_border_global_face_next_application_node_discontinuity_count:
                    partition_border_global_face_next_application.node_discontinuity_count,
                partition_border_global_face_next_application_ready:
                    partition_border_global_face_next_application.application_ready,
                partition_border_global_topology_candidate_edge_count:
                    partition_border_global_topology_candidate.edge_count,
                partition_border_global_topology_candidate_local_successor_count:
                    partition_border_global_topology_candidate.local_successor_count,
                partition_border_global_topology_candidate_global_override_count:
                    partition_border_global_topology_candidate.global_override_count,
                partition_border_global_topology_candidate_assigned_next_count:
                    partition_border_global_topology_candidate.assigned_next_count,
                partition_border_global_topology_candidate_unassigned_next_count:
                    partition_border_global_topology_candidate.unassigned_next_count,
                partition_border_global_topology_candidate_cycle_count:
                    partition_border_global_topology_candidate.cycle_count,
                partition_border_global_topology_candidate_closed_cycle_edge_count:
                    partition_border_global_topology_candidate.closed_cycle_edge_count,
                partition_border_global_topology_candidate_predecessor_conflict_count:
                    partition_border_global_topology_candidate.predecessor_conflict_count,
                partition_border_global_topology_candidate_node_discontinuity_count:
                    partition_border_global_topology_candidate.node_discontinuity_count,
                partition_border_global_topology_candidate_incomplete_application_plan_count:
                    partition_border_global_topology_candidate.incomplete_application_plan_count,
                partition_border_global_topology_candidate_ready:
                    partition_border_global_topology_candidate.candidate_ready,
                partition_border_global_topology_application_gate_edge_count:
                    partition_border_global_topology_application_gate.edge_count,
                partition_border_global_topology_application_gate_successor_count:
                    partition_border_global_topology_application_gate.candidate_successor_count,
                partition_border_global_topology_application_gate_adjacency_count:
                    partition_border_global_topology_application_gate.declared_adjacency_count,
                partition_border_global_topology_application_gate_applied_twin_count:
                    partition_border_global_topology_application_gate.applied_twin_count,
                partition_border_global_topology_application_gate_mapped_twin_count:
                    partition_border_global_topology_application_gate.mapped_twin_count,
                partition_border_global_topology_application_gate_unmapped_twin_count:
                    partition_border_global_topology_application_gate.unmapped_twin_count,
                partition_border_global_topology_application_gate_invalid_twin_count:
                    partition_border_global_topology_application_gate.invalid_twin_count,
                partition_border_global_topology_application_gate_predecessor_conflict_count:
                    partition_border_global_topology_application_gate.predecessor_conflict_count,
                partition_border_global_topology_application_gate_node_discontinuity_count:
                    partition_border_global_topology_application_gate.node_discontinuity_count,
                partition_border_global_topology_application_gate_ready:
                    partition_border_global_topology_application_gate.application_ready,
                partition_border_global_component_coverage_component_count:
                    partition_border_global_component_coverage.component_count,
                partition_border_global_component_coverage_face_count:
                    partition_border_global_component_coverage.face_count,
                partition_border_global_component_coverage_edge_count:
                    partition_border_global_component_coverage.edge_count,
                partition_border_global_component_coverage_face_edge_count:
                    partition_border_global_component_coverage.face_edge_count,
                partition_border_global_component_coverage_covered_face_edge_count:
                    partition_border_global_component_coverage.covered_face_edge_count,
                partition_border_global_component_coverage_uncovered_face_edge_count:
                    partition_border_global_component_coverage.uncovered_face_edge_count,
                partition_border_global_component_coverage_duplicate_face_count:
                    partition_border_global_component_coverage.duplicate_face_count,
                partition_border_global_component_coverage_duplicate_twin_edge_count:
                    partition_border_global_component_coverage.duplicate_twin_edge_count,
                partition_border_global_component_coverage_ready:
                    partition_border_global_component_coverage.coverage_ready,
                component_fallback_used,
                untiled_fallback_attempted,
                untiled_fallback_authoritative,
                untiled_fallback_output_polygon_count,
                untiled_fallback_used,
                coverage_resolution,
            },
        };
        if let Some(reason) = component_fallback_decline_reason {
            if let Some(trace) = trace.as_deref_mut() {
                trace.record_tile_component_fallback_declined(
                    result.stitching_report.unresolved_owned_polygon_count,
                    result.stitching_report.unresolved_ownership_domain_count,
                    result.stitching_report.unresolved_input_geometry_count,
                    result.stitching_report.unresolved_component_count,
                    reason,
                );
            }
        }
        if component_fallback_used {
            if let Some(trace) = trace.as_deref_mut() {
                for event in component_fallback_events {
                    trace.record_tile_component_fallback(
                        &event.input_geometry_indices,
                        event.output_polygon_count,
                        retained_tile_polygon_count,
                        event.replaced_retained_polygon_count,
                        event.recovered_component_count,
                    );
                }
            }
        }
        if untiled_fallback_used {
            if let Some(trace) = trace {
                trace.record_tile_untiled_fallback(
                    self.geometries.len(),
                    result.stitching_report.output_polygon_count,
                    result.stitching_report.unresolved_owned_polygon_count,
                    result.stitching_report.unresolved_ownership_domain_count,
                    result.stitching_report.unresolved_input_geometry_count,
                    result.stitching_report.unresolved_component_count,
                );
            }
        }
        Ok(result)
    }

    fn validate(&self) -> Result<()> {
        self.options.validate()?;
        self.execution_policy.check(
            "tile_input_geometries",
            self.tile_execution_policy.max_input_geometries,
            self.geometries.len(),
        )?;
        if self
            .tile_execution_policy
            .max_parallel_tiles
            .is_some_and(|parallelism| parallelism == 0)
        {
            return Err(PolygonizeError::InvalidArgumentType {
                field: "tile_execution_policy.max_parallel_tiles".to_string(),
                expected: "a positive integer".to_string(),
                actual: "0".to_string(),
            });
        }
        if !self.tile_size.is_finite() || self.tile_size <= 0.0 {
            return Err(PolygonizeError::InvalidArgumentType {
                field: "tile_size".to_string(),
                expected: "a finite positive number".to_string(),
                actual: self.tile_size.to_string(),
            });
        }
        if !self.buffer.is_finite() || self.buffer < 0.0 {
            return Err(PolygonizeError::InvalidArgumentType {
                field: "buffer".to_string(),
                expected: "a finite non-negative number".to_string(),
                actual: self.buffer.to_string(),
            });
        }
        if let Some(policy) = self.retry_policy {
            if policy.max_attempts == 0 {
                return Err(PolygonizeError::InvalidArgumentType {
                    field: "retry_policy.max_attempts".to_string(),
                    expected: "a positive integer".to_string(),
                    actual: policy.max_attempts.to_string(),
                });
            }
            if !policy.buffer_increment.is_finite() || policy.buffer_increment <= 0.0 {
                return Err(PolygonizeError::InvalidArgumentType {
                    field: "retry_policy.buffer_increment".to_string(),
                    expected: "a finite positive number".to_string(),
                    actual: policy.buffer_increment.to_string(),
                });
            }
            if !policy.max_buffer.is_finite() || policy.max_buffer <= self.buffer {
                return Err(PolygonizeError::InvalidArgumentType {
                    field: "retry_policy.max_buffer".to_string(),
                    expected: "a finite number greater than the initial buffer".to_string(),
                    actual: policy.max_buffer.to_string(),
                });
            }
        }
        let min = self.bbox.min();
        let max = self.bbox.max();
        if ![min.x, min.y, max.x, max.y].iter().all(|v| v.is_finite())
            || min.x >= max.x
            || min.y >= max.y
        {
            return Err(PolygonizeError::InvalidGeometry {
                reason: "tile bounding box must be finite with positive width and height"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tiling_tests.rs"]
mod tests;
