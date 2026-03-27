use crate::error::{PolygonizeError, Result};
use crate::graph::PlanarGraph;
use crate::options::PolygonizerOptions;
use crate::polygonizer::{
    construct_final_polygons, establish_topology, extract_and_classify_rings,
};
use crate::types::{Coord3D, Line3D, Polygon3D};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, serde::Serialize)]
pub struct PolygonizerUpdate {
    pub added_polygons: Vec<Polygon3D>,
    pub removed_polygons: Vec<Polygon3D>,
}

pub struct StatefulPolygonizer {
    graph: PlanarGraph,
    options: PolygonizerOptions,
    last_rings: HashMap<u64, (Vec<Coord3D>, Vec<u32>)>,
    last_shells: HashMap<u64, Polygon3D>,
}

impl StatefulPolygonizer {
    pub fn new(options: PolygonizerOptions) -> Self {
        Self {
            graph: PlanarGraph::new(),
            options,
            last_rings: HashMap::new(),
            last_shells: HashMap::new(),
        }
    }

    pub fn add_line(&mut self, line: Line3D) {
        self.graph.add_line(line);
    }

    pub fn remove_line_by_id(&mut self, line_id: u32) -> bool {
        self.graph.remove_line_by_id(line_id)
    }

    pub fn update(&mut self) -> Result<PolygonizerUpdate> {
        if self.options.node_input {
            return Err(PolygonizeError::UnsupportedOptionCombination {
                reason: "Incremental updates are not supported when node_input is true".to_string(),
            });
        }

        self.graph.reset_traversal_state();
        self.graph.sort_edges();
        self.graph.prune_dangles();

        let new_rings_with_ids = self.graph.get_edge_rings();

        // 1. Compute added and removed rings
        let mut new_rings_map = HashMap::new();
        for ring in new_rings_with_ids {
            let hash = crate::utils::hash_ring(&ring.1);
            new_rings_map.insert(hash, ring);
        }

        let mut added_rings = Vec::new();
        let mut retained_hashes = HashSet::new();

        for (hash, ring) in &new_rings_map {
            if !self.last_rings.contains_key(hash) {
                added_rings.push(ring.clone());
            } else {
                retained_hashes.insert(*hash);
            }
        }

        let mut removed_rings_hashes = Vec::new();
        for hash in self.last_rings.keys() {
            if !retained_hashes.contains(hash) {
                removed_rings_hashes.push(*hash);
            }
        }

        if added_rings.is_empty() && removed_rings_hashes.is_empty() {
            return Ok(PolygonizerUpdate {
                added_polygons: vec![],
                removed_polygons: vec![],
            });
        }

        self.last_rings = new_rings_map.clone();

        let (shells, holes, _) = extract_and_classify_rings(new_rings_map.into_values().collect());

        let (shells, shell_holes, shell_holes_ids) =
            establish_topology(shells, holes, &self.options);
        let polygons =
            construct_final_polygons(shells, shell_holes, shell_holes_ids, &self.options);

        // Diff the polygons
        let mut added_polygons = Vec::new();
        let mut removed_polygons = Vec::new();

        // Hash polygons
        let mut new_poly_hashes = HashMap::new();
        for poly in polygons {
            let hash = crate::utils::hash_polygon(&poly);
            new_poly_hashes.insert(hash, poly);
        }

        let mut old_poly_hashes = HashMap::new();
        for poly in self.last_shells.values() {
            let hash = crate::utils::hash_polygon(poly);
            old_poly_hashes.insert(hash, poly.clone());
        }

        for (hash, poly) in &new_poly_hashes {
            if !old_poly_hashes.contains_key(hash) {
                added_polygons.push(poly.clone());
            }
        }

        for (hash, poly) in old_poly_hashes {
            if !new_poly_hashes.contains_key(&hash) {
                removed_polygons.push(poly);
            }
        }

        self.last_shells = new_poly_hashes;

        Ok(PolygonizerUpdate {
            added_polygons,
            removed_polygons,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::PolygonizerOptions;
    use crate::types::{Coord3D, Line3D};

    #[test]
    fn test_incremental_polygonizer() {
        let mut poly = StatefulPolygonizer::new(PolygonizerOptions {
            node_input: false,
            ..Default::default()
        });

        // Add a square
        poly.add_line(Line3D::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            1,
        ));
        poly.add_line(Line3D::new(
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            2,
        ));
        poly.add_line(Line3D::new(
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            3,
        ));
        poly.add_line(Line3D::new(
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            4,
        ));

        let update1 = poly.update().unwrap();
        assert_eq!(update1.added_polygons.len(), 1);
        assert_eq!(update1.removed_polygons.len(), 0);

        // Remove a line
        poly.remove_line_by_id(4);
        let update2 = poly.update().unwrap();
        assert_eq!(update2.added_polygons.len(), 0);
        assert_eq!(update2.removed_polygons.len(), 1);

        // Add it back
        poly.add_line(Line3D::new(
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            4,
        ));
        let update3 = poly.update().unwrap();
        assert_eq!(update3.added_polygons.len(), 1);
        assert_eq!(update3.removed_polygons.len(), 0);
    }
}
