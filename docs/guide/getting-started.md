# Getting Started

Welcome to the `geo-polygonize` documentation!

This library provides high-performance polygonization of lines and rings. It is written in Rust and exposes APIs for:

* **JavaScript/TypeScript** via WebAssembly
* **Python** via a CFFI/PyO3 extension
* **Rust** via a native crate

## Installation

See the respective integration guides for details.

Rust callers should use the stateless entrypoint and an explicit options object:

```rust
use geo_polygonize_core::options::PolygonizerOptions;
use geo_polygonize_core::{polygonize, Coord3D, Line3D};

let a = Coord3D::new(0.0, 0.0, 0.0);
let b = Coord3D::new(1.0, 0.0, 0.0);
let c = Coord3D::new(0.0, 1.0, 0.0);
let lines = [
    Line3D::new(a, b, 1),
    Line3D::new(b, c, 2),
    Line3D::new(c, a, 3),
];

let result = polygonize(lines, &PolygonizerOptions::default())?;
assert_eq!(result.polygons.len(), 1);
# Ok::<(), geo_polygonize_core::error::PolygonizeError>(())
```

Python users install `geo-polygonize-py` and import `geo_polygonize`:

```bash
pip install geo-polygonize-py
```
