use crate::error::Result;
use crate::graph::PlanarGraph;
use geo::bounding_rect::BoundingRect;
use geo::Area;
use geo::Contains;
use geo_types::{Coord, Geometry, Line, LineString, Polygon};
use rstar::{RTree, RTreeObject, AABB};
use std::collections::HashSet;

use crate::noding::snap::SnapNoder;
use crate::utils::simd::SimdRing;
use crate::utils::z_order_index;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

// Wrapper for Polygon to be indexable by rstar
struct IndexedEnvelope {
    aabb: AABB<[f64; 2]>,
    index: usize,
}

impl RTreeObject for IndexedEnvelope {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb
    }
}

/// A robust polygonizer that reconstructs polygons from a set of lines.
///
/// The `Polygonizer` takes a collection of geometries (LineStrings, Polygons, etc.),
/// extracts all line segments, and reconstructs valid polygons from the linework.
/// It handles complex topologies such as:
/// - Nested holes
/// - Disconnected components (islands)
/// - Self-intersecting lines (if `node_input` is enabled)
/// - Overlapping polygons
///
/// # Example
///
/// ```rust
/// use geo_polygonize_core::Polygonizer;
/// use geo_types::{LineString, Geometry};
/// use geo::Area;
///
/// let mut polygonizer = Polygonizer::new();
///
/// // Add a square
/// polygonizer.add_geometry(Geometry::LineString(LineString::from(vec![
///     (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)
/// ])));
///
/// let result = polygonizer.polygonize().expect("Polygonization failed");
/// let polygons = result.polygons;
///
/// // Should find 1 polygon with area 100.0
/// assert_eq!(polygons.len(), 1);
/// assert!((polygons[0].unsigned_area() - 100.0).abs() < 1e-6);
/// ```
pub struct Polygonizer {
    graph: PlanarGraph,
    // Configuration
    /// Whether to check if rings are valid (closed, simple) before processing.
    /// Default: `true`.
    pub check_valid_rings: bool,
    /// Whether to node the input lines.
    ///
    /// If `true`, the polygonizer will use Iterated Snap Rounding to find and split
    /// intersecting lines. This is required if the input lines are not already noded
    /// (i.e., if they cross each other without a node at the intersection).
    /// Default: `false`.
    pub node_input: bool,
    /// The grid size used for snapping during noding.
    ///
    /// Points are snapped to this grid resolution to ensure robustness.
    /// Default: `1e-10`.
    pub snap_grid_size: f64,

    /// Whether to extract only disjoint, outer-most polygonal shells.
    ///
    /// If `true`, the polygonizer will discard any shells that are contained
    /// within other shells, returning only the top-level disjoint polygons.
    /// Default: `false`.
    pub extract_only_polygonal: bool,

    // Buffer for inputs if noding is required
    inputs: Vec<Geometry<f64>>,
    // Additional buffer for explicit line segments (e.g., from FFI)
    input_lines: Vec<Line<f64>>,
    dirty: bool,
}

pub struct PolygonizerResult {
    pub polygons: Vec<geo_types::Polygon<f64>>,
    pub dangles: Vec<geo_types::LineString<f64>>,
    pub invalid_rings: Vec<geo_types::LineString<f64>>,
}

impl Default for Polygonizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Polygonizer {
    /// Creates a new `Polygonizer` with default configuration.
    pub fn new() -> Self {
        Self {
            graph: PlanarGraph::new(),
            check_valid_rings: true,
            node_input: false,
            snap_grid_size: 1e-10, // Default tolerance
            extract_only_polygonal: false,
            inputs: Vec::new(),
            input_lines: Vec::new(),
            dirty: false,
        }
    }

    /// Sets the snap grid size for noding.
    ///
    /// # Arguments
    ///
    /// * `grid_size` - The size of the grid cells. Smaller values mean higher precision but potential for robustness issues if too small.
    pub fn with_snap_grid(mut self, grid_size: f64) -> Self {
        self.snap_grid_size = grid_size;
        self
    }

    /// Adds a geometry to the graph.
    ///
    /// This method accepts any `geo_types::Geometry`. Nested collections (GeometryCollection,
    /// MultiLineString, MultiPolygon) are flattened and all lineal components are extracted.
    pub fn add_geometry(&mut self, geom: Geometry<f64>) {
        self.inputs.push(geom);
        self.dirty = true;
    }

