## 2024-05-24 - Zero-allocation Geometry Property Checks
**Learning:** Allocating intermediate representations like `geo_types::Polygon<f64>` just to compute geometric properties like `area()` in the Rust core is a significant performance bottleneck due to the cost of `Vec` allocations for exterior and interior rings. Direct math on the core structures (`Polygon3D`) using algorithms like the Shoelace formula avoids allocations entirely and is ~20x faster.
**Action:** Always prefer direct property calculations on `Polygon3D` and `Coord3D` slices. Only convert to `geo_types` structures if integrating with the `geo` crate's advanced algorithms (e.g., boolean ops) or for output.

## 2024-06-21 - Caching computation states across sequential geometry computations
**Learning:** Hot computational loops computing geometric properties iteratively over coordinates (e.g. area and centroid algorithms utilizing the Shoelace formula) shouldn't re-compute translated values or iteratively access struct properties when the math relies heavily on sequential coordinate pairs. Relying on iterators over explicitly index bounds-checking and aggressively re-using translated values (like `let p2_x = curr.x - origin.x;` becoming `p1_x` in the next iteration) yielded up to a ~20% performance boost for high-vertex-count polygons.
**Action:** When computing geometric properties sequentially over coordinates, prefer passing the active coordinate into the next iteration as `prev`, and aggressively cache translated values (`x - origin.x`) from the previous iteration to use in the subsequent to avoid redundant mathematical operations and struct field accesses.

## 2024-08-01 - Avoid origin translation caching on simple shoelace algorithms
**Learning:** Applying the "caching computation states" origin translation pattern to simple O(N) iterative geometry functions (like `ring_signed_area_2d` using the Shoelace formula) inadvertently degrades performance by introducing more arithmetic overhead than the basic algorithm.
**Action:** Always benchmark before applying translation caching patterns.

## 2024-08-01 - Vectorized memory initialization in structs
**Learning:** When optimizing Rust collection initialization from iterators (especially for padded structs like `SimdRing::new_3d`), pre-calculating the exact capacity using bitwise operations (e.g., `(len + 3) & !3`), populating via `.extend()` with mapped iterators, and using `.resize()` for padding is ~40% faster than dynamic bounds checking with `.push()` inside a loop.
**Action:** Always prefer explicit sizing, map extensions, and `resize` for array population when padding is required.
