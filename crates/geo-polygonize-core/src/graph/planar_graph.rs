use crate::types::{Coord3D, Line3D};
use crate::utils::parallel::{
    par_flat_map, par_into_enumerate_map, par_sort_unstable, par_zip_for_each,
};
use crate::utils::{compare_angular, z_order_index};
use geo_types::{Coord, LineString};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;

thread_local! {
    static NEXT_POINTERS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// Index of a node in the graph.
pub type NodeId = usize;
/// Index of an undirected edge in the graph.
pub type EdgeId = usize;
/// Index of a directed half-edge in the graph.
pub type DirEdgeId = usize;

/// An undirected edge in the planar graph.
#[derive(Clone, Debug)]
pub struct Edge {
    /// The geometry of the edge.
    /// In JTS this might be a full LineString, but for the graph we mainly care about connectivity.
    /// We store Line to reduce heap allocations compared to LineString.
    pub line: Line3D,
    /// Indices of the two directed edges associated with this undirected edge.
    pub dir_edges: [DirEdgeId; 2],
    /// Flag indicating if the edge is marked (e.g. visited or pruned).
    pub is_marked: bool,
}

/// A directed half-edge in the planar graph.
#[derive(Clone, Debug)]
pub struct DirectedEdge {
    /// Source node index.
    pub src: NodeId,
    /// Destination node index.
    pub dst: NodeId,
    /// Reference to the parent geometry (undirected edge).
    pub edge_idx: EdgeId,
    /// Index of the symmetric (reverse) edge.
    pub sym_idx: DirEdgeId,
    /// Traversal state: has this edge been processed into a ring?
    pub is_visited: bool,
    /// Is this edge explicitly marked (e.g. as part of a dangle).
    pub is_marked: bool,
    /// Orientation in the parent LineString (true: same direction, false: opposite).
    pub edge_direction: bool,
}

/// A Planar Graph implementation using an arena-based index approach.
///
/// This structure represents the topological graph of the line segments.
/// Instead of pointer-based structures, it uses `Vec` arenas for Nodes, Edges, and DirectedEdges,
/// referencing them via integer indices (`NodeId`, `EdgeId`, `DirEdgeId`).
/// This layout is cache-friendly and plays well with Rust's ownership model.
#[derive(Clone)]
pub struct PlanarGraph {
    /// Node coordinates (X). Index is `NodeId`.
    pub nodes_x: Vec<f64>,
    /// Node coordinates (Y). Index is `NodeId`.
    pub nodes_y: Vec<f64>,
    /// Node coordinates (Z). Index is `NodeId`.
    pub nodes_z: Vec<f64>,
    /// Node adjacency lists. Index is `NodeId`.
    /// Stores the list of outgoing `DirEdgeId`s for each node.
    pub nodes_outgoing: Vec<Vec<DirEdgeId>>,
    /// Node connectivity degrees. Index is `NodeId`.
    pub nodes_degree: Vec<usize>,
    /// Node marked flags. Index is `NodeId`.
    pub nodes_marked: Vec<bool>,

    /// All undirected edges (geometry owners). Index is `EdgeId`.
    pub edges: Vec<Edge>,
    /// All directed half-edges. Index is `DirEdgeId`.
    pub directed_edges: Vec<DirectedEdge>,
    /// Lookup map to dedup nodes during construction.
    /// OPTIMIZATION: Used only for incremental additions. Bulk load bypasses this.
    pub node_map: HashMap<NodeKey, NodeId>,
}

// Wrapper for Coord to be Hashable (since f64 is not Hash)
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct NodeKey(i64, i64);

impl From<Coord<f64>> for NodeKey {
    fn from(c: Coord<f64>) -> Self {
        // Simple quantization for map lookup.
        NodeKey(c.x.to_bits() as i64, c.y.to_bits() as i64)
    }
}

struct NodeEntry {
    z_idx: u64, // Z-order index
    c: Coord3D,
}

impl PartialEq for NodeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.z_idx == other.z_idx && self.c.x == other.c.x && self.c.y == other.c.y
    }
}
impl Eq for NodeEntry {}

