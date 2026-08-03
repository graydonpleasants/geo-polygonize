use crate::options::ExecutionPolicy;
use crate::trace::TraceCaptureBudget;
use crate::types::{Coord3D, EdgeSources, Line3D};
use crate::utils::parallel::{par_flat_map, par_sort_unstable, par_zip_for_each};
use crate::utils::{
    canonical_coordinate_bits, compare_angular, minimum_rotation_index, z_order_index,
};
use geo_types::{Coord, LineString};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::HashMap;

use super::partition_border::{PartitionBorderHalfEdge, PartitionBorderSide};

/// Index of a node in the graph.
pub type NodeId = usize;
/// Index of an undirected edge in the graph.
pub type EdgeId = usize;
/// Index of a directed half-edge in the graph.
pub type DirEdgeId = usize;
/// Deterministic identity of a directed-edge face cycle.
pub type FaceId = usize;

#[derive(Clone)]
pub(crate) struct ExtractedRing {
    pub coords: Vec<Coord3D>,
    pub line_ids: Vec<u32>,
    pub source_line_ids: Vec<u32>,
    /// Component-local deterministic face identity for final ring extraction.
    /// Maximal trace rings are captured before face identities are assigned.
    pub face_id: Option<FaceId>,
    pub edge_keys: Vec<(NodeId, NodeId)>,
    pub node_ids: Vec<NodeId>,
}

/// An undirected edge in the planar graph.
#[derive(Clone, Debug)]
pub(crate) struct Edge {
    /// The geometry of the edge.
    /// In JTS this might be a full LineString, but for the graph we mainly care about connectivity.
    /// We store Line to reduce heap allocations compared to LineString.
    pub(crate) line: Line3D,
    /// All input lines that contribute to this geometric edge.
    pub(crate) sources: EdgeSources,
    /// Indices of the two directed edges associated with this undirected edge.
    pub(crate) dir_edges: [DirEdgeId; 2],
    /// Flag indicating if the edge is marked (e.g. visited or pruned).
    pub(crate) is_marked: bool,
    /// Flag indicating if the edge has been dynamically deleted.
    pub(crate) deleted: bool,
}

/// A directed half-edge in the planar graph.
#[derive(Clone, Debug)]
pub(crate) struct DirectedEdge {
    /// Source node index.
    pub(crate) src: NodeId,
    /// Destination node index.
    pub(crate) dst: NodeId,
    /// Reference to the parent geometry (undirected edge).
    pub(crate) edge_idx: EdgeId,
    /// Index of the symmetric (reverse) edge.
    pub(crate) sym_idx: DirEdgeId,
    /// Directed edge reached by the arrangement's clockwise face walk.
    pub(crate) next_idx: Option<DirEdgeId>,
    /// Deterministic identity of the current face cycle, when assigned.
    pub(crate) face_id: Option<FaceId>,
    /// Traversal state: has this edge been processed into a ring?
    pub(crate) is_visited: bool,
    /// Is this edge explicitly marked (e.g. as part of a dangle).
    pub(crate) is_marked: bool,
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
    pub(crate) nodes_x: Vec<f64>,
    /// Node coordinates (Y). Index is `NodeId`.
    pub(crate) nodes_y: Vec<f64>,
    /// Node coordinates (Z). Index is `NodeId`.
    pub(crate) nodes_z: Vec<f64>,
    /// Node adjacency lists. Index is `NodeId`.
    /// Stores the list of outgoing `DirEdgeId`s for each node.
    pub(crate) nodes_outgoing: Vec<Vec<DirEdgeId>>,
    /// Node connectivity degrees. Index is `NodeId`.
    pub(crate) nodes_degree: Vec<usize>,
    /// Node marked flags. Index is `NodeId`.
    pub(crate) nodes_marked: Vec<bool>,

    /// All undirected edges (geometry owners). Index is `EdgeId`.
    pub(crate) edges: Vec<Edge>,
    /// All directed half-edges. Index is `DirEdgeId`.
    pub(crate) directed_edges: Vec<DirectedEdge>,
    /// Lookup map to dedup nodes during construction.
    /// OPTIMIZATION: Used only for incremental additions. Bulk load bypasses this.
    pub(crate) node_map: HashMap<NodeKey, NodeId>,
    /// Number of deterministic face-cycle identities currently assigned.
    pub(crate) face_count: usize,
    /// Component-local outer face-cycle identities.
    pub(crate) unbounded_face_ids: Vec<FaceId>,
}

struct FaceCycleCandidate {
    key: Vec<[u64; 2]>,
    directed_edges: Vec<DirEdgeId>,
    signed_area: f64,
}

type ComponentOutput = (
    Vec<Vec<Coord3D>>,
    Vec<Vec<Coord3D>>,
    Vec<ExtractedRing>,
    Vec<ExtractedRing>,
);

struct ComponentPartition {
    nodes: Vec<NodeId>,
    edges: Vec<EdgeId>,
}

struct ComponentGraph {
    graph: PlanarGraph,
    global_node_ids: Vec<NodeId>,
}

fn append_component_output(target: &mut ComponentOutput, output: ComponentOutput) {
    target.0.extend(output.0);
    target.1.extend(output.1);
    target.2.extend(output.2);
    target.3.extend(output.3);
}

