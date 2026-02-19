//! Simple example of polygonizing a set of lines.

use geo_polygonize::Polygonizer;
use geo_types::{LineString, Geometry};
use geo::Area;

fn main() {
    let mut polygonizer = Polygonizer::new();

    // Create a simple square
    let square_lines = LineString::from(vec![
        (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)
    ]);

    // Add geometry to the polygonizer
    polygonizer.add_geometry(Geometry::LineString(square_lines));

    // Compute polygons
    let polygons = polygonizer.polygonize().expect("Failed to polygonize");

    println!("Found {} polygon(s)", polygons.len());

    for (i, poly) in polygons.iter().enumerate() {
        println!("Polygon {}: Area = {}", i, poly.unsigned_area());
    }
}