impl PartialOrd for NodeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.z_idx.cmp(&other.z_idx).then_with(|| {
            self.c
                .x
                .total_cmp(&other.c.x)
                .then(self.c.y.total_cmp(&other.c.y))
        })
    }
}

fn create_edge_components(
    i: usize,
    u: NodeId,
    v: NodeId,
    line: Line3D,
    edges_start_len: usize,
    dir_edges_start_len: usize,
) -> (
    NodeId,
    NodeId,
    DirEdgeId,
    DirEdgeId,
    DirectedEdge,
    DirectedEdge,
    Edge,
) {
    let edge_idx = edges_start_len + i;
    let de_u_v_idx = dir_edges_start_len + 2 * i;
    let de_v_u_idx = dir_edges_start_len + 2 * i + 1;

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
    };

    (u, v, de_u_v_idx, de_v_u_idx, de_u_v, de_v_u, edge)
}

impl Default for PlanarGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanarGraph {
    /// Creates a new, empty PlanarGraph.
    pub fn new() -> Self {
        Self {
            nodes_x: Vec::new(),
            nodes_y: Vec::new(),
            nodes_z: Vec::new(),
            nodes_outgoing: Vec::new(),
            nodes_degree: Vec::new(),
            nodes_marked: Vec::new(),
            edges: Vec::new(),
            directed_edges: Vec::new(),
            node_map: HashMap::new(),
        }
    }

    /// Adds a node at the given coordinate, returning its NodeId.
    /// Deduplicates nodes using a HashMap lookup (2D only).
    pub fn add_node(&mut self, coord: Coord3D) -> NodeId {
        let key = NodeKey::from(coord.to_coord_2d());
        if let Some(&id) = self.node_map.get(&key) {
            return id;
        }

        let id = self.nodes_x.len();
        self.nodes_x.push(coord.x);
        self.nodes_y.push(coord.y);
        self.nodes_z.push(coord.z);
        self.nodes_outgoing.push(Vec::new());
        self.nodes_degree.push(0);
        self.nodes_marked.push(false);
        self.node_map.insert(key, id);
        id
    }

