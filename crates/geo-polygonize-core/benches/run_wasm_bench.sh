#!/bin/bash
set -e

# Ensure wasm-pack is available
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack not found. Please install it."
    return 1 2>/dev/null || exit 1
fi

echo "Building Wasm Benchmark..."
cd "$(dirname "$0")/wasm_bench"
wasm-pack build --target nodejs --release

echo "Running Wasm Benchmark (Node.js)..."
node -e '
const { polygonize, polygonize_tiled, polygonize_random, polygonize_robust, bowtie_noder_auto, bowtie_noder_force_grid, bowtie_noder_force_simd, load_geoarrow, setup_panic_hook, get_edge_rings, get_edge_rings_with_dangles } = require("./pkg/wasm_bench.js");
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

// Implement a PRNG to replace Math.random() for reproducibility
function mulberry32(a) {
    return function() {
      var t = a += 0x6D2B79F5;
      t = Math.imul(t ^ t >>> 15, t | 1);
      t ^= t + Math.imul(t ^ t >>> 7, t | 61);
      return ((t ^ t >>> 14) >>> 0) / 4294967296;
    }
}

function generateRandomLines(count, seed) {
    const rng = mulberry32(seed);
    const lines = [];
    for (let i = 0; i < count; i++) {
        const x1 = rng() * 100.0;
        const y1 = rng() * 100.0;
        const x2 = rng() * 100.0;
        const y2 = rng() * 100.0;
        lines.push({
            type: "LineString",
            coordinates: [[x1, y1], [x2, y2]]
        });
    }
    return lines;
}

function generateParallelLines(n) {
    const lines = [];
    for (let i = 0; i < n; i++) {
        lines.push({
            type: "LineString",
            coordinates: [[0.0, i], [10.0, i]]
        });
    }
    return lines;
}

const sizes = [5, 10, 20, 50, 100];

console.log("=== Grid Benchmark ===");
console.log("| Grid Size | Polygonize (ms) | Polygonize Tiled (ms) | GeoArrow (ms) |");
console.log("|---|---|---|---|");

for (const size of sizes) {
    const cleanLines = generateGrid(size);

    let polyTotal = 0;
    let polyTiledTotal = 0;
    let arrowTotal = 0;
    const runs = 5;

    for (let i = 0; i < runs; i++) {
        let start = performance.now();
        polygonize(cleanLines);
        polyTotal += (performance.now() - start);

        if (size >= 50) {
            start = performance.now();
            polygonize_tiled(cleanLines, size);
            polyTiledTotal += (performance.now() - start);
        }

        start = performance.now();
        load_geoarrow(cleanLines);
        arrowTotal += (performance.now() - start);
    }

    const polyTiledStr = size >= 50 ? (polyTiledTotal / runs).toFixed(2) : "-";
    console.log(`| ${size}x${size} | ${(polyTotal / runs).toFixed(2)} | ${polyTiledStr} | ${(arrowTotal / runs).toFixed(2)} |`);
}

const dirtySizes = [10, 20, 50];

console.log("");
console.log("=== Bowtie Grid Benchmark ===");
console.log("| Grid Size | Robust (Dirty) Auto (ms) | Robust (Dirty) Force Grid (ms) | Robust (Dirty) Force SIMD (ms) |");
console.log("|---|---|---|---|");

for (const size of dirtySizes) {
    const dirtyLines = generateDirtyGrid(size);

    let robustTotal = 0;
    let gridTotal = 0;
    let simdTotal = 0;
    const runs = 5;

    for (let i = 0; i < runs; i++) {
        let start = performance.now();
        bowtie_noder_auto(dirtyLines);
        robustTotal += (performance.now() - start);

        start = performance.now();
        bowtie_noder_force_grid(dirtyLines);
        gridTotal += (performance.now() - start);

        if (size <= 20) {
            start = performance.now();
            bowtie_noder_force_simd(dirtyLines);
            simdTotal += (performance.now() - start);
        }
    }

    const simdStr = size <= 20 ? (simdTotal / runs).toFixed(2) : "-";
    console.log(`| ${size}x${size} | ${(robustTotal / runs).toFixed(2)} | ${(gridTotal / runs).toFixed(2)} | ${simdStr} |`);
}

const randomCounts = [50, 100, 200];

console.log("");
console.log("=== Random Benchmark ===");
console.log("| Random Count | Polygonize (ms) |");
console.log("|---|---|");

for (const count of randomCounts) {
    const randomLines = generateRandomLines(count, 42);

    let polyTotal = 0;
    const runs = 5;

    for (let i = 0; i < runs; i++) {
        let start = performance.now();
        polygonize_random(randomLines);
        polyTotal += (performance.now() - start);
    }

    console.log(`| ${count} | ${(polyTotal / runs).toFixed(2)} |`);
}

console.log("");
console.log("=== Large Parallel Benchmark ===");
console.log("| Count | Bowtie Noder (ms) |");
console.log("|---|---|");
const parallelLines = generateParallelLines(10000);
let parallelTotal = 0;
for (let i = 0; i < 5; i++) {
    let start = performance.now();
    bowtie_noder_force_grid(parallelLines);
    parallelTotal += (performance.now() - start);
}
console.log(`| 10000 | ${(parallelTotal / 5).toFixed(2)} |`);

console.log("");
console.log("=== Planar Graph Benchmark ===");
console.log("| Grid Size | Get Edge Rings (ms) |");
console.log("|---|---|");
const graphGridLines = generateGrid(50);
let ringsTotal = 0;
for (let i = 0; i < 5; i++) {
    let start = performance.now();
    get_edge_rings(graphGridLines);
    ringsTotal += (performance.now() - start);
}
console.log(`| 50 | ${(ringsTotal / 5).toFixed(2)} |`);


console.log("");
console.log("=== Planar Graph Dangles Benchmark ===");
console.log("| Count | Get Edge Rings (ms) |");
console.log("|---|---|");
const graphDanglesLines = generateRandomLines(500, 12345);
let danglesTotal = 0;
for (let i = 0; i < 5; i++) {
    let start = performance.now();
    get_edge_rings_with_dangles(graphDanglesLines);
    danglesTotal += (performance.now() - start);
}
console.log(`| 500 | ${(danglesTotal / 5).toFixed(2)} |`);
'