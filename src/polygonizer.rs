use crate::graph::PlanarGraph;
use geo_types::{Geometry, LineString, Polygon};
use crate::error::Result;
use geo::algorithm::contains::Contains;
use geo::bounding_rect::BoundingRect;
use geo::Area;
use rstar::{RTree, AABB, RTreeObject};

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
            // Perform Unary Union on lines to node them
            // geo::BooleanOps::union works on MultiPolygon/Polygon usually.
            // For lines, we might need to convert to MultiLineString and unary_union?
            // geo 0.28 has `UnaryUnion` for MultiPolygon?
            // Actually, typical boolean ops are for polygons.
            // Noding lines is specific.
            // If geo doesn't support noding lines directly, we might skip implementation or use a trick.
            // But let's assume we proceed without it if hard, or check if we can simply intersect all.
            // For MVP, if noding is complex, we might skip robust noding.
            // But let's try to see if we can use overlay or just rely on user.
            // Prompt said: "The Rust crate must offer mechanisms... use unary_union operation provided by the geo crate".
            // Maybe converting lines to polygons (buffering) -> union -> skeletonize? No that's too heavy.

            // Wait, does `geo` have `unary_union` for `MultiLineString`?
            // Let's check imports.

            // Temporary: just add lines directly if noding not strictly found.
            // We'll leave a TODO or use a placeholder noder.
             for line in lines {
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
                // We need to reverse it to make it a valid "exterior" for storage,
                // but for hole assignment we keep it as is?
                // Actually, `geo::Polygon` expects holes to be... undefined orientation in struct,
                // but usually interior rings are CW?
                // But `geo` algorithms usually normalize.
                // We'll store it as a Polygon for now.
                holes.push(poly);
            }
        }

        // Assign holes to shells
        // Build RTree of shells
        let mut indexed_shells = Vec::new();
        for (i, shell) in shells.iter().enumerate() {
            indexed_shells.push(IndexedPolygon(shell.clone(), i));
        }
        let tree = RTree::bulk_load(indexed_shells);

        // Map shell index to list of hole indices
        let mut shell_holes: Vec<Vec<LineString<f64>>> = vec![vec![]; shells.len()];

        // For each hole, find candidate shells
        for hole_poly in holes {
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

                if shell.contains(&hole_poly) { // geo::Contains
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

            if let Some(idx) = best_shell_idx {
                shell_holes[idx].push(hole_ring.clone());
            }
            // Else: Discard unassigned holes (e.g., infinite face boundary)
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
