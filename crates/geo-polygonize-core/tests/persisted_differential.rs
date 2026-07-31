use geo_polygonize_core::{polygonize, Coord3D, Line3D, PolygonizerOptions, TopologyFingerprintV1};
use serde_json::Value;
use std::fs;
use std::path::Path;

fn assert_keys(value: &Value, expected: &[&str]) {
    let mut actual: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

fn decode_bits(value: &Value) -> f64 {
    let bits = value.as_str().unwrap().strip_prefix("0x").unwrap();
    f64::from_bits(u64::from_str_radix(bits, 16).unwrap())
}

fn coordinate(value: &Value) -> Coord3D {
    Coord3D::new(
        decode_bits(&value["x"]),
        decode_bits(&value["y"]),
        decode_bits(&value["z"]),
    )
}

#[test]
fn persisted_differential_corpus_reproduces_strict_fingerprints() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/differential");
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "expected persisted differential fixtures"
    );

    for path in paths {
        let fixture: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_keys(
            &fixture,
            &["case_id", "classification", "golden", "schema_version"],
        );
        assert_eq!(fixture["schema_version"], 1);
        assert_eq!(
            fixture["case_id"].as_str().unwrap(),
            path.file_stem().unwrap().to_str().unwrap()
        );
        assert!(matches!(
            fixture["classification"].as_str().unwrap(),
            "expected_parity" | "expected_divergence" | "invalid_ambiguous"
        ));

        let golden = &fixture["golden"];
        assert_keys(
            golden,
            &[
                "fingerprint",
                "input",
                "options",
                "reference_metrics",
                "schema_version",
                "versions",
                "witness",
            ],
        );
        assert_eq!(golden["schema_version"], 1);
        let options: PolygonizerOptions =
            serde_json::from_value(golden["options"].clone()).unwrap();
        let lines: Vec<_> = golden["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| {
                let line_id = line["line_id"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("0x")
                    .unwrap();
                Line3D::new(
                    coordinate(&line["start"]),
                    coordinate(&line["end"]),
                    u32::from_str_radix(line_id, 16).unwrap(),
                )
            })
            .collect();
        let result = polygonize(lines, &options).unwrap();
        let actual = TopologyFingerprintV1::try_from_result(&result, &options).unwrap();
        assert_eq!(serde_json::to_value(actual).unwrap(), golden["fingerprint"]);
        assert!(golden["versions"]
            .as_object()
            .is_some_and(|versions| !versions.is_empty()));
        assert!(!golden["witness"]["path"].as_str().unwrap().is_empty());

        if fixture["classification"] == "expected_divergence" {
            let expected = &golden["reference_metrics"]["polygon_count"];
            let actual = golden["fingerprint"]["polygons"].as_array().unwrap().len();
            assert_ne!(expected, actual);
            assert_eq!(&golden["witness"]["expected"], expected);
            assert_eq!(golden["witness"]["actual"], actual);
        }
    }
}
