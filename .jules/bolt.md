## 2024-05-24 - Zero-allocation Geometry Property Checks
**Learning:** Allocating intermediate representations like `geo_types::Polygon<f64>` just to compute geometric properties like `area()` in the Rust core is a significant performance bottleneck due to the cost of `Vec` allocations for exterior and interior rings. Direct math on the core structures (`Polygon3D`) using algorithms like the Shoelace formula avoids allocations entirely and is ~20x faster.
**Action:** Always prefer direct property calculations on `Polygon3D` and `Coord3D` slices. Only convert to `geo_types` structures if integrating with the `geo` crate's advanced algorithms (e.g., boolean ops) or for output.

## 2024-06-21 - Caching computation states across sequential geometry computations
**Learning:** Hot computational loops computing geometric properties iteratively over coordinates (e.g. area and centroid algorithms utilizing the Shoelace formula) shouldn't re-compute translated values or iteratively access struct properties when the math relies heavily on sequential coordinate pairs. Relying on iterators over explicitly index bounds-checking and aggressively re-using translated values (like `let p2_x = curr.x - origin.x;` becoming `p1_x` in the next iteration) yielded up to a ~20% performance boost for high-vertex-count polygons.
**Action:** When computing geometric properties sequentially over coordinates, prefer passing the active coordinate into the next iteration as `prev`, and aggressively cache translated values (`x - origin.x`) from the previous iteration to use in the subsequent to avoid redundant mathematical operations and struct field accesses.

## 2026-03-11 - Iterators Over Index Loops for Adjacent Elements
**Learning:** Using `.windows(2)` iterators over slices instead of explicit index-based loops (`for i in 0..slice.len() - 1`) inside deep nested algorithms (like `O(N*M)` topology checks in `rings_share_edge`) completely eliminates array bounds-checking overhead. This micro-optimization leads to measurable improvements in hot paths because the compiler can safely reason about sequential memory access.
**Action:** When comparing adjacent elements in hot loops, always prefer `.windows(2)` or `.array_windows::<2>()` (when stabilized) over index-based looping to guarantee no bounds checks and cleaner code.
## 2025-05-18 - Exterior Only Area Calculations
**Learning:** Calculating areas of complex polygons (with multiple holes) in hot computational paths like `polygonizer.rs` uses `unsigned_area_2d()` which unnecessarily loops over and subtracts all interior hole areas. For determining bounding box containers and strictly testing `area_j > area_i`, only the `exterior` shell bounds matter. Looping over holes adds overhead during large spatial queries.
**Action:** Created `exterior_unsigned_area_2d()` that only applies the Shoelace formula to the exterior ring of `Polygon3D`. Replace uses of `unsigned_area_2d()` with `exterior_unsigned_area_2d()` when performing structural comparisons that only rely on the outermost boundary of the geometry. This yields ~11-21% improvement in benchmarks for complex grid/random topologies.

## 2024-05-24 - [SIMD Array Initialization Overhead]
**Learning:** Initializing collection sizes via iterative `.push()` inside loops when the exact required capacity (including padding) is known incurs unnecessary dynamic bounds checking overhead. Using pre-calculated capacity bitwise operations like `(len + 3) & !3` with `.extend()` mapping over iterators and using `.resize()` for padding is significantly faster, especially for structs like `SimdRing` constructed repeatedly.
**Action:** When initializing padded array structures from collections of a known size, calculate the target aligned capacity upfront with bitwise operations, collect via mapping iterators, and pad using bulk `.resize()` instead of relying on a variable loop length and successive `.push()` calls.

## 2025-05-18 - Iterator Array Optimization
**Learning:** Using `.windows(2)` and explicitly tracked iterators (like tracking `prev` while looping `curr` over `slice.iter()`) provides a measurable performance improvement and avoids O(N) array bounds-checking overhead compared to explicit index looping like `for i in 0..slice.len()`.
**Action:** When tracking rings or adjacent items sequentially within an array, maintain references to the `prev` and `curr` values while looping `for curr in iter` rather than accessing `slice[(k-1) % len]` and `slice[k]`.

