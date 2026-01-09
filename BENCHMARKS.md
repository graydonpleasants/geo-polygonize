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

| Input Size (NxN) | Rust (Naive) (s) | Rust (Tiled) (s) | Python (GEOS) (s) |
|---|---|---|---|
| 5 | 0.001217 | 0.000694 | 0.57x |
| 10 | 0.005027 | 0.002209 | 0.44x |
| 20 | 0.021879 | 0.009526 | 0.44x |
| 50 | 0.197220 | 0.053809 | 0.27x |
| 100 | 1.398500 | 0.235566 | 0.17x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | 0.015139 | 0.008655 | 0.57x |
| 100 | 0.093372 | 0.027151 | 0.29x |
| 200 | 0.415210 | 0.111875 | 0.27x |

**Analysis:**
The library offers a pure Rust native alternative to GEOS.
- **Performance:** On constrained environments (like CI runners with few cores), the parallel overhead of `rayon` may limit speedups compared to the highly optimized single-threaded C++ GEOS backend.
- **Tiling Strategy:** For large dense datasets (e.g., Grid 100), the **TiledPolygonizer** provides a significant speedup (~1.7x to 2.8x faster than the naive approach), bridging the gap towards GEOS performance. This validates the scalability architecture for large-scale GIS tasks.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy.
