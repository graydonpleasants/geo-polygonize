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

As of `geo-polygonize` v0.1.0 (with Parallel R-Tree noding, Edge memory optimization, Spatial Sorting, and Bulk Loading):

### Grid Topology (Intersecting Lines)

| Input Size (NxN) | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 5 | ~0.0012 | ~0.0011 | ~0.96x |
| 10 | ~0.0036 | ~0.0037 | ~1.03x |
| 20 | ~0.0163 | ~0.0140 | ~0.86x |
| 50 | ~0.1292 | ~0.0924 | ~0.72x |
| 100 | ~0.8818 | ~0.4145 | ~0.47x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | ~0.0104 | ~0.0150 | ~1.44x |
| 100 | ~0.0606 | ~0.0454 | ~0.75x |
| 200 | ~0.2782 | ~0.1877 | ~0.67x |

**Analysis:**
The library performs competitively with GEOS.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy with parallel spatial sorting (Z-Order) to minimize memory allocations and hashing overhead.
- **Performance:** While GEOS (C++) remains ~2x faster for very large grid inputs in this environment, `geo-polygonize` provides a pure Rust alternative with predictable scaling and memory safety. The parallel implementation significantly outperforms single-threaded versions.
