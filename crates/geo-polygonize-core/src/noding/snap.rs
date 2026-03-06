use crate::noding::grid::UniformGrid;
use crate::types::{Coord3D, Line3D};
use crate::utils::soa::SoALines;
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};
use geo::Coord;
use wide::f64x4;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodingStrategy {
    Auto,
    Scalar, // Deprecated/Fallback (Mapped to SIMD/Grid depending on impl)
    Simd,
    Grid,
}

pub struct SnapNoder {
    pub grid_size: f64,
    pub max_iter: usize,
    pub strategy: NodingStrategy,
}

impl SnapNoder {
    pub fn new(grid_size: f64) -> Self {
        Self {
            grid_size,
            max_iter: 10,
            strategy: NodingStrategy::Auto,
        }
    }

    pub fn with_strategy(mut self, strategy: NodingStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn node(&self, mut lines: Vec<Line3D>) -> Vec<Line3D> {
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
        let mut new_lines = Vec::new();
        for _iter in 0..self.max_iter {
            let use_grid = match self.strategy {
                NodingStrategy::Auto => lines.len() >= 256,
                NodingStrategy::Grid => true,
                NodingStrategy::Simd => false,
                NodingStrategy::Scalar => false, // Fallback to SIMD logic which handles scalar internally
            };

            let mut events = if !use_grid {
                // STRATEGY A: Small Input -> SIMD Brute Force
                self.find_splits_simd(&lines)
            } else {
                // STRATEGY B: Large Input -> Uniform Grid
                let grid = UniformGrid::new(&lines);
                grid.find_splits(&lines, self)
            };

            if events.is_empty() {
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

                // Sort by distance from start
                let start = line.start;
                points.sort_unstable_by(|a, b| {
                    let da = (a.x - start.x).powi(2) + (a.y - start.y).powi(2);
                    let db = (b.x - start.x).powi(2) + (b.y - start.y).powi(2);
                    da.total_cmp(&db)
                });

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

            if should_bail_early {
                break;
            }
        }

        lines
    }

    fn normalize_and_dedup(&self, lines: &mut Vec<Line3D>) {
        // Filter out invalid lines (NaN or infinite coordinates)
        lines.retain(|l| {
            l.start.x.is_finite()
                && l.start.y.is_finite()
                && l.end.x.is_finite()
                && l.end.y.is_finite()
        });

        for segment in lines.iter_mut() {
            if segment.start.x > segment.end.x
                || ((segment.start.x - segment.end.x).abs() < 1e-12
                    && segment.start.y > segment.end.y)
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
        });
        lines.dedup();
    }

    pub(crate) fn snap(&self, c: Coord3D) -> Coord3D {
        if self.grid_size == 0.0 {
            return c;
        }
        Coord3D {
            x: (c.x / self.grid_size).round() * self.grid_size,
            y: (c.y / self.grid_size).round() * self.grid_size,
            z: c.z, // Keep Z unchanged
        }
    }

    // Interpolates Z value for a point (px, py) assumed to be on the line segment
    fn interpolate_z(&self, p: Coord<f64>, line: Line3D) -> f64 {
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
        splits_grid.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| (a.1.x, a.1.y).partial_cmp(&(b.1.x, b.1.y)).unwrap())
        });
        splits_grid.dedup_by(|a, b| a.0 == b.0 && a.1.x == b.1.x && a.1.y == b.1.y);

        splits_simd.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| (a.1.x, a.1.y).partial_cmp(&(b.1.x, b.1.y)).unwrap())
        });
        splits_simd.dedup_by(|a, b| a.0 == b.0 && a.1.x == b.1.x && a.1.y == b.1.y);

        assert_eq!(
            splits_grid.len(),
            splits_simd.len(),
            "Different event counts"
        );

        for (e_g, e_s) in splits_grid.iter().zip(splits_simd.iter()) {
            assert_eq!(e_g.0, e_s.0, "Index mismatch");
            assert!(
                (e_g.1.x - e_s.1.x).abs() < 1e-10 && (e_g.1.y - e_s.1.y).abs() < 1e-10,
                "Point mismatch at index {}: {:?} vs {:?}",
                e_g.0,
                e_g.1,
                e_s.1
            );
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
}
