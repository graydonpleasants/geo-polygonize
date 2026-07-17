use geo_polygonize_core::{
    noding::{advanced::AdvancedNoder, snap::SnapNoder},
    options::{NodingBackend, PolygonizerOptions},
    types::{Coord3D, Line3D},
    Polygonizer,
};

fn coord(x: f64, y: f64) -> Coord3D {
    Coord3D { x, y, z: 0.0 }
}

fn line(c1: Coord3D, c2: Coord3D) -> Line3D {
    Line3D {
        start: c1,
        end: c2,
        line_id: 0,
    }
}

fn line_with_id(c1: Coord3D, c2: Coord3D, line_id: u32) -> Line3D {
    Line3D::new(c1, c2, line_id)
}

fn assert_same_lines(actual: &[Line3D], expected: &[Line3D]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.start, expected.start);
        assert_eq!(actual.end, expected.end);
        assert_eq!(actual.line_id, expected.line_id);
    }
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
    assert_eq!(
        result.polygons.len(),
        4,
        "Should have noded and created 4 triangles"
    );
}

#[test]
fn advanced_backend_is_exact_snap_compatibility_alias() {
    let lines = vec![
        line_with_id(coord(0.0, 0.0), coord(10.0, 10.0), 1),
        line_with_id(coord(0.0, 10.0), coord(10.0, 0.0), 2),
        line_with_id(coord(5.0, -2.0), coord(5.0, 12.0), 3),
        line_with_id(coord(2.0, 2.0), coord(8.0, 8.0), 4),
    ];

    let actual = AdvancedNoder::new().node(lines.clone());
    let expected = SnapNoder::new(0.0).node(lines);

    assert_same_lines(&actual, &expected);
}

#[test]
fn advanced_backend_handles_collinear_overlaps() {
    let lines = vec![
        line_with_id(coord(0.0, 0.0), coord(10.0, 0.0), 1),
        line_with_id(coord(5.0, 0.0), coord(15.0, 0.0), 2),
    ];

    let noded = AdvancedNoder::new().node(lines);

    assert!(noded
        .iter()
        .any(|line| line.start == coord(0.0, 0.0) && line.end == coord(5.0, 0.0)));
    assert!(noded
        .iter()
        .any(|line| line.start == coord(5.0, 0.0) && line.end == coord(10.0, 0.0)));
    assert!(noded
        .iter()
        .any(|line| line.start == coord(10.0, 0.0) && line.end == coord(15.0, 0.0)));
}

#[test]
fn advanced_backend_interpolates_z_per_source_line() {
    let lines = vec![
        line_with_id(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 10.0),
            1,
        ),
        line_with_id(
            Coord3D::new(5.0, -5.0, 100.0),
            Coord3D::new(5.0, 5.0, 300.0),
            2,
        ),
    ];

    let noded = AdvancedNoder::new().node(lines);
    let z_for = |line_id| {
        noded
            .iter()
            .filter(|line| line.line_id == line_id)
            .flat_map(|line| [line.start, line.end])
            .find(|point| point.x == 5.0 && point.y == 0.0)
            .map(|point| point.z)
            .unwrap()
    };

    assert_eq!(z_for(1), 5.0);
    assert_eq!(z_for(2), 200.0);
}

#[test]
fn advanced_backend_is_permutation_invariant() {
    let lines = vec![
        line_with_id(coord(0.0, 0.0), coord(10.0, 10.0), 1),
        line_with_id(coord(0.0, 10.0), coord(10.0, 0.0), 2),
        line_with_id(coord(5.0, -2.0), coord(5.0, 12.0), 3),
    ];
    let expected = AdvancedNoder::new().node(lines.clone());
    let mut reversed = lines;
    reversed.reverse();

    let actual = AdvancedNoder::new().node(reversed);

    assert_same_lines(&actual, &expected);
}
