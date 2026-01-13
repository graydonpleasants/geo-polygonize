use geo_types::{LineString, Coord, Polygon};
use geo_polygonize::Polygonizer;
use geo::Geometry;

#[test]
fn test_bowtie_noding() {
    // A bowtie shape: (0,0) -> (10,10) -> (10,0) -> (0,10) -> (0,0)
    // Intersection at (5,5).
    let ls = LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 10.0 },
        Coord { x: 10.0, y: 0.0 },
        Coord { x: 0.0, y: 10.0 },
        Coord { x: 0.0, y: 0.0 },
    ]);

    let mut poly = Polygonizer::new();
    poly.node_input = true;
    poly.snap_grid_size = 1e-6;
    poly.add_geometry(Geometry::LineString(ls));

    let results = poly.polygonize().expect("Polygonization failed");

    println!("Bowtie Results: {}", results.len());
    for (i, p) in results.iter().enumerate() {
        println!("Poly {}: {:?}", i, p);
    }

    assert_eq!(results.len(), 2, "Expected 2 polygons from bowtie");
}

#[test]
fn test_duplicate_edge_removal() {
    let mut poly = Polygonizer::new();
    poly.node_input = true;
    poly.snap_grid_size = 1e-6;

    // Triangle edge 1
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 0.0 }
    ])));
    // Duplicate edge 1
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 0.0 }
    ])));

    // Edge 2
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 10.0, y: 0.0 },
        Coord { x: 5.0, y: 5.0 }
    ])));
    // Edge 3
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 5.0, y: 5.0 },
        Coord { x: 0.0, y: 0.0 }
    ])));

    let results = poly.polygonize().expect("Polygonization failed");
    assert_eq!(results.len(), 1);
}
