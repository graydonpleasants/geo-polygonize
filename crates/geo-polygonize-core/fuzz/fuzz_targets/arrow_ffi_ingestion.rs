#![no_main]

use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
use geo_polygonize_arrow::ffi::{polygonize_ffi, PolygonizerOptions};
use geoarrow::array::{GeoArrowArray, LineStringBuilder};
use geoarrow::datatypes::{Dimension, LineStringType};
use libfuzzer_sys::fuzz_target;
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    let flags = data.first().copied().unwrap_or_default();
    let snap_grid_size = data
        .get(1..9)
        .map(|bytes| f64::from_le_bytes(bytes.try_into().unwrap()))
        .unwrap_or_default();
    let options = PolygonizerOptions {
        node_input: flags & 1,
        snap_grid_size,
        extract_only_polygonal: (flags >> 1) & 1,
        report_mode: (flags >> 2) & 1,
    };
    let mut builder = LineStringBuilder::new(LineStringType::new(
        Dimension::XY,
        Arc::new(Default::default()),
    ));
    let coords = data
        .get(9..)
        .unwrap_or_default()
        .as_chunks::<16>()
        .0
        .iter()
        .map(|chunk| {
            (
                f64::from_le_bytes(chunk[..8].try_into().unwrap()),
                f64::from_le_bytes(chunk[8..].try_into().unwrap()),
            )
        })
        .collect::<Vec<_>>();
    for coords in coords.chunks(usize::from(flags >> 4) + 1) {
        let line = geo::LineString::from(coords.to_vec());
        builder.push_line_string(Some(&line)).unwrap();
    }

    let input = builder.finish();
    let field = input.data_type().to_field("geometry", true);
    let array = input.into_array_ref();
    let (mut input_array, _) = arrow::ffi::to_ffi(&array.to_data()).unwrap();
    let mut input_schema = FFI_ArrowSchema::try_from(&field).unwrap();
    let mut output_array = FFI_ArrowArray::empty();
    let mut output_schema = FFI_ArrowSchema::empty();

    unsafe {
        polygonize_ffi(
            &mut input_array,
            &mut input_schema,
            &mut output_array,
            &mut output_schema,
            &options,
        );
    }
});
