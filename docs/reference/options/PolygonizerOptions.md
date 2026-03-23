# PolygonizerOptions

The canonical configuration object for the `geo-polygonize` engine.

This struct controls every aspect of the polygonization pipeline, including
topological robustness, feature output, containment policies, noding, and determinism.

## Fields

### `target`

**Type:** `TargetProfile`

Determines the execution environment profile (e.g., Native, WasmSingleThread, WasmThreads).

Default: `TargetProfile::Native`

### `node_input`

**Type:** `bool`

Whether to robustly node the input before polygonization.

Enable this for real-world linework where segment intersections may not
already exist as explicit vertices. This is slower than the fast path but
avoids missing faces and unresolved crossings.

Default: `false`

### `snap_grid_size`

**Type:** `f64`

The snapping grid size used for vertex deduplication and noding operations.

Vertices falling within the same grid cell are coalesced. A size of `0.0`
indicates exact floating-point evaluation without grid snapping.

Default: `1e-10`

### `extract_only_polygonal`

**Type:** `bool`

If `true`, only pure, outermost polygonal shells are returned.

Floating dangles, internal cut-lines, or invalid rings will be discarded.

Default: `false`

### `snap_strategy`

**Type:** `SnapStrategy`

The underlying strategy to apply when snapping coordinate geometries.

See `SnapStrategy` for differences between strict `Grid` snapping and
Shapely/GEOS `GeosCompat` strategies.

Default: `SnapStrategy::Grid`

### `noding`

**Type:** `NodingOptions`

Configures the noding engine backend and behavior.

### `containment`

**Type:** `ContainmentOptions`

Configures how topological relationships (containment) are calculated
during face formation.

### `tiling`

**Type:** `Option<TilingOptions>` (Optional)

Optional configuration for tiled, distributed execution across huge datasets.

### `z`

**Type:** `ZOptions`

Configures Z-axis coordinate handling.

### `determinism`

**Type:** `DeterminismOptions`

Configuration for enforcing exact topological determinism.

### `diagnostics`

**Type:** `DiagnosticsOptions`

Options for capturing diagnostic topology failures.

### `provenance`

**Type:** `ProvenanceOptions`

Options for mapping final faces back to original input geometry IDs.

### `input_profile_id`

**Type:** `Option<String>` (Optional)

An optional identifier for the input dataset.
