use crate::diagnostics::{ExecutionWorkTracker, NodingIterationStats, NodingWorkStats};
use crate::error::PolygonizeError;
use crate::index::{IndexedEnvelope, RStarBackend};
use crate::noding::grid::{
    UniformGrid, UniformGridCandidateTrace, UniformGridCellTrace, UniformGridGlobalLineTrace,
};
use crate::noding::{CandidateIntersectionTrace, CandidatePair, ExactCandidate};
use crate::options::{ExecutionPolicy, SnapStrategy, ZPolicy};
use crate::trace::{TraceCapture, TraceCaptureBudget};
use crate::types::{Coord3D, Line3D};
use crate::utils::soa::SoALines;
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};
use geo::Coord;
use rstar::AABB;
use wide::f64x4;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

fn nearest_reference_vertex(
    coord: Coord3D,
    reference_vertices: impl Iterator<Item = Coord3D>,
    tolerance_sq: f64,
) -> Coord3D {
    reference_vertices
        .filter_map(|vertex| {
            let dx = vertex.x - coord.x;
            let dy = vertex.y - coord.y;
            let dist_sq = dx * dx + dy * dy;
            (dist_sq > 0.0 && dist_sq <= tolerance_sq).then_some((dist_sq, vertex))
        })
        .min_by(|(dist_a, a), (dist_b, b)| {
            dist_a
                .total_cmp(dist_b)
                .then(a.x.total_cmp(&b.x))
                .then(a.y.total_cmp(&b.y))
        })
        .map(|(_, vertex)| vertex)
        .unwrap_or(coord)
}

