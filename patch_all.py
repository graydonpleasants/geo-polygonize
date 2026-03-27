import re
import os

with open("crates/geo-polygonize-core/src/graph/planar_graph.rs", "r") as f:
    content = f.read()

# Add deleted to Edge
content = content.replace(
"""    pub is_marked: bool,
}""",
"""    pub is_marked: bool,
    /// Flag indicating if the edge was dynamically removed.
    pub deleted: bool,
}""")

# Update create_edge_components
content = content.replace(
"""        is_marked: false,
    };""",
"""        is_marked: false,
        deleted: false,
    };""")

# Add add_line, remove_line_by_id, and reset_traversal_state to PlanarGraph
methods_code = """
    /// Dynamically adds a single line segment to the graph.
    /// Uses `add_node` internally which handles 2D deduplication.
    pub fn add_line(&mut self, line: Line3D) {
        if line.start.x == line.end.x && line.start.y == line.end.y {
            return;
        }

        let u = self.add_node(line.start);
        let v = self.add_node(line.end);

        let edge_idx = self.edges.len();
        let de_u_v_idx = self.directed_edges.len();
        let de_v_u_idx = de_u_v_idx + 1;

        let de_u_v = DirectedEdge {
            src: u,
            dst: v,
            edge_idx,
            sym_idx: de_v_u_idx,
            is_visited: false,
            is_marked: false,
            edge_direction: true,
        };

        let de_v_u = DirectedEdge {
            src: v,
            dst: u,
            edge_idx,
            sym_idx: de_u_v_idx,
            is_visited: false,
            is_marked: false,
            edge_direction: false,
        };

        let edge = Edge {
            line,
            dir_edges: [de_u_v_idx, de_v_u_idx],
            is_marked: false,
            deleted: false,
        };

        self.directed_edges.push(de_u_v);
        self.directed_edges.push(de_v_u);
        self.edges.push(edge);

        self.nodes_outgoing[u].push(de_u_v_idx);
        self.nodes_outgoing[v].push(de_v_u_idx);

        self.nodes_degree[u] += 1;
        self.nodes_degree[v] += 1;
    }

    /// Dynamically removes a line segment from the graph by its line_id.
    pub fn remove_line_by_id(&mut self, line_id: u32) -> bool {
        let mut found = false;
        for edge in &mut self.edges {
            if !edge.deleted && edge.line.line_id == line_id {
                edge.deleted = true;
                found = true;
            }
        }
        found
    }

    /// Resets graph traversal state and recalculates nodes_degree to ignore deleted edges.
    pub fn reset_traversal_state(&mut self) {
        for de in &mut self.directed_edges {
            de.is_visited = false;
            de.is_marked = false;
        }
        for e in &mut self.edges {
            e.is_marked = false;
        }
        for marked in &mut self.nodes_marked {
            *marked = false;
        }
        // Recalculate degree based on non-deleted outgoing edges
        for (i, outgoing) in self.nodes_outgoing.iter().enumerate() {
            let mut degree = 0;
            for &de_idx in outgoing {
                let edge_idx = self.directed_edges[de_idx].edge_idx;
                if !self.edges[edge_idx].deleted {
                    degree += 1;
                }
            }
            self.nodes_degree[i] = degree;
        }
    }
"""

content = content.replace("    pub fn sort_edges(&mut self) {", methods_code + "\n    pub fn sort_edges(&mut self) {")

# Update prune_dangles to ignore deleted edges
content = content.replace(
"""                let edge_idx = self.directed_edges[de_idx].edge_idx;
                let line = self.edges[edge_idx].line;""",
"""                let edge_idx = self.directed_edges[de_idx].edge_idx;
                if self.edges[edge_idx].deleted {
                    continue;
                }
                let line = self.edges[edge_idx].line;""")

# Update get_cut_edges to ignore deleted edges
content = content.replace(
"""            let de1 = &self.directed_edges[edge.dir_edges[0]];
            let de2 = &self.directed_edges[edge.dir_edges[1]];

            if de1.is_marked || de2.is_marked {""",
"""            if edge.deleted {
                continue;
            }
            let de1 = &self.directed_edges[edge.dir_edges[0]];
            let de2 = &self.directed_edges[edge.dir_edges[1]];

            if de1.is_marked || de2.is_marked {""")

