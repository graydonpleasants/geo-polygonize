#![no_main]

use arbitrary::Arbitrary;
use geo_polygonize_core::{
    normalize_polygonize_error, polygonize, polygonize_with_workspace, Coord3D, Line3D,
    PolygonizerOptions, PolygonizerWorkspace, PrecisionModel, SnapStrategy, TopologyFingerprintV1,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    lines: Vec<(f64, f64, f64, f64, f64, f64)>,
    node_input: bool,
    grid_size: f64,
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 64 {
        return;
    }
    let lines: Vec<_> = input
        .lines
        .into_iter()
        .enumerate()
        .filter_map(|(index, (sx, sy, sz, ex, ey, ez))| {
            [sx, sy, sz, ex, ey, ez]
                .iter()
                .all(|value| value.is_finite())
                .then(|| {
                    Line3D::new(
                        Coord3D::new(sx, sy, sz),
                        Coord3D::new(ex, ey, ez),
                        u32::try_from(index).unwrap(),
                    )
                })
        })
        .collect();
    let options = PolygonizerOptions {
        node_input: input.node_input,
        precision_model: PrecisionModel::FixedGrid {
            grid_size: input.grid_size.abs().clamp(1e-10, 1.0),
        },
        snap_strategy: SnapStrategy::Grid,
        ..Default::default()
    };
    let one_shot = polygonize(lines.iter().copied(), &options);
    let workspace = polygonize_with_workspace(&lines, &options, &mut PolygonizerWorkspace::new());

    match (one_shot, workspace) {
        (Ok(one_shot), Ok(workspace)) => {
            assert_eq!(
                TopologyFingerprintV1::try_from_result(&one_shot, &options).unwrap(),
                TopologyFingerprintV1::try_from_result(&workspace, &options).unwrap()
            );
        }
        (Err(one_shot), Err(workspace)) => assert_eq!(
            normalize_polygonize_error(&one_shot),
            normalize_polygonize_error(&workspace)
        ),
        (one_shot, workspace) => panic!("adapter outcome mismatch: {one_shot:?} != {workspace:?}"),
    }
});
