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
use geo_polygonize_core::{polygonize_line_strings, PolygonizerOptions};
use geo_types::LineString;

let ring = LineString::from(vec![
    (0.0, 0.0),
    (1.0, 0.0),
    (0.0, 1.0),
    (0.0, 0.0),
]);

let result = polygonize_line_strings([&ring], &PolygonizerOptions::default())?;
assert_eq!(result.polygons.len(), 1);
assert_eq!(result.into_multi_polygon().0.len(), 1);
# Ok::<(), geo_polygonize_core::PolygonizeError>(())
```

`PolygonizerOptions::default()` uses floating precision. Choose
`PrecisionModel::FixedGrid { grid_size }` only when the input has an explicit
grid contract; fixed precision rounds topology coordinates even without noding.
Set `noding.guarantee` to `CertifiedFixedPrecision` for hot-pixel snap rounding;
it requires `node_input`, fixed-grid precision, the Snap backend, and Grid output.

Topology is two-dimensional. Configure `options.z.policy` when 3D input needs
split-vertex interpolation, nearest-endpoint reconstruction, zeroed output, or
an error for same-XY Z conflicts.

Python users install `geo-polygonize-py` and import `geo_polygonize`:

```bash
pip install geo-polygonize-py
```
