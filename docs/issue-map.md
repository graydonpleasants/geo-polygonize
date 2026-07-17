# Issue Map

This document turns the roadmap into implementation-sized issues with IDs, dependencies, suggested owners, and PR slicing guidance.

## Labels

Suggested labels:
- `phase:1`
- `phase:2`
- `phase:3`
- `area:api`
- `area:bindings`
- `area:diagnostics`
- `area:provenance`
- `area:noding`
- `area:containment`
- `area:tiling`
- `area:index`
- `area:perf`
- `area:testing`
- `area:security`
- `area:compat`

## Dependency key
- `blocks`: must land first
- `parallel_with`: safe to work on concurrently

---

## P1-001 Canonical output mode
- **Phase**: 1
- **Area**: testing, api
- **Summary**: Add deterministic canonical ordering for polygons, rings, holes, and provenance line IDs.
- **Blocks**: none
- **Parallel with**: P1-002, P1-003
- **Suggested owner**: core geometry / testing
- **Acceptance**:
  - repeated runs are byte-identical in canonical mode
  - permutation invariance test passes
- **Suggested PR slices**:
  1. canonical sort helpers
  2. ring rotation normalization
  3. tests

## P1-002 Fixture corpus
- **Phase**: 1
- **Area**: testing
- **Summary**: Add checked-in fixtures for topology, tiling, provenance, compatibility, and benchmarks.
- **Blocks**: none
- **Parallel with**: P1-001, P1-004
- **Suggested owner**: testing / product-minded consumer
- **Acceptance**:
  - fixture directories exist
  - provenance and compat fixtures included

## P1-003 Diagnostics scaffold
- **Phase**: 1
- **Area**: diagnostics, perf
- **Summary**: Add `PolygonizerDiagnostics` and phase timing buckets.
- **Blocks**: none
- **Parallel with**: P1-001, P1-004
- **Suggested owner**: perf / runtime
- **Acceptance**:
  - diagnostics available without changing semantics
  - phase timings visible on representative workloads

## P1-004 Typed binding errors
- **Phase**: 1
- **Area**: bindings, security
- **Summary**: Map core structured errors into Wasm and Python with actionable messages.
- **Blocks**: none
- **Parallel with**: P1-002, P1-003
- **Suggested owner**: bindings
- **Status**: Complete
- **Acceptance**:
  - no opaque JS type errors for expected misuse
  - Python exposes typed exception families

## P1-005 Report mode scaffold
- **Phase**: 1
- **Area**: diagnostics, provenance, bindings
- **Summary**: Add a report/debug mode that returns counters and timings without changing semantics.
- **Blocks**: P1-003
- **Parallel with**: P1-006
- **Suggested owner**: diagnostics / bindings
- **Acceptance**:
  - Rust, Python, and Wasm can enable report mode
  - report mode is fixture-testable

## P1-006 Panic-safe boundaries and validation
- **Phase**: 1
- **Area**: security, bindings
- **Summary**: Add panic-catching and validate Arrow/Wasm buffer shapes and numeric inputs.
- **Blocks**: none
- **Parallel with**: P1-005
- **Suggested owner**: security / FFI
- **Acceptance**:
  - malformed input returns structured error
  - no unwind across FFI

---

## P2-001 Canonical options schema
- **Phase**: 2
- **Area**: api
- **Summary**: Add `PolygonizerOptions` and sub-structures.
- **Blocks**: P1-001, P1-003
- **Parallel with**: P2-002
- **Suggested owner**: API design
- **Status**: Complete
- **Acceptance**:
  - schema exists and serializes cleanly
  - defaults reproduce current behavior

## P2-002 Cross-binding `polygonize_with_options`
- **Phase**: 2
- **Area**: api, bindings
- **Summary**: Expose stable options-object entrypoints in Rust, Python, and Wasm.
- **Blocks**: P2-001, P1-004
- **Parallel with**: P2-003
- **Suggested owner**: API + bindings
- **Status**: Complete
- **Acceptance**:
  - all bindings expose canonical options path
  - legacy wrappers still work

## P2-003 Legacy wrapper parity
- **Phase**: 2
- **Area**: api, testing
- **Summary**: Route legacy APIs through `PolygonizerOptions` and prove parity via tests.
- **Blocks**: P2-001
- **Parallel with**: P2-002
- **Suggested owner**: API / testing
- **Status**: Complete
- **Acceptance**:
  - wrapper APIs produce same result as canonical options

## P2-004 Optional `line_ids`
- **Phase**: 2
- **Area**: provenance, bindings
- **Summary**: Accept optional input `line_ids` in all bindings.
- **Blocks**: P2-002
- **Parallel with**: P2-005
- **Suggested owner**: provenance / bindings
- **Status**: Complete
- **Acceptance**:
  - length validation works
  - missing `line_ids` remains valid

## P2-005 Per-polygon provenance
- **Phase**: 2
- **Area**: provenance
- **Summary**: Add `PolygonProvenance` with `boundary_line_ids` and `input_profile_id`.
- **Blocks**: P2-004, P1-005
- **Parallel with**: P2-006
- **Suggested owner**: provenance / core geometry
- **Status**: Complete
- **Acceptance**:
  - provenance optionality works
  - mixed-boundary fixture yields multi-family provenance

## P2-006 Profile passthrough + richer report payload
- **Phase**: 2
- **Area**: diagnostics, provenance
- **Summary**: Pass through `input_profile_id` and enrich report payload with counts and timing.
- **Blocks**: P1-005, P2-001
- **Parallel with**: P2-005
- **Suggested owner**: diagnostics
- **Status**: Complete
- **Acceptance**:
  - report explains mismatches by profile

