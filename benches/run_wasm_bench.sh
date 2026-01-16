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
const { polygonize, polygonize_robust, load_geoarrow, setup_panic_hook } = require("./pkg/wasm_bench.js");
const { performance } = require("perf_hooks");

global.window = {
    performance: performance
};

setup_panic_hook();

function generateGrid(size) {
    const lines = [];
    for (let i = 0; i <= size; i++) {
        lines.push({
            type: "LineString",
            coordinates: [[i, 0], [i, size]]
        });
        lines.push({
            type: "LineString",
            coordinates: [[0, i], [size, i]]
        });
    }
    return lines;
}

function generateDirtyGrid(size) {
    const lines = [];
    for (let i = 0; i < size; i++) {
        for (let j = 0; j < size; j++) {
            // Bowtie pattern (X)
            lines.push({
                type: "LineString",
                coordinates: [[i, j], [i+1, j+1]]
            });
            lines.push({
                type: "LineString",
                coordinates: [[i+1, j], [i, j+1]]
            });
        }
    }
    return lines;
}

const sizes = [10, 20, 50];

console.log("| Grid Size | Polygonize (ms) | GeoArrow (ms) | Robust (Dirty) (ms) |");
console.log("|---|---|---|---|");

for (const size of sizes) {
    const cleanLines = generateGrid(size);
    const dirtyLines = generateDirtyGrid(size);

    // Warmup
    try {
        polygonize(cleanLines);
        polygonize_robust(dirtyLines, 1e-6);
        load_geoarrow(cleanLines);
    } catch (e) {
        console.error("Warmup failed:", e);
    }

    let polyTotal = 0;
    let arrowTotal = 0;
    let robustTotal = 0;
    const runs = 5;

    for (let i = 0; i < runs; i++) {
        let start = performance.now();
        polygonize(cleanLines);
        polyTotal += (performance.now() - start);

        start = performance.now();
        load_geoarrow(cleanLines);
        arrowTotal += (performance.now() - start);

        start = performance.now();
        polygonize_robust(dirtyLines, 1e-6);
        robustTotal += (performance.now() - start);
    }

    console.log(`| ${size}x${size} | ${(polyTotal / runs).toFixed(2)} | ${(arrowTotal / runs).toFixed(2)} | ${(robustTotal / runs).toFixed(2)} |`);
}
'
