/* tslint:disable */
/* eslint-disable */

export class WasmPolygonResult {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  coords_len(): number;
  coords_ptr(): number;
  ring_offsets_len(): number;
  ring_offsets_ptr(): number;
  polygon_offsets_len(): number;
  polygon_offsets_ptr(): number;
  stride(): number;
}

export function polygonize(geojson_str: string): string;

export function polygonize_buffers(coords: Float64Array, offsets: Uint32Array, stride: number, node_input: boolean, snap_grid_size: number): WasmPolygonResult;

export function polygonize_geoarrow(ipc_bytes: Uint8Array, node_input: boolean, snap_grid_size: number, extract_only_polygonal: boolean): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wasmpolygonresult_free: (a: number, b: number) => void;
  readonly polygonize: (a: number, b: number) => [number, number, number, number];
  readonly polygonize_buffers: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
  readonly polygonize_geoarrow: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
  readonly wasmpolygonresult_coords_len: (a: number) => number;
  readonly wasmpolygonresult_coords_ptr: (a: number) => number;
  readonly wasmpolygonresult_polygon_offsets_len: (a: number) => number;
  readonly wasmpolygonresult_polygon_offsets_ptr: (a: number) => number;
  readonly wasmpolygonresult_ring_offsets_len: (a: number) => number;
  readonly wasmpolygonresult_ring_offsets_ptr: (a: number) => number;
  readonly wasmpolygonresult_stride: (a: number) => number;
  readonly polygonize_ffi: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
