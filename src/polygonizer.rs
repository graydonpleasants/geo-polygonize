use crate::graph::PlanarGraph;
use geo_types::{Geometry, LineString, Polygon};
use crate::error::Result;
use geo::algorithm::contains::Contains;
use geo::bounding_rect::BoundingRect;
use geo::algorithm::line_intersection::LineIntersection;
use geo::Area;
use geo::Line;
use rstar::{RTree, AABB, RTreeObject};
use rayon::prelude::*;

// Wrapper for Polygon to be indexable by rstar
struct IndexedPolygon(Polygon<f64>, usize);

impl RTreeObject for IndexedPolygon {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        let bbox = self.0.bounding_rect().unwrap();
        AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y])
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
        // Recurse? prune_dangles is iterative.

        // 3. Find rings
        let rings = self.graph.get_edge_rings();

        // 4. Assign holes
        let mut shells = Vec::new();
        let mut holes = Vec::new();

        for ring in rings {
            // Check orientation.
            // Using geo::algorithm::orient::Orient
            // However, LineString doesn't implement Orient directly in some versions?
            // Convert to Polygon (temporarily) to check orientation or use signed_area.
            // A simple Polygon from ring.
            let poly = Polygon::new(ring.clone(), vec![]);
            // OGC: Shells are CCW (Positive), Holes are CW (Negative).
            // JTS Polygonizer logic:
            // "The orientation of the ring determines whether it is a shell or a hole."
            // Shells are CCW. Holes are CW.
            // Note: geo::Orient for Polygon usually enforces CCW for exterior.
            // We need to check the raw signed area of the ring.

            // Calculate signed area.
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
        // This handles mesh cases where the CCW traversal misses inner faces.
        // Parallelized check.
        let promoted_shells: Vec<_> = holes.par_iter().filter_map(|hole| {
            let hole_area = hole.unsigned_area();
            // Naive check: Area match. (Optimization: Use RTree/Hash later if needed)
            // shells is read-only here so safe to share across threads.
            let has_twin = shells.iter().any(|shell| {
                if (shell.unsigned_area() - hole_area).abs() < 1e-6 {
                    // Potential twin. Check bounding rect.
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
        // Build RTree of shells
        let mut indexed_shells = Vec::new();
        for (i, shell) in shells.iter().enumerate() {
            indexed_shells.push(IndexedPolygon(shell.clone(), i));
        }
        let tree = RTree::bulk_load(indexed_shells);

        // Parallel hole assignment
        let assignments: Vec<_> = holes.par_iter().filter_map(|hole_poly| {
            let hole_ring = hole_poly.exterior();
            // A hole is contained in a shell if the shell contains the hole's envelope
            // AND the shell contains a point of the hole.

            // Query tree
            let hole_bbox = hole_poly.bounding_rect().unwrap();
            let hole_aabb = AABB::from_corners([hole_bbox.min().x, hole_bbox.min().y], [hole_bbox.max().x, hole_bbox.max().y]);

            let candidates = tree.locate_in_envelope_intersecting(&hole_aabb);

            let mut best_shell_idx = None;
            let mut min_area = f64::MAX;

            // Pick the smallest shell that contains the hole
            for cand in candidates {
                let shell = &cand.0;
                let idx = cand.1;

                if shell.contains(hole_poly) { // geo::Contains
                   let area = shell.unsigned_area();
                   let hole_area = hole_poly.unsigned_area();

                   // Ensure we don't assign a hole to its own twin shell (same area/geometry)
                   // and pick the smallest containing shell.
                   if area > hole_area + 1e-6 && area < min_area {
                       min_area = area;
                       best_shell_idx = Some(idx);
                   }
                }
            }

            best_shell_idx.map(|idx| (idx, hole_ring.clone()))
        }).collect();

        // Map shell index to list of hole indices
        let mut shell_holes: Vec<Vec<LineString<f64>>> = vec![vec![]; shells.len()];
        for (idx, hole) in assignments {
            shell_holes[idx].push(hole);
        }

        // Construct final polygons
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

/// Simple iterative noder (O(N^2) per pass).
/// Splits lines at intersections.
fn node_lines(input_lines: Vec<LineString<f64>>) -> Vec<LineString<f64>> {
    let mut segments: Vec<Line<f64>> = Vec::new();
    for ls in input_lines {
        for line in ls.lines() {
            segments.push(line);
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        let mut new_segments = Vec::with_capacity(segments.len());
        let mut split_indices = std::collections::HashSet::new();

        // Check pairwise intersections
        // Optimization: Could use spatial index here.
        'outer: for i in 0..segments.len() {
            if split_indices.contains(&i) { continue; }

            for j in (i+1)..segments.len() {
                if split_indices.contains(&j) { continue; }

                let s1 = segments[i];
                let s2 = segments[j];

                // Check intersection
                if let Some(res) = geo::algorithm::line_intersection::line_intersection(s1, s2) {
                    match res {
                        LineIntersection::SinglePoint { intersection: pt, is_proper: _ } => {
                            // Split s1 and s2 at pt
                            // We verify if pt is strictly internal to avoid infinite splitting at endpoints
                            let tol = 1e-10;
                            let internal_s1 = (pt.x - s1.start.x).abs() > tol && (pt.x - s1.end.x).abs() > tol || (pt.y - s1.start.y).abs() > tol && (pt.y - s1.end.y).abs() > tol;
                            let internal_s2 = (pt.x - s2.start.x).abs() > tol && (pt.x - s2.end.x).abs() > tol || (pt.y - s2.start.y).abs() > tol && (pt.y - s2.end.y).abs() > tol;

                            if internal_s1 || internal_s2 {
                                // Add split segments
                                if internal_s1 {
                                    new_segments.push(Line::new(s1.start, pt));
                                    new_segments.push(Line::new(pt, s1.end));
                                    split_indices.insert(i);
                                } else {
                                    // Keep s1 if not split (later) - but we need to handle logic carefully
                                }

                                if internal_s2 {
                                    new_segments.push(Line::new(s2.start, pt));
                                    new_segments.push(Line::new(pt, s2.end));
                                    split_indices.insert(j);
                                } else {
                                    // Keep s2 if not split
                                }

                                // Mark as changed
                                changed = true;

                                // For s1, if we split, we are done with s1 in this pass.
                                // For s2, we marked it split.
                                // If we only split one, the other remains?
                                // This logic is tricky. A simpler way:
                                // If intersection found, split BOTH (if internal), add pieces to a new list,
                                // add all other unprocessed segments to new list, and RESTART the loop (or continue carefully).
                                // Restarting is safer.

                                // Add all remaining segments?
                                // Or better: just collect all "valid" segments.
                                // If we assume iterative refinement:
                                // If we found ONE intersection, we break and rebuild list.
                                break 'outer;
                            }
                        }
                        LineIntersection::Collinear { intersection: _ } => {
                            // Overlapping segments. Complex case.
                            // For MVP, ignore or rely on robust graph building (it handles overlapping somewhat).
                        }
                    }
                }
            }
        }

        if changed {
             // Rebuild list
             // Add all non-split segments from old list
             for i in 0..segments.len() {
                 if !split_indices.contains(&i) {
                     new_segments.push(segments[i]);
                 }
             }
             segments = new_segments;
        }
    }

    // Convert back to LineStrings
    segments.into_iter().map(|s| LineString::from(vec![s.start, s.end])).collect()
}
