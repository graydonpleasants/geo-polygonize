import importlib.util
import json
from pathlib import Path

import pytest
from jsonschema import validate


ROOT = Path(__file__).resolve().parents[1]


def load_module(name, relative_path):
    path = ROOT / relative_path
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PUBLISHER = load_module("publish_benchmark", "benchmarks/publish_benchmark.py")
SUITE = load_module("validate_baseline_suite", "benchmarks/validate_baseline_suite.py")

COMPONENT_MEMORY_FIELDS = (
    "component_count",
    "active_node_count",
    "active_edge_count",
    "largest_component_node_count",
    "largest_component_edge_count",
    "partition_node_capacity",
    "partition_edge_capacity",
    "global_graph_node_capacity",
    "global_graph_edge_capacity",
    "global_graph_directed_edge_capacity",
    "global_graph_adjacency_capacity",
    "scratch_instance_count",
    "execution_worker_count",
    "max_scratch_node_capacity",
    "max_scratch_edge_capacity",
    "max_scratch_directed_edge_capacity",
    "max_scratch_adjacency_capacity",
    "max_scratch_global_node_capacity",
    "max_scratch_local_node_capacity",
    "max_scratch_global_dir_edge_capacity",
    "max_merged_output_item_count",
    "max_merged_output_coordinate_capacity",
)


def record(workload_id, lane, index, input_segments, commit="b" * 40):
    return {
        "schema_version": 1,
        "record_id": f"{workload_id}-{lane}-r{index}",
        "workload_id": workload_id,
        "artifact_sha256": (workload_id.encode().hex() + "a" * 64)[:64],
        "lane": lane,
        "implementation": {"name": "core", "version": "1", "features": ["parallel"]},
        "correctness_gate": {
            "status": "passed",
            "validation": {"promised": True, "result": "passed"},
            "compatibility": {"expected": "parity", "observed": "equal"},
            "fingerprint": {
                "outcome": "equal",
                "actual_sha256": "a" * 64,
                "reference_sha256": "a" * 64,
            },
        },
        "topology": {
            "polygons": 1,
            "rings": 1,
            "dangles": 0,
            "cut_edges": 0,
            "invalid_rings": 0,
            "provenance_sources": 1,
        },
        "measurement": {
            "p50_ms": 100 + index / 10,
            "p95_ms": 110 + index / 10,
            "throughput": {"value": input_segments / 100, "unit": "input-segments/second"},
            "samples": 30,
            "phase_times_ms": {"polygonize": 100},
            "allocations": {"count": 10, "bytes": 1000},
            "peak_rss_bytes": 2000,
        },
        "work": {
            "input_line_strings": 1,
            "input_segments": input_segments,
            "input_coordinates": input_segments + 1,
            "output_polygons": 1,
            "output_coordinates": 5,
            "candidate_pairs": 1,
            "exact_predicate_calls": 1,
            "split_events": 1,
            "segment_expansion": {
                "input_segments": input_segments,
                "noded_segments": input_segments,
                "ratio": 1,
            },
            "component_memory": {field: 1 for field in COMPONENT_MEMORY_FIELDS},
        },
        "environment": {
            "architecture": "x86_64",
            "os": {"name": "Linux", "version": "test"},
            "compiler": {"name": "rustc", "version": "test"},
            "dependencies": {"core": "1"},
            "commit_sha": commit,
        },
    }


def write_publication(tmp_path, entry, input_segments=None):
    tmp_path.mkdir(parents=True, exist_ok=True)
    if input_segments is None:
        input_segments = entry["minimum_input_segments"]
    records = []
    for index in range(1, 6):
        value = record(
            entry["workload_id"],
            entry["lane"],
            index,
            input_segments,
        )
        path = tmp_path / f"{entry['entry_id']}-record-{index}.json"
        path.write_text(json.dumps(value))
        records.append(path)
    publication = PUBLISHER.publish(records, "dedicated", 5)
    path = tmp_path / f"{entry['entry_id']}-publication.json"
    path.write_text(json.dumps(publication, indent=2) + "\n")
    return path


def entries():
    return json.loads(
        (ROOT / "benchmarks/production-baseline-suite-v1.json").read_text()
    )["required_publications"]


def test_production_baseline_suite_manifest_is_schema_valid():
    manifest = json.loads(
        (ROOT / "benchmarks/production-baseline-suite-v1.json").read_text()
    )
    schema = json.loads(
        (ROOT / "benchmarks/production-baseline-suite-v1.schema.json").read_text()
    )
    validate(manifest, schema)
    SUITE.validate_suite_document(ROOT / "benchmarks/production-baseline-suite-v1.json")
    assert len(manifest["required_publications"]) == 7
    assert {entry["minimum_input_segments"] for entry in manifest["required_publications"]} >= {
        1000,
        10000,
        100000,
    }


def test_complete_suite_emits_deterministic_evidence(tmp_path):
    publication_paths = [write_publication(tmp_path, entry) for entry in entries()]
    evidence = SUITE.validate_baseline_suite(
        ROOT / "benchmarks/production-baseline-suite-v1.json", publication_paths[::-1]
    )

    assert evidence["publication_count"] == 7
    assert evidence["runner_class"] == "dedicated"
    assert evidence["environment"]["commit_sha"] == "b" * 40
    assert [item["entry_id"] for item in evidence["publications"]] == sorted(
        item["entry_id"] for item in evidence["publications"]
    )
    assert all(item["record_count"] == 5 for item in evidence["publications"])

    output = tmp_path / "evidence.json"
    output.write_text(json.dumps(evidence, indent=2) + "\n")
    validate(
        evidence,
        json.loads(
            (ROOT / "benchmarks/production-baseline-evidence-v1.schema.json").read_text()
        ),
    )


def test_suite_rejects_missing_duplicate_mixed_and_under_sized_publications(tmp_path):
    publication_paths = [write_publication(tmp_path, entry) for entry in entries()]
    suite_path = ROOT / "benchmarks/production-baseline-suite-v1.json"

    with pytest.raises(ValueError, match="requires 7 publications"):
        SUITE.validate_baseline_suite(suite_path, publication_paths[:-1])

    with pytest.raises(ValueError, match="duplicate publication"):
        SUITE.validate_baseline_suite(suite_path, publication_paths[:-1] + [publication_paths[0]])

    mixed_path = publication_paths[-1]
    mixed = json.loads(mixed_path.read_text())
    for value in mixed["records"]:
        value["environment"]["commit_sha"] = "c" * 40
    mixed_path.write_text(json.dumps(mixed))
    with pytest.raises(ValueError, match="one architecture, OS, compiler, and commit"):
        SUITE.validate_baseline_suite(suite_path, publication_paths)

    undersized_paths = [write_publication(tmp_path / "undersized", entry) for entry in entries()]
    undersized = entries()[-1]
    undersized_path = tmp_path / "undersized" / f"{undersized['entry_id']}-publication.json"
    value = json.loads(undersized_path.read_text())
    for record_value in value["records"]:
        record_value["work"]["input_segments"] = 999
    undersized_path.write_text(json.dumps(value))
    with pytest.raises(ValueError, match="fewer than 100000 segments"):
        SUITE.validate_baseline_suite(suite_path, undersized_paths)
