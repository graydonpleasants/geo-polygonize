use geo_polygonize_core::{polygonize, Coord3D, Line3D, PolygonizerOptions};

fn fixture() -> Vec<Line3D> {
    [
        ((-10., 10.), (0., 0.)),
        ((0., 0.), (0., -10.)),
        ((0., -10.), (-10., 10.)),
        ((0., 0.), (-2., 1.)),
        ((-2., 1.), (-2., -1.)),
        ((-2., -1.), (0., 0.)),
    ]
    .iter()
    .enumerate()
    .map(|(i, &(a, b))| {
        Line3D::new(
            Coord3D::new(a.0, a.1, 0.),
            Coord3D::new(b.0, b.1, 0.),
            i as u32 + 101,
        )
    })
    .collect()
}

fn assert_simple_triangle(ring: &[Coord3D]) {
    assert_eq!(ring.len(), 4, "ring must be a simple triangle: {ring:?}");
    assert_eq!(ring[0], ring[3]);
    for i in 0..3 {
        for j in i + 1..3 {
            assert_ne!(ring[i], ring[j], "a minimal ring must not revisit a vertex");
        }
    }
}

#[test]
fn nested_point_touch_extracts_owned_hole_and_simple_shell() {
    for mode in 0..4 {
        let mut options = PolygonizerOptions::cfb_robust_v1();
        if mode < 3 {
            options.pre_snap_tolerance = 0.;
            if mode < 2 {
                options.precision_model = serde_json::from_str(r#"{"type":"floating"}"#).unwrap();
            }
            options.node_input = mode != 0;
        }
        // Reversing the feature order and every independent edge direction must not
        // change the split at the shared vertex. Source IDs remain attached to edges.
        for reverse_order in [false, true] {
            for reverse_edges in 0..64 {
                let mut lines = fixture();
                for (i, line) in lines.iter_mut().enumerate() {
                    if reverse_edges & (1 << i) != 0 {
                        std::mem::swap(&mut line.start, &mut line.end);
                    }
                }
                if reverse_order {
                    lines.reverse();
                }
                let result = polygonize(lines, &options).unwrap();
                assert_eq!(result.polygons.len(), 2);
                let outer = result
                    .polygons
                    .iter()
                    .find(|p| p.unsigned_area_2d() > 10.)
                    .unwrap();
                let inner = result
                    .polygons
                    .iter()
                    .find(|p| p.unsigned_area_2d() < 10.)
                    .unwrap();
                assert_eq!(
                    outer.interiors.len(),
                    1,
                    "mode={mode}, order={reverse_order}, directions={reverse_edges}"
                );
                assert_eq!(outer.unsigned_area_2d(), 48.);
                assert_eq!(inner.unsigned_area_2d(), 2.);
                assert_simple_triangle(&outer.exterior);
                assert_simple_triangle(&outer.interiors[0]);
                assert_simple_triangle(&inner.exterior);
                assert_eq!(
                    outer.provenance.as_ref().unwrap().boundary_line_ids,
                    vec![101, 102, 103, 104, 105, 106]
                );
                assert_eq!(
                    inner.provenance.as_ref().unwrap().boundary_line_ids,
                    vec![104, 105, 106]
                );
                assert!(result.invalid_rings.is_empty());
                assert!(result.dangles.is_empty());
                assert!(result.cut_edges.is_empty());
            }
        }
    }
}

#[test]
fn bowtie_separates_minimal_outer_rings_without_changing_bounded_faces() {
    let points = [(0., 0.), (10., 10.), (10., 0.), (0., 10.), (0., 0.)];
    let lines: Vec<Line3D> = points
        .windows(2)
        .enumerate()
        .map(|(i, p)| {
            Line3D::new(
                Coord3D::new(p[0].0, p[0].1, 0.),
                Coord3D::new(p[1].0, p[1].1, 0.),
                i as u32 + 1,
            )
        })
        .collect();
    let options: PolygonizerOptions = serde_json::from_str(
        r#"{
        "node_input": true,
        "precision_model": {"type":"fixed_grid","grid_size":0.000001},
        "diagnostics": {"enabled":true}
    }"#,
    )
    .unwrap();
    let result = polygonize(lines, &options).unwrap();
    assert_eq!(result.polygons.len(), 2);
    for polygon in &result.polygons {
        assert_eq!(polygon.unsigned_area_2d(), 25.);
        assert_simple_triangle(&polygon.exterior);
        assert!(polygon.interiors.is_empty());
    }
    let diagnostics = result.diagnostics.as_ref().unwrap();
    assert_eq!(diagnostics.ring_count, 4);
    assert_eq!(diagnostics.hole_count, 2);
    assert_eq!(diagnostics.unassigned_hole_count, 2);
    assert_eq!(diagnostics.unassigned_hole_area, 50.);
}
