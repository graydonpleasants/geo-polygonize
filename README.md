# Geo Polygonize

A native Rust port of the JTS/GEOS polygonization algorithm. This crate allows you to reconstruct valid polygons from a set of lines, including handling of complex topologies like holes, nested shells, and disconnected components.

## Features

- **Robust Polygonization**: Extracts polygons from unstructured linework.
- **Efficient Noding**: Implements an optimized R-Tree based iterative noder ($O(N \log N)$) with collinear overlap handling.
- **Performance**: Competitive with GEOS/Shapely (C++), outperforming it on random sparse inputs and scaling well on dense grids.
- **Hole Assignment**: Correctly assigns holes to their parent shells.
- **Planar Graph**: Uses an efficient arena-based index graph implementation (Structure of Arrays) for memory efficiency.
- **Geo Ecosystem**: Fully integrated with `geo-types` and `geo` crates.

## Usage

### Library

```rust
use geo_polygonize::Polygonizer;
use geo_types::LineString;

fn main() {
    let mut poly = Polygonizer::new();

    // Enable robust noding if lines might intersect
    poly.node_input = true;

    // Add lines (e.g., a square with diagonals)
    poly.add_geometry(LineString::from(vec![
        (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)
    ]).into());
    poly.add_geometry(LineString::from(vec![
        (0.0, 0.0), (10.0, 10.0)
    ]).into());

    let polygons = poly.polygonize().expect("Polygonization failed");

    for p in polygons {
        println!("Found polygon with area: {}", p.unsigned_area());
    }
}
```

### WebAssembly (WASM)

This library supports WebAssembly with an ergonomic dual-build configuration that automatically utilizes SIMD instructions where available.

**Installation:**
```bash
npm install geo-polygonize
```

**Standard Usage (Bundlers / Browser):**
The default entry point automatically handles feature detection (SIMD) and lazy-loading of the Wasm binary. The Wasm is inlined as a Base64 Data URI, so no extra bundler configuration is needed.

```javascript
import init, { polygonize } from "geo-polygonize";

async function run() {
    await init();

    const geojson = {
        "type": "FeatureCollection",
        "features": [
            // ... your line features
        ]
    };

    // Returns a GeoJSON FeatureCollection string
    const result = polygonize(JSON.stringify(geojson));
    console.log(JSON.parse(result));
}

run();
```

**Slim Usage (Manual Loading):**
If you prefer to manage the Wasm binary yourself (e.g., to reduce bundle size or load from a CDN), import from `geo-polygonize/slim`.

```javascript
import { initBest, polygonize } from "geo-polygonize/slim";

async function run() {
    // You must provide the compiled WebAssembly.Module or URL
    // You can choose to load the SIMD or Scalar version based on your own detection or availability
    const response = await fetch("geo_polygonize.wasm");
    const buffer = await response.arrayBuffer();
    const module = await WebAssembly.compile(buffer);

    // Helper to initialize the best available implementation
    // Pass the module to both arguments if you only have one version
    await initBest(module, module);

    // ... use polygonize
}
```

### CLI Example

The repository includes a CLI tool to polygonize GeoJSON files.

```bash
# Build the example
cargo build --example polygonize --release

# Run on input lines
cargo run --release --example polygonize -- --input lines.geojson --output polygons.geojson --node
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

This implementation moves away from the pointer-based graph structures of JTS/GEOS to a Rust-idiomatic Index Graph (Arena) approach. This ensures memory safety and enables potential parallelization. Optimization efforts have focused on:
1.  **Bulk Loading**: Graph nodes are built via parallel sort/deduplication to avoid `HashMap` overhead.
2.  **Memory Layout**: Edges are stored as compact `Line` structs rather than heap-allocated `LineString`s.
3.  **Spatial Indexing**: Noding uses `rstar` for efficient intersection detection.

## License

MIT/Apache-2.0