fn nearest_reference_vertex_indexed(
    coord: Coord3D,
    reference_vertices: &[Coord3D],
    index: &RStarBackend,
    tolerance: f64,
) -> (Coord3D, usize) {
    if !coord.x.is_finite() || !coord.y.is_finite() || !tolerance.is_finite() {
        return (
            nearest_reference_vertex(
                coord,
                reference_vertices.iter().copied(),
                tolerance * tolerance,
            ),
            reference_vertices.len(),
        );
    }

    let query = AABB::from_corners(
        [coord.x - tolerance, coord.y - tolerance],
        [coord.x + tolerance, coord.y + tolerance],
    );
    let tolerance_sq = tolerance * tolerance;
    let mut candidates = 0;
    let nearest = nearest_reference_vertex(
        coord,
        index.locate_in_envelope_intersecting(&query).map(|idx| {
            candidates += 1;
            reference_vertices[idx]
        }),
        tolerance_sq,
    );
    (nearest, candidates)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodingStrategy {
    Auto,
    Scalar, // Deprecated/Fallback (Mapped to SIMD/Grid depending on impl)
    Simd,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FloatingCandidateTrace {
    pub(crate) iteration_index: usize,
    pub(crate) first_segment: usize,
    pub(crate) second_segment: usize,
    pub(crate) first_source_id: u32,
    pub(crate) second_source_id: u32,
    pub(crate) witness: Option<CandidateIntersectionTrace>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FloatingSplitTrace {
    pub(crate) iteration_index: usize,
    pub(crate) source_segment: usize,
    pub(crate) source_id: u32,
    pub(crate) start: Coord3D,
    pub(crate) end: Coord3D,
}

type FloatingNodingTraceResult = (
    Vec<Line3D>,
    Vec<NodingIterationStats>,
    NodingWorkStats,
    Vec<FloatingCandidateTrace>,
    Vec<UniformGridCellTrace>,
    Vec<UniformGridGlobalLineTrace>,
    Vec<UniformGridCandidateTrace>,
    Vec<FloatingSplitTrace>,
    bool,
);

struct FloatingTraceCapture {
    candidates: Vec<FloatingCandidateTrace>,
    grid_cells: Vec<UniformGridCellTrace>,
    global_lines: Vec<UniformGridGlobalLineTrace>,
    grid_candidates: Vec<UniformGridCandidateTrace>,
    splits: Vec<FloatingSplitTrace>,
    budget: TraceCaptureBudget,
}

impl FloatingTraceCapture {
    fn new(byte_limit: usize) -> Self {
        Self {
            candidates: Vec::new(),
            grid_cells: Vec::new(),
            global_lines: Vec::new(),
            grid_candidates: Vec::new(),
            splits: Vec::new(),
            budget: TraceCaptureBudget::new(byte_limit),
        }
    }
}

const AUTO_SIMD_LIMIT: usize = 1024;

pub struct SnapNoder {
    pub grid_size: f64,
    pub max_iter: usize,
    pub strategy: NodingStrategy,
    pub snap_strategy: SnapStrategy,
    pub z_policy: ZPolicy,
}

impl SnapNoder {
    pub fn new(grid_size: f64) -> Self {
        Self {
            grid_size,
            max_iter: 10,
            strategy: NodingStrategy::Auto,
            snap_strategy: SnapStrategy::Grid,
            z_policy: ZPolicy::InterpolateAlongEdge,
        }
    }

    pub fn with_strategy(mut self, strategy: NodingStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_snap_strategy(mut self, snap_strategy: SnapStrategy) -> Self {
        self.snap_strategy = snap_strategy;
        self
    }

    pub fn with_z_policy(mut self, z_policy: ZPolicy) -> Self {
        self.z_policy = z_policy;
        self
    }

    pub fn node(&self, lines: Vec<Line3D>) -> Vec<Line3D> {
        self.node_impl(lines, None, None, None, None)
            .expect("unlimited noding cannot fail")
    }

    pub(crate) fn node_with_stats(
        &self,
        lines: Vec<Line3D>,
    ) -> (Vec<Line3D>, Vec<NodingIterationStats>, NodingWorkStats) {
        let mut stats = Vec::new();
        let mut work_stats = NodingWorkStats::default();
        let lines = self
            .node_impl(lines, Some(&mut stats), Some(&mut work_stats), None, None)
            .expect("unlimited noding cannot fail");
        (lines, stats, work_stats)
    }

    pub(crate) fn node_with_execution_policy(
        &self,
        lines: Vec<Line3D>,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<Vec<Line3D>> {
        self.node_impl(lines, None, None, Some(execution_policy), None)
    }

    pub(crate) fn node_with_stats_and_execution_policy(
        &self,
        lines: Vec<Line3D>,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<(Vec<Line3D>, Vec<NodingIterationStats>, NodingWorkStats)> {
        let mut stats = Vec::new();
        let mut work_stats = NodingWorkStats::default();
        let lines = self.node_impl(
            lines,
            Some(&mut stats),
            Some(&mut work_stats),
            Some(execution_policy),
            None,
        )?;
        Ok((lines, stats, work_stats))
    }

    pub(crate) fn node_with_trace(
        &self,
        lines: Vec<Line3D>,
        execution_policy: Option<&ExecutionPolicy>,
        capture_byte_limit: usize,
    ) -> crate::Result<FloatingNodingTraceResult> {
        let mut stats = Vec::new();
        let mut work_stats = NodingWorkStats::default();
        let mut trace = FloatingTraceCapture::new(capture_byte_limit);
        let lines = self.node_impl(
            lines,
            Some(&mut stats),
            Some(&mut work_stats),
            execution_policy,
            Some(&mut trace),
        )?;
        Ok((
            lines,
            stats,
            work_stats,
            trace.candidates,
            trace.grid_cells,
            trace.global_lines,
            trace.grid_candidates,
            trace.splits,
            trace.budget.truncated(),
        ))
    }

    fn node_impl(
        &self,
        mut lines: Vec<Line3D>,
        mut stats: Option<&mut Vec<NodingIterationStats>>,
        mut work_stats: Option<&mut NodingWorkStats>,
        execution_policy: Option<&ExecutionPolicy>,
        mut trace: Option<&mut FloatingTraceCapture>,
    ) -> crate::Result<Vec<Line3D>> {
        // 1. Initial Snap of endpoints
        for line in &mut lines {
            line.start = self.snap(line.start);
            line.end = self.snap(line.end);
        }

        // Remove degenerates and invalid lines
        lines.retain(|l| {
            let start = l.start.to_coord_2d();
            let end = l.end.to_coord_2d();
            start != end
                && l.start.x.is_finite()
                && l.start.y.is_finite()
                && l.end.x.is_finite()
                && l.end.y.is_finite()
        });

        // Normalize and dedup initial input
        self.normalize_and_dedup(&mut lines);

        // 2. Iterative Noding
        let auto_prefers_simd =
            self.strategy == NodingStrategy::Auto && self.auto_prefers_simd(&lines);
        let mut new_lines = Vec::new();
        let mut split_events = 0;
        for iteration_index in 0..self.max_iter {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled("candidate_enumeration")?;
                execution_policy.check_noding_iterations(iteration_index + 1)?;
            }
            let input_segment_count = lines.len();
            let use_grid = match self.strategy {
                NodingStrategy::Auto => {
                    lines.len() >= 256 && !(auto_prefers_simd && lines.len() <= AUTO_SIMD_LIMIT)
                }
                NodingStrategy::Grid => true,
                NodingStrategy::Simd => false,
                NodingStrategy::Scalar => false, // Fallback to SIMD logic which handles scalar internally
            };

            let mut events = if !use_grid {
                // STRATEGY A: Small Input -> SIMD Brute Force
                if trace.is_some() {
                    let mut tracker =
                        ExecutionWorkTracker::new(execution_policy, work_stats.as_deref_mut());
                    let trace = trace.as_deref_mut().unwrap();
                    let first_candidate = trace.candidates.len();
                    let mut candidates =
                        TraceCapture::new(&mut trace.candidates, &mut trace.budget);
                    let events =
                        self.find_splits_simd_tracked(&lines, &mut tracker, Some(&mut candidates))?;
                    for candidate in &mut trace.candidates[first_candidate..] {
                        candidate.iteration_index = iteration_index;
                    }
                    events
                } else if execution_policy.is_some() || work_stats.is_some() {
                    let mut tracker =
                        ExecutionWorkTracker::new(execution_policy, work_stats.as_deref_mut());
                    self.find_splits_simd_tracked(&lines, &mut tracker, None)?
                } else {
                    self.find_splits_simd(&lines)
                }
            } else {
                // STRATEGY B: Large Input -> Uniform Grid
                let grid = if let Some(execution_policy) = execution_policy {
                    UniformGrid::new_with_execution_policy(&lines, execution_policy)?
                } else {
                    UniformGrid::new(&lines)
                };
                if let Some(trace) = trace.as_deref_mut() {
                    let (cells, global_lines) =
                        grid.trace_structure(&lines, iteration_index, &mut trace.budget);
                    trace.grid_cells.extend(cells);
                    trace.global_lines.extend(global_lines);
                }
                if execution_policy.is_some() || work_stats.is_some() {
                    let mut tracker =
                        ExecutionWorkTracker::new(execution_policy, work_stats.as_deref_mut());
                    let mut grid_candidates = trace.as_deref_mut().map(|trace| {
                        TraceCapture::new(&mut trace.grid_candidates, &mut trace.budget)
                    });
                    grid.find_splits_tracked(
                        &lines,
                        self,
                        &mut tracker,
                        iteration_index,
                        grid_candidates.as_mut(),
                    )?
                } else {
                    grid.find_splits(&lines, self)
                }
            };

            if events.is_empty() {
                if let Some(stats) = stats.as_deref_mut() {
                    stats.push(NodingIterationStats {
                        iteration_index,
                        intersections_found: 0,
                        nodes_added: 0,
                    });
                }
                break;
            }

            // Sort and dedup events by (line_index, split_point) to stabilize near-equal repeats.
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_uncancellable_sort("noding_candidate_sort", events.len())?;
            }
            events.sort_unstable_by(|a, b| {
                a.0.cmp(&b.0)
                    .then(a.1.x.total_cmp(&b.1.x))
                    .then(a.1.y.total_cmp(&b.1.y))
            });
            events.dedup_by(|a, b| {
                a.0 == b.0 && a.1.x == b.1.x && a.1.y == b.1.y
                // Note: We don't check Z for dedup because for a given line index and X,Y,
                // Z should be consistent (interpolated from the same line).
            });
            let split_event_count = events.len();
            split_events += split_event_count;
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled("split_application")?;
                execution_policy.check_split_events(split_events)?;
            }
            if let Some(work_stats) = work_stats.as_deref_mut() {
                work_stats.split_events += split_event_count;
            }

            // Early bailout heuristic to avoid epsilon-thrashing on tiny residual updates.
            // Apply this iteration first, then exit before running another pass.
            let should_bail_early = events.len() < 3;

            // Apply splits. Copy untouched line ranges in bulk and only rebuild lines with split events.
            new_lines.clear();
            let estimated_len = lines.len().checked_add(events.len()).ok_or_else(|| {
                PolygonizeError::InternalInvariantViolation {
                    reason: "noded segment capacity overflow".to_string(),
                }
            })?;
            new_lines.reserve(
                execution_policy
                    .and_then(|policy| policy.max_noded_segments)
                    .map_or(estimated_len, |limit| estimated_len.min(limit)),
            );

            let mut event_idx = 0;
            let mut src_idx = 0;
            // Buffer to reuse for collecting points on the current split line.
            let mut points = Vec::new();

            while event_idx < events.len() {
                let line_idx = events[event_idx].0;

                // Copy untouched lines directly.
                if src_idx < line_idx {
                    if let Some(execution_policy) = execution_policy {
                        execution_policy.check(
                            "noded_segments",
                            execution_policy.max_noded_segments,
                            new_lines
                                .len()
                                .checked_add(line_idx - src_idx)
                                .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                                    reason: "noded segment count overflow".to_string(),
                                })?,
                        )?;
                    }
                    new_lines.extend_from_slice(&lines[src_idx..line_idx]);
                }

                points.clear();
                while event_idx < events.len() && events[event_idx].0 == line_idx {
                    if let Some(execution_policy) = execution_policy {
                        execution_policy.check_cancelled_every("split_application", event_idx)?;
                    }
                    points.push(events[event_idx].1);
                    event_idx += 1;
                }

                let line = lines[line_idx];
                points.push(line.start);
                points.push(line.end);

                // Filter out invalid points (NaN/Inf)
                points.retain(|p| p.x.is_finite() && p.y.is_finite());

                // Sort by parametric t value (dot product of (p - start) with direction vector)
                let start = line.start;
                let dx = line.end.x - start.x;
                let dy = line.end.y - start.y;
                let len_sq = dx * dx + dy * dy;

                if len_sq > 0.0 {
                    if let Some(execution_policy) = execution_policy {
                        execution_policy
                            .check_uncancellable_sort("split_point_sort", points.len())?;
                    }
                    points.sort_unstable_by(|a, b| {
                        let ta = ((a.x - start.x) * dx + (a.y - start.y) * dy) / len_sq;
                        let tb = ((b.x - start.x) * dx + (b.y - start.y) * dy) / len_sq;
                        ta.total_cmp(&tb)
                    });
                } else {
                    // Fallback to sort by X, then Y if segment is a zero-length point
                    if let Some(execution_policy) = execution_policy {
                        execution_policy
                            .check_uncancellable_sort("split_point_sort", points.len())?;
                    }
                    points.sort_unstable_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
                }

                // Dedup by 2D coordinates
                points.dedup_by(|a, b| a.x == b.x && a.y == b.y);

                // Create replacement segments for the split line.
                for (replacement_index, w) in points.windows(2).enumerate() {
                    if let Some(execution_policy) = execution_policy {
                        execution_policy
                            .check_cancelled_every("split_application", replacement_index)?;
                    }
                    let p0 = w[0];
                    let p1 = w[1];
                    // Check 2D equality
                    if p0.x != p1.x || p0.y != p1.y {
                        if let Some(execution_policy) = execution_policy {
                            execution_policy.check(
                                "noded_segments",
                                execution_policy.max_noded_segments,
                                new_lines.len().checked_add(1).ok_or_else(|| {
                                    PolygonizeError::InternalInvariantViolation {
                                        reason: "noded segment count overflow".to_string(),
                                    }
                                })?,
                            )?;
                        }
                        new_lines.push(Line3D::new(p0, p1, line.line_id));
                        if let Some(trace) = trace.as_deref_mut() {
                            trace.budget.capture(
                                &mut trace.splits,
                                FloatingSplitTrace {
                                    iteration_index,
                                    source_segment: line_idx,
                                    source_id: line.line_id,
                                    start: p0,
                                    end: p1,
                                },
                            );
                        }
                    }
                }

                src_idx = line_idx + 1;
            }

            // Copy any untouched trailing lines.
            if src_idx < lines.len() {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check(
                        "noded_segments",
                        execution_policy.max_noded_segments,
                        new_lines
                            .len()
                            .checked_add(lines.len() - src_idx)
                            .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                                reason: "noded segment count overflow".to_string(),
                            })?,
                    )?;
                }
                new_lines.extend_from_slice(&lines[src_idx..]);
            }

            self.normalize_and_dedup(&mut new_lines);
            std::mem::swap(&mut lines, &mut new_lines);

            if let Some(stats) = stats.as_deref_mut() {
                stats.push(NodingIterationStats {
                    iteration_index,
                    intersections_found: split_event_count,
                    nodes_added: lines.len().saturating_sub(input_segment_count),
                });
            }

            if should_bail_early {
                break;
            }
        }

        Ok(lines)
    }

    fn auto_prefers_simd(&self, lines: &[Line3D]) -> bool {
        // ponytail: bounded deterministic sample; expand only if cross-arch benchmarks
        // show that 16 evenly spaced lines misclassify real workloads.
        const SAMPLE_SIZE: usize = 16;
        if !(256..=AUTO_SIMD_LIMIT).contains(&lines.len()) {
            return false;
        }

        let last = lines.len() - 1;
        let mut split_pairs = 0;
        let mut pairs = 0;

        for left_sample in 0..SAMPLE_SIZE {
            let left = left_sample * last / (SAMPLE_SIZE - 1);
            for right_sample in left_sample + 1..SAMPLE_SIZE {
                let right = right_sample * last / (SAMPLE_SIZE - 1);
                let mut produces_split = false;
                self.process_intersection(lines[left], lines[right], left, right, |_, _| {
                    produces_split = true;
                });
                split_pairs += usize::from(produces_split);
                pairs += 1;
            }
        }

        split_pairs * 4 >= pairs
    }

    pub fn pre_snap_to_reference_vertices(lines: &[Line3D], tolerance: f64) -> Vec<Line3D> {
        Self::pre_snap_impl(lines, tolerance, true, ZPolicy::InterpolateAlongEdge, None)
            .expect("unlimited pre-snap cannot fail")
            .0
    }

    pub(crate) fn pre_snap_to_reference_vertices_with_stats(
        lines: &[Line3D],
        tolerance: f64,
        z_policy: ZPolicy,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<(Vec<Line3D>, usize)> {
        Self::pre_snap_impl(lines, tolerance, true, z_policy, Some(execution_policy))
    }

    fn pre_snap_impl(
        lines: &[Line3D],
        tolerance: f64,
        use_index: bool,
        z_policy: ZPolicy,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<(Vec<Line3D>, usize)> {
        if let Some(policy) = execution_policy {
            policy.check_cancelled("pre_snap")?;
        }
        if lines.is_empty() || tolerance <= 0.0 {
            return Ok((lines.to_vec(), 0));
        }

        let exact_noded = if let Some(policy) = execution_policy {
            SnapNoder::new(0.0).node_with_execution_policy(lines.to_vec(), policy)?
        } else {
            SnapNoder::new(0.0).node(lines.to_vec())
        };
        let mut reference_vertices = Vec::with_capacity(exact_noded.len().saturating_mul(2));
        for (index, line) in exact_noded.into_iter().enumerate() {
            if let Some(policy) = execution_policy {
                policy.check_cancelled_every("pre_snap", index)?;
            }
            reference_vertices.extend([line.start, line.end]);
        }

        if let Some(policy) = execution_policy {
            policy.check_cancelled("pre_snap")?;
        }
        reference_vertices.sort_unstable_by(|a, b| {
            a.x.total_cmp(&b.x)
                .then(a.y.total_cmp(&b.y))
                .then(a.z.total_cmp(&b.z))
        });
        reference_vertices.dedup_by(|a, b| a.x == b.x && a.y == b.y);

        let vertex_index = if use_index {
            let mut entries = Vec::with_capacity(reference_vertices.len());
            for (index, vertex) in reference_vertices.iter().enumerate() {
                if let Some(policy) = execution_policy {
                    policy.check_cancelled_every("pre_snap", index)?;
                }
                entries.push(IndexedEnvelope {
                    aabb: AABB::from_corners([vertex.x, vertex.y], [vertex.x, vertex.y]),
                    index,
                });
            }
            Some(RStarBackend::new(entries))
        } else {
            None
        };

        let tolerance_sq = tolerance * tolerance;
        let mut snapped = Vec::with_capacity(lines.len());
        let mut points = Vec::new();
        let mut vertex_candidates = 0;
        let z_resolver = SnapNoder::new(0.0).with_z_policy(z_policy);

        for (line_index, &line) in lines.iter().enumerate() {
            if let Some(policy) = execution_policy {
                policy.check_cancelled_every("pre_snap", line_index)?;
            }
            let (mut start, start_candidates) = if let Some(index) = vertex_index.as_ref() {
                nearest_reference_vertex_indexed(line.start, &reference_vertices, index, tolerance)
            } else {
                (
                    nearest_reference_vertex(
                        line.start,
                        reference_vertices.iter().copied(),
                        tolerance_sq,
                    ),
                    reference_vertices.len(),
                )
            };
            let (mut end, end_candidates) = if let Some(index) = vertex_index.as_ref() {
                nearest_reference_vertex_indexed(line.end, &reference_vertices, index, tolerance)
            } else {
                (
                    nearest_reference_vertex(
                        line.end,
                        reference_vertices.iter().copied(),
                        tolerance_sq,
                    ),
                    reference_vertices.len(),
                )
            };
            start.z = z_resolver.interpolate_z(start.to_coord_2d(), line);
            end.z = z_resolver.interpolate_z(end.to_coord_2d(), line);
            vertex_candidates += start_candidates + end_candidates;
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let len_sq = dx * dx + dy * dy;
            if len_sq == 0.0 {
                continue;
            }

            points.clear();
            points.push((0.0, start));
            points.push((1.0, end));

            {
                let mut consider_vertex = |vertex: Coord3D| -> crate::Result<()> {
                    vertex_candidates = vertex_candidates.checked_add(1).ok_or_else(|| {
                        PolygonizeError::InternalInvariantViolation {
                            reason: "pre-snap candidate counter overflow".to_string(),
                        }
                    })?;
                    if let Some(policy) = execution_policy {
                        policy.check_cancelled_every("pre_snap", vertex_candidates)?;
                    }
                    let vx = vertex.x - start.x;
                    let vy = vertex.y - start.y;
                    let t = (vx * dx + vy * dy) / len_sq;
                    if !(0.0..=1.0).contains(&t) {
                        return Ok(());
                    }

                    let nearest_x = start.x + t * dx;
                    let nearest_y = start.y + t * dy;
                    let dist_x = vertex.x - nearest_x;
                    let dist_y = vertex.y - nearest_y;
                    if dist_x * dist_x + dist_y * dist_y <= tolerance_sq {
                        points.push((
                            t,
                            Coord3D::new(
                                vertex.x,
                                vertex.y,
                                z_resolver.interpolate_z(vertex.to_coord_2d(), line),
                            ),
                        ));
                    }
                    Ok(())
                };

                if let Some(index) = vertex_index.as_ref().filter(|_| {
                    start.x.is_finite()
                        && start.y.is_finite()
                        && end.x.is_finite()
                        && end.y.is_finite()
                        && tolerance.is_finite()
                }) {
                    let query = AABB::from_corners(
                        [
                            start.x.min(end.x) - tolerance,
                            start.y.min(end.y) - tolerance,
                        ],
                        [
                            start.x.max(end.x) + tolerance,
                            start.y.max(end.y) + tolerance,
                        ],
                    );
                    for idx in index.locate_in_envelope_intersecting(&query) {
                        consider_vertex(reference_vertices[idx])?;
                    }
                } else {
                    for &vertex in &reference_vertices {
                        consider_vertex(vertex)?;
                    }
                }
            }

            points.sort_unstable_by(|(ta, a), (tb, b)| {
                ta.total_cmp(tb).then_with(|| {
                    if dx.abs() >= dy.abs() {
                        b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x))
                    } else {
                        b.x.total_cmp(&a.x).then(a.y.total_cmp(&b.y))
                    }
                })
            });
            points.dedup_by(|a, b| a.1.x == b.1.x && a.1.y == b.1.y);

            for pair in points.windows(2) {
                let p0 = pair[0].1;
                let p1 = pair[1].1;
                if p0.x != p1.x || p0.y != p1.y {
                    snapped.push(Line3D::new(p0, p1, line.line_id));
                }
            }
        }

        if let Some(policy) = execution_policy {
            policy.check_cancelled("pre_snap")?;
        }
        Ok((snapped, vertex_candidates))
    }

    pub(crate) fn normalize_and_dedup(&self, lines: &mut Vec<Line3D>) {
        // Filter out invalid lines (NaN or infinite coordinates)
        lines.retain(|l| {
            l.start.x.is_finite()
                && l.start.y.is_finite()
                && l.end.x.is_finite()
                && l.end.y.is_finite()
        });

        for segment in lines.iter_mut() {
            if segment.start.x > segment.end.x
                || (segment.start.x == segment.end.x && segment.start.y > segment.end.y)
            {
                std::mem::swap(&mut segment.start, &mut segment.end);
            }
        }
        lines.sort_by(|a, b| {
            a.start
                .x
                .total_cmp(&b.start.x)
                .then(a.start.y.total_cmp(&b.start.y))
                .then(a.end.x.total_cmp(&b.end.x))
                .then(a.end.y.total_cmp(&b.end.y))
                .then(a.line_id.cmp(&b.line_id))
        });
        lines.dedup_by(|a, b| {
            a.start.x == b.start.x
                && a.start.y == b.start.y
                && a.end.x == b.end.x
                && a.end.y == b.end.y
                && a.line_id == b.line_id
        });
    }

    pub(crate) fn snap(&self, c: Coord3D) -> Coord3D {
        if self.grid_size == 0.0 {
            return c;
        }

        let snap_val = |v: f64| -> f64 {
            match self.snap_strategy {
                SnapStrategy::Grid => (v / self.grid_size).round() * self.grid_size,
                SnapStrategy::GeosCompat => {
                    // GEOS C++ GEOSGeom_setPrecision uses std::round (round halfway cases away from zero).
                    // This behaves identically to Rust's native `.round()`, however providing
                    // a dedicated code path allows future divergence tuning for exact Shapely parity.
                    {
                        let scaled = v / self.grid_size;
                        // GEOS rounding behavior (C++ std::round):
                        // Round half away from zero.
                        let sign = scaled.signum();
                        let abs = scaled.abs();
                        let abs_rounded = (abs + 0.5).floor();
                        abs_rounded * sign * self.grid_size
                    }
                }
            }
        };

        Coord3D {
            x: snap_val(c.x),
            y: snap_val(c.y),
            z: c.z, // Keep Z unchanged
        }
    }

    // Interpolates Z value for a point (px, py) assumed to be on the line segment
    pub(crate) fn interpolate_z(&self, p: Coord<f64>, line: Line3D) -> f64 {
        let l_dx = line.end.x - line.start.x;
        let l_dy = line.end.y - line.start.y;
        let l_len_sq = l_dx * l_dx + l_dy * l_dy;

        if l_len_sq < 1e-18 {
            return line.start.z;
        }

        // Project p onto line to find t
        let dx = p.x - line.start.x;
        let dy = p.y - line.start.y;

        // Dot product projection
        let t = (dx * l_dx + dy * l_dy) / l_len_sq;

        // Clamp t to [0, 1] for safety, although intersection should be on segment
        let t = t.clamp(0.0, 1.0);

        if matches!(self.z_policy, ZPolicy::PreferNearestEndpoint) {
            return if t < 0.5 {
                line.start.z
            } else if t > 0.5 {
                line.end.z
            } else if line
                .start
                .x
                .total_cmp(&line.end.x)
                .then(line.start.y.total_cmp(&line.end.y))
                .then(line.start.z.total_cmp(&line.end.z))
                .is_le()
            {
                line.start.z
            } else {
                line.end.z
            };
        }

        line.start.z + t * (line.end.z - line.start.z)
    }

    #[inline]
    pub(crate) fn handle_intersection<F>(
        &self,
        res: LineIntersection<f64>,
        i: usize,
        j: usize,
        l1: Line3D,
        l2: Line3D,
        mut handler: F,
    ) where
        F: FnMut(usize, Coord3D),
    {
        match res {
            LineIntersection::SinglePoint {
                intersection: pt, ..
            } => {
                // Snap the 2D intersection point
                let snapped_2d = {
                    let s = self.snap(Coord3D::new(pt.x, pt.y, 0.0));
                    s.to_coord_2d()
                };

                let l1_start_2d = l1.start.to_coord_2d();
                let l1_end_2d = l1.end.to_coord_2d();
                let l2_start_2d = l2.start.to_coord_2d();
                let l2_end_2d = l2.end.to_coord_2d();

                if snapped_2d != l1_start_2d && snapped_2d != l1_end_2d {
                    let z = self.interpolate_z(snapped_2d, l1);
                    handler(i, Coord3D::new(snapped_2d.x, snapped_2d.y, z));
                }
                if snapped_2d != l2_start_2d && snapped_2d != l2_end_2d {
                    let z = self.interpolate_z(snapped_2d, l2);
                    handler(j, Coord3D::new(snapped_2d.x, snapped_2d.y, z));
                }
            }
            LineIntersection::Collinear {
                intersection: overlap,
            } => {
                // For collinear, we process endpoints of the overlap
                let p1_2d = {
                    let s = self.snap(Coord3D::new(overlap.start.x, overlap.start.y, 0.0));
                    s.to_coord_2d()
                };
                let p2_2d = {
                    let s = self.snap(Coord3D::new(overlap.end.x, overlap.end.y, 0.0));
                    s.to_coord_2d()
                };

                for p in [p1_2d, p2_2d] {
                    let l1_start_2d = l1.start.to_coord_2d();
                    let l1_end_2d = l1.end.to_coord_2d();
                    let l2_start_2d = l2.start.to_coord_2d();
                    let l2_end_2d = l2.end.to_coord_2d();

                    if p != l1_start_2d && p != l1_end_2d {
                        let z = self.interpolate_z(p, l1);
                        handler(i, Coord3D::new(p.x, p.y, z));
                    }
                    if p != l2_start_2d && p != l2_end_2d {
                        let z = self.interpolate_z(p, l2);
                        handler(j, Coord3D::new(p.x, p.y, z));
                    }
                }
            }
        }
    }

    fn find_splits_simd(&self, lines: &[Line3D]) -> Vec<(usize, Coord3D)> {
        let soa = SoALines::new(lines);

        #[cfg(feature = "parallel")]
        {
            // Rayon Heuristic: Thread spin-up dominates for small N.
            // Use sequential loop if lines < 1000.
            if lines.len() >= 1000 {
                // Parallel execution: each thread processes a subset of query lines
                // and returns a list of split events (line_index, point).
                lines
                    .par_iter()
                    .enumerate()
                    .flat_map_iter(|(i, &query_line)| {
                        let mut events = Vec::new();
                        self.visit_candidate_pairs_simd(
                            query_line,
                            i,
                            lines,
                            &soa,
                            None,
                            |candidate| {
                                self.process_candidate_pair(lines, candidate, None, &mut events);
                                Ok(())
                            },
                        )
                        .expect("unlimited noding cannot fail");
                        events
                    })
                    .collect()
            } else {
                // Sequential fallback
                let mut splits = Vec::new();
                for (i, &query_line) in lines.iter().enumerate() {
                    self.visit_candidate_pairs_simd(
                        query_line,
                        i,
                        lines,
                        &soa,
                        None,
                        |candidate| {
                            self.process_candidate_pair(lines, candidate, None, &mut splits);
                            Ok(())
                        },
                    )
                    .expect("unlimited noding cannot fail");
                }
                splits
            }
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut splits = Vec::new();
            for (i, &query_line) in lines.iter().enumerate() {
                self.visit_candidate_pairs_simd(query_line, i, lines, &soa, None, |candidate| {
                    self.process_candidate_pair(lines, candidate, None, &mut splits);
                    Ok(())
                })
                .expect("unlimited noding cannot fail");
            }
            splits
        }
    }

    fn find_splits_simd_tracked(
        &self,
        lines: &[Line3D],
        tracker: &mut ExecutionWorkTracker<'_>,
        mut trace_candidates: Option<&mut TraceCapture<'_, FloatingCandidateTrace>>,
    ) -> crate::Result<Vec<(usize, Coord3D)>> {
        let soa = SoALines::new(lines);
        let mut splits = Vec::new();
        tracker.check_cancelled()?;
        for (i, &query_line) in lines.iter().enumerate() {
            self.visit_candidate_pairs_simd(
                query_line,
                i,
                lines,
                &soa,
                Some(tracker),
                |candidate| {
                    self.process_candidate_pair(
                        lines,
                        candidate,
                        trace_candidates.as_deref_mut(),
                        &mut splits,
                    );
                    Ok(())
                },
            )?;
        }
        tracker.check_cancelled()?;
        Ok(splits)
    }

    #[inline]
    pub(crate) fn process_intersection<F>(
        &self,
        l1: Line3D,
        l2: Line3D,
        i: usize,
        j: usize,
        handler: F,
    ) where
        F: FnMut(usize, Coord3D),
    {
        let l1_2d = l1.to_line_2d();
        let l2_2d = l2.to_line_2d();

        if let Some(res) = line_intersection(l1_2d, l2_2d) {
            self.handle_intersection(res, i, j, l1, l2, handler);
        }
    }

    // Broad phase: visit AABB-overlapping pairs using the SIMD SoA.
    #[allow(clippy::manual_div_ceil)]
    #[inline]
    fn visit_candidate_pairs_simd<F>(
        &self,
        query_line: Line3D,
        i: usize,
        lines: &[Line3D],
        soa: &SoALines,
        mut tracker: Option<&mut ExecutionWorkTracker<'_>>,
        mut visit: F,
    ) -> crate::Result<()>
    where
        F: FnMut(CandidatePair) -> crate::Result<()>,
    {
        // Start block to avoid duplicate checks (j > i)
        // We start checking at index i+1.
        // The SoA batching index `j` steps by 4.
        // We want `j` such that the batch covers indices > i.
        // Ideally start `j` at next multiple of 4
        // Round UP: (i + 1 + 3) / 4 * 4
        let start_block = (i + 1 + 3) / 4 * 4;

        // Handling unaligned start to be absolutely safe and avoid self-check artifacts
        #[allow(clippy::needless_range_loop)]
        for j in (i + 1)..start_block.min(lines.len()) {
            let target_line = lines[j];
            // Standard BBox check
            let q_min_x = query_line.start.x.min(query_line.end.x);
            let q_max_x = query_line.start.x.max(query_line.end.x);
            let q_min_y = query_line.start.y.min(query_line.end.y);
            let q_max_y = query_line.start.y.max(query_line.end.y);

            let t_min_x = target_line.start.x.min(target_line.end.x);
            let t_max_x = target_line.start.x.max(target_line.end.x);
            let t_min_y = target_line.start.y.min(target_line.end.y);
            let t_max_y = target_line.start.y.max(target_line.end.y);

            let overlaps = q_max_x >= t_min_x
                && q_min_x <= t_max_x
                && q_max_y >= t_min_y
                && q_min_y <= t_max_y;
            if let Some(tracker) = tracker.as_deref_mut() {
                tracker.candidate(overlaps)?;
            }
            if overlaps {
                visit(CandidatePair {
                    first: i,
                    second: j,
                })?;
            }
        }

        // Pre-calculate query BBox splats
        let q_min_x = f64x4::splat(query_line.start.x.min(query_line.end.x));
        let q_max_x = f64x4::splat(query_line.start.x.max(query_line.end.x));
        let q_min_y = f64x4::splat(query_line.start.y.min(query_line.end.y));
        let q_max_y = f64x4::splat(query_line.start.y.max(query_line.end.y));

        for j in (start_block..soa.len()).step_by(4) {
            let mask = soa.intersects_bbox_batch_splatted(q_min_x, q_max_x, q_min_y, q_max_y, j);

            for k in 0..4 {
                let target_idx = j + k;
                if target_idx >= lines.len() || target_idx <= i {
                    continue;
                }
                let overlaps = (mask & (1 << k)) != 0;
                if let Some(tracker) = tracker.as_deref_mut() {
                    tracker.candidate(overlaps)?;
                }
                if overlaps {
                    visit(CandidatePair {
                        first: i,
                        second: target_idx,
                    })?;
                }
            }
        }
        Ok(())
    }

    // Exact phase: robust intersection, trace capture, and split accumulation.
    fn process_candidate_pair(
        &self,
        lines: &[Line3D],
        candidate: CandidatePair,
        trace_candidates: Option<&mut TraceCapture<'_, FloatingCandidateTrace>>,
        events: &mut Vec<(usize, Coord3D)>,
    ) {
        let exact = ExactCandidate::evaluate(lines, candidate);
        if let Some(trace_candidates) = trace_candidates {
            trace_candidates.push(FloatingCandidateTrace {
                iteration_index: 0,
                first_segment: candidate.first,
                second_segment: candidate.second,
                first_source_id: exact.first.line_id,
                second_source_id: exact.second.line_id,
                witness: exact.witness(),
            });
        }
        self.append_exact_candidate_splits(exact, events);
    }

    pub(crate) fn append_exact_candidate_splits(
        &self,
        exact: ExactCandidate,
        events: &mut Vec<(usize, Coord3D)>,
    ) {
        if let Some(intersection) = exact.intersection {
            self.handle_intersection(
                intersection,
                exact.pair.first,
                exact.pair.second,
                exact.first,
                exact.second,
                |index, point| events.push((index, point)),
            );
        }
    }

    #[inline]
    pub fn check_intersection(
        &self,
        lines: &[Line3D],
        i: usize,
        j: usize,
        events: &mut Vec<(usize, Coord3D)>,
    ) {
        if i >= lines.len() || j >= lines.len() {
            return;
        }

        let l1 = lines[i];
        let l2 = lines[j];

        self.process_intersection(l1, l2, i, j, |idx, pt| {
            events.push((idx, pt));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        normalize_polygonize_error, polygonize, CancellationToken, ExecutionPolicy,
        PolygonizeError, PolygonizerOptions, ProvenanceOptions, TopologyFingerprintV1, ZOptions,
        ZPolicy,
    };
    use rand::Rng;
    use std::collections::BTreeMap;

    fn make_line(x1: f64, y1: f64, x2: f64, y2: f64) -> Line3D {
        Line3D::new(Coord3D::new(x1, y1, 0.0), Coord3D::new(x2, y2, 0.0), 0)
    }

    fn assert_floating_backend_conformance(lines: &[Line3D]) -> (Vec<Line3D>, Vec<Line3D>) {
        let (simd_noded, _, simd_work, simd_candidates, _, _, _, _, _) = SnapNoder::new(0.0)
            .with_strategy(NodingStrategy::Simd)
            .node_with_trace(lines.to_vec(), None, usize::MAX)
            .unwrap();
        let (grid_noded, _, grid_work, _, _, _, grid_candidates, _, _) = SnapNoder::new(0.0)
            .with_strategy(NodingStrategy::Grid)
            .node_with_trace(lines.to_vec(), None, usize::MAX)
            .unwrap();

        assert_eq!(
            simd_work.candidate_pairs,
            simd_work.aabb_rejections + simd_work.exact_intersection_calls
        );
        assert_eq!(
            grid_work.candidate_pairs,
            grid_work.aabb_rejections + grid_work.exact_intersection_calls
        );
        assert_eq!(simd_work.exact_intersection_calls, simd_candidates.len());
        assert_eq!(grid_work.exact_intersection_calls, grid_candidates.len());

        let simd_outcomes: BTreeMap<_, _> = simd_candidates
            .iter()
            .map(|candidate| {
                (
                    (
                        candidate.iteration_index,
                        candidate.first_segment,
                        candidate.second_segment,
                        candidate.first_source_id,
                        candidate.second_source_id,
                    ),
                    candidate.witness,
                )
            })
            .collect();
        assert_eq!(simd_outcomes.len(), simd_candidates.len());
        let mut grid_outcomes = BTreeMap::new();
        for candidate in grid_candidates {
            let key = (
                candidate.iteration_index,
                candidate.first_segment,
                candidate.second_segment,
                candidate.first_source_id,
                candidate.second_source_id,
            );
            if let Some(previous) = grid_outcomes.insert(key, candidate.witness) {
                assert_eq!(previous, candidate.witness);
            }
        }
        assert_eq!(simd_outcomes, grid_outcomes);

        let line_bits = |lines: &[Line3D]| {
            lines
                .iter()
                .map(|line| {
                    (
                        line.line_id,
                        line.start.x.to_bits(),
                        line.start.y.to_bits(),
                        line.start.z.to_bits(),
                        line.end.x.to_bits(),
                        line.end.y.to_bits(),
                        line.end.z.to_bits(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(line_bits(&simd_noded), line_bits(&grid_noded));

        (simd_noded, grid_noded)
    }

    #[test]
    fn test_grid_vs_simd_equivalence() {
        let mut rng = rand::thread_rng();
        let mut lines = Vec::new();

        // Generate 100 random lines in a 100x100 grid
        for _ in 0..100 {
            let x1 = rng.gen_range(0.0..100.0);
            let y1 = rng.gen_range(0.0..100.0);
            let x2 = rng.gen_range(0.0..100.0);
            let y2 = rng.gen_range(0.0..100.0);
            lines.push(make_line(x1, y1, x2, y2));
        }

        // Add some guaranteed intersections
        lines.push(make_line(0.0, 0.0, 10.0, 10.0));
        lines.push(make_line(0.0, 10.0, 10.0, 0.0));

        let noder = SnapNoder::new(0.001);

        // Grid Logic (Force use by calling directly)
        let grid = UniformGrid::new(&lines);
        let mut splits_grid = grid.find_splits(&lines, &noder);

        // SIMD Logic
        let mut splits_simd = noder.find_splits_simd(&lines);

        // Both return Vec<(usize, Coord3D)>
        // Sort both by index, then coordinate
        splits_grid.sort_unstable_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.x.total_cmp(&b.1.x))
                .then(a.1.y.total_cmp(&b.1.y))
        });
        splits_grid.dedup_by(|a, b| a.0 == b.0 && a.1.x == b.1.x && a.1.y == b.1.y);

        splits_simd.sort_unstable_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.x.total_cmp(&b.1.x))
                .then(a.1.y.total_cmp(&b.1.y))
        });
        splits_simd.dedup_by(|a, b| a.0 == b.0 && a.1.x == b.1.x && a.1.y == b.1.y);

        assert_eq!(
            splits_grid.len(),
            splits_simd.len(),
            "Different event counts"
        );

        for (e_g, e_s) in splits_grid.iter().zip(splits_simd.iter()) {
            assert_eq!(e_g.0, e_s.0, "Index mismatch");
            assert!((e_g.1.x - e_s.1.x).abs() < 1e-10 && (e_g.1.y - e_s.1.y).abs() < 1e-10);
        }
    }

    #[test]
    fn grid_trace_records_shared_replacement_loop() {
        let lines = vec![make_line(0.0, 0.0, 2.0, 2.0), make_line(0.0, 2.0, 2.0, 0.0)];
        let (_, _, _, _, _, _, _, splits, truncated) = SnapNoder::new(0.0)
            .with_strategy(NodingStrategy::Grid)
            .node_with_trace(lines, None, usize::MAX)
            .unwrap();

        assert_eq!(splits.len(), 4);
        assert!(splits.iter().all(|split| split.iteration_index == 0));
        assert!(!truncated);
    }

    #[test]
    fn trace_capture_budget_stops_all_floating_capture_growth() {
        let lines = vec![make_line(0.0, 0.0, 2.0, 2.0), make_line(0.0, 2.0, 2.0, 0.0)];
        let noder = SnapNoder::new(0.0).with_strategy(NodingStrategy::Simd);
        let expected = noder.node(lines.clone());
        let (noded, _, work, candidates, cells, globals, grid_candidates, splits, truncated) =
            noder.node_with_trace(lines, None, 0).unwrap();

        assert_eq!(noded, expected);
        assert!(work.exact_intersection_calls > 0);
        assert!(candidates.is_empty());
        assert!(cells.is_empty());
        assert!(globals.is_empty());
        assert!(grid_candidates.is_empty());
        assert!(splits.is_empty());
        assert!(truncated);
    }

    #[test]
    fn simd_scan_observes_cancellation_within_the_poll_interval() {
        let token = CancellationToken::new();
        token.cancel();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };
        let lines = (0..300)
            .map(|y| make_line(0.0, y as f64, 1.0, y as f64))
            .collect::<Vec<_>>();

        assert!(matches!(
            SnapNoder::new(1.0).find_splits_simd_tracked(
                &lines,
                &mut ExecutionWorkTracker::new(Some(&policy), None),
                None,
            ),
            Err(PolygonizeError::Cancelled { stage }) if stage == "candidate_enumeration"
        ));
    }

    #[test]
    fn simd_midflight_cancellation_latency_is_bounded_in_work_items() {
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            ..Default::default()
        };
        let lines = (0..300)
            .map(|y| make_line(0.0, y as f64, 1.0, y as f64))
            .collect::<Vec<_>>();
        let mut stats = NodingWorkStats::default();
        let result = SnapNoder::new(1.0).find_splits_simd_tracked(
            &lines,
            &mut ExecutionWorkTracker::new(Some(&policy), Some(&mut stats))
                .cancel_at_candidate(token, 17),
            None,
        );

        assert!(matches!(
            result,
            Err(PolygonizeError::Cancelled { stage }) if stage == "candidate_enumeration"
        ));
        assert_eq!(
            stats.candidate_pairs,
            crate::options::CANCELLATION_CHECK_INTERVAL
        );
        assert!(stats.candidate_pairs < lines.len() * (lines.len() - 1) / 2);
    }

    #[test]
    fn simd_candidate_sink_streams_in_input_order() {
        let lines = vec![
            make_line(0.0, 0.0, 2.0, 2.0),
            make_line(0.0, 2.0, 2.0, 0.0),
            make_line(10.0, 10.0, 11.0, 11.0),
        ];
        let noder = SnapNoder::new(0.0);
        let soa = SoALines::new(&lines);
        let mut candidates = Vec::new();
        noder
            .visit_candidate_pairs_simd(lines[0], 0, &lines, &soa, None, |candidate| {
                candidates.push(candidate);
                Ok(())
            })
            .unwrap();

        assert_eq!(
            candidates,
            vec![CandidatePair {
                first: 0,
                second: 1
            }]
        );
        let mut splits = Vec::new();
        noder.process_candidate_pair(&lines, candidates[0], None, &mut splits);
        assert_eq!(
            splits,
            vec![
                (0, Coord3D::new(1.0, 1.0, 0.0)),
                (1, Coord3D::new(1.0, 1.0, 0.0))
            ]
        );
    }

    #[test]
    fn shared_exact_path_preserves_simd_grid_z_and_overlap_outcomes() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(4.0, 4.0, 40.0), 1),
            Line3D::new(
                Coord3D::new(0.0, 4.0, 10.0),
                Coord3D::new(4.0, 0.0, 30.0),
                2,
            ),
            Line3D::new(Coord3D::new(0.0, 6.0, 1.0), Coord3D::new(4.0, 6.0, 5.0), 3),
            Line3D::new(Coord3D::new(1.0, 6.0, 7.0), Coord3D::new(3.0, 6.0, 9.0), 4),
        ];
        let noder = SnapNoder::new(0.0);
        let mut grid_splits = UniformGrid::new(&lines).find_splits(&lines, &noder);
        let mut simd_splits = noder.find_splits_simd(&lines);
        let normalize = |splits: &mut Vec<(usize, Coord3D)>| {
            splits.sort_unstable_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then(left.1.x.total_cmp(&right.1.x))
                    .then(left.1.y.total_cmp(&right.1.y))
                    .then(left.1.z.total_cmp(&right.1.z))
            });
            splits.dedup();
        };
        normalize(&mut grid_splits);
        normalize(&mut simd_splits);

        assert_eq!(grid_splits, simd_splits);
        assert!(simd_splits.contains(&(0, Coord3D::new(2.0, 2.0, 20.0))));
        assert!(simd_splits.contains(&(1, Coord3D::new(2.0, 2.0, 20.0))));
    }

    #[test]
    fn floating_candidate_backends_have_conformant_topology_provenance_z_and_errors() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(4.0, 0.0, 0.0), 1),
            Line3D::new(Coord3D::new(4.0, 0.0, 0.0), Coord3D::new(4.0, 4.0, 0.0), 2),
            Line3D::new(Coord3D::new(4.0, 4.0, 0.0), Coord3D::new(0.0, 4.0, 0.0), 3),
            Line3D::new(Coord3D::new(0.0, 4.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 4),
            Line3D::new(
                Coord3D::new(0.0, 0.0, 10.0),
                Coord3D::new(4.0, 4.0, 30.0),
                5,
            ),
            Line3D::new(
                Coord3D::new(0.0, 4.0, 20.0),
                Coord3D::new(4.0, 0.0, 40.0),
                6,
            ),
        ];
        let (simd_noded, grid_noded) = assert_floating_backend_conformance(&lines);
        let options = PolygonizerOptions {
            node_input: false,
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            input_profile_id: Some("floating-candidate-conformance".to_string()),
            ..Default::default()
        };
        let simd_result = polygonize(simd_noded.clone(), &options).unwrap();
        let grid_result = polygonize(grid_noded.clone(), &options).unwrap();
        let simd_fingerprint =
            TopologyFingerprintV1::try_from_result(&simd_result, &options).unwrap();
        let grid_fingerprint =
            TopologyFingerprintV1::try_from_result(&grid_result, &options).unwrap();
        assert_eq!(simd_fingerprint, grid_fingerprint);
        assert!(!simd_result.polygons.is_empty());
        assert!(simd_result
            .polygons
            .iter()
            .all(|polygon| polygon.provenance.is_some()));

        let z_error_options = PolygonizerOptions {
            z: ZOptions {
                policy: ZPolicy::ErrorOnConflict,
                conflict_tolerance: 0.0,
            },
            ..options
        };
        let simd_error = polygonize(simd_noded, &z_error_options).unwrap_err();
        let grid_error = polygonize(grid_noded, &z_error_options).unwrap_err();
        assert_eq!(
            normalize_polygonize_error(&simd_error),
            normalize_polygonize_error(&grid_error)
        );
    }

    #[test]
    fn live_scan_supplies_diagnostics_and_budget_enforcement() {
        let lines = vec![
            make_line(0.0, 0.0, 2.0, 2.0),
            make_line(0.0, 2.0, 2.0, 0.0),
            make_line(1.0, -1.0, 1.0, 3.0),
        ];
        let policy = ExecutionPolicy {
            max_candidate_pairs: Some(1),
            ..Default::default()
        };
        let mut stats = NodingWorkStats::default();
        let result = SnapNoder::new(0.0).find_splits_simd_tracked(
            &lines,
            &mut ExecutionWorkTracker::new(Some(&policy), Some(&mut stats)),
            None,
        );

        assert!(matches!(
            result,
            Err(PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 1,
                observed: 2,
            }) if stage == "candidate_pairs"
        ));
        assert_eq!(stats.candidate_pairs, 2);
        assert_eq!(stats.exact_intersection_calls, 2);
    }

    #[test]
    fn test_scalar_strategy_simple() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 10.0),
            make_line(0.0, 10.0, 10.0, 0.0),
        ];

        let noder = SnapNoder::new(1e-6).with_strategy(NodingStrategy::Scalar);
        let noded = noder.node(lines);

        // Should result in 4 segments meeting at (5,5)
        // (0,0)->(5,5)
        // (5,5)->(10,10)
        // (0,10)->(5,5)
        // (5,5)->(10,0)
        assert_eq!(noded.len(), 4, "Expected 4 lines from simple intersection");

        let center = Coord3D::new(5.0, 5.0, 0.0);
        // Check if any line endpoint is close to center
        let center_hits = noded
            .iter()
            .filter(|l| {
                (l.start.x - center.x).abs() < 1e-6 && (l.start.y - center.y).abs() < 1e-6
                    || (l.end.x - center.x).abs() < 1e-6 && (l.end.y - center.y).abs() < 1e-6
            })
            .count();

        assert_eq!(center_hits, 4, "All 4 lines should touch the center point");
    }

    #[test]
    fn test_grid_work_stats() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 10.0),
            make_line(0.0, 10.0, 10.0, 0.0),
            make_line(20.0, 20.0, 30.0, 20.0),
        ];
        let noder = SnapNoder::new(0.0).with_strategy(NodingStrategy::Grid);

        let (_, _, stats) = noder.node_with_stats(lines);

        assert!(stats.grid_cells > 0);
        assert!(stats.grid_cell_entries > 0);
        assert_eq!(
            stats.candidate_pairs,
            stats.aabb_rejections + stats.exact_intersection_calls
        );
        assert!(stats.exact_intersection_calls >= 1);
        assert_eq!(stats.split_events, 2);
    }

    #[test]
    fn test_grid_nodes_independent_crossings_once() {
        let lines = (0..4)
            .flat_map(|x| {
                (0..4).flat_map(move |y| {
                    let (x, y) = (x as f64 * 2.0, y as f64 * 2.0);
                    [
                        make_line(x, y, x + 1.0, y + 1.0),
                        make_line(x + 1.0, y, x, y + 1.0),
                    ]
                })
            })
            .collect();
        let noder = SnapNoder::new(1e-10).with_strategy(NodingStrategy::Grid);

        let (noded, iterations, _) = noder.node_with_stats(lines);

        assert_eq!(noded.len(), 64);
        assert_eq!(iterations.len(), 2);
        assert_eq!(iterations[0].intersections_found, 32);
        assert_eq!(iterations[0].nodes_added, 32);
        assert_eq!(iterations[1].intersections_found, 0);
    }

    #[test]
    fn test_auto_uses_simd_only_for_dense_split_pairs() {
        let dense: Vec<_> = (0..256)
            .map(|i| {
                let angle = std::f64::consts::PI * i as f64 / 256.0;
                let (sin, cos) = angle.sin_cos();
                make_line(-cos, -sin, cos, sin)
            })
            .collect();
        let skewed: Vec<_> = (0..256)
            .map(|i| {
                let end = i as f64 * 0.0001;
                make_line(0.0, 0.0, end, end + 0.00001)
            })
            .collect();
        let noder = SnapNoder::new(1e-10);

        let oversized_dense: Vec<_> = (0..=AUTO_SIMD_LIMIT)
            .map(|i| {
                let angle = std::f64::consts::PI * i as f64 / AUTO_SIMD_LIMIT as f64;
                let (sin, cos) = angle.sin_cos();
                make_line(-cos, -sin, cos, sin)
            })
            .collect();

        assert!(noder.auto_prefers_simd(&dense));
        assert!(!noder.auto_prefers_simd(&skewed));
        assert!(!noder.auto_prefers_simd(&oversized_dense));
    }

    #[test]
    fn test_pre_snap_inserts_nearby_reference_vertices() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 0.0),
            make_line(5.0, 0.4, 5.0, -0.4),
        ];

        let snapped = SnapNoder::pre_snap_to_reference_vertices(&lines, 0.5);

        assert_eq!(
            snapped[1].start,
            Coord3D::new(5.0, 0.4, 0.0),
            "GEOS-compatible snap tie order"
        );
        assert!(snapped
            .iter()
            .any(|line| line.start == Coord3D::new(5.0, 0.4, 0.0)
                || line.end == Coord3D::new(5.0, 0.4, 0.0)));
        assert!(snapped
            .iter()
            .any(|line| line.start == Coord3D::new(5.0, -0.4, 0.0)
                || line.end == Coord3D::new(5.0, -0.4, 0.0)));
    }

    #[test]
    fn test_pre_snap_moves_nearby_endpoints() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 0.0),
            make_line(10.3, 0.02, 20.0, 0.0),
        ];

        let snapped = SnapNoder::pre_snap_to_reference_vertices(&lines, 0.5);

        assert!(snapped
            .iter()
            .any(|line| line.start == Coord3D::new(10.0, 0.0, 0.0)
                || line.end == Coord3D::new(10.0, 0.0, 0.0)));
        assert!(snapped
            .iter()
            .any(|line| (line.start == Coord3D::new(10.0, 0.0, 0.0)
                && line.end == Coord3D::new(10.3, 0.02, 0.0))
                || (line.start == Coord3D::new(10.3, 0.02, 0.0)
                    && line.end == Coord3D::new(10.0, 0.0, 0.0))));
    }

    #[test]
    fn indexed_pre_snap_matches_linear_candidate_scan() {
        let lines: Vec<_> = (0..100)
            .flat_map(|i| {
                let y = i as f64 * 2.0;
                [
                    make_line(0.0, y, 100.0, y),
                    make_line(50.0, y + 0.25, 50.5, y + 0.75),
                ]
            })
            .collect();

        let (linear, linear_candidates) =
            SnapNoder::pre_snap_impl(&lines, 0.5, false, ZPolicy::InterpolateAlongEdge, None)
                .unwrap();
        let (indexed, indexed_candidates) =
            SnapNoder::pre_snap_impl(&lines, 0.5, true, ZPolicy::InterpolateAlongEdge, None)
                .unwrap();

        assert_eq!(indexed.len(), linear.len());
        for (actual, expected) in indexed.iter().zip(linear) {
            assert_eq!(actual.start, expected.start);
            assert_eq!(actual.end, expected.end);
            assert_eq!(actual.line_id, expected.line_id);
        }
        assert_eq!(linear_candidates, 240_000);
        assert_eq!(indexed_candidates, 1_100);
    }

    #[test]
    fn pre_snap_reference_work_observes_cancellation() {
        let token = CancellationToken::new();
        token.cancel();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 0.0),
            make_line(5.0, 0.1, 5.0, 1.0),
        ];

        assert!(matches!(
            SnapNoder::pre_snap_impl(
                &lines,
                0.5,
                true,
                ZPolicy::InterpolateAlongEdge,
                Some(&policy),
            ),
            Err(PolygonizeError::Cancelled { stage }) if stage == "pre_snap"
        ));
    }

    #[test]
    fn test_check_intersection_direct() {
        let l1 = make_line(0.0, 0.0, 10.0, 10.0);
        let l2 = make_line(0.0, 10.0, 10.0, 0.0);
        let lines = vec![l1, l2];
        let mut events = Vec::new();

        let noder = SnapNoder::new(0.0);
        noder.check_intersection(&lines, 0, 1, &mut events);

        assert_eq!(events.len(), 2);

        let p = events[0].1;
        assert!((p.x - 5.0).abs() < 1e-10);
        assert!((p.y - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_check_intersection_out_of_bounds() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 10.0),
            make_line(0.0, 10.0, 10.0, 0.0),
        ];
        let mut events = Vec::new();
        let noder = SnapNoder::new(0.0);

        // i is out of bounds
        noder.check_intersection(&lines, 2, 0, &mut events);
        assert!(
            events.is_empty(),
            "Events should be empty when i is out of bounds"
        );

        // j is out of bounds
        noder.check_intersection(&lines, 0, 2, &mut events);
        assert!(
            events.is_empty(),
            "Events should be empty when j is out of bounds"
        );

        // Both are out of bounds
        noder.check_intersection(&lines, 2, 3, &mut events);
        assert!(
            events.is_empty(),
            "Events should be empty when both are out of bounds"
        );
    }

    #[test]
    fn test_check_intersection_identical_lines() {
        let l1 = make_line(0.0, 0.0, 10.0, 10.0);
        let lines = vec![l1, l1];
        let mut events = Vec::new();
        let noder = SnapNoder::new(0.0);

        noder.check_intersection(&lines, 0, 1, &mut events);

        // Identical lines will be collinear and overlap exactly.
        // Expecting endpoints to be generated as intersection points.
        // It returns endpoints of overlap, excluding endpoints if they match original start/end perfectly.
        // Wait, handle_intersection logic for Collinear:
        // if p != l1_start_2d && p != l1_end_2d { ... }
        // If identical, the overlap is the whole line. The overlap endpoints ARE the line endpoints.
        // Therefore, handle_intersection will filter them out because p == l1_start_2d || p == l1_end_2d.
        // Let's verify events is empty.
        assert!(
            events.is_empty(),
            "Identical lines should yield no internal split events"
        );
    }

    #[test]
    fn test_check_intersection_overlapping_lines() {
        // l1: (0,0) to (10,10)
        // l2: (5,5) to (15,15)
        let l1 = make_line(0.0, 0.0, 10.0, 10.0);
        let l2 = make_line(5.0, 5.0, 15.0, 15.0);
        let lines = vec![l1, l2];
        let mut events = Vec::new();
        let noder = SnapNoder::new(0.0);

        noder.check_intersection(&lines, 0, 1, &mut events);

        // Overlap is (5,5) to (10,10).
        // For l1, (10,10) is an endpoint, so it shouldn't be added to l1's events. (5,5) is internal, should be added.
        // For l2, (5,5) is an endpoint, so it shouldn't be added to l2's events. (10,10) is internal, should be added.
        // Let's see how many events are generated.
        // It should be 2 events total:
        // Event for i=0 (l1) at (5,5)
        // Event for j=1 (l2) at (10,10)
        assert_eq!(
            events.len(),
            2,
            "Overlapping lines should yield internal split events for both lines"
        );

        let has_5_5 = events
            .iter()
            .any(|(idx, pt)| *idx == 0 && (pt.x - 5.0).abs() < 1e-10 && (pt.y - 5.0).abs() < 1e-10);
        let has_10_10 = events.iter().any(|(idx, pt)| {
            *idx == 1 && (pt.x - 10.0).abs() < 1e-10 && (pt.y - 10.0).abs() < 1e-10
        });

        assert!(has_5_5, "Expected split event for l1 at (5,5)");
        assert!(has_10_10, "Expected split event for l2 at (10,10)");
    }

    #[test]
    fn test_check_intersection_disjoint_lines() {
        let l1 = make_line(0.0, 0.0, 10.0, 10.0);
        let l2 = make_line(0.0, 10.0, 5.0, 15.0);
        let lines = vec![l1, l2];
        let mut events = Vec::new();
        let noder = SnapNoder::new(0.0);

        noder.check_intersection(&lines, 0, 1, &mut events);

        // No intersection, events should be empty
        assert!(
            events.is_empty(),
            "Disjoint lines should yield no intersection events"
        );
    }
}

