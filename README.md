# Geo Polygonize

A native Rust port of the JTS/GEOS polygonization algorithm. This crate allows you to reconstruct valid polygons from a set of lines, including handling of complex topologies like holes, nested shells, and disconnected components.

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/graydonpleasants/geo-polygonize)

## Features

- **Robust Polygonization**: Extracts polygons from unstructured linework.
- **Iterative Grid Noding (Unchecked)**: Splits and snaps dirty linework, without claiming certified snap-rounding guarantees.
- **Certified Fixed-Precision Noding**: Optional hot-pixel snap rounding with an independent full-noding postcondition check.
- **Hardware Acceleration**: Uses **SIMD** instructions (via `wide` crate) for critical geometric predicates like Point-in-Polygon checks.
- **Wasm Optimized**: Tailored for WebAssembly with `talc` allocator and binary GeoArrow support.
- **Performance**: SIMD, spatial indexing, and optional parallel execution, with checked-in benchmark tooling.
- **Geo Ecosystem**: Fully integrated with `geo-types` and `geo` crates.
- **GeoArrow Support**: Arrow C Data Interface and Arrow IPC integration with GeoArrow metadata.

## Engineering Roadmap

For delivered foundations and the next evidence-gated work, see [ROADMAP.md](ROADMAP.md).

## Usage

### Library

```rust
use geo_polygonize_core::{polygonize_line_strings, PolygonizerOptions};
use geo_types::LineString;

fn main() {
    let ring = LineString::from(vec![
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 0.0),
    ]);

    let result = polygonize_line_strings([&ring], &PolygonizerOptions::default())
        .expect("Polygonization failed");
    let polygons = result.into_multi_polygon();

    for polygon in polygons {
        println!("Found polygon: {polygon:?}");
    }
}
```

`polygonize_line_strings` accepts owned or borrowed `geo_traits::LineStringTrait`
values. Use `polygonize` when source IDs or Z values must be supplied explicitly,
and keep its full result when dangles, cut edges, invalid rings, or diagnostics are
needed.

Rust consumers should import the supported facade from the crate root. Graph,
noding, containment, utility, mutable-builder, and tiled APIs are internal or
experimental and may change without compatibility guarantees before `1.0`.
`TiledPolygonizer` validates its grid/options and propagates per-tile errors,
but callers must still choose a sufficient buffer and verify untiled equivalence
for their workload. Its result includes per-tile topology counts and merge/dedup
counts; these are observational and do not certify buffer sufficiency. Canonical
mode applies the same final ordering pass after tile merge; the equivalence gate
covers one, two, four, and many boundary crossings plus concave, hole,
dirty-intersection, and dangle cases.

### Choosing noding and precision

Polygonization quality is heavily influenced by input noding strategy.

- **`node_input = false`** (default): Fastest path. Use this when your input linework is already noded (all intersections are explicit vertices).
- **`node_input = true`**: Enables unchecked iterative grid noding. Use this for real-world datasets that may contain slight misalignments, overlaps, or self-intersections, and validate outputs when correctness must be certified.
- **`PrecisionModel::Floating`** (default) preserves input coordinates and uses floating-point intersections.
- **`PrecisionModel::FixedGrid { grid_size }`** rounds topology coordinates to an explicit positive grid, even when input noding is disabled. Choose the grid in the units of your data; oversnapping can collapse narrow features.

Practical workflow:
1. Run with `node_input = false` first on trusted data.
2. If you observe missing polygons, sliver artifacts, or unresolved intersections, enable `node_input`.
3. Use a fixed precision model only when your application has an explicit coordinate grid contract.

Canonical options now use `precision_model` instead of `snap_grid_size`.
Legacy positional Python, Wasm, and C APIs still translate their grid argument
when noding is enabled and ignore it on the non-noding fast path.

Set `options.noding.guarantee` to `NodingGuarantee::Validate` to run an
independent full-noding check before graph construction. Validation reports the
first pair with an interior intersection or unnormalized collinear overlap.
Use `NodingGuarantee::CertifiedFixedPrecision` with `node_input`, `FixedGrid`,
the `Snap` backend, and `SnapStrategy::Grid` for hot-pixel snap rounding plus
that validation. Certified coordinates must fit exact integer grid indices.

### Z semantics

Topology is always computed in XY. `ZPolicy::InterpolateAlongEdge` is the
default and reconstructs split-vertex Z on each source edge;
`PreferNearestEndpoint` uses the nearer endpoint, and `Ignore` emits zero Z.
When several source edges meet at the same XY node, the graph chooses the Z
from the lowest line ID (then the lowest Z) and reports conflicts through
diagnostics. `ErrorOnConflict` instead returns a typed error when the difference
exceeds `z.conflict_tolerance`.

