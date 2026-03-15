pub use crate::options::DiagnosticsOptions;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PolygonizerPhaseTimes {
    pub ingest_and_node: Duration,
    pub graph_build: Duration,
    pub ring_extraction: Duration,
    pub containment: Duration,
    pub output_flatten: Duration,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodingIterationStats {
    pub iteration_index: usize,
    pub intersections_found: usize,
    pub nodes_added: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SnapStats {
    pub total_snapped_vertices: usize,
    pub snap_strategy: String, // E.g., "Grid", "GeosCompat"
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IntersectionStats {
    pub exact_intersections: usize,
    pub interpolated_intersections: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PolygonizerDiagnostics {
    pub input_segment_count: usize,
    pub noded_segment_count: usize,
    pub dangle_count: usize,
    pub cut_edge_count: usize,
    pub ring_count: usize,
    pub shell_count: usize,
    pub hole_count: usize,
    pub invalid_ring_count: usize,
    pub flat_line_count: usize,
    pub phase_times: PolygonizerPhaseTimes,
    pub noding_iterations: Vec<NodingIterationStats>,
    pub snap_stats: SnapStats,
    pub intersection_stats: IntersectionStats,
}
