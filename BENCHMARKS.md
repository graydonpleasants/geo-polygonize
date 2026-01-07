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

## Comparative Results (Example)

As of `geo-polygonize` v0.1.0 (with R-Tree noding and Edge memory optimization):

### Grid Topology (Intersecting Lines)

| Input Size (NxN) | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 5 | ~0.001 | ~0.001 | ~1.06x |
| 10 | ~0.003 | ~0.004 | ~1.12x |
| 20 | ~0.014 | ~0.014 | ~1.00x |
| 50 | ~0.125 | ~0.087 | ~0.70x |
| 100 | ~0.862 | ~0.445 | ~0.52x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | ~0.011 | ~0.038 | ~3.59x |
| 100 | ~0.060 | ~0.092 | ~1.53x |
| 200 | ~0.274 | ~0.248 | ~0.91x |

**Analysis:**
The library performs competitively with GEOS.
- **Small/Medium Random Inputs:** `geo-polygonize` is often **faster** than Shapely/GEOS (up to 3.6x speedup) due to lower FFI/interpreter overhead and efficient batch noding.
- **Dense Grids:** GEOS remains faster (~2x) for very large, highly connected grids (100x100), likely due to highly optimized graph traversal and C++ memory management. However, the Rust implementation scales reasonably well ($O(N \log N)$) and is suitable for most workloads.
- **Optimization:** Recent improvements (R-Tree noding, reduced heap allocations in Graph Edges) have significantly closed the gap from the initial implementation.
