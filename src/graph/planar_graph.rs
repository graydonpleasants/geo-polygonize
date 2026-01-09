use geo_types::{Coord, LineString};
use geo::Line;
use std::collections::HashMap;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use crate::utils::z_order_index;

// Type aliases for indices to ensure we don't mix them up
pub type NodeId = usize;
pub type EdgeId = usize;
pub type DirEdgeId = usize;

#[derive(Clone, Debug)]
pub struct Edge {
    // The geometry of the edge.
    // In JTS this might be a full LineString, but for the graph we mainly care about connectivity.
    // We store Line to reduce heap allocations compared to LineString.
    pub line: Line<f64>,
    // Indices of the two directed edges associated with this undirected edge.
    pub dir_edges: [DirEdgeId; 2],
    pub is_marked: bool,
}

#[derive(Clone, Debug)]
pub struct DirectedEdge {
    pub src: NodeId,
    pub dst: NodeId,
    /// Reference to the parent geometry (undirected edge)
    pub edge_idx: EdgeId,
    /// Index of the symmetric (reverse) edge
    pub sym_idx: DirEdgeId,
    /// Precomputed angle for efficient sorting
    pub angle: f64,
    /// Traversal state: has this edge been processed into a ring?
    pub is_visited: bool,
    /// Is this edge explicitly marked (e.g. as part of a dangle)
    pub is_marked: bool,
    /// Orientation in the parent LineString (true: same direction, false: opposite)
    pub edge_direction: bool,
}

pub struct PlanarGraph {
    /// Node coordinates (X). Index is `NodeId`.
    pub nodes_x: Vec<f64>,
    /// Node coordinates (Y). Index is `NodeId`.
    pub nodes_y: Vec<f64>,
    /// Node adjacency lists. Index is `NodeId`.
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

impl PlanarGraph {
    pub fn new() -> Self {
        Self {
            nodes_x: Vec::new(),
            nodes_y: Vec::new(),
            nodes_outgoing: Vec::new(),
            nodes_degree: Vec::new(),
            nodes_marked: Vec::new(),
            edges: Vec::new(),
            directed_edges: Vec::new(),
            node_map: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, coord: Coord<f64>) -> NodeId {
        let key = NodeKey::from(coord);
        if let Some(&id) = self.node_map.get(&key) {
            return id;
        }

        let id = self.nodes_x.len();
        self.nodes_x.push(coord.x);
        self.nodes_y.push(coord.y);
        self.nodes_outgoing.push(Vec::new());
        self.nodes_degree.push(0);
        self.nodes_marked.push(false);
        self.node_map.insert(key, id);
        id
    }

    /// Bulk loads edges into the graph.
    /// This is significantly faster than `add_line_string` for large datasets as it avoids HashMap lookups.
    pub fn bulk_load(&mut self, lines: Vec<Line<f64>>) {
        if lines.is_empty() {
            return;
        }

        // 1. Collect all coordinates and precompute Z-order
        struct NodeEntry {
            z: u64,
            c: Coord<f64>,
        }

        let mut entries: Vec<NodeEntry> = Vec::with_capacity(lines.len() * 2);
        for line in &lines {
            entries.push(NodeEntry { z: z_order_index(line.start), c: line.start });
            entries.push(NodeEntry { z: z_order_index(line.end), c: line.end });
        }

        // 2. Sort using precomputed Z-order
        #[cfg(feature = "parallel")]
        entries.par_sort_unstable_by(|a, b| {
            a.z.cmp(&b.z)
                .then_with(|| {
                    // Tie-break with exact coords for determinism/dedup
                    a.c.x.partial_cmp(&b.c.x).unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.c.y.partial_cmp(&b.c.y).unwrap_or(std::cmp::Ordering::Equal))
                })
        });

        #[cfg(not(feature = "parallel"))]
        entries.sort_unstable_by(|a, b| {
             a.z.cmp(&b.z)
                .then_with(|| {
                    // Tie-break with exact coords
                    a.c.x.partial_cmp(&b.c.x).unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.c.y.partial_cmp(&b.c.y).unwrap_or(std::cmp::Ordering::Equal))
                })
        });

        // Dedup using exact equality.
        entries.dedup_by(|a, b| {
            // Strict equality to match binary_search and add_node behavior
            a.c == b.c
        });