// Wrapper for Coord to be Hashable (since f64 is not Hash)
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub(crate) struct NodeKey(i64, i64);

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
    sources: EdgeSources,
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
        next_idx: None,
        face_id: None,
        is_visited: false,
        is_marked: false,
    };

    let de_v_u = DirectedEdge {
        src: v,
        dst: u,
        edge_idx,
        sym_idx: de_u_v_idx,
        next_idx: None,
        face_id: None,
        is_visited: false,
        is_marked: false,
    };

    let edge = Edge {
        line,
        sources,
        dir_edges: [de_u_v_idx, de_v_u_idx],
        is_marked: false,
        deleted: false,
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
            face_count: 0,
            unbounded_face_ids: Vec::new(),
        }
    }

    /// Creates a canonical partition-border observation for a directed local
    /// edge. The caller supplies the already-classified border side; this
    /// method only transfers arrangement identity, direction, provenance, and
    /// Z observations into the partition representation.
    pub fn partition_border_half_edge(
        &self,
        partition_id: usize,
        dir_edge_id: DirEdgeId,
        side: PartitionBorderSide,
    ) -> Option<PartitionBorderHalfEdge> {
        let directed = self.directed_edges.get(dir_edge_id)?;
        let edge = self.edges.get(directed.edge_idx)?;
        if edge.deleted {
            return None;
        }
        let start = Coord3D {
            x: *self.nodes_x.get(directed.src)?,
            y: *self.nodes_y.get(directed.src)?,
            z: *self.nodes_z.get(directed.src)?,
        };
        let end = Coord3D {
            x: *self.nodes_x.get(directed.dst)?,
            y: *self.nodes_y.get(directed.dst)?,
            z: *self.nodes_z.get(directed.dst)?,
        };
        PartitionBorderHalfEdge::new(
            partition_id,
            dir_edge_id,
            directed.face_id,
            side,
            start,
            end,
            edge.sources.line_ids.iter().copied(),
        )
    }

    pub(crate) fn clear(&mut self) {
        self.nodes_x.clear();
        self.nodes_y.clear();
        self.nodes_z.clear();
        self.nodes_outgoing.clear();
        self.nodes_degree.clear();
        self.nodes_marked.clear();
        self.edges.clear();
        self.directed_edges.clear();
        self.node_map.clear();
        self.face_count = 0;
        self.unbounded_face_ids.clear();
    }

    fn clear_next_links(&mut self) {
        for directed_edge in &mut self.directed_edges {
            directed_edge.next_idx = None;
        }
        self.clear_face_ids();
    }

    fn clear_face_ids(&mut self) {
        self.face_count = 0;
        self.unbounded_face_ids.clear();
        for directed_edge in &mut self.directed_edges {
            directed_edge.face_id = None;
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
        self.bulk_load_impl(lines, None)
            .expect("unlimited graph loading cannot fail")
    }

    pub(crate) fn bulk_load_with_execution_policy(
        &mut self,
        lines: Vec<Line3D>,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<()> {
        self.bulk_load_impl(lines, Some(execution_policy))
    }

    fn bulk_load_impl(
        &mut self,
        lines: Vec<Line3D>,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<()> {
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_cancelled("graph_construction")?;
        }
        if lines.is_empty() {
            return Ok(());
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
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_uncancellable_sort("graph_node_sort", entries.len())?;
        }
        par_sort_unstable(&mut entries);

        // Dedup using exact equality on X,Y. Z is ignored for dedup key but carried.
        // `NodeEntry` PartialEq/Ord implementation considers X,Y.
        // We need to ensure we don't have duplicates with same X,Y but different Z.
        // `dedup_by` keeps the first one.
        entries.dedup_by(|a, b| a.c.x == b.c.x && a.c.y == b.c.y);

        // 3. Build Nodes
        let start_node_idx = self.nodes_x.len();
        if let Some(execution_policy) = execution_policy {
            let observed = start_node_idx.checked_add(entries.len()).ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: "graph node count overflow".to_string(),
                }
            })?;
            execution_policy.check("graph_nodes", execution_policy.max_graph_nodes, observed)?;
        }
        self.nodes_x.reserve(entries.len());
        self.nodes_y.reserve(entries.len());
        self.nodes_z.reserve(entries.len());
        self.nodes_outgoing.reserve(entries.len());
        self.nodes_degree.reserve(entries.len());
        self.nodes_marked.reserve(entries.len());

        for (index, entry) in entries.iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_construction", index)?;
            }
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

        // Store valid edges as (u, v, line), then dissolve coincident XY edges.
        let mut valid_edges = Vec::with_capacity(lines.len());
        let mut degrees = vec![0usize; self.nodes_x.len()]; // This might be large?

        for (index, line) in lines.into_iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_construction", index)?;
            }
            let p0 = line.start;
            let p1 = line.end;

            if p0.x == p1.x && p0.y == p1.y {
                continue;
            }

            let u_opt = get_node_id(p0);
            let v_opt = get_node_id(p1);

            if let (Some(u), Some(v)) = (u_opt, v_opt) {
                valid_edges.push((u, v, line));
            }
        }

        if let Some(execution_policy) = execution_policy {
            execution_policy.check_uncancellable_sort("graph_edge_sort", valid_edges.len())?;
        }
        valid_edges.sort_unstable_by(|(u1, v1, l1), (u2, v2, l2)| {
            let key1 = ((*u1).min(*v1), (*u1).max(*v1));
            let key2 = ((*u2).min(*v2), (*u2).max(*v2));
            key1.cmp(&key2)
                .then(l1.line_id.cmp(&l2.line_id))
                .then(u1.cmp(u2))
                .then(v1.cmp(v2))
        });

        let mut dissolved: Vec<(NodeId, NodeId, Line3D, EdgeSources)> = Vec::new();
        for (index, (u, v, line)) in valid_edges.into_iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_construction", index)?;
            }
            let key = (u.min(v), u.max(v));
            if let Some((last_u, last_v, _, sources)) = dissolved.last_mut() {
                if ((*last_u).min(*last_v), (*last_u).max(*last_v)) == key {
                    sources.merge_line_id(line.line_id);
                    continue;
                }
            }
            dissolved.push((u, v, line, EdgeSources::from_line_id(line.line_id)));
        }
        for (index, (u, v, _, _)) in dissolved.iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_construction", index)?;
            }
            degrees[*u] += 1;
            degrees[*v] += 1;
        }

        if let Some(execution_policy) = execution_policy {
            let observed = self
                .edges
                .len()
                .checked_add(dissolved.len())
                .ok_or_else(|| crate::PolygonizeError::InternalInvariantViolation {
                    reason: "graph edge count overflow".to_string(),
                })?;
            execution_policy.check("graph_edges", execution_policy.max_graph_edges, observed)?;
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
        self.edges.reserve(dissolved.len());
        let directed_edge_count = dissolved.len().checked_mul(2).ok_or_else(|| {
            crate::PolygonizeError::InternalInvariantViolation {
                reason: "directed edge count overflow".to_string(),
            }
        })?;
        self.directed_edges.reserve(directed_edge_count);

        let edges_start_len = self.edges.len();
        let directed_edges_start_len = self.directed_edges.len();

        for (i, (u, v, line, sources)) in dissolved.into_iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_construction", i)?;
            }
            let (u, v, de_u_v_idx, de_v_u_idx, de_u_v, de_v_u, edge) = create_edge_components(
                i,
                u,
                v,
                line,
                sources,
                edges_start_len,
                directed_edges_start_len,
            );
            self.directed_edges.push(de_u_v);
            self.directed_edges.push(de_v_u);
            self.edges.push(edge);

            self.nodes_outgoing[u].push(de_u_v_idx);
            self.nodes_degree[u] += 1;
            self.nodes_outgoing[v].push(de_v_u_idx);
            self.nodes_degree[v] += 1;
        }
        Ok(())
    }

    /// Adds a line to the graph and returns the new EdgeId.
    pub fn add_line(&mut self, line: Line3D) -> EdgeId {
        let p0 = line.start;
        let p1 = line.end;

        let u = self.add_node(p0);
        let v = self.add_node(p1);

        if let Some(edge_idx) = self.nodes_outgoing[u]
            .iter()
            .find_map(|&directed_edge_idx| {
                let directed_edge = &self.directed_edges[directed_edge_idx];
                (directed_edge.dst == v && !self.edges[directed_edge.edge_idx].deleted)
                    .then_some(directed_edge.edge_idx)
            })
        {
            let edge = &mut self.edges[edge_idx];
            edge.sources.merge_line_id(line.line_id);
            edge.line.line_id = edge.sources.line_ids[0];
            return edge_idx;
        }

        let edge_idx = self.edges.len();
        let de_u_v_idx = self.directed_edges.len();
        let de_v_u_idx = self.directed_edges.len() + 1;

        let de_u_v = DirectedEdge {
            src: u,
            dst: v,
            edge_idx,
            sym_idx: de_v_u_idx,
            next_idx: None,
            face_id: None,
            is_visited: false,
            is_marked: false,
        };

        let de_v_u = DirectedEdge {
            src: v,
            dst: u,
            edge_idx,
            sym_idx: de_u_v_idx,
            next_idx: None,
            face_id: None,
            is_visited: false,
            is_marked: false,
        };

        self.directed_edges.push(de_u_v);
        self.directed_edges.push(de_v_u);

        self.edges.push(Edge {
            line,
            sources: EdgeSources::from_line_id(line.line_id),
            dir_edges: [de_u_v_idx, de_v_u_idx],
            is_marked: false,
            deleted: false,
        });

        self.nodes_outgoing[u].push(de_u_v_idx);
        self.nodes_degree[u] += 1;

        self.nodes_outgoing[v].push(de_v_u_idx);
        self.nodes_degree[v] += 1;

        edge_idx
    }

    /// Removes an edge by marking it as deleted by matching line ID.
    pub fn remove_line_by_id(&mut self, line_id: u32) -> bool {
        for edge in &mut self.edges {
            if !edge.deleted && edge.sources.remove_line_id(line_id) {
                if edge.sources.line_ids.is_empty() {
                    edge.deleted = true;
                } else if edge.line.line_id == line_id {
                    edge.line.line_id = edge.sources.line_ids[0];
                }
                return true;
            }
        }
        false
    }

    /// Resets all traversal and marked flags so a new polygonization pass can run.
    pub fn reset_traversal_state(&mut self) {
        // Recalculate degrees based only on non-deleted edges
        for d in &mut self.nodes_degree {
            *d = 0;
        }
        for (i, outgoing) in self.nodes_outgoing.iter().enumerate() {
            for &de_idx in outgoing {
                let de = &self.directed_edges[de_idx];
                if !self.edges[de.edge_idx].deleted {
                    self.nodes_degree[i] += 1;
                }
            }
        }

        for m in &mut self.nodes_marked {
            *m = false;
        }
        self.clear_next_links();
        for de in &mut self.directed_edges {
            de.is_visited = false;
            de.is_marked = false;
        }
        for edge in &mut self.edges {
            edge.is_marked = false;
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

            if p0.x == p1.x && p0.y == p1.y {
                continue;
            }

            self.add_line(Line3D::new(p0.into(), p1.into(), 0));
        }
    }

    /// Sorts all outgoing edges of all nodes by angle.
    pub fn sort_edges(&mut self) {
        self.clear_next_links();
        let nodes_x = &self.nodes_x;
        let nodes_y = &self.nodes_y;
        let directed_edges = &self.directed_edges;

        // Use a robust angular comparator.
        // This requires accessing coordinates of src and dst nodes.
        #[cfg(feature = "parallel")]
        self.nodes_outgoing
            .par_iter_mut()
            .zip(self.nodes_degree.par_iter_mut())
            .enumerate()
            .for_each(|(src_idx, (adj, degree))| {
                // Filter out deleted edges before sorting
                adj.retain(|&idx| !self.edges[self.directed_edges[idx].edge_idx].deleted);
                *degree = adj.len();

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
            .zip(self.nodes_degree.iter_mut())
            .enumerate()
            .for_each(|(src_idx, (adj, degree))| {
                // Filter out deleted edges before sorting
                adj.retain(|&idx| !self.edges[self.directed_edges[idx].edge_idx].deleted);
                *degree = adj.len();

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

        #[cfg(any(test, debug_assertions))]
        self.validate_arrangement_edge_invariants()
            .expect("post-sort arrangement edge invariants");
    }

    pub(crate) fn sort_edges_with_execution_policy(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<()> {
        execution_policy.check_cancelled("graph_construction")?;
        self.clear_next_links();
        let nodes_x = &self.nodes_x;
        let nodes_y = &self.nodes_y;
        let directed_edges = &self.directed_edges;

        for (src_idx, adj) in self.nodes_outgoing.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("graph_construction", src_idx)?;
            adj.retain(|&idx| !self.edges[self.directed_edges[idx].edge_idx].deleted);
            self.nodes_degree[src_idx] = adj.len();
            execution_policy.check_uncancellable_sort("graph_node_star_sort", adj.len())?;

            let center = Coord {
                x: nodes_x[src_idx],
                y: nodes_y[src_idx],
            };
            adj.sort_by(|&a_idx, &b_idx| {
                let a_de = &directed_edges[a_idx];
                let b_de = &directed_edges[b_idx];
                let target_a = Coord {
                    x: nodes_x[a_de.dst],
                    y: nodes_y[a_de.dst],
                };
                let target_b = Coord {
                    x: nodes_x[b_de.dst],
                    y: nodes_y[b_de.dst],
                };
                compare_angular(center, target_a, target_b)
            });
        }

        #[cfg(any(test, debug_assertions))]
        self.validate_arrangement_edge_invariants()?;

        Ok(())
    }

    /// Validates the post-sort, pre-pruning arrangement representation.
    ///
    /// This is compiled only for tests and debug builds so production release
    /// pipelines pay no validation cost. Keeping the check at the shared sort
    /// root also lets debug fuzz builds exercise it without a public graph API.
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn validate_arrangement_edge_invariants(&self) -> crate::Result<()> {
        let invariant = |reason| crate::PolygonizeError::InternalInvariantViolation { reason };
        let node_count = self.nodes_x.len();
        for (name, actual) in [
            ("nodes_y", self.nodes_y.len()),
            ("nodes_z", self.nodes_z.len()),
            ("nodes_outgoing", self.nodes_outgoing.len()),
            ("nodes_degree", self.nodes_degree.len()),
            ("nodes_marked", self.nodes_marked.len()),
        ] {
            if actual != node_count {
                return Err(invariant(format!(
                    "arrangement edge invariant node arena count mismatch: nodes_x={node_count}, {name}={actual}"
                )));
            }
        }

        let expected_directed = self.edges.len().checked_mul(2).ok_or_else(|| {
            invariant("arrangement edge invariant directed edge count overflow".to_string())
        })?;
        if self.directed_edges.len() != expected_directed {
            return Err(invariant(format!(
                "arrangement edge invariant directed edge count mismatch: edges={}, directed_edges={}, expected={expected_directed}",
                self.edges.len(),
                self.directed_edges.len()
            )));
        }

        let mut owners = vec![0usize; self.directed_edges.len()];
        for (edge_idx, edge) in self.edges.iter().enumerate() {
            let [forward_idx, reverse_idx] = edge.dir_edges;
            if forward_idx == reverse_idx {
                return Err(invariant(format!(
                    "arrangement edge invariant edge {edge_idx} has duplicate directed edge {forward_idx}"
                )));
            }
            for directed_idx in [forward_idx, reverse_idx] {
                let Some(directed) = self.directed_edges.get(directed_idx) else {
                    return Err(invariant(format!(
                        "arrangement edge invariant edge {edge_idx} references missing directed edge {directed_idx}"
                    )));
                };
                owners[directed_idx] += 1;
                if directed.edge_idx != edge_idx {
                    return Err(invariant(format!(
                        "arrangement edge invariant directed edge {directed_idx} parent mismatch: edge_idx={}, expected={edge_idx}",
                        directed.edge_idx
                    )));
                }
                if directed.src >= node_count || directed.dst >= node_count {
                    return Err(invariant(format!(
                        "arrangement edge invariant directed edge {directed_idx} endpoint out of bounds: src={}, dst={}, nodes={node_count}",
                        directed.src, directed.dst
                    )));
                }
            }

            let forward = &self.directed_edges[forward_idx];
            let reverse = &self.directed_edges[reverse_idx];
            if forward.sym_idx != reverse_idx || reverse.sym_idx != forward_idx {
                return Err(invariant(format!(
                    "arrangement edge invariant edge {edge_idx} twin involution mismatch: {forward_idx}.sym={}, {reverse_idx}.sym={}",
                    forward.sym_idx, reverse.sym_idx
                )));
            }
            if forward.src != reverse.dst || forward.dst != reverse.src {
                return Err(invariant(format!(
                    "arrangement edge invariant edge {edge_idx} twin endpoint mismatch: {forward_idx}=({}->{}), {reverse_idx}=({}->{})",
                    forward.src, forward.dst, reverse.src, reverse.dst
                )));
            }

            if !edge.deleted {
                let sources = edge.sources.line_ids.as_slice();
                if sources.is_empty() {
                    return Err(invariant(format!(
                        "arrangement edge invariant live edge {edge_idx} has no sources"
                    )));
                }
                if let Some(pair) = sources.windows(2).find(|pair| pair[0] >= pair[1]) {
                    return Err(invariant(format!(
                        "arrangement edge invariant live edge {edge_idx} sources are not strictly sorted: {} then {}",
                        pair[0], pair[1]
                    )));
                }
                if edge.line.line_id != sources[0] {
                    return Err(invariant(format!(
                        "arrangement edge invariant live edge {edge_idx} representative source mismatch: line_id={}, first_source={}",
                        edge.line.line_id, sources[0]
                    )));
                }
            }
        }

        for (directed_idx, owner_count) in owners.into_iter().enumerate() {
            if owner_count != 1 {
                return Err(invariant(format!(
                    "arrangement edge invariant directed edge {directed_idx} owner count mismatch: actual={owner_count}, expected=1"
                )));
            }
        }

        let mut adjacency_counts = vec![0usize; self.directed_edges.len()];
        for node_idx in 0..node_count {
            let outgoing = &self.nodes_outgoing[node_idx];
            if self.nodes_degree[node_idx] != outgoing.len() {
                return Err(invariant(format!(
                    "arrangement edge invariant node {node_idx} degree mismatch: degree={}, adjacency={}",
                    self.nodes_degree[node_idx],
                    outgoing.len()
                )));
            }

            for (position, &directed_idx) in outgoing.iter().enumerate() {
                let Some(directed) = self.directed_edges.get(directed_idx) else {
                    return Err(invariant(format!(
                        "arrangement edge invariant node {node_idx} adjacency[{position}] references missing directed edge {directed_idx}"
                    )));
                };
                if directed.src != node_idx {
                    return Err(invariant(format!(
                        "arrangement edge invariant node {node_idx} adjacency[{position}] source mismatch: directed edge {directed_idx} has src={}",
                        directed.src
                    )));
                }
                if self.edges[directed.edge_idx].deleted {
                    return Err(invariant(format!(
                        "arrangement edge invariant node {node_idx} adjacency[{position}] references deleted edge {} via directed edge {directed_idx}",
                        directed.edge_idx
                    )));
                }
                adjacency_counts[directed_idx] += 1;
            }

            let center = Coord {
                x: self.nodes_x[node_idx],
                y: self.nodes_y[node_idx],
            };
            for pair in outgoing.windows(2) {
                let first_idx = pair[0];
                let second_idx = pair[1];
                let first = &self.directed_edges[first_idx];
                let second = &self.directed_edges[second_idx];
                let ordering = compare_angular(
                    center,
                    Coord {
                        x: self.nodes_x[first.dst],
                        y: self.nodes_y[first.dst],
                    },
                    Coord {
                        x: self.nodes_x[second.dst],
                        y: self.nodes_y[second.dst],
                    },
                );
                if ordering != Ordering::Less {
                    return Err(invariant(format!(
                        "arrangement edge invariant node {node_idx} angular order is not strict between directed edges {first_idx} and {second_idx}: {ordering:?}"
                    )));
                }
            }
        }

        for (directed_idx, directed) in self.directed_edges.iter().enumerate() {
            let expected = usize::from(!self.edges[directed.edge_idx].deleted);
            if adjacency_counts[directed_idx] != expected {
                return Err(invariant(format!(
                    "arrangement edge invariant directed edge {directed_idx} adjacency count mismatch: actual={}, expected={expected}",
                    adjacency_counts[directed_idx]
                )));
            }
        }

        Ok(())
    }

    /// Prunes dangles (nodes with degree 1) from the graph iteratively.
    pub fn prune_dangles(&mut self) -> Vec<Vec<Coord3D>> {
        self.prune_dangles_impl(None)
            .expect("unlimited graph pruning cannot fail")
    }

    pub(crate) fn prune_dangles_with_execution_policy(
        &mut self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<Vec<Vec<Coord3D>>> {
        self.prune_dangles_impl(Some(execution_policy))
    }

    fn prune_dangles_impl(
        &mut self,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<Vec<Vec<Coord3D>>> {
        let mut dangles = Vec::new();
        let mut to_process = Vec::new();
        for (node_idx, &degree) in self.nodes_degree.iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_construction", node_idx)?;
            }
            if degree == 1 && !self.nodes_marked[node_idx] {
                to_process.push(node_idx);
            }
        }

        let mut processed = 0;
        while let Some(node_idx) = to_process.pop() {
            processed += 1;
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_construction", processed)?;
            }
            if self.nodes_degree[node_idx] != 1 {
                continue;
            }

            self.nodes_marked[node_idx] = true;
            self.nodes_degree[node_idx] = 0;

            let mut edge_found = false;
            let mut neighbor_idx = 0;

            let mut found_de_idx = None;
            for &de_idx in &self.nodes_outgoing[node_idx] {
                let de = &self.directed_edges[de_idx];
                if !de.is_marked && !self.edges[de.edge_idx].deleted {
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
        Ok(dangles)
    }

    /// Finds and removes edges whose two directions belong to the same maximal ring.
    pub fn delete_cut_edges(&mut self) -> Vec<Vec<Coord3D>> {
        self.delete_cut_edges_impl(None, false)
            .expect("unlimited cut-edge removal cannot fail")
    }

    pub(crate) fn delete_cut_edges_with_execution_policy(
        &mut self,
        execution_policy: &ExecutionPolicy,
        noding_postcondition_validated: bool,
    ) -> crate::Result<Vec<Vec<Coord3D>>> {
        self.delete_cut_edges_impl(Some(execution_policy), noding_postcondition_validated)
    }

    fn delete_cut_edges_impl(
        &mut self,
        execution_policy: Option<&ExecutionPolicy>,
        _noding_postcondition_validated: bool,
    ) -> crate::Result<Vec<Vec<Coord3D>>> {
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_cancelled("ring_extraction")?;
        }
        self.compute_next_cw_edges(execution_policy)?;

        #[cfg(any(test, debug_assertions))]
        if _noding_postcondition_validated {
            self.validate_arrangement_euler("maximal")?;
        } else {
            self.validate_arrangement_ring_cycles("maximal")?;
        }

        let mut labels = vec![-1_i64; self.directed_edges.len()];
        self.find_and_label_maximal_rings(&mut labels, execution_policy)?;

        let mut cuts = Vec::new();
        for (edge_idx, edge) in self.edges.iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("ring_extraction", edge_idx)?;
            }
            let [forward, reverse] = edge.dir_edges;
            if edge.deleted
                || self.directed_edges[forward].is_marked
                || self.directed_edges[reverse].is_marked
                || labels[forward] != labels[reverse]
            {
                continue;
            }

            self.directed_edges[forward].is_marked = true;
            self.directed_edges[reverse].is_marked = true;
            cuts.push(vec![edge.line.start, edge.line.end]);
        }
        Ok(cuts)
    }

    /// Extracts rings from the graph following the GEOS flow.
    pub fn get_edge_rings(&mut self) -> Vec<(Vec<Coord3D>, Vec<u32>)> {
        self.get_edge_rings_with_graph_ids(false, false)
            .into_iter()
            .map(|ring| (ring.coords, ring.line_ids))
            .collect()
    }

    pub(crate) fn get_edge_rings_with_graph_ids(
        &mut self,
        include_graph_ids: bool,
        include_source_ids: bool,
    ) -> Vec<ExtractedRing> {
        self.get_edge_rings_with_graph_ids_impl(
            include_graph_ids,
            include_source_ids,
            None,
            None,
            None,
            false,
        )
        .expect("unlimited ring extraction cannot fail")
    }

    pub(crate) fn get_edge_rings_with_graph_ids_and_execution_policy(
        &mut self,
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: &ExecutionPolicy,
        noding_postcondition_validated: bool,
    ) -> crate::Result<Vec<ExtractedRing>> {
        self.get_edge_rings_with_graph_ids_impl(
            include_graph_ids,
            include_source_ids,
            Some(execution_policy),
            None,
            None,
            noding_postcondition_validated,
        )
    }

    #[cfg(test)]
    pub(crate) fn get_edge_rings_with_maximal_and_execution_policy(
        &mut self,
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: &ExecutionPolicy,
        capture_byte_limit: usize,
        noding_postcondition_validated: bool,
    ) -> crate::Result<(Vec<ExtractedRing>, Vec<ExtractedRing>, bool)> {
        let mut capture_budget = TraceCaptureBudget::new(capture_byte_limit);
        self.get_edge_rings_with_maximal_and_execution_policy_with_budget(
            include_graph_ids,
            include_source_ids,
            execution_policy,
            &mut capture_budget,
            noding_postcondition_validated,
        )
    }

    fn get_edge_rings_with_maximal_and_execution_policy_with_budget(
        &mut self,
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: &ExecutionPolicy,
        capture_budget: &mut TraceCaptureBudget,
        noding_postcondition_validated: bool,
    ) -> crate::Result<(Vec<ExtractedRing>, Vec<ExtractedRing>, bool)> {
        let mut maximal = Vec::new();
        let minimal = self.get_edge_rings_with_graph_ids_impl(
            include_graph_ids,
            include_source_ids,
            Some(execution_policy),
            Some(&mut maximal),
            Some(capture_budget),
            noding_postcondition_validated,
        )?;
        Ok((maximal, minimal, capture_budget.truncated()))
    }

    fn get_edge_rings_with_graph_ids_impl(
        &mut self,
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: Option<&ExecutionPolicy>,
        maximal_trace: Option<&mut Vec<ExtractedRing>>,
        maximal_trace_budget: Option<&mut TraceCaptureBudget>,
        _noding_postcondition_validated: bool,
    ) -> crate::Result<Vec<ExtractedRing>> {
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_cancelled("ring_extraction")?;
        }
        let mut labels = vec![-1_i64; self.directed_edges.len()];

        // Step 1: computeNextCWEdges over every node.
        self.compute_next_cw_edges(execution_policy)?;

        #[cfg(any(test, debug_assertions))]
        if _noding_postcondition_validated {
            self.validate_arrangement_euler("maximal")?;
        } else {
            self.validate_arrangement_ring_cycles("maximal")?;
        }

        // Step 2: find and label maximal rings.
        let maximal_ring_starts =
            self.find_and_label_maximal_rings(&mut labels, execution_policy)?;
        if let (Some(maximal_trace), Some(maximal_trace_budget)) =
            (maximal_trace, maximal_trace_budget)
        {
            *maximal_trace = self.extract_rings_from_starts(
                &maximal_ring_starts,
                include_graph_ids,
                include_source_ids,
                execution_policy,
                maximal_trace_budget,
            )?;
        }

        // Step 3: convert maximal to minimal rings by relinking intersection nodes.
        self.convert_maximal_to_minimal_rings(&maximal_ring_starts, &labels, execution_policy)?;
        self.assign_deterministic_face_ids(execution_policy)?;

        #[cfg(any(test, debug_assertions))]
        self.validate_arrangement_ring_cycles("minimal")?;

        // Extract the minimal rings from the graph.
        self.extract_valid_rings(include_graph_ids, include_source_ids, execution_policy)
    }

    fn extract_rings_from_starts(
        &self,
        starts: &[DirEdgeId],
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: Option<&ExecutionPolicy>,
        capture_budget: &mut TraceCaptureBudget,
    ) -> crate::Result<Vec<ExtractedRing>> {
        let mut rings = Vec::with_capacity(starts.len());
        let mut ring_edges = Vec::new();
        let mut work_items = 0;
        for &start in starts {
            ring_edges.clear();
            let mut current = start;
            let mut valid = true;
            loop {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                }
                work_items += 1;
                let directed = &self.directed_edges[current];
                if directed.is_marked || self.edges[directed.edge_idx].deleted {
                    valid = false;
                    break;
                }
                ring_edges.push(current);
                let next = self.directed_edges[directed.sym_idx].next_idx;
                let Some(next) = next else {
                    valid = false;
                    break;
                };
                if next == start {
                    break;
                }
                if ring_edges.len() > self.directed_edges.len() {
                    valid = false;
                    break;
                }
                current = next;
            }
            if valid && !ring_edges.is_empty() {
                if !capture_budget.take(self.ring_capture_bytes(
                    &ring_edges,
                    include_graph_ids,
                    include_source_ids,
                )) {
                    break;
                }
                rings.push(self.materialize_ring(
                    &ring_edges,
                    include_graph_ids,
                    include_source_ids,
                    execution_policy,
                    &mut work_items,
                )?);
            }
        }
        Ok(rings)
    }

    fn ring_capture_bytes(
        &self,
        ring_edges: &[DirEdgeId],
        include_graph_ids: bool,
        include_source_ids: bool,
    ) -> usize {
        let edge_count = ring_edges.len();
        let source_count = if include_source_ids {
            ring_edges.iter().fold(0usize, |count, &directed_edge| {
                count.saturating_add(
                    self.edges[self.directed_edges[directed_edge].edge_idx]
                        .sources
                        .line_ids
                        .len(),
                )
            })
        } else {
            0
        };
        let graph_identity_bytes = if include_graph_ids {
            edge_count.saturating_mul(
                std::mem::size_of::<(NodeId, NodeId)>() + std::mem::size_of::<NodeId>(),
            )
        } else {
            0
        };
        std::mem::size_of::<ExtractedRing>()
            .saturating_add(
                edge_count
                    .saturating_add(1)
                    .saturating_mul(std::mem::size_of::<Coord3D>()),
            )
            .saturating_add(edge_count.saturating_mul(std::mem::size_of::<u32>()))
            .saturating_add(source_count.saturating_mul(std::mem::size_of::<u32>()))
            .saturating_add(graph_identity_bytes)
    }

    fn materialize_ring(
        &self,
        ring_edges: &[DirEdgeId],
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: Option<&ExecutionPolicy>,
        work_items: &mut usize,
    ) -> crate::Result<ExtractedRing> {
        let mut coords = Vec::with_capacity(ring_edges.len() + 1);
        let mut ids = Vec::with_capacity(ring_edges.len());
        let mut source_ids = Vec::new();
        let mut edge_keys = if include_graph_ids {
            Vec::with_capacity(ring_edges.len())
        } else {
            Vec::new()
        };
        let mut node_ids = if include_graph_ids {
            Vec::with_capacity(ring_edges.len())
        } else {
            Vec::new()
        };
        let face_id = self.directed_edges[ring_edges[0]].face_id;
        let start_node_idx = self.directed_edges[ring_edges[0]].src;
        coords.push(Coord3D {
            x: self.nodes_x[start_node_idx],
            y: self.nodes_y[start_node_idx],
            z: self.nodes_z[start_node_idx],
        });

        for &de_idx in ring_edges {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("ring_extraction", *work_items)?;
            }
            *work_items += 1;
            let de = &self.directed_edges[de_idx];
            let edge_idx = de.edge_idx;
            ids.push(self.edges[edge_idx].line.line_id);
            if include_source_ids {
                source_ids.extend_from_slice(&self.edges[edge_idx].sources.line_ids);
            }
            if include_graph_ids {
                edge_keys.push(if de.src < de.dst {
                    (de.src, de.dst)
                } else {
                    (de.dst, de.src)
                });
                node_ids.push(de.src);
            }

            coords.push(Coord3D {
                x: self.nodes_x[de.dst],
                y: self.nodes_y[de.dst],
                z: self.nodes_z[de.dst],
            });
        }

        source_ids.sort_unstable();
        source_ids.dedup();
        Ok(ExtractedRing {
            coords,
            line_ids: ids,
            source_line_ids: source_ids,
            face_id,
            edge_keys,
            node_ids,
        })
    }

    /// Step 1: computeNextCWEdges over every node.
    /// Edges in nodes_outgoing are in CCW order. For each pair of consecutive outgoing
    /// edges (prev, curr), set next(sym(prev)) = curr, and close the cycle.
    fn compute_next_cw_edges(
        &mut self,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<()> {
        self.clear_next_links();
        let mut valid_edges = Vec::new();
        let mut work_items = 0;
        for outgoing in &self.nodes_outgoing {
            valid_edges.clear();
            for &idx in outgoing {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                }
                work_items += 1;
                let de = &self.directed_edges[idx];
                if !de.is_marked && !self.edges[de.edge_idx].deleted {
                    valid_edges.push(idx);
                }
            }

            if valid_edges.is_empty() {
                continue;
            }

            let mut next = *valid_edges.last().unwrap();
            for &curr in &valid_edges {
                self.directed_edges[curr].next_idx = Some(next);
                next = curr;
            }
        }
        Ok(())
    }

    fn assign_deterministic_face_ids(
        &mut self,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<()> {
        self.clear_face_ids();
        let invariant = |reason| crate::PolygonizeError::InternalInvariantViolation { reason };
        let is_active = |directed_idx: DirEdgeId| {
            self.directed_edges
                .get(directed_idx)
                .and_then(|directed| {
                    self.edges
                        .get(directed.edge_idx)
                        .map(|edge| (directed, edge))
                })
                .is_some_and(|(directed, edge)| !directed.is_marked && !edge.deleted)
        };

        let mut assigned = vec![false; self.directed_edges.len()];
        let mut candidates = Vec::new();
        let mut work_items = 0;
        for start in 0..self.directed_edges.len() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("ring_extraction", work_items)?;
            }
            work_items += 1;
            if !is_active(start) || assigned[start] {
                continue;
            }

            let mut cycle = Vec::new();
            let mut current = start;
            loop {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                }
                work_items += 1;
                if !is_active(current) {
                    return Err(invariant(format!(
                        "face identity cycle {start} reached inactive directed edge {current}"
                    )));
                }
                if assigned[current] {
                    if current == start {
                        break;
                    }
                    return Err(invariant(format!(
                        "face identity cycle {start} reuses directed edge {current}"
                    )));
                }

                assigned[current] = true;
                cycle.push(current);
                let twin = self.directed_edges[current].sym_idx;
                let Some(next) = self
                    .directed_edges
                    .get(twin)
                    .and_then(|directed| directed.next_idx)
                else {
                    return Err(invariant(format!(
                        "face identity directed edge {current} has no successor at twin link {twin}"
                    )));
                };
                if next == start {
                    break;
                }
                current = next;
                if cycle.len() > self.directed_edges.len() {
                    return Err(invariant(format!(
                        "face identity cycle {start} exceeds directed edge count"
                    )));
                }
            }

            candidates.push(FaceCycleCandidate {
                key: self.face_cycle_key(&cycle),
                signed_area: self.face_cycle_signed_area(&cycle),
                directed_edges: cycle,
            });
        }

        if let Some(execution_policy) = execution_policy {
            execution_policy.check_uncancellable_sort("face_cycle_sort", candidates.len())?;
        }
        candidates.sort_unstable_by(|left, right| {
            left.key
                .cmp(&right.key)
                .then_with(|| left.signed_area.total_cmp(&right.signed_area))
        });

        self.face_count = candidates.len();
        for (face_id, candidate) in candidates.iter().enumerate() {
            if candidate.signed_area < 0.0 {
                self.unbounded_face_ids.push(face_id);
            }
            for &directed_idx in &candidate.directed_edges {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                }
                work_items += 1;
                self.directed_edges[directed_idx].face_id = Some(face_id);
            }
        }
        Ok(())
    }

    fn face_cycle_key(&self, cycle: &[DirEdgeId]) -> Vec<[u64; 2]> {
        let mut key: Vec<_> = cycle
            .iter()
            .map(|&directed_idx| {
                let node = self.directed_edges[directed_idx].src;
                [
                    canonical_coordinate_bits(self.nodes_x[node]),
                    canonical_coordinate_bits(self.nodes_y[node]),
                ]
            })
            .collect();
        let start = minimum_rotation_index(&key);
        key.rotate_left(start);
        key
    }

    fn face_cycle_signed_area(&self, cycle: &[DirEdgeId]) -> f64 {
        cycle.iter().fold(0.0, |area, &directed_idx| {
            let directed = &self.directed_edges[directed_idx];
            area + self.nodes_x[directed.src] * self.nodes_y[directed.dst]
                - self.nodes_x[directed.dst] * self.nodes_y[directed.src]
        }) * 0.5
    }

    /// Validates that active half-edges form disjoint closed cycles through
    /// the current persisted `next_idx` links.
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn validate_arrangement_ring_cycles(&self, phase: &str) -> crate::Result<usize> {
        let invariant = |reason| crate::PolygonizeError::InternalInvariantViolation { reason };

        let is_active = |directed_idx: DirEdgeId| {
            let directed = &self.directed_edges[directed_idx];
            !directed.is_marked && !self.edges[directed.edge_idx].deleted
        };

        for (directed_idx, directed) in self.directed_edges.iter().enumerate() {
            if !is_active(directed_idx) {
                continue;
            }
            let Some(successor_idx) = self.directed_edges[directed.sym_idx].next_idx else {
                return Err(invariant(format!(
                    "arrangement {phase} ring invariant directed edge {directed_idx} has no successor at twin link {}",
                    directed.sym_idx
                )));
            };
            let Some(successor) = self.directed_edges.get(successor_idx) else {
                return Err(invariant(format!(
                    "arrangement {phase} ring invariant directed edge {directed_idx} successor {successor_idx} is out of bounds"
                )));
            };
            if !is_active(successor_idx) {
                return Err(invariant(format!(
                    "arrangement {phase} ring invariant directed edge {directed_idx} successor {successor_idx} is inactive"
                )));
            }
            if directed.dst != successor.src {
                return Err(invariant(format!(
                    "arrangement {phase} ring invariant continuity mismatch: directed edge {directed_idx} ends at {}, successor {successor_idx} starts at {}",
                    directed.dst, successor.src
                )));
            }
        }

        let mut assigned_cycle = vec![None; self.directed_edges.len()];
        let mut cycle_count = 0;
        for start_idx in 0..self.directed_edges.len() {
            if !is_active(start_idx) || assigned_cycle[start_idx].is_some() {
                continue;
            }
            cycle_count += 1;

            let mut current_idx = start_idx;
            loop {
                if let Some(first_start) = assigned_cycle[current_idx] {
                    if current_idx == start_idx && first_start == start_idx {
                        break;
                    }
                    return Err(invariant(format!(
                        "arrangement {phase} ring invariant cycle {start_idx} reuses directed edge {current_idx} assigned to cycle {first_start} before closure"
                    )));
                }
                assigned_cycle[current_idx] = Some(start_idx);
                let twin_idx = self.directed_edges[current_idx].sym_idx;
                let Some(next_idx) = self.directed_edges[twin_idx].next_idx else {
                    return Err(invariant(format!(
                        "arrangement {phase} ring invariant directed edge {current_idx} has no successor at twin link {twin_idx}"
                    )));
                };
                current_idx = next_idx;
            }
        }

        for (directed_idx, assignment) in assigned_cycle.into_iter().enumerate() {
            if is_active(directed_idx) && assignment.is_none() {
                return Err(invariant(format!(
                    "arrangement {phase} ring invariant directed edge {directed_idx} is unassigned"
                )));
            }
        }

        Ok(cycle_count)
    }

    /// Assigns stable component IDs to nodes incident to active edges.
    pub(crate) fn active_component_ids(&self) -> Vec<Option<usize>> {
        self.active_component_ids_with_execution_policy(None)
            .expect("unlimited component identification cannot fail")
    }

    fn active_component_ids_with_execution_policy(
        &self,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<Vec<Option<usize>>> {
        let is_active = |directed_idx: DirEdgeId| {
            let directed = &self.directed_edges[directed_idx];
            !directed.is_marked && !self.edges[directed.edge_idx].deleted
        };
        let mut work_items = 0;
        let mut seeds = Vec::new();
        for node in 0..self.nodes_x.len() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_components", work_items)?;
            }
            work_items += 1;
            if self.nodes_outgoing[node]
                .iter()
                .any(|&directed| is_active(directed))
            {
                seeds.push(node);
            }
        }
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_uncancellable_sort("graph_component_seed_sort", seeds.len())?;
        }
        seeds.sort_unstable_by(|&a, &b| {
            self.nodes_x[a]
                .total_cmp(&self.nodes_x[b])
                .then_with(|| self.nodes_y[a].total_cmp(&self.nodes_y[b]))
                .then(a.cmp(&b))
        });

        let mut component_ids = vec![None; self.nodes_x.len()];
        let mut stack = Vec::new();
        let mut next_component_id = 0;
        for seed in seeds {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("graph_components", work_items)?;
            }
            work_items += 1;
            if component_ids[seed].is_some() {
                continue;
            }
            let component_id = next_component_id;
            next_component_id += 1;
            component_ids[seed] = Some(component_id);
            stack.push(seed);
            while let Some(node) = stack.pop() {
                for &directed_idx in &self.nodes_outgoing[node] {
                    if let Some(execution_policy) = execution_policy {
                        execution_policy.check_cancelled_every("graph_components", work_items)?;
                    }
                    work_items += 1;
                    if !is_active(directed_idx) {
                        continue;
                    }
                    let neighbor = self.directed_edges[directed_idx].dst;
                    if component_ids[neighbor].is_none() {
                        component_ids[neighbor] = Some(component_id);
                        stack.push(neighbor);
                    }
                }
            }
        }
        Ok(component_ids)
    }

    fn active_component_partitions(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<Vec<ComponentPartition>> {
        let component_ids =
            self.active_component_ids_with_execution_policy(Some(execution_policy))?;
        let component_count = component_ids
            .iter()
            .flatten()
            .max()
            .map_or(0, |component| component + 1);
        let mut nodes = vec![Vec::new(); component_count];
        for (node_idx, component_id) in component_ids.iter().enumerate() {
            if let Some(component_id) = component_id {
                nodes[*component_id].push(node_idx);
            }
        }
        for (component_id, component_nodes) in nodes.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("graph_components", component_id)?;
            execution_policy
                .check_uncancellable_sort("graph_component_node_sort", component_nodes.len())?;
            component_nodes.sort_unstable_by(|&left, &right| {
                self.nodes_x[left]
                    .total_cmp(&self.nodes_x[right])
                    .then(self.nodes_y[left].total_cmp(&self.nodes_y[right]))
                    .then(left.cmp(&right))
            });
        }

        let mut edges = vec![Vec::new(); component_count];
        for (edge_idx, edge) in self.edges.iter().enumerate() {
            execution_policy.check_cancelled_every("graph_components", edge_idx)?;
            let [forward_idx, reverse_idx] = edge.dir_edges;
            if edge.deleted
                || self.directed_edges[forward_idx].is_marked
                || self.directed_edges[reverse_idx].is_marked
            {
                continue;
            }
            let source_node = self.directed_edges[forward_idx].src;
            let component_id = component_ids[source_node].ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: format!(
                        "active graph edge {edge_idx} has no connected-component assignment"
                    ),
                }
            })?;
            edges[component_id].push(edge_idx);
        }
        for (component_id, component_edges) in edges.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("graph_components", component_id)?;
            execution_policy
                .check_uncancellable_sort("graph_component_edge_sort", component_edges.len())?;
            component_edges.sort_unstable();
        }

        Ok(nodes
            .into_iter()
            .zip(edges)
            .map(|(nodes, edges)| ComponentPartition { nodes, edges })
            .collect())
    }

    fn component_graphs_with_execution_policy(
        &self,
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<Vec<ComponentGraph>> {
        let partitions = self.active_component_partitions(execution_policy)?;
        let mut components = Vec::with_capacity(partitions.len());
        for (component_id, partition) in partitions.into_iter().enumerate() {
            execution_policy.check_cancelled_every("graph_components", component_id)?;
            let mut graph = PlanarGraph::new();
            let mut local_node_ids = HashMap::with_capacity(partition.nodes.len());
            let mut global_node_ids = Vec::with_capacity(partition.nodes.len());
            for (node_offset, global_node_id) in partition.nodes.into_iter().enumerate() {
                execution_policy.check_cancelled_every("graph_components", node_offset)?;
                let local_node_id = graph.add_node(Coord3D {
                    x: self.nodes_x[global_node_id],
                    y: self.nodes_y[global_node_id],
                    z: self.nodes_z[global_node_id],
                });
                local_node_ids.insert(global_node_id, local_node_id);
                global_node_ids.push(global_node_id);
            }

            for (edge_offset, global_edge_id) in partition.edges.into_iter().enumerate() {
                execution_policy.check_cancelled_every("graph_components", edge_offset)?;
                let edge = &self.edges[global_edge_id];
                let [forward_idx, _] = edge.dir_edges;
                let global_src = self.directed_edges[forward_idx].src;
                let global_dst = self.directed_edges[forward_idx].dst;
                let local_src = *local_node_ids.get(&global_src).ok_or_else(|| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "component edge {global_edge_id} source node {global_src} is missing"
                        ),
                    }
                })?;
                let local_dst = *local_node_ids.get(&global_dst).ok_or_else(|| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: format!(
                            "component edge {global_edge_id} destination node {global_dst} is missing"
                        ),
                    }
                })?;
                let local_edge_id = graph.add_line(edge.line);
                graph.edges[local_edge_id].sources = edge.sources.clone();
                graph.edges[local_edge_id].line.line_id = edge.line.line_id;
                debug_assert_eq!(
                    graph.directed_edges[graph.edges[local_edge_id].dir_edges[0]].src,
                    local_src
                );
                debug_assert_eq!(
                    graph.directed_edges[graph.edges[local_edge_id].dir_edges[0]].dst,
                    local_dst
                );
            }
            components.push(ComponentGraph {
                graph,
                global_node_ids,
            });
        }
        Ok(components)
    }

    pub(crate) fn process_components_with_execution_policy(
        &self,
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: &ExecutionPolicy,
        noding_postcondition_validated: bool,
        capture_byte_limit: Option<usize>,
    ) -> crate::Result<(ComponentOutput, bool)> {
        // ponytail: materialize local graphs for simple parallel ownership; scratch reuse and
        // peak-memory measurement remain the next P2.3 optimization slice.
        let mut components = self.component_graphs_with_execution_policy(execution_policy)?;
        let mut merged = ComponentOutput::default();

        let capture_truncated = if let Some(capture_byte_limit) = capture_byte_limit {
            let mut capture_budget = TraceCaptureBudget::new(capture_byte_limit);
            for component in &mut components {
                append_component_output(
                    &mut merged,
                    component.process(
                        include_graph_ids,
                        include_source_ids,
                        execution_policy,
                        noding_postcondition_validated,
                        Some(&mut capture_budget),
                    )?,
                );
            }
            capture_budget.truncated()
        } else {
            #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
            let outputs: Vec<_> = components
                .par_iter_mut()
                .map(|component| {
                    component.process(
                        include_graph_ids,
                        include_source_ids,
                        execution_policy,
                        noding_postcondition_validated,
                        None,
                    )
                })
                .collect();
            #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
            let outputs: Vec<_> = components
                .iter_mut()
                .map(|component| {
                    component.process(
                        include_graph_ids,
                        include_source_ids,
                        execution_policy,
                        noding_postcondition_validated,
                        None,
                    )
                })
                .collect();

            for output in outputs {
                append_component_output(&mut merged, output?);
            }
            false
        };

        Ok((merged, capture_truncated))
    }

    /// Validates Euler's planar relation for the active maximal-ring graph.
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn validate_arrangement_euler(&self, phase: &str) -> crate::Result<()> {
        let invariant = |reason| crate::PolygonizeError::InternalInvariantViolation { reason };
        let boundary_cycles = self.validate_arrangement_ring_cycles(phase)?;
        let component_ids = self.active_component_ids();
        let mut edge_count = 0usize;

        for edge in &self.edges {
            let [forward_idx, reverse_idx] = edge.dir_edges;
            if edge.deleted
                || self.directed_edges[forward_idx].is_marked
                || self.directed_edges[reverse_idx].is_marked
            {
                continue;
            }
            edge_count += 1;
        }

        let vertex_count = component_ids.iter().flatten().count();
        let component_count = component_ids
            .iter()
            .flatten()
            .max()
            .map_or(0, |component| component + 1);
        let face_count = boundary_cycles
            .checked_sub(component_count)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| {
                invariant(format!(
                    "arrangement {phase} Euler invariant invalid boundary count: cycles={boundary_cycles}, components={component_count}"
                ))
            })?;
        let lhs = vertex_count as i128 - edge_count as i128 + face_count as i128;
        let rhs = component_count as i128 + 1;
        if lhs != rhs {
            return Err(invariant(format!(
                "arrangement {phase} Euler invariant mismatch: vertices={vertex_count}, edges={edge_count}, faces={face_count}, components={component_count}, boundary_cycles={boundary_cycles}, lhs={lhs}, rhs={rhs}"
            )));
        }
        Ok(())
    }

    /// Step 2: find and label maximal rings.
    fn find_and_label_maximal_rings(
        &self,
        labels: &mut [i64],
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<Vec<DirEdgeId>> {
        let mut maximal_ring_starts = Vec::new();
        let mut curr_label = 1_i64;
        let mut work_items = 0;
        for start_de_idx in 0..self.directed_edges.len() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("ring_extraction", work_items)?;
            }
            work_items += 1;
            if self.directed_edges[start_de_idx].is_marked || labels[start_de_idx] >= 0 {
                continue;
            }

            maximal_ring_starts.push(start_de_idx);
            let mut curr = start_de_idx;
            loop {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                }
                work_items += 1;
                if labels[curr] >= 0 {
                    break;
                }
                labels[curr] = curr_label;

                let Some(next) = self.directed_edges[self.directed_edges[curr].sym_idx].next_idx
                else {
                    break;
                };
                if next == start_de_idx {
                    break;
                }
                curr = next;
            }
            curr_label += 1;
        }
        Ok(maximal_ring_starts)
    }

    /// Step 3: convert maximal to minimal rings by relinking intersection nodes.
    /// findIntersectionNodes + computeNextCCWEdges, scoped by ring label.
    fn convert_maximal_to_minimal_rings(
        &mut self,
        maximal_ring_starts: &[DirEdgeId],
        labels: &[i64],
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<()> {
        self.clear_face_ids();
        let mut intersection_nodes = Vec::<NodeId>::new();
        let mut seen_intersection_nodes = vec![false; self.nodes_x.len()];
        let mut work_items = 0;

        for &start_de_idx in maximal_ring_starts {
            let ring_label = labels[start_de_idx];
            if ring_label < 0 {
                continue;
            }

            intersection_nodes.clear();

            // findIntersectionNodes(startDE, label)
            let mut curr = start_de_idx;
            loop {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                }
                work_items += 1;
                let node = self.directed_edges[curr].src;

                // Degree of this node within the current ring label.
                let mut degree_for_label = 0;
                for &out_de in &self.nodes_outgoing[node] {
                    if let Some(execution_policy) = execution_policy {
                        execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                    }
                    work_items += 1;
                    if labels[out_de] == ring_label {
                        degree_for_label += 1;
                    }
                }

                if degree_for_label > 1 && !seen_intersection_nodes[node] {
                    seen_intersection_nodes[node] = true;
                    intersection_nodes.push(node);
                }

                let Some(next) = self.directed_edges[self.directed_edges[curr].sym_idx].next_idx
                else {
                    break;
                };
                if next == start_de_idx {
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
                    if let Some(execution_policy) = execution_policy {
                        execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                    }
                    work_items += 1;
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
                            let link_idx = self.directed_edges[prev_in_idx].sym_idx;
                            self.directed_edges[link_idx].next_idx = Some(out_de_idx);
                        }
                        if first_out.is_none() {
                            first_out = Some(out_de_idx);
                        }
                    }
                }

                if let (Some(prev_in_idx), Some(first_out_idx)) = (prev_in, first_out) {
                    let link_idx = self.directed_edges[prev_in_idx].sym_idx;
                    self.directed_edges[link_idx].next_idx = Some(first_out_idx);
                }

                seen_intersection_nodes[node] = false;
            }
        }
        Ok(())
    }

    /// Extracts valid rings by following persisted directed-edge `next_idx` links.
    fn extract_valid_rings(
        &mut self,
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<Vec<ExtractedRing>> {
        for (de_idx, de) in self.directed_edges.iter_mut().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("ring_extraction", de_idx)?;
            }
            de.is_visited = false;
        }

        // Reuse vector to avoid allocations
        let mut ring_edges = Vec::new();
        let mut rings = Vec::new();
        let mut work_items = 0;

        for start_de_idx in 0..self.directed_edges.len() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("ring_extraction", work_items)?;
            }
            work_items += 1;
            if self.directed_edges[start_de_idx].is_visited
                || self.directed_edges[start_de_idx].is_marked
            {
                continue;
            }

            if self.edges[self.directed_edges[start_de_idx].edge_idx].deleted {
                continue;
            }

            ring_edges.clear();
            let mut curr_de_idx = start_de_idx;
            let mut is_valid_ring = true;

            loop {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("ring_extraction", work_items)?;
                }
                work_items += 1;
                let curr_de = &mut self.directed_edges[curr_de_idx];
                curr_de.is_visited = true;
                ring_edges.push(curr_de_idx);

                let Some(next_de_idx) =
                    self.directed_edges[self.directed_edges[curr_de_idx].sym_idx].next_idx
                else {
                    is_valid_ring = false;
                    break;
                };

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
                rings.push(self.materialize_ring(
                    &ring_edges,
                    include_graph_ids,
                    include_source_ids,
                    execution_policy,
                    &mut work_items,
                )?);
            }
        }
        Ok(rings)
    }
}

