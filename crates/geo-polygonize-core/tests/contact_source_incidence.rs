//! Stage-two guarantees: exact original-source membership survives final noding.
use geo_polygonize_core::{
    contact::{AcceptedContact, ContactReport, FinalNode, FinalSourceIncidence},
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

// Every fixture also checks that opt-in instrumentation leaves the public output alone.
fn run(lines: &[Line3D], options: PolygonizerOptions, budget: usize) -> ContactReport {
    run_output(lines, options, budget).1
}

fn run_output(
    lines: &[Line3D],
    options: PolygonizerOptions,
    budget: usize,
) -> (serde_json::Value, ContactReport) {
    let mut instrumented = Polygonizer::with_options(options.clone());
    instrumented.add_lines(lines.to_vec());
    let (result, report) = instrumented
        .polygonize_with_contact_provenance(budget)
        .unwrap();
    let mut ordinary = Polygonizer::with_options(options);
    ordinary.add_lines(lines.to_vec());
    let mut actual = serde_json::to_value(result).unwrap();
    let mut expected = serde_json::to_value(ordinary.polygonize().unwrap()).unwrap();
    // Wall-clock phase times vary even when all deterministic diagnostics agree.
    for value in [&mut actual, &mut expected] {
        if let Some(diagnostics) = value.get_mut("diagnostics").and_then(|d| d.as_object_mut()) {
            diagnostics.remove("phase_times");
        }
    }
    assert_eq!(actual, expected);
    (actual, report)
}

fn contact(report: &ContactReport, original: usize, source: usize) -> &AcceptedContact {
    report
        .accepted_contacts
        .iter()
        .find(|c| c.original_vertex == original && c.accepted_source_segment == source)
        .unwrap_or_else(|| panic!("missing acceptance for original {original}, source {source}"))
}

#[test]
fn collapsed_source_is_absent_even_at_an_unrelated_surviving_node() {
    let lines = vec![
        line((0., 0., 1.), (0.01, 0., 2.)),
        line((0., 0., 3.), (10., 0., 4.)),
    ];
    let report = run(&lines, options(0.0001), 100);
    assert!(report.complete_presnap_acceptances().is_some());
    assert!(report.complete_source_incidence().is_some());
    let collapsed: Vec<_> = report
        .accepted_contacts
        .iter()
        .filter(|c| c.accepted_source_segment == 0)
        .collect();
    assert!(!collapsed.is_empty());
    for c in collapsed {
        assert!(matches!(c.final_node, FinalNode::Present { .. }));
        assert_eq!(c.final_source_incidence, FinalSourceIncidence::Absent);
    }
    assert!(matches!(
        contact(&report, 2, 1).final_source_incidence,
        FinalSourceIncidence::Present {
            dangle: 1,
            polygon_boundary: 0,
            ..
        }
    ));
}

#[test]
fn isolated_collapsed_source_has_complete_absence_and_no_node() {
    let report = run(
        &[line((30., 30., 9.), (30.01, 30., 10.))],
        options(0.0001),
        100,
    );
    assert!(report.complete_source_incidence().is_some());
    assert_eq!(report.accepted_contacts.len(), 2);
    for c in &report.accepted_contacts {
        assert_eq!(c.final_node, FinalNode::Missing);
        assert_eq!(c.final_source_incidence, FinalSourceIncidence::Absent);
    }
}

#[test]
fn dangling_source_does_not_inherit_boundary_classification_at_a_shared_node() {
    let lines = vec![
        line((0., 0., 0.), (10., 0., 0.)),
        line((10., 0., 0.), (10., 10., 0.)),
        line((10., 10., 0.), (0., 10., 0.)),
        line((0., 10., 0.), (0., 0., 0.)),
        line((5., 0.01, 23.), (5., 5., 29.)),
    ];
    let report = run(&lines, options(0.02), 1000);
    assert!(report.complete_source_incidence().is_some());
    let dangling = contact(&report, 8, 4);
    assert!(matches!(dangling.final_node, FinalNode::Present { .. }));
    assert_eq!(
        dangling.final_source_incidence,
        FinalSourceIncidence::Present {
            polygon_boundary: 0,
            dangle: 1,
            cut_edge: 0,
            invalid_ring: 0,
            other: 0,
        }
    );
    assert_eq!(
        contact(&report, 8, 0).final_source_incidence,
        FinalSourceIncidence::Present {
            polygon_boundary: 2,
            dangle: 0,
            cut_edge: 0,
            invalid_ring: 0,
            other: 0,
        }
    );
}

#[test]
fn duplicate_default_ids_keep_exact_sources_after_deduplication() {
    let lines = vec![
        line((0., 0., 1.), (0.01, 0., 2.)), // Same ID and node, but no surviving edge.
        line((0., 0., 3.), (10., 0., 4.)),
        line((10., 0., 17.), (0., 0., 18.)), // Reversed duplicate, different raw Z.
    ];
    let report = run(&lines, options(0.0001), 1000);
    assert!(report.complete_source_incidence().is_some());
    for original in [0, 2, 5] {
        assert_eq!(
            contact(&report, original, 0).final_source_incidence,
            FinalSourceIncidence::Absent
        );
        for source in [1, 2] {
            assert_eq!(
                contact(&report, original, source).final_source_incidence,
                FinalSourceIncidence::Present {
                    polygon_boundary: 0,
                    dangle: 1,
                    cut_edge: 0,
                    invalid_ring: 0,
                    other: 0,
                }
            );
        }
    }
    assert_eq!(report.original_vertices[0].raw.z, 1.);
    assert_eq!(report.original_vertices[2].raw.z, 3.);
    assert_eq!(report.original_vertices[5].raw.z, 18.);
}

#[test]
fn final_intersection_splits_preserve_both_incident_source_fragments() {
    let lines = vec![
        line((0., 0., 0.), (10., 0., 0.)),
        line((5., 0.01, 7.), (5., 2., 9.)),
        line((7., -1., 13.), (7., 1., 15.)),
    ];
    let report = run(&lines, options(0.02), 1000);
    assert!(report.complete_source_incidence().is_some());
    assert_eq!(
        contact(&report, 2, 0).final_source_incidence,
        FinalSourceIncidence::Present {
            polygon_boundary: 0,
            dangle: 2,
            cut_edge: 0,
            invalid_ring: 0,
            other: 0,
        }
    );
    assert_eq!(
        contact(&report, 0, 0).final_source_incidence,
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
fn source_incidence_is_repeatable_and_reversal_equivariant_with_distinct_z() {
    let lines = vec![
        line((0., 0., 1.), (10., 0., 2.)),
        line((5., 0.01, 17.), (5., 2., 18.)),
        line((7., -1., 23.), (7., 1., 24.)),
    ];
    let first = run(&lines, options(0.02), 1000);
    assert_eq!(first, run(&lines, options(0.02), 1000));
    let reversed: Vec<_> = lines
        .iter()
        .rev()
        .map(|l| Line3D::new(l.end, l.start, l.line_id))
        .collect();
    let second = run(&reversed, options(0.02), 1000);
    fn canonical(report: &ContactReport, lines: &[Line3D]) -> Vec<String> {
        let mut rows: Vec<_> = report
            .accepted_contacts
            .iter()
            .map(|c| {
                let mut endpoints = [
                    format!("{:?}", lines[c.accepted_source_segment].start),
                    format!("{:?}", lines[c.accepted_source_segment].end),
                ];
                endpoints.sort();
                format!(
                    "{:?}|{:?}|{},{}|{:?}",
                    report.original_vertices[c.original_vertex].raw,
                    endpoints,
                    c.output.x,
                    c.output.y,
                    c.final_source_incidence
                )
            })
            .collect();
        rows.sort();
        rows
    }
    assert_eq!(canonical(&first, &lines), canonical(&second, &reversed));
}

#[test]
fn source_completeness_fails_closed_when_capture_budget_is_exhausted() {
    let lines = vec![
        line((0., 0., 0.), (10., 0., 0.)),
        line((5., 0.01, 1.), (5., 5., 2.)),
    ];
    for budget in 0..30 {
        let report = run(&lines, options(0.02), budget);
        assert!(report.original_vertices.len() + report.accepted_contacts.len() <= budget);
        if report.truncated {
            assert!(report.complete_presnap_acceptances().is_none());
            assert!(report.complete_source_incidence().is_none());
        } else {
            assert!(report.complete_presnap_acceptances().is_some());
            assert!(report.complete_source_incidence().is_some());
            assert!(report
                .accepted_contacts
                .iter()
                .all(|c| c.final_source_incidence != FinalSourceIncidence::NotTracked));
        }
    }
}

#[test]
fn one_source_can_have_boundary_and_dangle_fragments_at_the_same_node() {
    let lines = vec![
        line((-5., 0., 0.), (15., 0., 0.)),
        line((0., 0., 0.), (0., 10., 0.)),
        line((0., 10., 0.), (10., 10., 0.)),
        line((10., 10., 0.), (10., 0., 0.)),
    ];
    let report = run(&lines, options(0.0001), 1000);
    assert!(report.complete_source_incidence().is_some());
    for original in [2, 7] {
        assert_eq!(
            contact(&report, original, 0).final_source_incidence,
            FinalSourceIncidence::Present {
                polygon_boundary: 1,
                dangle: 1,
                cut_edge: 0,
                invalid_ring: 0,
                other: 0,
            }
        );
    }
}

#[test]
fn bridge_source_is_a_cut_edge_rather_than_its_neighbors_boundary() {
    let lines = vec![
        line((0., 0., 0.), (2., 0., 0.)),
        line((2., 0., 0.), (2., 2., 0.)),
        line((2., 2., 0.), (0., 2., 0.)),
        line((0., 2., 0.), (0., 0., 0.)),
        line((4., 0., 0.), (6., 0., 0.)),
        line((6., 0., 0.), (6., 2., 0.)),
        line((6., 2., 0.), (4., 2., 0.)),
        line((4., 2., 0.), (4., 0., 0.)),
        line((2., 0., 0.), (4., 0., 0.)),
    ];
    let report = run(&lines, options(0.0001), 1000);
    assert!(report.complete_source_incidence().is_some());
    for original in [16, 17] {
        assert_eq!(
            contact(&report, original, 8).final_source_incidence,
            FinalSourceIncidence::Present {
                polygon_boundary: 0,
                dangle: 0,
                cut_edge: 1,
                invalid_ring: 0,
                other: 0,
            }
        );
    }
}

#[test]
fn output_filter_preserves_membership_but_fails_closed_for_classification() {
    let square = vec![
        line((0., 0., 0.), (10., 0., 0.)),
        line((10., 0., 0.), (10., 10., 0.)),
        line((10., 10., 0.), (0., 10., 0.)),
        line((0., 10., 0.), (0., 0., 0.)),
    ];
    let ordinary = run(&square, options(0.0001), 1000);
    assert!(ordinary.complete_source_incidence().is_some());
    assert!(ordinary.complete_output_classification().is_some());
    let mut filtered_options = options(0.0001);
    filtered_options.output_filter.minimum_face_area = Some(101.);
    let filtered = run(&square, filtered_options, 1000);
    assert!(filtered.complete_presnap_acceptances().is_some());
    assert!(filtered.complete_source_incidence().is_some());
    assert!(filtered.complete_output_classification().is_none());
    assert_eq!(
        contact(&filtered, 0, 0).final_source_incidence,
        FinalSourceIncidence::Present {
            polygon_boundary: 0,
            dangle: 0,
            cut_edge: 0,
            invalid_ring: 0,
            other: 1,
        }
    );
}

#[test]
fn final_split_lineage_budget_can_fail_independently_of_presnap_capture() {
    use geo_polygonize_core::contact::LineageStatus;
    // Twenty input lines yield 40 originals and 40 endpoint acceptances, but
    // their 100 interior crossings create 220 source-bearing fragments.
    let mut lines = Vec::new();
    for i in 1..=10 {
        let coordinate = i as f64;
        lines.push(line((0., coordinate, 0.), (11., coordinate, 0.)));
        lines.push(line((coordinate, 0., 0.), (coordinate, 11., 0.)));
    }
    let report = run(&lines, options(0.0001), 100);
    assert!(!report.truncated);
    assert_eq!(report.original_vertices.len(), 40);
    assert_eq!(report.accepted_contacts.len(), 40);
    assert!(report.complete_presnap_acceptances().is_some());
    assert_eq!(report.lineage_status, LineageStatus::LimitExceeded);
    assert!(report.complete_source_incidence().is_none());
    assert!(report.complete_output_classification().is_none());
    assert!(report
        .accepted_contacts
        .iter()
        .all(|c| c.final_source_incidence == FinalSourceIncidence::NotTracked));

    let sufficient = run(&lines, options(0.0001), 1000);
    assert!(sufficient.complete_source_incidence().is_some());
    assert!(sufficient.complete_output_classification().is_some());
}

fn square_at(x: f64) -> Vec<Line3D> {
    vec![
        line((x, 0., 0.), (x + 10., 0., 0.)),
        line((x + 10., 0., 0.), (x + 10., 10., 0.)),
        line((x + 10., 10., 0.), (x, 10., 0.)),
        line((x, 10., 0.), (x, 0., 0.)),
    ]
}

fn verify_boundary_references(result: &serde_json::Value, report: &ContactReport) {
    assert!(report.complete_output_classification().is_some());
    let polygons = result["polygons"].as_array().unwrap();
    let mut referenced = std::collections::BTreeSet::new();
    for edge in &report.source_edges {
        assert_eq!(edge.polygon_boundary, !edge.output_boundaries.is_empty());
        for reference in &edge.output_boundaries {
            let polygon = &polygons[reference.polygon_index];
            let ring = match reference.interior_ring {
                None => &polygon["exterior"],
                Some(index) => &polygon["interiors"][index],
            }
            .as_array()
            .unwrap();
            assert!(
                reference.edge_index + 1 < ring.len(),
                "reference indexes an actual ring edge"
            );
            let xy = |p: &serde_json::Value| (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap());
            let a = xy(&ring[reference.edge_index]);
            let b = xy(&ring[reference.edge_index + 1]);
            let start = (edge.start.x, edge.start.y);
            let end = (edge.end.x, edge.end.y);
            assert!((a == start && b == end) || (a == end && b == start));
            referenced.insert((
                reference.polygon_index,
                reference.interior_ring,
                reference.edge_index,
            ));
        }
    }
    for (polygon_index, polygon) in polygons.iter().enumerate() {
        for edge_index in 0..polygon["exterior"].as_array().unwrap().len() - 1 {
            assert!(referenced.contains(&(polygon_index, None, edge_index)));
        }
        for (interior_ring, ring) in polygon["interiors"].as_array().unwrap().iter().enumerate() {
            for edge_index in 0..ring.as_array().unwrap().len() - 1 {
                assert!(referenced.contains(&(polygon_index, Some(interior_ring), edge_index)));
            }
        }
    }
}

#[test]
fn exact_output_boundary_ownership_is_independent_of_duplicate_caller_ids() {
    let mut lines = square_at(0.);
    lines.extend(square_at(20.));
    let (result, report) = run_output(&lines, options(0.0001), 1000);
    assert_eq!(result["polygons"].as_array().unwrap().len(), 2);
    verify_boundary_references(&result, &report);
    let c = contact(&report, 0, 0);
    let FinalNode::Present { node_id, .. } = c.final_node else {
        panic!("missing final node")
    };
    let target_polygons: std::collections::BTreeSet<_> = report
        .source_edges
        .iter()
        .filter(|edge| {
            edge.node_ids.contains(&node_id)
                && edge.source_segments.contains(&c.accepted_source_segment)
        })
        .flat_map(|edge| {
            edge.output_boundaries
                .iter()
                .map(|reference| reference.polygon_index)
        })
        .collect();
    assert_eq!(target_polygons.len(), 1);
    let target = *target_polygons.iter().next().unwrap();
    assert!(result["polygons"][target]["exterior"]
        .as_array()
        .unwrap()
        .iter()
        .all(|point| point["x"].as_f64().unwrap() <= 10.));
    for edge in &report.source_edges {
        for reference in &edge.output_boundaries {
            let left = result["polygons"][reference.polygon_index]["exterior"]
                .as_array()
                .unwrap()
                .iter()
                .all(|point| point["x"].as_f64().unwrap() <= 10.);
            assert!(edge
                .source_segments
                .iter()
                .all(|source| (*source < 4) == left));
        }
    }
}

#[test]
fn shared_boundary_edge_references_both_output_polygons() {
    let mut lines = square_at(0.);
    lines.extend(square_at(10.));
    let (result, report) = run_output(&lines, options(0.0001), 1000);
    assert_eq!(result["polygons"].as_array().unwrap().len(), 2);
    verify_boundary_references(&result, &report);
    let edge = report
        .source_edges
        .iter()
        .find(|edge| edge.source_segments.contains(&1) && edge.source_segments.contains(&7))
        .unwrap();
    assert_eq!(edge.output_boundaries.len(), 2);
    assert_ne!(
        edge.output_boundaries[0].polygon_index,
        edge.output_boundaries[1].polygon_index
    );
}

#[test]
fn boundary_references_distinguish_interior_rings_from_exteriors() {
    let mut lines = square_at(0.);
    lines.extend([
        line((2., 2., 0.), (8., 2., 0.)),
        line((8., 2., 0.), (8., 8., 0.)),
        line((8., 8., 0.), (2., 8., 0.)),
        line((2., 8., 0.), (2., 2., 0.)),
    ]);
    let (result, report) = run_output(&lines, options(0.0001), 1000);
    verify_boundary_references(&result, &report);
    assert!(report
        .source_edges
        .iter()
        .flat_map(|edge| &edge.output_boundaries)
        .any(|reference| reference.interior_ring.is_some()));
}
