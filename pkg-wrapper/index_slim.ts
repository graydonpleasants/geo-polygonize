import initScalar, * as scalarExports from "../pkg-scalar/geo_polygonize.js";
import { selectRuntime } from "./runtime";
import { createPackedPolygonizer } from "./packed";

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
export * from "./topology_trace";
export { PackedPolygonResult, packedBuffersToGeoJSON, packedSnapshotTransferList } from "./packed";
export type {
    BorrowedPackedBuffers,
    GeoJsonFeatureCollection,
    PackedBoundaryMetrics,
    PackedFullReport,
    PackedPolygonSnapshot,
    PackedProjection,
} from "./packed";

type PackedApi = ReturnType<typeof createPackedPolygonizer>;
let packedApi: PackedApi | undefined;

function requirePackedApi(): PackedApi {
    if (!packedApi) throw new Error("geo-polygonize must be initialized before packed use");
    return packedApi;
}

export function polygonizePackedWithOptions(
    ...args: Parameters<PackedApi["polygonizePackedWithOptions"]>
) {
    return requirePackedApi().polygonizePackedWithOptions(...args);
}

export function polygonizePackedGeometryWithOptions(
    ...args: Parameters<PackedApi["polygonizePackedGeometryWithOptions"]>
) {
    return requirePackedApi().polygonizePackedGeometryWithOptions(...args);
}

// We provide a helper to choose based on feature detection if the user wants to use it
function normalizeInitInput(input: unknown): Parameters<typeof initScalar>[0] {
    if (input && typeof input === "object" && "module" in input && !("module_or_path" in input)) {
        return { ...input, module_or_path: input.module };
    }
    return input as Parameters<typeof initScalar>[0];
}

export async function initBest(scalarModule: unknown, simdModule?: unknown) {
    const runtime = selectRuntime(scalarModule, simdModule ?? scalarModule);
    const instance = await initScalar(normalizeInitInput(runtime.module));
    packedApi = createPackedPolygonizer(scalarExports, instance.memory);
    return { ...scalarExports, ...packedApi };
}
