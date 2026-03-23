use geo_polygonize_core::options::*;
use ts_rs::TS;

fn main() -> Result<(), ts_rs::ExportError> {
    PolygonizerOptions::export_all(&ts_rs::Config::default())
}
