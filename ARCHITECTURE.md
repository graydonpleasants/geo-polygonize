# Architecture & Optimizations

## 1. Robustness: Iterated Snap Rounding (ISR)
To handle "dirty" input geometries (self-intersections, overlaps, floating-point inconsistencies), the engine implements **Iterated Snap Rounding**.
-   **Module:** `src/noding/snap.rs`
-   **Algorithm:**
    1.  Snap all vertices to a configurable grid (default `1e-10`).
    2.  Find intersections using an R-Tree (`rstar`).
    3.  Split segments at intersections and snap split points.
    4.  Repeat until no new intersections are found (topology stabilizes).
-   **Usage:** Enable `node_input = true` on `Polygonizer`.

## 2. Spatial Indexing
-   **Dynamic R-Tree (`rstar`):** Used for both graph construction (noding) and hole-to-shell assignment.
    -   *Note:* A static packed R-Tree approach (`geo-index`) was evaluated but reverted due to Wasm alignment issues. The dynamic R-Tree provides sufficient performance for current workloads.

## 3. Hardware Acceleration: SIMD
Critical geometric predicates are accelerated using Single Instruction, Multiple Data (SIMD) instructions via the `wide` crate (targeting `wasm32` SIMD128 and native AVX/SSE).
-   **Ray Casting:** The `SimdRing` struct (`src/utils/simd.rs`) processes 4 segments in parallel to determine Point-in-Polygon inclusion.
-   **Impact:** Significantly reduces the cost of the "Hole Assignment" phase, which is O(N*M) in the worst case (checking every hole against every shell candidate).

## 4. Memory Management
-   **Allocator:** Uses `talc` for Wasm targets to improve allocation throughput for small, short-lived objects (nodes, edges).
-   **Data Layout:** `PlanarGraph` uses Structure of Arrays (SoA) for node coordinates to improve cache coherence.
