# SnapStrategy

Strategy for snapping coordinates to a grid.

`Grid` applies a standard round-to-nearest integer grid policy `(coord / size).round() * size`.
`GeosCompat` attempts to emulate GEOS / Shapely's `set_precision` behavior, which uses
C++ std::round (rounding halfway cases away from zero).

**Scale Guidance:**
Use `Grid` for native Rust applications where absolute topological determinism and
intuitive rounding is preferred.
Use `GeosCompat` if you are using `geo-polygonize` to replace a Shapely / GEOS pipeline
and require exact parity for edge cases at precision boundaries.

**Expected Parity:**
Rust's native `f64::round()` rounds halfway cases away from zero. This perfectly
matches C++ `std::round` behavior, meaning that `Grid` and `GeosCompat` currently
map identical values for basic single-coordinate precision points (e.g., `-0.5` => `-1.0`
and `0.5` => `1.0`).

**Expected Divergence:**
The divergence between `Grid` and `GeosCompat` strategies typically arises under complex
topological scaling sequences rather than simple single point coordinate rounding, but
the option ensures the library remains semantically compatible with Python/C++ GEOS pipelines.

## Variants

### `Grid`

### `GeosCompat`

