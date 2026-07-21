use geo_polygonize_core::noding::snap::SnapNoder;
use geo_polygonize_core::{
    polygonize, Coord3D, DiagnosticsOptions, Line3D, PolygonizeError, PolygonizerOptions,
    ProvenanceOptions, ZOptions, ZPolicy,
};

fn conflicting_square() -> Vec<Line3D> {
    vec![
        Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 10),
        Line3D::new(
            Coord3D::new(1.0, 0.0, 10.0),
            Coord3D::new(1.0, 1.0, 10.0),
            20,
        ),
        Line3D::new(
            Coord3D::new(1.0, 1.0, 20.0),
            Coord3D::new(0.0, 1.0, 20.0),
            30,
        ),
        Line3D::new(
            Coord3D::new(0.0, 1.0, 30.0),
            Coord3D::new(0.0, 0.0, 30.0),
            40,
        ),
    ]
}

#[test]
fn reports_and_reconciles_same_xy_z_conflicts_deterministically() {
    let options = PolygonizerOptions {
        diagnostics: DiagnosticsOptions {
            enabled: true,
            ..Default::default()
        },
        provenance: ProvenanceOptions {
            enabled: true,
            include_boundary_line_ids: true,
        },
        ..Default::default()
    };

    let result = polygonize(conflicting_square(), &options).unwrap();
    let conflicts = result.diagnostics.unwrap().z_conflicts;

    assert_eq!(conflicts.conflict_node_count, 4);
    assert_eq!(conflicts.contributing_line_ids, [10, 20, 30, 40]);
    assert_eq!(result.polygons.len(), 1);
}

#[test]
fn ignore_and_error_policies_are_explicit() {
    let ignored = polygonize(
        conflicting_square(),
        &PolygonizerOptions {
            z: ZOptions {
                policy: ZPolicy::Ignore,
                conflict_tolerance: 0.0,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert!(ignored.polygons[0]
        .exterior
        .iter()
        .all(|coordinate| coordinate.z == 0.0));

    let error = polygonize(
        conflicting_square(),
        &PolygonizerOptions {
            z: ZOptions {
                policy: ZPolicy::ErrorOnConflict,
                conflict_tolerance: 0.0,
            },
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PolygonizeError::ZConflict {
            x: 0.0,
            y: 0.0,
            line_ids
        } if line_ids == [10, 40]
    ));

    assert!(polygonize(
        conflicting_square(),
        &PolygonizerOptions {
            z: ZOptions {
                policy: ZPolicy::ErrorOnConflict,
                conflict_tolerance: 30.0,
            },
            ..Default::default()
        },
    )
    .is_ok());
}

#[test]
fn split_vertices_apply_the_selected_source_edge_policy() {
    let lines = vec![
        Line3D::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 100.0),
            1,
        ),
        Line3D::new(
            Coord3D::new(2.0, -1.0, 1000.0),
            Coord3D::new(2.0, 1.0, 2000.0),
            2,
        ),
    ];

    let interpolated = SnapNoder::new(0.0).node(lines.clone());
    let nearest = SnapNoder::new(0.0)
        .with_z_policy(ZPolicy::PreferNearestEndpoint)
        .node(lines);

    let intersection_z = |segments: &[Line3D], line_id| {
        segments
            .iter()
            .filter(|line| line.line_id == line_id)
            .flat_map(|line| [line.start, line.end])
            .find(|coordinate| coordinate.x == 2.0 && coordinate.y == 0.0)
            .unwrap()
            .z
    };
    assert_eq!(intersection_z(&interpolated, 1), 20.0);
    assert_eq!(intersection_z(&interpolated, 2), 1500.0);
    assert_eq!(intersection_z(&nearest, 1), 0.0);
    assert_eq!(intersection_z(&nearest, 2), 1000.0);
}
