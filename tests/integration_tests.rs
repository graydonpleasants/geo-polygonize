use geo_polygonize::Polygonizer;
use geo_types::{LineString, Geometry, Polygon, Coord};
use geo::Area;
use std::f64::consts::PI;

#[test]
fn test_nested_holes() {
    let mut poly = Polygonizer::new();

    // Outer Box (0,0) - (100,100)
    poly.add_geometry(LineString::from(vec![
        (0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0), (0.0, 0.0)
    ]).into());

    // Inner Hole (20,20) - (80,80)
    poly.add_geometry(LineString::from(vec![
        (20.0, 20.0), (20.0, 80.0), (80.0, 80.0), (80.0, 20.0), (20.0, 20.0)
    ]).into());

    // Island inside Hole (40,40) - (60,60)
    poly.add_geometry(LineString::from(vec![
        (40.0, 40.0), (60.0, 40.0), (60.0, 60.0), (40.0, 60.0), (40.0, 40.0)
    ]).into());

    let polygons = poly.polygonize().unwrap();

    // The polygonizer produces a full mesh:
    // 1. The Donut (Outer - Hole). Area = 10000 - 3600 = 6400.
    // 2. The Filled Hole (Hole - Island). Area = 3600 - 400 = 3200.
    // 3. The Island. Area = 400.

    assert_eq!(polygons.len(), 3);

    let donut = polygons.iter().find(|p| (p.unsigned_area() - 6400.0).abs() < 1e-6);
    assert!(donut.is_some(), "Donut polygon with area 6400 not found");

    let filled_hole = polygons.iter().find(|p| (p.unsigned_area() - 3200.0).abs() < 1e-6);
    assert!(filled_hole.is_some(), "Filled hole polygon with area 3200 not found");

    let island = polygons.iter().find(|p| (p.unsigned_area() - 400.0).abs() < 1e-6);
    assert!(island.is_some(), "Island polygon with area 400 not found");
}

#[test]
fn test_touching_polygons() {
    let mut poly = Polygonizer::new();
    poly.node_input = true; // Required to deduplicate the shared edge

    // Square 1: (0,0)-(50,0)-(50,50)-(0,50)
    poly.add_geometry(LineString::from(vec![
        (0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0), (0.0, 0.0)
    ]).into());

    // Square 2: (50,0)-(100,0)-(100,50)-(50,50)-(50,0)
    // Shared edge: (50,0)-(50,50)
    poly.add_geometry(LineString::from(vec![
        (50.0, 0.0), (100.0, 0.0), (100.0, 50.0), (50.0, 50.0), (50.0, 0.0)
    ]).into());

    let polygons = poly.polygonize().unwrap();

    // Should find 3 polygons (Mesh behavior):
    // 1. Square 1 (Area 2500)
    // 2. Square 2 (Area 2500)
    // 3. Union / Outer Shell (Area 5000) or similar.

    assert!(polygons.len() >= 2);

    let squares_count = polygons.iter().filter(|p| (p.unsigned_area() - 2500.0).abs() < 1e-6).count();
    assert_eq!(squares_count, 2, "Expected 2 squares of area 2500");
}

#[test]
fn test_dangles() {
    let mut poly = Polygonizer::new();
    // A square with a tail
    poly.add_geometry(LineString::from(vec![
        (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)
    ]).into());

    // Tail
    poly.add_geometry(LineString::from(vec![
        (10.0, 10.0), (20.0, 20.0)
    ]).into());

    let polygons = poly.polygonize().unwrap();
    assert_eq!(polygons.len(), 1);
    assert!((polygons[0].unsigned_area() - 100.0).abs() < 1e-6);
}

#[test]
fn test_bowtie() {
    let mut poly = Polygonizer::new();
    poly.node_input = true;

    // Bowtie: (0,0)->(10,10)->(0,10)->(10,0)->(0,0)
    // Intersects at (5,5)
    poly.add_geometry(LineString::from(vec![
        (0.0, 0.0), (10.0, 10.0), (0.0, 10.0), (10.0, 0.0), (0.0, 0.0)
    ]).into());

    let polygons = poly.polygonize().unwrap();

    // Produces:
    // 1. Triangle 1 (Shell). Area 25.
    // 2. Triangle 2 (Shell). Area 25.
    // 3. The "Universe" or Outer Frame.

    assert!(polygons.len() >= 2);

    let triangles = polygons.iter().filter(|p| (p.unsigned_area() - 25.0).abs() < 1e-6).count();
    assert_eq!(triangles, 2);
}

fn create_circle(x: f64, y: f64, r: f64, points: usize) -> LineString<f64> {
    let step = 2.0 * PI / ((points - 1) as f64);
    let mut coords = Vec::new();
    for i in 0..points {
        let angle = (i as f64) * step;
        coords.push(Coord {
            x: x + r * angle.cos(),
            y: y + r * angle.sin(),
        });
    }
    LineString::new(coords)
}

#[test]
fn test_overlapping_circles() {
    let mut poly = Polygonizer::new();
    poly.node_input = true;

    // 1. Overlapping Circles
    let c1 = create_circle(30.0, 30.0, 30.0, 100);
    let c2 = create_circle(60.0, 30.0, 30.0, 100);
    let c3 = create_circle(45.0, 55.0, 30.0, 100);

    poly.add_geometry(c1.into());
    poly.add_geometry(c2.into());
    poly.add_geometry(c3.into());

    let polygons = poly.polygonize().unwrap();
    // Expect 8 (7 regions + 1 union).
    assert_eq!(polygons.len(), 8);
}

#[test]
fn test_curved_holes() {
    let mut poly = Polygonizer::new();
    poly.node_input = true;

    // 2. Curved Holes
    let outer = create_circle(50.0, 50.0, 50.0, 200);
    let h1 = create_circle(30.0, 30.0, 10.0, 100);
    let h2 = create_circle(70.0, 30.0, 10.0, 100);
    let h3 = create_circle(50.0, 70.0, 15.0, 100);
    let h4 = create_circle(50.0, 40.0, 5.0, 100);

    poly.add_geometry(outer.into());
    poly.add_geometry(h1.into());
    poly.add_geometry(h2.into());
    poly.add_geometry(h3.into());
    poly.add_geometry(h4.into());

    let polygons = poly.polygonize().unwrap();

    // Expect 5 (Outer + 4 holes).
    assert!(polygons.len() >= 5);
}
