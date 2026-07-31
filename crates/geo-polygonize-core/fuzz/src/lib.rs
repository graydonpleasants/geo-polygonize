use arbitrary::{Arbitrary, Unstructured};
use geo_polygonize_core::{
    differential::{
        minimize_line_set, minimize_xy_coordinates, DifferentialMismatchCandidateV1,
        DifferentialOutcomeV1, DifferentialRunV1,
    },
    polygonize, polygonize_with_workspace, Coord3D, FingerprintDiffV1, Line3D,
    NormalizedPolygonizeErrorV1, PolygonizerOptions, PolygonizerWorkspace, PrecisionModel,
    SnapStrategy,
};
use std::collections::BTreeMap;

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

pub struct AdapterDifferentialCase {
    pub lines: Vec<Line3D>,
    pub options: PolygonizerOptions,
}

#[derive(Debug)]
pub struct PreparedDifferentialCandidate {
    pub candidate: DifferentialMismatchCandidateV1,
    pub original_line_count: usize,
    pub minimized_line_count: usize,
}

#[derive(Debug)]
pub enum CandidatePreparation {
    Ignored,
    Matched,
    Candidate(Box<PreparedDifferentialCandidate>),
}

fn decode_adapter_differential(data: &[u8]) -> Option<AdapterDifferentialCase> {
    if data.len() < AdapterDifferentialInput::size_hint(0).0 {
        return None;
    }
    let Ok(input) = AdapterDifferentialInput::arbitrary_take_rest(Unstructured::new(data)) else {
        return None;
    };
    if input.lines.len() > 64 {
        return None;
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
    Some(AdapterDifferentialCase {
        lines,
        options: PolygonizerOptions {
            node_input: input.node_input,
            precision_model: PrecisionModel::FixedGrid {
                grid_size: input.grid_size.abs().clamp(1e-10, 1.0),
            },
            snap_strategy: SnapStrategy::Grid,
            ..Default::default()
        },
    })
}

fn adapter_outcomes(
    lines: &[Line3D],
    options: &PolygonizerOptions,
) -> Result<(DifferentialOutcomeV1, DifferentialOutcomeV1), String> {
    let baseline =
        DifferentialOutcomeV1::from_result(polygonize(lines.iter().copied(), options), options)
            .map_err(|error| format!("one-shot fingerprint failed: {error}"))?;
    let comparison = DifferentialOutcomeV1::from_result(
        polygonize_with_workspace(lines, options, &mut PolygonizerWorkspace::new()),
        options,
    )
    .map_err(|error| format!("workspace fingerprint failed: {error}"))?;
    Ok((baseline, comparison))
}

/// Decode and compare one raw `adapter_differential` libFuzzer input.
pub fn replay_adapter_differential(data: &[u8]) -> Result<ReplayOutcome, String> {
    let Some(case) = decode_adapter_differential(data) else {
        return Ok(ReplayOutcome::Ignored);
    };
    let (baseline, comparison) = adapter_outcomes(&case.lines, &case.options)?;
    if baseline == comparison {
        Ok(ReplayOutcome::Matched)
    } else {
        Err(format!(
            "adapter outcome mismatch: {baseline:?} != {comparison:?}"
        ))
    }
}

/// Minimize one raw mismatch artifact and prepare it for human review.
pub fn prepare_adapter_differential_candidate(data: &[u8]) -> Result<CandidatePreparation, String> {
    let Some(case) = decode_adapter_differential(data) else {
        return Ok(CandidatePreparation::Ignored);
    };
    Ok(match prepare_case_with(case, adapter_outcomes)? {
        Some(candidate) => CandidatePreparation::Candidate(Box::new(candidate)),
        None => CandidatePreparation::Matched,
    })
}

#[derive(Clone, Debug, PartialEq)]
enum MismatchSignature {
    Fingerprint(FingerprintDiffV1),
    Errors(
        Box<NormalizedPolygonizeErrorV1>,
        Box<NormalizedPolygonizeErrorV1>,
    ),
    OutcomeKinds(bool, bool),
}

fn mismatch_signature(
    baseline: &DifferentialOutcomeV1,
    comparison: &DifferentialOutcomeV1,
) -> Option<MismatchSignature> {
    match (baseline, comparison) {
        (DifferentialOutcomeV1::Success(baseline), DifferentialOutcomeV1::Success(comparison)) => {
            baseline
                .diff(comparison)
                .map(MismatchSignature::Fingerprint)
        }
        (DifferentialOutcomeV1::Error(baseline), DifferentialOutcomeV1::Error(comparison)) => {
            (baseline != comparison).then(|| {
                MismatchSignature::Errors(Box::new(baseline.clone()), Box::new(comparison.clone()))
            })
        }
        (baseline, comparison) => Some(MismatchSignature::OutcomeKinds(
            matches!(baseline, DifferentialOutcomeV1::Success(_)),
            matches!(comparison, DifferentialOutcomeV1::Success(_)),
        )),
    }
}

fn prepare_case_with<F>(
    case: AdapterDifferentialCase,
    mut compare: F,
) -> Result<Option<PreparedDifferentialCandidate>, String>
where
    F: FnMut(
        &[Line3D],
        &PolygonizerOptions,
    ) -> Result<(DifferentialOutcomeV1, DifferentialOutcomeV1), String>,
{
    let original_line_count = case.lines.len();
    let (baseline, comparison) = compare(&case.lines, &case.options)?;
    let Some(signature) = mismatch_signature(&baseline, &comparison) else {
        return Ok(None);
    };
    let minimized = {
        let mut reproduces = |lines: &[Line3D]| {
            compare(lines, &case.options)
                .ok()
                .and_then(|(baseline, comparison)| mismatch_signature(&baseline, &comparison))
                .is_some_and(|candidate| candidate == signature)
        };
        let line_minimized = minimize_line_set(case.lines, &mut reproduces)
            .ok_or_else(|| "artifact mismatch was not deterministic".to_string())?;
        minimize_xy_coordinates(line_minimized, &mut reproduces)
            .ok_or_else(|| "artifact mismatch changed during coordinate minimization".to_string())?
    };

    let (baseline_outcome, comparison_outcome) = compare(&minimized, &case.options)?;
    if mismatch_signature(&baseline_outcome, &comparison_outcome) != Some(signature) {
        return Err("minimized artifact changed the observed mismatch".to_string());
    }
    let candidate = DifferentialMismatchCandidateV1::new(
        "adapter_differential",
        &minimized,
        &case.options,
        BTreeMap::new(),
        DifferentialRunV1 {
            implementation: "one_shot".to_string(),
            outcome: baseline_outcome,
        },
        DifferentialRunV1 {
            implementation: "workspace".to_string(),
            outcome: comparison_outcome,
        },
    )
    .map_err(|error| error.to_string())?;

    Ok(Some(PreparedDifferentialCandidate {
        candidate,
        original_line_count,
        minimized_line_count: minimized.len(),
    }))
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

    #[test]
    fn preparation_minimizes_geometry_without_classifying_or_admitting() {
        let coord = |x, y, z| Coord3D::new(x, y, z);
        let case = AdapterDifferentialCase {
            lines: vec![
                Line3D::new(coord(0.0, 0.0, 10.0), coord(1.0, 0.0, 11.0), 10),
                Line3D::new(coord(1.0, 0.0, 12.0), coord(1.0, 1.0, 13.0), 11),
                Line3D::new(coord(1.0, 1.0, 14.0), coord(0.0, 1.0, 15.0), 12),
                Line3D::new(coord(0.0, 1.0, 16.0), coord(0.0, 0.0, 17.0), 13),
                Line3D::new(coord(8.0, 8.0, 18.0), coord(8.0, 8.0, 19.0), 99),
            ],
            options: PolygonizerOptions::default(),
        };
        let prepared = prepare_case_with(case, |lines, options| {
            Ok((
                DifferentialOutcomeV1::from_result(
                    polygonize(lines.iter().copied(), options),
                    options,
                )
                .unwrap(),
                DifferentialOutcomeV1::from_result(
                    polygonize(Vec::<Line3D>::new(), options),
                    options,
                )
                .unwrap(),
            ))
        })
        .unwrap()
        .unwrap();
        let ids: Vec<_> = prepared
            .candidate
            .input
            .iter()
            .map(|line| line.line_id.as_str())
            .collect();

        assert_eq!(prepared.original_line_count, 5);
        assert_eq!(prepared.minimized_line_count, 4);
        assert_eq!(
            ids,
            ["0x0000000a", "0x0000000b", "0x0000000c", "0x0000000d"]
        );
        assert_eq!(prepared.candidate.input[0].start.z, "0x4024000000000000");
    }

    #[test]
    fn fingerprint_signature_preserves_values_at_the_same_path() {
        let square = |offset, first_id| {
            [
                Line3D::new(
                    Coord3D::new(offset, 0.0, 0.0),
                    Coord3D::new(offset + 1.0, 0.0, 0.0),
                    first_id,
                ),
                Line3D::new(
                    Coord3D::new(offset + 1.0, 0.0, 0.0),
                    Coord3D::new(offset + 1.0, 1.0, 0.0),
                    first_id + 1,
                ),
                Line3D::new(
                    Coord3D::new(offset + 1.0, 1.0, 0.0),
                    Coord3D::new(offset, 1.0, 0.0),
                    first_id + 2,
                ),
                Line3D::new(
                    Coord3D::new(offset, 1.0, 0.0),
                    Coord3D::new(offset, 0.0, 0.0),
                    first_id + 3,
                ),
            ]
        };
        let options = PolygonizerOptions::default();
        let one = square(0.0, 0).to_vec();
        let shifted = square(3.0, 4).to_vec();
        let outcome = |lines: &[Line3D]| {
            DifferentialOutcomeV1::from_result(
                polygonize(lines.iter().copied(), &options),
                &options,
            )
            .unwrap()
        };
        let empty = outcome(&[]);
        let one_signature = mismatch_signature(&outcome(&one), &empty).unwrap();
        let shifted_signature = mismatch_signature(&outcome(&shifted), &empty).unwrap();
        let (
            MismatchSignature::Fingerprint(one_diff),
            MismatchSignature::Fingerprint(shifted_diff),
        ) = (&one_signature, &shifted_signature)
        else {
            panic!("expected fingerprint signatures");
        };

        assert_eq!(one_diff.path, shifted_diff.path);
        assert_ne!(one_signature, shifted_signature);
    }
}
