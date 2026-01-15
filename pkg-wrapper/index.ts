import initScalar, * as scalarExports from "../pkg-scalar/geo_polygonize.js";
import initSimd, * as simdExports from "../pkg-simd/geo_polygonize.js";
import wasmScalarUrl from "../pkg-scalar/geo_polygonize_bg.wasm";
import wasmSimdUrl from "../pkg-simd/geo_polygonize_bg.wasm";

let simdSupported: boolean | undefined;

function hasSimd() {
    if (simdSupported !== undefined) return simdSupported;
    try {
        // Check if WebAssembly.validate validates a small SIMD module
        simdSupported = WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,123,3,2,1,0,10,10,1,8,0,65,0,253,15,253,98,11]));
    } catch (e) {
        simdSupported = false;
    }
    return simdSupported;
}

// We need to export all the functions from the library.
// Since the exports are identical for both, we can use the scalar exports type for TS.
export * from "../pkg-scalar/geo_polygonize.js";

// Override the init function
// input is ignored because we are using inlined Wasm
export default async function init(_input?: any) {
    if (hasSimd()) {
        await initSimd(wasmSimdUrl);
        return simdExports;
    } else {
        await initScalar(wasmScalarUrl);
        return scalarExports;
    }
}
