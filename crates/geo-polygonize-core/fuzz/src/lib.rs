use arbitrary::{Arbitrary, Unstructured};
use geo_polygonize_core::{
    normalize_polygonize_error, polygonize, polygonize_with_workspace, Coord3D, Line3D,
    PolygonizerOptions, PolygonizerWorkspace, PrecisionModel, SnapStrategy, TopologyFingerprintV1,
};

#[derive(Arbitrary, Debug)]
struct AdapterDifferentialInput {
    lines: Vec<(f64, f64, f64, f64, f64, f64)>,
    node_input: bool,
    grid_size: f64,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReplayOutcome {
    Ignored,
    Matched,
}

/// Decode and compare one raw `adapter_differential` libFuzzer input.
pub fn replay_adapter_differential(data: &[u8]) -> Result<ReplayOutcome, String> {
    if data.len() < AdapterDifferentialInput::size_hint(0).0 {
        return Ok(ReplayOutcome::Ignored);
    }
    let Ok(input) = AdapterDifferentialInput::arbitrary_take_rest(Unstructured::new(data)) else {
        return Ok(ReplayOutcome::Ignored);
    };
    if input.lines.len() > 64 {
        return Ok(ReplayOutcome::Ignored);
    }

    let lines: Vec<_> = input
        .lines
        .into_iter()
        .enumerate()
        .filter_map(|(index, (sx, sy, sz, ex, ey, ez))| {
            if [sx, sy, sz, ex, ey, ez]
                .iter()
                .all(|value| value.is_finite())
            {
                Some(Line3D::new(
                    Coord3D::new(sx, sy, sz),
                    Coord3D::new(ex, ey, ez),
                    u32::try_from(index).unwrap(),
                ))
            } else {
                None
            }
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
            let one_shot = TopologyFingerprintV1::try_from_result(&one_shot, &options)
                .map_err(|error| format!("one-shot fingerprint failed: {error}"))?;
            let workspace = TopologyFingerprintV1::try_from_result(&workspace, &options)
                .map_err(|error| format!("workspace fingerprint failed: {error}"))?;
            if one_shot == workspace {
                Ok(ReplayOutcome::Matched)
            } else {
                Err(format!(
                    "adapter fingerprint mismatch: {one_shot:?} != {workspace:?}"
                ))
            }
        }
        (Err(one_shot), Err(workspace)) => {
            let one_shot = normalize_polygonize_error(&one_shot);
            let workspace = normalize_polygonize_error(&workspace);
            if one_shot == workspace {
                Ok(ReplayOutcome::Matched)
            } else {
                Err(format!(
                    "adapter error mismatch: {one_shot:?} != {workspace:?}"
                ))
            }
        }
        (one_shot, workspace) => Err(format!(
            "adapter outcome mismatch: {one_shot:?} != {workspace:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_replay_uses_the_fuzz_decoder_and_comparison() {
        let artifact = [0_u8; 10];
        assert_eq!(
            replay_adapter_differential(&artifact),
            Ok(ReplayOutcome::Matched)
        );
    }
}
