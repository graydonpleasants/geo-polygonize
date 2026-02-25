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
}

export function initThreadPool(num_threads: number): Promise<any>;

export function polygonize(geojson_str: string): string;

export function polygonize_buffers(coords: Float64Array, offsets: Uint32Array, node_input: boolean, snap_grid_size: number): WasmPolygonResult;

export class wbg_rayon_PoolBuilder {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  numThreads(): number;
  build(): void;
  receiver(): number;
}

export function wbg_rayon_start_worker(receiver: number): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wasmpolygonresult_free: (a: number, b: number) => void;
  readonly polygonize: (a: number, b: number) => [number, number, number, number];
  readonly polygonize_buffers: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
  readonly wasmpolygonresult_coords_len: (a: number) => number;
  readonly wasmpolygonresult_coords_ptr: (a: number) => number;
  readonly wasmpolygonresult_polygon_offsets_len: (a: number) => number;
  readonly wasmpolygonresult_polygon_offsets_ptr: (a: number) => number;
  readonly wasmpolygonresult_ring_offsets_len: (a: number) => number;
  readonly wasmpolygonresult_ring_offsets_ptr: (a: number) => number;
  readonly polygonize_ffi: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly polygonize_result_free: (a: number) => void;
  readonly polygonize_result_get_count: (a: number) => number;
  readonly polygonize_result_get_hole_count: (a: number, b: number) => number;
  readonly polygonize_result_get_hole_point_count: (a: number, b: number, c: number) => number;
  readonly polygonize_result_get_hole_points: (a: number, b: number, c: number, d: number) => void;
  readonly polygonize_result_get_shell_point_count: (a: number, b: number) => number;
  readonly polygonize_result_get_shell_points: (a: number, b: number, c: number) => void;
  readonly polygonize_result_get_status: (a: number) => number;
  readonly __wbg_wbg_rayon_poolbuilder_free: (a: number, b: number) => void;
  readonly initThreadPool: (a: number) => any;
  readonly wbg_rayon_poolbuilder_build: (a: number) => void;
  readonly wbg_rayon_poolbuilder_numThreads: (a: number) => number;
  readonly wbg_rayon_poolbuilder_receiver: (a: number) => number;
  readonly wbg_rayon_start_worker: (a: number) => void;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
