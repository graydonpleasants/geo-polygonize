# WASM API Reference

Auto-generated reference for the `geo-polygonize` WebAssembly bindings.

## `polygonize_with_options_js`

Polygonizes a GeoJSON FeatureCollection using the canonical `PolygonizerOptions`.

This is the primary entry point for JavaScript users. It accepts a JSON string
of options and returns a JSON string representing the result, including faces,
dangles, cut-lines, and (optionally) provenance/diagnostics.

### Signature

```typescript
function polygonize_with_options_js(geojson_str: string, options_val: any): any
```

## `polygonize`

### Signature

```typescript
function polygonize(geojson_str: string, node_input: boolean, snap_grid_size: number, extract_only_polygonal: boolean, report_mode: boolean): any
```

## `polygonize_with_options_buffer_js`

### Signature

```typescript
function polygonize_with_options_buffer_js(coords: number, offsets: number, stride: number, options_val: any, line_ids: number): any
```

## `polygonize_buffers`

### Signature

```typescript
function polygonize_buffers(coords: number, offsets: number, stride: number, node_input: boolean, snap_grid_size: number, line_ids: number): any
```

## `polygonize_geoarrow_with_options_js`

Polygonizes an Arrow IPC stream containing a GeoArrow LineString array.

This zero-copy path avoids JSON serialization overhead and returns a binary
Arrow IPC stream containing a GeoArrow Polygon array. Requires the options
to be passed as a parsed JS object.

### Signature

```typescript
function polygonize_geoarrow_with_options_js(ipc_bytes: number, options_val: any): any
```

## `polygonize_geoarrow`

### Signature

```typescript
function polygonize_geoarrow(ipc_bytes: number, node_input: boolean, snap_grid_size: number, extract_only_polygonal: boolean): any
```

