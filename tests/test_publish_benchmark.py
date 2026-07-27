import importlib.util
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

PATH = Path(__file__).resolve().parents[1] / "benchmarks/publish_benchmark.py"
SPEC = importlib.util.spec_from_file_location("publish_benchmark", PATH)
PUBLISHER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISHER)


def record(index, p50=100.0):
    return {
        "schema_version": 1,
        "record_id": f"coverage-abcdef123456-already-noded-r{index}",
        "workload_id": "coverage",
        "lane": "already-noded-polygonization",
        "implementation": {"name": "core", "version": "1", "features": []},
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
            "p50_ms": p50,
            "p95_ms": p50 + 1,
            "throughput": {"value": 10, "unit": "input-segments/second"},
            "samples": 30,
            "phase_times_ms": {"polygonize": p50},
            "allocations": {"count": 1, "bytes": 1},
            "peak_rss_bytes": 1,
        },
        "work": {
            "input_line_strings": 1,
            "input_segments": 1,
            "input_coordinates": 2,
            "output_polygons": 1,
            "output_coordinates": 5,
            "candidate_pairs": 0,
            "exact_predicate_calls": 0,
            "split_events": 0,
            "segment_expansion": {
                "input_segments": 1,
                "noded_segments": 1,
                "ratio": 1,
            },
        },
        "environment": {
            "architecture": "x86_64",
            "os": {"name": "Linux", "version": "1"},
            "compiler": {"name": "rustc", "version": "1"},
            "dependencies": {"core": "1"},
            "commit_sha": "b" * 40,
        },
    }


def write_records(tmp_path, records):
    tmp_path.mkdir(parents=True, exist_ok=True)
    paths = []
    for index, value in enumerate(records):
        path = tmp_path / f"{index}.json"
        path.write_text(json.dumps(value))
        paths.append(path)
    return paths


def test_publication_enforces_decision_quality_policy(tmp_path):
    paths = write_records(
        tmp_path,
        [
            record(index, p50)
            for index, p50 in enumerate([100, 101, 99, 100.5, 99.5], 1)
        ],
    )
    publication = PUBLISHER.publish(paths, "dedicated", 5)
    assert publication["measurement_class"] == "decision-quality"
    assert publication["process_repetitions"] == 5
    assert publication["p50_relative_mad_percent"] == 0.5

    with pytest.raises(ValueError, match="runner"):
        PUBLISHER.publish(paths, "shared-hosted", 5)
    with pytest.raises(ValueError, match="warmup"):
        PUBLISHER.publish(paths, "dedicated", 4)
    with pytest.raises(ValueError, match="process repetitions"):
        PUBLISHER.publish(paths[:4], "dedicated", 5)
    with pytest.raises(ValueError, match="record IDs"):
        PUBLISHER.publish(paths[:-1] + [paths[0]], "dedicated", 5)

    mixed = [record(index) for index in range(1, 6)]
    mixed[-1]["environment"]["commit_sha"] = "c" * 40
    with pytest.raises(ValueError, match="one commit"):
        PUBLISHER.publish(write_records(tmp_path / "mixed", mixed), "dedicated", 5)

    noisy = write_records(
        tmp_path / "noisy",
        [record(index, p50) for index, p50 in enumerate([100, 110, 90, 120, 80], 1)],
    )
    with pytest.raises(ValueError, match="dispersion"):
        PUBLISHER.publish(noisy, "dedicated", 5)


def test_decision_schema_keeps_rejections_and_crossovers_linked():
    schema = json.loads(
        (PATH.parent / "benchmark-decision-v1.schema.json").read_text()
    )
    Draft202012Validator.check_schema(schema)
    artifact = {"uri": "artifacts/candidate.json", "sha256": "a" * 64}
    decision = {
        "schema_version": 1,
        "decision_id": "candidate-layout-v1",
        "title": "Candidate layout experiment",
        "policy_id": "benchmark-decision-v1",
        "hypothesis": "The candidate reduces end-to-end p50.",
        "target_workloads": ["dense-crossings-v1"],
        "non_targets": ["small sparse inputs"],
        "semantic_invariants": ["canonical topology remains equal"],
        "predeclared_thresholds": {
            "primary_metric": "p50_ms",
            "minimum_effect_size_percent": 5.0,
            "regression_budget_percent": 2.0,
        },
        "outcome": "rejected",
        "rationale": "The candidate missed the effect-size threshold.",
        "baseline_publications": [
            {"uri": "artifacts/baseline.json", "sha256": "b" * 64}
        ],
        "candidate_publications": [artifact],
        "rejected_experiments": [
            {
                "name": "candidate layout",
                "reason": "End-to-end p50 improved by less than 5%.",
                "publications": [artifact],
            }
        ],
        "crossover": {
            "status": "measured",
            "range": {
                "descriptor": "input segments",
                "lower_bound": 1000,
                "upper_bound": 2000,
                "unit": "segments",
            },
            "publications": [artifact],
        },
    }
    validator = Draft202012Validator(schema)
    validator.validate(decision)
    for path in (PATH.parent / "decisions").glob("*.json"):
        validator.validate(json.loads(path.read_text()))

    decision["rejected_experiments"][0]["publications"] = []
    with pytest.raises(ValidationError):
        validator.validate(decision)
    decision["rejected_experiments"][0]["publications"] = [artifact]
    decision["crossover"].pop("publications")
    with pytest.raises(ValidationError):
        validator.validate(decision)
