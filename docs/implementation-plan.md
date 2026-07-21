# Implementation Plan

This document translates the roadmap into concrete API, data model, migration, and testing tasks for coding agents.

It is intentionally more specific than `ROADMAP.md`. The roadmap explains what we want and why. This document explains how to land it in code without unnecessary churn.

## 1. Scope

This implementation plan focuses on six concrete outcomes:

1. a stable cross-binding options-object API
2. provenance-aware polygon output
3. report/debug mode for explainability
4. typed, actionable binding errors
5. explicit snap strategy compatibility modes
6. safe migration from legacy APIs to canonical options

## 2. Required Cross-Binding API Contract

### 2.1 Canonical entrypoints
All bindings must expose an options-object entrypoint.

**Rust**
```rust
pub fn polygonize(
	lines: impl IntoIterator<Item = Line3D>,
	options: &PolygonizerOptions,
) -> Result<PolygonizeResult, PolygonizeError>
```

**Python**
```python
polygonize_with_options(lines, options=None, line_ids=None)
```

**Wasm**
```ts
polygonizeWithOptions(lines, options)
polygonizeWithOptionsBuffer(buffer, options)
```

### 2.2 Legacy compatibility
Existing positional APIs remain as wrappers.

Requirements:
- wrappers must map into `PolygonizerOptions`
- wrappers must not drift semantically from the canonical path
- wrappers may be deprecated later, but not removed during this migration phase

### 2.3 Cross-binding parity
The same conceptual options must be available across Rust core, Python bindings, Wasm object API, Wasm buffer API, and Arrow / FFI pathways where applicable.

## 3. Canonical Options Schema

### 3.1 Required top-level shape
```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolygonizerOptions {
	pub node_input: bool,
	pub snap_grid_size: f64,
	pub extract_only_polygonal: bool,
	pub snap_strategy: SnapStrategy,
	pub noding: NodingOptions,
	pub containment: ContainmentOptions,
	pub determinism: DeterminismOptions,
	pub diagnostics: DiagnosticsOptions,
	pub provenance: ProvenanceOptions,
	pub input_profile_id: Option<String>,
}
```

### 3.2 Required enums
```rust
pub enum SnapStrategy {
	Grid,
	GeosCompat,
}

pub enum TouchPolicy {
	AllowPointTouchDisallowEdgeShare,
	TreatAnyTouchAsDisjoint,
	AllowEdgeShare,
}

pub enum TileOwnershipPolicy {
	Centroid,
	RepresentativePointInsidePolygon,
	LexicographicMinVertex,
	CanonicalBoundaryHash,
}
```

### 3.3 Option sub-structures
```rust
pub struct NodingOptions {
	pub backend: NodingBackend,
}

pub struct ContainmentOptions {
	pub touch_policy: TouchPolicy,
}

pub struct DeterminismOptions {
	pub canonical_sort: bool,
	pub canonical_ring_rotation: bool,
	pub stable_tie_breaks: bool,
}

pub struct DiagnosticsOptions {
	pub enabled: bool,
	pub report_mode: bool,
}

pub struct ProvenanceOptions {
	pub enabled: bool,
	pub include_boundary_line_ids: bool,
}
```

## 4. Provenance Contract

### 4.1 Input requirements
All bindings should accept optional `line_ids`.

Requirements:
- `line_ids` length must match line input length when provided
- missing `line_ids` is valid
- duplicate `line_ids` is valid unless explicitly documented otherwise
- `line_ids` are caller-controlled identifiers, not internal row numbers

### 4.2 Output requirements
Per-polygon provenance must be available when requested.

```rust
pub struct PolygonProvenance {
	pub boundary_line_ids: Vec<u64>,
	pub input_profile_id: Option<String>,
}
```

### 4.3 Provenance semantics
`boundary_line_ids` should mean:
- IDs of caller-provided source lines that contributed to the polygon boundary
- all IDs on coincident or partially overlapping boundary segments; geometric dissolve must not discard source multiplicity
- deduplicated
- deterministically ordered in canonical mode

`input_profile_id` should mean:
- the caller-provided profile tag
- passed through unchanged for downstream comparison/debug use

### 4.4 Provenance overhead policy
Provenance must be optional.

### 4.5 Mixed-boundary support
Acceptance requirement:
- at least one mixed-boundary fixture should yield a polygon whose provenance references multiple source boundary families

## 5. Diagnostics and Report Contract

### 5.1 Diagnostics object
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

### 5.2 Report object
```rust
pub struct PolygonizerReport {
	pub polygon_count: usize,
	pub dangles: usize,
	pub invalid_rings: usize,
	pub flat_lines: usize,
	pub snapped_stats: SnapStats,
	pub intersection_stats: IntersectionStats,
	pub stage_timings: StageTimings,
	pub snap_strategy: SnapStrategy,
	pub input_profile_id: Option<String>,
}
```

### 5.3 Report mode semantics
`report_mode` is intended for:
- explaining mismatches by policy profile
- explaining mismatches by provenance / boundary lines
- differential testing
- external debugging and hybrid scoring systems

## 6. Error Taxonomy

