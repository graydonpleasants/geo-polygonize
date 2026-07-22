use crate::diagnostics::{NodingIterationStats, NodingWorkStats};
use crate::index::{IndexedEnvelope, RStarBackend};
use crate::noding::grid::UniformGrid;
use crate::options::{ExecutionPolicy, SnapStrategy, ZPolicy};
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
        self.node_impl(lines, None, None, None)
            .expect("unlimited noding cannot fail")
    }

    pub(crate) fn node_with_stats(
        &self,
        lines: Vec<Line3D>,
    ) -> (Vec<Line3D>, Vec<NodingIterationStats>, NodingWorkStats) {
        let mut stats = Vec::new();
        let mut work_stats = NodingWorkStats::default();
        let lines = self
            .node_impl(lines, Some(&mut stats), Some(&mut work_stats), None)
            .expect("unlimited noding cannot fail");
        (lines, stats, work_stats)
    }

    pub(crate) fn node_with_execution_policy(
        &self,
        lines: Vec<Line3D>,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<Vec<Line3D>> {
        self.node_impl(lines, None, None, Some(execution_policy))
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
        )?;
        Ok((lines, stats, work_stats))
    }

    fn node_impl(
        &self,
        mut lines: Vec<Line3D>,
        mut stats: Option<&mut Vec<NodingIterationStats>>,
        mut work_stats: Option<&mut NodingWorkStats>,
        execution_policy: Option<&ExecutionPolicy>,
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
        let mut total_work = NodingWorkStats::default();
        let mut split_events = 0;
        for iteration_index in 0..self.max_iter {
            if let Some(execution_policy) = execution_policy {
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
                let measured_work = (work_stats.is_some() || execution_policy.is_some())
                    .then(|| Self::measure_simd_work(&lines));
                if let Some(measured_work) = measured_work {
                    if let Some(execution_policy) = execution_policy {
                        total_work.merge(measured_work.clone());
                        execution_policy.check_noding_work(
                            total_work.candidate_pairs,
                            total_work.exact_intersection_calls,
                        )?;
                    }
                    if let Some(work_stats) = work_stats.as_deref_mut() {
                        work_stats.merge(measured_work);
                    }
                }
                self.find_splits_simd(&lines)
            } else {
                // STRATEGY B: Large Input -> Uniform Grid
                let grid = UniformGrid::new(&lines);
                let measured_work = (work_stats.is_some() || execution_policy.is_some())
                    .then(|| grid.measure_work(&lines));
                if let Some(measured_work) = measured_work {
                    if let Some(execution_policy) = execution_policy {
                        total_work.merge(measured_work.clone());
                        execution_policy.check_noding_work(
                            total_work.candidate_pairs,
                            total_work.exact_intersection_calls,
                        )?;
                    }
                    if let Some(work_stats) = work_stats.as_deref_mut() {
                        work_stats.merge(measured_work);
                    }
                }
                grid.find_splits(&lines, self)
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
            new_lines.reserve(lines.len() + events.len());

            let mut event_idx = 0;
            let mut src_idx = 0;
            // Buffer to reuse for collecting points on the current split line.
            let mut points = Vec::new();

            while event_idx < events.len() {
                let line_idx = events[event_idx].0;

                // Copy untouched lines directly.
                if src_idx < line_idx {
                    new_lines.extend_from_slice(&lines[src_idx..line_idx]);
                }

                points.clear();
                while event_idx < events.len() && events[event_idx].0 == line_idx {
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
                    points.sort_unstable_by(|a, b| {
                        let ta = ((a.x - start.x) * dx + (a.y - start.y) * dy) / len_sq;
                        let tb = ((b.x - start.x) * dx + (b.y - start.y) * dy) / len_sq;
                        ta.total_cmp(&tb)
                    });
                } else {
                    // Fallback to sort by X, then Y if segment is a zero-length point
                    points.sort_unstable_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
                }

                // Dedup by 2D coordinates
                points.dedup_by(|a, b| a.x == b.x && a.y == b.y);

                // Create replacement segments for the split line.
                for w in points.windows(2) {
                    let p0 = w[0];
                    let p1 = w[1];
                    // Check 2D equality
                    if p0.x != p1.x || p0.y != p1.y {
                        new_lines.push(Line3D::new(p0, p1, line.line_id));
                    }
                }

                src_idx = line_idx + 1;
            }

            // Copy any untouched trailing lines.
            if src_idx < lines.len() {
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

    fn measure_simd_work(lines: &[Line3D]) -> NodingWorkStats {
        let mut stats = NodingWorkStats::default();
        for (i, left) in lines.iter().enumerate() {
            for right in &lines[i + 1..] {
                stats.candidate_pairs += 1;
                let overlaps = left.start.x.max(left.end.x) >= right.start.x.min(right.end.x)
                    && left.start.x.min(left.end.x) <= right.start.x.max(right.end.x)
                    && left.start.y.max(left.end.y) >= right.start.y.min(right.end.y)
                    && left.start.y.min(left.end.y) <= right.start.y.max(right.end.y);
                if overlaps {
                    stats.exact_intersection_calls += 1;
                } else {
                    stats.aabb_rejections += 1;
                }
            }
        }
        stats
    }

    pub fn pre_snap_to_reference_vertices(lines: &[Line3D], tolerance: f64) -> Vec<Line3D> {
        Self::pre_snap_impl(lines, tolerance, true, ZPolicy::InterpolateAlongEdge).0
    }

    pub(crate) fn pre_snap_to_reference_vertices_with_stats(
        lines: &[Line3D],
        tolerance: f64,
        z_policy: ZPolicy,
    ) -> (Vec<Line3D>, usize) {
        Self::pre_snap_impl(lines, tolerance, true, z_policy)
    }

    fn pre_snap_impl(
        lines: &[Line3D],
        tolerance: f64,
        use_index: bool,
        z_policy: ZPolicy,
    ) -> (Vec<Line3D>, usize) {
        if lines.is_empty() || tolerance <= 0.0 {
            return (lines.to_vec(), 0);
        }

        let mut reference_vertices: Vec<Coord3D> = SnapNoder::new(0.0)
            .node(lines.to_vec())
            .into_iter()
            .flat_map(|line| [line.start, line.end])
            .collect();

        reference_vertices.sort_unstable_by(|a, b| {
            a.x.total_cmp(&b.x)
                .then(a.y.total_cmp(&b.y))
                .then(a.z.total_cmp(&b.z))
        });
        reference_vertices.dedup_by(|a, b| a.x == b.x && a.y == b.y);

        let vertex_index = use_index.then(|| {
            RStarBackend::new(
                reference_vertices
                    .iter()
                    .enumerate()
                    .map(|(index, vertex)| IndexedEnvelope {
                        aabb: AABB::from_corners([vertex.x, vertex.y], [vertex.x, vertex.y]),
                        index,
                    })
                    .collect(),
            )
        });

        let tolerance_sq = tolerance * tolerance;
        let mut snapped = Vec::with_capacity(lines.len());
        let mut points = Vec::new();
        let mut vertex_candidates = 0;
        let z_resolver = SnapNoder::new(0.0).with_z_policy(z_policy);

        for &line in lines {
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
                let mut consider_vertex = |vertex: Coord3D| {
                    vertex_candidates += 1;
                    let vx = vertex.x - start.x;
                    let vy = vertex.y - start.y;
                    let t = (vx * dx + vy * dy) / len_sq;
                    if !(0.0..=1.0).contains(&t) {
                        return;
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
                        consider_vertex(reference_vertices[idx]);
                    }
                } else {
                    for &vertex in &reference_vertices {
                        consider_vertex(vertex);
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

        (snapped, vertex_candidates)
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
                    .flat_map(|(i, &query_line)| {
                        self.check_intersection_simd(query_line, i, lines, &soa)
                    })
                    .collect()
            } else {
                // Sequential fallback
                lines
                    .iter()
                    .enumerate()
                    .flat_map(|(i, &query_line)| {
                        self.check_intersection_simd(query_line, i, lines, &soa)
                    })
                    .collect()
            }
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut splits = Vec::new();
            for (i, &query_line) in lines.iter().enumerate() {
                let events = self.check_intersection_simd(query_line, i, lines, &soa);
                splits.extend(events);
            }
            splits
        }
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

    // Helper to check one line against all others using SIMD SoA
    #[allow(clippy::manual_div_ceil)]
    #[inline]
    fn check_intersection_simd(
        &self,
        query_line: Line3D,
        i: usize,
        lines: &[Line3D],
        soa: &SoALines,
    ) -> Vec<(usize, Coord3D)> {
        let mut events = Vec::new();
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

            if q_max_x >= t_min_x && q_min_x <= t_max_x && q_max_y >= t_min_y && q_min_y <= t_max_y
            {
                self.process_intersection(query_line, target_line, i, j, |idx, pt| {
                    events.push((idx, pt))
                });
            }
        }

        // Pre-calculate query BBox splats
        let q_min_x = f64x4::splat(query_line.start.x.min(query_line.end.x));
        let q_max_x = f64x4::splat(query_line.start.x.max(query_line.end.x));
        let q_min_y = f64x4::splat(query_line.start.y.min(query_line.end.y));
        let q_max_y = f64x4::splat(query_line.start.y.max(query_line.end.y));

        for j in (start_block..soa.len()).step_by(4) {
            let mask = soa.intersects_bbox_batch_splatted(q_min_x, q_max_x, q_min_y, q_max_y, j);

            if mask != 0 {
                for k in 0..4 {
                    if (mask & (1 << k)) != 0 {
                        let target_idx = j + k;
                        if target_idx >= lines.len() {
                            continue;
                        }
                        if target_idx <= i {
                            continue;
                        } // Enforce i < j

                        let target_line = lines[target_idx];
                        self.process_intersection(
                            query_line,
                            target_line,
                            i,
                            target_idx,
                            |idx, pt| events.push((idx, pt)),
                        );
                    }
                }
            }
        }
        events
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
    use rand::Rng;

    fn make_line(x1: f64, y1: f64, x2: f64, y2: f64) -> Line3D {
        Line3D::new(Coord3D::new(x1, y1, 0.0), Coord3D::new(x2, y2, 0.0), 0)
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
            SnapNoder::pre_snap_impl(&lines, 0.5, false, ZPolicy::InterpolateAlongEdge);
        let (indexed, indexed_candidates) =
            SnapNoder::pre_snap_impl(&lines, 0.5, true, ZPolicy::InterpolateAlongEdge);

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
