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
ANALYZER = load_module(
    "analyze_component_memory", "benchmarks/analyze_component_memory.py"
)


COMPONENT_MEMORY_FIELDS = ANALYZER.COMPONENT_MEMORY_FIELDS


def record(workload_id, index, commit="b" * 40):
    return {
        "schema_version": 1,
        "record_id": f"{workload_id}-r{index}",
        "workload_id": workload_id,
        "artifact_sha256": "a" * 64,
        "lane": "already-noded-polygonization",
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
            "p50_ms": 10 + index / 100,
            "p95_ms": 12 + index / 100,
            "throughput": {"value": 100, "unit": "input-segments/second"},
            "samples": 30,
            "phase_times_ms": {"polygonize": 10},
            "allocations": {"count": 10, "bytes": 1000 + index},
            "peak_rss_bytes": 2000 + index,
        },
        "work": {
            "input_line_strings": 4,
            "input_segments": 100,
            "input_coordinates": 200,
            "output_polygons": 1,
            "output_coordinates": 5,
            "candidate_pairs": 1,
            "exact_predicate_calls": 1,
            "split_events": 1,
            "segment_expansion": {
                "input_segments": 100,
                "noded_segments": 100,
                "ratio": 1,
            },
            "component_memory": {
                field: 1 for field in COMPONENT_MEMORY_FIELDS
            },
        },
        "environment": {
            "architecture": "x86_64",
            "os": {"name": "Linux", "version": "test"},
            "compiler": {"name": "rustc", "version": "test"},
            "dependencies": {"core": "1"},
            "commit_sha": commit,
        },
    }


def write_publication(tmp_path, workload_id="balanced-components-v1"):
    tmp_path.mkdir(parents=True, exist_ok=True)
    records = []
    for index in range(1, 6):
        path = tmp_path / f"record-{index}.json"
        path.write_text(json.dumps(record(workload_id, index)))
        records.append(path)
    publication = PUBLISHER.publish(records, "dedicated", 5)
    path = tmp_path / "publication.json"
    path.write_text(json.dumps(publication, indent=2) + "\n")
    return path


def test_component_memory_report_is_schema_valid_and_deterministic(tmp_path):
    publication = write_publication(tmp_path)
    report = ANALYZER.analyze_component_memory([publication])

    assert report["publication_count"] == 1
    assert report["report_id"].startswith("component-memory-")
    summary = report["publications"][0]
    assert summary["component_memory"]["component_count"] == 1
    assert summary["derived"]["largest_component_edge_fraction"] == 1
    assert summary["derived"]["scratch_instances_per_worker"] == 1
    assert summary["derived"]["vec_vec_storage_words"] == 4
    assert summary["derived"]["csr_storage_words"] == 2
    assert summary["derived"]["csr_to_vec_vec_storage_ratio"] == 0.5
    validate(
        report,
        json.loads(
            (ROOT / "benchmarks/component-memory-evidence-v1.schema.json").read_text()
        ),
    )
    assert report == ANALYZER.analyze_component_memory([publication])


def test_component_memory_report_rejects_mixed_environments(tmp_path):
    first = write_publication(tmp_path / "first", "first-v1")
    second = write_publication(tmp_path / "second", "second-v1")
    value = json.loads(second.read_text())
    for record_value in value["records"]:
        record_value["environment"]["commit_sha"] = "c" * 40
    second.write_text(json.dumps(value))

    with pytest.raises(ValueError, match="one environment"):
        ANALYZER.analyze_component_memory([first, second])
