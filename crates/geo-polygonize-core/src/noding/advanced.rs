use crate::types::{Coord3D, Line3D};
use std::collections::HashSet;

pub struct AdvancedNoder;

impl Default for AdvancedNoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedNoder {
    pub fn new() -> Self {
        Self
    }

    pub fn node(&self, lines: Vec<Line3D>) -> Vec<Line3D> {
        // We will use a fast R-Tree-accelerated intersection finding.
        // It provides O(N log N + K) complexity where K is the number of intersections,
        // which avoids the O(N^2) brute force.
        use rstar::{RTree, RTreeObject, AABB};

        #[derive(Clone, Copy, Debug)]
        struct RTreeLine {
            line: Line3D,
            id: usize,
        }

        impl RTreeObject for RTreeLine {
            type Envelope = AABB<[f64; 2]>;

            fn envelope(&self) -> Self::Envelope {
                let min_x = f64::min(self.line.start.x, self.line.end.x);
                let max_x = f64::max(self.line.start.x, self.line.end.x);
                let min_y = f64::min(self.line.start.y, self.line.end.y);
                let max_y = f64::max(self.line.start.y, self.line.end.y);
                AABB::from_corners([min_x, min_y], [max_x, max_y])
            }
        }

        let mut segments = lines;
        let mut changed = true;

        while changed {
            changed = false;

            let mut tree_elements = Vec::with_capacity(segments.len());
            for (i, seg) in segments.iter().enumerate() {
                tree_elements.push(RTreeLine { line: *seg, id: i });
            }

            // Build R-Tree for fast spatial queries
            let tree = RTree::bulk_load(tree_elements);

            let mut new_segments = Vec::new();
            let mut skip_indices = HashSet::new();

            for (i, s1) in segments.iter().enumerate() {
                if skip_indices.contains(&i) {
                    continue;
                }

                let mut intersected = false;

                let min_x = f64::min(s1.start.x, s1.end.x);
                let max_x = f64::max(s1.start.x, s1.end.x);
                let min_y = f64::min(s1.start.y, s1.end.y);
                let max_y = f64::max(s1.start.y, s1.end.y);
                let aabb = AABB::from_corners([min_x, min_y], [max_x, max_y]);

                // Query R-Tree for intersecting bounding boxes
                let candidates = tree.locate_in_envelope_intersecting(&aabb);

                for cand in candidates {
                    let j = cand.id;
                    if j <= i {
                        continue;
                    }
                    if skip_indices.contains(&j) {
                        continue;
                    }
                    let s2 = cand.line;

                    if let Some(
                        geo::algorithm::line_intersection::LineIntersection::SinglePoint {
                            intersection: pt,
                            ..
                        },
                    ) = geo::algorithm::line_intersection::line_intersection(
                        geo::Line::new(s1.start.to_coord_2d(), s1.end.to_coord_2d()),
                        geo::Line::new(s2.start.to_coord_2d(), s2.end.to_coord_2d()),
                    ) {
                        let eps = 1e-9;
                        let s1_start_dist =
                            (pt.x - s1.start.x).powi(2) + (pt.y - s1.start.y).powi(2);
                        let s1_end_dist = (pt.x - s1.end.x).powi(2) + (pt.y - s1.end.y).powi(2);
                        let s2_start_dist =
                            (pt.x - s2.start.x).powi(2) + (pt.y - s2.start.y).powi(2);
                        let s2_end_dist = (pt.x - s2.end.x).powi(2) + (pt.y - s2.end.y).powi(2);

                        if s1_start_dist > eps
                            && s1_end_dist > eps
                            && s2_start_dist > eps
                            && s2_end_dist > eps
                        {
                            // True interior intersection!
                            let t1 = s1_start_dist.sqrt()
                                / ((s1.end.x - s1.start.x).powi(2)
                                    + (s1.end.y - s1.start.y).powi(2))
                                .sqrt();
                            let z_interp = s1.start.z + t1 * (s1.end.z - s1.start.z);

                            let intersect_coord = Coord3D {
                                x: pt.x,
                                y: pt.y,
                                z: z_interp,
                            };

                            let s2_1 = Line3D {
                                start: s2.start,
                                end: intersect_coord,
                                line_id: s2.line_id,
                            };
                            let s2_2 = Line3D {
                                start: intersect_coord,
                                end: s2.end,
                                line_id: s2.line_id,
                            };

                            if (s2_1.start.x - s2_1.end.x).powi(2)
                                + (s2_1.start.y - s2_1.end.y).powi(2)
                                > eps
                            {
                                new_segments.push(s2_1);
                            }
                            if (s2_2.start.x - s2_2.end.x).powi(2)
                                + (s2_2.start.y - s2_2.end.y).powi(2)
                                > eps
                            {
                                new_segments.push(s2_2);
                            }

                            let s1_1 = Line3D {
                                start: s1.start,
                                end: intersect_coord,
                                line_id: s1.line_id,
                            };
                            let s1_2 = Line3D {
                                start: intersect_coord,
                                end: s1.end,
                                line_id: s1.line_id,
                            };

                            if (s1_1.start.x - s1_1.end.x).powi(2)
                                + (s1_1.start.y - s1_1.end.y).powi(2)
                                > eps
                            {
                                new_segments.push(s1_1);
                            }
                            if (s1_2.start.x - s1_2.end.x).powi(2)
                                + (s1_2.start.y - s1_2.end.y).powi(2)
                                > eps
                            {
                                new_segments.push(s1_2);
                            }

                            intersected = true;
                            changed = true;
                            skip_indices.insert(j);
                            break;
                        }
                    }
                }

                if !intersected {
                    new_segments.push(*s1);
                }
            }
            if changed {
                segments = new_segments;
            }
        }

        segments
    }
}
