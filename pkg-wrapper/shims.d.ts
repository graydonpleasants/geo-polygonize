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
