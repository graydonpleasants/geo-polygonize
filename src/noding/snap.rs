use crate::noding::grid::UniformGrid;
use crate::utils::soa::SoALines;
use geo::algorithm::line_intersection::LineIntersection;
use geo::{Coord, Line};
use std::cmp::Ordering;
use std::collections::HashMap;
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

        // Remove degenerates
        lines.retain(|l| l.start != l.end);

        // Normalize and dedup initial input
        self.normalize_and_dedup(&mut lines);

        // 2. Iterative Noding
        for _iter in 0..self.max_iter {
            let use_grid = match self.strategy {
                NodingStrategy::Auto => lines.len() >= 256,
                NodingStrategy::Grid => true,
                NodingStrategy::Simd => false,
                NodingStrategy::Scalar => false, // Fallback to SIMD logic which handles scalar internally
            };

            let splits = if !use_grid {
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
            if segment.start.x > segment.end.x
                || ((segment.start.x - segment.end.x).abs() < 1e-12
                    && segment.start.y > segment.end.y)
            {
                std::mem::swap(&mut segment.start, &mut segment.end);
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
        if self.grid_size == 0.0 {
            return c;
        }
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
            let all_splits: Vec<(usize, Coord<f64>)> = lines
                .par_iter()
                .enumerate()
                .flat_map(|(i, &query_line)| {
                    self.check_intersection_simd(query_line, i, lines, &soa)
                })
                .collect();

            // Aggregate results into HashMap
            let mut splits: HashMap<usize, Vec<Coord<f64>>> = HashMap::new();
            for (idx, pt) in all_splits {
                splits.entry(idx).or_default().push(pt);
            }
            splits
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut splits: HashMap<usize, Vec<Coord<f64>>> = HashMap::new();
            for (i, &query_line) in lines.iter().enumerate() {
                let events = self.check_intersection_simd(query_line, i, lines, &soa);
                for (idx, pt) in events {
                    splits.entry(idx).or_default().push(pt);
                }
            }
            splits
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
                if let Some(res) =
                    geo::algorithm::line_intersection::line_intersection(query_line, target_line)
                {
                    self.collect_intersection_events(
                        res,
                        i,
                        j,
                        query_line,
                        target_line,
                        &mut events,
                    );
                }
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
                        if let Some(res) = geo::algorithm::line_intersection::line_intersection(
                            query_line,
                            target_line,
                        ) {
                            // We can't update a shared HashMap here.
                            // Return the intersection events for the caller to aggregate.
                            self.collect_intersection_events(
                                res,
                                i,
                                target_idx,
                                query_line,
                                target_line,
                                &mut events,
                            );
                        }
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
        splits: &mut HashMap<usize, Vec<Coord<f64>>>,
    ) {
        if i >= lines.len() || j >= lines.len() {
            return;
        }

        let l1 = lines[i];
        let l2 = lines[j];

        if let Some(res) = geo::algorithm::line_intersection::line_intersection(l1, l2) {
            match res {
                LineIntersection::SinglePoint {
                    intersection: pt, ..
                } => {
                    let snapped = self.snap(pt);
                    if snapped != l1.start && snapped != l1.end {
                        splits.entry(i).or_default().push(snapped);
                    }
                    if snapped != l2.start && snapped != l2.end {
                        splits.entry(j).or_default().push(snapped);
                    }
                }
                LineIntersection::Collinear {
                    intersection: overlap,
                } => {
                    let p1 = self.snap(overlap.start);
                    let p2 = self.snap(overlap.end);
                    for p in [p1, p2] {
                        if p != l1.start && p != l1.end {
                            splits.entry(i).or_default().push(p);
                        }
                        if p != l2.start && p != l2.end {
                            splits.entry(j).or_default().push(p);
                        }
                    }
                }
            }
        }
    }

    // Helper to collect events into a local vector instead of HashMap
    fn collect_intersection_events(
        &self,
        res: LineIntersection<f64>,
        i: usize,
        j: usize,
        l1: Line<f64>,
        l2: Line<f64>,
        events: &mut Vec<(usize, Coord<f64>)>,
    ) {
        match res {
            LineIntersection::SinglePoint {
                intersection: pt, ..
            } => {
                let snapped = self.snap(pt);
                if snapped != l1.start && snapped != l1.end {
                    events.push((i, snapped));
                }
                if snapped != l2.start && snapped != l2.end {
                    events.push((j, snapped));
                }
            }
            LineIntersection::Collinear {
                intersection: overlap,
            } => {
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
        let splits_grid = grid.find_splits(&lines, &noder);

        // SIMD Logic
        let splits_simd = noder.find_splits_simd(&lines);

        assert_eq!(
            splits_grid.len(),
            splits_simd.len(),
            "Different number of lines with splits"
        );

        for (idx, points_grid) in &splits_grid {
            let points_simd = splits_simd.get(idx).expect("Index missing in SIMD splits");

            // Sort points to ensure order independence
            let mut p_grid = points_grid.clone();
            p_grid.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
            p_grid.dedup();

            let mut p_simd = points_simd.clone();
            p_simd.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
            p_simd.dedup();

            for (p_g, p_s) in p_grid.iter().zip(p_simd.iter()) {
                assert!(
                    (p_g.x - p_s.x).abs() < 1e-10 && (p_g.y - p_s.y).abs() < 1e-10,
                    "Point mismatch: {:?} vs {:?}",
                    p_g,
                    p_s
                );
            }
        }
    }
}
