export * from "../pkg-threads/geo_polygonize.js";
import initThreads, * as threadExports from "../pkg-threads/geo_polygonize.js";
import { createPackedPolygonizer } from "./packed";

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

export default async function init(input?: Parameters<typeof initThreads>[0]) {
    const instance = await initThreads(input);
    packedApi = createPackedPolygonizer(threadExports, instance.memory);
    return { ...threadExports, ...packedApi };
}
