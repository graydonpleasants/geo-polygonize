use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

fn schema() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/benchmark-record-v1.schema.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn benchmark_file(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn required(value: &Value) -> HashSet<&str> {
    value["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect()
}

fn enum_values<'a>(value: &'a Value, path: &[&str]) -> HashSet<&'a str> {
    let value = path.iter().fold(value, |value, key| &value[*key]);
    value["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|variant| variant.as_str().unwrap())
        .collect()
}

#[test]
fn benchmark_schema_requires_correctness_before_timings() {
    let schema = schema();
    assert_eq!(schema["properties"]["schema_version"]["const"], 1);
    assert_eq!(
        enum_values(&schema, &["properties", "lane"]),
        HashSet::from([
            "already-noded-polygonization",
            "floating-noding-plus-polygonization",
            "certified-fixed-precision-noding-plus-polygonization",
        ])
    );

    let gate = &schema["properties"]["correctness_gate"];
    assert_eq!(gate["properties"]["status"]["const"], "passed");
    assert_eq!(
        enum_values(gate, &["properties", "validation", "properties", "result"]),
        HashSet::from(["passed", "not-promised"])
    );
    assert_eq!(
        enum_values(
            gate,
            &["properties", "fingerprint", "properties", "outcome"]
        ),
        HashSet::from(["equal", "expected-divergence"])
    );
    assert!(required(gate).is_superset(&HashSet::from([
        "status",
        "validation",
        "compatibility",
        "fingerprint",
    ])));
}

#[test]
fn benchmark_schema_requires_measurement_work_and_environment_evidence() {
    let schema = schema();
    let properties = &schema["properties"];
    assert!(
        required(&properties["topology"]).is_superset(&HashSet::from([
            "polygons",
            "rings",
            "dangles",
            "cut_edges",
            "invalid_rings",
            "provenance_sources",
        ]))
    );
    assert!(
        required(&properties["measurement"]).is_superset(&HashSet::from([
            "p50_ms",
            "p95_ms",
            "throughput",
            "samples",
            "phase_times_ms",
            "allocations",
            "peak_rss_bytes",
        ]))
    );
    let layout_candidate = &properties["measurement"]["properties"]["layout_candidate"];
    assert_eq!(layout_candidate["additionalProperties"], false);
    assert!(required(layout_candidate).is_superset(&HashSet::from([
        "candidate_id",
        "conformance",
        "samples",
        "nested_traversal_p50_ns",
        "csr_materialization_ns",
        "csr_traversal_p50_ns",
    ])));
    assert!(required(&properties["work"]).is_superset(&HashSet::from([
        "candidate_pairs",
        "exact_predicate_calls",
        "split_events",
        "segment_expansion",
    ])));
    let component_memory = &properties["work"]["properties"]["component_memory"];
    assert_eq!(component_memory["additionalProperties"], false);
    assert!(required(component_memory).is_superset(&HashSet::from([
        "component_count",
        "active_node_count",
        "active_edge_count",
        "largest_component_node_count",
        "largest_component_edge_count",
        "partition_node_capacity",
        "partition_edge_capacity",
        "global_graph_adjacency_capacity",
        "global_graph_adjacency_row_capacity",
        "global_graph_csr_offset_count",
        "global_graph_csr_directed_edge_count",
        "scratch_instance_count",
        "execution_worker_count",
        "max_scratch_adjacency_capacity",
        "max_merged_output_coordinate_capacity",
    ])));
    assert!(
        required(&properties["environment"]).is_superset(&HashSet::from([
            "architecture",
            "os",
            "compiler",
            "dependencies",
            "commit_sha",
        ]))
    );
    assert!(required(&schema).contains("artifact_sha256"));
    assert_eq!(
        schema["properties"]["artifact_sha256"]["pattern"],
        "^[0-9a-f]{64}$"
    );
}

#[test]
fn decision_policy_separates_diagnostics_from_publishable_evidence() {
    let policy = benchmark_file("benchmark-decision-policy-v1.json");
    let classes = &policy["measurement_classes"];
    assert_eq!(policy["schema_version"], 1);
    assert_eq!(classes["diagnostic"]["publishable"], false);
    assert_eq!(classes["decision_quality"]["publishable"], true);
    assert_eq!(
        classes["decision_quality"]["allowed_runner_classes"],
        serde_json::json!(["dedicated"])
    );
    assert!(
        classes["decision_quality"]["minimum_samples_per_process"]
            .as_u64()
            .unwrap()
            > classes["diagnostic"]["minimum_samples"].as_u64().unwrap()
    );
    assert!(
        classes["decision_quality"]["minimum_process_repetitions"]
            .as_u64()
            .unwrap()
            > 1
    );

    let promotion = &policy["promotion"];
    assert!(
        promotion["minimum_effect_size_percent"].as_f64().unwrap()
            > promotion["regression_budget_percent"].as_f64().unwrap()
    );
    for requirement in [
        "require_correctness_gate",
        "require_pinned_environment",
        "require_same_commit",
    ] {
        assert_eq!(classes["decision_quality"][requirement], true);
    }

    let policy_schema = benchmark_file("benchmark-decision-policy-v1.schema.json");
    assert_eq!(policy_schema["properties"]["schema_version"]["const"], 1);
    assert_eq!(
        policy_schema["properties"]["measurement_classes"]["properties"]["diagnostic"]
            ["properties"]["publishable"]["const"],
        false
    );
    assert_eq!(
        policy_schema["properties"]["measurement_classes"]["properties"]["decision_quality"]
            ["properties"]["publishable"]["const"],
        true
    );
}

#[test]
fn mismatch_candidate_schema_keeps_exact_input_and_reduced_outcomes() {
    let schema = benchmark_file("benchmark-mismatch-candidate-v1.schema.json");
    assert_eq!(schema["additionalProperties"], false);
    assert!(required(&schema).is_superset(&HashSet::from([
        "producer",
        "workload_id",
        "lane",
        "input",
        "options",
        "versions",
        "baseline",
        "comparison",
    ])));
    assert_eq!(
        schema["properties"]["producer"]["const"],
        "benchmark_record"
    );
    assert!(required(&schema["$defs"]["coordinate"]).contains("z"));
    assert_eq!(
        schema["$defs"]["outcome"]["oneOf"][0]["properties"]["value"]["$ref"],
        "#/$defs/topology"
    );
    assert_eq!(
        schema["$defs"]["outcome"]["oneOf"][1]["properties"]["value"]["$ref"],
        "#/$defs/failure"
    );
    assert!(schema["properties"].get("classification").is_none());
    assert!(schema["properties"].get("case_id").is_none());
}
