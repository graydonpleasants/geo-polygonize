# Wasm API Documentation

This document describes the WebAssembly (Wasm) API available in the `geo-polygonize` package.

## Exported Functions

### `polygonize(geojson_str, node_input?, snap_grid_size?, extract_only_polygonal?)`

Polygonizes linework provided as a GeoJSON FeatureCollection, Feature, or Geometry string. Returns a GeoJSON FeatureCollection string containing the resulting polygons.

*   `geojson_str` (string): The input GeoJSON as a string.
*   `node_input` (boolean, optional): Whether to use unchecked iterative grid noding to find intersections between line segments before polygonization. Defaults to `false`.
*   `snap_grid_size` (number, optional): Compatibility shorthand used when noding is enabled. Zero selects floating precision; a positive value selects that fixed grid. Omission retains the legacy `1e-10` grid. It is ignored when noding is disabled.
*   `extract_only_polygonal` (boolean, optional): Whether to strictly extract only fully polygonal regions, discarding non-polygonal linework. Defaults to `false`.

**Example:**

```javascript
import init, { polygonize } from "geo-polygonize";

const geojson = {
    type: "FeatureCollection",
    features: [
        {
            type: "Feature",
            geometry: {
                type: "LineString",
                coordinates: [[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]
            }
        }
    ]
};

await init();

// With explicit parameters to match backend configurations
const resultStr = polygonize(JSON.stringify(geojson), true, 0.5);
const result = JSON.parse(resultStr);
```

For production app bundles, prefer `geo-polygonize/slim` with explicit Wasm
asset URLs and the versioned CFB profile:

```ts
import { cfbRobustOptions, initBest } from "geo-polygonize/slim";
import scalarUrl from "geo-polygonize/geo_polygonize.wasm?url";
import simdUrl from "geo-polygonize/geo_polygonize_simd.wasm?url";

const wasm = await initBest(
  { module_or_path: scalarUrl },
  { module_or_path: simdUrl },
);

// This compatibility API returns polygons only.
const resultStr = wasm.polygonizeWithOptions(
  JSON.stringify(geojson),
  cfbRobustOptions,
);
```

### `polygonizeTraceWithOptions(geojson, options, level, byteLimit)`

Returns a JSON string containing the canonical topology report and the physical
pipeline's versioned trace. Trace capture is opt-in and bounded by `byteLimit`;
the response reports exact bytes used and whether capture was truncated.

* `level`: `summary`, `noding`, `graph`, `rings`, or `full`.
* `byteLimit`: an integer from `0` through `u32::MAX`. This is an operational
  trace budget and is not part of `PolygonizerOptions`.

```ts
const traced = JSON.parse(wasm.polygonizeTraceWithOptions(
  JSON.stringify(geojson),
  cfbRobustOptions,
  "noding",
  1_000_000,
));

console.log(traced.topology, traced.trace.events, traced.trace.truncated);
```

Use `polygonizeTraceWithOptionsAsync` when the call must be abortable. It uses
the same scalar/SIMD runtime selection as direct calls and terminates its worker
when the supplied `AbortSignal` fires.

### `polygonize_buffers(coords, offsets, stride, node_input, snap_grid_size)`

Polygonizes raw coordinate arrays. This is an advanced API for high-performance integrations bypassing JSON serialization.

*   `coords` (Float64Array): A flat array of coordinate values `[x1, y1, x2, y2, ...]`.
*   `offsets` (Uint32Array): Start indices of each line segment in `coords` (measured in coordinate points, not flat floats).
*   `stride` (number): Coordinate stride (2 for 2D, 3 for 3D). Must be `2` or `3`.
*   `node_input` (boolean): Whether to perform node noding on the inputs.
*   `snap_grid_size` (number): Compatibility shorthand used when noding is enabled: zero is floating and a positive value is fixed-grid. It is ignored otherwise.

Returns a `WasmPolygonResult` object (see below).

### `polygonize_geoarrow(ipc_bytes, node_input, snap_grid_size, extract_only_polygonal)`

Polygonizes data provided as an Arrow IPC byte array representing a GeoArrow LineString column. Returns an Arrow IPC byte array of the resulting polygons.

