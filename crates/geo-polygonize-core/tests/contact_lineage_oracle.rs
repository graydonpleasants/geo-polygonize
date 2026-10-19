// Independent review oracle: exact axis-aligned interval membership, not lineage internals.
use geo_polygonize_core::*;
fn lines(points: &[(f64, f64)], id: u32) -> Vec<Line3D> {
    points
        .windows(2)
        .map(|p| {
            Line3D::new(
                Coord3D::new(p[0].0, p[0].1, id as f64),
                Coord3D::new(p[1].0, p[1].1, id as f64 + 1.),
                id,
            )
        })
        .collect()
}
fn check_refs(
    r: &geo_polygonize_core::PolygonizerResult,
    c: &geo_polygonize_core::contact::ContactReport,
) {
    let mut seen = std::collections::BTreeSet::new();
    for e in &c.source_edges {
        assert_eq!(e.polygon_boundary, !e.output_boundaries.is_empty());
        for b in &e.output_boundaries {
            let p = &r.polygons[b.polygon_index];
            let ring = b
                .interior_ring
                .map(|i| &p.interiors[i])
                .unwrap_or(&p.exterior);
            let a = ring[b.edge_index].to_coord_2d();
            let z = ring[b.edge_index + 1].to_coord_2d();
            assert!(
                (a == e.start.to_coord_2d() && z == e.end.to_coord_2d())
                    || (z == e.start.to_coord_2d() && a == e.end.to_coord_2d())
            );
            assert!(seen.insert((b.polygon_index, b.interior_ring, b.edge_index)));
        }
    }
    for (i, p) in r.polygons.iter().enumerate() {
        for j in 0..p.exterior.len() - 1 {
            assert!(seen.contains(&(i, None, j)));
        }
        for (k, h) in p.interiors.iter().enumerate() {
            for j in 0..h.len() - 1 {
                assert!(seen.contains(&(i, Some(k), j)));
            }
        }
    }
}
fn boundary_budget() {
    let mut input = Vec::new();
    for i in 0..10 {
        input.extend(lines(&[(i as f64, 0.), (i as f64, 9.)], 0));
        input.extend(lines(&[(0., i as f64), (9., i as f64)], 0));
    }
    let opts = PolygonizerOptions {
        node_input: true,
        pre_snap_tolerance: 0.000001,
        ..Default::default()
    };
    let mut p = Polygonizer::with_options(opts);
    p.add_lines(input);
    let (r, c) = p.polygonize_with_contact_provenance(200).unwrap();
    assert_eq!(r.polygons.len(), 81);
    assert!(!c.truncated);
    assert_eq!(c.lineage_status, contact::LineageStatus::LimitExceeded);
    assert!(c.source_edges.is_empty());
    assert!(c.complete_source_incidence().is_none());
    assert!(c.complete_output_classification().is_none());
    assert!(c
        .accepted_contacts
        .iter()
        .all(|v| v.final_source_incidence == contact::FinalSourceIncidence::NotTracked));
    println!("isolated output-boundary reference overflow fails closed: 180 memberships < 200 cap < 324 boundary refs");
}
fn run(name: &str, l: Vec<Line3D>, opts: PolygonizerOptions) {
    let mut p = Polygonizer::with_options(opts.clone());
    p.add_lines(l.clone());
    let (r, c) = p.polygonize_with_contact_provenance(100000).unwrap();
    check_refs(&r, &c);
    let mut q = Polygonizer::with_options(opts);
    q.add_lines(l);
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        serde_json::to_value(q.polygonize().unwrap()).unwrap()
    );
    println!(
        "{name}: polygons={} invalid={} cuts={} dangles={} edges={} unclassified={} complete={:?}",
        r.polygons.len(),
        r.invalid_rings.len(),
        r.cut_edges.len(),
        r.dangles.len(),
        c.source_edges.len(),
        c.source_edges
            .iter()
            .filter(|e| !(e.polygon_boundary || e.invalid_ring || e.dangle || e.cut_edge))
            .count(),
        c.lineage_status
    );
    assert_eq!(
        c.complete_output_classification().is_some(),
        name != "filtered"
    );
}
fn fuzz() {
    let mut seed = 42u64;
    for case in 0..300 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as u32
        };
        let mut input = Vec::new();
        for _ in 0..15 {
            let x = (next() % 10) as f64;
            let y = (next() % 10) as f64;
            let v = (next() % 10) as f64;
            let (ex, ey) = if next() % 2 == 0 { (x, v) } else { (v, y) };
            if x != ex || y != ey {
                input.push(Line3D::new(
                    Coord3D::new(x, y, next() as f64),
                    Coord3D::new(ex, ey, next() as f64),
                    next() % 3,
                ));
            }
        }
        let opts = PolygonizerOptions {
            node_input: true,
            pre_snap_tolerance: 0.000001,
            precision_model: PrecisionModel::FixedGrid { grid_size: 0.01 },
            ..Default::default()
        };
        let mut p = Polygonizer::with_options(opts.clone());
        p.add_lines(input.clone());
        let (r, c) = p.polygonize_with_contact_provenance(100000).unwrap();
        assert!(c.complete_source_incidence().is_some());
        check_refs(&r, &c);
        let mut q = Polygonizer::with_options(opts);
        q.add_lines(input.clone());
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            serde_json::to_value(q.polygonize().unwrap()).unwrap()
        );
        for e in &c.source_edges {
            let on = |p: Coord3D, l: &Line3D| {
                (l.start.x == l.end.x
                    && p.x == l.start.x
                    && p.y >= l.start.y.min(l.end.y)
                    && p.y <= l.start.y.max(l.end.y))
                    || (l.start.y == l.end.y
                        && p.y == l.start.y
                        && p.x >= l.start.x.min(l.end.x)
                        && p.x <= l.start.x.max(l.end.x))
            };
            let expected: Vec<usize> = input
                .iter()
                .enumerate()
                .filter(|(_, l)| on(e.start, l) && on(e.end, l))
                .map(|(i, _)| i)
                .collect();
            assert_eq!(e.source_segments, expected, "case {case} edge {e:?}");
        }
    }
    println!("300 axis-aligned randomized overlap/source-ID/reversal fixtures passed exact source-membership oracle and output equality");
}
#[test]
fn independent_membership_and_boundary_reference_oracle() {
    boundary_budget();
    fuzz();
    let opts = PolygonizerOptions {
        node_input: true,
        pre_snap_tolerance: 0.00001,
        precision_model: PrecisionModel::FixedGrid { grid_size: 0.001 },
        ..Default::default()
    };
    let shell = lines(
        &[
            (0., 0.),
            (5., 0.),
            (10., 0.),
            (10., 10.),
            (0., 10.),
            (0., 0.),
        ],
        0,
    );
    let hole = lines(
        &[(2., 2.), (4., 2.), (6., 2.), (6., 6.), (2., 6.), (2., 2.)],
        0,
    );
    run("collinear", shell.clone(), opts.clone());
    let mut adjacent = lines(&[(0., 0.), (5., 0.), (5., 5.), (0., 5.), (0., 0.)], 0);
    adjacent.extend(lines(
        &[(5., 0.), (10., 0.), (10., 5.), (5., 5.), (5., 0.)],
        1,
    ));
    run("shared_two_faces", adjacent, opts.clone());
    let mut both = shell.clone();
    both.extend(hole);
    run("hole", both.clone(), opts.clone());
    let mut poly = opts.clone();
    poly.extract_only_polygonal = true;
    run("hole_onlypoly", both, poly);
    let mut filtered = opts.clone();
    filtered.output_filter.minimum_face_area = Some(101.);
    run("filtered", shell.clone(), filtered);
    let mut duplicate = shell.clone();
    duplicate.extend(shell.iter().map(|l| Line3D::new(l.end, l.start, 42)));
    run("differentids", duplicate, opts.clone());
    run(
        "selftouch",
        lines(&[(0., 0.), (4., 4.), (0., 4.), (4., 0.), (0., 0.)], 7),
        opts.clone(),
    );
    run(
        "chain",
        lines(&[(0., 0.), (1., 0.), (2., 0.), (3., 0.)], 0),
        opts,
    );
}
