use geo_polygonize_core::{
    polygonize, Coord3D, Line3D, PolygonizerOptions, PrecisionModel, TopologyFingerprintV1,
};

fn fixture() -> Vec<Line3D> {
    [
        ((10., -10.), (0., 0.)),
        ((-10., 1.), (0., -0.09)),
        ((10., -10.), (-10., 1.)),
        ((10., -10.), (3., -4.)),
        ((3., -4.), (2., -4.)),
        ((2., -4.), (10., -10.)),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (a, b))| {
        Line3D::new(
            Coord3D::new(a.0, a.1, 0.),
            Coord3D::new(b.0, b.1, 0.),
            101 + i as u32,
        )
    })
    .collect()
}

fn assert_simple_cycle(ring: &[Coord3D]) {
    assert_eq!(ring.first(), ring.last());
    for (i, vertex) in ring[..ring.len() - 1].iter().enumerate() {
        assert!(!ring[i + 1..ring.len() - 1].contains(vertex));
    }
}

#[test]
fn endpoint_repair_and_point_touch_splitting_preserve_the_same_complete_result() {
    // Closing the outer gap and separating the hole at the opposite corner
    // require both fixes in one polygonization, including exact source metadata.
    for floating in [false, true] {
        for tolerance in [0.1, 0.5] {
            let mut options = PolygonizerOptions::cfb_robust_v1();
            options.pre_snap_tolerance = tolerance;
            if floating {
                options.precision_model = PrecisionModel::Floating;
            }
            if floating || tolerance != 0.5 {
                options.input_profile_id = None;
            }
            let mut expected = None;
            for reverse_order in [false, true] {
                for directions in 0..64 {
                    let mut lines = fixture();
                    for (i, line) in lines.iter_mut().enumerate() {
                        if directions & (1 << i) != 0 {
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
                    assert_eq!(outer.interiors.len(), 1);
                    assert_eq!(outer.exterior.len(), 5);
                    assert_eq!(outer.interiors[0].len(), 4);
                    assert!(inner.interiors.is_empty());
                    assert!((outer.unsigned_area_2d() - 41.55).abs() < 1e-10);
                    assert!((inner.unsigned_area_2d() - 3.).abs() < 1e-10);
                    for ring in [&outer.exterior, &outer.interiors[0], &inner.exterior] {
                        assert_simple_cycle(ring);
                    }
                    assert_eq!(
                        outer.provenance.as_ref().unwrap().boundary_line_ids,
                        vec![101, 102, 103, 104, 105, 106]
                    );
                    assert_eq!(
                        inner.provenance.as_ref().unwrap().boundary_line_ids,
                        vec![104, 105, 106]
                    );
                    assert!(result.dangles.is_empty());
                    assert!(result.cut_edges.is_empty());
                    assert!(result.invalid_rings.is_empty());
                    let fingerprint =
                        TopologyFingerprintV1::try_from_result(&result, &options).unwrap();
                    if let Some(expected) = &expected {
                        assert_eq!(expected, &fingerprint);
                    } else {
                        expected = Some(fingerprint);
                    }
                }
            }
        }
    }
}
