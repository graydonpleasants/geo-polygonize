# Migrating from 0.x to 1.0

`geo-polygonize 1.x` keeps the supported polygonization facade deliberately
narrow. The Rust crate root, documented npm exports, and documented Python
helpers are supported; graph, noding, trace, differential, tiling, and utility
internals remain research surfaces.

## Rust

The preferred entrypoint is the canonical-options facade:

```rust
use geo_polygonize_core::{polygonize_line_strings, PolygonizerOptions};

let result = polygonize_line_strings(line_strings, &PolygonizerOptions::default())?;
```

Use `polygonize` with `Line3D` values when source IDs or Z values are part of
the application contract. Keep the full `PolygonizerResult` when dangles, cut
edges, invalid rings, provenance, or diagnostics are needed. Use
`into_multi_polygon` only when those result families are intentionally
discarded.

The old compatibility aliases are gone. Use `NodingBackend::Snap` with the
selected precision model instead of `NodingBackend::Advanced`, and use
`TileOwnershipPolicy::RepresentativePointInsidePolygon` instead of the retired
canonical-boundary-hash alias.

## Precision and noding

`node_input = true` with iterative grid noding is explicitly unchecked. It is
useful for dirty linework but does not provide a certified snap-rounding
guarantee. Select `NodingGuarantee::Validate` when the input must satisfy the
independent full-noding postcondition. Select
`NodingGuarantee::CertifiedFixedPrecision` with `FixedGrid`, the `Snap` backend,
and the `Grid` strategy for certified hot-pixel fixed-precision processing.

## Python and WebAssembly

Use the canonical options helpers (`polygonize_with_options` in Python and
`polygonizeReportWithOptions` in WebAssembly) when the full versioned report is
needed. Legacy positional wrappers remain compatibility shims, but their
precision arguments are translated only when input noding is enabled.

The npm package's standard and slim entry points have the same semantic report
contract. The Python package requires CPython through the `abi3-py38` wheel
contract; PyPy is not a supported target.

## Release and compatibility policy

The stable root facade follows semantic versioning. Patch releases do not
raise the documented Rust MSRV or change supported semantics. The exact MSRV
and public root exports are checked in CI. Compiler-public research items are
not stabilized by being visible to Rust code or by appearing in generated
documentation.
