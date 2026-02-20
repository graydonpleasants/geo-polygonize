use crate::error::Result;
use crate::graph::PlanarGraph;
use geo::algorithm::centroid::Centroid;
use geo::bounding_rect::BoundingRect;
use geo::Area;
use geo_types::{Coord, Geometry, Line, LineString, Point, Polygon};
use rstar::{RTree, RTreeObject, AABB};

use crate::noding::snap::SnapNoder;
use crate::utils::simd::SimdRing;
use crate::utils::z_order_index;
#[cfg(feature = "parallel")]
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
/// use geo_polygonize::Polygonizer;
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
/// let polygons = polygonizer.polygonize().expect("Polygonization failed");
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

    // Buffer for inputs if noding is required
    inputs: Vec<Geometry<f64>>,
    dirty: bool,
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
            inputs: Vec::new(),
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
        }

        // Use bulk load
        self.graph.bulk_load(segments);

        self.dirty = false;
        Ok(())
    }

    /// Computes the polygons.
    /// This is the main entry point.
    ///
    /// Returns a vector of `geo_types::Polygon<f64>`.
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
                (shell.unsigned_area() - hole_area).abs() < 1e-6
                    && shell.bounding_rect() == hole.bounding_rect()
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

        // Precompute SIMD shells
        let simd_shells: Vec<SimdRing> = shells
            .iter()
            .map(|s| SimdRing::new(&s.exterior().0))
            .collect();

        // Assign holes to shells using RTree (Dynamic, but robust)
        let mut indexed_shells = Vec::with_capacity(shells.len());
        for (i, shell) in shells.iter().enumerate() {
            indexed_shells.push(IndexedPolygon(shell.clone(), i));
        }
        let tree = RTree::bulk_load(indexed_shells);

        // Process holes
        let process_hole_assignment =
            |hole_poly: &Polygon<f64>| -> Option<(usize, LineString<f64>)> {
                let hole_ring = hole_poly.exterior();
                let bbox = hole_poly.bounding_rect().unwrap();
                let hole_aabb =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

                let candidates = tree.locate_in_envelope_intersecting(&hole_aabb);

                let mut best_shell_idx = None;
                let mut min_area = f64::MAX;

                // Use centroid for inclusion check to avoid boundary issues
                let probe_point = hole_poly.centroid().unwrap_or_else(|| {
                    // Fallback to first point if centroid fails (e.g. degenerate)
                    Point(hole_ring.0[0])
                });

                for cand in candidates {
                    let idx = cand.1;
                    // Use SIMD check first
                    let simd_shell = &simd_shells[idx];

                    if simd_shell.contains(probe_point.0) {
                        let shell = &shells[idx];
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
            assignments = holes
                .par_iter()
                .filter_map(process_hole_assignment)
                .collect();
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
        for (shell, holes) in shells.into_iter().zip(shell_holes.into_iter()) {
            let p = Polygon::new(shell.exterior().clone(), holes);
            if p.unsigned_area() > 1e-6 {
                result.push(p);
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
