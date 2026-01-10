# Architectural Optimization Roadmap

Based on "Architectural Optimization Strategies for High-Throughput Computational Geometry in WebAssembly".

## 1. Advanced Performance Profiling and Instrumentation
- [x] **Decoupled Instrumentation**: Move timing logic to JavaScript host to avoid observer effect.
    - *Status*: Implemented in `benches/wasm_bench` using Node.js `performance.now()` and a `BenchmarkContext` pattern.
- [ ] **Puppeteer Orchestration**: Automate browser-based benchmarking to capture real-world DOM/GC behavior.
    - *Status*: Pending. Currently using Node.js for benchmarks.

## 2. Memory Architecture
- [x] **Allocator Optimization**: Integrate `talc` allocator for high-throughput Wasm environments.
    - *Status*: Implemented in `benches/wasm_bench` via `#[global_allocator]`.
- [ ] **Pre-allocation Strategy**: Implement memory estimation to prevent `memory.grow` stutter.
    - *Status*: Pending.

## 3. Zero-Copy Data Architectures
- [ ] **GeoArrow Integration**: Adopt columnar memory layout for zero-copy data transfer.
    - *Status*: Pending.
- [ ] **Shared Memory Views**: Implement `Float64Array` views for raw coordinate access.
    - *Status*: Pending.

## 4. Computational Geometry Robustness
- [x] **Robust Predicates**: Integrate `robust` crate for exact geometric predicates.
    - *Status*: Implemented robust angular sorting for graph edges, removing unstable floating-point `pseudo_angle`.
- [ ] **Iterated Snap Rounding**: Implement snap rounding for robust noding.
    - *Status*: Pending.

## 5. Hardware Acceleration (SIMD)
- [ ] **SIMD Targets**: Enable `+simd128` and optimize hot loops.
    - *Status*: Pending.
- [ ] **Vector Libraries**: Evaluate `glam` or `simsimd` for distance/intersection checks.
    - *Status*: Pending.

## 6. Spatial Indexing
- [ ] **Static Packed R-Trees**: Optimization for read-heavy workloads using `geo-index` (flatbush port).
    - *Status*: Pending.
