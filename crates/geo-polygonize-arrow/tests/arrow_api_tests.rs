use arrow::array::{Array, Float64Array, LargeListArray, ListArray, StructArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use geo_polygonize_arrow::{polygonize_arrow, PolygonizerOptions};
use geo_polygonize_core::PolygonizeErrorKind;
use geo_traits::{LineStringTrait, PolygonTrait};
use geoarrow::array::{GeoArrowArray, GeoArrowArrayAccessor, LineStringBuilder};
use geoarrow::datatypes::{Crs, Dimension, Edges, LineStringType, Metadata};
use std::sync::Arc;

#[allow(dead_code)]
mod geoarrow_reference {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/geoarrow/reference.rs"
    ));
}

#[test]
fn test_polygonize_arrow_invalid_type_error_path() {
    let array = Float64Array::from(vec![1.0, 2.0, 3.0]);
    let field = Field::new("geometry", DataType::Float64, true);
    let options = PolygonizerOptions::default();

    let error = polygonize_arrow(&array, &field, options).unwrap_err();
    assert_eq!(error.kind(), PolygonizeErrorKind::ArrowError);
}

#[test]
fn test_polygonize_arrow_invalid_buffer_shape_error_path() {
    let values = Arc::new(Float64Array::from(vec![0.0, 1.0])) as Arc<dyn Array>;
    let item = Arc::new(Field::new("item", DataType::Float64, false));
    let array = ListArray::try_new(item, OffsetBuffer::from_lengths([2]), values, None).unwrap();
    let field = Field::new("geometry", array.data_type().clone(), true);

    let error = polygonize_arrow(&array, &field, PolygonizerOptions::default()).unwrap_err();
    assert_eq!(error.kind(), PolygonizeErrorKind::InvalidBufferShape);
}

#[test]
fn test_polygonize_arrow_fallback_large_list() {
    // Construct coordinate arrays
    let x_array = Float64Array::from(vec![0.0, 10.0, 10.0, 0.0, 0.0]);
    let y_array = Float64Array::from(vec![0.0, 0.0, 10.0, 10.0, 0.0]);

    let x_field = Arc::new(Field::new("x", DataType::Float64, false));
    let y_field = Arc::new(Field::new("y", DataType::Float64, false));

    // Create StructArray containing x and y coordinates
    let struct_array = StructArray::try_new(
        vec![x_field.clone(), y_field.clone()].into(),
        vec![
            Arc::new(x_array) as Arc<dyn Array>,
            Arc::new(y_array) as Arc<dyn Array>,
        ],
        None,
    )
    .unwrap();

    // Create a LargeListArray wrapper
    let offsets = OffsetBuffer::<i64>::from_lengths([5]);

    // Create the Field for the list items (Struct)
    let struct_field = Arc::new(Field::new("item", struct_array.data_type().clone(), false));

    let large_list_array =
        LargeListArray::try_new(struct_field, offsets, Arc::new(struct_array), None).unwrap();

    // The root field is LargeList
    let root_field = Field::new("geometry", large_list_array.data_type().clone(), true);

    let options = PolygonizerOptions::default();

    // Call polygonize_arrow on this large list array
    let result = polygonize_arrow(&large_list_array, &root_field, options)
        .expect("Failed to polygonize LargeList array");

    assert_eq!(result.len(), 1);

    if let Ok(Some(poly)) = result.get(0) {
        let exterior = poly.exterior().expect("Missing exterior");
        assert_eq!(exterior.num_coords(), 5);
    } else {
        panic!("Missing polygon");
    }
}

#[test]
fn polygonize_arrow_preserves_crs_metadata() {
    for crs in [
        Crs::from_authority_code("EPSG:3857".to_string()),
        Crs::from_wkt2_2019("PROJCRS[\"test\"]".to_string()),
        Crs::from_unknown_crs_type("opaque-crs".to_string()),
    ] {
        let input = geoarrow_reference::square(Arc::new(Metadata::new(crs, None)));
        let field = input.data_type().to_field("geometry", true);
        let array = input.into_array_ref();

        let result =
            polygonize_arrow(array.as_ref(), &field, PolygonizerOptions::default()).unwrap();
        let output_field = result.data_type().to_field("geometry", true);

        assert_eq!(output_field.extension_type_name(), Some("geoarrow.polygon"));
        assert_eq!(
            output_field.extension_type_metadata(),
            field.extension_type_metadata()
        );
    }
}

#[test]
fn polygonizes_official_geoarrow_reference_layouts() {
    for bytes in [
        geoarrow_reference::official_separated_ipc(),
        geoarrow_reference::official_interleaved_ipc(),
    ] {
        let (array, field) = geoarrow_reference::read_geometry_ipc(&bytes);
        let result =
            polygonize_arrow(array.as_ref(), &field, PolygonizerOptions::default()).unwrap();

        assert_eq!(field.extension_type_name(), Some("geoarrow.linestring"));
        assert_eq!(result.len(), 0);
        assert_eq!(
            result
                .data_type()
                .to_field("geometry", true)
                .extension_type_name(),
            Some("geoarrow.polygon")
        );
    }
}

#[test]
fn rejects_official_non_xy_geoarrow_reference_cases() {
    for bytes in geoarrow_reference::official_non_xy_ipc() {
        let (array, field) = geoarrow_reference::read_geometry_ipc(&bytes);
        let error = polygonize_arrow(array.as_ref(), &field, PolygonizerOptions::default())
            .unwrap_err()
            .to_string();

        assert!(error.contains("supports XY coordinates only"));
    }
}

#[test]
fn polygonize_arrow_rejects_non_planar_edges() {
    let metadata = Arc::new(Metadata::new(Default::default(), Some(Edges::Spherical)));
    let typ = LineStringType::new(Dimension::XY, metadata);
    let builder = LineStringBuilder::new(typ);
    let input = builder.finish();
    let field = input.data_type().to_field("geometry", true);
    let array = input.into_array_ref();

    let error = polygonize_arrow(array.as_ref(), &field, PolygonizerOptions::default())
        .unwrap_err()
        .to_string();

    assert!(error.contains("supports planar edges only"));
}
