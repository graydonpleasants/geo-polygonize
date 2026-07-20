#![no_main]

use geo_polygonize_core::types::{Coord3D, Line3D};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // This is a placeholder since we can't easily fuzz the exact JS typed buffer API
    // from native Rust, but we can fuzz the parsing logic.
    if data.len() % (8 * 3) != 0 || data.len() > 100 * 8 * 3 {
        return;
    }

    let mut coords = Vec::new();
    for chunk in data.chunks_exact(8 * 3) {
        let x = f64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let y = f64::from_le_bytes(chunk[8..16].try_into().unwrap());
        let z = f64::from_le_bytes(chunk[16..24].try_into().unwrap());
        if x.is_nan() || y.is_nan() || z.is_nan() {
            return;
        }
        coords.push(Coord3D { x, y, z });
    }

    let mut lines = Vec::new();
    for i in 0..coords.len().saturating_sub(1) {
        lines.push(Line3D {
            start: coords[i],
            end: coords[i + 1],
            line_id: i as u32,
        });
    }

    let options = geo_polygonize_core::options::PolygonizerOptions::default();
    let _ = geo_polygonize_core::polygonizer::polygonize(lines.iter().copied(), &options);
});
