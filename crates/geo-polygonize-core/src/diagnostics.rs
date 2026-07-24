use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const POLYGONIZER_DIAGNOSTICS_V1_SCHEMA_VERSION: u32 = 1;

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
pub struct ZConflictStats {
    pub conflict_node_count: usize,
    /// Sorted input line IDs contributing to conflicting nodes; populated with provenance.
    pub contributing_line_ids: Vec<u32>,
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
    pub shared_edge_pair_checks: usize,
    pub graph_edge_key_checks: usize,
    pub shared_vertex_checks: usize,
    pub shared_vertex_pair_checks: usize,
    pub graph_vertex_id_checks: usize,
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
        self.shared_edge_pair_checks += other.shared_edge_pair_checks;
        self.graph_edge_key_checks += other.graph_edge_key_checks;
        self.shared_vertex_checks += other.shared_vertex_checks;
        self.shared_vertex_pair_checks += other.shared_vertex_pair_checks;
        self.graph_vertex_id_checks += other.graph_vertex_id_checks;
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
    pub pre_snap_vertex_candidates: usize,
}

pub(crate) struct ExecutionWorkTracker<'a> {
    policy: Option<&'a crate::ExecutionPolicy>,
    stats: Option<&'a mut NodingWorkStats>,
    candidate_pairs: usize,
    exact_intersection_calls: usize,
    #[cfg(test)]
    cancel_at_candidate: Option<(crate::CancellationToken, usize)>,
}

impl<'a> ExecutionWorkTracker<'a> {
    pub(crate) fn new(
        policy: Option<&'a crate::ExecutionPolicy>,
        stats: Option<&'a mut NodingWorkStats>,
    ) -> Self {
        let candidate_pairs = stats.as_deref().map_or(0, |stats| stats.candidate_pairs);
        let exact_intersection_calls = stats
            .as_deref()
            .map_or(0, |stats| stats.exact_intersection_calls);
        Self {
            policy,
            stats,
            candidate_pairs,
            exact_intersection_calls,
            #[cfg(test)]
            cancel_at_candidate: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel_at_candidate(
        mut self,
        token: crate::CancellationToken,
        candidate: usize,
    ) -> Self {
        self.cancel_at_candidate = Some((token, candidate));
        self
    }

    pub(crate) fn grid(&mut self, cells: usize, entries: usize, globals: usize) {
        if let Some(stats) = self.stats.as_deref_mut() {
            stats.grid_cells = stats.grid_cells.saturating_add(cells);
            stats.grid_cell_entries = stats.grid_cell_entries.saturating_add(entries);
            stats.global_lines = stats.global_lines.saturating_add(globals);
        }
    }

    pub(crate) fn check_cancelled(&self) -> crate::Result<()> {
        if let Some(policy) = self.policy {
            policy.check_cancelled("candidate_enumeration")?;
        }
        Ok(())
    }

    pub(crate) fn candidate(&mut self, overlaps: bool) -> crate::Result<()> {
        self.candidate_pairs = self.candidate_pairs.checked_add(1).ok_or_else(|| {
            crate::PolygonizeError::InternalInvariantViolation {
                reason: "candidate-pair counter overflow".to_string(),
            }
        })?;
        if overlaps {
            self.exact_intersection_calls = self
                .exact_intersection_calls
                .checked_add(1)
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "exact-intersection counter overflow".to_string(),
                })?;
        }
        if let Some(stats) = self.stats.as_deref_mut() {
            stats.candidate_pairs = stats.candidate_pairs.saturating_add(1);
            if overlaps {
                stats.exact_intersection_calls = stats.exact_intersection_calls.saturating_add(1);
            } else {
                stats.aabb_rejections = stats.aabb_rejections.saturating_add(1);
            }
        }
        #[cfg(test)]
        if let Some((token, candidate)) = &self.cancel_at_candidate {
            if self.candidate_pairs == *candidate {
                token.cancel();
            }
        }
        if let Some(policy) = self.policy {
            policy.check_cancelled_every("candidate_enumeration", self.candidate_pairs)?;
            policy.check_noding_work(self.candidate_pairs, self.exact_intersection_calls)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolygonizerDiagnostics {
    #[serde(default = "diagnostics_schema_version")]
    pub schema_version: u32,
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
    #[serde(default)]
    pub z_conflicts: ZConflictStats,
    pub containment_stats: ContainmentStats,
    pub noding_work_stats: NodingWorkStats,
}

fn diagnostics_schema_version() -> u32 {
    POLYGONIZER_DIAGNOSTICS_V1_SCHEMA_VERSION
}

impl Default for PolygonizerDiagnostics {
    fn default() -> Self {
        Self {
            schema_version: diagnostics_schema_version(),
            input_segment_count: 0,
            noded_segment_count: 0,
            dangle_count: 0,
            cut_edge_count: 0,
            ring_count: 0,
            shell_count: 0,
            hole_count: 0,
            unassigned_hole_count: 0,
            unassigned_hole_area: 0.0,
            invalid_ring_count: 0,
            flat_line_count: 0,
            phase_times: PolygonizerPhaseTimes::default(),
            noding_iterations: Vec::new(),
            snap_stats: SnapStats::default(),
            intersection_stats: IntersectionStats::default(),
            z_conflicts: ZConflictStats::default(),
            containment_stats: ContainmentStats::default(),
            noding_work_stats: NodingWorkStats::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_include_a_schema_version() {
        assert_eq!(
            serde_json::to_value(PolygonizerDiagnostics::default()).unwrap()["schema_version"],
            POLYGONIZER_DIAGNOSTICS_V1_SCHEMA_VERSION
        );
    }
}
