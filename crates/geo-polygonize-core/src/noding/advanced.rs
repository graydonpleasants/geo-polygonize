use crate::types::{Coord3D, Line3D};
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};

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
        let mut segments = lines;
        let mut changed = true;

        // Basic prototype: Repeated O(N^2) brute-force intersection finding.
        // In a real advanced noder, this would be a sweep-line algorithm (e.g., Bentley-Ottmann)
        // or a monotone-chain noder.
        while changed {
            changed = false;
            let mut new_segments = Vec::with_capacity(segments.len());
            let mut i = 0;

            while i < segments.len() {
                let s1 = segments[i];
                let mut intersected = false;

                for j in (i + 1)..segments.len() {
                    let s2 = segments[j];

                    // Simple bounding box check
                    let s1_min_x = f64::min(s1.start.x, s1.end.x);
                    let s1_max_x = f64::max(s1.start.x, s1.end.x);
                    let s1_min_y = f64::min(s1.start.y, s1.end.y);
                    let s1_max_y = f64::max(s1.start.y, s1.end.y);

                    let s2_min_x = f64::min(s2.start.x, s2.end.x);
                    let s2_max_x = f64::max(s2.start.x, s2.end.x);
                    let s2_min_y = f64::min(s2.start.y, s2.end.y);
                    let s2_max_y = f64::max(s2.start.y, s2.end.y);

                    if s1_max_x < s2_min_x || s1_min_x > s2_max_x || s1_max_y < s2_min_y || s1_min_y > s2_max_y {
                        continue;
                    }

                    if let Some(intersection) = line_intersection(
                        geo::Line::new(s1.start.to_coord_2d(), s1.end.to_coord_2d()),
                        geo::Line::new(s2.start.to_coord_2d(), s2.end.to_coord_2d()),
                    ) {
                        match intersection {
                            LineIntersection::SinglePoint { intersection: pt, .. } => {
                                let eps = 1e-9;
                                // Ignore intersections at existing endpoints
                                let s1_start_dist = (pt.x - s1.start.x).powi(2) + (pt.y - s1.start.y).powi(2);
                                let s1_end_dist = (pt.x - s1.end.x).powi(2) + (pt.y - s1.end.y).powi(2);
                                let s2_start_dist = (pt.x - s2.start.x).powi(2) + (pt.y - s2.start.y).powi(2);
                                let s2_end_dist = (pt.x - s2.end.x).powi(2) + (pt.y - s2.end.y).powi(2);

                                if s1_start_dist > eps && s1_end_dist > eps && s2_start_dist > eps && s2_end_dist > eps {
                                    // Found a true interior intersection!
                                    // We'll interpolate Z just roughly for prototype.
                                    let t1 = ((pt.x - s1.start.x).powi(2) + (pt.y - s1.start.y).powi(2)).sqrt() /
                                             ((s1.end.x - s1.start.x).powi(2) + (s1.end.y - s1.start.y).powi(2)).sqrt();
                                    let z_interp = s1.start.z + t1 * (s1.end.z - s1.start.z);

                                    let intersect_coord = Coord3D { x: pt.x, y: pt.y, z: z_interp };

                                    // Split both segments. For prototype, we'll just queue the splits and restart.
                                    // Remove s2, replace it with splits
                                    segments.remove(j);
                                    let s2_1 = Line3D { start: s2.start, end: intersect_coord, line_id: s2.line_id };
                                    let s2_2 = Line3D { start: intersect_coord, end: s2.end, line_id: s2.line_id };
                                    if (s2_1.start.x - s2_1.end.x).powi(2) + (s2_1.start.y - s2_1.end.y).powi(2) > eps {
                                        segments.push(s2_1);
                                    }
                                    if (s2_2.start.x - s2_2.end.x).powi(2) + (s2_2.start.y - s2_2.end.y).powi(2) > eps {
                                        segments.push(s2_2);
                                    }

                                    // Split s1
                                    let s1_1 = Line3D { start: s1.start, end: intersect_coord, line_id: s1.line_id };
                                    let s1_2 = Line3D { start: intersect_coord, end: s1.end, line_id: s1.line_id };
                                    if (s1_1.start.x - s1_1.end.x).powi(2) + (s1_1.start.y - s1_1.end.y).powi(2) > eps {
                                        new_segments.push(s1_1);
                                    }
                                    if (s1_2.start.x - s1_2.end.x).powi(2) + (s1_2.start.y - s1_2.end.y).powi(2) > eps {
                                        new_segments.push(s1_2);
                                    }

                                    intersected = true;
                                    changed = true;
                                    break;
                                }
                            }
                            LineIntersection::Collinear { .. } => {
                                // Ignore collinear overlaps in this simple prototype
                            }
                        }
                    }
                }

                if !intersected {
                    new_segments.push(s1);
                }
                i += 1;
            }
            if changed {
                segments = new_segments;
            }
        }

        segments
    }
}
