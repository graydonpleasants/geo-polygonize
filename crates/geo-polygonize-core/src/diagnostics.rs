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
pub struct ContainmentStats {
    pub prepared_shells: usize,
    pub envelope_candidates: usize,
    pub area_rejections: usize,
    pub point_in_ring_calls: usize,
    pub max_point_in_ring_calls_per_shell: usize,
    pub shells_with_64_plus_point_in_ring_calls: usize,
    pub shared_edge_checks: usize,
    pub shared_vertex_checks: usize,
}

impl ContainmentStats {
    pub(crate) fn merge(&mut self, other: Self) {
        self.prepared_shells += other.prepared_shells;
        self.envelope_candidates += other.envelope_candidates;
        self.area_rejections += other.area_rejections;
        self.point_in_ring_calls += other.point_in_ring_calls;
        self.max_point_in_ring_calls_per_shell = self
            .max_point_in_ring_calls_per_shell
            .max(other.max_point_in_ring_calls_per_shell);
        self.shells_with_64_plus_point_in_ring_calls +=
            other.shells_with_64_plus_point_in_ring_calls;
        self.shared_edge_checks += other.shared_edge_checks;
        self.shared_vertex_checks += other.shared_vertex_checks;
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodingWorkStats {
    pub grid_cells: usize,
    pub grid_cell_entries: usize,
    pub global_lines: usize,
    pub candidate_pairs: usize,
    pub aabb_rejections: usize,
    pub exact_intersection_calls: usize,
    pub split_events: usize,
}

impl NodingWorkStats {
    pub(crate) fn merge(&mut self, other: Self) {
        self.grid_cells += other.grid_cells;
        self.grid_cell_entries += other.grid_cell_entries;
        self.global_lines += other.global_lines;
        self.candidate_pairs += other.candidate_pairs;
        self.aabb_rejections += other.aabb_rejections;
        self.exact_intersection_calls += other.exact_intersection_calls;
        self.split_events += other.split_events;
    }
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
    pub unassigned_hole_count: usize,
    pub unassigned_hole_area: f64,
    pub invalid_ring_count: usize,
    pub flat_line_count: usize,
    pub phase_times: PolygonizerPhaseTimes,
    pub noding_iterations: Vec<NodingIterationStats>,
    pub snap_stats: SnapStats,
    pub intersection_stats: IntersectionStats,
    pub containment_stats: ContainmentStats,
    pub noding_work_stats: NodingWorkStats,
}