impl ComponentGraph {
    fn process(
        &mut self,
        include_graph_ids: bool,
        include_source_ids: bool,
        execution_policy: &ExecutionPolicy,
        noding_postcondition_validated: bool,
        capture_budget: Option<&mut TraceCaptureBudget>,
    ) -> crate::Result<ComponentOutput> {
        self.graph
            .sort_edges_with_execution_policy(execution_policy)?;
        let dangles = self
            .graph
            .prune_dangles_with_execution_policy(execution_policy)?;
        let cut_edges = self.graph.delete_cut_edges_with_execution_policy(
            execution_policy,
            noding_postcondition_validated,
        )?;
        let (mut maximal, mut minimal) = if let Some(capture_budget) = capture_budget {
            let (maximal, minimal, _) = self
                .graph
                .get_edge_rings_with_maximal_and_execution_policy_with_budget(
                    include_graph_ids,
                    include_source_ids,
                    execution_policy,
                    capture_budget,
                    noding_postcondition_validated,
                )?;
            (maximal, minimal)
        } else {
            (
                Vec::new(),
                self.graph
                    .get_edge_rings_with_graph_ids_and_execution_policy(
                        include_graph_ids,
                        include_source_ids,
                        execution_policy,
                        noding_postcondition_validated,
                    )?,
            )
        };
        self.remap_rings(&mut maximal, include_graph_ids)?;
        self.remap_rings(&mut minimal, include_graph_ids)?;
        Ok((dangles, cut_edges, maximal, minimal))
    }

