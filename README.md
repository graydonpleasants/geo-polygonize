# Geo Polygonize

A native Rust port of the JTS/GEOS polygonization algorithm. This crate allows you to reconstruct valid polygons from a set of lines, including handling of complex topologies like holes, nested shells, and disconnected components.

## Features

- **Robust Polygonization**: Extracts polygons from unstructured linework.
- **Hole Assignment**: Correctly assigns holes to their parent shells.
- **Planar Graph**: Uses an efficient arena-based index graph implementation.
- **Geo Ecosystem**: Fully integrated with `geo-types` and `geo` crates.

## Usage

```rust
use geo_polygonize::Polygonizer;
use geo_types::LineString;

fn main() {
    let mut poly = Polygonizer::new();

    // Add lines (e.g., a square)
    poly.add_geometry(LineString::from(vec![
        (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)
    ]).into());

    let polygons = poly.polygonize().expect("Polygonization failed");

    for p in polygons {
        println!("Found polygon with area: {}", p.unsigned_area());
    }
}
```

## Architecture

This implementation moves away from the pointer-based graph structures of JTS/GEOS to a Rust-idiomatic Index Graph (Arena) approach. This ensures memory safety and enables potential parallelization.

## License

MIT/Apache-2.0
