use geo::{Line, Coord};
use std::cmp::Ordering;
use geo::algorithm::line_intersection::LineIntersection;
use crate::utils::soa::SoALines;
use std::collections::HashMap;
use crate::noding::grid::UniformGrid;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

pub struct SnapNoder {
    pub grid_size: f64,
    pub max_iter: usize,
}

impl SnapNoder {
    pub fn new(grid_size: f64) -> Self {
        Self { grid_size, max_iter: 10 }
    }

    pub fn node(&self, mut lines: Vec<Line<f64>>) -> Vec<Line<f64>> {
        // 1. Initial Snap of endpoints
        for line in &mut lines {
            line.start = self.snap(line.start);
            line.end = self.snap(line.end);
        }

        // Remove degenerates
        lines.retain(|l| l.start != l.end);

        // Normalize and dedup initial input
        self.normalize_and_dedup(&mut lines);

        // 2. Iterative Noding
        for _iter in 0..self.max_iter {
            let splits = if lines.len() < 256 {
                // STRATEGY A: Small Input -> SIMD Brute Force
                self.find_splits_simd(&lines)
            } else {
                // STRATEGY B: Large Input -> Uniform Grid
                let grid = UniformGrid::new(&lines);
                grid.find_splits(&lines, self)
            };

            if splits.is_empty() {
                break;
            }

            // Apply splits
            let mut new_lines = Vec::with_capacity(lines.len() * 2);
            for (i, line) in lines.iter().enumerate() {
                if let Some(splits) = splits.get(&i) {
                    let mut points = splits.clone();
                    // Add endpoints
                    points.push(line.start);
                    points.push(line.end);

                    // Sort by distance from start
                    let start = line.start;
                    points.sort_by(|a, b| {
                        let da = (a.x - start.x).powi(2) + (a.y - start.y).powi(2);
                        let db = (b.x - start.x).powi(2) + (b.y - start.y).powi(2);
                        da.partial_cmp(&db).unwrap_or(Ordering::Equal)
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
            lines = new_lines;
        }

        lines
    }

    fn normalize_and_dedup(&self, lines: &mut Vec<Line<f64>>) {
        for segment in lines.iter_mut() {
            if segment.start.x > segment.end.x ||
               ((segment.start.x - segment.end.x).abs() < 1e-12 && segment.start.y > segment.end.y) {
                 let temp = segment.start;
                 segment.start = segment.end;
                 segment.end = temp;
            }
        }
        lines.sort_by(|a, b| {
             let sa = (a.start.x, a.start.y, a.end.x, a.end.y);
             let sb = (b.start.x, b.start.y, b.end.x, b.end.y);
             sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
        });
        lines.dedup();
    }

    pub(crate) fn snap(&self, c: Coord<f64>) -> Coord<f64> {
        if self.grid_size == 0.0 { return c; }
        Coord {
            x: (c.x / self.grid_size).round() * self.grid_size,
            y: (c.y / self.grid_size).round() * self.grid_size,
        }
    }

    fn find_splits_simd(&self, lines: &[Line<f64>]) -> HashMap<usize, Vec<Coord<f64>>> {
        let soa = SoALines::new(lines);

        #[cfg(feature = "parallel")]
        {
            // Parallel execution: each thread processes a subset of query lines
            // and returns a list of split events (line_index, point).
            let all_splits: Vec<(usize, Coord<f64>)> = lines.par_iter().enumerate()
                .flat_map(|(i, &query_line)| {
                    self.check_intersection_simd(query_line, i, lines, &soa)
                })
                .collect();

            // Aggregate results into HashMap
            let mut splits = HashMap::new();
            for (idx, pt) in all_splits {
                splits.entry(idx).or_insert_with(Vec::new).push(pt);
            }
            splits
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut splits = HashMap::new();
            for (i, &query_line) in lines.iter().enumerate() {
                let events = self.check_intersection_simd(query_line, i, lines, &soa);
                for (idx, pt) in events {
                     splits.entry(idx).or_insert_with(Vec::new).push(pt);
                }
            }
            splits
        }
    }

    // Helper to check one line against all others using SIMD SoA
    #[inline]
    fn check_intersection_simd(&self, query_line: Line<f64>, i: usize, lines: &[Line<f64>], soa: &SoALines) -> Vec<(usize, Coord<f64>)> {
        let mut events = Vec::new();
        // Start block to avoid duplicate checks (j > i)
        // We start checking at index i+1.
        // The SoA batching index `j` steps by 4.
        // We want `j` such that the batch covers indices > i.
        // Ideally start `j` at `(i + 1) / 4 * 4`.
        let start_block = (i + 1) / 4 * 4;

        // Check unaligned start if necessary (handled by the loop if we are careful, or explicit loop)
        // The current loop below starts at `start_block`.
        // If `start_block` is less than `i+1`, we might re-check `i`.
        // Example: i=5. i+1=6. start_block = 6/4*4 = 4.
        // j=4 covers 4,5,6,7. 4<=5, 5<=5. We must skip those.
        // The loop below has `if target_idx <= i { continue; }` which handles this.

        for j in (start_block..soa.len()).step_by(4) {
            let mask = soa.intersects_bbox_batch(query_line, j);

            if mask != 0 {
                for k in 0..4 {
                    if (mask & (1 << k)) != 0 {
                        let target_idx = j + k;
                        if target_idx >= lines.len() { continue; }
                        if target_idx <= i { continue; } // Enforce i < j

                        let target_line = lines[target_idx];
                        if let Some(res) = geo::algorithm::line_intersection::line_intersection(query_line, target_line) {
                             // We can't update a shared HashMap here.
                             // Return the intersection events for the caller to aggregate.
                             self.collect_intersection_events(res, i, target_idx, query_line, target_line, &mut events);
                        }
                    }
                }
            }
        }
        events
    }

    // Helper to collect events into a local vector instead of HashMap
    fn collect_intersection_events(&self,
        res: LineIntersection<f64>,
        i: usize,
        j: usize,
        l1: Line<f64>,
        l2: Line<f64>,
        events: &mut Vec<(usize, Coord<f64>)>
    ) {
         match res {
            LineIntersection::SinglePoint { intersection: pt, .. } => {
                let snapped = self.snap(pt);
                if snapped != l1.start && snapped != l1.end {
                    events.push((i, snapped));
                }
                if snapped != l2.start && snapped != l2.end {
                    events.push((j, snapped));
                }
            },
            LineIntersection::Collinear { intersection: overlap } => {
                let p1 = self.snap(overlap.start);
                let p2 = self.snap(overlap.end);
                for p in [p1, p2] {
                     if p != l1.start && p != l1.end {
                         events.push((i, p));
                     }
                     if p != l2.start && p != l2.end {
                         events.push((j, p));
                     }
                }
            }
         }
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
            lines.push(Line::new(Coord{x:x1, y:y1}, Coord{x:x2, y:y2}));
        }

        // Add some guaranteed intersections
        lines.push(Line::new(Coord{x:0.0, y:0.0}, Coord{x:10.0, y:10.0}));
        lines.push(Line::new(Coord{x:0.0, y:10.0}, Coord{x:10.0, y:0.0}));

        let noder = SnapNoder::new(0.001);

        // Grid Logic (Force use by calling directly)
        let grid = UniformGrid::new(&lines);
        let splits_grid = grid.find_splits(&lines, &noder);

        // SIMD Logic
        let splits_simd = noder.find_splits_simd(&lines);

        // Compare counts
        // Note: splits_grid might return slightly different results if ownership logic differs slightly
        // from what brute force would capture, BUT for "find_splits" they should be effectively same set of points per line.
        // However, brute force finds ALL intersections. Grid finds intersections and assigns them via ownership.
        // Wait, ownership check prevents double reporting, but for a given line index, the set of split points should be identical.

        // Actually, find_splits_simd (brute force) also duplicates?
        // Let's check collect_intersection_events.
        // It pushes to . Then  HashMap collects them.
        // If (i, j) intersect at P.
        // i gets P. j gets P.
        // Grid logic:
        // iterate cells. find (i, j). check ownership. if owned, i gets P, j gets P.
        // if not owned, ignored (will be owned by another cell).
        // So the result should be identical.

        // One caveat: floating point differences in ownership check vs brute force?
        // Usually shouldn't matter for "set of points".

        // Let's compare lengths first.
        assert_eq!(splits_grid.len(), splits_simd.len(), "Different number of lines with splits");

        for (idx, points_grid) in &splits_grid {
            let points_simd = splits_simd.get(idx).expect("Index missing in SIMD splits");

            // Sort points to ensure order independence
            let mut p_grid = points_grid.clone();
            p_grid.sort_by(|a,b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
            p_grid.dedup();

            let mut p_simd = points_simd.clone();
            p_simd.sort_by(|a,b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
            p_simd.dedup();

            // Allow small epsilon diff? Or exact?
            // SnapNoder snaps to grid, so they should be exact if grid_size is same.
            // SnapNoder::snap() is used in both.

            assert_eq!(p_grid.len(), p_simd.len(), "Different number of points for line {}", idx);

            for (p_g, p_s) in p_grid.iter().zip(p_simd.iter()) {
                assert!((p_g.x - p_s.x).abs() < 1e-10 && (p_g.y - p_s.y).abs() < 1e-10,
                        "Point mismatch: {:?} vs {:?}", p_g, p_s);
            }
        }
    }
}