Use `Line3D` or the Python/Wasm typed-buffer APIs with `stride=3` for Z-aware
processing. The GeoRust, GeoJSON, and current GeoArrow adapters are XY-only.

### Output semantics

The polygonizer intentionally returns only valid polygonal areas that can be formed from closed cycles:

- **Dangles are removed**: dead-end edges do not appear in output polygons.
- **Cut edges are excluded**: edges that are connected but cannot bound a face are ignored.
- **Holes and nested shells are preserved** when enough boundary information is present.

This behavior matches classical JTS/GEOS polygonization semantics and is useful for cleaning linework before area analysis.

### GeoArrow Integration

Rust Arrow and GeoArrow adapters live in the `geo-polygonize-arrow` crate;
the Wasm crate includes it automatically.

For the Arrow C Data Interface ABI, including ownership and error retrieval,
see [docs/C_ABI.md](docs/C_ABI.md).

```rust
use geo_polygonize_arrow::{polygonize_arrow, PolygonizerOptions};
// ... create Arrow array ...
// let result = polygonize_arrow(&array, &field, options);
```

The FlatGeoBuf file adapter lives in the `geo-polygonize-flatgeobuf` crate.

### Python

The Python package is published as `geo-polygonize-py` and imported as `geo_polygonize`.

```bash
pip install geo-polygonize-py
```

```python
import numpy as np
from geo_polygonize import polygonize, import_probe

# 1. Using Shapely LineStrings or coordinate lists directly
lines = [
    [(0, 0), (10, 0), (10, 10), (0, 10), (0, 0)],
    [(0, 0), (10, 10)]
]

# return_polygons=True returns a list of shapely.geometry.Polygon objects
polygons = polygonize(lines=lines, return_polygons=True)
for p in polygons:
    print(p.area)

# 2. Using High-Performance Flat Arrays
# Flat buffers avoid Python object-per-coordinate overhead
coords = np.array([
    0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0,
    0.0, 0.0, 10.0, 10.0
], dtype=np.float64)

# Start indices for each line segment.
# The final closing offset is computed implicitly.
offsets = np.array([0, 5], dtype=np.uint32)

# Returns a stable dictionary with 'polygons', diagnostics, and provenance.
result_dict = polygonize(coords=coords, offsets=offsets)

# Native-extension probes are cheap and safe for optional integrations.
ok, error = import_probe()
```

CFB/autograder integrations should use the versioned production profile rather
than assembling caller-side knobs or using legacy `polygonize(..., node=True,
snap=0.5)` calls:

```python
from geo_polygonize import cfb_robust_options, polygonize_with_options

result = polygonize_with_options(
    coords=coords,
    offsets=offsets,
    options=cfb_robust_options(),
)
```

The default return shape is a stable dictionary with `polygons` as
`SimplePolygon` values. Use `return_polygons=True` only when you want Shapely
`Polygon` objects.

When provenance is enabled, coincident and partially overlapping input lines
are dissolved into one topology edge while every contributing nonzero
`line_id` remains in the polygon's sorted `boundary_line_ids`.

`SnapStrategy::Grid` keeps topology and output coordinates on a configured
fixed precision grid. The CFB profile uses `GeosCompat`: the grid establishes robust
topology, then output nodes regain deterministic source XY coordinates to better
match Shapely `snap` plus full-precision noding. It is not `set_precision`
emulation, and exact parity is not guaranteed for many-to-one snaps.

For Shapely parity checks, compare report-mode outputs with the built-in
mismatch helper:

```python
from geo_polygonize import explain_mismatch, polygonize_with_options

options = cfb_robust_options()
result_a = polygonize_with_options(coords=coords_a, offsets=offsets_a, options=options)
result_b = polygonize_with_options(coords=coords_b, offsets=offsets_b, options=options)
result_a["options"] = options
result_b["options"] = options

mismatch = explain_mismatch(result_a, result_b)
```

For a minimal Shapely smoke comparison, use area signatures:

```python
from shapely.ops import polygonize as shapely_polygonize

rust_polys = polygonize_with_options(lines=lines, options=cfb_robust_options(), return_polygons=True)
rust_areas = sorted(round(poly.area, 6) for poly in rust_polys)
shapely_areas = sorted(round(poly.area, 6) for poly in shapely_polygonize(lines))
```

### WebAssembly (WASM)

This library supports WebAssembly with an ergonomic dual-build configuration that automatically utilizes SIMD instructions where available.

**Installation:**
```bash
npm install geo-polygonize
```

