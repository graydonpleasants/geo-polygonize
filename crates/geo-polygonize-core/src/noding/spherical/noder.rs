use crate::types::{Coord3D, Line3D};
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};

pub struct SphericalNoder;

impl Default for SphericalNoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SphericalNoder {
    pub fn new() -> Self {
        Self
    }

    /// A placeholder noder for spherical geometry.
    pub fn node(&self, lines: Vec<Line3D>) -> Vec<Line3D> {
        // As a starting implementation, we return lines as is or just apply basic spherical noding.
        // We will perform O(n^2) intersection finding for MVP. A true sweep-line for geodesic
        // is complex.

        let mut segments = lines;
        let mut changed = true;

        while changed {
            changed = false;
            let mut new_segments = Vec::new();
            let mut i = 0;

            while i < segments.len() {
                let seg1 = segments[i];
                let mut intersected = false;

                for j in (i + 1)..segments.len() {
                    let seg2 = segments[j];

                    // Simple bounding box check could go here

                    // For now we'll use standard 2D Cartesian intersection
                    // until full geographic geodesic intersections are supported.
                    if let Some(intersection) = line_intersection(seg1.to_line_2d(), seg2.to_line_2d()) {
                        match intersection {
                            LineIntersection::SinglePoint { intersection: pt, is_proper } => {
                                if is_proper {
                                    let c = Coord3D::new(pt.x, pt.y, 0.0);

                                    new_segments.push(Line3D::new(seg1.start, c, seg1.line_id));
                                    new_segments.push(Line3D::new(c, seg1.end, seg1.line_id));

                                    new_segments.push(Line3D::new(seg2.start, c, seg2.line_id));
                                    new_segments.push(Line3D::new(c, seg2.end, seg2.line_id));

                                    segments.remove(j);
                                    intersected = true;
                                    changed = true;
                                    break;
                                }
                            }
                            LineIntersection::Collinear { .. } => {
                                // Handled elsewhere or ignore for now
                            }
                        }
                    }
                }

                if !intersected {
                    new_segments.push(seg1);
                }

                i += 1;
            }

            segments = new_segments;
        }

        segments
    }
}
