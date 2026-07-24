import initScalar, * as scalarExports from "../pkg-scalar/geo_polygonize.js";
import { selectRuntime } from "./runtime";

// We re-export everything. The user is responsible for calling init with the correct module/url.
export * from "../pkg-scalar/geo_polygonize.js";

// Export auto-generated ts-rs bindings
export * from "./bindings/PolygonizerOptions";
export * from "./bindings/ContainmentOptions";
export * from "./bindings/DeterminismOptions";
export * from "./bindings/DiagnosticsOptions";
export * from "./bindings/NodingBackend";
export * from "./bindings/NodingGuarantee";
export * from "./bindings/NodingOptions";
export * from "./bindings/OutputFilterOptions";
export * from "./bindings/PrecisionModel";
export * from "./bindings/ProvenanceOptions";
export * from "./bindings/SnapStrategy";
export * from "./bindings/TileOwnershipPolicy";
export * from "./bindings/TouchPolicy";
export * from "./bindings/ZOptions";
export * from "./bindings/ZPolicy";
export * from "./cfb";

// We provide a helper to choose based on feature detection if the user wants to use it
function normalizeInitInput(input: any) {
    if (input && typeof input === "object" && "module" in input && !("module_or_path" in input)) {
        return { ...input, module_or_path: input.module };
    }
    return input;
}

export async function initBest(scalarModule: any, simdModule?: any) {
    const runtime = selectRuntime(scalarModule, simdModule ?? scalarModule);
    await initScalar(normalizeInitInput(runtime.module));
    return scalarExports;
}
