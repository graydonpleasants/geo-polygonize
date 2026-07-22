# Engineering Roadmap

This is the current, evidence-gated roadmap for `geo-polygonize`. The original
phase plan has been delivered where noted below; its detailed implementation
and issue plans are retained as historical records, not active commitments.

## Delivered

The releases from `0.40.0` through `0.51.2` established the supported
foundation:

- stateless one-shot polygonization and reusable allocation-only workspaces;
- one validated, serde-defaulted `PolygonizerOptions` schema across Rust,
  Python, and Wasm;
- finite-coordinate validation and scale-independent topology validity, with
  explicit optional minimum-area filtering;
- explicit floating and fixed precision models, full-noding validation, and
  certified hot-pixel noding alongside iterative grid noding, which remains
  documented as unchecked;
- deterministic output, strict read-only golden comparisons, typed binding
  errors, diagnostics, provenance, and explicit Z policies;
- experimental tiled polygonization with named ownership/dedup policies,
  collision-safe canonical deduplication, deterministic output, and outcome
  reporting;
- dedicated core, Arrow/GeoArrow/GeoParquet, Python, and FlatGeoBuf adapter
  crates;
- native and Wasm benchmarks, fuzzing, differential tests, supply-chain
  checks, release provenance, and documented Wasm deployment requirements.

## Next

Work these in order. Each item should remain a small, independently releasable
change.

### 1. Broaden the golden corpus

- [x] Add explicit expected topology metrics to every golden fixture.
- [x] Cover benchmark-sized inputs in addition to the existing basic, topology,
  provenance, dirty-input, Z-policy, tiling, and compatibility cases.
- [x] Reuse the same fixtures in canonical, benchmark, and differential paths
  where their contracts overlap.

Done means a missing expected result fails, all result families (including cut
edges and provenance) are compared, and every checked-in golden runs in
canonical mode. The harness already enforces read-only expected results and
full result-family comparisons; the remaining work is breadth, explicit metric
fields, and reuse.

### 2. Add a compatibility corpus

- [x] Classify cases as expected parity, expected divergence, or invalid /
  ambiguous input.
- [x] Record GEOS/Shapely comparison results without turning compatibility
  observations into unsupported robustness guarantees.
- [x] Exercise `grid`, `geos_compat`, and certified fixed-precision policies on
  the same inputs.

Done means regressions can be distinguished from documented semantic
differences without relying on ad hoc generated cases.

### 3. Measure before optimizing

- [x] Compare scalar and existing portable-SIMD kernels on supported native
  targets using the checked-in benchmarks.
- [x] Keep the existing architecture-aware dispatch; the measurements do not
  justify another native backend or dispatch layer.

The latest [x86-64 and AArch64 Criterion run](https://github.com/graydonpleasants/geo-polygonize/actions/runs/29914817090)
confirmed that no single point-in-ring kernel wins across architectures and
ring sizes. Representative repeated-query medians were:

| Ring edges | Linux x86-64 | Linux AArch64 |
|---:|---:|---:|
| 128 | wide 133.92 µs; scalar 152.64 µs | scalar 127.86 µs; wide 220.91 µs |
| 256 | wide 265.41 µs; scalar 277.56 µs | scalar 253.86 µs; wide 444.95 µs |
| 1,024 | scalar 1.022 ms; wide 1.064 ms | scalar 1.029 ms; wide 1.765 ms |

This supports the existing short-ring SIMD/long-ring scalar crossover on
x86-64 and scalar path on Linux AArch64. The same run confirmed that forced
grid versus brute-force SIMD noding still varies by workload shape, so the
existing workload-aware `Auto` policy remains preferable to architecture-only
selection.

Wasm SIMD feature selection already exists. This item concerns native runtime
dispatch; AVX2, AVX-512, or GPU backends are not commitments.

## Evidence-gated later work

These are valid directions, but none should begin without a concrete consumer,
representative data, and a measured limitation in the current implementation:

- production tiling equivalence and recovery contracts;
- streaming or out-of-core processing;
- deeper zero-copy paths across Rust, Python, and Wasm;
- distributed-compute and database adapters;
- a different advanced noder after exact `SnapNoder` profiling demonstrates a
  scaling problem;
- graph-native overlay, topology-preserving simplification, buffering, or
  direct MVT/TopoJSON emission;
- incremental topology, geodesic algorithms, or GPU compute.

Crate splitting is complete for the current adapters. New crates should follow
only from a real dependency boundary, not from this list.

## Invariants for all future work

- Keep core behavior expressible through the canonical options schema across
  Rust, Python, and Wasm.
- Preserve deterministic canonical output and structured, actionable errors.
- Treat tiled polygonization as experimental until its documented equivalence
  and error-reporting limits are closed.
- Do not claim robustness beyond the selected noding policy's checked
  postconditions.
- Add the smallest regression check that would have caught each bug.
