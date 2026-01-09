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
| 5 | 0.000158 | 0.000679 | 4.30x |
| 10 | 0.000382 | 0.002262 | 5.93x |
| 20 | 0.001289 | 0.008210 | 6.37x |
| 50 | 0.007698 | 0.050776 | 6.60x |
| 100 | 0.040030 | 0.217363 | 5.43x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | 0.001009 | 0.007992 | 7.92x |
| 100 | 0.004164 | 0.025902 | 6.22x |
| 200 | 0.016103 | 0.102249 | 6.35x |

**Analysis:**
The library performs competitively with GEOS.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy with parallel spatial sorting (Z-Order) to minimize memory allocations and hashing overhead.
- **Performance:** While GEOS (C++) remains ~2x faster for very large grid inputs in this environment, `geo-polygonize` provides a pure Rust alternative with predictable scaling and memory safety. The parallel implementation significantly outperforms single-threaded versions.
