use geo_polygonize_core::{
    contact::{ContactReport, FinalNode},
    Coord3D, Line3D, Polygonizer, PolygonizerOptions, PrecisionModel, SnapStrategy,
};

fn line(a: (f64, f64, f64), b: (f64, f64, f64)) -> Line3D {
    Line3D::new(Coord3D::new(a.0, a.1, a.2), Coord3D::new(b.0, b.1, b.2), 0)
}
fn options(tolerance: f64) -> PolygonizerOptions {
    PolygonizerOptions {
        node_input: true,
        pre_snap_tolerance: tolerance,
        precision_model: PrecisionModel::FixedGrid { grid_size: 0.1 },
        snap_strategy: SnapStrategy::GeosCompat,
        ..Default::default()
    }
}
fn run(lines: &[Line3D], tolerance: f64, budget: usize) -> (serde_json::Value, ContactReport) {
    let mut polygonizer = Polygonizer::with_options(options(tolerance));
    polygonizer.add_lines(lines.to_vec());
    let (result, report) = polygonizer
        .polygonize_with_contact_provenance(budget)
        .unwrap();
    (serde_json::to_value(result).unwrap(), report)
}
fn fixture() -> Vec<Line3D> {
    vec![
        line((0., 0., 7.), (5., 0., 8.)),
        line((5., 0.01, 17.), (10., 0.01, 18.)),
    ]
}
#[test]
fn accepted_incidence_is_not_inferred_from_grid_colocation() {
    let lines = fixture();
    let (_, missing) = run(&lines, 0.001, 100);
    assert!(missing.complete_presnap_acceptances().is_some());
    assert!(!missing
        .accepted_contacts
        .iter()
        .any(|c| c.original_vertex == 1 && c.accepted_source_segment == 1));
    let a = missing
        .accepted_contacts
        .iter()
        .find(|c| c.original_vertex == 1)
        .unwrap();
    let b = missing
        .accepted_contacts
        .iter()
        .find(|c| c.original_vertex == 2)
        .unwrap();
    assert_eq!(
        a.final_node, b.final_node,
        "grid-only co-location is not acceptance"
    );
    let (_, accepted) = run(&lines, 0.02, 100);
    assert!(accepted
        .accepted_contacts
        .iter()
        .any(|c| c.original_vertex == 1 && c.accepted_source_segment == 1));
    assert!(accepted
        .accepted_contacts
        .iter()
        .any(|c| c.original_vertex == 2 && c.accepted_source_segment == 0));
    assert_eq!(accepted.original_vertices[1].raw.z, 8.);
    assert_eq!(accepted.original_vertices[2].raw.z, 17.);
    assert_ne!(
        accepted.original_vertices[1].raw,
        accepted.original_vertices[1].pre_snap_endpoint.unwrap()
    );
}
#[test]
fn opt_in_preserves_geometry_and_default_result_serialization() {
    let lines = fixture();
    let (observed, _) = run(&lines, 0.02, 100);
    let mut ordinary = Polygonizer::with_options(options(0.02));
    ordinary.add_lines(lines);
    assert_eq!(
        observed,
        serde_json::to_value(ordinary.polygonize().unwrap()).unwrap()
    );
    assert!(observed.get("accepted_contacts").is_none());
    // Capture state is removed; repeated ordinary calls stay identical.
    let (_, _) = ordinary.polygonize_with_contact_provenance(100).unwrap();
    assert_eq!(
        observed,
        serde_json::to_value(ordinary.polygonize().unwrap()).unwrap()
    );
}
#[test]
fn truncation_fails_closed_and_has_a_combined_record_bound() {
    for limit in 0..12 {
        let (_, report) = run(&fixture(), 0.02, limit);
        assert!(report.original_vertices.len() + report.accepted_contacts.len() <= limit);
        if report.truncated {
            assert!(report.complete_presnap_acceptances().is_none());
        }
    }
    let (_, report) = run(&fixture(), 0.02, 0);
    assert!(report.truncated);
}
#[test]
fn collapsed_sources_have_explicit_missing_nodes() {
    let lines = vec![line((30., 30., 9.), (30.01, 30., 10.))];
    let (_, report) = run(&lines, 0.0001, 100);
    assert_eq!(report.accepted_contacts.len(), 2);
    assert!(report
        .accepted_contacts
        .iter()
        .all(|c| c.final_node == FinalNode::Missing));
}
#[test]
fn duplicate_ids_and_duplicate_edges_keep_all_accepted_sources() {
    let mut lines = fixture();
    lines.push(lines[0]);
    let (_, report) = run(&lines, 0.02, 100);
    let sources: Vec<_> = report
        .accepted_contacts
        .iter()
        .filter(|c| c.original_vertex == 2)
        .map(|c| c.accepted_source_segment)
        .collect();
    assert_eq!(sources, vec![0, 1, 2]);
    assert!(report
        .accepted_contacts
        .iter()
        .all(|c| matches!(c.final_node, FinalNode::Present { .. })));
}
#[test]
fn lineage_is_repeatable_and_reversal_equivariant() {
    let lines = fixture();
    let (_, first) = run(&lines, 0.02, 100);
    assert_eq!(first, run(&lines, 0.02, 100).1);
    let reversed: Vec<_> = lines
        .iter()
        .rev()
        .map(|l| Line3D::new(l.end, l.start, l.line_id))
        .collect();
    let (_, second) = run(&reversed, 0.02, 100);
    fn canonical(report: &ContactReport, lines: &[Line3D]) -> Vec<String> {
        let mut rows: Vec<_> = report
            .accepted_contacts
            .iter()
            .map(|c| {
                let raw = report.original_vertices[c.original_vertex].raw;
                let source = lines[c.accepted_source_segment];
                let mut endpoints = [format!("{:?}", source.start), format!("{:?}", source.end)];
                endpoints.sort();
                // Node IDs are local, output XY is deterministic.
                format!(
                    "{:?}|{:?}|{:?}|{},{}",
                    raw, endpoints, c.accepted, c.output.x, c.output.y
                )
            })
            .collect();
        rows.sort();
        rows
    }
    assert_eq!(canonical(&first, &lines), canonical(&second, &reversed));
}