### 6.1 Core error families
```rust
pub enum PolygonizeError {
	InvalidArgumentType {
		field: String,
		expected: String,
		actual: String,
	},
	InvalidGeometry {
		reason: String,
	},
	InvalidBufferShape {
		reason: String,
	},
	UnsupportedOptionCombination {
		reason: String,
	},
	TopologyFailure {
		reason: String,
	},
	InternalInvariantViolation {
		reason: String,
	},
}
```

### 6.2 Wasm requirements
Wasm must not surface opaque JS type errors for expected misuse.

### 6.3 Python requirements
Suggested families:
- `PolygonizeTypeError`
- `PolygonizeGeometryError`
- `PolygonizeOptionsError`
- `PolygonizeTopologyError`

### 6.4 Error message quality
Bad:
```text
TypeError: invalid input
```

Good:
```text
InvalidBufferShape: line_ids length 18 does not match line count 19
```

## 7. Snap Strategy Compatibility Contract

### 7.1 Strategies
```rust
pub enum SnapStrategy {
	Grid,
	GeosCompat,
}
```

### 7.2 Intended meaning
`Grid`
- uses the precision grid for topology and output coordinates
- provides stable, explicit native snap-rounding semantics

`GeosCompat`
- uses the grid for topology, then restores one deterministic nearest source coordinate per node
- targets Shapely `snap` followed by full-precision noding and polygonization, not `set_precision`
- preserves source-coordinate fidelity where many-to-one snaps do not make that ambiguous

### 7.3 Documentation requirements
The docs must clearly explain:
- where `grid` and `geos_compat` are expected to behave similarly
- where they are expected to diverge
- scale guidance and tolerance sensitivity
- that exact parity is not guaranteed in all degenerate cases

## 8. Migration Plan

### 8.1 Stage 1: Introduce options without removal
- add `PolygonizerOptions`
- add `polygonize_with_options`
- keep wrappers for all current APIs
- add tests proving wrapper parity

### 8.2 Stage 2: Route all bindings through canonical path
- make Python route through canonical options resolution
- make Wasm route through canonical options resolution
- make Arrow/FFI resolve into canonical options

### 8.3 Stage 3: Add provenance and report mode
- add optional `line_ids`
- add `PolygonProvenance`
- add report payloads
- add fixtures and acceptance tests

### 8.4 Stage 4: Add `geos_compat`
- implement compatibility strategy
- add docs and fixture corpus
- ensure mismatch reports expose active strategy

### 8.5 Stage 5: Deprecation planning
- mark legacy APIs as soft-deprecated in docs once canonical options are stable

## 9. Testing Plan

### 9.1 Required fixture families
Create fixtures for:
- basic planar polygonization
- dirty / non-noded linework
- touch-policy edge cases
- conflicting Z-at-XY cases
- mixed boundary families
- profile-tag passthrough
- `grid` vs `geos_compat`
- invalid Wasm/Python binding inputs

### 9.2 Required assertions
The suite should assert:
- same fixture with report mode explains mismatches by profile
- same fixture with report mode explains mismatches by boundary lines
- mixed-boundary fixture yields at least one polygon with provenance containing lines from different boundary families
- wrapper APIs produce the same result as canonical options
- typed binding errors are stable and actionable

## 10. Suggested Result Types

### 10.1 Core result shape
```rust
pub struct PolygonizeResult {
	pub polygons: Vec<PolygonOutput>,
	pub diagnostics: Option<PolygonizerDiagnostics>,
	pub report: Option<PolygonizerReport>,
}

pub struct PolygonOutput {
	pub polygon: Polygon3D,
	pub provenance: Option<PolygonProvenance>,
}
```

## 11. Suggested File / Module Layout

- `crates/geo-polygonize-core/src/options.rs`
- `crates/geo-polygonize-core/src/errors.rs`
- `crates/geo-polygonize-core/src/diagnostics.rs`
- `crates/geo-polygonize-core/src/provenance.rs`
- `crates/geo-polygonize-core/src/report.rs`
- `crates/geo-polygonize-core/src/containment.rs`
- `crates/geo-polygonize-core/src/polygonizer.rs`

## 12. Minimal First Landing

1. `PolygonizerOptions`
2. `polygonize_with_options`
3. wrapper parity tests
4. `DiagnosticsOptions { enabled, report_mode }`
5. typed Wasm/Python errors
6. optional `input_profile_id`
7. placeholder report payload without full provenance
8. optional `line_ids`
9. `PolygonProvenance { boundary_line_ids, input_profile_id }`

## 13. Milestone-to-Implementation Mapping

### M1
- canonical sort mode
- fixtures
- diagnostics scaffold
- typed binding errors
- report mode scaffold

### M2
- `PolygonizerOptions`
- cross-binding `polygonize_with_options`
- wrapper parity
- optional `line_ids`
- `input_profile_id`
- `PolygonProvenance`

### M3
- `SnapStrategy`
- parametric split accumulation
- clone reduction
- integer snap-grid prototype
- per-kernel benches

### M4
- tile ownership policies
- containment forest
- touch policies
- cross-tile dedup
- report mode explains profile/provenance mismatches

### M5
- `rstar` containment index
- adaptive regrid
- optional advanced noder
- hardened `geos_compat`

## 14. Non-Goals for This Document

This document does not fully specify:
- the final advanced noder algorithm
- every internal memory-layout optimization
- every future release policy detail
