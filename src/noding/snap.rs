use geo::{Line, Coord};
use rstar::{RTree, RTreeObject, AABB};
use std::cmp::Ordering;
use geo::algorithm::line_intersection::LineIntersection;
use crate::utils::soa::SoALines;
use std::collections::HashMap;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[derive(Clone, Copy, Debug)]
struct IndexedLine {
    line: Line<f64>,
    index: usize,
}

impl RTreeObject for IndexedLine {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        let p1 = self.line.start;
        let p2 = self.line.end;
        AABB::from_corners(
            [p1.x.min(p2.x), p1.y.min(p2.y)],
            [p1.x.max(p2.x), p1.y.max(p2.y)]
        )
    }
}

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

        // 2. Iterative Noding
        for _ in 0..self.max_iter {
            // Check for intersections
            // If Parallel: Use SIMD for < 4096 (brute force scales well with cores)
            // If Sequential: Use SIMD for < 128 (brute force gets slow quickly)
            #[cfg(feature = "parallel")]
            let threshold = 4096;
            #[cfg(not(feature = "parallel"))]
            let threshold = 128;

            let split_map = if lines.len() < threshold {
                self.find_splits_simd(&lines)
            } else {
                self.find_splits(&lines)
            };

            if split_map.is_empty() {
                break;
            }

            // Apply splits
            let mut new_lines = Vec::with_capacity(lines.len() * 2);
            for (i, line) in lines.iter().enumerate() {
                if let Some(splits) = split_map.get(&i) {
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

            // Deduplicate segments and normalize direction
             for segment in &mut new_lines {
                if segment.start.x > segment.end.x ||
                   ((segment.start.x - segment.end.x).abs() < 1e-12 && segment.start.y > segment.end.y) {
                     let temp = segment.start;
                     segment.start = segment.end;
                     segment.end = temp;
                }
            }
            new_lines.sort_by(|a, b| {
                 let sa = (a.start.x, a.start.y, a.end.x, a.end.y);
                 let sb = (b.start.x, b.start.y, b.end.x, b.end.y);
                 sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
            });
            new_lines.dedup();

            lines = new_lines;
        }

        lines
    }

    fn snap(&self, c: Coord<f64>) -> Coord<f64> {
        if self.grid_size == 0.0 { return c; }
        Coord {
            x: (c.x / self.grid_size).round() * self.grid_size,
            y: (c.y / self.grid_size).round() * self.grid_size,
        }
    }

    fn find_splits(&self, lines: &[Line<f64>]) -> HashMap<usize, Vec<Coord<f64>>> {
        let mut splits = HashMap::new();

        let indexed: Vec<IndexedLine> = lines.iter().enumerate()
            .map(|(i, l)| IndexedLine { line: *l, index: i })
            .collect();

        let tree = RTree::bulk_load(indexed);

        // Find intersections
        let candidates = tree.intersection_candidates_with_other_tree(&tree);

        for (idx1, idx2) in candidates {
            let i = idx1.index;
            let j = idx2.index;
            if i >= j { continue; } // Handle unique pairs

            let l1 = idx1.line;
            let l2 = idx2.line;

            if let Some(res) = geo::algorithm::line_intersection::line_intersection(l1, l2) {
                 self.handle_intersection(res, i, j, l1, l2, &mut splits);
            }
        }

        splits
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

    fn handle_intersection(&self,
        res: LineIntersection<f64>,
        i: usize,
        j: usize,
        l1: Line<f64>,
        l2: Line<f64>,
        splits: &mut HashMap<usize, Vec<Coord<f64>>>
    ) {
         match res {
            LineIntersection::SinglePoint { intersection: pt, .. } => {
                let snapped = self.snap(pt);

                // Check if split needed for L1
                if snapped != l1.start && snapped != l1.end {
                    splits.entry(i).or_insert_with(Vec::new).push(snapped);
                }
                // Check if split needed for L2
                if snapped != l2.start && snapped != l2.end {
                    splits.entry(j).or_insert_with(Vec::new).push(snapped);
                }
            },
            LineIntersection::Collinear { intersection: overlap } => {
                // For collinear, we split at the overlap endpoints
                let p1 = self.snap(overlap.start);
                let p2 = self.snap(overlap.end);

                for p in [p1, p2] {
                     if p != l1.start && p != l1.end {
                         splits.entry(i).or_insert_with(Vec::new).push(p);
                     }
                     if p != l2.start && p != l2.end {
                         splits.entry(j).or_insert_with(Vec::new).push(p);
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
    fn test_simd_vs_rtree_equivalence() {
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

        let splits_rtree = noder.find_splits(&lines);
        let splits_simd = noder.find_splits_simd(&lines);

        assert_eq!(splits_rtree.len(), splits_simd.len(), "Different number of split events");

        for (idx, points_rtree) in &splits_rtree {
            let points_simd = splits_simd.get(idx).expect("Index missing in SIMD splits");

            // Sort points to ensure order independence
            let mut p_rtree = points_rtree.clone();
            p_rtree.sort_by(|a,b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());

            let mut p_simd = points_simd.clone();
            p_simd.sort_by(|a,b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());

            assert_eq!(p_rtree, p_simd, "Split points differ for line {}", idx);
        }
    }
}
