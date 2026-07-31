use geo_polygonize_core::{
    normalize_polygonize_error, polygonize, polygonize_line_strings,
    polygonize_line_strings_with_execution_policy, polygonize_to_multi_polygon,
    polygonize_with_execution_policy, polygonize_with_workspace,
    polygonize_with_workspace_and_execution_policy, Coord3D, DeterminismOptions,
    DiagnosticsOptions, ExecutionPolicy, Line3D, NodingGuarantee, NodingOptions,
    NodingValidationKind, Polygon3D, PolygonizeError, PolygonizerOptions, PolygonizerResult,
    PolygonizerWorkspace, ProvenanceOptions, TopologyFingerprintV1,
};
use geo_types::LineString;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

#[derive(Deserialize)]
struct Fixture {
    options: Option<PolygonizerOptions>,
    inputs: Vec<FixtureLine>,
}

#[derive(Deserialize)]
struct FixtureLine {
    start: FixtureCoordinate,
    end: FixtureCoordinate,
    id: u32,
}

#[derive(Deserialize)]
struct FixtureCoordinate {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Deserialize)]
struct AdapterFixture {
    coords: Vec<f64>,
    offsets: Vec<u32>,
    stride: u8,
    line_ids: Vec<u32>,
    options: PolygonizerOptions,
    expected_fingerprint: Value,
}

fn adapter_fixture() -> (Vec<Line3D>, PolygonizerOptions, Value) {
    let fixture: AdapterFixture = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/conformance/axis_aligned_ring_v1.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(fixture.stride, 2);

    let mut lines = Vec::new();
    for (line_index, &start) in fixture.offsets.iter().enumerate() {
        let end = fixture
            .offsets
            .get(line_index + 1)
            .copied()
            .unwrap_or((fixture.coords.len() / fixture.stride as usize) as u32);
        for point_index in start..end - 1 {
            let i = point_index as usize * fixture.stride as usize;
            let j = i + fixture.stride as usize;
            lines.push(Line3D::new(
                Coord3D::new(fixture.coords[i], fixture.coords[i + 1], 0.0),
                Coord3D::new(fixture.coords[j], fixture.coords[j + 1], 0.0),
                fixture.line_ids[line_index],
            ));
        }
    }
    (lines, fixture.options, fixture.expected_fingerprint)
}

fn fixture(path: &str) -> (Vec<Line3D>, PolygonizerOptions) {
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(path),
        )
        .unwrap(),
    )
    .unwrap();
    let options = PolygonizerOptions {
        determinism: DeterminismOptions {
            canonical_sort: true,
            canonical_ring_rotation: true,
            stable_tie_breaks: true,
        },
        diagnostics: DiagnosticsOptions {
            enabled: true,
            ..Default::default()
        },
        provenance: ProvenanceOptions {
            enabled: true,
            include_boundary_line_ids: true,
        },
        ..fixture.options.unwrap_or_default()
    };
    (
        fixture
            .inputs
            .into_iter()
            .map(|line| {
                Line3D::new(
                    Coord3D::new(line.start.x, line.start.y, line.start.z),
                    Coord3D::new(line.end.x, line.end.y, line.end.z),
                    line.id,
                )
            })
            .collect(),
        options,
    )
}

fn fingerprint(
    result: &geo_polygonize_core::PolygonizerResult,
    options: &PolygonizerOptions,
) -> TopologyFingerprintV1 {
    TopologyFingerprintV1::try_from_result(result, options).unwrap()
}

fn synthetic_polygon(
    exterior: Vec<Coord3D>,
    exterior_ids: Vec<u32>,
    interiors: Vec<Vec<Coord3D>>,
    interiors_ids: Vec<Vec<u32>>,
) -> PolygonizerResult {
    PolygonizerResult {
        polygons: vec![Polygon3D::new(
            exterior,
            interiors,
            exterior_ids,
            interiors_ids,
        )],
        dangles: Vec::new(),
        cut_edges: Vec::new(),
        invalid_rings: Vec::new(),
        diagnostics: None,
    }
}

fn square() -> Vec<Coord3D> {
    vec![
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(2.0, 0.0, 0.0),
        Coord3D::new(2.0, 2.0, 0.0),
        Coord3D::new(0.0, 2.0, 0.0),
        Coord3D::new(0.0, 0.0, 0.0),
    ]
}

#[test]
fn fingerprint_preserves_edge_attribution_rotation_reversal_and_multiplicity() {
    let options = PolygonizerOptions::default();
    let expected = fingerprint(
        &synthetic_polygon(square(), vec![7, 8, 7, 9], Vec::new(), Vec::new()),
        &options,
    );
    assert_eq!(
        expected.polygons[0].exterior_edge_ids,
        ["0x00000007", "0x00000008", "0x00000007", "0x00000009"]
    );

    let rotated = vec![
        Coord3D::new(2.0, 2.0, 0.0),
        Coord3D::new(0.0, 2.0, 0.0),
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(2.0, 0.0, 0.0),
        Coord3D::new(2.0, 2.0, 0.0),
    ];
    assert_eq!(
        expected,
        fingerprint(
            &synthetic_polygon(rotated, vec![7, 9, 7, 8], Vec::new(), Vec::new()),
            &options
        )
    );

    let mut reversed = square();
    reversed.reverse();
    assert_eq!(
        expected,
        fingerprint(
            &synthetic_polygon(reversed, vec![9, 7, 8, 7], Vec::new(), Vec::new()),
            &options
        )
    );

    let different = fingerprint(
        &synthetic_polygon(square(), vec![7, 8, 9, 7], Vec::new(), Vec::new()),
        &options,
    );
    assert_ne!(expected, different);
}

