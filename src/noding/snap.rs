use geo::{Line, Coord};
use rstar::{RTree, RTreeObject, AABB};
use std::cmp::Ordering;
use geo::algorithm::line_intersection::LineIntersection;

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
            let split_map = self.find_splits(&lines);

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

            // Deduplicate segments?
            // Yes, duplicate segments are common in noding.
            // Also normalize direction.
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

    fn find_splits(&self, lines: &[Line<f64>]) -> std::collections::HashMap<usize, Vec<Coord<f64>>> {
        let mut splits = std::collections::HashMap::new();

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

            // Fast bounding box check (handled by RTree, but good to be sure)

            // Intersection
            if let Some(res) = geo::algorithm::line_intersection::line_intersection(l1, l2) {
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

        splits
    }
}
