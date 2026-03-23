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
| 5 | 0.000163 | 0.000612 | 0.002290 | 3.76x | 0.27x | 14.08x |
| 10 | 0.000374 | 0.002048 | 0.001200 | 5.47x | 1.71x | 3.21x |
| 20 | 0.000985 | 0.007700 | 0.003360 | 7.82x | 2.29x | 3.41x |
| 50 | 0.004442 | 0.049837 | 0.013090 | 11.22x | 3.81x | 2.95x |
| 100 | 0.018617 | 0.227430 | 0.029410 | 12.22x | 7.73x | 1.58x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Wasm Time (s) | Speedup (Py/Rs) | Speedup (Py/Wasm) | Speedup (Wasm/Rs) |
|---|---|---|---|---|---|---|
| 50 | 0.000898 | 0.007781 | 0.001050 | 8.67x | 7.41x | 1.17x |
| 100 | 0.003004 | 0.024631 | 0.003720 | 8.20x | 6.62x | 1.24x |
| 200 | 0.011015 | 0.102293 | 0.014910 | 9.29x | 6.86x | 1.35x |

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
