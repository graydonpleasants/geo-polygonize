use crate::containment::ContainmentForest;
use crate::error::Result;
use crate::graph::{EdgeId, PlanarGraph};
use crate::options::PolygonizerOptions;
use crate::polygonizer::{
    apply_determinism, construct_final_polygons, extract_and_classify_rings, process_invalid_rings,
};
use crate::types::{Coord3D, Line3D, Polygon3D};
use geo_types::Polygon;

#[derive(Clone, Debug, serde::Serialize)]
pub struct PolygonizerUpdate {
    pub added: Vec<Polygon3D>,
    pub removed: Vec<Polygon3D>,
}

pub struct StatefulPolygonizer {
    pub graph: PlanarGraph,
    pub options: PolygonizerOptions,
    pub last_polygons: Vec<Polygon3D>,
}

impl StatefulPolygonizer {
    pub fn new(options: PolygonizerOptions) -> Self {
        Self {
            graph: PlanarGraph::new(),
            options,
            last_polygons: Vec::new(),
        }
    }

    pub fn add_line(&mut self, line: Line3D) -> EdgeId {
        self.graph.add_line(line)
    }

    pub fn remove_line(&mut self, line_id: u32) -> bool {
        self.graph.remove_line_by_id(line_id)
    }

    pub fn update(&mut self) -> Result<PolygonizerUpdate> {
        // Reset traversal state
        self.graph.reset_traversal_state();

        // Sort edges
        self.graph.sort_edges();

        // Prune dangles
        let mut dangles = self.graph.prune_dangles();

        // Find rings
        let rings_with_ids = self.graph.get_edge_rings();

        // Find cut edges
        let mut cut_edges = self.graph.get_cut_edges();
        dangles.append(&mut cut_edges);

        // Classify Rings
        let (shells, holes, invalid_rings_candidates) = extract_and_classify_rings(rings_with_ids);

        // Establish Topology
        let mut forest = ContainmentForest::new(&shells, &self.options.containment.index_backend);

        let mut filtered_shells = shells;
        if self.options.extract_only_polygonal {
            let keep_mask =
                forest.filter_polygonal(&filtered_shells, &self.options.containment.touch_policy);
            let mut new_shells = Vec::new();
            for (keep, s) in keep_mask.into_iter().zip(filtered_shells) {
                if keep {
                    new_shells.push(s);
                }
            }
            filtered_shells = new_shells;
            forest =
                ContainmentForest::new(&filtered_shells, &self.options.containment.index_backend);
        }

        let process_hole_assignment =
            |hole_3d: Polygon3D| -> Option<(usize, Vec<Coord3D>, Vec<u32>)> {
                let best_shell_idx = forest.assign_hole(
                    &hole_3d,
                    &filtered_shells,
                    &self.options.containment.touch_policy,
                );
                best_shell_idx.map(|idx| (idx, hole_3d.exterior, hole_3d.exterior_ids))
            };

        let assignments: Vec<_> = holes
            .into_iter()
            .filter_map(process_hole_assignment)
            .collect();

        let mut shell_holes: Vec<Vec<Vec<Coord3D>>> = vec![vec![]; filtered_shells.len()];
        let mut shell_holes_ids: Vec<Vec<Vec<u32>>> = vec![vec![]; filtered_shells.len()];

        for (idx, hole_coords, hole_ids) in assignments {
            shell_holes[idx].push(hole_coords);
            shell_holes_ids[idx].push(hole_ids);
        }

        // Construct Final Polygons
        let mut result =
            construct_final_polygons(filtered_shells, shell_holes, shell_holes_ids, &self.options);

        let mut invalid_rings = if invalid_rings_candidates.is_empty() {
            Vec::new()
        } else {
            let shells_2d: Vec<Polygon<f64>> = result.iter().map(|s| s.to_polygon_2d()).collect();
            process_invalid_rings(invalid_rings_candidates, &shells_2d)
        };

        result = apply_determinism(result, &mut dangles, &mut invalid_rings, &self.options);

        // Compute Delta
        let new_polygons = result;
        let mut added = Vec::new();
        let mut removed = Vec::new();

        // Simplistic diff by exterior coords comparison.
        // For production, we should hash or have ID mappings.
        for old_poly in &self.last_polygons {
            let found = new_polygons.iter().any(|new_poly| {
                new_poly.exterior == old_poly.exterior && new_poly.interiors == old_poly.interiors
            });
            if !found {
                removed.push(old_poly.clone());
            }
        }

        for new_poly in &new_polygons {
            let found = self.last_polygons.iter().any(|old_poly| {
                old_poly.exterior == new_poly.exterior && old_poly.interiors == new_poly.interiors
            });
            if !found {
                added.push(new_poly.clone());
            }
        }

        self.last_polygons = new_polygons;

        Ok(PolygonizerUpdate { added, removed })
    }
}