    /// Adds a collection of explicit line segments to the graph.
    ///
    /// This is useful for FFI or cases where you have raw segments.
    pub fn add_lines(&mut self, lines: Vec<Line<f64>>) {
        self.input_lines.extend(lines);
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
                let pa = a.0.first().cloned().unwrap_or(Coord { x: 0., y: 0. });
                let pb = b.0.first().cloned().unwrap_or(Coord { x: 0., y: 0. });
                pa.x.total_cmp(&pb.x).then(pa.y.total_cmp(&pb.y))
            });
            lines.dedup();

            // Convert LineStrings to Lines
            let mut input_segments = Vec::new();
            for ls in lines {
                for line in ls.lines() {
                    input_segments.push(line);
                }
            }
            // Add explicit lines
            input_segments.extend(self.input_lines.iter().cloned());

            // OPTIMIZATION: Spatial Sort (Z-Order)
            // This improves cache locality for both the Grid and the SIMD noder.
            let mut numbered_lines: Vec<(u64, Line<f64>)> = input_segments
                .iter()
                .map(|l| (z_order_index(l.start), *l))
                .collect();

            // Unstable sort is faster and sufficient
            numbered_lines.sort_unstable_by_key(|k| k.0);

            input_segments = numbered_lines.into_iter().map(|k| k.1).collect();

            let noder = SnapNoder::new(self.snap_grid_size);
            segments = noder.node(input_segments);
        } else {
            for ls in lines {
                for line in ls.lines() {
                    segments.push(line);
                }
            }
            // Add explicit lines
            segments.extend(self.input_lines.iter().cloned());
        }

        // Use bulk load
        self.graph.bulk_load(segments);

        self.dirty = false;
        Ok(())
    }

    /// Computes the polygons.
    /// This is the main entry point.
    ///
    /// Returns a `PolygonizerResult` containing polygons and dangles.
    pub fn polygonize(&mut self) -> Result<PolygonizerResult> {
        self.build_graph()?;

        // 1. Sort edges (Geometry Graph operation)
        self.graph.sort_edges();

        // 2. Prune dangles
        let mut dangles = self.graph.prune_dangles();

        // 3. Find rings
        // Extracts all minimal cycles from the planar graph.
        let rings = self.graph.get_edge_rings();

        // 3b. Find cut edges (unvisited edges)
        let mut cut_edges = self.graph.get_cut_edges();
        dangles.append(&mut cut_edges);

        // 4. Classify Rings (Shell vs Hole)
        // Standard GEOS behavior:
        // - CCW rings (positive signed area) are Shells.
        // - CW rings (negative signed area) are Holes (or the exterior of the universe).
        let mut shells = Vec::new();
        let mut holes = Vec::new();
        let mut invalid_rings_candidates = Vec::new();

        shells.reserve(rings.len() / 2);
        holes.reserve(rings.len() / 2);

        for ring in rings {
            // Note: LineString::signed_area() might return 0 even if closed in some geo versions/contexts?
            // Safer to wrap in Polygon which guarantees area calculation logic for rings.
            // Polygon::new is cheap (moves LineString).
            let poly = Polygon::new(ring, vec![]);
            let area = poly.signed_area();

            if !area.is_finite() || area.abs() < 1e-9 {
                invalid_rings_candidates.push(poly); // Degenerate or invalid
                continue;
            }

            if area > 0.0 {
                // CCW -> Shell
                shells.push(poly);
            } else {
                // CW -> Hole
                holes.push(poly);
            }
        }

        // NOTE: Previous heuristic to promote CW rings to Shells if !has_twin is removed.
        // We explicitly rely on the topological relationship (containment) to assign holes.
        // Any CW ring that is not contained in a Shell (e.g. Universe hole) will be discarded
        // during the assignment phase or filtered out.

        // 5. Establish Topology (Assign Holes to Shells)
        // A hole is assigned to the shell that contains it.
        // If multiple shells contain it, it is assigned to the one with the smallest area (deepest).

        // Precompute SIMD shells for fast inclusion checks
        let mut simd_shells: Vec<SimdRing> = shells
            .iter()
            .map(|s| SimdRing::new(&s.exterior().0))
            .collect();

        // Build RTree for shells to optimize spatial lookups
        let mut indexed_shells = Vec::with_capacity(shells.len());
        for (i, shell) in shells.iter().enumerate() {
            if let Some(bbox) = shell.bounding_rect() {
                let aabb =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
                indexed_shells.push(IndexedEnvelope { aabb, index: i });
            }
        }
        let mut tree = RTree::bulk_load(indexed_shells);

        // Filter shells if requested (Extract Only Polygonal)
        if self.extract_only_polygonal {
            let mut keep_mask = vec![true; shells.len()];
            let mut removed_count = 0;
            let mut discarded_edges = HashSet::new();

            // Precompute probe points
            let probe_points: Vec<Option<geo_types::Point<f64>>> =
                shells.iter().map(guaranteed_interior_probe).collect();

            // We can iterate indices because the tree uses indices into the original `shells` vector
            for (i, shell) in shells.iter().enumerate() {
                let bbox = match shell.bounding_rect() {
                    Some(b) => b,
                    None => {
                        keep_mask[i] = false;
                        removed_count += 1;
                        continue;
                    }
                };
                let aabb =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

                let candidates = tree.locate_in_envelope_intersecting(&aabb);
                let probe = probe_points[i];

                if let Some(probe_pt) = probe {
                    for cand in candidates {
                        let j = cand.index;
                        if i == j {
                            continue;
                        }

                        // Check if shell[i] is inside shell[j]
                        if simd_shells[j].contains(probe_pt.0) {
                            let area_i = shell.unsigned_area();
                            let area_j = shells[j].unsigned_area();

                            // Discard i if it is inside j.
                            // Use area and index as tie-breakers for duplicates.
                            if area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i) {
                                keep_mask[i] = false;
                                removed_count += 1;
                                break;
                            }
                        }
                    }
                } else {
                    // No valid interior point found (collapsed?), remove it
                    keep_mask[i] = false;
                    removed_count += 1;
                }
            }

            if removed_count > 0 {
                // Rebuild shells vector
                let mut iter = shells.into_iter();
                shells = keep_mask
                    .into_iter()
                    .filter_map(|keep| {
                        let s = iter.next().unwrap();
                        if keep {
                            Some(s)
                        } else {
                            // If we discard a shell, we should also track its edge to discard the corresponding hole
                            // (which would otherwise form a void in the parent shell).
                            let ext = s.exterior();
                            if ext.0.len() >= 2 {
                                let p1 = ext.0[0];
                                let p2 = ext.0[1];
                                discarded_edges.insert((
                                    (p1.x.to_bits(), p1.y.to_bits()),
                                    (p2.x.to_bits(), p2.y.to_bits()),
                                ));
                            }
                            None
                        }
                    })
                    .collect();

                // Also filter holes that correspond to discarded shells
                if !discarded_edges.is_empty() {
                    holes.retain(|h| {
                        // Check if hole has any edge that matches a discarded shell's representative edge (reversed)
                        // Discarded shell has edge u->v. Hole should have v->u.
                        // We stored u->v. We check if hole has v->u.
                        // We iterate all edges of hole.
                        let coords = &h.exterior().0;
                        for k in 0..coords.len().saturating_sub(1) {
                            let p_start = coords[k];
                            let p_end = coords[k + 1];
                            // Hole edge: p_start -> p_end.
                            // We look for u->v in discarded set where u=p_end, v=p_start.
                            let key = (
                                (p_end.x.to_bits(), p_end.y.to_bits()),     // u
                                (p_start.x.to_bits(), p_start.y.to_bits()), // v
                            );
                            if discarded_edges.contains(&key) {
                                return false; // Discard hole
                            }
                        }
                        true
                    });
                }

                // Rebuild helper structures for hole assignment
                simd_shells = shells
                    .iter()
                    .map(|s| SimdRing::new(&s.exterior().0))
                    .collect();

                let mut indexed_shells = Vec::with_capacity(shells.len());
                for (i, shell) in shells.iter().enumerate() {
                    if let Some(bbox) = shell.bounding_rect() {
                        let aabb = AABB::from_corners(
                            [bbox.min().x, bbox.min().y],
                            [bbox.max().x, bbox.max().y],
                        );
                        indexed_shells.push(IndexedEnvelope { aabb, index: i });
                    }
                }
                tree = RTree::bulk_load(indexed_shells);
            }
        }

        // Process hole assignment
        let process_hole_assignment =
            |hole_poly: Polygon<f64>| -> Option<(usize, LineString<f64>)> {
                let bbox = hole_poly.bounding_rect()?;
                let hole_aabb =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

                let candidates = tree.locate_in_envelope_intersecting(&hole_aabb);

                let mut best_shell_idx = None;
                let mut min_area = f64::MAX;

                let probe_point = guaranteed_interior_probe(&hole_poly)?;

                for cand in candidates {
                    let idx = cand.index;
                    // Use SIMD check first
                    let simd_shell = &simd_shells[idx];

                    if simd_shell.contains(probe_point.0) {
                        let shell = &shells[idx];

                        // GEOS topology strictness:
                        // - Point-touch between hole and shell is valid and kept.
                        // - Edge-sharing invalidates the hole assignment here and we drop the hole.
                        if rings_share_edge(shell.exterior(), hole_poly.exterior(), 1e-10) {
                            continue;
                        }

                        let area = shell.unsigned_area();
                        let hole_area = hole_poly.unsigned_area();

                        // Only assign if shell is larger than hole (and not equal, to skip Universe hole matching Shell)
                        if area > hole_area + 1e-6 && area < min_area {
                            min_area = area;
                            best_shell_idx = Some(idx);
                        }
                    }
                }

                best_shell_idx.map(|idx| {
                    let (ext, _) = hole_poly.into_inner();
                    (idx, ext)
                })
            };

        let assignments: Vec<_>;
        #[cfg(feature = "parallel")]
        {
            assignments = holes
                .into_par_iter()
                .filter_map(process_hole_assignment)
                .collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            assignments = holes
                .into_iter()
                .filter_map(process_hole_assignment)
                .collect();
        }

        // Group holes by shell
        let mut shell_holes: Vec<Vec<LineString<f64>>> = vec![vec![]; shells.len()];
        for (idx, hole) in assignments {
            shell_holes[idx].push(hole);
        }

        // 6. Construct Final Polygons
        let mut result = Vec::new();
        for (shell, holes) in shells.into_iter().zip(shell_holes.into_iter()) {
            let (exterior, _) = shell.into_inner();
            let p = Polygon::new(exterior, holes);
            // Filter out empty/degenerate polygons
            if p.unsigned_area() > 1e-6 {
                result.push(p);
            }
        }

        // Ensure we don't crash on NaNs during processing
        let invalid_rings = process_invalid_rings(invalid_rings_candidates);

        Ok(PolygonizerResult {
            polygons: result,
            dangles,
            invalid_rings,
        })
    }
}

