# Benchmarks

This repository contains benchmarks to compare the performance of `geo-polygonize` against the optimized GEOS C++ library (via Python `shapely`).

## Running Benchmarks

### Prerequisites

* Rust (cargo)
* Python 3
* `shapely` python package (`pip install shapely`)

### Automated Comparison

Run the provided script to build and run both benchmarks and generate a comparison table:

```bash
bash crates/geo-polygonize-core/benches/run_comparison.sh
```

### Manual Execution

**Rust Benchmarks:**

```bash
cargo bench -p geo-polygonize-core --bench polygonize_bench
```

**Python Benchmarks:**

```bash
python3 crates/geo-polygonize-core/benches/bench_shapely.py
```

## Comparative Results

As of `geo-polygonize` v0.1.0 (with Parallel R-Tree noding, Memory Pooling, Tiling, and Parallel Bulk Loading):

**Environment:** GitHub Action Runner (Standard Linux, likely 2 vCPUs).

### Grid Topology (Intersecting Lines)

| Input Size (NxN) | Rust Time (s) | Python Time (s) | Wasm Time (s) | Speedup (Py/Rs) | Speedup (Py/Wasm) | Speedup (Wasm/Rs) |
|---|---|---|---|---|---|---|
<<<<<<< optimize-ci-build-times-827039151796391152
| 5 | 0.000200 | 0.000709 | 0.002520 | 3.55x | 0.28x | 12.60x |
| 10 | 0.000409 | 0.002211 | 0.001200 | 5.41x | 1.84x | 2.94x |
| 20 | 0.001049 | 0.008256 | 0.004490 | 7.87x | 1.84x | 4.28x |
| 50 | 0.004337 | 0.052597 | 0.010530 | 12.13x | 4.99x | 2.43x |
| 100 | 0.018240 | 0.240220 | 0.029700 | 13.17x | 8.09x | 1.63x |
=======
| 5 | 0.000184 | 0.000678 | 0.002540 | 3.69x | 0.27x | 13.83x |
| 10 | 0.000400 | 0.002211 | 0.001280 | 5.53x | 1.73x | 3.20x |
| 20 | 0.001037 | 0.008083 | 0.003710 | 7.79x | 2.18x | 3.58x |
| 50 | 0.004328 | 0.050565 | 0.011260 | 11.68x | 4.49x | 2.60x |
| 100 | 0.016374 | 0.219965 | 0.029190 | 13.43x | 7.54x | 1.78x |
>>>>>>> main

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Wasm Time (s) | Speedup (Py/Rs) | Speedup (Py/Wasm) | Speedup (Wasm/Rs) |
|---|---|---|---|---|---|---|
<<<<<<< optimize-ci-build-times-827039151796391152
| 50 | 0.000935 | 0.008238 | 0.001070 | 8.81x | 7.70x | 1.14x |
| 100 | 0.003069 | 0.026532 | 0.004140 | 8.65x | 6.41x | 1.35x |
| 200 | 0.011148 | 0.106587 | 0.015720 | 9.56x | 6.78x | 1.41x |
=======
| 50 | 0.000945 | 0.008114 | 0.001110 | 8.58x | 7.31x | 1.17x |
| 100 | 0.003018 | 0.026087 | 0.003780 | 8.64x | 6.90x | 1.25x |
| 200 | 0.010904 | 0.104510 | 0.015480 | 9.58x | 6.75x | 1.42x |
>>>>>>> main

**Analysis:**
The library offers a pure Rust native alternative to GEOS.
- **Performance:** On constrained environments (like CI runners with few cores), the parallel overhead of `rayon` may limit speedups compared to the highly optimized single-threaded C++ GEOS backend.
- **Tiling Strategy:** For large dense datasets (e.g., Grid 100), the **TiledPolygonizer** provides a significant speedup (~1.7x to 2.8x faster than the naive approach), bridging the gap towards GEOS performance. This validates the scalability architecture for large-scale GIS tasks.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading Z-order sort.

## WebAssembly Benchmarks

Benchmarks running in Node.js (V8) via `wasm-bindgen`, utilizing `talc` allocator and SIMD optimizations.

| Grid Size | Polygonize (Clean) | GeoArrow Ingest | Robust Noding (Dirty) |
|---|---|---|---|
| 10x10 | 0.35 ms | 0.33 ms | 7.36 ms |
| 20x20 | 0.35 ms | 0.22 ms | 21.29 ms |
| 50x50 | 0.67 ms | 0.64 ms | 156.35 ms |

*Note:*
- **Clean Input:** Pre-noded lines (no intersection checks).
- **Dirty Input:** Self-intersecting lines (bowtie grid) requiring Iterated Snap Rounding.
- **GeoArrow:** Measures ingestion into Arrow columnar memory. Note that current Wasm benchmarks include JSON deserialization overhead, which dominates small-scale tests.
