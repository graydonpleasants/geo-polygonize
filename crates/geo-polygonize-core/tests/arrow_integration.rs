use arrow::array::Array;
use arrow::datatypes::Field;
use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
use geo_polygonize_core::ffi::{polygonize_ffi, PolygonizerOptions};
use geoarrow::array::{GeoArrowArray, PolygonArray};
use geoarrow::datatypes::{Crs, Metadata};
use std::convert::TryFrom;
use std::sync::Arc;

#[allow(dead_code)]
mod geoarrow_reference {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/geoarrow/reference.rs"
    ));
}

#[test]
fn test_ffi_arrow_integration_square() {
    let metadata = Arc::new(Metadata::new(
        Crs::from_authority_code("EPSG:3857".to_string()),
        None,
    ));
    let input_array = geoarrow_reference::square(metadata);
    let input_field = input_array.data_type().to_field("geometry", true);

    // 2. Export Input to FFI
    let arrow_array = input_array.into_array_ref();
    let (input_array_ffi, _) =
        arrow::ffi::to_ffi(&arrow_array.to_data()).expect("Failed to export input array to FFI");
    let input_schema_ffi =
        FFI_ArrowSchema::try_from(&input_field).expect("Failed to export GeoArrow input field");

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

    geoarrow_reference::assert_square(&polygon_array);
    assert_eq!(
        field.extension_type_metadata(),
        input_field.extension_type_metadata()
    );
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
