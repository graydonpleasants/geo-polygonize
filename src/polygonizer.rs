use crate::graph::PlanarGraph;
use geo_types::{Geometry, LineString, Polygon, Coord};
use crate::error::Result;
use geo::algorithm::contains::Contains;
use geo::bounding_rect::BoundingRect;
use geo::algorithm::line_intersection::LineIntersection;
use geo::Area;
use geo::Line;
use rstar::{RTree, AABB, RTreeObject};
use rayon::prelude::*;
use std::cmp::Ordering;

// Wrapper for Polygon to be indexable by rstar
struct IndexedPolygon(Polygon<f64>, usize);

impl RTreeObject for IndexedPolygon {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        let bbox = self.0.bounding_rect().unwrap();
        AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y])
    }
}

// Wrapper for Line to be indexable by rstar
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
        let min_x = p1.x.min(p2.x);
        let min_y = p1.y.min(p2.y);
        let max_x = p1.x.max(p2.x);
        let max_y = p1.y.max(p2.y);
        AABB::from_corners([min_x, min_y], [max_x, max_y])
    }
}

pub struct Polygonizer {
    graph: PlanarGraph,
    // Configuration
    pub check_valid_rings: bool,
    pub node_input: bool,

    // Buffer for inputs if noding is required
    inputs: Vec<Geometry<f64>>,
    dirty: bool,
}

impl Polygonizer {
    pub fn new() -> Self {
        Self {
            graph: PlanarGraph::new(),
            check_valid_rings: true,
            node_input: false,
            inputs: Vec::new(),
            dirty: false,
        }
    }

    /// Adds a geometry to the graph.
    pub fn add_geometry(&mut self, geom: Geometry<f64>) {
        self.inputs.push(geom);
        self.dirty = true;
    }

    fn build_graph(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        // Flatten inputs to lineal components
        let mut lines = Vec::new();
        for geom in &self.inputs {
            extract_lines(geom, &mut lines);
        }

        if self.node_input {
            let noded_lines = node_lines(lines);
            for line in noded_lines {
                self.graph.add_line_string(line);
            }
        } else {
            for line in lines {
                self.graph.add_line_string(line);
            }
        }

        self.dirty = false;
        Ok(())
    }

    /// Computes the polygons.
    /// This is the main entry point.
    pub fn polygonize(&mut self) -> Result<Vec<geo_types::Polygon<f64>>> {
        self.build_graph()?;

        // 1. Sort edges (Geometry Graph operation)
        self.graph.sort_edges();

        // 2. Prune dangles
        let _dangles_removed = self.graph.prune_dangles();

        // 3. Find rings
        let rings = self.graph.get_edge_rings();

        // 4. Assign holes
        let mut shells = Vec::new();
        let mut holes = Vec::new();

        for ring in rings {
            let poly = Polygon::new(ring.clone(), vec![]);
            let area = poly.signed_area();

            if area.abs() < 1e-9 {
                continue; // Degenerate
            }

            if area > 0.0 {
                // CCW -> Shell
                shells.push(poly);
            } else {
                // CW -> Hole
                holes.push(poly);
            }
        }

        // Promote CW rings to Shells if they don't have a corresponding CCW Twin.
        let promoted_shells: Vec<_> = holes.par_iter().filter_map(|hole| {
            let hole_area = hole.unsigned_area();
            let has_twin = shells.iter().any(|shell| {
                if (shell.unsigned_area() - hole_area).abs() < 1e-6 {
                    if shell.bounding_rect() == hole.bounding_rect() {
                        return true;
                    }
                }
                false
            });

            if !has_twin {
                let mut shell_copy = hole.clone();
                shell_copy.exterior_mut(|ext| {
                    use geo::algorithm::winding_order::Winding;
                    ext.make_ccw_winding();
                });
                Some(shell_copy)
            } else {
                None
            }
        }).collect();
        shells.extend(promoted_shells);

        // Assign holes to shells
        let mut indexed_shells = Vec::new();
        for (i, shell) in shells.iter().enumerate() {
            indexed_shells.push(IndexedPolygon(shell.clone(), i));
        }
        let tree = RTree::bulk_load(indexed_shells);

        let assignments: Vec<_> = holes.par_iter().filter_map(|hole_poly| {
            let hole_ring = hole_poly.exterior();
            let hole_bbox = hole_poly.bounding_rect().unwrap();
            let hole_aabb = AABB::from_corners([hole_bbox.min().x, hole_bbox.min().y], [hole_bbox.max().x, hole_bbox.max().y]);

            let candidates = tree.locate_in_envelope_intersecting(&hole_aabb);

            let mut best_shell_idx = None;
            let mut min_area = f64::MAX;

            for cand in candidates {
                let shell = &cand.0;
                let idx = cand.1;

                if shell.contains(hole_poly) {
                   let area = shell.unsigned_area();
                   let hole_area = hole_poly.unsigned_area();

                   if area > hole_area + 1e-6 && area < min_area {
                       min_area = area;
                       best_shell_idx = Some(idx);
                   }
                }
            }

            best_shell_idx.map(|idx| (idx, hole_ring.clone()))
        }).collect();

        let mut shell_holes: Vec<Vec<LineString<f64>>> = vec![vec![]; shells.len()];
        for (idx, hole) in assignments {
            shell_holes[idx].push(hole);
        }

        let mut result = Vec::new();
        for (i, shell) in shells.into_iter().enumerate() {
            let holes = shell_holes[i].clone();
            result.push(Polygon::new(shell.exterior().clone(), holes));
        }

        Ok(result)
    }
}

