use geo::LineString;
use geo_polygonize::Polygonizer;

#[test]
fn test_nan_polygon_panic() {
    let mut polygonizer = Polygonizer::new();
    // Add geometry with NaN
    // This previously caused a panic in process_holes due to assuming valid geometry
    polygonizer.add_geometry(
        LineString::from(vec![(0.0, 0.0), (f64::NAN, 0.0), (0.0, 10.0), (0.0, 0.0)]).into(),
    );

    // This should handle it gracefully or return Err, but NOT panic.
    let _result = polygonizer.polygonize();
}