/// Sorts invalid rings by bounding box area and filters out inner redundant rings.
/// Ported from GEOS `extractInvalidLines`.
fn process_invalid_rings(rings: Vec<Polygon<f64>>) -> Vec<LineString<f64>> {
    // Separate rings with NaN/Inf coordinates from processable ones to avoid panics in geo algorithms
    let (mut processable, others): (Vec<_>, Vec<_>) = rings.into_iter().partition(|p| {
        p.exterior()
            .0
            .iter()
            .all(|c| c.x.is_finite() && c.y.is_finite())
    });

    // 1. Sort by bounding box area (descending)
    // GEOS sorts by Envelope area.
    processable.sort_by(|a, b| {
        let area_a = a
            .bounding_rect()
            .map(|b| (b.max().x - b.min().x) * (b.max().y - b.min().y))
            .unwrap_or(0.0);
        let area_b = b
            .bounding_rect()
            .map(|b| (b.max().x - b.min().x) * (b.max().y - b.min().y))
            .unwrap_or(0.0);
        area_b
            .partial_cmp(&area_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 2. Filter inner redundant rings
    let mut valid_rings = Vec::with_capacity(processable.len());
    for ring in processable {
        let is_contained = valid_rings.iter().any(|existing: &Polygon<f64>| {
            // Check if ring is contained in existing
            existing.contains(&ring)
        });

        if !is_contained {
            valid_rings.push(ring);
        }
    }

    let mut result: Vec<LineString<f64>> =
        valid_rings.into_iter().map(|p| p.into_inner().0).collect();

    // Append the ones we couldn't process safely
    result.extend(others.into_iter().map(|p| p.into_inner().0));

    result
}

fn guaranteed_interior_probe(poly: &Polygon<f64>) -> Option<geo_types::Point<f64>> {
    let ring = poly.exterior();
    let coords = &ring.0;
    if coords.len() < 4 {
        return None;
    }

    let unique_n = coords.len().saturating_sub(1);
    if unique_n < 3 {
        return None;
    }

    let area = poly.signed_area();
    if !area.is_finite() || area.abs() < 1e-12 {
        return None;
    }

    let hole_simd = SimdRing::new(coords);
    let diag = poly
        .bounding_rect()
        .map(|b| {
            let dx = b.max().x - b.min().x;
            let dy = b.max().y - b.min().y;
            (dx * dx + dy * dy).sqrt()
        })
        .unwrap_or(1.0);
    let eps = (diag * 1e-9).max(1e-10);

    for i in 0..unique_n {
        let prev = coords[(i + unique_n - 1) % unique_n];
        let curr = coords[i];
        let next = coords[(i + 1) % unique_n];

        let in_edge = Coord {
            x: curr.x - prev.x,
            y: curr.y - prev.y,
        };
        let out_edge = Coord {
            x: next.x - curr.x,
            y: next.y - curr.y,
        };

        let in_len = (in_edge.x * in_edge.x + in_edge.y * in_edge.y).sqrt();
        let out_len = (out_edge.x * out_edge.x + out_edge.y * out_edge.y).sqrt();
        if in_len < 1e-12 || out_len < 1e-12 {
            continue;
        }

        let turn = in_edge.x * out_edge.y - in_edge.y * out_edge.x;
        let convex = if area > 0.0 {
            turn > 1e-12
        } else {
            turn < -1e-12
        };
        if !convex {
            continue;
        }

        let to_prev = Coord {
            x: (prev.x - curr.x) / in_len,
            y: (prev.y - curr.y) / in_len,
        };
        let to_next = Coord {
            x: (next.x - curr.x) / out_len,
            y: (next.y - curr.y) / out_len,
        };

        let bisector = Coord {
            x: to_prev.x + to_next.x,
            y: to_prev.y + to_next.y,
        };
        let bisector_len = (bisector.x * bisector.x + bisector.y * bisector.y).sqrt();
        if bisector_len < 1e-12 {
            continue;
        }

        let bisector_unit = Coord {
            x: bisector.x / bisector_len,
            y: bisector.y / bisector_len,
        };

        for sign in [1.0, -1.0] {
            let candidate = Coord {
                x: curr.x + sign * bisector_unit.x * eps,
                y: curr.y + sign * bisector_unit.y * eps,
            };
            if hole_simd.contains(candidate) {
                return Some(geo_types::Point(candidate));
            }
        }
    }

    Some(geo_types::Point(coords[0]))
}

fn rings_share_edge(shell: &LineString<f64>, hole: &LineString<f64>, eps: f64) -> bool {
    if shell.0.len() < 2 || hole.0.len() < 2 {
        return false;
    }

    let shell_n = shell.0.len() - 1;
    let hole_n = hole.0.len() - 1;

    for i in 0..shell_n {
        let a1 = shell.0[i];
        let a2 = shell.0[i + 1];
        for j in 0..hole_n {
            let b1 = hole.0[j];
            let b2 = hole.0[j + 1];
            if segments_overlap_with_length(a1, a2, b1, b2, eps) {
                return true;
            }
        }
    }

    false
}

fn segments_overlap_with_length(
    a1: Coord<f64>,
    a2: Coord<f64>,
    b1: Coord<f64>,
    b2: Coord<f64>,
    eps: f64,
) -> bool {
    let ax = a2.x - a1.x;
    let ay = a2.y - a1.y;
    let a_len_sq = ax * ax + ay * ay;
    if a_len_sq <= eps * eps {
        return false;
    }

    // Collinearity checks for segment B endpoints against segment A line
    let cross_b1 = ax * (b1.y - a1.y) - ay * (b1.x - a1.x);
    let cross_b2 = ax * (b2.y - a1.y) - ay * (b2.x - a1.x);
    let tol = eps * a_len_sq.sqrt();
    if cross_b1.abs() > tol || cross_b2.abs() > tol {
        return false;
    }

    let t1 = ((b1.x - a1.x) * ax + (b1.y - a1.y) * ay) / a_len_sq;
    let t2 = ((b2.x - a1.x) * ax + (b2.y - a1.y) * ay) / a_len_sq;

    let min_t = t1.min(t2);
    let max_t = t1.max(t2);
    let overlap_start = 0.0_f64.max(min_t);
    let overlap_end = 1.0_f64.min(max_t);

    overlap_end - overlap_start > eps
}

fn extract_lines(geom: &Geometry<f64>, out: &mut Vec<LineString<f64>>) {
    match geom {
        Geometry::LineString(ls) => out.push(ls.clone()),
        Geometry::MultiLineString(mls) => {
            out.extend(mls.0.clone());
        }
        Geometry::Polygon(poly) => {
            out.push(poly.exterior().clone());
            out.extend(poly.interiors().iter().cloned());
        }
        Geometry::MultiPolygon(mpoly) => {
            for poly in mpoly {
                out.push(poly.exterior().clone());
                out.extend(poly.interiors().iter().cloned());
            }
        }
        Geometry::GeometryCollection(gc) => {
            for g in gc {
                extract_lines(g, out);
            }
        }
        _ => {}
    }
}
