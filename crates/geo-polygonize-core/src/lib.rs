//! A native Rust port of the JTS/GEOS polygonization algorithm.
//!
//! This crate allows you to reconstruct valid polygons from a set of lines,
//! including handling of complex topologies like holes, nested shells, and disconnected components.
//!
//! # Features
//! - **Robust Noding**: Uses Iterated Snap Rounding to handle dirty inputs.
//! - **Performance**: SIMD-accelerated predicates and efficient memory layout.
//! - **Wasm**: Optimized for WebAssembly environments.

#[doc(hidden)]
pub mod containment;
mod diagnostics;
mod error;
// Kept compiler-public for the repository's microbenchmarks; not a supported API.
#[doc(hidden)]
pub mod graph;
mod index;
// Kept compiler-public for the repository's microbenchmarks; not a supported API.
#[doc(hidden)]
pub mod noding;
mod options;
mod polygonizer;
#[doc(hidden)]
pub mod tiling;
mod types;
// Kept compiler-public for the repository's microbenchmarks; not a supported API.
#[doc(hidden)]
pub mod utils;

#[cfg(test)]
mod polygonizer_tests;

pub use diagnostics::{
    ContainmentStats, IntersectionStats, NodingIterationStats, NodingWorkStats,
    PolygonizerDiagnostics, PolygonizerPhaseTimes, SnapStats, ZConflictStats,
};
pub use error::{PolygonizeError, Result};
pub use options::{
    ContainmentOptions, DeterminismOptions, DiagnosticsOptions, NodingBackend, NodingGuarantee,
    NodingOptions, OutputFilterOptions, PolygonizerOptions, PrecisionModel, ProvenanceOptions,
    SnapStrategy, TouchPolicy, ZOptions, ZPolicy,
};
#[doc(hidden)]
pub use options::{DedupPolicy, TileOwnershipPolicy};
#[doc(hidden)]
pub use polygonizer::Polygonizer;
pub use polygonizer::{
    polygonize, polygonize_line_strings, polygonize_to_multi_polygon, polygonize_with_workspace,
    PolygonizerResult, PolygonizerWorkspace,
};
#[doc(hidden)]
pub use tiling::{StitchingReport, TileReport, TiledPolygonizeResult, TiledPolygonizer};
pub use types::{Coord3D, Line3D, Polygon3D, PolygonProvenance};