# Update compute_next_cw_edges to ignore deleted edges
content = content.replace(
"""                    .iter()
                    .copied()
                    .filter(|&idx| !self.directed_edges[idx].is_marked),""",
"""                    .iter()
                    .copied()
                    .filter(|&idx| {
                        !self.directed_edges[idx].is_marked
                            && !self.edges[self.directed_edges[idx].edge_idx].deleted
                    }),""")

# Update extract_valid_rings to ignore deleted edges
content = content.replace(
"""        for start_de_idx in 0..self.directed_edges.len() {
            if self.directed_edges[start_de_idx].is_visited
                || self.directed_edges[start_de_idx].is_marked
            {
                continue;
            }""",
"""        for start_de_idx in 0..self.directed_edges.len() {
            if self.directed_edges[start_de_idx].is_visited
                || self.directed_edges[start_de_idx].is_marked
                || self.edges[self.directed_edges[start_de_idx].edge_idx].deleted
            {
                continue;
            }""")

content = content.replace(
"""            self.edges.push(Edge {
                line: Line3D::new(p0.into(), p1.into(), 0),
                dir_edges: [de_u_v_idx, de_v_u_idx],
                is_marked: false,
            });""",
"""            self.edges.push(Edge {
                line: Line3D::new(p0.into(), p1.into(), 0),
                dir_edges: [de_u_v_idx, de_v_u_idx],
                is_marked: false,
                deleted: false,
            });""")

with open("crates/geo-polygonize-core/src/graph/planar_graph.rs", "w") as f:
    f.write(content)


# Update index.rs with PartialEq for IndexedEnvelope
with open("crates/geo-polygonize-core/src/index.rs", "r") as f:
    content = f.read()

content = content.replace(
"""pub struct IndexedEnvelope {
    pub aabb: AABB<[f64; 2]>,
    pub index: usize,
}""",
"""#[derive(Clone, Copy, Debug)]
pub struct IndexedEnvelope {
    pub aabb: AABB<[f64; 2]>,
    pub index: usize,
}

impl PartialEq for IndexedEnvelope {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.aabb.lower() == other.aabb.lower()
            && self.aabb.upper() == other.aabb.upper()
    }
}

impl Eq for IndexedEnvelope {}""")

# Add insert and remove to SpatialIndexBackend
content = content.replace(
"""impl SpatialIndex2D for SpatialIndexBackend {
    fn locate_in_envelope_intersecting<'a>(""",
"""impl SpatialIndexBackend {
    pub fn insert(&mut self, env: IndexedEnvelope) {
        match self {
            SpatialIndexBackend::RStar(backend) => backend.insert(env),
            SpatialIndexBackend::PackedNative(_) => {
                // Not supported
            }
        }
    }

    pub fn remove(&mut self, env: &IndexedEnvelope) {
        match self {
            SpatialIndexBackend::RStar(backend) => backend.remove(env),
            SpatialIndexBackend::PackedNative(_) => {
                // Not supported
            }
        }
    }
}

impl SpatialIndex2D for SpatialIndexBackend {
    fn locate_in_envelope_intersecting<'a>(""")

content = content.replace(
"""impl RStarBackend {
    pub fn new(envelopes: Vec<IndexedEnvelope>) -> Self {""",
"""impl RStarBackend {
    pub fn insert(&mut self, env: IndexedEnvelope) {
        self.tree.insert(env);
    }

    pub fn remove(&mut self, env: &IndexedEnvelope) {
        self.tree.remove(env);
    }

    pub fn new(envelopes: Vec<IndexedEnvelope>) -> Self {""")

with open("crates/geo-polygonize-core/src/index.rs", "w") as f:
    f.write(content)


# ContainmentForest
with open("crates/geo-polygonize-core/src/containment.rs", "r") as f:
    content = f.read()

