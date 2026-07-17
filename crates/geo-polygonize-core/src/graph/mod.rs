pub mod planar_graph;
pub(crate) use planar_graph::ExtractedRing;
pub use planar_graph::{DirEdgeId, EdgeId, NodeId, PlanarGraph};

#[cfg(test)]
mod tests;
