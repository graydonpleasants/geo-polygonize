# Engineering Roadmap

This document outlines the strategic improvements planned for `geo-polygonize` to make it a best-in-class geospatial kernel across Rust, Python, and WebAssembly.

The roadmap is divided into three sequential phases. Each phase contains specific modules, or “agent tracks,” to allow for parallel development and minimize conflicts. The main architectural direction is to turn today’s implicit behavior into explicit, testable policy: target-aware backends, deterministic output, precision policy, Z policy, touch policy, tile ownership policy, provenance, and phase-level diagnostics.

This roadmap is intentionally implementation-oriented. Each track includes concrete deliverables, dependencies, acceptance criteria, likely code touch points, and notes for safe migration.

---

## North Star

By the end of this roadmap, the library should have:

- deterministic, reproducible polygonization across Rust, Python, and Wasm
- a canonical `PolygonizerOptions` schema shared across bindings
- a stable `polygonize_with_options(options)` API across Rust, Python, and Wasm
- explicit 2D/3D and precision semantics
- pluggable spatial-index and noding backends selected by `TargetProfile`
- a containment forest abstraction that centralizes shell/hole logic
- deterministic tiled polygonization with robust ownership and dedup
- provenance-aware output, including optional per-polygon boundary line attribution
- report/debug mode that can explain output differences by policy profile and by source boundary lines
- a documented `geos_compat` snap strategy for users targeting GEOS/Shapely parity
- typed, actionable binding errors instead of opaque JS/Python type failures
- phase-level diagnostics, memory profiling, and CI-grade performance baselines
- fuzzing, metamorphic tests, and differential tests against GEOS/JTS semantics where appropriate

---

## Guiding Principles

### 1. Make policies explicit
Anything that changes output or robustness must become a named option, not an implementation accident.

This includes:
- precision / snap mode
- snap strategy
- noding backend
- index backend
- Z policy
- touch policy
- tile ownership policy
- determinism / canonical sort policy
- provenance / report mode behavior

### 2. Separate native and Wasm defaults
Native and Wasm should share the same semantics where possible, but not the same default internals.

- Native should optimize for throughput and packed static structures.
- Wasm should optimize for code size, alignment safety, simpler backends, and browser/runtime constraints.

### 3. Prefer deterministic and benchmarkable over clever
A slower change with stable ordering, explicit diagnostics, and golden coverage is better than a faster opaque change.

### 4. Add stage boundaries before major algorithm swaps
Before swapping in new indexes or noders, isolate the pipeline stages:

- ingest / normalize
- noding
- graph build
- ring extraction
- containment
- tiled ownership / dedup
- output flatten / serialization

That makes profiling, replacement, and rollback much safer.

### 5. Optimize for library consumers, not just internal kernels
The library must expose stable, inspectable behavior across Rust, Python, and WebAssembly.

This means:
- a shared options-object API across bindings
- typed, actionable errors at all binding boundaries
- explicit compatibility modes where semantics differ
- provenance and diagnostics that help downstream systems explain results and mismatches

---

# Phase 1: Baseline & Safety

**Goal**: Establish deterministic output, regression harnesses, diagnostics, binding safety, and policy scaffolding before invasive algorithmic changes.

This phase is the prerequisite for every later speedup.

## 1. Testing & Validation (Agent Track A)

**Goal**: Build a strong correctness baseline so aggressive changes like integer snapping, new spatial indexes, provenance tracking, or containment refactors can be validated cheaply and repeatedly.

### Deliverables
- deterministic output contract
- checked-in golden corpus
- metamorphic/property tests
- fuzzing entrypoints
- stronger differential testing against GEOS/Shapely

### 1.1 Deterministic Output and Canonical Ordering
**Action**
- Guarantee deterministic ordering of polygons, outer rings, holes, dangles, invalid rings, and provenance line ID arrays in canonical mode.
- Add a `DeterminismOptions` block with `canonical_sort`, `canonical_ring_rotation`, and `stable_tie_breaks`.

**Acceptance criteria**
- [ ] Same input produces byte-identical serialized output across repeated runs.
- [ ] Same input with segment order permuted produces identical canonical output.
- [ ] Same input on native parallel and native non-parallel builds produces equivalent canonical output.

### 1.2 Golden Fixture Corpus
**Action**
Create:
- `fixtures/basic/`
- `fixtures/topology/`
- `fixtures/dirty/`
- `fixtures/tiling/`
- `fixtures/z/`
- `fixtures/provenance/`
- `fixtures/compat/`
- `fixtures/bench/`

