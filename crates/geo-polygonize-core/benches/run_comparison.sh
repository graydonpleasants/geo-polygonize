#!/bin/bash
set -e

FAST_CI=0
if [[ "$1" == "--fast-ci" ]]; then
    FAST_CI=1
    echo "Running in fast CI mode..."
fi

echo "Building Rust benchmarks..."
cargo build -p geo-polygonize-core --bench polygonize_bench --release

echo "Running Rust benchmarks..."
if [[ "$FAST_CI" -eq 1 ]]; then
    BENCH_FAST_CI=1 cargo bench -p geo-polygonize-core --bench polygonize_bench -- polygonize/grid/20 > rust_bench_output.txt
else
    cargo bench -p geo-polygonize-core --bench polygonize_bench > rust_bench_output.txt
fi

echo "Running Python benchmarks..."
if [[ "$FAST_CI" -eq 1 ]]; then
    python3 crates/geo-polygonize-core/benches/bench_shapely.py --fast-ci > python_bench_output.txt
else
    python3 crates/geo-polygonize-core/benches/bench_shapely.py > python_bench_output.txt
fi

echo "Running Wasm benchmarks..."
if [[ "$FAST_CI" -eq 1 ]]; then
    echo "Skipped in fast CI mode." > wasm_bench_output.txt
else
    bash crates/geo-polygonize-core/benches/run_wasm_bench.sh > wasm_bench_output.txt
fi

echo "Processing results..."
# Here I could write a python script to parse both output files and produce a combined table.
python3 crates/geo-polygonize-core/benches/compare_results.py

echo "Done."
