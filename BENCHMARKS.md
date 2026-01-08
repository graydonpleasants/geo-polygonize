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
| 5 | ~0.001 | ~0.001 | ~1.04x |
| 10 | ~0.003 | ~0.004 | ~1.06x |
| 20 | ~0.015 | ~0.014 | ~0.92x |
| 50 | ~0.136 | ~0.093 | ~0.68x |
| 100 | ~1.127 | ~0.420 | ~0.37x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | ~0.011 | ~0.015 | ~1.39x |
| 100 | ~0.061 | ~0.045 | ~0.73x |
| 200 | ~0.278 | ~0.189 | ~0.68x |

**Analysis:**
The library performs competitively with GEOS.
- **Architecture:** The noding algorithm uses a robust parallel iterative R-Tree approach ($O(N \log N)$), and the graph construction uses a bulk-loading strategy with parallel sorting to minimize memory allocations and hashing overhead.
- **Performance:** While GEOS (C++) remains ~2-3x faster for very large grid inputs in this environment, `geo-polygonize` provides a pure Rust alternative with predictable scaling and memory safety. The parallel implementation significantly outperforms single-threaded versions.