**Acceptance criteria**
- [ ] Each fixture has explicit expected topology metrics.
- [ ] Canonical-mode tests run on all fixtures.
- [ ] Fixture corpus is reused in benches and differential tests.

### 1.3 Metamorphic and Property Tests
**Action**
Add property tests for permutation invariance, idempotence, stability under below-grid perturbations, ring-start rotation invariance, tiled vs non-tiled equivalence where expected, and provenance stability in canonical mode.

### 1.4 Fuzzing Rollout
**Action**
Add `cargo-fuzz` harnesses for the core pipeline, Wasm typed-buffer ingestion, Arrow/FFI ingestion, tile ownership + dedup, and provenance-enabled report mode.

### 1.5 Differential Tests Update
**Action**
Extend the Shapely differential harness with adversarial random corpora and explicit buckets for expected parity, expected divergence, and invalid / ambiguous inputs.

## 2. Security & Boundaries (Agent Track B)

**Goal**: Make all language and memory boundaries explicit, panic-safe, and documented.

### Deliverables
- `SECURITY.md`
- threat model
- panic-safe FFI boundaries
- documented `unsafe`
- input validation hardening

### 2.1 Security Policy and Threat Model
- [ ] Add `SECURITY.md` and document the threat model.

### 2.2 Panic Safety
- [ ] Wrap FFI/Wasm/Python boundary entrypoints with panic-catching and structured error reporting.

### 2.3 Unsafe Audit
- [ ] Document every `unsafe` block with invariants and rationale.

### 2.4 Input Validation
- [ ] Validate Arrow offsets and lengths, Wasm typed-array buffer sizes, stride mismatches, and NaN/Inf coordinates.

## 3. Allocation & Diagnostics (Agent Track C)

**Goal**: Create instrumentation that tells us where time and memory go before doing the big refactors.

### Deliverables
- `PolygonizerDiagnostics`
- phase timers
- allocation profiling
- instruction-count benchmarks
- perf CI strategy

### 3.1 Diagnostics Object
```rust
pub struct PolygonizerDiagnostics {
	pub input_segment_count: usize,
	pub noded_segment_count: usize,
	pub dangle_count: usize,
	pub cut_edge_count: usize,
	pub ring_count: usize,
	pub shell_count: usize,
	pub hole_count: usize,
	pub invalid_ring_count: usize,
	pub flat_line_count: usize,
	pub phase_times: PolygonizerPhaseTimes,
	pub noding_iterations: Vec<NodingIterationStats>,
	pub snap_stats: SnapStats,
	pub intersection_stats: IntersectionStats,
}
```

### 3.2 Allocation Checks
- [ ] Use `dhat-rs` for allocation regression checks.
- [ ] Use `iai-callgrind` for instruction counts / cache behavior.

### 3.3 Perf Workflows
- [ ] Add a dedicated `perf` workflow with stable microbench baselines.

## 4. Binding Contracts & Report Mode (Agent Track D)

**Goal**: Ensure the library is debuggable and ergonomically safe from the perspective of external consumers before expanding algorithmic complexity.

### Deliverables
- typed errors in Wasm/Python
- report/debug mode scaffolding
- fixture-level provenance acceptance tests
- boundary-family mismatch explainability hooks

### 4.1 Typed Binding Errors
- [ ] Define structured error families and map them cleanly into Wasm and Python.

### 4.2 Report / Debug Mode Scaffold
- [ ] Add a `report_mode` flag that returns structured execution metadata without changing semantics.

### 4.3 Provenance Acceptance Fixtures
- [ ] Add fixtures validating mixed-boundary attribution and profile-tag passthrough.

---

# Phase 2: Core Speed & API Normalization

**Goal**: Normalize public configuration, formalize precision and dimensionality semantics, and deliver the highest-value internal improvements with low conceptual risk.

## 1. API & Configuration (Agent Track A)

**Goal**: Create a single, stable options-object API across Rust, Python, and Wasm, while preserving legacy positional APIs as wrappers.

### Deliverables
- canonical `PolygonizerOptions`
- stable `polygonize_with_options(options)` entrypoint in all bindings
- `TargetProfile`
- migration path from legacy positional and field-based APIs
- explicit policy enums
- diagnostics and provenance toggles
- compatibility-oriented snap strategies

