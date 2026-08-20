//! A native Rust port of the JTS/GEOS polygonization algorithm.
//!
//! This crate allows you to reconstruct valid polygons from a set of lines,
//! including handling of complex topologies like holes, nested shells, and disconnected components.
//!
//! The supported 1.x facade is the non-hidden API exported at this crate root.
//! Graph, noding, containment, tiling, trace, differential, utility, and
//! mutable-builder modules are compiler-public research surfaces for repository
//! tooling and are not covered by the support policy. The checked-in
//! `release/stable-api-v1.txt` allowlist records the supported root exports.
//!
//! # Features
//! - **Unchecked iterative noding**: Grid iteration is available for dirty
//!   linework, but does not claim a certified snap-rounding guarantee.
//! - **Certified fixed-precision noding**: Hot-pixel snap rounding plus an
//!   independent full-noding validation is available when explicitly selected.
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

#[doc(hidden)]
pub use diagnostics::ComponentMemoryStats;
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
    StitchingReport, TileBoundarySide, TileComponentConnection, TileCoverageGuarantee,
    TileCoverageIssue, TileCoverageResolution, TileCoverageResolutionKind,
    TileExcludedComponentIssue, TileExecutionPolicy, TileInputBoundaryIssue,
    TileOwnershipDomainIssue, TileReport, TileRetryAttempt, TileRetryPolicy, TiledPolygonizeError,
    TiledPolygonizeResult, TiledPolygonizer, TiledStitchedOutput, TracedTiledPolygonizeResultV1,
};
pub use types::{Coord3D, Line3D, Polygon3D, PolygonProvenance};
