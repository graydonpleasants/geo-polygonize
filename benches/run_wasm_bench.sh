#!/bin/bash
set -e

# Ensure wasm-pack is available
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack not found. Please install it."
    exit 1
fi

echo "Building Wasm Benchmark..."
cd benches/wasm_bench
wasm-pack build --target nodejs --release

echo "Running Wasm Benchmark (Node.js)..."
node -e '
const { BenchmarkContext, setup_panic_hook } = require("./pkg/wasm_bench.js");
const { performance } = require("perf_hooks");

setup_panic_hook();

// Warmup
{
    const ctx = BenchmarkContext.new(10);
    ctx.run();
    ctx.free();
}

const sizes = [10, 20, 50, 80];

console.log("| Input Size | Time (ms) |");
console.log("|---|---|");

for (const size of sizes) {
    let total = 0;
    const runs = 5;
    for (let i = 0; i < runs; i++) {
        const ctx = BenchmarkContext.new(size);
        const start = performance.now();
        ctx.run();
        const end = performance.now();
        total += (end - start);
        ctx.free();
    }
    const avg = total / runs;
    console.log(`| ${size} | ${avg.toFixed(2)} |`);
}
'
