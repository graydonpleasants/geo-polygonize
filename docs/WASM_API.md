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

const resultStr = wasm.polygonizeWithOptions(
  JSON.stringify(geojson),
  cfbRobustOptions,
);
```

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