    fn remap_rings(
        &self,
        rings: &mut [ExtractedRing],
        include_graph_ids: bool,
    ) -> crate::Result<()> {
        if !include_graph_ids {
            return Ok(());
        }
        for ring in rings {
            for (start, end) in &mut ring.edge_keys {
                *start = *self.global_node_ids.get(*start).ok_or_else(|| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: "component ring edge key references missing local node".to_string(),
                    }
                })?;
                *end = *self.global_node_ids.get(*end).ok_or_else(|| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: "component ring edge key references missing local node".to_string(),
                    }
                })?;
            }
            for node_id in &mut ring.node_ids {
                *node_id = *self.global_node_ids.get(*node_id).ok_or_else(|| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: "component ring references missing local node".to_string(),
                    }
                })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod arrangement_ring_invariant_tests {
    use super::*;

    fn next_links(graph: &mut PlanarGraph) -> Vec<DirEdgeId> {
        graph.compute_next_cw_edges(None).unwrap();
        graph
            .directed_edges
            .iter()
            .map(|directed| directed.next_idx.unwrap_or(usize::MAX))
            .collect()
    }

    fn face_snapshot(graph: &PlanarGraph) -> Vec<(Vec<[u64; 2]>, bool)> {
        let mut snapshot = Vec::new();
        for face_id in 0..graph.face_count {
            let start = graph
                .directed_edges
                .iter()
                .position(|directed| directed.face_id == Some(face_id))
                .unwrap();
            let mut cycle = Vec::new();
            let mut current = start;
            loop {
                cycle.push(current);
                let next = graph.directed_edges[graph.directed_edges[current].sym_idx]
                    .next_idx
                    .unwrap();
                current = next;
                if current == start {
                    break;
                }
            }
            snapshot.push((
                graph.face_cycle_key(&cycle),
                graph.unbounded_face_ids.contains(&face_id),
            ));
        }
        snapshot
    }

    fn add_triangle(graph: &mut PlanarGraph, points: [Coord3D; 3], first_line_id: u32) {
        for offset in 0..3 {
            graph.add_line(Line3D::new(
                points[offset],
                points[(offset + 1) % 3],
                first_line_id + offset as u32,
            ));
        }
    }

    fn component_snapshot(lines: &[Line3D]) -> Vec<(f64, f64, usize)> {
        let mut graph = PlanarGraph::new();
        for &line in lines {
            graph.add_line(line);
        }
        let component_ids = graph.active_component_ids();
        let mut snapshot: Vec<_> = component_ids
            .into_iter()
            .enumerate()
            .filter_map(|(node, component)| {
                component.map(|component| (graph.nodes_x[node], graph.nodes_y[node], component))
            })
            .collect();
        snapshot.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        snapshot
    }

    fn invariant_reason(graph: &PlanarGraph, phase: &str) -> String {
        match graph.validate_arrangement_ring_cycles(phase).unwrap_err() {
            crate::PolygonizeError::InternalInvariantViolation { reason } => reason,
            error => panic!("unexpected validation error: {error}"),
        }
    }

    #[test]
    fn arrangement_ring_validator_accepts_maximal_and_minimal_cycles() {
        let mut graph = PlanarGraph::new();
        let center = Coord3D::new(0.0, 0.0, 0.0);
        add_triangle(
            &mut graph,
            [
                center,
                Coord3D::new(2.0, 0.0, 0.0),
                Coord3D::new(1.0, 1.0, 0.0),
            ],
            10,
        );
        add_triangle(
            &mut graph,
            [
                center,
                Coord3D::new(-2.0, 0.0, 0.0),
                Coord3D::new(-1.0, -1.0, 0.0),
            ],
            20,
        );
        graph.sort_edges();

        next_links(&mut graph);
        graph.validate_arrangement_ring_cycles("maximal").unwrap();

        let mut labels = vec![-1_i64; graph.directed_edges.len()];
        let starts = graph
            .find_and_label_maximal_rings(&mut labels, None)
            .unwrap();
        graph
            .convert_maximal_to_minimal_rings(&starts, &labels, None)
            .unwrap();
        graph.validate_arrangement_ring_cycles("minimal").unwrap();
    }

    #[test]
    fn directed_edge_next_links_persist_and_are_deterministic() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(2.0, 0.0, 0.0), 10),
            Line3D::new(Coord3D::new(2.0, 0.0, 0.0), Coord3D::new(2.0, 2.0, 0.0), 11),
            Line3D::new(Coord3D::new(2.0, 2.0, 0.0), Coord3D::new(0.0, 2.0, 0.0), 12),
            Line3D::new(Coord3D::new(0.0, 2.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 13),
        ];

        let mut first = PlanarGraph::new();
        first.bulk_load(lines.clone());
        first.sort_edges();
        assert!(first
            .directed_edges
            .iter()
            .all(|directed| directed.next_idx.is_none()));
        first.compute_next_cw_edges(None).unwrap();
        let first_links: Vec<_> = first
            .directed_edges
            .iter()
            .map(|directed| directed.next_idx)
            .collect();
        assert!(first_links.iter().all(Option::is_some));

        let mut reversed = lines;
        reversed.reverse();
        let mut second = PlanarGraph::new();
        second.bulk_load(reversed);
        second.sort_edges();
        second.compute_next_cw_edges(None).unwrap();
        let second_links: Vec<_> = second
            .directed_edges
            .iter()
            .map(|directed| directed.next_idx)
            .collect();
        assert_eq!(first_links, second_links);

        first.reset_traversal_state();
        assert!(first
            .directed_edges
            .iter()
            .all(|directed| directed.next_idx.is_none()));
    }

    #[test]
    fn face_ids_are_deterministic_and_mark_the_outer_cycle() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(2.0, 0.0, 0.0), 10),
            Line3D::new(Coord3D::new(2.0, 0.0, 0.0), Coord3D::new(2.0, 2.0, 0.0), 11),
            Line3D::new(Coord3D::new(2.0, 2.0, 0.0), Coord3D::new(0.0, 2.0, 0.0), 12),
            Line3D::new(Coord3D::new(0.0, 2.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 13),
        ];

        let mut first = PlanarGraph::new();
        for line in lines.iter().copied() {
            first.add_line(line);
        }
        first.sort_edges();
        first.compute_next_cw_edges(None).unwrap();
        first.assign_deterministic_face_ids(None).unwrap();
        assert_eq!(first.face_count, 2);
        assert_eq!(first.unbounded_face_ids.len(), 1);
        assert!(first
            .directed_edges
            .iter()
            .all(|directed| directed.face_id.is_some()));
        let expected = face_snapshot(&first);

        let mut second = PlanarGraph::new();
        for line in lines.into_iter().rev() {
            second.add_line(line);
        }
        second.sort_edges();
        second.compute_next_cw_edges(None).unwrap();
        second.assign_deterministic_face_ids(None).unwrap();
        assert_eq!(face_snapshot(&second), expected);

        first.reset_traversal_state();
        assert_eq!(first.face_count, 0);
        assert!(first.unbounded_face_ids.is_empty());
        assert!(first
            .directed_edges
            .iter()
            .all(|directed| directed.face_id.is_none()));
    }

    #[test]
    fn extracted_rings_retain_deterministic_face_and_boundary_payloads() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 1.0), Coord3D::new(2.0, 0.0, 2.0), 10),
            Line3D::new(Coord3D::new(2.0, 0.0, 2.0), Coord3D::new(2.0, 2.0, 3.0), 11),
            Line3D::new(Coord3D::new(2.0, 2.0, 3.0), Coord3D::new(0.0, 2.0, 4.0), 12),
            Line3D::new(Coord3D::new(0.0, 2.0, 4.0), Coord3D::new(0.0, 0.0, 1.0), 13),
        ];

        let snapshot = |lines: Vec<Line3D>| {
            let mut graph = PlanarGraph::new();
            graph.bulk_load(lines);
            graph.sort_edges();
            let mut rings = graph.get_edge_rings_with_graph_ids(true, true);
            rings.sort_unstable_by_key(|ring| ring.face_id);
            rings
                .into_iter()
                .map(|ring| {
                    (
                        ring.face_id,
                        ring.source_line_ids,
                        ring.coords
                            .iter()
                            .map(|coord| coord.z.to_bits())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };

        let mut reversed = lines.clone();
        reversed.reverse();
        let first = snapshot(lines);
        let second = snapshot(reversed);

        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|(face_id, _, _)| face_id.is_some()));
        assert!(first
            .iter()
            .all(|(_, source_line_ids, _)| source_line_ids == &[10, 11, 12, 13]));
        assert!(first.iter().all(|(_, _, z_bits)| {
            let mut sorted = z_bits[..z_bits.len() - 1].to_vec();
            sorted.sort_unstable();
            sorted
                == [
                    1.0f64.to_bits(),
                    2.0f64.to_bits(),
                    3.0f64.to_bits(),
                    4.0f64.to_bits(),
                ]
        }));
    }

    #[test]
    fn arrangement_ring_validator_reports_deterministic_witnesses() {
        let mut graph = PlanarGraph::new();
        let center = Coord3D::new(0.0, 0.0, 0.0);
        for (line_id, end) in [
            (10, Coord3D::new(10.0, 0.0, 0.0)),
            (20, Coord3D::new(0.0, 10.0, 0.0)),
            (30, Coord3D::new(-10.0, 0.0, 0.0)),
        ] {
            graph.add_line(Line3D::new(center, end, line_id));
        }
        graph.sort_edges();
        let links = next_links(&mut graph);
        let twin_of_zero = graph.directed_edges[0].sym_idx;

        graph.directed_edges[twin_of_zero].next_idx = None;
        assert_eq!(
            invariant_reason(&graph, "maximal"),
            "arrangement maximal ring invariant directed edge 0 has no successor at twin link 1"
        );

        for (directed, &next) in graph.directed_edges.iter_mut().zip(&links) {
            directed.next_idx = (next != usize::MAX).then_some(next);
        }
        graph.directed_edges[twin_of_zero].next_idx = Some(0);
        assert_eq!(
            invariant_reason(&graph, "maximal"),
            "arrangement maximal ring invariant continuity mismatch: directed edge 0 ends at 1, successor 0 starts at 0"
        );

        for (directed, &next) in graph.directed_edges.iter_mut().zip(&links) {
            directed.next_idx = (next != usize::MAX).then_some(next);
        }
        graph.directed_edges[2].next_idx = Some(4);
        assert_eq!(
            invariant_reason(&graph, "maximal"),
            "arrangement maximal ring invariant cycle 0 reuses directed edge 4 assigned to cycle 0 before closure"
        );
    }

    #[test]
    fn arrangement_euler_validator_counts_components_and_the_unbounded_face() {
        let mut graph = PlanarGraph::new();
        add_triangle(
            &mut graph,
            [
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(2.0, 0.0, 0.0),
                Coord3D::new(1.0, 1.0, 0.0),
            ],
            10,
        );
        add_triangle(
            &mut graph,
            [
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(12.0, 0.0, 0.0),
                Coord3D::new(11.0, 1.0, 0.0),
            ],
            20,
        );
        graph.sort_edges();

        next_links(&mut graph);
        graph.validate_arrangement_euler("maximal").unwrap();
        graph.assign_deterministic_face_ids(None).unwrap();
        assert_eq!(graph.face_count, 4);
        assert_eq!(graph.unbounded_face_ids.len(), 2);
    }

    #[test]
    fn active_component_ids_are_stable_across_insertion_order() {
        let a = [
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(2.0, 0.0, 0.0),
            Coord3D::new(1.0, 1.0, 0.0),
        ];
        let b = [
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(12.0, 0.0, 0.0),
            Coord3D::new(11.0, 1.0, 0.0),
        ];
        let mut lines = Vec::new();
        for (line_id, points) in [(10, a), (20, b)] {
            for offset in 0..3 {
                lines.push(Line3D::new(
                    points[offset],
                    points[(offset + 1) % 3],
                    line_id + offset as u32,
                ));
            }
        }
        let expected = component_snapshot(&lines);
        lines.reverse();

        assert_eq!(component_snapshot(&lines), expected);
        assert!(expected
            .iter()
            .all(|(x, _, component)| (*x < 10.0 && *component == 0)
                || (*x >= 10.0 && *component == 1)));
    }

    #[test]
    fn arrangement_euler_validator_rejects_unnoded_crossings() {
        let mut graph = PlanarGraph::new();
        let points = [
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(2.0, 0.0, 0.0),
            Coord3D::new(2.0, 2.0, 0.0),
            Coord3D::new(0.0, 2.0, 0.0),
        ];
        for index in 0..4 {
            graph.add_line(Line3D::new(
                points[index],
                points[(index + 1) % 4],
                index as u32,
            ));
        }
        graph.add_line(Line3D::new(points[0], points[2], 4));
        graph.add_line(Line3D::new(points[1], points[3], 5));
        graph.sort_edges();

        next_links(&mut graph);
        let reason = match graph.validate_arrangement_euler("maximal").unwrap_err() {
            crate::PolygonizeError::InternalInvariantViolation { reason } => reason,
            error => panic!("unexpected validation error: {error}"),
        };
        assert_eq!(
            reason,
            "arrangement maximal Euler invariant mismatch: vertices=4, edges=6, faces=2, components=1, boundary_cycles=2, lhs=0, rhs=2"
        );
    }
}
