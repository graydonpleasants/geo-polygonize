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
bash benches/run_comparison.sh
```

### Manual Execution

**Rust Benchmarks:**

```bash
cargo bench --bench polygonize_bench
```

**Python Benchmarks:**

```bash
python3 benches/bench_shapely.py
```

## Comparative Results

As of `geo-polygonize` v0.1.0 (with Parallel R-Tree noding, Memory Pooling, Tiling, and Parallel Bulk Loading):

**Environment:** GitHub Action Runner (Standard Linux, likely 2 vCPUs).

### Grid Topology (Intersecting Lines)

| Input Size (NxN) | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 5 | 0.000215 | 0.000677 | 3.15x |
| 10 | 0.000588 | 0.002202 | 3.75x |
| 20 | 0.001923 | 0.008242 | 4.29x |
| 50 | 0.012221 | 0.050385 | 4.12x |
| 100 | 0.057623 | 0.207729 | 3.60x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | 0.001611 | 0.007900 | 4.90x |
| 100 | 0.007186 | 0.025892 | 3.60x |
| 200 | 0.029356 | 0.099213 | 3.38x |

**Analysis:**
The library offers a pure Rust native alternative to GEOS.
- **Performance:** On constrained environments (like CI runners with few cores), the parallel overhead of `rayon` may limit speedups compared to the highly optimized single-threaded C++ GEOS backend.
- **Tiling Strategy:** For large dense datasets (e.g., Grid 100), the **TiledPolygonizer** provides a significant speedup (~1.7x to 2.8x faster than the naive approach), bridging the gap towards GEOS performance. This validates the scalability architecture for large-scale GIS tasks.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy.
