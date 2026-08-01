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
#[doc(hidden)]
pub mod differential;
mod error;
#[doc(hidden)]
pub mod fingerprint;
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
#[doc(hidden)]
pub mod trace;
mod types;
// Kept compiler-public for the repository's microbenchmarks; not a supported API.
#[doc(hidden)]
pub mod utils;

#[cfg(test)]
mod polygonizer_tests;

pub use diagnostics::{
    ContainmentStats, IntersectionStats, NodingIterationStats, NodingWorkStats,
    PolygonizerDiagnostics, PolygonizerPhaseTimes, SnapStats, ZConflictStats,
    POLYGONIZER_DIAGNOSTICS_V1_SCHEMA_VERSION,
};
pub use error::{NodingValidationKind, PolygonizeError, PolygonizeErrorKind, Result};
#[doc(hidden)]
pub use fingerprint::{
    normalize_polygonize_error, CoordinateFingerprintV1, ErrorWitnessV1, FingerprintDiffV1,
    NormalizedPolygonizeErrorV1, TopologyFingerprintV1, TOPOLOGY_FINGERPRINT_V1_SCHEMA_VERSION,
};
pub use options::{
    CancellationToken, ContainmentOptions, DeterminismOptions, DiagnosticsOptions, ExecutionPolicy,
    NodingBackend, NodingGuarantee, NodingOptions, OutputFilterOptions, PolygonizerOptions,
    PrecisionModel, ProvenanceOptions, SnapStrategy, TouchPolicy, ZOptions, ZPolicy,
};
#[doc(hidden)]
pub use options::{DedupPolicy, TileOwnershipPolicy};
#[doc(hidden)]
pub use polygonizer::Polygonizer;
pub use polygonizer::{
    polygonize, polygonize_line_strings, polygonize_line_strings_with_execution_policy,
    polygonize_to_multi_polygon, polygonize_with_execution_policy, polygonize_with_trace,
    polygonize_with_trace_limits, polygonize_with_workspace,
    polygonize_with_workspace_and_execution_policy, PolygonizerResult, PolygonizerWorkspace,
};
#[doc(hidden)]
pub use tiling::{
    StitchingReport, TileBoundarySide, TileCoverageGuarantee, TileCoverageIssue,
    TileExcludedComponentIssue, TileInputBoundaryIssue, TileReport, TiledPolygonizeError,
    TiledPolygonizeResult, TiledPolygonizer, TracedTiledPolygonizeResultV1,
};
pub use types::{Coord3D, Line3D, Polygon3D, PolygonProvenance};
