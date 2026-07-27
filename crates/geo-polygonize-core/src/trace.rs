//! Internal bounded topology trace schema.

use crate::fingerprint::coordinate_fingerprint;
use crate::graph::PlanarGraph;
use crate::{CoordinateFingerprintV1, Line3D, PolygonizerOptions, PolygonizerResult};
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

pub struct TracedPolygonizerResultV1 {
    pub result: PolygonizerResult,
    pub trace: TopologyTraceV1,
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
}
