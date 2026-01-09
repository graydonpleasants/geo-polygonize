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

        let mut segments = Vec::new();
        if self.node_input {
            segments = node_lines(lines);
        } else {
            for ls in lines {
                for line in ls.lines() {
                    segments.push(line);
                }
            }
        }

        // Use bulk load
        self.graph.bulk_load(segments);

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

        shells.reserve(rings.len() / 2);
        holes.reserve(rings.len() / 2);

        for ring in rings {
            // Note: LineString::signed_area() might return 0 even if closed in some geo versions/contexts?
            // Safer to wrap in Polygon which guarantees area calculation logic for rings.
            // Polygon::new is cheap (moves LineString).
            let poly = Polygon::new(ring, vec![]);
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

/// Robust Noding with Parallel R-Tree queries and Flat Memory Layout
fn node_lines(input_lines: Vec<LineString<f64>>) -> Vec<Line<f64>> {
    let mut segments: Vec<Line<f64>> = Vec::new();
    for ls in input_lines {
        for line in ls.lines() {
            segments.push(line);
        }
    }

    let tol = 1e-10;

    // One-Pass Robust Noding
    // We run a single pass to collect all intersection events.
    // Assuming the initial set of lines covers the geometry, splitting them at all intersection points
    // should result in a fully noded graph (barring numerical robustness issues which we handle with tolerance).

    // 1. Build Index
    let mut indexed_segments = Vec::with_capacity(segments.len());
    for (i, s) in segments.iter().enumerate() {
        indexed_segments.push(IndexedLine { line: *s, index: i });
    }
    let tree = RTree::bulk_load(indexed_segments);

    // 2. Find ALL intersection events using bulk query
    // Returns a flat list of (segment_index, split_point)
    // Common event processing logic
    let process_intersection = |acc: &mut Vec<(usize, Coord<f64>)>, cand1: &IndexedLine, cand2: &IndexedLine| {
        let idx1 = cand1.index;
        let idx2 = cand2.index;

        // Optimization: only process unique pairs
        if idx1 >= idx2 { return; }

        let s1 = cand1.line;
        let s2 = cand2.line;

        // Direct line intersection check, no pre-check
        let Some(res) = geo::algorithm::line_intersection::line_intersection(s1, s2) else {
            return;
        };

        // Use distance squared for internal check
        let is_internal = |s: Line<f64>, p: Coord<f64>| {
            let dx0 = p.x - s.start.x;
            let dy0 = p.y - s.start.y;
            let dx1 = p.x - s.end.x;
            let dy1 = p.y - s.end.y;
            let tol2 = tol * tol;
            (dx0 * dx0 + dy0 * dy0) > tol2 && (dx1 * dx1 + dy1 * dy1) > tol2
        };

        match res {
            LineIntersection::SinglePoint { intersection: pt, .. } => {
                if is_internal(s1, pt) { acc.push((idx1, pt)); }
                if is_internal(s2, pt) { acc.push((idx2, pt)); }
            },
            LineIntersection::Collinear { intersection: overlap } => {
                // Add overlap endpoints as split points if internal
                let p1 = overlap.start;
                let p2 = overlap.end;

                if is_internal(s1, p1) { acc.push((idx1, p1)); }
                if is_internal(s1, p2) { acc.push((idx1, p2)); }

                if is_internal(s2, p1) { acc.push((idx2, p1)); }
                if is_internal(s2, p2) { acc.push((idx2, p2)); }
            }
        }
    };

    #[cfg(feature = "parallel")]
    let intersection_events: Vec<(usize, Coord<f64>)> = tree
        .intersection_candidates_with_other_tree(&tree)
        .par_bridge()
        .fold(Vec::new, |mut acc, (cand1, cand2)| {
            process_intersection(&mut acc, cand1, cand2);
            acc
        })
        .reduce(Vec::new, |mut a, mut b| {
            a.append(&mut b);
            a
        });

    #[cfg(not(feature = "parallel"))]
    let intersection_events: Vec<(usize, Coord<f64>)> = tree
        .intersection_candidates_with_other_tree(&tree)
        .fold(Vec::new(), |mut acc, (cand1, cand2)| {
            process_intersection(&mut acc, cand1, cand2);
            acc
        });

    // 3. Apply splits
    if !intersection_events.is_empty() {
        // 1. Sort events by Segment Index
        let mut events = intersection_events;
        // Parallel sort the events
        events.par_sort_unstable_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| {
                     // Secondary sort by distance along segment?
                     // Or just coordinate sort is enough for dedup
                     a.1.x.partial_cmp(&b.1.x).unwrap_or(Ordering::Equal)
                })
        });

        // Reconstruct segments
        let mut new_segments = Vec::with_capacity(segments.len() * 2);
        let mut event_idx = 0;

        for (seg_idx, segment) in segments.iter().enumerate() {
            // Gather all points for this segment
            let mut points_on_seg = Vec::new();

            while event_idx < events.len() && events[event_idx].0 == seg_idx {
                points_on_seg.push(events[event_idx].1);
                event_idx += 1;
            }

            if points_on_seg.is_empty() {
                new_segments.push(*segment);
                continue;
            }

            // Sort points by distance from start
            let start = segment.start;
            points_on_seg.sort_by(|a, b| {
                 let da = (a.x - start.x).powi(2) + (a.y - start.y).powi(2);
                 let db = (b.x - start.x).powi(2) + (b.y - start.y).powi(2);
                 da.partial_cmp(&db).unwrap_or(Ordering::Equal)
            });

            // Dedup points
            points_on_seg.dedup_by(|a, b| {
                 (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
            });

            // Create sub-segments
            let mut curr = start;
            for pt in points_on_seg {
                // Ensure min length
                 if (pt.x - curr.x).powi(2) + (pt.y - curr.y).powi(2) > tol * tol {
                     new_segments.push(Line::new(curr, pt));
                     curr = pt;
                 }
            }
            // Final segment
            if (segment.end.x - curr.x).powi(2) + (segment.end.y - curr.y).powi(2) > tol * tol {
                new_segments.push(Line::new(curr, segment.end));
            }
        }
        segments = new_segments;
    }

    // Final global dedup
    segments.par_sort_unstable_by(|a, b| {
        let sa = (a.start.x, a.start.y, a.end.x, a.end.y);
        let sb = (b.start.x, b.start.y, b.end.x, b.end.y);
        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });
    segments.dedup_by(|a, b| {
        let tol = 1e-10;
        (a.start.x - b.start.x).abs() < tol && (a.start.y - b.start.y).abs() < tol &&
        (a.end.x - b.end.x).abs() < tol && (a.end.y - b.end.y).abs() < tol
    });

    segments
}
