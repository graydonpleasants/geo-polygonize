import initScalar, * as scalarExports from "../pkg-scalar/geo_polygonize.js";
import initSimd, * as simdExports from "../pkg-simd/geo_polygonize.js";

// We re-export everything. The user is responsible for calling init with the correct module/url.
export * from "../pkg-scalar/geo_polygonize.js";

// We provide a helper to choose based on feature detection if the user wants to use it
export async function initBest(scalarModule: any, simdModule: any) {
    let simdSupported = false;
    try {
        simdSupported = WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,123,3,2,1,0,10,10,1,8,0,65,0,253,15,253,98,11]));
    } catch (e) {}

    if (simdSupported && simdModule) {
        await initSimd(simdModule);
        return simdExports;
    } else {
        await initScalar(scalarModule);
        return scalarExports;
    }
}
