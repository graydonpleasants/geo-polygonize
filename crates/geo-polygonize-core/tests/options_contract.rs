use geo_polygonize_core::PolygonizerOptions;
use serde_json::Value;

fn canonical_fixture() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/conformance/canonical_options_v1.json"
    ))
    .unwrap()
}

#[test]
fn canonical_options_round_trip_through_rust_serde() {
    let fixture = canonical_fixture();
    let options: PolygonizerOptions = serde_json::from_value(fixture["options"].clone()).unwrap();
    assert_eq!(serde_json::to_value(options).unwrap(), fixture["options"]);
}

#[test]
fn legacy_canonical_options_payload_preserves_defaults() {
    let legacy: Value = serde_json::from_str(include_str!(
        "fixtures/conformance/canonical_options_legacy_v0.json"
    ))
    .unwrap();
    let options: PolygonizerOptions = serde_json::from_value(legacy["options"].clone()).unwrap();
    assert_eq!(
        serde_json::to_value(options).unwrap(),
        canonical_fixture()["options"]
    );
}
