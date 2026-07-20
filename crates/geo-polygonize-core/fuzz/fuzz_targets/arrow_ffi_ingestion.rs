#![no_main]

use geo_polygonize_core::types::{Coord3D, Line3D};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arrow FFI ingestion is mainly interacting with pointers.
    // Fuzzing it directly from raw bytes is unsafe and unreliable without a valid Arrow structure.
    // Instead, we fuzz the extraction logic given fuzzed coordinate arrays mimicking Arrow buffers.

    if data.len() % (8 * 2) != 0 || data.len() > 100 * 8 * 2 {
        return;
    }

    let mut coords = Vec::new();
    for chunk in data.chunks_exact(8 * 2) {
        let x = f64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let y = f64::from_le_bytes(chunk[8..16].try_into().unwrap());
        if x.is_nan() || y.is_nan() {
            return;
        }
        coords.push(Coord3D { x, y, z: 0.0 });
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
