use geo_polygonize_core::noding::snap::SnapNoder;
use geo_polygonize_core::{polygonize, Coord3D, Line3D, PolygonizerOptions};

fn fixture(gap: f64) -> Vec<Line3D> {
    [
        ((10., -10.), (0., 0.)),
        ((-10., 1.), (0., -gap)),
        ((10., -10.), (-10., 1.)),
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

#[test]
fn pre_snap_retains_nearby_reference_beyond_moved_endpoint() {
    let a = Coord3D::new(0., 0., 0.);
    let b = Coord3D::new(0., -0.09, 0.);
    for tolerance in [0.1, 0.5] {
        let snapped = SnapNoder::pre_snap_to_reference_vertices(&fixture(0.09), tolerance);
        for id in [101, 102] {
            assert!(snapped.iter().any(|l| l.line_id==id &&
                ((l.start==a && l.end==b)||(l.start==b && l.end==a))),
                "the gap connector must preserve both incident source IDs, tolerance={tolerance}, id={id}, actual={snapped:?}");
        }
    }
}

#[test]
fn cfb_pre_snap_closes_inward_gap_for_all_orders_and_directions() {
    for tolerance in [0.1, 0.5] {
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            for reversed in 0..8 {
                let source = fixture(0.09);
                let lines: Vec<_> = order
                    .iter()
                    .map(|&i| {
                        let l = source[i];
                        if reversed & (1 << i) != 0 {
                            Line3D::new(l.end, l.start, l.line_id)
                        } else {
                            l
                        }
                    })
                    .collect();
                let mut options = PolygonizerOptions::cfb_robust_v1();
                options.pre_snap_tolerance = tolerance;
                let result = polygonize(lines, &options).unwrap();
                assert_eq!(
                    result.polygons.len(),
                    1,
                    "tolerance={tolerance}, order={order:?}, reversed={reversed}"
                );
                assert!((result.polygons[0].unsigned_area_2d() - 44.55).abs() < 1e-10);
                assert!(result.dangles.is_empty());
                assert!(result.cut_edges.is_empty());
                assert!(result.invalid_rings.is_empty());
                assert_eq!(
                    result.polygons[0]
                        .provenance
                        .as_ref()
                        .unwrap()
                        .boundary_line_ids,
                    vec![101, 102, 103]
                );
            }
        }
    }
}

#[test]
fn pre_snap_does_not_extend_to_reference_outside_tolerance() {
    let source = vec![
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(1., 0., 0.), 1),
        Line3D::new(Coord3D::new(1.11, 0., 0.), Coord3D::new(2., 0., 0.), 2),
    ];
    let snapped = SnapNoder::pre_snap_to_reference_vertices(&source, 0.1);
    assert_eq!(snapped.len(), source.len());
    for (actual, expected) in snapped.iter().zip(source) {
        assert_eq!(actual.start, expected.start);
        assert_eq!(actual.end, expected.end);
        assert_eq!(actual.line_id, expected.line_id);
    }
}

#[test]
fn pre_snap_endpoint_extension_preserves_source_z_and_ids() {
    let mut lines = fixture(0.09);
    for (i, line) in lines.iter_mut().enumerate() {
        line.start.z = i as f64 + 3.0;
        line.end.z = i as f64 + 3.0;
    }
    let snapped = SnapNoder::pre_snap_to_reference_vertices(&lines, 0.1);
    for line in snapped {
        let expected_z = (line.line_id - 101) as f64 + 3.0;
        assert_eq!(line.start.z, expected_z);
        assert_eq!(line.end.z, expected_z);
    }
}
