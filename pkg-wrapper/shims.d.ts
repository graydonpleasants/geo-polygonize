declare module '*.wasm' {
    const content: string;
    export default content;
}

// Override auto-generated WASM typings to use our auto-generated types
import { PolygonizerOptions } from './bindings/PolygonizerOptions';
import { WasmPolygonResult } from '../pkg-scalar/geo_polygonize.js';

export declare function polygonizeWithOptions(
    geojson_str: string,
    options_val: Partial<PolygonizerOptions>
): string;

/** Versioned full topology report; unlike `polygonizeWithOptions`, this retains non-polygon output. */
export declare function polygonizeReportWithOptions(
    geojson_str: string,
    options_val: Partial<PolygonizerOptions>
): string;

/** Versioned topology report plus a bounded physical-pipeline trace. */
export declare function polygonizeTraceWithOptions(
    geojson_str: string,
    options_val: Partial<PolygonizerOptions>,
    trace_level: 'summary' | 'noding' | 'graph' | 'rings' | 'full',
    byte_limit: number
): string;

export declare function polygonizeGeoArrowWithOptions(
    ipc_bytes: Uint8Array,
    options_val: Partial<PolygonizerOptions>
): Uint8Array;

export declare function polygonizeWithOptionsBuffer(
    coords: Float64Array,
    offsets: Uint32Array,
    stride: number,
    options_val: Partial<PolygonizerOptions>,
    line_ids?: Uint32Array | null
): WasmPolygonResult;
