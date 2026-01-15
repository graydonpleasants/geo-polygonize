# Geo Polygonize

A native Rust port of the JTS/GEOS polygonization algorithm. This crate allows you to reconstruct valid polygons from a set of lines, including handling of complex topologies like holes, nested shells, and disconnected components.

## Features

- **Robust Polygonization**: Extracts polygons from unstructured linework.
- **Robust Noding**: Implements **Iterated Snap Rounding (ISR)** to guarantee topological correctness on dirty inputs (self-intersections, overlaps).
- **Hardware Acceleration**: Uses **SIMD** instructions (via `wide` crate) for critical geometric predicates like Point-in-Polygon checks.
- **Wasm Optimized**: Tailored for WebAssembly with `talc` allocator and Zero-Copy data support (`geoarrow`).
- **Performance**: Competitive with GEOS/Shapely (C++), outperforming it on random sparse inputs and scaling well on dense grids.
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
    // Optional: Configure snap grid (default 1e-10)
    poly.snap_grid_size = 1e-6;

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

This implementation moves away from the pointer-based graph structures of JTS/GEOS to a Rust-idiomatic Index Graph (Arena) approach.

See [ARCHITECTURE.md](ARCHITECTURE.md) for a deep dive into the optimization strategies.

Key optimizations include:
1.  **Robust Noding**: Iterated Snap Rounding (ISR) using `rstar` for intersection detection and grid snapping.
2.  **Vectorization**: SIMD-accelerated Ray Casting for efficient Hole Assignment.
3.  **Memory Layout**: Structure of Arrays (SoA) for graph nodes and `talc` allocator for Wasm.

## License

MIT/Apache-2.0
