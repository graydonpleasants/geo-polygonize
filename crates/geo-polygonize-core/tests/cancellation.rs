use geo_polygonize_core::{
    polygonize_with_workspace_and_execution_policy, CancellationToken, Coord3D, ExecutionPolicy,
    Line3D, PolygonizeError, PolygonizerOptions, PolygonizerWorkspace,
};

fn square() -> [Line3D; 4] {
    [
        Line3D::new(Coord3D::new(0., 0., 0.), Coord3D::new(1., 0., 0.), 0),
        Line3D::new(Coord3D::new(1., 0., 0.), Coord3D::new(1., 1., 0.), 1),
        Line3D::new(Coord3D::new(1., 1., 0.), Coord3D::new(0., 1., 0.), 2),
        Line3D::new(Coord3D::new(0., 1., 0.), Coord3D::new(0., 0., 0.), 3),
    ]
}

#[test]
fn cancelled_workspace_run_can_be_reused() {
    let token = CancellationToken::new();
    let policy = ExecutionPolicy {
        cancellation_token: Some(token.clone()),
        ..Default::default()
    };
    let options = PolygonizerOptions::default();
    let mut workspace = PolygonizerWorkspace::new();
    token.cancel();

    assert!(matches!(
        polygonize_with_workspace_and_execution_policy(&square(), &options, &mut workspace, &policy),
        Err(PolygonizeError::Cancelled { stage }) if stage == "ingest"
    ));

    token.reset();
    assert_eq!(
        polygonize_with_workspace_and_execution_policy(
            &square(),
            &options,
            &mut workspace,
            &policy
        )
        .unwrap()
        .polygons
        .len(),
        1
    );
}
