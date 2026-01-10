use crate::graph::PlanarGraph;
use geo_types::{Geometry, LineString, Polygon, Coord};
use crate::error::Result;
use geo::algorithm::contains::Contains;
use geo::bounding_rect::BoundingRect;
use geo::algorithm::line_intersection::LineIntersection;
use geo::algorithm::intersects::Intersects;
use geo::Area;
use geo::Line;
use rstar::{RTree, AABB, RTreeObject};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::cmp::Ordering;
use smallvec::SmallVec;

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
             // Deduplicate identical inputs before expensive noding
             lines.sort_by(|a, b| {
                 // Simple sort
                 let pa = a.0.first().cloned().unwrap_or(Coord{x:0.,y:0.});
                 let pb = b.0.first().cloned().unwrap_or(Coord{x:0.,y:0.});
                 pa.x.partial_cmp(&pb.x).unwrap_or(Ordering::Equal)
                    .then(pa.y.partial_cmp(&pb.y).unwrap_or(Ordering::Equal))
             });
             lines.dedup();

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
        let process_holes = |hole: &Polygon<f64>| -> Option<Polygon<f64>> {
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
        };

        let promoted_shells: Vec<_>;
        #[cfg(feature = "parallel")]
        {
            promoted_shells = holes.par_iter().filter_map(process_holes).collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            promoted_shells = holes.iter().filter_map(process_holes).collect();
        }

        shells.extend(promoted_shells);

        // Assign holes to shells
        let mut indexed_shells = Vec::new();
        for (i, shell) in shells.iter().enumerate() {
            indexed_shells.push(IndexedPolygon(shell.clone(), i));
        }
        let tree = RTree::bulk_load(indexed_shells);

        let process_hole_assignment = |hole_poly: &Polygon<f64>| -> Option<(usize, LineString<f64>)> {
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
        };

        let assignments: Vec<_>;
        #[cfg(feature = "parallel")]
        {
            assignments = holes.par_iter().filter_map(process_hole_assignment).collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            assignments = holes.iter().filter_map(process_hole_assignment).collect();
        }

        let mut shell_holes: Vec<Vec<LineString<f64>>> = vec![vec![]; shells.len()];
        for (idx, hole) in assignments {
            shell_holes[idx].push(hole);
        }

        let mut result = Vec::new();
        for (i, shell) in shells.into_iter().enumerate() {
            let holes = shell_holes[i].clone();
            let poly = Polygon::new(shell.exterior().clone(), holes);
            // Filter out polygons with negligible area (e.g. collapsed shells or shells completely filled by holes)
            if poly.unsigned_area() > 1e-6 {
                result.push(poly);
            }
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
    let indexed_segments: Vec<IndexedLine>;
    #[cfg(feature = "parallel")]
    {
        indexed_segments = segments.par_iter().enumerate()
            .map(|(i, s)| IndexedLine { line: *s, index: i })
            .collect();
    }
    #[cfg(not(feature = "parallel"))]
    {
        indexed_segments = segments.iter().enumerate()
            .map(|(i, s)| IndexedLine { line: *s, index: i })
            .collect();
    }

    let tree = RTree::bulk_load(indexed_segments);

    // 2. Find ALL intersection events using bulk query
    // Returns a flat list of (segment_index, split_point)
    // We use intersection_candidates_with_other_tree which is usually optimized for internal node checks.

    // Closure to process intersection
    let process_intersection = |(cand1, cand2): (&IndexedLine, &IndexedLine)| -> SmallVec<[(usize, Coord<f64>); 2]> {
        let idx1 = cand1.index;
        let idx2 = cand2.index;

        // Optimization: only process unique pairs
        if idx1 >= idx2 { return SmallVec::new(); }

        let s1 = cand1.line;
        let s2 = cand2.line;

        let mut events = SmallVec::new();

        // Fast check before robust intersection
        if !s1.intersects(&s2) { return events; }

        if let Some(res) = geo::algorithm::line_intersection::line_intersection(s1, s2) {
            match res {
                LineIntersection::SinglePoint { intersection: pt, .. } => {
                    // Check strict internal (robustness)
                    let is_internal_s1 = (pt.x - s1.start.x).abs() > tol && (pt.x - s1.end.x).abs() > tol
                                      || (pt.y - s1.start.y).abs() > tol && (pt.y - s1.end.y).abs() > tol;
                    let is_internal_s2 = (pt.x - s2.start.x).abs() > tol && (pt.x - s2.end.x).abs() > tol
                                      || (pt.y - s2.start.y).abs() > tol && (pt.y - s2.end.y).abs() > tol;

                    if is_internal_s1 { events.push((idx1, pt)); }
                    if is_internal_s2 { events.push((idx2, pt)); }
                },
                LineIntersection::Collinear { intersection: overlap } => {
                    // Add overlap endpoints as split points if internal
                    let p1 = overlap.start;
                    let p2 = overlap.end;

                    let s1_has_p1 = (p1.x - s1.start.x).abs() > tol && (p1.x - s1.end.x).abs() > tol || (p1.y - s1.start.y).abs() > tol && (p1.y - s1.end.y).abs() > tol;
                    let s1_has_p2 = (p2.x - s1.start.x).abs() > tol && (p2.x - s1.end.x).abs() > tol || (p2.y - s1.start.y).abs() > tol && (p2.y - s1.end.y).abs() > tol;

                    if s1_has_p1 { events.push((idx1, p1)); }
                    if s1_has_p2 { events.push((idx1, p2)); }

                    let s2_has_p1 = (p1.x - s2.start.x).abs() > tol && (p1.x - s2.end.x).abs() > tol || (p1.y - s2.start.y).abs() > tol && (p1.y - s2.end.y).abs() > tol;
                    let s2_has_p2 = (p2.x - s2.start.x).abs() > tol && (p2.x - s2.end.x).abs() > tol || (p2.y - s2.start.y).abs() > tol && (p2.y - s2.end.y).abs() > tol;

                    if s2_has_p1 { events.push((idx2, p1)); }
                    if s2_has_p2 { events.push((idx2, p2)); }
                }
            }
        }
        events
    };

    let intersection_events: Vec<(usize, Coord<f64>)>;

    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
         // Heuristic: Don't spin up Rayon for small candidate sets (if we can estimate).
         // But we can't estimate intersection count easily without collecting.
         // However, we can collect candidates first (Native only) and check size.
         let candidates: Vec<_> = tree.intersection_candidates_with_other_tree(&tree).collect();

         if candidates.len() > 1000 {
             intersection_events = candidates.into_par_iter()
                .flat_map_iter(|(cand1, cand2)| process_intersection((cand1, cand2)))
                .collect();
         } else {
             intersection_events = candidates.into_iter()
                .flat_map(|(cand1, cand2)| process_intersection((cand1, cand2)))
                .collect();
         }
    }

    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
         // Stream processing for Wasm (Low Memory Profile) or Sequential
         intersection_events = tree.intersection_candidates_with_other_tree(&tree)
            .flat_map(|(cand1, cand2)| process_intersection((cand1, cand2)))
            .collect();
    }

    // 3. Apply splits
    if !intersection_events.is_empty() {
        // 1. Sort events by Segment Index
        let mut events = intersection_events;

        // Helper to sort events
        let sort_events = |a: &(usize, Coord<f64>), b: &(usize, Coord<f64>)| {
            a.0.cmp(&b.0)
                .then_with(|| {
                     // Secondary sort by distance along segment?
                     // Or just coordinate sort is enough for dedup
                     a.1.x.partial_cmp(&b.1.x).unwrap_or(Ordering::Equal)
                })
        };

        #[cfg(feature = "parallel")]
        events.par_sort_unstable_by(sort_events);

        #[cfg(not(feature = "parallel"))]
        events.sort_unstable_by(sort_events);

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

    // Snap Vertices (Robustness for near-miss intersections)
    let mut points: Vec<_> = segments.iter().enumerate().flat_map(|(i, s)| {
        vec![
            (s.start, i, true), // coord, seg_idx, is_start
            (s.end, i, false)
        ]
    }).collect();

    points.sort_by(|a, b| {
        a.0.x.partial_cmp(&b.0.x).unwrap_or(Ordering::Equal)
            .then(a.0.y.partial_cmp(&b.0.y).unwrap_or(Ordering::Equal))
    });

    let mut merged = vec![false; points.len()];

    for i in 0..points.len() {
        if merged[i] { continue; }

        let rep = points[i].0;

        // Look ahead
        for j in (i + 1)..points.len() {
             if points[j].0.x - rep.x > tol {
                 break;
             }
             if merged[j] { continue; }

             if (points[j].0.x - rep.x).abs() < tol && (points[j].0.y - rep.y).abs() < tol {
                 // Merge
                 merged[j] = true;
                 let (seg_idx, is_start) = (points[j].1, points[j].2);
                 if is_start {
                     segments[seg_idx].start = rep;
                 } else {
                     segments[seg_idx].end = rep;
                 }
             }
        }
    }

    // Normalize segment direction to ensure deduplication works for reverse duplicates
    for segment in &mut segments {
        if segment.start.x > segment.end.x || ((segment.start.x - segment.end.x).abs() < 1e-12 && segment.start.y > segment.end.y) {
             let temp = segment.start;
             segment.start = segment.end;
             segment.end = temp;
        }
    }

    // Final global dedup
    #[cfg(feature = "parallel")]
    segments.par_sort_unstable_by(|a, b| {
        let sa = (a.start.x, a.start.y, a.end.x, a.end.y);
        let sb = (b.start.x, b.start.y, b.end.x, b.end.y);
        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });
    #[cfg(not(feature = "parallel"))]
    segments.sort_unstable_by(|a, b| {
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