*   `ipc_bytes` (Uint8Array): The input Arrow IPC byte buffer.
*   `node_input` (boolean): Whether to perform node noding on the inputs.
*   `snap_grid_size` (number): Compatibility shorthand used when noding is enabled: zero is floating and a positive value is fixed-grid. It is ignored otherwise.
*   `extract_only_polygonal` (boolean): Whether to extract only polygonal structures.

## WasmPolygonResult Object

Returned by `polygonize_buffers`.

Methods:

*   `coords_ptr()`: Pointer to the flat output coordinates array in Wasm memory.
*   `coords_len()`: Length of the coordinates array.
*   `ring_offsets_ptr()`: Pointer to the ring offsets array.
*   `ring_offsets_len()`: Length of the ring offsets array.
*   `polygon_offsets_ptr()`: Pointer to the polygon offsets array.
*   `polygon_offsets_len()`: Length of the polygon offsets array.
*   `stride()`: The stride of the output coordinates.

You can construct standard JavaScript `Float64Array` and `Uint32Array` views over the Wasm memory using these pointers and lengths to access the raw data with zero-copy overhead.

## Managed packed results

Prefer the managed facade for application code:

```ts
import init, {
  packedSnapshotTransferList,
  polygonizePackedGeometryWithOptions,
  polygonizePackedWithOptions,
} from "geo-polygonize";

await init();
const result = polygonizePackedGeometryWithOptions(
  coordinates,
  lineStarts,
  2,
  options,
  lineIds,
);

try {
  const polygonCount = result.withBorrowed(({ polygonOffsets }) =>
    polygonOffsets.length
  );
  const snapshot = result.snapshot();
  worker.postMessage(snapshot, packedSnapshotTransferList(snapshot));
} finally {
  result.dispose();
}

const full = polygonizePackedWithOptions(coordinates, lineStarts, 2, options, lineIds);
try {
  consumeCanonicalReport(full.fullReport?.topologyFingerprint);
  const geojson = full.toGeoJSON();
} finally {
  full.dispose();
}
```

`polygonizePackedWithOptions` retains the same exact fingerprint and report
fields as `polygonizeWithOptionsBuffer`. The explicitly lossy
`polygonizePackedGeometryWithOptions` retains only the packed polygon geometry,
representative line IDs, stride, and boundary metrics. Projection selection is
not a polygonizer option and does not change core semantics.

The `geo-polygonize-packed-v1` layout is:

- `coordinates`: flat `Float64Array`; `stride` is exactly 2 or 3. Values are in
  the input coordinate system.
- `ringOffsets`: start-only `Uint32Array` measured in coordinate tuples, not
  float elements. A ring ends at the next ring start or
  `coordinates.length / stride` for the final ring.
- `polygonOffsets`: start-only `Uint32Array` measured in rings. A polygon ends
  at the next polygon start or `ringOffsets.length` for the final polygon.
- `representativeLineIds`: one exact `u32` value per coordinate tuple. These
  are representative edge IDs, not complete provenance; full provenance is
  available only on the full projection. The preserved raw layout uses `0`
  when an output coordinate, commonly the repeated closure coordinate, has no
  corresponding representative entry.

The first ring of each polygon is its exterior and remaining rings are holes.
Ordering and ring closure are exactly the core result: closed rings repeat the
first coordinate as the final coordinate. Empty output has four empty arrays.
No terminal sentinel is appended to either offsets array.

`withBorrowed` acquires all views together and invokes its callback
synchronously. It performs no coordinate copies, but the view objects
themselves are allocations. Do not retain them across callback return, `await`,
disposal, or any direct/re-entrant Wasm operation that may grow memory. Views
are read-only by contract; writes do not edit engine topology, reports, or
provenance. Each later borrow reacquires against the current memory buffer.

`snapshot` copies all four arrays into independent JavaScript-owned buffers and
includes `layout`, `stride`, and `projection`. It survives result disposal and
later Wasm calls and is the only supported worker-transfer representation.
Never transfer Wasm linear memory. `dispose`, `free`, and `Symbol.dispose` are
idempotent; all other managed-handle access after disposal throws.

Input arrays still cross the generated `wasm-bindgen` slice ABI by copy, then
Rust constructs owned linework. Packed output removes GeoJSON serialization and
nested-coordinate allocation; it is not a zero-copy input API. See
[the design note](./wasm-packed-results-design.md) for the complete boundary.
