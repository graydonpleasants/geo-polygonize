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

As of `geo-polygonize` v0.1.0 (with R-Tree noding, Edge memory optimization, and Bulk Loading):

### Grid Topology (Intersecting Lines)

| Input Size (NxN) | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 5 | ~0.001 | ~0.001 | ~0.79x |
| 10 | ~0.003 | ~0.004 | ~1.10x |
| 20 | ~0.015 | ~0.013 | ~0.92x |
| 50 | ~0.126 | ~0.085 | ~0.67x |
| 100 | ~0.945 | ~0.370 | ~0.39x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | ~0.016 | ~0.013 | ~0.85x |
| 100 | ~0.067 | ~0.042 | ~0.63x |
| 200 | ~0.295 | ~0.166 | ~0.56x |

**Analysis:**
The library performs competitively with GEOS.
- **Architecture:** The noding algorithm uses a robust iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy with parallel sorting to minimize memory allocations and hashing overhead.
- **Performance:** While GEOS (C++) remains ~2x faster for large inputs, `geo-polygonize` provides a pure Rust alternative with predictable scaling and memory safety. The performance gap is primarily due to the maturity of GEOS's optimized graph algorithms and C++ memory management.