#[test]
fn interior_rings_use_the_same_attribution_canonicalization() {
    let options = PolygonizerOptions::default();
    let result = synthetic_polygon(
        square(),
        vec![7, 8, 7, 9],
        vec![square()],
        vec![vec![7, 8, 7, 9]],
    );
    let report = fingerprint(&result, &options);
    assert_eq!(
        report.polygons[0].exterior_edge_ids,
        report.polygons[0].interiors[0].edge_ids
    );
}

#[test]
fn large_ring_fingerprint_is_linear_space() {
    let vertex_count = 10_001;
    let mut ring: Vec<_> = (0..vertex_count)
        .map(|index| {
            let angle = std::f64::consts::TAU * index as f64 / vertex_count as f64;
            Coord3D::new(angle.cos(), angle.sin(), 0.0)
        })
        .collect();
    ring.push(ring[0]);
    let ids = (0..vertex_count as u32).collect();
    let report = fingerprint(
        &synthetic_polygon(ring, ids, Vec::new(), Vec::new()),
        &PolygonizerOptions::default(),
    );
    assert_eq!(report.polygons[0].exterior.len(), vertex_count + 1);
    assert_eq!(report.polygons[0].exterior_edge_ids.len(), vertex_count);
}

#[test]
fn one_shot_and_workspace_share_the_same_fingerprint() {
    let (lines, options) = fixture("topology/reported_outputs.json");
    let one_shot = fingerprint(&polygonize(lines.clone(), &options).unwrap(), &options);
    let mut workspace = PolygonizerWorkspace::new();
    let reused = fingerprint(
        &polygonize_with_workspace(&lines, &options, &mut workspace).unwrap(),
        &options,
    );
    assert_eq!(one_shot, reused);
}

#[test]
fn shared_adapter_fixture_matches_every_full_result_entrypoint() {
    let (lines, options, expected) = adapter_fixture();
    let policy = ExecutionPolicy::default();
    let one_shot_result = polygonize(lines.clone(), &options).unwrap();
    let one_shot = fingerprint(&one_shot_result, &options);
    assert_eq!(serde_json::to_value(&one_shot).unwrap(), expected);
    assert_eq!(
        one_shot,
        fingerprint(
            &polygonize_with_execution_policy(lines.clone(), &options, &policy).unwrap(),
            &options,
        )
    );

    let mut workspace = PolygonizerWorkspace::new();
    assert_eq!(
        one_shot,
        fingerprint(
            &polygonize_with_workspace(&lines, &options, &mut workspace).unwrap(),
            &options,
        )
    );
    assert_eq!(
        one_shot,
        fingerprint(
            &polygonize_with_workspace_and_execution_policy(
                &lines,
                &options,
                &mut workspace,
                &policy,
            )
            .unwrap(),
            &options,
        )
    );

    let ring = LineString::from(vec![
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 3.0),
        (0.0, 3.0),
        (0.0, 0.0),
    ]);
    assert_eq!(
        one_shot,
        fingerprint(
            &polygonize_line_strings([&ring], &options).unwrap(),
            &options
        )
    );
    assert_eq!(
        one_shot,
        fingerprint(
            &polygonize_line_strings_with_execution_policy([&ring], &options, &policy).unwrap(),
            &options,
        )
    );

    // This convenience entrypoint intentionally retains polygons in XY only.
    assert_eq!(
        polygonize_to_multi_polygon(lines, &options).unwrap(),
        one_shot_result.into_multi_polygon(),
    );
}

#[test]
fn borrowed_georust_input_retains_the_owned_contract() {
    let ring = LineString::from(vec![
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 3.0),
        (0.0, 3.0),
        (0.0, 0.0),
    ]);
    let options = PolygonizerOptions {
        provenance: ProvenanceOptions {
            enabled: true,
            include_boundary_line_ids: true,
        },
        ..Default::default()
    };
    let owned = vec![
        Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(4.0, 0.0, 0.0), 0),
        Line3D::new(Coord3D::new(4.0, 0.0, 0.0), Coord3D::new(4.0, 3.0, 0.0), 0),
        Line3D::new(Coord3D::new(4.0, 3.0, 0.0), Coord3D::new(0.0, 3.0, 0.0), 0),
        Line3D::new(Coord3D::new(0.0, 3.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 0),
    ];
    assert_eq!(
        fingerprint(&polygonize(owned, &options).unwrap(), &options),
        fingerprint(
            &polygonize_line_strings([&ring], &options).unwrap(),
            &options
        ),
    );
}

