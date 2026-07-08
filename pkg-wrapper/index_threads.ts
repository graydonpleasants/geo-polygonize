export * from "../pkg-threads/geo_polygonize.js";
import init from "../pkg-threads/geo_polygonize.js";

// Export auto-generated ts-rs bindings
export * from "./bindings/PolygonizerOptions";
export * from "./bindings/ContainmentOptions";
export * from "./bindings/DeterminismOptions";
export * from "./bindings/DiagnosticsOptions";
export * from "./bindings/NodingBackend";
export * from "./bindings/NodingOptions";
export * from "./bindings/ProvenanceOptions";
export * from "./bindings/SnapStrategy";
export * from "./bindings/TileOwnershipPolicy";
export * from "./bindings/TilingOptions";
export * from "./bindings/TouchPolicy";
export * from "./cfb";

export default init;