## P2-007 Snap strategy enum
- **Phase**: 2
- **Area**: compat, api
- **Summary**: Add `SnapStrategy::{Grid, GeosCompat}` and documentation hooks.
- **Blocks**: P2-001
- **Suggested owner**: API / compat
- **Acceptance**:
  - strategy visible in all bindings
  - fixtures can record active strategy

## P2-010 Parametric split accumulation
- **Phase**: 2
- **Area**: noding, perf
- **Summary**: Replace squared-distance split ordering with parametric `t` accumulation.
- **Blocks**: P1-003
- **Suggested owner**: noding
- **Acceptance**:
  - no correctness regressions
  - measurable hot-path win

## P2-011 Clone reduction + buffer reuse
- **Phase**: 2
- **Area**: perf
- **Summary**: Remove avoidable clones and reuse buffers where safe.
- **Blocks**: P1-003
- **Parallel with**: P2-010
- **Suggested owner**: perf / memory
- **Acceptance**:
  - peak allocations reduced in baseline cases

## P2-012 Kernel benches + SIMD dispatch
- **Phase**: 2
- **Area**: perf
- **Summary**: Add kernel-level Criterion benches and explicit runtime SIMD dispatch.
- **Blocks**: P1-003
- **Parallel with**: P2-011
- **Suggested owner**: perf / low-level runtime
- **Acceptance**:
  - per-kernel benches checked in
  - scalar vs SIMD comparison possible

---

## P3-001 Parallel `UniformGrid::new`
- **Phase**: 3
- **Area**: noding, perf
- **Summary**: Parallelize grid construction hot path.
- **Blocks**: P2-012
- **Parallel with**: P3-002
- **Suggested owner**: perf / concurrency
- **Acceptance**:
  - deterministic speedup on dense workloads

## P3-002 Adaptive regrid
- **Phase**: 3
- **Area**: noding
- **Summary**: Add bounded adaptive grid tuning based on occupancy distribution.
- **Blocks**: P1-003
- **Parallel with**: P3-001
- **Suggested owner**: noding
- **Acceptance**:
  - candidate explosions reduced on skewed data

## P3-003 Tile ownership policies
- **Phase**: 3
- **Area**: tiling
- **Summary**: Replace centroid-only ownership with named policies.
- **Parallel with**: P3-004
- **Suggested owner**: tiling / topology
- **Acceptance**:
  - concave tile-spanning fixtures behave predictably

## P3-004 Cross-tile dedup
- **Phase**: 3
- **Area**: tiling, provenance
- **Summary**: Add canonical ring or edge-set dedup for cross-tile outputs.
- **Blocks**: P3-003
- **Parallel with**: P3-005
- **Suggested owner**: tiling / hashing
- **Acceptance**:
  - duplicate emissions removed in adversarial fixtures

## P3-005 Containment forest
- **Phase**: 3
- **Area**: containment
- **Summary**: Extract containment into a reusable forest abstraction.
- **Blocks**: P1-001, P1-003
- **Parallel with**: P3-004, P3-006
- **Suggested owner**: topology
- **Acceptance**:
  - shell/hole classification reproduced under default policy

## P3-006 Touch policies
- **Phase**: 3
- **Area**: containment, compat
- **Summary**: Add explicit touch-policy enums and fixture coverage.
- **Blocks**: P3-005, P2-001
- **Suggested owner**: topology / compat
- **Acceptance**:
  - touching-point and touching-edge semantics are fixture-covered

## P3-009 Optional advanced noder backend
- **Phase**: 3
- **Area**: noding
- **Summary**: Retired prototype; the compatibility option now uses exact `SnapNoder` semantics.
- **Blocks**: P3-007, P1-002
- **Parallel with**: P3-008
- **Suggested owner**: computational geometry
- **Status**: Retired after correctness audit
- **Acceptance**:
  - compatibility option matches exact `SnapNoder` on the golden and adversarial corpus

## P3-010 Harden `geos_compat`
- **Phase**: 3
- **Area**: compat
- **Summary**: Refine compatibility-oriented snapping and document scale guidance.
- **Blocks**: P2-007, P2-002
- **Parallel with**: P3-011
- **Suggested owner**: compat / testing
- **Acceptance**:
  - docs explain expected parity and expected divergence

## P3-011 Differential explainability
- **Phase**: 3
- **Area**: compat, diagnostics, provenance
- **Summary**: Extend report mode so mismatches are attributable to profile, strategy, touch policy, and provenance.
- **Blocks**: P2-005, P2-006, P3-010
- **Parallel with**: P3-012
- **Suggested owner**: diagnostics / compat
- **Acceptance**:
  - same fixture can explain mismatches by profile and boundary lines

## P3-012 Supply chain and release hardening
- **Phase**: 3
- **Area**: security, release
- **Summary**: Add SBOM generation, provenance/release automation, `cargo deny`, and `cargo audit`.
- **Blocks**: none
- **Parallel with**: P3-011
- **Suggested owner**: release engineering
- **Acceptance**:
  - release workflow emits provenance artifacts

---

## Suggested first epic cuts

### Epic A: Consumer-facing contract
- P1-004
- P1-005
- P2-001
- P2-002
- P2-003
- P2-004
- P2-005
- P2-006

### Epic B: Determinism and correctness
- P1-001
- P1-002
- P1-003
- P1-006
- P2-010

### Epic C: Scale and hardening
- P2-012
- P3-001
- P3-002
- P3-003
- P3-004
- P3-005
- P3-009

### Epic D: Compatibility and explainability
- P2-007
- P3-006
- P3-010
- P3-011

---

## Suggested issue template fields

For each GitHub issue, use:
- **Summary**
- **Why**
- **Scope**
- **Out of scope**
- **Acceptance criteria**
- **Dependencies**
- **Suggested PR slices**
- **Bench / fixture impact**
- **Binding impact**
