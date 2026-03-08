# Engineering Roadmap

This document outlines the strategic improvements planned for `geo-polygonize` to make it a best-in-class geospatial kernel across Rust, Python, and WebAssembly.

The roadmap is divided into three sequential phases. Each phase contains specific modules (or "agent-tracks") to allow for parallel development and minimize conflicts.

---

## Phase 1: Baseline & Safety

**Goal**: Establish strong testing baselines, deterministic output, and core safety.

### 1. Testing & Validation (Agent Track A)
**Context:** We need a solid regression harness to ensure that aggressive algorithmic replacements (like integer snapping or spatial index swaps) do not break polygonization validity. JTS and GEOS provide robust reference implementations.

- **Deterministic Output & Golden Tests**
  - **Action**: Guarantee deterministic polygon/ring ordering or expose a canonical sort option. Ensure noding and containment stages have stable tie-breaks.
  - **Action**: Check in a curated benchmark corpus (`fixtures/bench/`) with provenance (small hand-built planar graphs, "massive touching rings", etc.) and expected topology metrics.
  - **Migration**: Keep existing public fields stable; introduce sorting as an additive `DeterminismOptions` block.
- **Metamorphic & Property Tests**
  - **Action**: Add property tests for permutation invariance (input segment order) and idempotence (repeated polygonization).
  - **Action**: Test stable behavior under tiny coordinate perturbations.
- **Fuzzing Rollout**
  - **Action**: Add continuous fuzzing using `cargo-fuzz` (libFuzzer) targeting the core polygonize pipeline and WASM buffer ingestion. Add "fuzz-smoke" (e.g., 30–60s) to CI behind a nightly toolchain.
- **Differential Tests Update**
  - **Action**: Extend the existing Shapely differential harness (`python/test_wrapper.py`) with adversarial random corpora to compare against GEOS behaviors.

### 2. Security & Boundaries (Agent Track B)
- **Security Policy**
  - **Action**: Add `SECURITY.md` with a coordinated disclosure process.
  - **Action**: Define and publish a threat model for Rust API untrusted geometry, Arrow/FFI buffers, Python bindings, and WASM inputs in browser and server runtimes.
- **Safety Audits**
  - **Action**: Audit and document all `unsafe` blocks with invariants and proof comments.
  - **Action**: Ensure all FFI boundaries are panic-safe (using `catch_unwind`), returning structured typed error codes instead of unwinding across language boundaries.
  - **Action**: Validate all offsets/lengths in zero-copy Arrow inputs before access.

### 3. Allocation & Diagnostics (Agent Track C)
**Context:** Benchmarking currently captures end-to-end time, but we need phase breakdown and memory profiling to guide optimizations (e.g. WASM deserialization vs polygonization).

- **Phase Breakdown & Diagnostics**
  - **Action**: Add a `PolygonizerDiagnostics` structure returning counts (dangles removed, cut edges) and per-phase durations (ingest, noding, graph build, containment, output) behind an option.
  - **CI Integration**: Keep out of default PR CI if flaky. Run on a schedule or dedicated "perf" workflow.
- **Allocation Checks**
  - **Action**: Add allocation sanity tests using `dhat-rs` to assert "exactly N allocations" in hot paths.
  - **Action**: Add `iai-callgrind` benches for stable CI-grade measurements of instruction counts and cache behavior.

---

## Phase 2: Core Speed & API Normalization

**Goal**: Refactor internal structures for speed, introduce strong precision policies, and establish a unified configuration schema.

### 1. API & Configuration (Agent Track A)
**Context:** The project needs a unified options model across Rust, Python, and WASM to safely expose new policies without API fragmentation.

- **Unified `PolygonizerOptions` Schema**
  - **Rust Sketch:**
    ```rust
    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    pub struct PolygonizerOptions {
        pub target: TargetProfile,
        pub noding: NodingOptions,
        pub containment: ContainmentOptions,
        pub tiling: Option<TilingOptions>,
        pub z: ZOptions,
        pub determinism: DeterminismOptions,
    }
    pub enum TargetProfile { Native, WasmSingleThread, WasmThreads }
    ```
  - **Action**: Define this canonical `serde`-serializable schema. Introduce `TargetProfile` which influences defaults (e.g. indexing, WASM allocators). Add `Polygonizer::with_options()`.
  - **Migration**: Treat existing public fields (`node_input`, `snap_grid_size`) as legacy shorthand that maps into this options model.
- **Explicit Z Policies**
  - **Context**: 3D data interpolation is currently implicit. We need explicit 2D topology first.
  - **Action**: Add a `ZPolicy`. Implement `Ignore` (matches JTS/GEOS "2D engine" mental model) and `InterpolateAlongEdge` (current behavior).
  - **Migration**: Make `Ignore` recommended for typical use, document current behavior clearly.

### 2. Core Noding & Dedup (Agent Track B)
**Context:** Robustness requires addressing epsilon thrash in float-based snap rounding.

- **Integerized Snap-Grid**
  - **Action**: Introduce integer snap-grid coordinate conversion behind a feature flag (`snap-int`).
  - **Implementation**:
    ```rust
    #[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
    pub struct IPoint { pub x: i64, pub y: i64 }
    ```
    Use `i64` for internal coordinates, use integer equality for event dedup, split sorting keys, and canonicalization. Convert back to `f64` at API boundaries.
  - **Tradeoffs**: Integerization adds upfront conversion cost but typically pays for itself by eliminating epsilon thrash and hash instability in hot loops.
