pub(crate) mod layout_benchmark;
pub mod partition_border;
pub mod planar_graph;
pub use layout_benchmark::AdjacencyLayoutBenchmark;
pub(crate) use planar_graph::ExtractedRing;
pub use planar_graph::{DirEdgeId, EdgeId, NodeId, PlanarGraph};

#[cfg(test)]
mod tests;