### 1.1 Stable Options-Object API
- [ ] Introduce `polygonize_with_options(options)` in Rust, Python, and Wasm.
- [ ] Keep positional APIs as wrappers.

### 1.2 Canonical Options Schema
```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolygonizerOptions {
	pub target: TargetProfile,
	pub node_input: bool,
	pub snap_grid_size: f64,
	pub extract_only_polygonal: bool,
	pub snap_strategy: SnapStrategy,
	pub noding: NodingOptions,
	pub containment: ContainmentOptions,
	pub tiling: Option<TilingOptions>,
	pub z: ZOptions,
	pub determinism: DeterminismOptions,
	pub diagnostics: DiagnosticsOptions,
	pub provenance: ProvenanceOptions,
	pub input_profile_id: Option<String>,
}
```

### 1.3 Policy Enums
- `NodingBackend`
- `SnapMode`
- `SnapStrategy`
- `IndexBackend`
- `ZPolicy`
- `TouchPolicy`
- `TileOwnershipPolicy`
- `DedupPolicy`

### 1.4 Snap Strategy Compatibility Modes
```rust
pub enum SnapStrategy {
	Grid,
	GeosCompat,
}
```

### 1.5 Legacy Compatibility
- [ ] Keep existing fields and positional APIs as shorthands that map into `PolygonizerOptions`.

## 2. Provenance & Explainability (Agent Track B)

**Goal**: Make output polygons explainable to external consumers by exposing boundary lineage, caller profile tags, and structured diagnostics.

### Deliverables
- optional line ID ingestion
- per-polygon provenance payload
- caller-provided profile passthrough
- diagnostics/report payload
- mismatch explanation support

### 2.1 Optional Input Line IDs
- [ ] Accept optional `line_ids` alongside linework input in all bindings.

### 2.2 Per-Polygon Provenance
```rust
pub struct PolygonProvenance {
	pub boundary_line_ids: Vec<u64>,
	pub input_profile_id: Option<String>,
}
```

### 2.3 Diagnostics Payload
- [ ] Expand diagnostics/report payload to include polygon count, dangles, invalid rings, flat lines, snapped/intersection stats, and stage timings.

### 2.4 Report Mode for Hybrid Scoring / Debug
- [ ] Same fixture run with report mode can explain mismatches by profile and boundary lines.

## 3. Precision, Z Semantics, and Core Noding Cleanup (Agent Track C)

### 3.1 Snap Mode
- `FloatExact`
- `FloatEpsilonDedup`
- `IntegerGrid`

### 3.2 Integerized Snap-Grid
```rust
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct IPoint {
	pub x: i64,
	pub y: i64,
}
```

### 3.3 Parametric Split Accumulation
- [ ] Replace squared-distance split sorting with parametric `t` accumulation and sorting.

### 3.4 Z Policy
- `Ignore`
- `InterpolateAlongEdge`
- `PreferNearestEndpoint`
- `ErrorOnConflict { max_delta }`

### 3.5 Remove Avoidable Clones
- [ ] Eliminate `input_lines.clone()` when `node_input=false` and reuse buffers where safe.

## 4. Observability & SIMD / Runtime Dispatch (Agent Track D)

### 4.1 Kernel Benches
- [ ] Add Criterion benches for split finding, grid build, split apply, containment, hashing, and provenance-enabled report overhead.

### 4.2 Runtime SIMD Dispatch
- [ ] Add architecture-aware runtime dispatch for scalar, Wasm SIMD `v128`, x86_64 AVX2, and optionally AVX-512.

### 4.3 Wasm Docs
- [ ] Document high-level vs typed-buffer APIs, `wasm-opt`, and COOP/COEP requirements.

---

# Phase 3: Hardening & Scale

**Goal**: Introduce the heavier algorithmic and architectural improvements once correctness, observability, and policy scaffolding are in place.

## 1. Scaling & Concurrency (Agent Track A)
- [ ] Parallelize `UniformGrid::new`.
- [ ] Implement adaptive regrid.
- [ ] Replace centroid-only ownership with stronger tile ownership policies.
- [ ] Add cross-tile dedup via canonical ring or edge-set hashing.

## 2. Topology & Containment Forests (Agent Track B)
- [ ] Extract containment into a dedicated module.
- [ ] Build a containment forest once and reuse it for shell/hole classification.
- [ ] Implement named touch policies.

