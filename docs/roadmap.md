# Engineering Roadmap

This document proposes ambitious, high-impact improvements to make `geo-polygonize` a best-in-class geospatial kernel across Rust, Python, and WebAssembly.

## 1) Performance: push throughput, reduce memory, and stabilize tail latency

### 1.1 Build a repeatable perf lab
- Add a `criterion` + dataset matrix that measures:
  - sparse random lines
  - high-density city parcel boundaries
  - near-degenerate/intersection-heavy inputs
- Add CI perf baselines with threshold alerts (e.g., fail on >5% regression on key benches).
- Check in a curated benchmark corpus (`fixtures/bench/`) with provenance and expected topology metrics.

### 1.2 Data-oriented core refactors
- Convert hot edge/node structures to compact SoA where beneficial (coordinates, adjacency, visitation flags).
- Replace hash-heavy graph traversals in hot loops with index-based arenas and contiguous vectors.
- Use small-vector stack allocations in tight paths to reduce allocator pressure.

### 1.3 SIMD and numerics strategy
- Introduce architecture-aware dispatch (`x86_64` AVX2/FMA, `aarch64` NEON) behind a stable abstraction.
- Benchmark SIMD kernels independently and gate merges on per-kernel regression checks.
- Define clear epsilon and snapping policies to avoid hidden precision drift under vectorization.

### 1.4 Parallel scaling
- Add adaptive work partitioning based on line count + graph complexity (not just static chunking).
- Use tile-aware scheduling with halo boundaries to reduce cross-thread merge overhead.
- Publish scaling curves in docs (1, 2, 4, 8, 16 cores) and large-input memory profiles.

## 2) Security and robustness: harden all boundaries

### 2.1 Threat model + security policy
- Add `SECURITY.md` with coordinated disclosure process and supported versions.
- Publish threat model for each surface:
  - Rust API untrusted geometry
  - Arrow/FFI buffers
  - Python bindings and capsule ownership
  - WASM inputs in browser and server runtimes

### 2.2 Fuzzing and property testing
- Add continuous fuzzing for parser/FFI/wrapper entry points (`cargo-fuzz` + OSS-Fuzz-ready setup).
- Add metamorphic/property tests:
  - permutation invariance of input segment order
  - idempotence under repeated polygonization
  - stable behavior under tiny coordinate perturbations
- Add differential tests against GEOS/JTS for randomized corpora.

### 2.3 Memory safety and panic boundaries
- Audit and document all `unsafe` blocks with invariants and proof comments.
- Ensure all FFI boundaries are panic-safe and return structured errors, never unwind across language boundaries.
- Validate all offsets/lengths in zero-copy Arrow inputs before access; reject malformed buffers deterministically.

### 2.4 Supply-chain hygiene
- Enforce dependency policy:
  - `cargo audit`
  - `cargo deny`
  - lockfile freshness checks
- Enable provenance/SBOM generation for release artifacts.
- Pin critical CI actions and add periodic update automation.

## 3) API consistency and usability

### 3.1 One option model across Rust/Python/WASM
- Define a canonical `PolygonizerOptions` schema and keep field names/semantics aligned across all bindings.
- Standardize defaults (`node_input`, `snap_grid_size`, output options) and document rationale.
- Add versioned compatibility guarantees for serialized option payloads.

### 3.2 Error taxonomy and diagnostics
- Introduce stable, typed error codes that map across Rust, Python exceptions, and WASM error objects.
- Add optional diagnostics mode returning counters (dangles removed, cut edges, invalid rings detected).
- Provide machine-readable warnings alongside output for data quality pipelines.

### 3.3 Output contract and determinism
- Guarantee deterministic ordering of polygons/rings (or explicitly expose a canonical sort option).
- Document winding/orientation behavior and ensure consistency across backends.
- Add golden tests that verify byte-stable outputs for fixed inputs.

## 4) Code organization and maintainability

### 4.1 Clear architecture boundaries
- Split crate modules by pipeline stage:
  - ingest/normalize
  - noding
  - graph build
  - polygon extraction
  - post-processing/validation
- Define narrow interfaces between stages to simplify profiling and future algorithm swaps.

### 4.2 Internal design docs
- Add ADRs (Architecture Decision Records) for:
  - snap-rounding strategy
  - graph representation choices
  - SIMD abstraction decisions
  - cross-language API stability policy

### 4.3 Developer experience
- Provide `just`/`make` targets for common workflows: lint, test, fuzz-smoke, bench-smoke, release-check.
- Add a contributor guide with “first good issue” pathways and local perf-testing instructions.
- Standardize formatting/linting across Rust, Python, and JS with one documented command entrypoint.

## 5) Product-quality operations

### 5.1 Release engineering
- Ship signed artifacts and reproducible builds for core release targets.
- Define semantic versioning policy per surface (core crate vs Python wheel vs npm package).
- Automate changelog sections by scope (core/python/wasm).

### 5.2 Observability for integrators
- Add optional tracing spans around major stages with timing + counts.
- Expose lightweight telemetry hooks so embedding systems can record quality/perf metrics.

## 6) 90-day execution plan (suggested)

### Phase 1 (Weeks 1-3): Baseline and safety
- Perf corpus + CI thresholding.
- `SECURITY.md` + dependency policy checks.
- FFI panic-boundary audit.

### Phase 2 (Weeks 4-7): Core speed + API normalization
- Hot-path data-layout improvements.
- Unified options schema across bindings.
- Deterministic output and golden tests.

### Phase 3 (Weeks 8-12): Hardening + scale
- Fuzzing rollout and differential tests.
- Parallel scheduler improvements.
- Release provenance and SBOM pipeline.

## Success criteria

- **Performance:** measurable p50/p95 improvements on benchmark corpus and no CI perf regressions.
- **Security:** continuous fuzzing coverage + documented disclosure process.
- **API quality:** aligned options/errors across Rust/Python/WASM with stable compatibility tests.
- **Maintainability:** clearer module boundaries and ADR-backed engineering decisions.