fn extract_lines(geom: &Geometry<f64>, out: &mut Vec<LineString<f64>>) {
    match geom {
        Geometry::LineString(ls) => out.push(ls.clone()),
        Geometry::MultiLineString(mls) => {
            out.extend(mls.0.clone());
        },
        Geometry::Polygon(poly) => {
            out.push(poly.exterior().clone());
            out.extend(poly.interiors().iter().cloned());
        },
        Geometry::MultiPolygon(mpoly) => {
            for poly in mpoly {
                out.push(poly.exterior().clone());
                out.extend(poly.interiors().iter().cloned());
            }
        },
        Geometry::GeometryCollection(gc) => {
            for g in gc {
                extract_lines(g, out);
            }
        },
        _ => {},
    }
}

/// Robust Noding with R-Tree acceleration.
fn node_lines(input_lines: Vec<LineString<f64>>) -> Vec<LineString<f64>> {
    let mut segments: Vec<Line<f64>> = Vec::new();
    for ls in input_lines {
        for line in ls.lines() {
            segments.push(line);
        }
    }

    let tol = 1e-10;

    loop {
        // Store split points as Coords
        let mut split_points: Vec<Vec<Coord<f64>>> = vec![vec![]; segments.len()];
        let mut found_intersection = false;

        let mut indexed_segments = Vec::with_capacity(segments.len());
        for (i, s) in segments.iter().enumerate() {
            indexed_segments.push(IndexedLine { line: *s, index: i });
        }
        let tree = RTree::bulk_load(indexed_segments);

        for (_i, s1_wrapper) in tree.iter().enumerate() {
            let s1 = s1_wrapper.line;
            let idx1 = s1_wrapper.index;
            let s1_aabb = s1_wrapper.envelope();

            // Find candidates
            let candidates = tree.locate_in_envelope_intersecting(&s1_aabb);

            for cand in candidates {
                let idx2 = cand.index;
                if idx2 <= idx1 { continue; } // Avoid duplicates and self

                let s2 = cand.line;

                if let Some(res) = geo::algorithm::line_intersection::line_intersection(s1, s2) {
                    match res {
                        LineIntersection::SinglePoint { intersection: pt, is_proper: _ } => {
                            // Check if internal
                             let internal_s1 = (pt.x - s1.start.x).abs() > tol && (pt.x - s1.end.x).abs() > tol || (pt.y - s1.start.y).abs() > tol && (pt.y - s1.end.y).abs() > tol;
                             let internal_s2 = (pt.x - s2.start.x).abs() > tol && (pt.x - s2.end.x).abs() > tol || (pt.y - s2.start.y).abs() > tol && (pt.y - s2.end.y).abs() > tol;

                             if internal_s1 || internal_s2 {
                                 found_intersection = true;
                                 let coord = pt;
                                 if internal_s1 {
                                     split_points[idx1].push(coord);
                                 }
                                 if internal_s2 {
                                     split_points[idx2].push(coord);
                                 }
                             }
                        },
                        LineIntersection::Collinear { intersection: overlap } => {
                             // Handle collinear overlap.
                             // We split both segments at the endpoints of the overlap.
                             // The overlap segment is strictly contained in or equal to s1 and s2 (or parts of them).
                             // We treat the overlap endpoints as split points.
                             // We only need to add them if they are internal to the respective segment (or if we want to force splitting at endpoints even if equal).
                             // Actually, for collinear logic, we want to normalize the segments.
                             // If s1 and s2 overlap, we want to decompose them.
                             // Adding split points at overlap start/end will achieve this decomposition in the next pass.

                             let p1 = overlap.start;
                             let p2 = overlap.end;

                             // For s1
                             let s1_has_p1 = (p1.x - s1.start.x).abs() > tol && (p1.x - s1.end.x).abs() > tol || (p1.y - s1.start.y).abs() > tol && (p1.y - s1.end.y).abs() > tol;
                             let s1_has_p2 = (p2.x - s1.start.x).abs() > tol && (p2.x - s1.end.x).abs() > tol || (p2.y - s1.start.y).abs() > tol && (p2.y - s1.end.y).abs() > tol;

                             if s1_has_p1 || s1_has_p2 {
                                 found_intersection = true;
                                 if s1_has_p1 { split_points[idx1].push(p1); }
                                 if s1_has_p2 { split_points[idx1].push(p2); }
                             }

                             // For s2
                             let s2_has_p1 = (p1.x - s2.start.x).abs() > tol && (p1.x - s2.end.x).abs() > tol || (p1.y - s2.start.y).abs() > tol && (p1.y - s2.end.y).abs() > tol;
                             let s2_has_p2 = (p2.x - s2.start.x).abs() > tol && (p2.x - s2.end.x).abs() > tol || (p2.y - s2.start.y).abs() > tol && (p2.y - s2.end.y).abs() > tol;

                             if s2_has_p1 || s2_has_p2 {
                                 found_intersection = true;
                                 if s2_has_p1 { split_points[idx2].push(p1); }
                                 if s2_has_p2 { split_points[idx2].push(p2); }
                             }

                             // If the segments are identical (or overlap covers whole segment), no internal split points are added.
                             // This effectively means we keep both copies.
                             // We rely on dedup later to merge them.
                             // If s1 is (0,10) and s2 is (0,10), overlap is (0,10). p1=0, p2=10.
                             // s1_has_p1 is False (endpoint). s1_has_p2 is False.
                             // found_intersection remains whatever it was.
                             // We need to flag that an overlap exists so we can at least dedup?
                             // Dedup happens at the end of the function regardless of 'found_intersection'.
                             // So if we have duplicates, they will be removed at the end.
                             // But if we have partial overlap (0,10) and (5,15), overlap (5,10).
                             // s1 gets split at 5. s2 gets split at 10.
                             // Next pass: s1 -> (0,5), (5,10). s2 -> (5,10), (10,15).
                             // (5,10) matches (5,10).
                             // They will be deduped at the end.
                             // Correct.
                        }
                    }
                }
            }
        }

        if !found_intersection {
            break;
        }

        // Apply splits
        let mut new_segments = Vec::with_capacity(segments.len() * 2);
        for (i, segment) in segments.iter().enumerate() {
            let points = &mut split_points[i];
            if points.is_empty() {
                new_segments.push(*segment);
            } else {
                let start = segment.start;

                // Sort by distance from start
                points.sort_by(|a, b| {
                    let da = (a.x - start.x).powi(2) + (a.y - start.y).powi(2);
                    let db = (b.x - start.x).powi(2) + (b.y - start.y).powi(2);
                    da.partial_cmp(&db).unwrap_or(Ordering::Equal)
                });

                points.dedup_by(|a, b| {
                     (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
                });

                let mut curr = start;
                for pt in points {
                     if (pt.x - curr.x).powi(2) + (pt.y - curr.y).powi(2) > tol * tol {
                         new_segments.push(Line::new(curr, *pt));
                         curr = *pt;
                     }
                }
                if (segment.end.x - curr.x).powi(2) + (segment.end.y - curr.y).powi(2) > tol * tol {
                    new_segments.push(Line::new(curr, segment.end));
                }
            }
        }
        segments = new_segments;
    }

    // Dedup segments
    // Sort to bring duplicates together
    segments.sort_by(|a, b| {
        let sa = (a.start.x, a.start.y, a.end.x, a.end.y);
        let sb = (b.start.x, b.start.y, b.end.x, b.end.y);
        // Normalize orientation for undirected comparison?
        // Noding usually preserves direction if input is directed graph?
        // But here inputs are just lines.
        // If we have (0,0)->(10,0) and (10,0)->(0,0), they are different directed edges but same geometry.
        // If we want to unique-ify geometry, we should sort endpoints.
        // But PlanarGraph might depend on direction?
        // PlanarGraph::add_line_string adds edges.
        // If we return (0,10) and (10,0), the graph will have both directed edges.
        // This is fine. But if we have TWO (0,10), we definitely want to remove one.

        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });
    segments.dedup_by(|a, b| {
        let tol = 1e-10;
        (a.start.x - b.start.x).abs() < tol && (a.start.y - b.start.y).abs() < tol &&
        (a.end.x - b.end.x).abs() < tol && (a.end.y - b.end.y).abs() < tol
    });

    segments.into_iter().map(|s| LineString::from(vec![s.start, s.end])).collect()
}
