use geo_polygonize_core::options::DeterminismOptions;
use geo_polygonize_core::types::{Coord3D, Line3D};
use geo_polygonize_core::Polygonizer;

#[test]
fn test_determinism_canonical_sort_and_rotation() {
    // We will build a test that verifies:
    // 1. the same input geometries with their segment order permuted produces identical canonical output.
    // 2. polygon holes are ordered consistently.
    // 3. polygons are ordered consistently.

    let create_polygons_from_lines = |lines: Vec<Line3D>, use_determinism: bool| {
        let mut poly = Polygonizer::new();
        poly.node_input = true;
        if use_determinism {
            poly.determinism = DeterminismOptions {
                canonical_sort: true,
                canonical_ring_rotation: true,
                stable_tie_breaks: true,
            };
        }
        poly.add_lines(lines);
        poly.polygonize().unwrap()
    };

    // A large box with two smaller boxes inside (holes), and an extra disconnected box.
    // Box 1 (Outer): (0,0) to (10,10)
    // Hole 1: (2,2) to (4,4)
    // Hole 2: (6,6) to (8,8)
    // Box 2 (Disconnected): (20,20) to (30,30)

    let create_box = |min_x, min_y, max_x, max_y| -> Vec<Line3D> {
        vec![
            Line3D::new(
                Coord3D::new(min_x, min_y, 0.0),
                Coord3D::new(max_x, min_y, 0.0),
                0,
            ),
            Line3D::new(
                Coord3D::new(max_x, min_y, 0.0),
                Coord3D::new(max_x, max_y, 0.0),
                0,
            ),
            Line3D::new(
                Coord3D::new(max_x, max_y, 0.0),
                Coord3D::new(min_x, max_y, 0.0),
                0,
            ),
            Line3D::new(
                Coord3D::new(min_x, max_y, 0.0),
                Coord3D::new(min_x, min_y, 0.0),
                0,
            ),
        ]
    };

    let mut lines_order_1 = Vec::new();
    lines_order_1.extend(create_box(0.0, 0.0, 10.0, 10.0));
    lines_order_1.extend(create_box(2.0, 2.0, 4.0, 4.0));
    lines_order_1.extend(create_box(6.0, 6.0, 8.0, 8.0));
    lines_order_1.extend(create_box(20.0, 20.0, 30.0, 30.0));

    let mut lines_order_2 = Vec::new();
    lines_order_2.extend(create_box(20.0, 20.0, 30.0, 30.0));
    // change the direction/start points
    lines_order_2.extend(create_box(6.0, 6.0, 8.0, 8.0).into_iter().rev());
    lines_order_2.extend(create_box(2.0, 2.0, 4.0, 4.0));
    lines_order_2.extend(create_box(0.0, 0.0, 10.0, 10.0).into_iter().rev());

    let res1 = create_polygons_from_lines(lines_order_1.clone(), true);
    let res2 = create_polygons_from_lines(lines_order_2.clone(), true);

    // Assert that the exact same result is returned in both cases
    assert_eq!(res1.polygons.len(), 4);
    assert_eq!(res2.polygons.len(), 4);

    for (p1, p2) in res1.polygons.iter().zip(res2.polygons.iter()) {
        assert_eq!(p1.exterior.len(), p2.exterior.len());
        assert_eq!(p1.interiors.len(), p2.interiors.len());

        for (c1, c2) in p1.exterior.iter().zip(p2.exterior.iter()) {
            assert_eq!(c1, c2);
        }

        for (h1, h2) in p1.interiors.iter().zip(p2.interiors.iter()) {
            assert_eq!(h1.len(), h2.len());
            for (c1, c2) in h1.iter().zip(h2.iter()) {
                assert_eq!(c1, c2);
            }
        }
    }
}
