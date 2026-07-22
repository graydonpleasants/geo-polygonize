use arrow::array::Array;
use arrow::datatypes::Field;
use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
use geo_polygonize_arrow::ffi::{polygonize_ffi, polygonize_with_options_ffi, PolygonizerOptions};
use geo_polygonize_arrow::{polygonize_arrow, PolygonizerOptions as CoreOptions};
use geo_traits::{CoordTrait, LineStringTrait, PolygonTrait};
use geoarrow::array::{
    GeoArrowArray, GeoArrowArrayAccessor, LineStringArray, LineStringBuilder, PolygonArray,
};
use geoarrow::datatypes::{Crs, Dimension, LineStringType, Metadata};
use serde_json::Value;
use std::convert::TryFrom;
use std::sync::Arc;

#[allow(dead_code)]
mod geoarrow_reference {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/geoarrow/reference.rs"
    ));
}

fn conformance_fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../geo-polygonize-core/tests/fixtures/conformance/axis_aligned_ring_v1.json"
    ))
    .unwrap()
}

fn conformance_input(fixture: &Value) -> (LineStringArray, Field) {
    let coords = fixture["coords"].as_array().unwrap();
    let line = geo::LineString::from(
        coords
            .chunks_exact(2)
            .map(|point| (point[0].as_f64().unwrap(), point[1].as_f64().unwrap()))
            .collect::<Vec<_>>(),
    );
    let mut builder = LineStringBuilder::new(LineStringType::new(
        Dimension::XY,
        Arc::new(Default::default()),
    ));
    builder.push_line_string(Some(&line)).unwrap();
    let array = builder.finish();
    let field = array.data_type().to_field("geometry", true);
    (array, field)
}

fn canonical_ring(mut ring: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    ring.pop();
    let twice_area: f64 = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|((x1, y1), (x2, y2))| x1 * y2 - x2 * y1)
        .sum();
    if twice_area < 0.0 {
        ring.reverse();
    }
    let first = ring
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        })
        .unwrap()
        .0;
    ring.rotate_left(first);
    ring.push(ring[0]);
    ring
}

fn assert_conformance_polygon(fixture: &Value, polygons: &PolygonArray) {
    let expected = fixture["expected_fingerprint"]["polygons"][0]["exterior"]
        .as_array()
        .unwrap()
        .iter()
        .map(|coord| {
            let parse = |name| {
                f64::from_bits(
                    u64::from_str_radix(&coord[name].as_str().unwrap()[2..], 16).unwrap(),
                )
            };
            (parse("x"), parse("y"))
        })
        .collect::<Vec<_>>();
    let polygon = polygons.get(0).unwrap().unwrap();
    let exterior = polygon.exterior().unwrap();
    let actual = (0..exterior.num_coords())
        .map(|index| {
            let coord = exterior.coord(index).unwrap();
            (coord.x(), coord.y())
        })
        .collect::<Vec<_>>();
    assert_eq!(canonical_ring(actual), canonical_ring(expected));
}

#[test]
fn arrow_and_c_data_retain_the_shared_conformance_polygon() {
    let fixture = conformance_fixture();

    let (input, field) = conformance_input(&fixture);
    let polygons = polygonize_arrow(
        input.into_array_ref().as_ref(),
        &field,
        CoreOptions::default(),
    )
    .unwrap();
    assert_conformance_polygon(&fixture, &polygons);

    let (input, field) = conformance_input(&fixture);
    let input = input.into_array_ref();
    let (input_array, _) = arrow::ffi::to_ffi(&input.to_data()).unwrap();
    let input_schema = FFI_ArrowSchema::try_from(&field).unwrap();
    let mut input_array = std::mem::ManuallyDrop::new(input_array);
    let mut input_schema = std::mem::ManuallyDrop::new(input_schema);
    let mut output_array = FFI_ArrowArray::empty();
    let mut output_schema = FFI_ArrowSchema::empty();
    let options = serde_json::to_vec(&fixture["options"]).unwrap();
    let status = unsafe {
        polygonize_with_options_ffi(
            &mut *input_array,
            &mut *input_schema,
            &mut output_array,
            &mut output_schema,
            options.as_ptr(),
            options.len(),
        )
    };
    assert_eq!(status, 0);
    let output = unsafe { arrow::ffi::from_ffi(output_array, &output_schema).unwrap() };
    let output = arrow::array::make_array(output);
    let field = Field::try_from(&output_schema).unwrap();
    let polygons = PolygonArray::try_from((output.as_ref(), &field)).unwrap();
    assert_conformance_polygon(&fixture, &polygons);
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
