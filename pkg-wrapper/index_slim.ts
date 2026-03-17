import initScalar, * as scalarExports from "../pkg-scalar/geo_polygonize.js";
import initSimd, * as simdExports from "../pkg-simd/geo_polygonize.js";

// We re-export everything. The user is responsible for calling init with the correct module/url.
export * from "../pkg-scalar/geo_polygonize.js";

// Export auto-generated ts-rs bindings
export * from "./bindings/PolygonizerOptions";
export * from "./bindings/ContainmentOptions";
export * from "./bindings/DeterminismOptions";
export * from "./bindings/DiagnosticsOptions";
export * from "./bindings/IndexBackend";
export * from "./bindings/NodingBackend";
export * from "./bindings/NodingOptions";
export * from "./bindings/ProvenanceOptions";
export * from "./bindings/SnapMode";
export * from "./bindings/SnapStrategy";
export * from "./bindings/TargetProfile";
export * from "./bindings/TileOwnershipPolicy";
export * from "./bindings/TilingOptions";
export * from "./bindings/TouchPolicy";
export * from "./bindings/ZOptions";
export * from "./bindings/ZPolicy";

// We provide a helper to choose based on feature detection if the user wants to use it
let isSimdSupported: boolean | undefined;
const SIMD_TEST_BYTES = new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,123,3,2,1,0,10,10,1,8,0,65,0,253,15,253,98,11]);

export async function initBest(scalarModule: any, simdModule: any) {
    if (isSimdSupported === undefined) {
        try {
            isSimdSupported = WebAssembly.validate(SIMD_TEST_BYTES);
        } catch (e) {
            isSimdSupported = false;
        }
    }

    if (isSimdSupported && simdModule) {
        await initSimd(simdModule);
        return simdExports;
    } else {
        await initScalar(scalarModule);
        return scalarExports;
    }
}