**Standard Usage (Quick Demos):**
The default entry point automatically handles feature detection (SIMD) and lazy-loading of the Wasm binary. The Wasm is inlined as a Base64 Data URI, so no extra bundler configuration is needed. For app builds, prefer the slim entry point below so your bundler keeps the Wasm assets out of the JavaScript chunk.

```javascript
import init, { polygonize, polygonize_geoarrow } from "geo-polygonize";

async function run() {
    await init();

    const geojson = {
        "type": "FeatureCollection",
        "features": [
            // ... your line features
        ]
    };

    // Returns a GeoJSON FeatureCollection string
    // Pass explicitly matching backend configuration if desired
    const result = polygonize(
        JSON.stringify(geojson),
        true, // node_input
        0.5   // snap_grid_size
    );
    console.log(JSON.parse(result));

    // Or use Arrow IPC bytes
    // const ipcBuffer = ...;
    // const arrowResult = polygonize_geoarrow(ipcBuffer, false, 1e-10, false);
}
```

**Slim Usage (Apps / Manual Loading):**
For Vite and other app bundlers, import from `geo-polygonize/slim` and pass explicit Wasm asset URLs.

```javascript
import { cfbRobustOptions, initBest } from "geo-polygonize/slim";
import scalarUrl from "geo-polygonize/geo_polygonize.wasm?url";
import simdUrl from "geo-polygonize/geo_polygonize_simd.wasm?url";

async function run() {
    const wasm = await initBest(
        { module_or_path: scalarUrl },
        { module_or_path: simdUrl },
    );

    const result = wasm.polygonizeWithOptions(
        JSON.stringify(geojson),
        cfbRobustOptions,
    );
}
```

**Multithreaded Usage (Experimental):**
This library provides a multithreaded build powered by `wasm-bindgen-rayon`.

```javascript
import init, { initThreadPool, polygonize } from "geo-polygonize/threads";

async function run() {
    await init();

    // Initialize thread pool (e.g., with navigator.hardwareConcurrency)
    await initThreadPool(navigator.hardwareConcurrency);

    // ... use polygonize as usual
}
```

**Important:** Multithreaded WebAssembly requires `SharedArrayBuffer`, which is only available in secure contexts. You **must** serve your page with the following headers:
```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

### CLI Example

The repository includes a CLI tool to polygonize GeoJSON files.

```bash
# Build the example
cargo build -p geo-polygonize-core --example polygonize --release

# Run on input lines
cargo run -p geo-polygonize-core --release --example polygonize -- --input lines.geojson --output polygons.geojson --node
```

### Visualization

You can visualize the results using the provided Python script (requires `matplotlib` and `shapely`).

```bash
python3 scripts/visualize.py --input lines.geojson --output polygons.geojson --save result.png
```

## Examples

Below are some examples of what the polygonizer can do.

### Nested Holes and Islands

The algorithm correctly identifies nested structures (Island inside a Hole inside a Shell).

![Nested Holes](images/nested_holes.png)

### Incomplete Grid / Dangles

The algorithm prunes dangles (dead-end lines) and extracts only closed cycles.

![Incomplete Grid](images/grid_incomplete.png)

### Touching Polygons (Shared Edges)

Using robust noding (`--node`), it can reconstruct adjacent polygons that share boundaries, even if the input lines are not perfectly noded.

![Touching Polygons](images/touching_polys.png)

### Self-Intersecting Geometry (Bowtie)

Self-intersecting lines are split at intersection points, and valid cycles are extracted.

![Bowtie](images/complex_bowtie.png)

### Complex Geometries

The polygonizer can handle complex, curved inputs (approximated by LineStrings) such as overlapping circles and shapes with multiple holes.

**Overlapping Circles**: Note how the intersection regions are correctly identified as separate polygons.

![Overlapping Circles](images/overlapping_circles.png)

**Curved Holes**: A complex polygon with multiple circular holes.

![Curved Holes](images/curved_holes.png)

## Benchmarks

This library includes a "severe" comparison suite against `shapely` (GEOS).

See [BENCHMARKS.md](BENCHMARKS.md) for detailed results and instructions on how to run them.

## Architecture

This implementation moves away from the pointer-based graph structures of JTS/GEOS to a Rust-idiomatic Index Graph (Arena) approach.

See [ARCHITECTURE.md](ARCHITECTURE.md) for a deep dive into the optimization strategies.

Key optimizations include:
1.  **Noding**: Unchecked iterative grid noding with spatially dispatched intersection detection.
2.  **Vectorization**: SIMD-accelerated Ray Casting for efficient Hole Assignment.
3.  **Memory Layout**: Structure of Arrays (SoA) for graph nodes and `talc` allocator for Wasm.

## License

MIT/Apache-2.0
