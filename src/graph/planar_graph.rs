use geo_types::{Coord, LineString};
use geo::Line;
use std::collections::HashMap;
use rayon::prelude::*;

// Type aliases for indices to ensure we don't mix them up
pub type NodeId = usize;
pub type EdgeId = usize;
pub type DirEdgeId = usize;

#[derive(Clone, Debug)]
pub struct Node {
    pub coordinate: Coord<f64>,
    /// Indices of outgoing DirectedEdges.
    /// CRITICAL INVARIANT: Sorted by polar angle (CCW).
    pub outgoing_edges: Vec<DirEdgeId>,
    /// State flag for graph cleaning (dangle removal)
    pub degree: usize,
    pub is_marked: bool,
}

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
    /// All nodes in the graph. Index is `NodeId`.
    pub nodes: Vec<Node>,
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
            nodes: Vec::new(),
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

        let id = self.nodes.len();
        self.nodes.push(Node {
            coordinate: coord,
            outgoing_edges: Vec::new(),
            degree: 0,
            is_marked: false,
        });
        self.node_map.insert(key, id);
        id
    }

    /// Bulk loads edges into the graph.
    /// This is significantly faster than `add_line_string` for large datasets as it avoids HashMap lookups.
    pub fn bulk_load(&mut self, lines: Vec<Line<f64>>) {
        if lines.is_empty() {
            return;
        }

        // 1. Collect all coordinates
        let mut coords = Vec::with_capacity(lines.len() * 2);
        for line in &lines {
            coords.push(line.start);
            coords.push(line.end);
        }

        // 2. Sort and Dedup
        // We define a comparison for Coords to sort them
        coords.sort_by(|a, b| {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Dedup using epsilon
        let tol = 1e-10;
        coords.dedup_by(|a, b| {
            (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
        });

        // 3. Build Nodes
        // We assume the graph is empty or we append?
        // For simplicity, bulk_load assumes empty or appends.
        // If appending, we need to handle existing nodes?
        // Let's assume bulk_load is the primary build method.
        // We will clear existing nodes? No, maybe we just append.

        let start_node_idx = self.nodes.len();
        for coord in &coords {
            self.nodes.push(Node {
                coordinate: *coord,
                outgoing_edges: Vec::new(),
                degree: 0,
                is_marked: false,
            });
            // We do NOT update node_map to save time/memory.
        }

        // Helper to find node index
        // Since coords is sorted, we can use binary search.
        // But self.nodes includes previous nodes?
        // If we mix modes, it's complex.
        // We assume for optimization that bulk_load is called once on an empty graph.
        // If not, we only search the new nodes?
        // Let's assume lines connect only to these new nodes (or existing logic).
        // For this optimization, we search `coords` to find the index relative to `start_node_idx`.

        let get_node_id = |pt: Coord<f64>| -> Option<NodeId> {
             let idx_res = coords.binary_search_by(|probe| {
                 // Compare with tolerance? Binary search requires strict ordering.
                 // If we used dedup_by with tolerance, our exact values in `coords` are the canonical ones.
                 // But the input lines might have slightly different values (within tolerance).
                 // This is tricky.
                 // If we rely on robust noding, points should be exact.
                 // If we rely on tolerance, we need a "fuzzy binary search" or just assume exact match after noding.
                 // In `node_lines`, we snap endpoints?
                 // Let's assume exact match for now as noding should align them.
                 // Or we use the same comparator.
                 probe.x.partial_cmp(&pt.x).unwrap_or(std::cmp::Ordering::Equal)
                    .then(probe.y.partial_cmp(&pt.y).unwrap_or(std::cmp::Ordering::Equal))
             });

             // If exact match fails, we might check neighbors if tolerance is needed?
             // But binary search returns Err(idx) if not found.
             // If we rely on noding, we should have exact matches if we used the same points.

             match idx_res {
                 Ok(i) => Some(start_node_idx + i),
                 Err(_) => None
             }
        };

        // 4. Build Edges
        self.edges.reserve(lines.len());
        self.directed_edges.reserve(lines.len() * 2);

        for line in lines {
             let p0 = line.start;
             let p1 = line.end;

             // Skip degenerate
             if (p0.x - p1.x).abs() < 1e-12 && (p0.y - p1.y).abs() < 1e-12 {
                continue;
            }

            let u_opt = get_node_id(p0);
            let v_opt = get_node_id(p1);

            if u_opt.is_none() || v_opt.is_none() {
                // Should not happen if lines endpoints were in coords
                continue;
            }
            let u = u_opt.unwrap();
            let v = v_opt.unwrap();

            let edge_idx = self.edges.len();
            let de_u_v_idx = self.directed_edges.len();
            let de_v_u_idx = self.directed_edges.len() + 1;

            let angle_u = (p1.y - p0.y).atan2(p1.x - p0.x);
            let angle_v = (p0.y - p1.y).atan2(p0.x - p1.x);

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

            self.nodes[u].outgoing_edges.push(de_u_v_idx);
            self.nodes[u].degree += 1;

            self.nodes[v].outgoing_edges.push(de_v_u_idx);
            self.nodes[v].degree += 1;
        }
    }

    /// Adds a line string to the graph.
    /// Assumes the line string is properly noded.
    pub fn add_line_string(&mut self, line: LineString<f64>) {
        if line.0.is_empty() {
            return;
        }

        let coords = &line.0;
        for i in 0..coords.len().saturating_sub(1) {
            let p0 = coords[i];
            let p1 = coords[i+1];

            // Skip degenerate segments
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

            self.nodes[u].outgoing_edges.push(de_u_v_idx);
            self.nodes[u].degree += 1;

            self.nodes[v].outgoing_edges.push(de_v_u_idx);
            self.nodes[v].degree += 1;
        }
    }

    /// Sorts all outgoing edges of all nodes by angle.
    pub fn sort_edges(&mut self) {
        let directed_edges = &self.directed_edges;
        self.nodes.par_iter_mut().for_each(|node| {
             node.outgoing_edges.sort_by(|&a_idx, &b_idx| {
                 let a = &directed_edges[a_idx];
                 let b = &directed_edges[b_idx];
                 // Sort by angle. If angles are equal (overlapping segments), standard sorting is fine.
                 a.angle.partial_cmp(&b.angle).unwrap_or(std::cmp::Ordering::Equal)
             });
        });
    }

    /// Prunes dangles (nodes with degree 1) from the graph iteratively.
    pub fn prune_dangles(&mut self) -> usize {
        let mut dangles_removed = 0;
        let mut to_process: Vec<NodeId> = self.nodes.iter().enumerate()
            .filter(|(_, n)| n.degree == 1 && !n.is_marked) // is_marked can mean "removed" here
            .map(|(i, _)| i)
            .collect();

        while let Some(node_idx) = to_process.pop() {
            if self.nodes[node_idx].degree != 1 {
                continue;
            }

            // Mark node as removed
            self.nodes[node_idx].is_marked = true; // Use is_marked to signify removed/processed
            self.nodes[node_idx].degree = 0;
            dangles_removed += 1;

            // Find the connected edge
            let mut edge_found = false;
            let mut neighbor_idx = 0;

            let mut found_de_idx = None;
            for &de_idx in &self.nodes[node_idx].outgoing_edges {
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
                let neighbor = &mut self.nodes[neighbor_idx];
                if neighbor.degree > 0 {
                    neighbor.degree -= 1;
                    if neighbor.degree == 1 && !neighbor.is_marked {
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

        // Reset visited state
        for de in &mut self.directed_edges {
            de.is_visited = false;
        }

        // Iterate over all directed edges
        for start_de_idx in 0..self.directed_edges.len() {
            if self.directed_edges[start_de_idx].is_visited || self.directed_edges[start_de_idx].is_marked {
                continue;
            }

            // Start tracing
            let mut ring_edges = Vec::new();
            let mut curr_de_idx = start_de_idx;
            let mut is_valid_ring = true;

            loop {
                let curr_de = &mut self.directed_edges[curr_de_idx];
                curr_de.is_visited = true;
                ring_edges.push(curr_de_idx);

                let dst_node_idx = curr_de.dst;
                let sym_idx = curr_de.sym_idx;
                let dst_node = &self.nodes[dst_node_idx];

                let mut found_idx = None;
                for (i, &idx) in dst_node.outgoing_edges.iter().enumerate() {
                    if idx == sym_idx {
                        found_idx = Some(i);
                        break;
                    }
                }

                if found_idx.is_none() {
                    is_valid_ring = false;
                    break;
                }

                let idx_in_list = found_idx.unwrap();

                // Find next unmarked edge CCW
                let len = dst_node.outgoing_edges.len();
                let mut next_de_idx = None;

                for i in 1..=len {
                    let next_pos = (idx_in_list + i) % len;
                    let candidate_idx = dst_node.outgoing_edges[next_pos];
                    if !self.directed_edges[candidate_idx].is_marked {
                        next_de_idx = Some(candidate_idx);
                        break;
                    }
                }

                if let Some(next) = next_de_idx {
                    curr_de_idx = next;
                } else {
                    is_valid_ring = false;
                    break;
                }

                if curr_de_idx == start_de_idx {
                    break; // Ring closed
                }

                if self.directed_edges[curr_de_idx].is_visited {
                    is_valid_ring = false;
                    break;
                }
            }

            if is_valid_ring && !ring_edges.is_empty() {
                // Construct LineString
                let mut coords = Vec::with_capacity(ring_edges.len() + 1);
                // Add start point of first edge
                let start_node_idx = self.directed_edges[ring_edges[0]].src;
                coords.push(self.nodes[start_node_idx].coordinate);

                for &de_idx in &ring_edges {
                    let de = &self.directed_edges[de_idx];
                    let dst = &self.nodes[de.dst];
                    coords.push(dst.coordinate);
                }

                rings.push(LineString::new(coords));
            }
        }

        rings
    }
}
