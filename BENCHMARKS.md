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

As of `geo-polygonize` v0.1.0 with R-Tree optimizations:

### Grid Topology (Intersecting Lines)

| Input Size (NxN) | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 5 | ~0.001 | ~0.001 | ~0.97x |
| 10 | ~0.004 | ~0.004 | ~0.95x |
| 20 | ~0.014 | ~0.016 | ~1.13x |
| 50 | ~0.175 | ~0.087 | ~0.50x |
| 100 | ~0.852 | ~0.384 | ~0.45x |

### Random Lines

| Count | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |
|---|---|---|---|
| 50 | ~0.011 | ~0.015 | ~1.38x |
| 100 | ~0.063 | ~0.043 | ~0.69x |
| 200 | ~0.281 | ~0.181 | ~0.65x |

**Analysis:**
The improved noding algorithm uses an R-Tree to detect intersections, significantly improving performance from $O(N^2)$ to effectively $O(N \log N)$ (plus intersections). `geo-polygonize` is now comparable to GEOS for small inputs and scales reasonably well for larger inputs, though GEOS (C++) remains ~2x faster for very dense grids.