## 3. Spatial Index Backends & Advanced Noders (Agent Track C)
- [ ] Add `SpatialIndex2D` trait and wrap current `rstar` usage.
- [ ] Add native packed static index backend.
- [ ] Prototype optional advanced noder backend.

## 4. Supply Chain, Release, and OSS Quality (Agent Track D)
- [ ] Add SBOM generation, provenance/release automation, `cargo deny`, and `cargo audit`.

## 5. Compatibility Profiles & Differential Explainability (Agent Track E)
- [ ] Harden `snap_strategy=geos_compat`.
- [ ] Extend report mode so mismatches can be attributed to profile, snap strategy, touch policy, and provenance differences.
- [ ] Add a parity-focused compatibility corpus.

---

# Recommended Execution Order

## First wave
1. deterministic output + canonical sorting
2. golden corpus + adversarial fixtures
3. `PolygonizerDiagnostics`
4. typed binding errors
5. report mode scaffold
6. canonical `PolygonizerOptions`
7. remove obvious clone/allocation waste

## Second wave
8. stable `polygonize_with_options(options)` across bindings
9. optional `line_ids` + provenance payload
10. `ZPolicy`
11. `SnapMode` with integer-grid feature
12. `SnapStrategy` with `grid` and `geos_compat`
13. parametric split accumulation
14. tile ownership policies
15. parallel `UniformGrid::new`

## Third wave
16. `ContainmentForest`
17. `SpatialIndex2D` trait + `rstar` adapter
18. native packed index backend
19. adaptive regrid
20. optional advanced noder
21. hardened mismatch explainability by profile and provenance

---

# Suggested Agent Ownership

## Agent Track A
- deterministic ordering
- golden fixtures
- metamorphic tests
- canonical options schema
- docs + migration notes

## Agent Track B
- panic-safe boundaries
- Arrow/Wasm validation
- Z policy
- snap mode / integer grid
- clone reduction

## Agent Track C
- diagnostics
- Criterion / iai-callgrind / dhat integration
- SIMD dispatch
- perf workflows

## Agent Track D
- typed binding errors
- report mode
- provenance fixtures
- cross-binding API contract

## Agent Track E
- containment forest
- tile ownership + dedup
- index abstraction
- packed index experiments
- adaptive grid
- compatibility explainability

---

# Done Definition

The roadmap is complete when:

- every public binding can express the same core behavior through `PolygonizerOptions`
- every public binding exposes `polygonize_with_options(options)`
- legacy positional APIs are preserved as wrappers
- deterministic mode is stable and covered by goldens
- precision and Z semantics are explicit and documented
- tiled and non-tiled behavior is predictable under named policies
- provenance-aware output is available when requested
- report mode can explain mismatches by profile and by boundary lines
- `geos_compat` is documented with clear scale guidance and known semantic differences
- Wasm/Python return typed, actionable errors for invalid inputs
- containment logic is centralized
- native and Wasm have target-appropriate backends
- perf regressions are caught by diagnostics and CI-grade measurement
- fuzzing and differential testing run continuously

---

# Short-Term Milestone Plan

## Milestone M1: Deterministic Baseline + Binding Safety
- [ ] canonical sort mode
- [ ] checked-in fixtures
- [ ] golden tests
- [ ] first diagnostics object
- [ ] typed Wasm/Python errors
- [ ] initial report mode scaffold

## Milestone M2: Stable Options API + Provenance Surface
- [ ] canonical `PolygonizerOptions`
- [ ] `polygonize_with_options(options)` in Rust/Python/Wasm
- [ ] legacy API wrappers
- [ ] optional `line_ids`
- [ ] `input_profile_id` passthrough
- [ ] initial per-polygon provenance payload

## Milestone M3: Precision and Hot Path Cleanup
- [ ] `SnapStrategy` with `grid` and `geos_compat`
- [ ] parametric split accumulation
- [ ] clone reduction
- [ ] integer snap-grid prototype
- [ ] per-kernel benches

## Milestone M4: Tiling + Containment + Explainability
- [ ] tile ownership policies
- [ ] containment forest
- [ ] touch policies
- [ ] cross-tile dedup
- [ ] report mode explains mismatches by profile and provenance

## Milestone M5: Native Scale Features + Compatibility Hardening
- [ ] `SpatialIndex2D` abstraction
- [ ] packed native index
- [ ] adaptive regrid
- [ ] optional advanced noder prototype
- [ ] hardened `geos_compat` mode with scale guidance
