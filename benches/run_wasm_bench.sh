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
const { polygonize, load_geoarrow, setup_panic_hook } = require("./pkg/wasm_bench.js");
const { performance } = require("perf_hooks");

// Polyfill for browser performance API used in wasm-bindgen
global.window = {
    performance: performance
};

setup_panic_hook();

function generateGrid(size) {
    const lines = [];
    for (let i = 0; i <= size; i++) {
        // Vertical
        lines.push({
            type: "LineString",
            coordinates: [[i, 0], [i, size]]
        });
        // Horizontal
        lines.push({
            type: "LineString",
            coordinates: [[0, i], [size, i]]
        });
    }
    return lines;
}

const sizes = [10, 20, 50];

console.log("| Grid Size | Polygonize (ms) | GeoArrow Ingest+Iter (ms) |");
console.log("|---|---|---|");

for (const size of sizes) {
    const lines = generateGrid(size);

    // Warmup
    try {
        polygonize(lines);
        load_geoarrow(lines);
    } catch (e) {
        console.error("Warmup failed:", e);
    }

    let polyTotal = 0;
    let arrowTotal = 0;
    const runs = 5;

    for (let i = 0; i < runs; i++) {
        // Benchmark Polygonize
        const startPoly = performance.now();
        polygonize(lines);
        const endPoly = performance.now();
        polyTotal += (endPoly - startPoly);

        // Benchmark GeoArrow
        const startArrow = performance.now();
        load_geoarrow(lines);
        const endArrow = performance.now();
        arrowTotal += (endArrow - startArrow);
    }

    console.log(`| ${size}x${size} | ${(polyTotal / runs).toFixed(2)} | ${(arrowTotal / runs).toFixed(2)} |`);
}
'
