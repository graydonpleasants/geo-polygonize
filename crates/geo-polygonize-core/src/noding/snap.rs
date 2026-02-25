use crate::noding::grid::UniformGrid;
use crate::utils::soa::SoALines;
use geo::algorithm::line_intersection::LineIntersection;
use geo::{Coord, Line};
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

    pub fn node(&self, mut lines: Vec<Line<f64>>) -> Vec<Line<f64>> {
        // 1. Initial Snap of endpoints
        for line in &mut lines {
            line.start = self.snap(line.start);
            line.end = self.snap(line.end);
        }

        // Remove degenerates and invalid lines
        lines.retain(|l| {
            l.start != l.end
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

            // Sort events by line index to allow linear scan
            events.sort_unstable_by_key(|e| e.0);

            // Apply splits
            new_lines.clear();
            new_lines.reserve(lines.len() * 2);

            let mut event_idx = 0;
            // Buffer to reuse for collecting points
            let mut points = Vec::new();

            for (i, line) in lines.iter().enumerate() {
                points.clear();

                // Collect all split points for line i
                while event_idx < events.len() && events[event_idx].0 == i {
                    points.push(events[event_idx].1);
                    event_idx += 1;
                }

                if !points.is_empty() {
                    // Add endpoints
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

                    points.dedup();

                    // Create segments
                    for w in points.windows(2) {
                        let p0 = w[0];
                        let p1 = w[1];
                        if p0 != p1 {
                            new_lines.push(Line::new(p0, p1));
                        }
                    }
                } else {
                    new_lines.push(*line);
                }
            }

            self.normalize_and_dedup(&mut new_lines);
            std::mem::swap(&mut lines, &mut new_lines);
        }

        lines
    }

    fn normalize_and_dedup(&self, lines: &mut Vec<Line<f64>>) {
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

    pub(crate) fn snap(&self, c: Coord<f64>) -> Coord<f64> {
        if self.grid_size == 0.0 {
            return c;
        }
        Coord {
            x: (c.x / self.grid_size).round() * self.grid_size,
            y: (c.y / self.grid_size).round() * self.grid_size,
        }
    }

    #[inline]
    pub(crate) fn handle_intersection<F>(
        &self,
        res: LineIntersection<f64>,
        i: usize,
        j: usize,
        l1: Line<f64>,
        l2: Line<f64>,
        mut handler: F,
    ) where
        F: FnMut(usize, Coord<f64>),
    {
        match res {
            LineIntersection::SinglePoint {
                intersection: pt, ..
            } => {
                let snapped = self.snap(pt);
                if snapped != l1.start && snapped != l1.end {
                    handler(i, snapped);
                }
                if snapped != l2.start && snapped != l2.end {
                    handler(j, snapped);
                }
            }
            LineIntersection::Collinear {
                intersection: overlap,
            } => {
                let p1 = self.snap(overlap.start);
                let p2 = self.snap(overlap.end);
                for p in [p1, p2] {
                    if p != l1.start && p != l1.end {
                        handler(i, p);
                    }
                    if p != l2.start && p != l2.end {
                        handler(j, p);
                    }
                }
            }
        }
    }

    fn find_splits_simd(&self, lines: &[Line<f64>]) -> Vec<(usize, Coord<f64>)> {
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
        l1: Line<f64>,
        l2: Line<f64>,
        i: usize,
        j: usize,
        handler: F,
    ) where
        F: FnMut(usize, Coord<f64>),
    {
        if let Some(res) = geo::algorithm::line_intersection::line_intersection(l1, l2) {
            self.handle_intersection(res, i, j, l1, l2, handler);
        }
    }

    // Helper to check one line against all others using SIMD SoA
    #[allow(clippy::manual_div_ceil)]
    #[inline]
    fn check_intersection_simd(
        &self,
        query_line: Line<f64>,
        i: usize,
        lines: &[Line<f64>],
        soa: &SoALines,
    ) -> Vec<(usize, Coord<f64>)> {
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
        lines: &[Line<f64>],
        i: usize,
        j: usize,
        events: &mut Vec<(usize, Coord<f64>)>,
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
            lines.push(Line::new(Coord { x: x1, y: y1 }, Coord { x: x2, y: y2 }));
        }

        // Add some guaranteed intersections
        lines.push(Line::new(
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
        ));
        lines.push(Line::new(
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 10.0, y: 0.0 },
        ));

        let noder = SnapNoder::new(0.001);

        // Grid Logic (Force use by calling directly)
        let grid = UniformGrid::new(&lines);
        let mut splits_grid = grid.find_splits(&lines, &noder);

        // SIMD Logic
        let mut splits_simd = noder.find_splits_simd(&lines);

        // Both return Vec<(usize, Coord)>
        // Sort both by index, then coordinate
        splits_grid.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| (a.1.x, a.1.y).partial_cmp(&(b.1.x, b.1.y)).unwrap())
        });
        splits_grid.dedup(); // Remove duplicate events if any

        splits_simd.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| (a.1.x, a.1.y).partial_cmp(&(b.1.x, b.1.y)).unwrap())
        });
        splits_simd.dedup();

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
        let mut lines = Vec::new();
        // Intersection at (5, 5)
        lines.push(Line::new(
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
        ));
        lines.push(Line::new(
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 10.0, y: 0.0 },
        ));

        let noder = SnapNoder::new(1e-6).with_strategy(NodingStrategy::Scalar);
        let noded = noder.node(lines);

        // Should result in 4 segments meeting at (5,5)
        // (0,0)->(5,5)
        // (5,5)->(10,10)
        // (0,10)->(5,5)
        // (5,5)->(10,0)
        assert_eq!(noded.len(), 4, "Expected 4 lines from simple intersection");

        let center = Coord { x: 5.0, y: 5.0 };
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
        let l1 = Line::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 });
        let l2 = Line::new(Coord { x: 0.0, y: 10.0 }, Coord { x: 10.0, y: 0.0 });
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