content = content.replace(
"""pub struct ContainmentForest {
    pub tree: SpatialIndexBackend,
    pub simd_shells: Vec<SimdRing>,
    // Cache exterior areas to avoid O(N) recalculations of `exterior_unsigned_area_2d()` inside the tree intersection loops.
    pub shell_areas: Vec<f64>,
}""",
"""pub struct ContainmentForest {
    pub tree: SpatialIndexBackend,
    pub simd_shells: Vec<Option<SimdRing>>,
    // Cache exterior areas to avoid O(N) recalculations of `exterior_unsigned_area_2d()` inside the tree intersection loops.
    pub shell_areas: Vec<Option<f64>>,
}""")

content = content.replace(
"""        let simd_shells: Vec<SimdRing>;
        let shell_areas: Vec<f64>;
        #[cfg(feature = "parallel")]
        {
            (simd_shells, shell_areas) = shells
                .par_iter()
                .map(|s| (SimdRing::new_3d(&s.exterior), s.exterior_unsigned_area_2d()))
                .unzip();
        }
        #[cfg(not(feature = "parallel"))]
        {
            (simd_shells, shell_areas) = shells
                .iter()
                .map(|s| (SimdRing::new_3d(&s.exterior), s.exterior_unsigned_area_2d()))
                .unzip();
        }""",
"""        let simd_shells_raw: Vec<SimdRing>;
        let shell_areas_raw: Vec<f64>;
        #[cfg(feature = "parallel")]
        {
            (simd_shells_raw, shell_areas_raw) = shells
                .par_iter()
                .map(|s| (SimdRing::new_3d(&s.exterior), s.exterior_unsigned_area_2d()))
                .unzip();
        }
        #[cfg(not(feature = "parallel"))]
        {
            (simd_shells_raw, shell_areas_raw) = shells
                .iter()
                .map(|s| (SimdRing::new_3d(&s.exterior), s.exterior_unsigned_area_2d()))
                .unzip();
        }
        let simd_shells = simd_shells_raw.into_iter().map(Some).collect();
        let shell_areas = shell_areas_raw.into_iter().map(Some).collect();""")

content = content.replace(
"""    pub fn contains(&self, shell_idx: usize, hole_probe: &Coord3D, hole_ring: &[Coord3D]) -> bool {
        let simd_shell = &self.simd_shells[shell_idx];""",
"""    pub fn contains(&self, shell_idx: usize, hole_probe: &Coord3D, hole_ring: &[Coord3D]) -> bool {
        let simd_shell = self.simd_shells[shell_idx].as_ref().unwrap();""")

content = content.replace(
"""    pub fn contains_shell(&self, outer_idx: usize, inner_shell: &[Coord3D]) -> bool {
        let outer_area = self.shell_areas[outer_idx];
        let inner_area = Polygon3D::ring_signed_area_2d(inner_shell).abs();

        if inner_area >= outer_area {
            return false;
        }""",
"""    pub fn contains_shell(&self, outer_idx: usize, inner_shell: &[Coord3D]) -> bool {
        let outer_area = self.shell_areas[outer_idx].unwrap();
        let inner_area = Polygon3D::ring_signed_area_2d(inner_shell).abs();

        if inner_area >= outer_area {
            return false;
        }""")

content = content.replace(
"""        let simd_shell = &self.simd_shells[outer_idx];""",
"""        let simd_shell = self.simd_shells[outer_idx].as_ref().unwrap();""")

content = content.replace(
"""                let area_i = self.shell_areas[i];
                let area_j = self.shell_areas[j];

                if area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i) {""",
"""                let area_i = self.shell_areas[i].unwrap();
                let area_j = self.shell_areas[j].unwrap();

                if area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i) {""")

content = content.replace(
"""                    let simd_shell = &self.simd_shells[j];

                    if simd_shell.contains(probe_pt.0) {
                        // Using cached areas instead of `shell.exterior_unsigned_area_2d()`
                        let area_i = self.shell_areas[i];
                        let area_j = self.shell_areas[j];

                        // If i is strictly contained inside j, increment container count
                        if area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i) {""",
"""                    let simd_shell = self.simd_shells[j].as_ref().unwrap();

                    if simd_shell.contains(probe_pt.0) {
                        // Using cached areas instead of `shell.exterior_unsigned_area_2d()`
                        let area_i = self.shell_areas[i].unwrap();
                        let area_j = self.shell_areas[j].unwrap();

                        // If i is strictly contained inside j, increment container count
                        if area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i) {""")

