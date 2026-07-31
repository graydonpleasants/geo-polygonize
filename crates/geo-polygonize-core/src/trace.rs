//! Internal bounded topology trace schema.

use crate::fingerprint::coordinate_fingerprint;
use crate::graph::{ExtractedRing, PlanarGraph};
use crate::noding::grid::{
    UniformGridCandidateTrace, UniformGridCellTrace, UniformGridGlobalLineTrace,
};
use crate::noding::hot_pixel::{
    HotPixelCandidateTrace, HotPixelIntersectionTrace, HotPixelSplitTrace,
};
use crate::noding::snap::{FloatingCandidateTrace, FloatingIntersectionTrace, FloatingSplitTrace};
use crate::{CoordinateFingerprintV1, Line3D, Polygon3D, PolygonizerOptions, PolygonizerResult};
use serde::Serialize;

pub const TOPOLOGY_TRACE_V1_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceLevelV1 {
    Summary,
    Noding,
    Graph,
    Rings,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStageV1 {
    Summary,
    Noding,
    Graph,
    Rings,
    Output,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TraceEventV1 {
    pub sequence: usize,
    pub stage: TraceStageV1,
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InputSegmentTraceV1 {
    pub index: usize,
    pub start: CoordinateFingerprintV1,
    pub end: CoordinateFingerprintV1,
    pub source_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HotPixelTraceV1 {
    pub index: usize,
    pub grid_x: i64,
    pub grid_y: i64,
    pub coordinate: CoordinateFingerprintV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IntersectionWitnessTraceV1 {
    Point {
        coordinate: CoordinateFingerprintV1,
    },
    Collinear {
        start: CoordinateFingerprintV1,
        end: CoordinateFingerprintV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CandidatePairTraceV1 {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_index: Option<usize>,
    pub first_segment: usize,
    pub second_segment: usize,
    pub first_source_id: String,
    pub second_source_id: String,
    pub witness: Option<IntersectionWitnessTraceV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SplitEventTraceV1 {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_index: Option<usize>,
    pub source_segment: usize,
    pub source_id: String,
    pub start: CoordinateFingerprintV1,
    pub end: CoordinateFingerprintV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UniformGridCellTraceV1 {
    pub index: usize,
    pub iteration_index: usize,
    pub row: usize,
    pub column: usize,
    pub min: CoordinateFingerprintV1,
    pub max: CoordinateFingerprintV1,
    pub segment_indices: Vec<usize>,
    pub source_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UniformGridGlobalLineTraceV1 {
    pub index: usize,
    pub iteration_index: usize,
    pub segment_index: usize,
    pub source_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UniformGridCandidateTraceV1 {
    pub index: usize,
    pub iteration_index: usize,
    pub scan: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    pub first_segment: usize,
    pub second_segment: usize,
    pub first_source_id: String,
    pub second_source_id: String,
    pub witness: Option<IntersectionWitnessTraceV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned_by_cell: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphNodeTraceV1 {
    pub node_id: usize,
    pub coordinate: CoordinateFingerprintV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphEdgeTraceV1 {
    pub edge_id: usize,
    pub start: CoordinateFingerprintV1,
    pub end: CoordinateFingerprintV1,
    pub source_ids: Vec<String>,
    pub directed_edge_ids: [usize; 2],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DirectedHalfedgeTraceV1 {
    pub directed_edge_id: usize,
    pub source_node_id: usize,
    pub destination_node_id: usize,
    pub edge_id: usize,
    pub symmetric_edge_id: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClassifiedLineTraceV1 {
    pub index: usize,
    pub coordinates: Vec<CoordinateFingerprintV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RingTraceV1 {
    pub index: usize,
    pub coordinates: Vec<CoordinateFingerprintV1>,
    pub edge_ids: Vec<String>,
    pub source_ids: Vec<String>,
    pub invalid_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContainmentCandidateTraceV1 {
    pub hole_index: usize,
    pub candidate_shell_indices: Vec<usize>,
    pub selected_shell_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CanonicalOrderTraceV1 {
    pub family: String,
    pub owner_index: Option<usize>,
    pub ordered_original_indices: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RingRotationTraceV1 {
    pub family: String,
    pub owner_index: usize,
    pub ring_index: Option<usize>,
    pub original_start_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TileOwnershipTraceV1 {
    pub tile_index: usize,
    pub polygon_index: usize,
    pub ownership_point: Option<CoordinateFingerprintV1>,
    pub owned: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TileDedupTraceV1 {
    pub polygon_index: usize,
    pub retained: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopologyTraceV1 {
    pub schema_version: u32,
    pub library_version: String,
    pub level: TraceLevelV1,
    pub byte_limit: usize,
    pub bytes_used: usize,
    pub truncated: bool,
    pub options: serde_json::Value,
    pub events: Vec<TraceEventV1>,
}

/// Collects serialized trace events up to an exact byte budget.
///
/// Callers hold this as an `Option`; `None` is the disabled fast path.
pub struct TraceRecorderV1 {
    trace: TopologyTraceV1,
}

pub(crate) struct TraceCaptureBudget {
    remaining_bytes: usize,
    truncated: bool,
}

pub(crate) struct TraceCapture<'a, T> {
    values: &'a mut Vec<T>,
    budget: &'a mut TraceCaptureBudget,
}

pub struct TracedPolygonizerResultV1 {
    pub result: PolygonizerResult,
    pub trace: TopologyTraceV1,
}

impl TraceCaptureBudget {
    pub(crate) fn new(byte_limit: usize) -> Self {
        Self {
            remaining_bytes: byte_limit,
            truncated: false,
        }
    }

    pub(crate) fn capture<T>(&mut self, values: &mut Vec<T>, value: T) -> bool {
        if !self.take(std::mem::size_of::<T>().max(1)) {
            return false;
        }
        values.push(value);
        true
    }

    pub(crate) fn take(&mut self, bytes: usize) -> bool {
        let Some(remaining_bytes) = self.remaining_bytes.checked_sub(bytes) else {
            self.truncated = true;
            return false;
        };
        self.remaining_bytes = remaining_bytes;
        true
    }

    pub(crate) fn truncated(&self) -> bool {
        self.truncated
    }
}

impl<'a, T> TraceCapture<'a, T> {
    pub(crate) fn new(values: &'a mut Vec<T>, budget: &'a mut TraceCaptureBudget) -> Self {
        Self { values, budget }
    }

    pub(crate) fn push(&mut self, value: T) -> bool {
        self.budget.capture(self.values, value)
    }
}

impl TraceRecorderV1 {
    pub fn new(
        level: Option<TraceLevelV1>,
        byte_limit: usize,
        options: &PolygonizerOptions,
    ) -> Option<Self> {
        level.map(|level| Self {
            trace: TopologyTraceV1 {
                schema_version: TOPOLOGY_TRACE_V1_SCHEMA_VERSION,
                library_version: env!("CARGO_PKG_VERSION").to_string(),
                level,
                byte_limit,
                bytes_used: 0,
                truncated: false,
                options: serde_json::to_value(options).expect("validated options serialize"),
                events: Vec::new(),
            },
        })
    }

    /// Records an allowed event, returning false when the byte budget is exhausted.
    pub fn record(
        &mut self,
        stage: TraceStageV1,
        kind: impl Into<String>,
        payload: serde_json::Value,
    ) -> bool {
        if self.trace.truncated || !self.trace.level.allows(stage) {
            return !self.trace.truncated;
        }
        let event = TraceEventV1 {
            sequence: self.trace.events.len(),
            stage,
            kind: kind.into(),
            payload,
        };
        let event_bytes = serde_json::to_vec(&event)
            .expect("trace event serializes")
            .len();
        let Some(bytes_used) = self.trace.bytes_used.checked_add(event_bytes) else {
            self.trace.truncated = true;
            return false;
        };
        if bytes_used > self.trace.byte_limit {
            self.trace.truncated = true;
            return false;
        }
        self.trace.bytes_used = bytes_used;
        self.trace.events.push(event);
        true
    }

    pub fn finish(self) -> TopologyTraceV1 {
        self.trace
    }

    pub(crate) fn records_stage(&self, stage: TraceStageV1) -> bool {
        self.trace.level.allows(stage) && !self.trace.truncated
    }

    pub(crate) fn capture_byte_limit(&self) -> usize {
        self.trace.byte_limit.saturating_sub(self.trace.bytes_used)
    }

    pub(crate) fn mark_capture_truncated(&mut self) {
        self.trace.truncated = true;
    }

    pub(crate) fn record_input_segments(&mut self, lines: &[Line3D]) -> crate::Result<()> {
        self.record_noding_segments("normalized_input_segment", lines)
    }

    pub(crate) fn record_noding_segments(
        &mut self,
        kind: &'static str,
        lines: &[Line3D],
    ) -> crate::Result<()> {
        if !self.trace.level.allows(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, line) in lines.iter().enumerate() {
            let payload = serde_json::to_value(InputSegmentTraceV1 {
                index,
                start: coordinate_fingerprint(line.start)?,
                end: coordinate_fingerprint(line.end)?,
                source_ids: vec![format!("0x{:08x}", line.line_id)],
            })
            .expect("input trace event serializes");
            if !self.record(TraceStageV1::Noding, kind, payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_hot_pixels(
        &mut self,
        hot_pixels: &[crate::types::IPoint],
        grid_size: f64,
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, point) in hot_pixels.iter().enumerate() {
            let payload = serde_json::to_value(HotPixelTraceV1 {
                index,
                grid_x: point.x,
                grid_y: point.y,
                coordinate: coordinate_fingerprint(crate::Coord3D::new(
                    point.x as f64 * grid_size,
                    point.y as f64 * grid_size,
                    0.0,
                ))?,
            })
            .expect("hot-pixel trace event serializes");
            if !self.record(TraceStageV1::Noding, "certified_hot_pixel", payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_certified_candidates(
        &mut self,
        candidates: &[HotPixelCandidateTrace],
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, candidate) in candidates.iter().enumerate() {
            let witness = match candidate.witness {
                Some(HotPixelIntersectionTrace::Point(coordinate)) => {
                    Some(IntersectionWitnessTraceV1::Point {
                        coordinate: coordinate_fingerprint(coordinate)?,
                    })
                }
                Some(HotPixelIntersectionTrace::Collinear(start, end)) => {
                    Some(IntersectionWitnessTraceV1::Collinear {
                        start: coordinate_fingerprint(start)?,
                        end: coordinate_fingerprint(end)?,
                    })
                }
                None => None,
            };
            let payload = serde_json::to_value(CandidatePairTraceV1 {
                index,
                iteration_index: None,
                first_segment: candidate.first_segment,
                second_segment: candidate.second_segment,
                first_source_id: format!("0x{:08x}", candidate.first_source_id),
                second_source_id: format!("0x{:08x}", candidate.second_source_id),
                witness,
            })
            .expect("candidate-pair trace event serializes");
            if !self.record(TraceStageV1::Noding, "certified_candidate_pair", payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_floating_candidates(
        &mut self,
        candidates: &[FloatingCandidateTrace],
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, candidate) in candidates.iter().enumerate() {
            let witness = match candidate.witness {
                Some(FloatingIntersectionTrace::Point(coordinate)) => {
                    Some(IntersectionWitnessTraceV1::Point {
                        coordinate: coordinate_fingerprint(coordinate)?,
                    })
                }
                Some(FloatingIntersectionTrace::Collinear(start, end)) => {
                    Some(IntersectionWitnessTraceV1::Collinear {
                        start: coordinate_fingerprint(start)?,
                        end: coordinate_fingerprint(end)?,
                    })
                }
                None => None,
            };
            let payload = serde_json::to_value(CandidatePairTraceV1 {
                index,
                iteration_index: Some(candidate.iteration_index),
                first_segment: candidate.first_segment,
                second_segment: candidate.second_segment,
                first_source_id: format!("0x{:08x}", candidate.first_source_id),
                second_source_id: format!("0x{:08x}", candidate.second_source_id),
                witness,
            })
            .expect("candidate-pair trace event serializes");
            if !self.record(TraceStageV1::Noding, "floating_candidate_pair", payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_uniform_grid(
        &mut self,
        cells: &[UniformGridCellTrace],
        global_lines: &[UniformGridGlobalLineTrace],
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, cell) in cells.iter().enumerate() {
            let payload = serde_json::to_value(UniformGridCellTraceV1 {
                index,
                iteration_index: cell.iteration_index,
                row: cell.row,
                column: cell.column,
                min: coordinate_fingerprint(cell.min)?,
                max: coordinate_fingerprint(cell.max)?,
                segment_indices: cell.segment_indices.clone(),
                source_ids: cell
                    .source_ids
                    .iter()
                    .map(|source_id| format!("0x{source_id:08x}"))
                    .collect(),
            })
            .expect("uniform-grid cell trace event serializes");
            if !self.record(TraceStageV1::Noding, "uniform_grid_cell", payload) {
                return Ok(());
            }
        }
        for (index, line) in global_lines.iter().enumerate() {
            let payload = serde_json::to_value(UniformGridGlobalLineTraceV1 {
                index,
                iteration_index: line.iteration_index,
                segment_index: line.segment_index,
                source_id: format!("0x{:08x}", line.source_id),
            })
            .expect("uniform-grid global-line trace event serializes");
            if !self.record(TraceStageV1::Noding, "uniform_grid_global_line", payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_uniform_grid_candidates(
        &mut self,
        candidates: &[UniformGridCandidateTrace],
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, candidate) in candidates.iter().enumerate() {
            let witness = match candidate.witness {
                Some(FloatingIntersectionTrace::Point(coordinate)) => {
                    Some(IntersectionWitnessTraceV1::Point {
                        coordinate: coordinate_fingerprint(coordinate)?,
                    })
                }
                Some(FloatingIntersectionTrace::Collinear(start, end)) => {
                    Some(IntersectionWitnessTraceV1::Collinear {
                        start: coordinate_fingerprint(start)?,
                        end: coordinate_fingerprint(end)?,
                    })
                }
                None => None,
            };
            let payload = serde_json::to_value(UniformGridCandidateTraceV1 {
                index,
                iteration_index: candidate.iteration_index,
                scan: candidate.scan.to_string(),
                row: candidate.row,
                column: candidate.column,
                first_segment: candidate.first_segment,
                second_segment: candidate.second_segment,
                first_source_id: format!("0x{:08x}", candidate.first_source_id),
                second_source_id: format!("0x{:08x}", candidate.second_source_id),
                witness,
                owned_by_cell: candidate.owned_by_cell,
            })
            .expect("uniform-grid candidate trace event serializes");
            if !self.record(TraceStageV1::Noding, "uniform_grid_candidate_pair", payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_certified_splits(
        &mut self,
        splits: &[HotPixelSplitTrace],
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, split) in splits.iter().enumerate() {
            let payload = serde_json::to_value(SplitEventTraceV1 {
                index,
                iteration_index: None,
                source_segment: split.source_segment,
                source_id: format!("0x{:08x}", split.source_id),
                start: coordinate_fingerprint(split.start)?,
                end: coordinate_fingerprint(split.end)?,
            })
            .expect("split-event trace serializes");
            if !self.record(TraceStageV1::Noding, "certified_split_segment", payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_floating_splits(
        &mut self,
        splits: &[FloatingSplitTrace],
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Noding) {
            return Ok(());
        }
        for (index, split) in splits.iter().enumerate() {
            let payload = serde_json::to_value(SplitEventTraceV1 {
                index,
                iteration_index: Some(split.iteration_index),
                source_segment: split.source_segment,
                source_id: format!("0x{:08x}", split.source_id),
                start: coordinate_fingerprint(split.start)?,
                end: coordinate_fingerprint(split.end)?,
            })
            .expect("split-event trace serializes");
            if !self.record(TraceStageV1::Noding, "floating_split_segment", payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_graph(&mut self, graph: &PlanarGraph) -> crate::Result<()> {
        if !self.trace.level.allows(TraceStageV1::Graph) {
            return Ok(());
        }
        for node_id in 0..graph.nodes_x.len() {
            let payload = serde_json::to_value(GraphNodeTraceV1 {
                node_id,
                coordinate: coordinate_fingerprint(crate::Coord3D::new(
                    graph.nodes_x[node_id],
                    graph.nodes_y[node_id],
                    graph.nodes_z[node_id],
                ))?,
            })
            .expect("graph node trace event serializes");
            if !self.record(TraceStageV1::Graph, "graph_node", payload) {
                return Ok(());
            }
        }
        for (edge_id, edge) in graph.edges.iter().enumerate() {
            let payload = serde_json::to_value(GraphEdgeTraceV1 {
                edge_id,
                start: coordinate_fingerprint(edge.line.start)?,
                end: coordinate_fingerprint(edge.line.end)?,
                source_ids: edge
                    .sources
                    .line_ids
                    .iter()
                    .map(|source_id| format!("0x{source_id:08x}"))
                    .collect(),
                directed_edge_ids: edge.dir_edges,
            })
            .expect("graph edge trace event serializes");
            if !self.record(TraceStageV1::Graph, "dissolved_edge", payload) {
                return Ok(());
            }
        }
        for (directed_edge_id, edge) in graph.directed_edges.iter().enumerate() {
            let payload = serde_json::to_value(DirectedHalfedgeTraceV1 {
                directed_edge_id,
                source_node_id: edge.src,
                destination_node_id: edge.dst,
                edge_id: edge.edge_idx,
                symmetric_edge_id: edge.sym_idx,
            })
            .expect("directed halfedge trace event serializes");
            if !self.record(TraceStageV1::Graph, "directed_halfedge", payload) {
                return Ok(());
            }
        }
        Ok(())
    }

    pub(crate) fn record_classified_lines(
        &mut self,
        kind: &'static str,
        lines: &[Vec<crate::Coord3D>],
    ) -> crate::Result<()> {
        if !self.trace.level.allows(TraceStageV1::Graph) {
            return Ok(());
        }
        for (index, line) in lines.iter().enumerate() {
            let payload = serde_json::to_value(ClassifiedLineTraceV1 {
                index,
                coordinates: line
                    .iter()
                    .copied()
                    .map(coordinate_fingerprint)
                    .collect::<crate::Result<_>>()?,
            })
            .expect("classified line trace event serializes");
            if !self.record(TraceStageV1::Graph, kind, payload) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_extracted_rings(
        &mut self,
        kind: &'static str,
        rings: &[ExtractedRing],
    ) -> crate::Result<()> {
        if !self.records_stage(TraceStageV1::Rings) {
            return Ok(());
        }
        for (index, ring) in rings.iter().enumerate() {
            if !self.record_ring(
                kind,
                RingTraceV1 {
                    index,
                    coordinates: exact_coordinates(&ring.coords)?,
                    edge_ids: ring
                        .line_ids
                        .iter()
                        .map(|id| format!("0x{id:08x}"))
                        .collect(),
                    source_ids: ring
                        .source_line_ids
                        .iter()
                        .map(|id| format!("0x{id:08x}"))
                        .collect(),
                    invalid_reason: None,
                },
            ) {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn record_invalid_ring(
        &mut self,
        index: usize,
        ring: &Polygon3D,
        reason: &'static str,
    ) -> crate::Result<()> {
        let payload = RingTraceV1 {
            index,
            coordinates: exact_coordinates(&ring.exterior)?,
            edge_ids: ring
                .exterior_ids
                .iter()
                .map(|id| format!("0x{id:08x}"))
                .collect(),
            source_ids: ring
                .boundary_source_line_ids
                .iter()
                .map(|id| format!("0x{id:08x}"))
                .collect(),
            invalid_reason: Some(reason.to_string()),
        };
        self.record_ring("invalid_ring", payload);
        Ok(())
    }

    pub(crate) fn record_classified_ring(
        &mut self,
        kind: &'static str,
        index: usize,
        ring: &Polygon3D,
    ) -> crate::Result<()> {
        let payload = RingTraceV1 {
            index,
            coordinates: exact_coordinates(&ring.exterior)?,
            edge_ids: ring
                .exterior_ids
                .iter()
                .map(|id| format!("0x{id:08x}"))
                .collect(),
            source_ids: ring
                .boundary_source_line_ids
                .iter()
                .map(|id| format!("0x{id:08x}"))
                .collect(),
            invalid_reason: None,
        };
        self.record_ring(kind, payload);
        Ok(())
    }

    pub(crate) fn record_containment_candidates(
        &mut self,
        hole_index: usize,
        candidate_shell_indices: Vec<usize>,
        selected_shell_index: Option<usize>,
    ) {
        self.record(
            TraceStageV1::Rings,
            "containment_candidates",
            serde_json::to_value(ContainmentCandidateTraceV1 {
                hole_index,
                candidate_shell_indices,
                selected_shell_index,
            })
            .expect("containment trace event serializes"),
        );
    }

    pub(crate) fn record_canonical_order(
        &mut self,
        family: &'static str,
        owner_index: Option<usize>,
        ordered_original_indices: Vec<usize>,
    ) {
        self.record(
            TraceStageV1::Output,
            "canonical_order",
            serde_json::to_value(CanonicalOrderTraceV1 {
                family: family.to_string(),
                owner_index,
                ordered_original_indices,
            })
            .expect("canonical order trace event serializes"),
        );
    }

    pub(crate) fn record_ring_rotation(
        &mut self,
        family: &'static str,
        owner_index: usize,
        ring_index: Option<usize>,
        original_start_index: usize,
    ) {
        self.record(
            TraceStageV1::Output,
            "canonical_ring_rotation",
            serde_json::to_value(RingRotationTraceV1 {
                family: family.to_string(),
                owner_index,
                ring_index,
                original_start_index,
            })
            .expect("ring rotation trace event serializes"),
        );
    }

    pub(crate) fn record_tile_ownership(
        &mut self,
        tile_index: usize,
        polygon_index: usize,
        ownership_point: Option<crate::Coord3D>,
        owned: bool,
    ) -> crate::Result<()> {
        self.record(
            TraceStageV1::Output,
            "tile_ownership",
            serde_json::to_value(TileOwnershipTraceV1 {
                tile_index,
                polygon_index,
                ownership_point: ownership_point.map(coordinate_fingerprint).transpose()?,
                owned,
            })
            .expect("tile ownership trace event serializes"),
        );
        Ok(())
    }

    pub(crate) fn record_tile_dedup(&mut self, polygon_index: usize, retained: bool) {
        self.record(
            TraceStageV1::Output,
            "tile_deduplication",
            serde_json::to_value(TileDedupTraceV1 {
                polygon_index,
                retained,
            })
            .expect("tile deduplication trace event serializes"),
        );
    }

    fn record_ring(&mut self, kind: &'static str, ring: RingTraceV1) -> bool {
        self.record(
            TraceStageV1::Rings,
            kind,
            serde_json::to_value(ring).expect("ring trace event serializes"),
        )
    }
}

fn exact_coordinates(
    coordinates: &[crate::Coord3D],
) -> crate::Result<Vec<CoordinateFingerprintV1>> {
    coordinates
        .iter()
        .copied()
        .map(coordinate_fingerprint)
        .collect()
}

impl TraceLevelV1 {
    fn allows(self, stage: TraceStageV1) -> bool {
        stage == TraceStageV1::Summary
            || matches!(
                (self, stage),
                (Self::Noding, TraceStageV1::Noding)
                    | (Self::Graph, TraceStageV1::Graph)
                    | (Self::Rings, TraceStageV1::Rings)
                    | (Self::Full, _)
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        polygonize, polygonize_with_trace, Coord3D, ExecutionPolicy, TopologyFingerprintV1,
    };
    use serde_json::json;

    #[test]
    fn disabled_trace_allocates_no_recorder_and_enabled_trace_is_bounded() {
        let options = PolygonizerOptions::default();
        assert!(TraceRecorderV1::new(None, 1024, &options).is_none());

        let mut recorder = TraceRecorderV1::new(Some(TraceLevelV1::Noding), 120, &options).unwrap();
        assert!(recorder.record(
            TraceStageV1::Graph,
            "ignored",
            json!({"large": "x".repeat(500)})
        ));
        assert!(recorder.record(TraceStageV1::Noding, "candidate", json!({"pair": [1, 2]})));
        assert!(!recorder.record(
            TraceStageV1::Noding,
            "oversized",
            json!({"large": "x".repeat(500)})
        ));

        let trace = recorder.finish();
        assert_eq!(trace.schema_version, TOPOLOGY_TRACE_V1_SCHEMA_VERSION);
        assert_eq!(trace.events.len(), 1);
        assert_eq!(trace.events[0].sequence, 0);
        assert!(trace.bytes_used <= trace.byte_limit);
        assert!(trace.truncated);
        assert!(trace.options.is_object());
    }

    #[test]
    fn capture_budget_stops_before_trace_only_vector_growth() {
        let item_bytes = std::mem::size_of::<FloatingSplitTrace>();
        let mut budget = TraceCaptureBudget::new(item_bytes * 2);
        let mut values = Vec::new();
        let split = FloatingSplitTrace {
            iteration_index: 0,
            source_segment: 0,
            source_id: 1,
            start: Coord3D::new(0.0, 0.0, 0.0),
            end: Coord3D::new(1.0, 0.0, 0.0),
        };

        assert!(budget.capture(&mut values, split));
        assert!(budget.capture(&mut values, split));
        assert!(!budget.capture(&mut values, split));
        assert_eq!(values.len(), 2);
        assert!(budget.truncated());
    }

    #[test]
    fn traced_entrypoint_records_exact_input_without_changing_results() {
        let lines = vec![
            Line3D::new(
                Coord3D::new(-0.0, 0.0, 10.0),
                Coord3D::new(1.0, 0.0, 20.0),
                7,
            ),
            Line3D::new(
                Coord3D::new(1.0, 0.0, 30.0),
                Coord3D::new(0.0, 1.0, 40.0),
                9,
            ),
        ];
        let options = PolygonizerOptions::default();
        let expected = polygonize(lines.iter().copied(), &options).unwrap();
        let traced = polygonize_with_trace(
            lines,
            &options,
            &ExecutionPolicy::default(),
            TraceLevelV1::Noding,
            usize::MAX,
        )
        .unwrap();

        assert_eq!(traced.trace.events.len(), 2);
        assert_eq!(
            traced.trace.events[0].payload["start"]["x"],
            "0x0000000000000000"
        );
        assert_eq!(
            traced.trace.events[0].payload["source_ids"],
            json!(["0x00000007"])
        );
        assert_eq!(
            TopologyFingerprintV1::try_from_result(&traced.result, &options).unwrap(),
            TopologyFingerprintV1::try_from_result(&expected, &options).unwrap()
        );
    }

    #[test]
    fn graph_trace_retains_dissolved_sources_and_halfedge_links() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 7),
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 9),
        ];
        let traced = polygonize_with_trace(
            lines,
            &PolygonizerOptions::default(),
            &ExecutionPolicy::default(),
            TraceLevelV1::Graph,
            usize::MAX,
        )
        .unwrap();

        assert_eq!(
            traced
                .trace
                .events
                .iter()
                .filter(|event| event.kind == "graph_node")
                .count(),
            2
        );
        let edge = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "dissolved_edge")
            .unwrap();
        assert_eq!(
            edge.payload["source_ids"],
            json!(["0x00000007", "0x00000009"])
        );
        let halfedges: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "directed_halfedge")
            .collect();
        assert_eq!(halfedges.len(), 2);
        assert_eq!(halfedges[0].payload["symmetric_edge_id"], 1);
        assert_eq!(halfedges[1].payload["symmetric_edge_id"], 0);
    }

    #[test]
    fn graph_trace_records_dangle_and_cut_edge_classification() {
        let mut lines = Vec::new();
        let mut add_ring = |x: f64, first_id: u32| {
            let points = [(x, 0.0), (x + 1.0, 0.0), (x + 1.0, 1.0), (x, 1.0)];
            for index in 0..4 {
                let start = points[index];
                let end = points[(index + 1) % points.len()];
                lines.push(Line3D::new(
                    Coord3D::new(start.0, start.1, 0.0),
                    Coord3D::new(end.0, end.1, 0.0),
                    first_id + index as u32,
                ));
            }
        };
        add_ring(0.0, 1);
        add_ring(2.0, 5);
        lines.push(Line3D::new(
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(2.0, 0.0, 0.0),
            9,
        ));
        lines.push(Line3D::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(-1.0, 0.0, 0.0),
            10,
        ));

        let traced = polygonize_with_trace(
            lines,
            &PolygonizerOptions::default(),
            &ExecutionPolicy::default(),
            TraceLevelV1::Graph,
            usize::MAX,
        )
        .unwrap();
        let dangles = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "dangle")
            .count();
        let cut_edges = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "cut_edge")
            .count();

        assert_eq!(dangles, traced.result.dangles.len());
        assert_eq!(cut_edges, traced.result.cut_edges.len());
        assert_eq!((dangles, cut_edges), (1, 1));
    }

    #[test]
    fn noding_trace_records_the_physical_fixed_grid_output() {
        let lines = vec![Line3D::new(
            Coord3D::new(0.14, 0.26, 3.0),
            Coord3D::new(1.04, 0.26, 4.0),
            7,
        )];
        let options = PolygonizerOptions {
            precision_model: crate::PrecisionModel::FixedGrid { grid_size: 0.1 },
            ..Default::default()
        };
        let traced = polygonize_with_trace(
            lines,
            &options,
            &ExecutionPolicy::default(),
            TraceLevelV1::Noding,
            usize::MAX,
        )
        .unwrap();
        let snapped = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "fixed_grid_segment")
            .unwrap();

        assert_eq!(
            snapped.payload["start"]["x"],
            format!("0x{:016x}", 0.1f64.to_bits())
        );
        assert_eq!(
            snapped.payload["start"]["y"],
            format!("0x{:016x}", (3.0f64 * 0.1).to_bits())
        );
        assert_eq!(snapped.payload["source_ids"], json!(["0x00000007"]));
    }

    #[test]
    fn noding_trace_records_certified_hot_pixel_grid_cells() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(2.0, 2.0, 0.0), 1),
            Line3D::new(Coord3D::new(0.0, 2.0, 0.0), Coord3D::new(2.0, 0.0, 0.0), 2),
        ];
        let mut options = PolygonizerOptions {
            node_input: true,
            precision_model: crate::PrecisionModel::FixedGrid { grid_size: 1.0 },
            ..Default::default()
        };
        options.noding.guarantee = crate::NodingGuarantee::CertifiedFixedPrecision;
        let traced = polygonize_with_trace(
            lines,
            &options,
            &ExecutionPolicy::default(),
            TraceLevelV1::Noding,
            usize::MAX,
        )
        .unwrap();
        let hot_pixels: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "certified_hot_pixel")
            .collect();

        assert_eq!(hot_pixels.len(), 5);
        let intersection = hot_pixels
            .iter()
            .find(|event| event.payload["grid_x"] == 1 && event.payload["grid_y"] == 1)
            .unwrap();
        assert_eq!(
            intersection.payload["coordinate"]["x"],
            format!("0x{:016x}", 1.0f64.to_bits())
        );
        let candidates: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "certified_candidate_pair")
            .collect();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload["first_source_id"], "0x00000001");
        assert_eq!(candidates[0].payload["second_source_id"], "0x00000002");
        assert_eq!(candidates[0].payload["witness"]["kind"], "point");
        assert_eq!(
            candidates[0].payload["witness"]["coordinate"]["x"],
            format!("0x{:016x}", 1.0f64.to_bits())
        );
        let splits: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "certified_split_segment")
            .collect();
        assert_eq!(splits.len(), 4);
        assert_eq!(
            splits
                .iter()
                .filter(|event| event.payload["source_id"] == "0x00000001")
                .count(),
            2
        );
        assert!(splits.iter().any(|event| {
            event.payload["end"]["x"] == format!("0x{:016x}", 1.0f64.to_bits())
                && event.payload["end"]["y"] == format!("0x{:016x}", 1.0f64.to_bits())
        }));
    }

    #[test]
    fn noding_trace_records_floating_simd_candidates() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(2.0, 2.0, 0.0), 1),
            Line3D::new(Coord3D::new(0.0, 2.0, 0.0), Coord3D::new(2.0, 0.0, 0.0), 2),
        ];
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let traced = polygonize_with_trace(
            lines,
            &options,
            &ExecutionPolicy::default(),
            TraceLevelV1::Noding,
            usize::MAX,
        )
        .unwrap();
        let candidates: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "floating_candidate_pair")
            .collect();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload["iteration_index"], 0);
        assert_eq!(candidates[0].payload["first_source_id"], "0x00000001");
        assert_eq!(candidates[0].payload["second_source_id"], "0x00000002");
        assert_eq!(candidates[0].payload["witness"]["kind"], "point");
        assert_eq!(
            candidates[0].payload["witness"]["coordinate"]["x"],
            format!("0x{:016x}", 1.0f64.to_bits())
        );
        let splits: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "floating_split_segment")
            .collect();
        assert_eq!(splits.len(), 4);
        assert!(splits
            .iter()
            .all(|event| event.payload["iteration_index"] == 0));
        assert_eq!(
            splits
                .iter()
                .filter(|event| event.payload["source_id"] == "0x00000001")
                .count(),
            2
        );
    }

    #[test]
    fn noding_trace_records_uniform_grid_cells() {
        let lines: Vec<_> = (0..256)
            .map(|index| {
                let y = index as f64;
                Line3D::new(
                    Coord3D::new(0.0, y, 0.0),
                    Coord3D::new(10.0, y + 10.0, 0.0),
                    index as u32,
                )
            })
            .collect();
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let traced = polygonize_with_trace(
            lines,
            &options,
            &ExecutionPolicy::default(),
            TraceLevelV1::Noding,
            usize::MAX,
        )
        .unwrap();
        let cells: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "uniform_grid_cell")
            .collect();

        assert!(!cells.is_empty());
        assert_eq!(cells[0].payload["iteration_index"], 0);
        assert!(
            cells[0].payload["segment_indices"]
                .as_array()
                .unwrap()
                .len()
                >= 2
        );
        assert_eq!(
            cells[0].payload["segment_indices"]
                .as_array()
                .unwrap()
                .len(),
            cells[0].payload["source_ids"].as_array().unwrap().len()
        );
        let candidates: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "uniform_grid_candidate_pair")
            .collect();
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].payload["iteration_index"], 0);
        assert_eq!(candidates[0].payload["scan"], "cell");
        assert!(candidates[0].payload["witness"].is_null());
        assert_eq!(candidates[0].payload["owned_by_cell"], false);
    }
}