#[test]
fn permutation_and_feature_builds_keep_the_canonical_fingerprint() {
    let (lines, options) = fixture("topology/reported_outputs.json");
    let expected = fingerprint(&polygonize(lines.clone(), &options).unwrap(), &options);
    let mut permuted = lines;
    permuted.reverse();
    assert_eq!(
        expected,
        fingerprint(&polygonize(permuted, &options).unwrap(), &options)
    );
}

#[test]
fn golden_output_families_and_provenance_are_retained() {
    let (lines, options) = fixture("topology/reported_outputs.json");
    let report = fingerprint(&polygonize(lines, &options).unwrap(), &options);
    assert_eq!(report.polygons.len(), 3);
    assert_eq!(report.dangles.len(), 1);
    assert_eq!(report.cut_edges.len(), 1);
    assert_eq!(report.invalid_rings.len(), 2);
    assert_eq!(
        report.polygons[0]
            .provenance
            .as_ref()
            .unwrap()
            .boundary_line_ids[0],
        "0x0000000000000001"
    );

    let (lines, options) = fixture("provenance/mixed_boundary_with_profile.json");
    let report = fingerprint(&polygonize(lines, &options).unwrap(), &options);
    assert_eq!(report.polygons.len(), 2);
    assert!(report
        .polygons
        .iter()
        .all(|polygon| polygon.provenance.is_some()));
    let (lines, options) = fixture("provenance/mixed_boundary_with_profile.json");
    assert_eq!(
        report,
        fingerprint(&polygonize(lines, &options).unwrap(), &options)
    );
}

#[test]
fn z_output_is_exact_and_negative_zero_is_normalized() {
    let (lines, options) = fixture("z/ignore_conflicts.json");
    let report = fingerprint(&polygonize(lines, &options).unwrap(), &options);
    assert!(report.polygons[0]
        .exterior
        .iter()
        .all(|coordinate| coordinate.z == "0x0000000000000000"));
    let synthetic = geo_polygonize_core::PolygonizerResult {
        polygons: Vec::new(),
        dangles: vec![vec![Coord3D::new(-0.0, -0.0, -0.0)]],
        cut_edges: Vec::new(),
        invalid_rings: Vec::new(),
        diagnostics: None,
    };
    let coordinate = fingerprint(&synthetic, &PolygonizerOptions::default()).dangles[0][0].clone();
    assert_eq!(coordinate.x, "0x0000000000000000");
    assert_eq!(coordinate.y, "0x0000000000000000");
    assert_eq!(coordinate.z, "0x0000000000000000");
}

#[test]
fn validation_and_option_failures_normalize_deterministically() {
    let crossing = vec![
        Line3D::new(Coord3D::new(-1.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 1),
        Line3D::new(Coord3D::new(0.0, -1.0, 0.0), Coord3D::new(0.0, 1.0, 0.0), 2),
    ];
    let validation = polygonize(
        crossing,
        &PolygonizerOptions {
            noding: NodingOptions {
                guarantee: NodingGuarantee::Validate,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap_err();
    let normalized = normalize_polygonize_error(&validation);
    assert_eq!(normalized.code, "interior_intersection");
    assert_eq!(
        normalized.witness.unwrap().ids,
        ["0x0000000000000000", "0x0000000000000001"]
    );

    let options = PolygonizerOptions {
        pre_snap_tolerance: -1.0,
        ..Default::default()
    };
    let first =
        normalize_polygonize_error(&polygonize(Vec::<Line3D>::new(), &options).unwrap_err());
    let second =
        normalize_polygonize_error(&polygonize(Vec::<Line3D>::new(), &options).unwrap_err());
    assert_eq!(first, second);
    assert_eq!(first.field.as_deref(), Some("pre_snap_tolerance"));
}

#[test]
fn normalized_codes_do_not_depend_on_message_wording() {
    let validation = PolygonizeError::NodingValidationFailure {
        first_segment: 1,
        second_segment: 2,
        kind: NodingValidationKind::CollinearOverlap,
        reason: "wording may change freely".to_string(),
    };
    assert_eq!(
        normalize_polygonize_error(&validation).code,
        "collinear_overlap"
    );

    let non_finite = PolygonizeError::NonFiniteCoordinate {
        reason: "localized message".to_string(),
    };
    assert_eq!(
        normalize_polygonize_error(&non_finite).code,
        "non_finite_coordinate"
    );
}

#[test]
fn field_level_diffs_name_the_changed_value() {
    let (lines, options) = fixture("basic/square.json");
    let expected = fingerprint(&polygonize(lines, &options).unwrap(), &options);
    let mut actual = expected.clone();
    actual.polygons[0].exterior[0].x = "0x3ff0000000000000".into();
    let diff = expected.diff(&actual).unwrap();
    assert_eq!(diff.path, "$.polygons[0].exterior[0].x");
    assert_eq!(diff.expected, "0x0000000000000000");
    assert_eq!(diff.actual, "0x3ff0000000000000");
}
