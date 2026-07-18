use arrow::array::{Array, Float64Array, LargeListArray, StructArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use geo_polygonize_core::arrow_api::{polygonize_arrow, PolygonizerOptions};
use geo_traits::{LineStringTrait, PolygonTrait};
use geoarrow::array::{GeoArrowArray, GeoArrowArrayAccessor, LineStringBuilder};
use geoarrow::datatypes::{Crs, Dimension, Edges, LineStringType, Metadata};
use std::sync::Arc;

#[test]
fn test_polygonize_arrow_invalid_type_error_path() {
    let array = Float64Array::from(vec![1.0, 2.0, 3.0]);
    let field = Field::new("geometry", DataType::Float64, true);
    let options = PolygonizerOptions::default();

    let result = polygonize_arrow(&array, &field, options);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Failed to convert input array to LineStringArray and fallback failed")
            && err.contains("DataType: Float64")
            && err.contains("Field { name: \"geometry\", data_type: Float64, nullable: true")
    );
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
    let metadata = Arc::new(Metadata::new(
        Crs::from_authority_code("EPSG:3857".to_string()),
        None,
    ));
    let typ = LineStringType::new(Dimension::XY, metadata.clone());
    let mut builder = LineStringBuilder::new(typ);
    builder
        .push_line_string(Some(&geo::LineString::from(vec![
            (0., 0.),
            (10., 0.),
            (10., 10.),
            (0., 10.),
            (0., 0.),
        ])))
        .unwrap();
    let input = builder.finish();
    let field = input.data_type().to_field("geometry", true);
    let array = input.into_array_ref();

    let result = polygonize_arrow(array.as_ref(), &field, PolygonizerOptions::default()).unwrap();
    let output_field = result.data_type().to_field("geometry", true);

    assert_eq!(output_field.extension_type_name(), Some("geoarrow.polygon"));
    assert_eq!(
        output_field.extension_type_metadata(),
        field.extension_type_metadata()
    );
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
