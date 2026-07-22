use geo_polygonize_core::{
    polygonize_line_strings_with_execution_policy, polygonize_with_execution_policy, Coord3D,
    ExecutionPolicy, Line3D, NodingGuarantee, NodingOptions, PolygonizeError, PolygonizerOptions,
    PrecisionModel, SnapStrategy,
};
use geo_types::LineString;

fn assert_limit(error: PolygonizeError, stage: &str, limit: usize, observed: usize) {
    assert!(matches!(
        error,
        PolygonizeError::ResourceLimitExceeded {
            stage: actual_stage,
            limit: actual_limit,
            observed: actual_observed,
        } if actual_stage == stage && actual_limit == limit && actual_observed == observed
    ));
}

#[test]
fn input_limits_stop_before_polygonization() {
    let line = LineString::from(vec![(0., 0.), (1., 0.), (1., 1.)]);
    let options = PolygonizerOptions::default();

    assert_limit(
        polygonize_line_strings_with_execution_policy(
            [&line],
            &options,
            &ExecutionPolicy {
                max_input_line_strings: Some(0),
                ..Default::default()
            },
        )
        .unwrap_err(),
        "input_line_strings",
        0,
        1,
    );
    assert_limit(
        polygonize_line_strings_with_execution_policy(
            [&line],
            &options,
            &ExecutionPolicy {
                max_input_coordinates: Some(2),
                ..Default::default()
            },
        )
        .unwrap_err(),
        "input_coordinates",
        2,
        3,
    );

    let segment = Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(1., 0., 0.), 0);
    assert_limit(
        polygonize_with_execution_policy(
            [segment.clone(), segment],
            &options,
            &ExecutionPolicy {
                max_input_segments: Some(1),
                ..Default::default()
            },
        )
        .unwrap_err(),
        "input_segments",
        1,
        2,
    );
}

#[test]
fn noded_segment_limit_stops_after_noding() {
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let crossing = [
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(2., 2., 0.), 0),
        Line3D::new(Coord3D::new(0., 2., 0.), Coord3D::new(2., 0., 0.), 1),
    ];

    assert!(matches!(
        polygonize_with_execution_policy(
            crossing,
            &options,
            &ExecutionPolicy {
                max_noded_segments: Some(0),
                ..Default::default()
            },
        ),
        Err(PolygonizeError::ResourceLimitExceeded {
            stage,
            limit: 0,
            observed,
        }) if stage == "noded_segments" && observed > 0
    ));
}

#[test]
fn noding_work_limits_stop_dense_candidates_before_splitting() {
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let crossing = [
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(2., 2., 0.), 0),
        Line3D::new(Coord3D::new(0., 2., 0.), Coord3D::new(2., 0., 0.), 1),
    ];

    for (policy, stage) in [
        (
            ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
            "candidate_pairs",
        ),
        (
            ExecutionPolicy {
                max_exact_intersection_calls: Some(0),
                ..Default::default()
            },
            "exact_intersection_calls",
        ),
    ] {
        assert!(matches!(
            polygonize_with_execution_policy(crossing, &options, &policy),
            Err(PolygonizeError::ResourceLimitExceeded {
                stage: actual_stage,
                limit: 0,
                observed,
            }) if actual_stage == stage && observed > 0
        ));
    }
}

#[test]
fn certified_noding_obeys_candidate_limit() {
    let options = PolygonizerOptions {
        node_input: true,
        precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
        snap_strategy: SnapStrategy::Grid,
        noding: NodingOptions {
            guarantee: NodingGuarantee::CertifiedFixedPrecision,
            ..Default::default()
        },
        ..Default::default()
    };
    let crossing = [
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(2., 2., 0.), 0),
        Line3D::new(Coord3D::new(0., 2., 0.), Coord3D::new(2., 0., 0.), 1),
    ];

    assert_limit(
        polygonize_with_execution_policy(
            crossing,
            &options,
            &ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
        )
        .unwrap_err(),
        "candidate_pairs",
        0,
        1,
    );
}

#[test]
fn split_and_iteration_limits_stop_noding() {
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let crossing = [
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(2., 2., 0.), 0),
        Line3D::new(Coord3D::new(0., 2., 0.), Coord3D::new(2., 0., 0.), 1),
    ];

    for (policy, stage) in [
        (
            ExecutionPolicy {
                max_split_events: Some(0),
                ..Default::default()
            },
            "split_events",
        ),
        (
            ExecutionPolicy {
                max_noding_iterations: Some(0),
                ..Default::default()
            },
            "noding_iterations",
        ),
    ] {
        assert!(matches!(
            polygonize_with_execution_policy(crossing, &options, &policy),
            Err(PolygonizeError::ResourceLimitExceeded {
                stage: actual_stage,
                limit: 0,
                observed,
            }) if actual_stage == stage && observed > 0
        ));
    }
}

#[test]
fn graph_and_ring_limits_stop_after_their_boundary() {
    let options = PolygonizerOptions::default();
    let segment = Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(1., 0., 0.), 0);

    for (policy, stage, observed) in [
        (
            ExecutionPolicy {
                max_graph_nodes: Some(1),
                ..Default::default()
            },
            "graph_nodes",
            2,
        ),
        (
            ExecutionPolicy {
                max_graph_edges: Some(0),
                ..Default::default()
            },
            "graph_edges",
            1,
        ),
    ] {
        assert_limit(
            polygonize_with_execution_policy([segment], &options, &policy).unwrap_err(),
            stage,
            policy.max_graph_nodes.or(policy.max_graph_edges).unwrap(),
            observed,
        );
    }

    let square = [
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(1., 0., 0.), 0),
        Line3D::new(Coord3D::new(1., 0., 0.), Coord3D::new(1., 1., 0.), 1),
        Line3D::new(Coord3D::new(1., 1., 0.), Coord3D::new(0., 1., 0.), 2),
        Line3D::new(Coord3D::new(0., 1., 0.), Coord3D::new(0., 0., 0.), 3),
    ];
    assert!(matches!(
        polygonize_with_execution_policy(
            square,
            &options,
            &ExecutionPolicy {
                max_rings: Some(0),
                ..Default::default()
            },
        ),
        Err(PolygonizeError::ResourceLimitExceeded {
            stage,
            limit: 0,
            observed,
        }) if stage == "rings" && observed > 0
    ));
}

#[test]
fn output_limits_stop_before_returning_a_result() {
    let square = [
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(1., 0., 0.), 0),
        Line3D::new(Coord3D::new(1., 0., 0.), Coord3D::new(1., 1., 0.), 1),
        Line3D::new(Coord3D::new(1., 1., 0.), Coord3D::new(0., 1., 0.), 2),
        Line3D::new(Coord3D::new(0., 1., 0.), Coord3D::new(0., 0., 0.), 3),
    ];
    let options = PolygonizerOptions::default();

    assert_limit(
        polygonize_with_execution_policy(
            square,
            &options,
            &ExecutionPolicy {
                max_output_polygons: Some(0),
                ..Default::default()
            },
        )
        .unwrap_err(),
        "output_polygons",
        0,
        1,
    );
    assert!(matches!(
        polygonize_with_execution_policy(
            square,
            &options,
            &ExecutionPolicy {
                max_output_polygons: Some(1),
                max_output_coordinates: Some(0),
                ..Default::default()
            },
        ),
        Err(PolygonizeError::ResourceLimitExceeded {
            stage,
            limit: 0,
            observed,
        }) if stage == "output_coordinates" && observed > 0
    ));
}
