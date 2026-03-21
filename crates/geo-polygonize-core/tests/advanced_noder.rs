use geo_polygonize_core::{
    options::{NodingBackend, PolygonizerOptions},
    types::{Coord3D, Line3D},
    Polygonizer,
};

fn coord(x: f64, y: f64) -> Coord3D {
    Coord3D { x, y, z: 0.0 }
}

fn line(c1: Coord3D, c2: Coord3D) -> Line3D {
    Line3D { start: c1, end: c2, line_id: 0 }
}

#[test]
fn test_advanced_noder_basic_intersection() {
    let mut options = PolygonizerOptions::default();
    options.noding.backend = NodingBackend::Advanced;
    // node_input must be true to trigger noding backend
    options.node_input = true;

    let mut polygonizer = Polygonizer::with_options(options);

    // Two crossing lines
    let line1 = line(coord(0.0, 0.0), coord(10.0, 10.0));
    let line2 = line(coord(0.0, 10.0), coord(10.0, 0.0));

    polygonizer.add_lines(vec![line1, line2]);

    // Should create 4 polygons (triangles) if bound by a box, but here just an X shape.
    // So there should be no polygons, but we can check graph size or just run it to ensure no panics
    // Let's add a bounding box so it forms polygons.
    let bbox_lines = vec![
        line(coord(0.0, 0.0), coord(10.0, 0.0)),
        line(coord(10.0, 0.0), coord(10.0, 10.0)),
        line(coord(10.0, 10.0), coord(0.0, 10.0)),
        line(coord(0.0, 10.0), coord(0.0, 0.0)),
    ];
    polygonizer.add_lines(bbox_lines);

    let result = polygonizer.polygonize().expect("Polygonize should succeed");

    // The bbox + crossing lines should create 4 triangles.
    assert_eq!(result.polygons.len(), 4, "Should have noded and created 4 triangles");
}