    /// Bulk loads edges into the graph.
    /// This is significantly faster than `add_line_string` for large datasets as it avoids HashMap lookups.
    pub fn bulk_load(&mut self, lines: Vec<Line3D>) {
        if lines.is_empty() {
            return;
        }

        // 1. Collect all coordinates and precompute Z-order
        let to_entries = |line: &Line3D| {
            [
                NodeEntry {
                    z_idx: z_order_index(line.start.to_coord_2d()),
                    c: line.start,
                },
                NodeEntry {
                    z_idx: z_order_index(line.end.to_coord_2d()),
                    c: line.end,
                },
            ]
        };

        let mut entries: Vec<NodeEntry> = par_flat_map(&lines, to_entries);

        // 2. Sort using precomputed Z-order
        par_sort_unstable(&mut entries);

        // Dedup using exact equality on X,Y. Z is ignored for dedup key but carried.
        // `NodeEntry` PartialEq/Ord implementation considers X,Y.
        // We need to ensure we don't have duplicates with same X,Y but different Z.
        // `dedup_by` keeps the first one.
        entries.dedup_by(|a, b| a.c.x == b.c.x && a.c.y == b.c.y);

        // 3. Build Nodes
        let start_node_idx = self.nodes_x.len();
        self.nodes_x.reserve(entries.len());
        self.nodes_y.reserve(entries.len());
        self.nodes_z.reserve(entries.len());
        self.nodes_outgoing.reserve(entries.len());
        self.nodes_degree.reserve(entries.len());
        self.nodes_marked.reserve(entries.len());

        for entry in &entries {
            self.nodes_x.push(entry.c.x);
            self.nodes_y.push(entry.c.y);
            self.nodes_z.push(entry.c.z);
            self.nodes_outgoing.push(Vec::new());
            self.nodes_degree.push(0);
            self.nodes_marked.push(false);
        }

        // Helper to find node index using precomputed Z array (entries)
        let get_node_id = |pt: Coord3D| -> Option<NodeId> {
            // Binary search must respect the sort order (Z-order)
            let z_pt = z_order_index(pt.to_coord_2d());

            // Binary search on the sorted entries
            let idx_res = entries.binary_search_by(|probe| {
                probe
                    .z_idx
                    .cmp(&z_pt)
                    .then_with(|| probe.c.x.total_cmp(&pt.x).then(probe.c.y.total_cmp(&pt.y)))
            });

            match idx_res {
                Ok(i) => Some(start_node_idx + i),
                Err(_) => None,
            }
        };

        // 4. Precompute Adjacency Lists sizes
        // We do a first pass to map endpoints to node IDs and count degrees.
        // This allows us to reserve exact capacity for outgoing_edges.
        // It also avoids repeated binary searches in the second pass.

        // Store valid edges as (u, v, line)
        let mut valid_edges = Vec::with_capacity(lines.len());
        let mut degrees = vec![0usize; self.nodes_x.len()]; // This might be large?

        for line in lines {
            let p0 = line.start;
            let p1 = line.end;

            if (p0.x - p1.x).abs() < 1e-12 && (p0.y - p1.y).abs() < 1e-12 {
                continue;
            }

            let u_opt = get_node_id(p0);
            let v_opt = get_node_id(p1);

            if let (Some(u), Some(v)) = (u_opt, v_opt) {
                valid_edges.push((u, v, line));
                degrees[u] += 1;
                degrees[v] += 1;
            }
        }

        // Reserve exact capacity
        par_zip_for_each(
            &mut self.nodes_outgoing,
            &degrees,
            |adj: &mut Vec<usize>, deg: &usize| {
                adj.reserve(*deg);
            },
        );

        // 5. Build Edges
        self.edges.reserve(valid_edges.len());
        self.directed_edges.reserve(valid_edges.len() * 2);

        let edges_start_len = self.edges.len();
        let directed_edges_start_len = self.directed_edges.len();

        let mapper = |(i, (u, v, line)): (usize, (NodeId, NodeId, Line3D))| {
            create_edge_components(i, u, v, line, edges_start_len, directed_edges_start_len)
        };

        let new_edges_data: Vec<_> = par_into_enumerate_map(valid_edges, mapper);

        for (u, v, de_u_v_idx, de_v_u_idx, de_u_v, de_v_u, edge) in new_edges_data {
            self.directed_edges.push(de_u_v);
            self.directed_edges.push(de_v_u);
            self.edges.push(edge);

            self.nodes_outgoing[u].push(de_u_v_idx);
            self.nodes_degree[u] += 1;
            self.nodes_outgoing[v].push(de_v_u_idx);
            self.nodes_degree[v] += 1;
        }
    }

    /// Adds a line string to the graph.
    pub fn add_line_string(&mut self, line: LineString<f64>) {
        if line.0.is_empty() {
            return;
        }

        let coords = &line.0;
        // Optimization: using .windows(2) is faster than loop indexing
        // because it eliminates array bounds checks.
        for w in coords.windows(2) {
            let p0 = w[0];
            let p1 = w[1];

            if (p0.x - p1.x).abs() < 1e-12 && (p0.y - p1.y).abs() < 1e-12 {
                continue;
            }

            let u = self.add_node(p0.into());
            let v = self.add_node(p1.into());

            let edge_idx = self.edges.len();

            let de_u_v_idx = self.directed_edges.len();
            let de_v_u_idx = self.directed_edges.len() + 1;

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

            self.directed_edges.push(de_u_v);
            self.directed_edges.push(de_v_u);

            self.edges.push(Edge {
                line: Line3D::new(p0.into(), p1.into(), 0),
                dir_edges: [de_u_v_idx, de_v_u_idx],
                is_marked: false,
            });

            self.nodes_outgoing[u].push(de_u_v_idx);
            self.nodes_degree[u] += 1;

            self.nodes_outgoing[v].push(de_v_u_idx);
            self.nodes_degree[v] += 1;
        }
    }

