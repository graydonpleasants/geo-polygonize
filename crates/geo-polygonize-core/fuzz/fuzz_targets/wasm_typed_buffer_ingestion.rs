#![no_main]

use geo_polygonize_core::{polygonize, PolygonizerOptions};
use geo_polygonize_wasm::parse_buffer_lines;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let flags = data.first().copied().unwrap_or_default();
    let stride = match flags & 3 {
        0 => 2,
        1 => 3,
        invalid => invalid - 2,
    };
    let coords = data
        .get(1..)
        .unwrap_or_default()
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| f64::from_le_bytes(*chunk))
        .collect::<Vec<_>>();
    let offsets = data
        .get(1..)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .take(16)
        .map(|chunk| u16::from_le_bytes(*chunk) as u32)
        .collect::<Vec<_>>();
    let line_ids = (flags & 4 != 0).then(|| {
        data.get(1..)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .take(if flags & 8 == 0 {
                offsets.len()
            } else {
                offsets.len().saturating_sub(1)
            })
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect::<Vec<_>>()
    });

    if let Ok(lines) = parse_buffer_lines(&coords, &offsets, stride, line_ids.as_deref()) {
        let _ = polygonize(lines, &PolygonizerOptions::default());
    }
});
