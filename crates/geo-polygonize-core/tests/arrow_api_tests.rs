use arrow::array::Float64Array;
use arrow::datatypes::{DataType, Field};
use geo_polygonize_core::arrow_api::{polygonize_arrow, PolygonizerOptions};

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
