use geo_polygonize_core::{
    differential::DifferentialOutcomeV1, polygonize, polygonize_with_workspace, Coord3D, Line3D,
    PolygonizerOptions, PolygonizerWorkspace,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn exact_keys(value: &Value, expected: &[&str]) -> Result<(), String> {
    let mut actual: Vec<_> = value
        .as_object()
        .ok_or("expected an object")?
        .keys()
        .map(String::as_str)
        .collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    (actual == expected)
        .then_some(())
        .ok_or_else(|| format!("expected keys {expected:?}, found {actual:?}"))
}

fn decode_bits(value: &Value) -> Result<f64, String> {
    let bits = value
        .as_str()
        .and_then(|value| value.strip_prefix("0x"))
        .ok_or("expected a hexadecimal float")?;
    u64::from_str_radix(bits, 16)
        .map(f64::from_bits)
        .map_err(|error| error.to_string())
}

fn lines(value: &Value) -> Result<Vec<Line3D>, String> {
    value
        .as_array()
        .ok_or("input must be an array")?
        .iter()
        .map(|line| {
            let coordinate = |value: &Value| -> Result<Coord3D, String> {
                Ok(Coord3D::new(
                    decode_bits(&value["x"])?,
                    decode_bits(&value["y"])?,
                    decode_bits(&value["z"])?,
                ))
            };
            let line_id = line["line_id"]
                .as_str()
                .and_then(|value| value.strip_prefix("0x"))
                .ok_or("expected a hexadecimal line_id")?;
            Ok(Line3D::new(
                coordinate(&line["start"])?,
                coordinate(&line["end"])?,
                u32::from_str_radix(line_id, 16).map_err(|error| error.to_string())?,
            ))
        })
        .collect()
}

fn validate(path: &Path, fixture: &Value) -> Result<(), String> {
    exact_keys(
        fixture,
        &["candidate", "case_id", "classification", "schema_version"],
    )?;
    if fixture["schema_version"] != 2 {
        return Err("expected persisted differential schema 2".to_string());
    }
    if fixture["case_id"].as_str() != path.file_stem().and_then(|value| value.to_str()) {
        return Err("case_id must match the fixture filename".to_string());
    }
    if !matches!(
        fixture["classification"].as_str(),
        Some("expected_parity" | "expected_divergence" | "invalid_ambiguous")
    ) {
        return Err("unknown compatibility classification".to_string());
    }

    let candidate = &fixture["candidate"];
    exact_keys(
        candidate,
        &[
            "baseline",
            "comparison",
            "input",
            "options",
            "producer",
            "schema_version",
            "versions",
        ],
    )?;
    if candidate["schema_version"] != 1
        || candidate["producer"] != "adapter_differential"
        || candidate["baseline"]["implementation"] != "one_shot"
        || candidate["comparison"]["implementation"] != "workspace"
    {
        return Err("unknown producer or implementation labels".to_string());
    }
    if candidate["baseline"]["outcome"] == candidate["comparison"]["outcome"] {
        return Err("persisted outcomes must differ".to_string());
    }
    if candidate["versions"]
        .as_object()
        .is_none_or(|versions| versions.is_empty())
    {
        return Err("candidate versions are required".to_string());
    }

    let options: PolygonizerOptions =
        serde_json::from_value(candidate["options"].clone()).map_err(|error| error.to_string())?;
    let lines = lines(&candidate["input"])?;
    let baseline =
        DifferentialOutcomeV1::from_result(polygonize(lines.iter().copied(), &options), &options)
            .map_err(|error| error.to_string())?;
    let comparison = DifferentialOutcomeV1::from_result(
        polygonize_with_workspace(&lines, &options, &mut PolygonizerWorkspace::new()),
        &options,
    )
    .map_err(|error| error.to_string())?;
    if serde_json::to_value(baseline).unwrap() != candidate["baseline"]["outcome"] {
        return Err("baseline outcome changed on rerun".to_string());
    }
    if serde_json::to_value(comparison).unwrap() != candidate["comparison"]["outcome"] {
        return Err("comparison outcome changed on rerun".to_string());
    }
    Ok(())
}

#[test]
fn persisted_two_sided_differential_corpus_reruns_exact_outcomes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/differential-v2");
    let mut paths: Vec<PathBuf> = if root.exists() {
        fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect()
    } else {
        Vec::new()
    };
    if let Some(candidate) = std::env::var_os("PERSISTED_DIFFERENTIAL_V2_CANDIDATE") {
        paths.push(candidate.into());
    }
    paths.sort();

    for path in paths {
        let fixture: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        validate(&path, &fixture).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}

#[test]
fn rerun_validation_rejects_a_changed_full_outcome() {
    let options = PolygonizerOptions::default();
    let input = serde_json::json!([{
        "start": {"x": "0x0000000000000000", "y": "0x0000000000000000", "z": "0x4024000000000000"},
        "end": {"x": "0x0000000000000000", "y": "0x0000000000000000", "z": "0x4026000000000000"},
        "line_id": "0x00000007"
    }]);
    let lines = lines(&input).unwrap();
    let baseline =
        DifferentialOutcomeV1::from_result(polygonize(lines, &options), &options).unwrap();
    let fixture = serde_json::json!({
        "schema_version": 2,
        "case_id": "changed-outcome",
        "classification": "invalid_ambiguous",
        "candidate": {
            "schema_version": 1,
            "producer": "adapter_differential",
            "input": input,
            "options": options,
            "versions": {"geo-polygonize-core": env!("CARGO_PKG_VERSION")},
            "baseline": {"implementation": "one_shot", "outcome": baseline},
            "comparison": {"implementation": "workspace", "outcome": {"status": "error", "value": null}}
        }
    });

    assert_eq!(
        validate(Path::new("changed-outcome.json"), &fixture),
        Err("comparison outcome changed on rerun".to_string())
    );
}