#[cfg(test)]
mod tests_geos_compat {
    use super::*;
    use crate::options::SnapStrategy;
    use crate::types::Coord3D;

    #[test]
    fn test_geos_compat_rounding() {
        let noder = SnapNoder::new(1.0).with_snap_strategy(SnapStrategy::GeosCompat);

        assert_eq!(noder.snap(Coord3D::new(0.5, 0.0, 0.0)).x, 1.0);
        assert_eq!(noder.snap(Coord3D::new(1.5, 0.0, 0.0)).x, 2.0);
        assert_eq!(noder.snap(Coord3D::new(2.5, 0.0, 0.0)).x, 3.0);

        assert_eq!(noder.snap(Coord3D::new(-0.5, 0.0, 0.0)).x, -1.0);
        assert_eq!(noder.snap(Coord3D::new(-1.5, 0.0, 0.0)).x, -2.0);
        assert_eq!(noder.snap(Coord3D::new(-2.5, 0.0, 0.0)).x, -3.0);

        assert_eq!(noder.snap(Coord3D::new(0.25, 0.0, 0.0)).x, 0.0);
        assert_eq!(noder.snap(Coord3D::new(0.75, 0.0, 0.0)).x, 1.0);
        assert_eq!(noder.snap(Coord3D::new(-0.25, 0.0, 0.0)).x, 0.0);
        assert_eq!(noder.snap(Coord3D::new(-0.75, 0.0, 0.0)).x, -1.0);
    }
}
