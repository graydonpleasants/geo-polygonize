use arrow::array::Array;
use arrow::datatypes::Field;
use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
use geo_polygonize_core::ffi::{polygonize_ffi, PolygonizerOptions};
use geo_traits::LineStringTrait;
use geo_traits::PolygonTrait;
use geoarrow::array::{GeoArrowArray, GeoArrowArrayAccessor, PolygonArray};
use std::convert::TryFrom;
use std::sync::Arc;

#[test]
fn test_ffi_arrow_integration_square() {
    // 1. Create Input Arrow Array (LineStringArray)
    let coord0 = geo::Coord { x: 0.0, y: 0.0 };
    let coord1 = geo::Coord { x: 10.0, y: 0.0 };
    let coord2 = geo::Coord { x: 10.0, y: 10.0 };
    let coord3 = geo::Coord { x: 0.0, y: 10.0 };

    let line_string = geo::LineString::new(vec![coord0, coord1, coord2, coord3, coord0]);

    use geoarrow::datatypes::{Dimension, LineStringType};
    let typ = LineStringType::new(Dimension::XY, Arc::new(Default::default()));
    let mut builder = geoarrow::array::LineStringBuilder::new(typ);
    builder.push_line_string(Some(&line_string)).unwrap();
    let input_array = builder.finish();

    // 2. Export Input to FFI
    let arrow_array = input_array.into_array_ref();
    let (input_array_ffi, input_schema_ffi) =
        arrow::ffi::to_ffi(&arrow_array.to_data()).expect("Failed to export input array to FFI");

    let mut input_array_ffi = std::mem::ManuallyDrop::new(input_array_ffi);
    let mut input_schema_ffi = std::mem::ManuallyDrop::new(input_schema_ffi);

    let mut output_array = FFI_ArrowArray::empty();
    let mut output_schema = FFI_ArrowSchema::empty();

    let options = PolygonizerOptions {
        node_input: 0,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
        report_mode: 0,
    };

    // 5. Call FFI
    let status = unsafe {
        polygonize_ffi(
            &mut *input_array_ffi,
            &mut *input_schema_ffi,
            &mut output_array,
            &mut output_schema,
            &options,
        )
    };

    assert_eq!(status, 0, "FFI call failed with code {}", status);

    // 6. Import Output from FFI
    let output_data = unsafe {
        arrow::ffi::from_ffi(output_array, &output_schema)
            .expect("Failed to import output from FFI")
    };
    let output_arrow_array = arrow::array::make_array(output_data);

    // 7. Verify Output (PolygonArray)
    let field = Field::try_from(&output_schema).expect("Failed to import output field");

    let polygon_array = PolygonArray::try_from((output_arrow_array.as_ref(), &field))
        .expect("Failed to convert to PolygonArray");

    assert_eq!(polygon_array.len(), 1);

    if let Ok(Some(poly)) = polygon_array.get(0) {
        let exterior = poly.exterior().expect("Missing exterior");
        assert_eq!(exterior.num_coords(), 5);
    } else {
        panic!("Missing polygon");
    }
}

#[test]
fn test_ffi_arrow_integration_empty() {
    use geoarrow::datatypes::{Dimension, LineStringType};
    let typ = LineStringType::new(Dimension::XY, Arc::new(Default::default()));
    let builder = geoarrow::array::LineStringBuilder::new(typ);
    let input_array = builder.finish();

    let arrow_array = input_array.into_array_ref();
    let (input_array_ffi, input_schema_ffi) =
        arrow::ffi::to_ffi(&arrow_array.to_data()).expect("Failed to export input array to FFI");

    let mut input_array_ffi = std::mem::ManuallyDrop::new(input_array_ffi);
    let mut input_schema_ffi = std::mem::ManuallyDrop::new(input_schema_ffi);

    let mut output_array = FFI_ArrowArray::empty();
    let mut output_schema = FFI_ArrowSchema::empty();

    let options = PolygonizerOptions {
        node_input: 0,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
        report_mode: 0,
    };

    let status = unsafe {
        polygonize_ffi(
            &mut *input_array_ffi,
            &mut *input_schema_ffi,
            &mut output_array,
            &mut output_schema,
            &options,
        )
    };

    assert_eq!(status, 0, "FFI call failed with code {}", status);

    let output_data = unsafe { arrow::ffi::from_ffi(output_array, &output_schema).unwrap() };
    let output_arrow_array = arrow::array::make_array(output_data);
    assert_eq!(output_arrow_array.len(), 0);
}