#[test]
fn unrelated_surviving_node_does_not_prove_collapsed_source_incidence() {
    use geo_polygonize_core::contact::FinalSourceIncidence;
    let lines = vec![
        line((0., 0., 1.), (0.01, 0., 2.)), // Collapses completely at grid 0.1.
        line((0., 0., 3.), (10., 0., 4.)),
    ];
    let (_, report) = run(&lines, 0.0001, 100);
    let contacts: Vec<_> = report
        .accepted_contacts
        .iter()
        .filter(|c| c.accepted_source_segment == 0)
        .collect();
    assert!(!contacts.is_empty());
    assert!(contacts
        .iter()
        .all(|c| matches!(c.final_node, FinalNode::Present { .. })));
    assert!(contacts
        .iter()
        .all(|c| c.final_source_incidence == FinalSourceIncidence::Absent));
    assert_eq!(report.original_vertices[0].raw.z, 1.);
    assert_eq!(report.original_vertices[2].raw.z, 3.);
}

#[test]
fn pre_pruning_node_status_does_not_claim_dangling_source_survives() {
    use geo_polygonize_core::contact::FinalSourceIncidence;
    let lines = vec![
        line((0., 0., 0.), (10., 0., 0.)),
        line((10., 0., 0.), (10., 10., 0.)),
        line((10., 10., 0.), (0., 10., 0.)),
        line((0., 10., 0.), (0., 0., 0.)),
        line((5., 0.01, 0.), (5., 5., 0.)),
    ];
    let (geometry, report) = run(&lines, 0.02, 1000);
    assert!(!geometry["polygons"].as_array().unwrap().is_empty());
    assert!(!geometry["dangles"].as_array().unwrap().is_empty());
    let contact = report
        .accepted_contacts
        .iter()
        .find(|c| c.original_vertex == 8 && c.accepted_source_segment == 4)
        .unwrap();
    assert!(matches!(contact.final_node, FinalNode::Present { .. }));
    assert_eq!(
        contact.final_source_incidence,
        FinalSourceIncidence::Present {
            polygon_boundary: 0,
            dangle: 1,
            cut_edge: 0,
            invalid_ring: 0,
            other: 0,
        }
    );
}

#[test]
fn final_noding_splits_do_not_erase_recorded_pre_snap_contacts() {
    let lines = vec![
        line((0., 0., 0.), (10., 0., 0.)),
        line((5., 0.01, 0.), (5., 2., 0.)),
        line((7., -1., 0.), (7., 1., 0.)),
    ];
    let (observed, report) = run(&lines, 0.02, 1000);
    let contact = report
        .accepted_contacts
        .iter()
        .find(|c| c.original_vertex == 2 && c.accepted_source_segment == 0)
        .unwrap();
    assert!(matches!(contact.final_node, FinalNode::Present { .. }));
    let mut ordinary = Polygonizer::with_options(options(0.02));
    ordinary.add_lines(lines);
    assert_eq!(
        observed,
        serde_json::to_value(ordinary.polygonize().unwrap()).unwrap()
    );
}

#[test]
fn raw_acceptance_absence_does_not_mean_moved_endpoint_node_absence() {
    let lines = vec![
        line((0., 0., 1.), (1., 0., 2.)),
        line((0.5, 0.1, 3.), (0.5, 10., 4.)),
    ];
    let (_, report) = run(&lines, 0.6, 100);
    assert!(report.complete_presnap_acceptances().is_some());
    assert!(!report
        .accepted_contacts
        .iter()
        .any(|c| c.accepted_source_segment == 0));
    assert!(!report
        .accepted_contacts
        .iter()
        .any(|c| c.original_vertex == 1));
    assert_eq!(
        report.original_vertices[1].pre_snap_endpoint.unwrap().x,
        0.5
    );
    assert!(matches!(
        report.original_vertices[1].endpoint_node,
        FinalNode::Present { .. }
    ));
}

#[test]
fn signed_zero_contacts_use_equal_xy_but_keep_raw_bits() {
    let lines = vec![
        line((-0., 0., 1.), (1., 0., 2.)),
        line((0., 0., 3.), (0., 1., 4.)),
    ];
    let (_, report) = run(&lines, 0.0001, 100);
    assert_eq!(
        report.original_vertices[0].raw.x.to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(
        report.original_vertices[2].raw.x.to_bits(),
        0.0_f64.to_bits()
    );
    for source in [0, 1] {
        assert!(report
            .accepted_contacts
            .iter()
            .any(|c| c.original_vertex == 0 && c.accepted_source_segment == source));
        assert!(report
            .accepted_contacts
            .iter()
            .any(|c| c.original_vertex == 2 && c.accepted_source_segment == source));
    }
}
