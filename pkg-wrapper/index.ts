import wasmInit, * as exports from "../pkg-scalar/geo_polygonize.js";
import wasmScalarUrl from "../pkg-scalar/geo_polygonize_bg.wasm";
import wasmSimdUrl from "../pkg-simd/geo_polygonize_bg.wasm";

// Perform SIMD detection once at module load time
const simdSupported = (() => {
    try {
        return WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,123,3,2,1,0,10,10,1,8,0,65,0,253,15,253,98,11]));
    } catch (e) {
        return false;
    }
})();

// Cache the initialization promise
let initPromise: Promise<typeof exports> | undefined;

// We re-export everything from the scalar package.
// The JS bindings in pkg-scalar/geo_polygonize.js are identical to pkg-simd/geo_polygonize.js
// because the exported API is the same.
// By calling init() with the correct Wasm binary, these exported functions will use that binary.
export * from "../pkg-scalar/geo_polygonize.js";

// Override the init function
// input is ignored because we are using inlined Wasm
export default function init(_input?: any): Promise<typeof exports> {
    if (initPromise) return initPromise;

    const url = simdSupported ? wasmSimdUrl : wasmScalarUrl;

    // Create the promise and cache it
    initPromise = (async () => {
        await wasmInit(url);
        return exports;
    })();

    return initPromise;
}