    /// Sorts all outgoing edges of all nodes by angle.
    pub fn sort_edges(&mut self) {
        let nodes_x = &self.nodes_x;
        let nodes_y = &self.nodes_y;
        let directed_edges = &self.directed_edges;

        // Use a robust angular comparator.
        // This requires accessing coordinates of src and dst nodes.
        #[cfg(feature = "parallel")]
        self.nodes_outgoing
            .par_iter_mut()
            .enumerate()
            .for_each(|(src_idx, adj)| {
                let center = Coord {
                    x: nodes_x[src_idx],
                    y: nodes_y[src_idx],
                };
                adj.sort_by(|&a_idx, &b_idx| {
                    let a_de = &directed_edges[a_idx];
                    let b_de = &directed_edges[b_idx];

                    // Get destination coordinates
                    let dst_a_idx = a_de.dst;
                    let dst_b_idx = b_de.dst;

                    let target_a = Coord {
                        x: nodes_x[dst_a_idx],
                        y: nodes_y[dst_a_idx],
                    };
                    let target_b = Coord {
                        x: nodes_x[dst_b_idx],
                        y: nodes_y[dst_b_idx],
                    };

                    compare_angular(center, target_a, target_b)
                });
            });

        #[cfg(not(feature = "parallel"))]
        self.nodes_outgoing
            .iter_mut()
            .enumerate()
            .for_each(|(src_idx, adj)| {
                let center = Coord {
                    x: nodes_x[src_idx],
                    y: nodes_y[src_idx],
                };
                adj.sort_by(|&a_idx, &b_idx| {
                    let a_de = &directed_edges[a_idx];
                    let b_de = &directed_edges[b_idx];

                    let dst_a_idx = a_de.dst;
                    let dst_b_idx = b_de.dst;

                    let target_a = Coord {
                        x: nodes_x[dst_a_idx],
                        y: nodes_y[dst_a_idx],
                    };
                    let target_b = Coord {
                        x: nodes_x[dst_b_idx],
                        y: nodes_y[dst_b_idx],
                    };

                    compare_angular(center, target_a, target_b)
                });
            });
    }

    /// Prunes dangles (nodes with degree 1) from the graph iteratively.
    pub fn prune_dangles(&mut self) -> Vec<Vec<Coord3D>> {
        let mut dangles = Vec::new();
        let mut to_process: Vec<NodeId> = self
            .nodes_degree
            .iter()
            .enumerate()
            .filter(|(i, &d)| d == 1 && !self.nodes_marked[*i])
            .map(|(i, _)| i)
            .collect();

        while let Some(node_idx) = to_process.pop() {
            if self.nodes_degree[node_idx] != 1 {
                continue;
            }

            self.nodes_marked[node_idx] = true;
            self.nodes_degree[node_idx] = 0;

            let mut edge_found = false;
            let mut neighbor_idx = 0;

            let mut found_de_idx = None;
            for &de_idx in &self.nodes_outgoing[node_idx] {
                if !self.directed_edges[de_idx].is_marked {
                    found_de_idx = Some(de_idx);
                    break;
                }
            }

            if let Some(de_idx) = found_de_idx {
                self.directed_edges[de_idx].is_marked = true;
                let sym_idx = self.directed_edges[de_idx].sym_idx;
                self.directed_edges[sym_idx].is_marked = true;

                // Capture the geometry
                let edge_idx = self.directed_edges[de_idx].edge_idx;
                let line = self.edges[edge_idx].line;
                dangles.push(vec![line.start, line.end]);

                neighbor_idx = self.directed_edges[de_idx].dst;
                edge_found = true;
            }

            if edge_found && self.nodes_degree[neighbor_idx] > 0 {
                self.nodes_degree[neighbor_idx] -= 1;
                if self.nodes_degree[neighbor_idx] == 1 && !self.nodes_marked[neighbor_idx] {
                    to_process.push(neighbor_idx);
                }
            }
        }
        dangles
    }

