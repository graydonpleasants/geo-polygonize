#!/bin/bash
set -e

echo "Building Rust benchmarks..."
cargo build --bench polygonize_bench --release

echo "Running Rust benchmarks..."
cargo bench --bench polygonize_bench > rust_bench_output.txt

echo "Running Python benchmarks..."
python3 benches/bench_shapely.py > python_bench_output.txt

echo "Processing results..."
# Here I could write a python script to parse both output files and produce a combined table.
python3 benches/compare_results.py

echo "Done."