## 2025-03-16 - [Caching shoelace areas and avoiding square roots]
**Learning:** O(N) recalculations of `exterior_unsigned_area_2d()` inside the tree intersection loops is a major bottleneck. Additionally, inside `simd.rs` and `polygonizer.rs` hot loops, computing square roots via `.sqrt()` causes unnecessary CPU overhead.
**Action:** Pre-calculate `exterior_unsigned_area_2d` for all shells when initializing the `ContainmentForest` and store them in a vector `shell_areas: Vec<f64>`. Use squared comparisons (e.g. `tol_sq = eps * eps * a_len_sq`) to avoid costly `.sqrt()` mathematical computations.

## 2026-03-17 - Manual loop tracking and early returns
**Learning:** When refactoring index-based loops (`for i in 0..len` with modulo accesses) to use explicit variables like `prev`, `curr`, and `next` updated at each iteration, ensure that these tracking variables (`prev = curr; curr = next;`) are accurately maintained inside the loop structure, notably *before* any `continue` statements that short-circuit iteration. Failing to update state properly results in `unused_mut` warnings on loop variables and, more significantly, breaks algorithmic correctness.
**Action:** When tracking rings or sequences iteratively using explicit variable updates without macros/utilities like `windows`, carefully analyze all branching and early return (`continue`) paths to guarantee state updates occur everywhere necessary.

## 2026-03-16 - [Schwartzian Transform for Ring Sorting]
**Learning:** Performing expensive O(N) calculations like `ring_signed_area_2d` inside an O(K log K) sorting closure results in redundant computations (O(N * K log K)). Caching these values beforehand (Schwartzian Transform) reduces the complexity to O(N * K + K log K).
**Action:** When sorting geometric rings by area in `polygonizer.rs` (for holes or invalid rings), always pre-calculate and cache the areas in a temporary `Vec` of tuples before sorting to avoid redundant Shoelace formula evaluations in the comparison closure.

## 2026-03-22 - Parallel unzip initialization
**Learning:** Using `rayon`'s `.par_iter().map(|item| (val1, val2)).unzip()` is more efficient for initializing multiple parallel collections than mapping over the same data source twice or using manual loops. This minimizes allocation overhead and redundant mapping.
**Action:** When initializing multiple parallel collections from the same source array/collection, prefer `.unzip()` with mapped iterators to minimize manual allocation overhead and simplify logic.

## 2023-10-27 - Adaptive Regrid Optimization
**Learning:** Skewed spatial data can cause a `UniformGrid` single cell to be overwhelmed with segments, leading to O(N^2) explosion inside the cell.
**Action:** Implemented a bounded adaptive regrid loop in `UniformGrid::new` that halves the cell size up to 2 times if any single cell exceeds a threshold of 500 segments.

## 2024-05-24 - [Optimize Canonical Sorting]
**Learning:** In spatial sorts, repeatedly evaluating geometric properties like bounds and areas in `O(N log N)` `sort_by` comparators creates a hidden performance bottleneck.
**Action:** Always pre-calculate expensive sorting properties using a Schwartzian Transform pattern (mapping to a tuple with the cached data) and use `sort_unstable_by` for operations with deterministic tie-breaks.

## 2024-05-28 - [Eliminate Bounds Checks via Iterator Enumerate]
**Learning:** An index-based range iterator in Rust, like `(0..n).min_by(|i, j| slice[*i].cmp(&slice[*j]))`, requires the compiler to insert bounds-checking on each iteration because the closure receives raw index values that it uses to index the slice. This hurts performance in hot paths. Using `.iter().enumerate().min_by(...)` provides identical functionality, but eliminates the bounds-checking overhead while preserving the readability of iterator-chain methods, preventing the need to rewrite standard logic into lower-level manual `for` loops.
**Action:** To safely eliminate array bounds checks in Rust while preserving readability during a sequence scan (like finding the minimum index of a collection using `min_by`), prefer iterating over the slice directly (e.g., `slice.iter().enumerate().min_by(...)`) instead of using an index range `(0..n)` which forces the compiler to insert bounds checks. Never sacrifice readability by converting these to manual `for` loops just for micro-optimizations.