    /// Returns unvisited edges (neither marked as dangle nor visited by ring extraction).
    pub fn get_cut_edges(&self) -> Vec<Vec<Coord3D>> {
        let mut cuts = Vec::new();
        for edge in &self.edges {
            let de1 = &self.directed_edges[edge.dir_edges[0]];
            let de2 = &self.directed_edges[edge.dir_edges[1]];

            if de1.is_marked || de2.is_marked {
                continue;
            }

            if !de1.is_visited && !de2.is_visited {
                cuts.push(vec![edge.line.start, edge.line.end]);
            }
        }
        cuts
    }


    /// Extracts rings from the graph following the GEOS flow.
    pub fn get_edge_rings(&mut self) -> Vec<(Vec<Coord3D>, Vec<u32>)> {
        NEXT_POINTERS.with(|cell| {
            let mut next_pointers = cell.borrow_mut();
            next_pointers.clear();
            next_pointers.resize(self.directed_edges.len(), usize::MAX);

            let mut labels = vec![-1_i64; self.directed_edges.len()];

            // Step 1: computeNextCWEdges over every node.
            self.compute_next_cw_edges(&mut next_pointers);

            // Step 2: find and label maximal rings.
            let maximal_ring_starts = self.find_and_label_maximal_rings(&next_pointers, &mut labels);

            // Step 3: convert maximal to minimal rings by relinking intersection nodes.
            self.convert_maximal_to_minimal_rings(&maximal_ring_starts, &mut next_pointers, &labels);

            // Extract the minimal rings from the graph.
            self.extract_valid_rings(&next_pointers)
        })
    }

    /// Step 1: computeNextCWEdges over every node.
    /// Edges in nodes_outgoing are in CCW order. For each pair of consecutive outgoing
    /// edges (prev, curr), set next(sym(prev)) = curr, and close the cycle.
    fn compute_next_cw_edges(&self, next_pointers: &mut [usize]) {
        let mut valid_edges = Vec::new();
        for outgoing in &self.nodes_outgoing {
            valid_edges.clear();
            valid_edges.extend(
                outgoing
                    .iter()
                    .copied()
                    .filter(|&idx| !self.directed_edges[idx].is_marked),
            );

            if valid_edges.is_empty() {
                continue;
            }

            let mut next = *valid_edges.last().unwrap();
            for &curr in &valid_edges {
                next_pointers[curr] = next;
                next = curr;
            }
        }
    }

    /// Step 2: find and label maximal rings.
    fn find_and_label_maximal_rings(
        &self,
        next_pointers: &[usize],
        labels: &mut [i64],
    ) -> Vec<DirEdgeId> {
        let mut maximal_ring_starts = Vec::new();
        let mut curr_label = 1_i64;
        for start_de_idx in 0..self.directed_edges.len() {
            if self.directed_edges[start_de_idx].is_marked || labels[start_de_idx] >= 0 {
                continue;
            }

            maximal_ring_starts.push(start_de_idx);
            let mut curr = start_de_idx;
            loop {
                if labels[curr] >= 0 {
                    break;
                }
                labels[curr] = curr_label;

                let next = next_pointers[self.directed_edges[curr].sym_idx];
                if next == usize::MAX || next == start_de_idx {
                    break;
                }
                curr = next;
            }
            curr_label += 1;
        }
        maximal_ring_starts
    }

