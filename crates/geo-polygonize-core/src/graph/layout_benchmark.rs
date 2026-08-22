use super::planar_graph::{DirEdgeId, PlanarGraph};
use crate::{PolygonizeError, Result};
use std::hint::black_box;
use std::time::Instant;

// ponytail: benchmark the smallest shared adjacency operations; integrate the
// layout only after end-to-end production evidence justifies replacing lists.
pub(crate) struct CsrAdjacency {
    pub(crate) offsets: Vec<usize>,
    pub(crate) directed_edges: Vec<DirEdgeId>,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct AdjacencyLayoutBenchmark {
    pub candidate_id: &'static str,
    pub conformance: bool,
    pub samples: usize,
    pub node_count: usize,
    pub nested_storage_words: usize,
    pub csr_storage_words: usize,
    pub nested_traversal_p50_ns: u64,
    pub csr_materialization_ns: u64,
    pub csr_traversal_p50_ns: u64,
}

trait AdjacencyView {
    fn node_count(&self) -> usize;
    fn outgoing(&self, node: usize) -> &[DirEdgeId];
}

impl AdjacencyView for PlanarGraph {
    fn node_count(&self) -> usize {
        self.nodes_outgoing.len()
    }

    fn outgoing(&self, node: usize) -> &[DirEdgeId] {
        &self.nodes_outgoing[node]
    }
}

impl AdjacencyView for CsrAdjacency {
    fn node_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    fn outgoing(&self, node: usize) -> &[DirEdgeId] {
        &self.directed_edges[self.offsets[node]..self.offsets[node + 1]]
    }
}

impl CsrAdjacency {
    pub(crate) fn from_graph(graph: &PlanarGraph) -> Self {
        let mut offsets = Vec::with_capacity(graph.nodes_outgoing.len() + 1);
        let mut directed_edges = Vec::new();
        offsets.push(0);
        for outgoing in &graph.nodes_outgoing {
            directed_edges.extend_from_slice(outgoing);
            offsets.push(directed_edges.len());
        }
        Self {
            offsets,
            directed_edges,
        }
    }

    #[cfg(test)]
    pub(crate) fn active_component_ids(&self, graph: &PlanarGraph) -> Vec<Option<usize>> {
        active_component_ids(self, graph)
    }

    #[cfg(test)]
    pub(crate) fn outgoing(&self, node: usize) -> &[DirEdgeId] {
        &self.directed_edges[self.offsets[node]..self.offsets[node + 1]]
    }

    #[cfg(test)]
    pub(crate) fn next_links(&self, graph: &PlanarGraph) -> Vec<Option<DirEdgeId>> {
        next_links(self, graph)
    }
}

fn is_active(graph: &PlanarGraph, directed_idx: DirEdgeId) -> bool {
    let directed = &graph.directed_edges[directed_idx];
    !directed.is_marked && !graph.edges[directed.edge_idx].deleted
}

fn active_component_ids<V: AdjacencyView>(view: &V, graph: &PlanarGraph) -> Vec<Option<usize>> {
    let mut seeds = (0..view.node_count())
        .filter(|&node| {
            view.outgoing(node)
                .iter()
                .copied()
                .any(|edge| is_active(graph, edge))
        })
        .collect::<Vec<_>>();
    seeds.sort_unstable_by(|&left, &right| {
        graph.nodes_x[left]
            .total_cmp(&graph.nodes_x[right])
            .then_with(|| graph.nodes_y[left].total_cmp(&graph.nodes_y[right]))
            .then(left.cmp(&right))
    });

    let mut component_ids = vec![None; view.node_count()];
    let mut stack = Vec::new();
    let mut next_component_id = 0;
    for seed in seeds {
        if component_ids[seed].is_some() {
            continue;
        }
        let component_id = next_component_id;
        next_component_id += 1;
        component_ids[seed] = Some(component_id);
        stack.push(seed);
        while let Some(node) = stack.pop() {
            for &directed_idx in view.outgoing(node) {
                if !is_active(graph, directed_idx) {
                    continue;
                }
                let neighbor = graph.directed_edges[directed_idx].dst;
                if component_ids[neighbor].is_none() {
                    component_ids[neighbor] = Some(component_id);
                    stack.push(neighbor);
                }
            }
        }
    }
    component_ids
}

fn next_links<V: AdjacencyView>(view: &V, graph: &PlanarGraph) -> Vec<Option<DirEdgeId>> {
    let mut links = vec![None; graph.directed_edges.len()];
    for node in 0..view.node_count() {
        let active = view
            .outgoing(node)
            .iter()
            .copied()
            .filter(|&directed_idx| is_active(graph, directed_idx))
            .collect::<Vec<_>>();
        let Some(&last) = active.last() else {
            continue;
        };
        let mut next = last;
        for directed_idx in active {
            links[directed_idx] = Some(next);
            next = directed_idx;
        }
    }
    links
}

fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u64::MAX as u128) as u64
}

fn median_low(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[(values.len() - 1) / 2]
}

pub(crate) fn benchmark(graph: &PlanarGraph, samples: usize) -> Result<AdjacencyLayoutBenchmark> {
    if samples == 0 {
        return Err(PolygonizeError::InvalidArgumentType {
            field: "samples".to_string(),
            expected: "a positive integer".to_string(),
            actual: "0".to_string(),
        });
    }

    let materialization_started = Instant::now();
    let csr = CsrAdjacency::from_graph(graph);
    let csr_materialization_ns = elapsed_ns(materialization_started);
    let mut nested_samples = Vec::with_capacity(samples);
    let mut csr_samples = Vec::with_capacity(samples);

    for _ in 0..samples {
        let nested_started = Instant::now();
        let nested_components = black_box(active_component_ids(graph, graph));
        let nested_next = black_box(next_links(graph, graph));
        nested_samples.push(elapsed_ns(nested_started));

        let csr_started = Instant::now();
        let csr_components = black_box(active_component_ids(&csr, graph));
        let csr_next = black_box(next_links(&csr, graph));
        csr_samples.push(elapsed_ns(csr_started));

        if nested_components != csr_components || nested_next != csr_next {
            return Err(PolygonizeError::InternalInvariantViolation {
                reason: "packed CSR adjacency diverged from nested adjacency".to_string(),
            });
        }
    }

    let nested_storage_words = graph.nodes_outgoing.capacity() * 3
        + graph
            .nodes_outgoing
            .iter()
            .map(Vec::capacity)
            .sum::<usize>();
    let csr_storage_words = csr.offsets.len() + csr.directed_edges.len();
    Ok(AdjacencyLayoutBenchmark {
        candidate_id: "packed-csr-adjacency-v1",
        conformance: true,
        samples,
        node_count: graph.nodes_outgoing.len(),
        nested_storage_words,
        csr_storage_words,
        nested_traversal_p50_ns: median_low(&mut nested_samples),
        csr_materialization_ns,
        csr_traversal_p50_ns: median_low(&mut csr_samples),
    })
}