        // 3. Build Nodes
        let start_node_idx = self.nodes_x.len();
        self.nodes_x.reserve(entries.len());
        self.nodes_y.reserve(entries.len());
        self.nodes_outgoing.reserve(entries.len());
        self.nodes_degree.reserve(entries.len());
        self.nodes_marked.reserve(entries.len());

        for entry in &entries {
            self.nodes_x.push(entry.c.x);
            self.nodes_y.push(entry.c.y);
            self.nodes_outgoing.push(Vec::new());
            self.nodes_degree.push(0);
            self.nodes_marked.push(false);
        }

        // Helper to find node index using precomputed Z array (entries)
        let get_node_id = |pt: Coord<f64>| -> Option<NodeId> {
             // Binary search must respect the sort order (Z-order)
             let z_pt = z_order_index(pt);

             // Binary search on the sorted entries
             let idx_res = entries.binary_search_by(|probe| {
                 probe.z.cmp(&z_pt)
                    .then_with(|| {
                        probe.c.x.partial_cmp(&pt.x).unwrap_or(std::cmp::Ordering::Equal)
                            .then(probe.c.y.partial_cmp(&pt.y).unwrap_or(std::cmp::Ordering::Equal))
                    })
             });

             match idx_res {
                 Ok(i) => Some(start_node_idx + i),
                 Err(_) => None
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
        #[cfg(feature = "parallel")]
        self.nodes_outgoing.par_iter_mut().zip(degrees.par_iter()).for_each(|(adj, &deg)| {
            adj.reserve(deg);
        });

        #[cfg(not(feature = "parallel"))]
        self.nodes_outgoing.iter_mut().zip(degrees.iter()).for_each(|(adj, &deg)| {
            adj.reserve(deg);
        });

        // 5. Build Edges
        self.edges.reserve(valid_edges.len());
        self.directed_edges.reserve(valid_edges.len() * 2);

        for (u, v, line) in valid_edges {
            let edge_idx = self.edges.len();
            let de_u_v_idx = self.directed_edges.len();
            let de_v_u_idx = self.directed_edges.len() + 1;

            let angle_u = (self.nodes_y[v] - self.nodes_y[u]).atan2(self.nodes_x[v] - self.nodes_x[u]);
            let angle_v = (self.nodes_y[u] - self.nodes_y[v]).atan2(self.nodes_x[u] - self.nodes_x[v]);

            self.directed_edges.push(DirectedEdge {
                src: u,
                dst: v,
                edge_idx,
                sym_idx: de_v_u_idx,
                angle: angle_u,
                is_visited: false,
                is_marked: false,
                edge_direction: true,
            });

            self.directed_edges.push(DirectedEdge {
                src: v,
                dst: u,
                edge_idx,
                sym_idx: de_u_v_idx,
                angle: angle_v,
                is_visited: false,
                is_marked: false,
                edge_direction: false,
            });

            self.edges.push(Edge {
                line,
                dir_edges: [de_u_v_idx, de_v_u_idx],
                is_marked: false,
            });

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
        for i in 0..coords.len().saturating_sub(1) {
            let p0 = coords[i];
            let p1 = coords[i+1];

            if (p0.x - p1.x).abs() < 1e-12 && (p0.y - p1.y).abs() < 1e-12 {
                continue;
            }

            let u = self.add_node(p0);
            let v = self.add_node(p1);

            let edge_idx = self.edges.len();

            let de_u_v_idx = self.directed_edges.len();
            let de_v_u_idx = self.directed_edges.len() + 1;

            let angle_u = (p1.y - p0.y).atan2(p1.x - p0.x);
            let angle_v = (p0.y - p1.y).atan2(p0.x - p1.x);

            let de_u_v = DirectedEdge {
                src: u,
                dst: v,
                edge_idx,
                sym_idx: de_v_u_idx,
                angle: angle_u,
                is_visited: false,
                is_marked: false,
                edge_direction: true,
            };

            let de_v_u = DirectedEdge {
                src: v,
                dst: u,
                edge_idx,
                sym_idx: de_u_v_idx,
                angle: angle_v,
                is_visited: false,
                is_marked: false,
                edge_direction: false,
            };

            self.directed_edges.push(de_u_v);
            self.directed_edges.push(de_v_u);

            self.edges.push(Edge {
                line: Line::new(p0, p1),
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
        let directed_edges = &self.directed_edges;
        #[cfg(feature = "parallel")]
        self.nodes_outgoing.par_iter_mut().for_each(|adj| {
             adj.sort_by(|&a_idx, &b_idx| {
                 let a = &directed_edges[a_idx];
                 let b = &directed_edges[b_idx];
                 a.angle.partial_cmp(&b.angle).unwrap_or(std::cmp::Ordering::Equal)
             });
        });

        #[cfg(not(feature = "parallel"))]
        self.nodes_outgoing.iter_mut().for_each(|adj| {
             adj.sort_by(|&a_idx, &b_idx| {
                 let a = &directed_edges[a_idx];
                 let b = &directed_edges[b_idx];
                 a.angle.partial_cmp(&b.angle).unwrap_or(std::cmp::Ordering::Equal)
             });
        });
    }

    /// Prunes dangles (nodes with degree 1) from the graph iteratively.
    pub fn prune_dangles(&mut self) -> usize {
        let mut dangles_removed = 0;
        let mut to_process: Vec<NodeId> = self.nodes_degree.iter().enumerate()
            .filter(|(i, &d)| d == 1 && !self.nodes_marked[*i])
            .map(|(i, _)| i)
            .collect();

        while let Some(node_idx) = to_process.pop() {
            if self.nodes_degree[node_idx] != 1 {
                continue;
            }

            self.nodes_marked[node_idx] = true;
            self.nodes_degree[node_idx] = 0;
            dangles_removed += 1;

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

                neighbor_idx = self.directed_edges[de_idx].dst;
                edge_found = true;
            }

            if edge_found {
                if self.nodes_degree[neighbor_idx] > 0 {
                    self.nodes_degree[neighbor_idx] -= 1;
                    if self.nodes_degree[neighbor_idx] == 1 && !self.nodes_marked[neighbor_idx] {
                        to_process.push(neighbor_idx);
                    }
                }
            }
        }
        dangles_removed
    }

    /// Extracts rings from the graph using the Next-CCW rule.
    pub fn get_edge_rings(&mut self) -> Vec<LineString<f64>> {
        let mut rings = Vec::new();

        // Build "next unmarked" pointers
        // next_pointers[de_idx] = the index of the next valid (unmarked) edge
        // in the CCW list of the node that de_idx originates from.
        // During traversal, we look at next_pointers[sym_idx], which gives us the
        // edge after the incoming edge (sym) in CCW order at the node.
        let mut next_pointers = vec![usize::MAX; self.directed_edges.len()];

        for (i, degree) in self.nodes_degree.iter().enumerate() {
            if *degree == 0 { continue; }

            // Filter out marked edges from the adjacency list
            let valid_edges: Vec<usize> = self.nodes_outgoing[i].iter()
                .cloned()
                .filter(|&idx| !self.directed_edges[idx].is_marked)
                .collect();

            if valid_edges.is_empty() { continue; }

            // Link them circular
            for k in 0..valid_edges.len() {
                let curr = valid_edges[k];
                let next = valid_edges[(k + 1) % valid_edges.len()];
                next_pointers[curr] = next;
            }
        }

        for de in &mut self.directed_edges {
            de.is_visited = false;
        }

        // Reuse vector to avoid allocations
        let mut ring_edges = Vec::new();

        for start_de_idx in 0..self.directed_edges.len() {
            if self.directed_edges[start_de_idx].is_visited || self.directed_edges[start_de_idx].is_marked {
                continue;
            }

            ring_edges.clear();
            let mut curr_de_idx = start_de_idx;
            let mut is_valid_ring = true;

            loop {
                let curr_de = &mut self.directed_edges[curr_de_idx];
                curr_de.is_visited = true;
                ring_edges.push(curr_de_idx);

                let sym_idx = curr_de.sym_idx;
                let next_de_idx = next_pointers[sym_idx];

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
                let start_node_idx = self.directed_edges[ring_edges[0]].src;
                coords.push(Coord { x: self.nodes_x[start_node_idx], y: self.nodes_y[start_node_idx] });

                for &de_idx in &ring_edges {
                    let de = &self.directed_edges[de_idx];
                    let dst_idx = de.dst;
                    coords.push(Coord { x: self.nodes_x[dst_idx], y: self.nodes_y[dst_idx] });
                }

                rings.push(LineString::new(coords));
            }
        }

        rings
    }
}
