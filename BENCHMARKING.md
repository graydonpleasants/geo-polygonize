# Benchmarking Guide

This guide describes how to run performance benchmarks for `geo-polygonize` in both Native (Rust) and WebAssembly (Node.js) environments.

## Native Benchmarks

Native benchmarks use [Criterion.rs](https://bheisler.github.io/criterion.rs/) to measure the performance of the polygonization algorithm with different datasets (Grid, Random).

### Prerequisites

- Rust Toolchain (`cargo`)
- Python 3 (optional, for comparing with Shapely/GEOS)

### Running Native Benchmarks

To run the standard Rust benchmarks:

```bash
cargo bench --bench polygonize_bench
```

This will compile and run the benchmarks defined in `benches/polygonize_bench.rs`. The output will show the average time per iteration for various input sizes.

### Containment Benchmarks

Run the isolated prepared-ring, shell-filtering, hole-assignment, and end-to-end containment matrix with:

```bash
cargo bench -p geo-polygonize-core --bench hole_sort_bench
```

To compare a change against a saved Criterion baseline:

```bash
cargo bench -p geo-polygonize-core --bench hole_sort_bench -- --save-baseline before
# edit the implementation
cargo bench -p geo-polygonize-core --bench hole_sort_bench -- --baseline before
```

Use `--quick` while iterating; omit it for decision-quality results.

### Comparing with Shapely (Python)

To run a comparison between the Rust implementation and Python's `shapely.ops.polygonize`:

```bash
./benches/run_comparison.sh
```

This script:
1. Builds and runs the Rust benchmarks.
2. Runs the Python equivalent (`benches/bench_shapely.py`).
3. (Ideally) Processes/compares the output (currently it runs them sequentially).

## WebAssembly Benchmarks

Wasm benchmarks measure the performance of the library when compiled to WebAssembly and executed in a Node.js environment. This is critical for assessing the overhead of `wasm-bindgen` and allocator performance (`talc`).

### Prerequisites

- `wasm-pack`: Install via `cargo install wasm-pack`
- Node.js

### Running Wasm Benchmarks

To run the Wasm benchmarks:

```bash
./benches/run_wasm_bench.sh
```

This script:
1. Builds the `benches/wasm_bench` crate targeting `nodejs`.
2. Executes an inline Node.js script that:
   - Generates test data (Clean Grid, Dirty Grid/Bowtie).
   - Runs `polygonize` (Standard).
   - Runs `load_geoarrow` (Ingestion benchmark).
   - Runs `polygonize_robust` (Noding enabled).
   - Outputs a Markdown table with the results.

### Benchmark Scenarios

- **Grid**: A regular grid of lines. Tests the standard polygonization assembly speed.
- **Dirty Grid (Bowtie)**: A grid where every cell contains a bowtie pattern (crossing lines). This scenario guarantees numerous intersections and is used to stress-test the robustness and performance of the `SnapNoder`.
- **GeoArrow**: Benchmarks the ingestion of LineStrings into a GeoArrow memory layout.

## Profiling

For native profiling, you can use `flamegraph`:

```bash
cargo install flamegraph
cargo flamegraph --bench polygonize_bench
```