- **Parametric Split Accumulation**
  - **Action**: Replace per-line squared-distance split sorting with parametric `t` sorting to optimize the hot path.
- **Spatial Index Abstraction**
  - **Action**: Introduce a `SpatialIndex2D` trait (e.g. `build`, `query_aabb`).
  - **Action**: Wrap the current `rstar` backend inside this trait. This prepares for the native packed index in Phase 3.

### 3. Observability & SIMD (Agent Track C)
- **Microbenchmarks & CI Gates**
  - **Action**: Build per-kernel Criterion benchmarks (e.g., split scan, hole containment checks).
  - **Action**: Introduce architecture-aware runtime dispatch for SIMD operations. Explicitly support WASM `v128` and `x86_64` AVX2/AVX-512.
- **Preallocation**
  - **Action**: Remove the `input_lines.clone()` in `build_graph` when `node_input=false` by bulk-loading directly.

---

## Phase 3: Hardening & Scale

**Goal**: Tackle complex scaling logic, advanced algorithm swaps, and release operations.

### 1. Scaling & Concurrency (Agent Track A)
**Context:** Tiling and grid algorithms currently have single-threaded chokepoints.

- **Parallel Uniform Grid Construction**
  - **Action**: Parallelize `UniformGrid::new` counts and population passes.
- **Adaptive Grid Tuning**
  - **Action**: Implement "adaptive regrid". Measure occupancy distribution (e.g., p95 cell load). If load is too high, shrink cell size automatically.
- **Tile Ownership Policies**
  - **Action**: Introduce `TileOwnershipPolicy`. Centroid ownership is brittle for concave shapes.
  - **Action**: Implement `RepresentativePointInsidePolygon` (guaranteed interior point, reusing hole assignment logic) as the recommended default.
  - **Action**: Add `EdgeSetHashDedup` canonicalization/hashing for robust cross-tile dedup.

### 2. Topology & Containment Forests (Agent Track B)
**Context:** Hole assignment and filtering (`extract_only_polygonal`) currently do overlapping containment/point-in-ring work.

- **Containment Forest Abstraction**
  - **Action**: Extract "topology establishment" into its own module.
  - **Action**: Compute per-ring bbox, representative interior probe point, and query the index to build an explicit `ContainmentForest` (a DAG of immediate-parent edges). Convert forest by parity depth to final polygons.
  - **Action**: Implement explicit touch policies (e.g. `AllowPointTouchDisallowEdgeShare` vs `AllowEdgeShare`).
  - **Tradeoffs**: Building the forest adds a structured data object but heavily reduces repeated R-tree queries and repeated point-in-ring tests across stages.

### 3. Spatial Indexes & OSS Integration (Agent Track C)
- **Packed Index Backends (Native Only)**
  - **Action**: Implement or integrate a static packed index backend (e.g. FlatGeobuf's packed Hilbert R-tree or an STRtree equivalent) for the `SpatialIndex2D` trait.
  - **Action**: Make packed indices the default for the Native profile for static sets (like shell bboxes for hole assignment). Leave `rstar` as default for WASM.
- **Supply Chain Automation**
  - **Action**: Automate release provenance, SBOM generation, and enforce `cargo deny` / `cargo audit` in CI.

---

## OSS Techniques Comparison Matrix

| Project | Noding approach | Spatial index | Tiling | Z semantics | Relevance to geo-polygonize Phase Adoption |
|---|---|---|---|---|---|
| **geo-polygonize** | ISR with SIMD vs uniform grid | `rstar` | buffered, centroid ownership | partial (interpolated) | Current Baseline |
| **JTS** | Index-based (monotone chains) | STRtree, HPRtree | N/A | Ignored in comparison | Reference for packed indexes & `Ignore` Z policy (Phases 2-3) |
| **GEOS** | Precision-model driven / OverlayNG | STRtree | N/A | Z often NaN for new vertices | Robustness ladder model for noding tolerance (Phase 3+) |
| **S2 Geometry** | snap-rounding framework | S2ShapeIndex | S2 cells | model-specific | Inspiration for robust snap framework & grid size (Phase 3+) |
| **FlatGeobuf** | N/A | Packed Hilbert R-tree | N/A | N/A | Precedent for Native-only packed index backend (Phase 3) |
| **Mapbox** | N/A | N/A | buffered geometry extending tile | N/A | Reference for buffered tiling contracts and dedup (Phase 3) |
| **Clipper2** | integer-core intersection | internal | N/A | integer core | Precedent for integerized snapping robustness (Phase 2-3) |

## Key References & Precedent
- **Standards**: OGC Simple Feature Access model (Z/M values ignored in topology); JTS valid polygon constraints (holes touch shell/hole at most one point).
- **Techniques**: Stable snap rounding for idempotence; JTS `MCIndexNoder` using monotone chains; GEOS OverlayNG robustness strategies.
- **WASM Constraints**: `wasm-bindgen-rayon` requires cross-origin isolation (COOP/COEP); WebAssembly SIMD proposal (`v128`).
- **Tooling**: `criterion` (microbenchmarks), `cargo-fuzz` (libFuzzer), `iai-callgrind` (stable CI instruction counting), `dhat-rs` (allocation profiling).