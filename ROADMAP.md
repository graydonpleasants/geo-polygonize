# Roadmap

This document outlines future improvements and optimization ideas for the `geo-polygonize` library.

## Performance

- [ ] **Parallel Uniform Grid Construction**: The current `UniformGrid::new` implementation is single-threaded. Parallelizing the insertion of lines into grid cells could significantly speed up the noding phase for large datasets.
- [ ] **Zero-Copy Wasm Ingestion**: Improve `LineStringBuilder` and Wasm bindings to minimize or eliminate copying when passing data from JavaScript to Wasm.
- [ ] **SIMD Optimization**: Further optimize `SoALines` and `check_intersection_simd` to leverage wider SIMD registers (AVX-512) where available.
- [ ] **Grid Cell Size Heuristic**: Refine the heuristic for determining the optimal `UniformGrid` cell size. Currently, it uses a simple density-based formula, but adaptive sizing or QuadTrees might perform better for non-uniform distributions.

## Robustness

- [ ] **Exact Arithmetic**: Explore using exact geometric predicates (e.g., `robust` crate for more than just orientation) or exact arithmetic types to handle extreme edge cases and degeneracies without epsilon tuning.
- [ ] **Snap Rounding Precision**: Investigate dynamic precision scaling for Snap Rounding to handle datasets with varying coordinate scales.

## Features

- [ ] **Polygon Simplification**: Add an optional post-processing step to simplify the resulting polygons (e.g., Douglas-Peucker) while maintaining topology.
- [ ] **Hole Assignment Optimization**: The current R-Tree based hole assignment is robust but can be slow. Explore plane-sweep algorithms or using the existing `PlanarGraph` topology for faster hole detection.

## Testing

- [ ] **Fuzz Testing**: Implement fuzz testing to discover edge cases in noding and polygon assembly.
- [ ] **Complex Topologies**: Add more integration tests for complex scenarios like self-intersecting polygons, butterfly polygons, and massive datasets of touching rings.
