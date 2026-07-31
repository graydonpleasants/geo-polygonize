use geo_polygonize_core::{
    polygonize, polygonize_with_execution_policy, CancellationToken, Coord3D, ExecutionPolicy,
    Line3D, NodingGuarantee, NodingOptions, NodingValidationKind, PolygonizeErrorKind,
    PolygonizerOptions, ZOptions, ZPolicy,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConstructionPath {
    CoreEntrypoint,
    AdapterEntrypoint,
    InternalOnly,
    BoundaryOnly,
}

fn construction_path(kind: PolygonizeErrorKind) -> ConstructionPath {
    match kind {
        PolygonizeErrorKind::InvalidArgumentType
        | PolygonizeErrorKind::InvalidGeometry
        | PolygonizeErrorKind::ResourceLimitExceeded
        | PolygonizeErrorKind::Cancelled
        | PolygonizeErrorKind::UnsupportedOptionCombination
        | PolygonizeErrorKind::ZConflict => ConstructionPath::CoreEntrypoint,
        PolygonizeErrorKind::NodingValidationFailure(kind) => {
            match kind {
                NodingValidationKind::ZeroLengthSegment
                | NodingValidationKind::CollinearOverlap
                | NodingValidationKind::InteriorIntersection => {}
            }
            ConstructionPath::CoreEntrypoint
        }
        PolygonizeErrorKind::InvalidBufferShape | PolygonizeErrorKind::ArrowError => {
            ConstructionPath::AdapterEntrypoint
        }
        PolygonizeErrorKind::TopologyFailure
        | PolygonizeErrorKind::InternalInvariantViolation
        | PolygonizeErrorKind::NullPointer => ConstructionPath::InternalOnly,
        PolygonizeErrorKind::Panic => ConstructionPath::BoundaryOnly,
    }
}

fn line(start: (f64, f64, f64), end: (f64, f64, f64), id: u32) -> Line3D {
    Line3D::new(
        Coord3D::new(start.0, start.1, start.2),
        Coord3D::new(end.0, end.1, end.2),
        id,
    )
}

#[test]
fn supported_core_entrypoints_construct_every_core_error_family() {
    let invalid_argument = PolygonizerOptions {
        pre_snap_tolerance: -1.0,
        ..Default::default()
    }
    .validate()
    .unwrap_err();
    let unsupported_options = PolygonizerOptions {
        pre_snap_tolerance: 1.0,
        ..Default::default()
    }
    .validate()
    .unwrap_err();
    let invalid_geometry = polygonize(
        [line((f64::NAN, 0.0, 0.0), (1.0, 0.0, 0.0), 1)],
        &PolygonizerOptions::default(),
    )
    .unwrap_err();
    let resource_limit = polygonize_with_execution_policy(
        [line((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), 1)],
        &PolygonizerOptions::default(),
        &ExecutionPolicy {
            max_input_segments: Some(0),
            ..Default::default()
        },
    )
    .unwrap_err();

    let token = CancellationToken::new();
    token.cancel();
    let cancelled = polygonize_with_execution_policy(
        [],
        &PolygonizerOptions::default(),
        &ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        },
    )
    .unwrap_err();

    let z_conflict = polygonize(
        [
            line((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), 1),
            line((1.0, 0.0, 1.0), (1.0, 1.0, 1.0), 2),
        ],
        &PolygonizerOptions {
            z: ZOptions {
                policy: ZPolicy::ErrorOnConflict,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap_err();
    let noding_validation = polygonize(
        [
            line((-1.0, 0.0, 0.0), (1.0, 0.0, 0.0), 1),
            line((0.0, -1.0, 0.0), (0.0, 1.0, 0.0), 2),
        ],
        &PolygonizerOptions {
            noding: NodingOptions {
                guarantee: NodingGuarantee::Validate,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap_err();

    let actual = [
        invalid_argument.kind(),
        invalid_geometry.kind(),
        resource_limit.kind(),
        cancelled.kind(),
        unsupported_options.kind(),
        z_conflict.kind(),
        noding_validation.kind(),
    ];
    let expected = [
        PolygonizeErrorKind::InvalidArgumentType,
        PolygonizeErrorKind::InvalidGeometry,
        PolygonizeErrorKind::ResourceLimitExceeded,
        PolygonizeErrorKind::Cancelled,
        PolygonizeErrorKind::UnsupportedOptionCombination,
        PolygonizeErrorKind::ZConflict,
        PolygonizeErrorKind::NodingValidationFailure(NodingValidationKind::InteriorIntersection),
    ];

    assert_eq!(actual, expected);
    assert!(actual
        .into_iter()
        .all(|kind| construction_path(kind) == ConstructionPath::CoreEntrypoint));
}

#[test]
fn non_core_error_families_remain_explicitly_classified() {
    for (kind, expected) in [
        (
            PolygonizeErrorKind::InvalidBufferShape,
            ConstructionPath::AdapterEntrypoint,
        ),
        (
            PolygonizeErrorKind::ArrowError,
            ConstructionPath::AdapterEntrypoint,
        ),
        (
            PolygonizeErrorKind::TopologyFailure,
            ConstructionPath::InternalOnly,
        ),
        (
            PolygonizeErrorKind::InternalInvariantViolation,
            ConstructionPath::InternalOnly,
        ),
        (
            PolygonizeErrorKind::NullPointer,
            ConstructionPath::InternalOnly,
        ),
        (PolygonizeErrorKind::Panic, ConstructionPath::BoundaryOnly),
    ] {
        assert_eq!(construction_path(kind), expected);
    }
}
