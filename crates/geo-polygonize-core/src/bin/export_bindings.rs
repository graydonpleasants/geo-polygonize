use geo_polygonize_core::options::*;
use ts_rs::TS;

fn main() {
    let config = ts_rs::Config::from_env();

    // We export each root type with export_all so it writes out that type and its dependencies.
    let _ = PolygonizerOptions::export_all(&config);
    let _ = SnapStrategy::export_all(&config);
    let _ = TouchPolicy::export_all(&config);
    let _ = TileOwnershipPolicy::export_all(&config);
    let _ = NodingBackend::export_all(&config);
    let _ = NodingOptions::export_all(&config);
    let _ = OutputFilterOptions::export_all(&config);
    let _ = ContainmentOptions::export_all(&config);
    let _ = DeterminismOptions::export_all(&config);
    let _ = geo_polygonize_core::diagnostics::DiagnosticsOptions::export_all(&config);
    let _ = ProvenanceOptions::export_all(&config);
    let _ = DedupPolicy::export_all(&config);
}
