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

As of `geo-polygonize` v0.1.0 (with Parallel R-Tree noding, Edge memory optimization, and Bulk Loading):

### Grid Topology (Intersecting Lines)

| Input Size (NxN) | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 5 | ~0.001 | ~0.001 | ~1.03x |
| 10 | ~0.003 | ~0.004 | ~1.08x |
| 20 | ~0.014 | ~0.014 | ~1.01x |
| 50 | ~0.123 | ~0.085 | ~0.70x |
| 100 | ~0.854 | ~0.379 | ~0.44x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | ~0.010 | ~0.013 | ~1.35x |
| 100 | ~0.063 | ~0.044 | ~0.71x |
| 200 | ~0.277 | ~0.173 | ~0.62x |

**Analysis:**
The library performs competitively with GEOS.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy with parallel sorting to minimize memory allocations and hashing overhead.
- **Performance:** While GEOS (C++) remains ~2x faster for large inputs, `geo-polygonize` provides a pure Rust alternative with predictable scaling and memory safety. The performance gap is primarily due to the maturity of GEOS's optimized graph algorithms and C++ memory management.
