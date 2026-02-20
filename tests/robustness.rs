use geo::Geometry;
use geo_polygonize::Polygonizer;
use geo_types::{Coord, LineString};

#[test]
fn test_bowtie_noding() -> Result<(), Box<dyn std::error::Error>> {
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

    let results = poly.polygonize()?;

    assert_eq!(results.len(), 2, "Expected 2 polygons from bowtie");
    Ok(())
}

#[test]
fn test_duplicate_edge_removal() -> Result<(), Box<dyn std::error::Error>> {
    let mut poly = Polygonizer::new();
    poly.node_input = true;
    poly.snap_grid_size = 1e-6;

    // Triangle edge 1
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 0.0 },
    ])));
    // Duplicate edge 1
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 0.0 },
    ])));

    // Edge 2
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 10.0, y: 0.0 },
        Coord { x: 5.0, y: 5.0 },
    ])));
    // Edge 3
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 5.0, y: 5.0 },
        Coord { x: 0.0, y: 0.0 },
    ])));

    let results = poly.polygonize()?;
    assert_eq!(results.len(), 1);
    Ok(())
}
