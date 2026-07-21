use geo::Area;
use geo_polygonize_core::{
    polygonize_line_strings, polygonize_to_multi_polygon, Coord3D, Line3D, PolygonizerOptions,
};
use geo_types::LineString;

#[test]
fn polygonizes_borrowed_geo_traits_line_strings() {
    let ring = LineString::from(vec![
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 3.0),
        (0.0, 3.0),
        (0.0, 0.0),
    ]);

    let result = polygonize_line_strings([&ring], &PolygonizerOptions::default()).unwrap();
    let polygons = result.into_multi_polygon();

    assert_eq!(polygons.0.len(), 1);
    assert_eq!(polygons.unsigned_area(), 12.0);
}

#[test]
fn returns_geo_types_multi_polygon_directly() {
    let points = [
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1.0, 0.0, 0.0),
        Coord3D::new(0.0, 1.0, 0.0),
    ];
    let lines = (0..3).map(|i| Line3D::new(points[i], points[(i + 1) % 3], i as u32));

    let polygons = polygonize_to_multi_polygon(lines, &PolygonizerOptions::default()).unwrap();

    assert_eq!(polygons.0.len(), 1);
    assert_eq!(polygons.unsigned_area(), 0.5);
}
