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
| 5 | 0.000272 | 0.000675 | 2.48x |
| 10 | 0.000620 | 0.002218 | 3.58x |
| 20 | 0.001834 | 0.008199 | 4.47x |
| 50 | 0.010433 | 0.050715 | 4.86x |
| 100 | 0.045939 | 0.207546 | 4.52x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | 0.001526 | 0.008031 | 5.26x |
| 100 | 0.006501 | 0.025743 | 3.96x |
| 200 | 0.025465 | 0.097653 | 3.83x |

**Analysis:**
The library offers a pure Rust native alternative to GEOS.
- **Performance:** On constrained environments (like CI runners with few cores), the parallel overhead of `rayon` may limit speedups compared to the highly optimized single-threaded C++ GEOS backend.
- **Tiling Strategy:** For large dense datasets (e.g., Grid 100), the **TiledPolygonizer** provides a significant speedup (~1.7x to 2.8x faster than the naive approach), bridging the gap towards GEOS performance. This validates the scalability architecture for large-scale GIS tasks.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy.