content = content.replace(
"""            let area = self.shell_areas[cand_idx];
            let simd_shell = &self.simd_shells[cand_idx];

            if simd_shell.contains(probe_point.0) {
                // If it contains the hole and is the smallest containing shell found so far
                if area > hole_area + 1e-6 && area < min_area {
                    // Check touch policy for holes""",
"""            let area = self.shell_areas[cand_idx].unwrap();
            let simd_shell = self.simd_shells[cand_idx].as_ref().unwrap();

            if simd_shell.contains(probe_point.0) {
                // If it contains the hole and is the smallest containing shell found so far
                if area > hole_area + 1e-6 && area < min_area {
                    // Check touch policy for holes""")

with open("crates/geo-polygonize-core/src/containment.rs", "w") as f:
    f.write(content)


# Update utils.rs to export hash_ring and hash_polygon
with open("crates/geo-polygonize-core/src/utils/mod.rs", "r") as f:
    content = f.read()

if "pub fn hash_ring" not in content:
    content += """
use std::collections::hash_map::DefaultHasher as AHasher;
use std::hash::{Hash, Hasher};

pub fn hash_ring(ids: &[u32]) -> u64 {
    let mut hasher = AHasher::default();
    ids.hash(&mut hasher);
    hasher.finish()
}

use crate::types::Polygon3D;
pub fn hash_polygon(poly: &Polygon3D) -> u64 {
    let mut hasher = AHasher::default();
    poly.exterior_ids.hash(&mut hasher);
    for holes in &poly.interiors_ids {
        holes.hash(&mut hasher);
    }
    hasher.finish()
}
"""
    with open("crates/geo-polygonize-core/src/utils/mod.rs", "w") as f:
        f.write(content)

with open("crates/geo-polygonize-core/src/polygonizer.rs", "r") as f:
    poly_content = f.read()

poly_content = poly_content.replace("fn extract_and_classify_rings(", "pub fn extract_and_classify_rings(")
poly_content = poly_content.replace("fn establish_topology(", "pub fn establish_topology(")
poly_content = poly_content.replace("fn construct_final_polygons(", "pub fn construct_final_polygons(")

with open("crates/geo-polygonize-core/src/polygonizer.rs", "w") as f:
    f.write(poly_content)

stateful_code = """use crate::error::{PolygonizeError, Result};
use crate::graph::PlanarGraph;
use crate::options::PolygonizerOptions;
use crate::polygonizer::{construct_final_polygons, establish_topology, extract_and_classify_rings};
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
        poly.add_line(Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(10.0, 0.0, 0.0), 1));
        poly.add_line(Line3D::new(Coord3D::new(10.0, 0.0, 0.0), Coord3D::new(10.0, 10.0, 0.0), 2));
        poly.add_line(Line3D::new(Coord3D::new(10.0, 10.0, 0.0), Coord3D::new(0.0, 10.0, 0.0), 3));
        poly.add_line(Line3D::new(Coord3D::new(0.0, 10.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 4));

        let update1 = poly.update().unwrap();
        assert_eq!(update1.added_polygons.len(), 1);
        assert_eq!(update1.removed_polygons.len(), 0);

        // Remove a line
        poly.remove_line_by_id(4);
        let update2 = poly.update().unwrap();
        assert_eq!(update2.added_polygons.len(), 0);
        assert_eq!(update2.removed_polygons.len(), 1);

        // Add it back
        poly.add_line(Line3D::new(Coord3D::new(0.0, 10.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 4));
        let update3 = poly.update().unwrap();
        assert_eq!(update3.added_polygons.len(), 1);
        assert_eq!(update3.removed_polygons.len(), 0);
    }
}
"""
with open("crates/geo-polygonize-core/src/stateful.rs", "w") as f:
    f.write(stateful_code)

with open("crates/geo-polygonize-core/src/lib.rs", "r") as f:
    content = f.read()

if "pub mod stateful;" not in content:
    content += "\npub mod stateful;\n"

with open("crates/geo-polygonize-core/src/lib.rs", "w") as f:
    f.write(content)
