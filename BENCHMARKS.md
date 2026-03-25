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
| 5 | 0.000206 | 0.000627 | 0.002660 | 3.05x | 0.24x | 12.93x |
| 10 | 0.000430 | 0.002119 | 0.001250 | 4.93x | 1.70x | 2.91x |
| 20 | 0.001154 | 0.007866 | 0.003650 | 6.82x | 2.16x | 3.16x |
| 50 | 0.004771 | 0.050115 | 0.010940 | 10.50x | 4.58x | 2.29x |
| 100 | 0.017477 | 0.213985 | 0.031040 | 12.24x | 6.89x | 1.78x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Wasm Time (s) | Speedup (Py/Rs) | Speedup (Py/Wasm) | Speedup (Wasm/Rs) |
|---|---|---|---|---|---|---|
| 50 | 0.001023 | 0.007919 | 0.001150 | 7.74x | 6.89x | 1.12x |
| 100 | 0.003328 | 0.025088 | 0.004010 | 7.54x | 6.26x | 1.20x |
| 200 | 0.011460 | 0.097606 | 0.016510 | 8.52x | 5.91x | 1.44x |

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
