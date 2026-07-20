//! Simple example of polygonizing a set of lines.

use geo_polygonize_core::options::PolygonizerOptions;
use geo_polygonize_core::{polygonize, Coord3D, Line3D};

fn main() {
    let points = [
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(10.0, 0.0, 0.0),
        Coord3D::new(10.0, 10.0, 0.0),
        Coord3D::new(0.0, 10.0, 0.0),
    ];
    let lines = (0..4).map(|i| Line3D::new(points[i], points[(i + 1) % 4], i as u32));
    let result = polygonize(lines, &PolygonizerOptions::default()).expect("Failed to polygonize");

    println!("Found {} polygon(s)", result.polygons.len());
    for (i, polygon) in result.polygons.iter().enumerate() {
        println!("Polygon {}: Area = {}", i, polygon.unsigned_area_2d());
    }
}
