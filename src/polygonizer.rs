use crate::graph::PlanarGraph;
use geo_types::{Geometry, LineString, Polygon, Coord, Point};
use crate::error::Result;
use geo::bounding_rect::BoundingRect;
use geo::Area;
use geo::algorithm::centroid::Centroid;

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::cmp::Ordering;
use crate::noding::snap::SnapNoder;
use geo_index::rtree::RTreeIndex; // Import trait for search
use crate::utils::simd::SimdRing;

pub struct Polygonizer {
    graph: PlanarGraph,
    // Configuration
    pub check_valid_rings: bool,
    pub node_input: bool,
    pub snap_grid_size: f64,

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
            snap_grid_size: 1e-10, // Default tolerance
            inputs: Vec::new(),
            dirty: false,
        }
    }

    pub fn with_snap_grid(mut self, grid_size: f64) -> Self {
        self.snap_grid_size = grid_size;
        self
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

            // Convert LineStrings to Lines
            let mut input_segments = Vec::new();
            for ls in lines {
                for line in ls.lines() {
                    input_segments.push(line);
                }
            }

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

        // Precompute SIMD shells
        let simd_shells: Vec<SimdRing> = shells.iter()
            .map(|s| SimdRing::new(&s.exterior().0))
            .collect();

        // Assign holes to shells using geo-index (Packed RTree)
        // Optimization: Only build tree if we have enough shells to justify it (and avoid panics on small inputs)
        let tree = if shells.len() >= 50 {
            let mut builder = geo_index::rtree::RTreeBuilder::new(shells.len());
            for shell in &shells {
                let bbox = shell.bounding_rect().unwrap();
                builder.add(bbox.min().x, bbox.min().y, bbox.max().x, bbox.max().y);
            }
            Some(builder.finish::<geo_index::rtree::sort::HilbertSort>())
        } else {
            None
        };

        // Process holes
        let process_hole_assignment = |hole_poly: &Polygon<f64>| -> Option<(usize, LineString<f64>)> {
            let hole_ring = hole_poly.exterior();
            let bbox = hole_poly.bounding_rect().unwrap();

            let candidates_indices = if let Some(tree) = &tree {
                tree.search(bbox.min().x, bbox.min().y, bbox.max().x, bbox.max().y)
            } else {
                // Linear scan for small sets
                (0..shells.len()).collect()
            };

            let mut best_shell_idx = None;
            let mut min_area = f64::MAX;

            // Use centroid for inclusion check to avoid boundary issues
            let probe_point = hole_poly.centroid().unwrap_or_else(|| {
                // Fallback to first point if centroid fails (e.g. degenerate)
                Point(hole_ring.0[0])
            });

            for idx in candidates_indices {
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
            let p = Polygon::new(shell.exterior().clone(), holes);
            // Filter out collapsed polygons (e.g. shells completely filled by holes)
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
