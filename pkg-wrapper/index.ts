import init, * as exports from "../pkg-scalar/geo_polygonize.js";
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

// We re-export everything from the scalar package.
// The JS bindings in pkg-scalar/geo_polygonize.js are identical to pkg-simd/geo_polygonize.js
// because the exported API is the same.
// By calling init() with the correct Wasm binary, these exported functions will use that binary.
export * from "../pkg-scalar/geo_polygonize.js";

// Override the init function
// input is ignored because we are using inlined Wasm
export default async function(_input?: any) {
    const url = hasSimd() ? wasmSimdUrl : wasmScalarUrl;
    await init(url);
    return exports;
}
