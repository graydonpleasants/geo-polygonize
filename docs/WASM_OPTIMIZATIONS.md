# Wasm Optimization Strategy

This document outlines potential strategies to further improve WebAssembly (Wasm) performance for `geo-polygonize` while maintaining or improving Native performance.

## 1. Monotone Chain Sweep Line (Bentley-Ottmann)

The current implementation uses an R-Tree for noding (finding intersections). While robust and parallelizable (good for Native), it is memory-intensive and O(N^2) in worst-case dense grids.

*   **Pros:**
    *   **Algorithmic Efficiency:** O((N + k) log N) complexity is superior for dense intersections.
    *   **Memory Footprint:** Significantly lower memory usage than constructing an R-Tree, crucial for Wasm's 4GB limit (and practical browser limits).
*   **Cons:**
    *   **Robustness:** Extremely difficult to implement robustly with floating-point arithmetic compared to the "find all, then split" R-Tree approach.
    *   **Parallelism:** Inherently sequential algorithm. Replacing the parallel R-Tree with this would **degrade Native performance** on multi-core systems unless we maintain two separate implementations (high maintenance).

## 2. Shared Memory Parallelism (Rayon on Wasm)

Enable threads in Wasm using `SharedArrayBuffer` and `wasm-bindgen-rayon`.

*   **Pros:**
    *   **Speed:** Directly utilizes multi-core CPUs in the browser, potentially bringing Wasm parity with Native.
*   **Cons:**
    *   **Deployment Complexity:** Requires the hosting server to send specific headers (`Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp`). This breaks many standard deployments (e.g., simple CDNs, iframes).
    *   **Browser Support:** Good but not universal (e.g., Safari restrictions).
    *   **Overhead:** Thread startup in Wasm is heavier than Native.

## 3. Geometry Quantization (Int32 Coordinates)

Convert all `f64` coordinates to `i32` (fixed precision) before processing.

*   **Pros:**
    *   **Math Speed:** Integer arithmetic is faster and exact.
    *   **Size:** Reduces memory bandwidth (4 bytes vs 8 bytes per coord).
*   **Cons:**
    *   **Precision Loss:** Coordinates are snapped to a grid.
    *   **Conversion Cost:** Overhead of converting to/from float at boundaries.
    *   **API Breaking:** Changes the public API or requires a wrapper.

## 4. Arena / Bump Allocation

Use a custom allocator (like `bumpalo`) for the graph nodes and edges instead of `Vec` or standard heap.

*   **Pros:**
    *   **Allocation Speed:** Bump allocation is effectively instantaneous.
    *   **Cache Locality:** Related data is stored contiguously.
*   **Cons:**
    *   **Memory Peaks:** Memory cannot be freed individually; it grows until the entire operation finishes. This increases the risk of OOM on Wasm for large datasets, even if it's faster.

## 5. `wasm-opt` Tuning (Toolchain)

Use `binaryen`'s `wasm-opt` to optimize the final binary.

*   **Pros:**
    *   **Free Performance:** 10-20% size reduction and speedup without code changes.
*   **Cons:**
    *   **Build Time:** Adds to CI/CD pipeline time.
    *   **Debugging:** Makes stack traces harder to read (though DWARF helps).

## Recommendation

For the immediate future, we should stick to **Algorithmic Improvements** that benefit both targets (like the Tiled Polygonizer for large grids) and **Memory Optimizations** (like the lazy streaming implemented in this PR). Switching to a Sweep Line algorithm is the high-risk/high-reward option if memory constraints become the primary blocker.
