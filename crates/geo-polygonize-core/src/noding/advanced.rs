use crate::types::{Coord3D, Line3D};
use intersect2d::algorithm::AlgorithmData;
use std::collections::HashMap;

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
        let mut data = AlgorithmData::<f64>::default();

        let geo_lines: Vec<_> = lines
            .iter()
            .map(|l| geo::Line::new(l.start.to_coord_2d(), l.end.to_coord_2d()))
            .collect();

        let _ = data.with_lines(geo_lines.into_iter());

        // compute returns Ok(result_iterator) or Err
        let res = match data.compute() {
            Ok(res) => res,
            Err(_) => return lines,
        };

        // Collect intersections
        let mut splits_by_line: HashMap<usize, Vec<Coord3D>> = HashMap::new();
        let eps = 1e-9;

        for (coord, line_indices) in res {
            for &idx in &line_indices {
                let l = lines[idx];

                let pt = Coord3D {
                    x: coord.x,
                    y: coord.y,
                    z: 0.0, // Z will be interpolated below
                };

                let start_dist = (pt.x - l.start.x).powi(2) + (pt.y - l.start.y).powi(2);
                let end_dist = (pt.x - l.end.x).powi(2) + (pt.y - l.end.y).powi(2);

                if start_dist > eps && end_dist > eps {
                    // True interior intersection
                    let total_dist = ((l.end.x - l.start.x).powi(2) + (l.end.y - l.start.y).powi(2)).sqrt();
                    let t = if total_dist > 0.0 {
                        ((pt.x - l.start.x).powi(2) + (pt.y - l.start.y).powi(2)).sqrt() / total_dist
                    } else {
                        0.5
                    };
                    let z_interp = l.start.z + t * (l.end.z - l.start.z);

                    let intersect_coord = Coord3D {
                        x: pt.x,
                        y: pt.y,
                        z: z_interp,
                    };

                    splits_by_line.entry(idx).or_default().push(intersect_coord);
                }
            }
        }

        let mut noded_lines = Vec::with_capacity(lines.len() + splits_by_line.len() * 2);

        for (i, line) in lines.into_iter().enumerate() {
            if let Some(mut splits) = splits_by_line.remove(&i) {
                // Sort splits by distance from start
                splits.sort_by(|a, b| {
                    let dist_a = (a.x - line.start.x).powi(2) + (a.y - line.start.y).powi(2);
                    let dist_b = (b.x - line.start.x).powi(2) + (b.y - line.start.y).powi(2);
                    dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
                });

                // Remove very close splits
                splits.dedup_by(|a, b| {
                    (a.x - b.x).powi(2) + (a.y - b.y).powi(2) < eps
                });

                let mut current_start = line.start;
                for split in splits {
                    if (current_start.x - split.x).powi(2) + (current_start.y - split.y).powi(2) > eps {
                        noded_lines.push(Line3D {
                            start: current_start,
                            end: split,
                            line_id: line.line_id,
                        });
                        current_start = split;
                    }
                }

                if (current_start.x - line.end.x).powi(2) + (current_start.y - line.end.y).powi(2) > eps {
                    noded_lines.push(Line3D {
                        start: current_start,
                        end: line.end,
                        line_id: line.line_id,
                    });
                }
            } else {
                noded_lines.push(line);
            }
        }

        noded_lines
    }
}