    /// Step 3: convert maximal to minimal rings by relinking intersection nodes.
    /// findIntersectionNodes + computeNextCCWEdges, scoped by ring label.
    fn convert_maximal_to_minimal_rings(
        &self,
        maximal_ring_starts: &[DirEdgeId],
        next_pointers: &mut [usize],
        labels: &[i64],
    ) {
        let mut intersection_nodes = Vec::<NodeId>::new();
        let mut seen_intersection_nodes = vec![false; self.nodes_x.len()];

        for &start_de_idx in maximal_ring_starts {
            let ring_label = labels[start_de_idx];
            if ring_label < 0 {
                continue;
            }

            intersection_nodes.clear();

            // findIntersectionNodes(startDE, label)
            let mut curr = start_de_idx;
            loop {
                let node = self.directed_edges[curr].src;

                // Degree of this node within the current ring label.
                let mut degree_for_label = 0;
                for &out_de in &self.nodes_outgoing[node] {
                    if labels[out_de] == ring_label {
                        degree_for_label += 1;
                    }
                }

                if degree_for_label > 1 && !seen_intersection_nodes[node] {
                    seen_intersection_nodes[node] = true;
                    intersection_nodes.push(node);
                }

                let next = next_pointers[self.directed_edges[curr].sym_idx];
                if next == usize::MAX || next == start_de_idx {
                    break;
                }
                curr = next;
            }

            // computeNextCCWEdges(node, label)
            for &node in &intersection_nodes {
                let outgoing = &self.nodes_outgoing[node];
                let mut first_out: Option<DirEdgeId> = None;
                let mut prev_in: Option<DirEdgeId> = None;

                // Traverse node star in reverse to process CCW linking semantics.
                for &de_idx in outgoing.iter().rev() {
                    let sym_idx = self.directed_edges[de_idx].sym_idx;

                    let out_de = (labels[de_idx] == ring_label).then_some(de_idx);
                    let in_de = (labels[sym_idx] == ring_label).then_some(sym_idx);

                    if out_de.is_none() && in_de.is_none() {
                        continue;
                    }

                    if let Some(in_de_idx) = in_de {
                        prev_in = Some(in_de_idx);
                    }

                    if let Some(out_de_idx) = out_de {
                        if let Some(prev_in_idx) = prev_in.take() {
                            next_pointers[self.directed_edges[prev_in_idx].sym_idx] = out_de_idx;
                        }
                        if first_out.is_none() {
                            first_out = Some(out_de_idx);
                        }
                    }
                }

                if let (Some(prev_in_idx), Some(first_out_idx)) = (prev_in, first_out) {
                    next_pointers[self.directed_edges[prev_in_idx].sym_idx] = first_out_idx;
                }

                seen_intersection_nodes[node] = false;
            }
        }
    }

    /// Extracts valid rings by following `next_pointers`.
    fn extract_valid_rings(
        &mut self,
        next_pointers: &[usize],
    ) -> Vec<(Vec<Coord3D>, Vec<u32>)> {
        for de in &mut self.directed_edges {
            de.is_visited = false;
        }

        // Reuse vector to avoid allocations
        let mut ring_edges = Vec::new();
        let mut rings = Vec::new();

        for start_de_idx in 0..self.directed_edges.len() {
            if self.directed_edges[start_de_idx].is_visited
                || self.directed_edges[start_de_idx].is_marked
            {
                continue;
            }

            ring_edges.clear();
            let mut curr_de_idx = start_de_idx;
            let mut is_valid_ring = true;

            loop {
                let curr_de = &mut self.directed_edges[curr_de_idx];
                curr_de.is_visited = true;
                ring_edges.push(curr_de_idx);

                let next_de_idx = next_pointers[self.directed_edges[curr_de_idx].sym_idx];

                if next_de_idx == usize::MAX {
                    is_valid_ring = false;
                    break;
                }

                curr_de_idx = next_de_idx;

                if curr_de_idx == start_de_idx {
                    break;
                }

                if self.directed_edges[curr_de_idx].is_visited {
                    is_valid_ring = false;
                    break;
                }
            }

            if is_valid_ring && !ring_edges.is_empty() {
                let mut coords = Vec::with_capacity(ring_edges.len() + 1);
                let mut ids = Vec::with_capacity(ring_edges.len());
                let start_node_idx = self.directed_edges[ring_edges[0]].src;
                coords.push(Coord3D {
                    x: self.nodes_x[start_node_idx],
                    y: self.nodes_y[start_node_idx],
                    z: self.nodes_z[start_node_idx],
                });

                for &de_idx in &ring_edges {
                    let de = &self.directed_edges[de_idx];
                    let edge_idx = de.edge_idx;
                    ids.push(self.edges[edge_idx].line.line_id);

                    let dst_idx = de.dst;
                    coords.push(Coord3D {
                        x: self.nodes_x[dst_idx],
                        y: self.nodes_y[dst_idx],
                        z: self.nodes_z[dst_idx],
                    });
                }

                rings.push((coords, ids));
            }
        }
        rings
    }
}
