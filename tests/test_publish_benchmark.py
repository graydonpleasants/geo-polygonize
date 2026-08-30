import importlib.util
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

PATH = Path(__file__).resolve().parents[1] / "benchmarks/publish_benchmark.py"
SPEC = importlib.util.spec_from_file_location("publish_benchmark", PATH)
PUBLISHER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISHER)
TREND_PATH = PATH.parent / "render_benchmark_trends.py"
TREND_SPEC = importlib.util.spec_from_file_location(
    "render_benchmark_trends", TREND_PATH
)
TREND_RENDERER = importlib.util.module_from_spec(TREND_SPEC)
TREND_SPEC.loader.exec_module(TREND_RENDERER)


def record(index, p50=100.0):
    return {
        "schema_version": 1,
        "record_id": f"coverage-abcdef123456-already-noded-r{index}",
        "workload_id": "coverage",
        "artifact_sha256": "c" * 64,
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
            "layout_candidate": {
                "candidate_id": "packed-csr-adjacency-v1",
                "conformance": True,
                "samples": 30,
                "node_count": 1,
                "nested_storage_words": 4,
                "csr_storage_words": 2,
                "nested_traversal_p50_ns": 10,
                "csr_materialization_ns": 5,
                "csr_traversal_p50_ns": 4,
            },
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


def router_record(index, p50=10.0):
    value = record(index)
    value["partition_router"] = {
        "config": {"tile_size": 10.0, "buffer": 1.0},
        "correctness_gate": {
            "schema_version": 1,
            "oracle_difference": None,
            "routed_assignment_oracle_difference": None,
            "routed_local_snapshot_difference": None,
            "routed_local_snapshot_checked_partition_count": 1,
            "router_work": {
                "source_segment_count": 1,
                "direct_assignment_count": 1,
                "slow_path_segment_count": 0,
                "candidate_partition_visit_count": 0,
                "exact_intersection_test_count": 0,
                "emitted_assignment_count": 1,
            },
            "assignments": [
                {
                    "partition_id": 0,
                    "geometry_envelope_segment_count": 1,
                    "independent_segment_count": 1,
                    "routed_segment_count": 1,
                    "geometry_envelope_false_positive_count": 0,
                }
            ],
        },
        "measurement": {
            "p50_ms": p50,
            "p95_ms": p50 + 0.1,
            "samples": 30,
            "allocations": {"count": 2, "bytes": 7},
        },
    }
    return value


def write_records(tmp_path, records):
    tmp_path.mkdir(parents=True, exist_ok=True)
    paths = []
    for index, value in enumerate(records):
        path = tmp_path / f"{index}.json"
        path.write_text(json.dumps(value))
        paths.append(path)
    return paths


def decision_record():
    artifact = {"uri": "artifacts/candidate.json", "sha256": "a" * 64}
    return {
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

    mixed_artifact = [record(index) for index in range(1, 6)]
    mixed_artifact[-1]["artifact_sha256"] = "d" * 64
    with pytest.raises(
        ValueError, match="one commit, environment, workload, and artifact"
    ):
        PUBLISHER.publish(
            write_records(tmp_path / "mixed-artifact", mixed_artifact), "dedicated", 5
        )

    noisy = write_records(
        tmp_path / "noisy",
        [record(index, p50) for index, p50 in enumerate([100, 110, 90, 120, 80], 1)],
    )
    with pytest.raises(ValueError, match="dispersion"):
        PUBLISHER.publish(noisy, "dedicated", 5)


def test_partition_router_publication_is_stable_and_dispersion_gated(tmp_path):
    records = [
        router_record(index, p50)
        for index, p50 in enumerate([10, 10.1, 9.9, 10.05, 9.95], 1)
    ]
    paths = write_records(tmp_path / "router", records)
    publication = PUBLISHER.publish(paths, "dedicated", 5)
    assert publication["partition_router_p50_relative_mad_percent"] == pytest.approx(
        0.5
    )

    mixed = [router_record(index) for index in range(1, 6)]
    mixed[-1]["partition_router"]["config"]["tile_size"] = 20.0
    with pytest.raises(ValueError, match="one commit"):
        PUBLISHER.publish(
            write_records(tmp_path / "mixed-router", mixed), "dedicated", 5
        )

    noisy = [
        router_record(index, p50) for index, p50 in enumerate([10, 11, 9, 12, 8], 1)
    ]
    with pytest.raises(ValueError, match="partition router p50 dispersion"):
        PUBLISHER.publish(
            write_records(tmp_path / "noisy-router", noisy), "dedicated", 5
        )

    publication_path = tmp_path / "router-publication.json"
    publication_path.write_text(json.dumps(publication))
    dashboard = TREND_RENDERER.render([publication_path], [])
    assert "| 10.000 | 7 | 5 | 0.000 | 0.500 |" in dashboard


def test_decision_schema_keeps_rejections_and_crossovers_linked():
    schema = json.loads((PATH.parent / "benchmark-decision-v1.schema.json").read_text())
    Draft202012Validator.check_schema(schema)
    decision = decision_record()
    validator = Draft202012Validator(schema)
    validator.validate(decision)
    for path in (PATH.parent / "decisions").glob("*.json"):
        validator.validate(json.loads(path.read_text()))

    decision["rejected_experiments"][0]["publications"] = []
    with pytest.raises(ValidationError):
        validator.validate(decision)
    decision["rejected_experiments"][0]["publications"] = decision[
        "candidate_publications"
    ]
    decision["crossover"].pop("publications")
    with pytest.raises(ValidationError):
        validator.validate(decision)


def test_trend_renderer_uses_only_schema_valid_evidence(tmp_path):
    record_paths = write_records(
        tmp_path / "records",
        [
            record(index, p50)
            for index, p50 in enumerate([100, 101, 99, 100.5, 99.5], 1)
        ],
    )
    publication_path = tmp_path / "publication.json"
    publication_path.write_text(
        json.dumps(PUBLISHER.publish(record_paths, "dedicated", 5))
    )
    decision = decision_record()
    decision["target_workloads"] = ["dense|crossings"]
    decision_path = tmp_path / "decision.json"
    decision_path.write_text(json.dumps(decision))

    dashboard = TREND_RENDERER.render([publication_path], [decision_path])
    assert "| coverage | already-noded-polygonization | x86_64 |" in dashboard
    assert "| candidate-layout-v1 | rejected | dense\\|crossings |" in dashboard
    assert "1000–2000 segments (input segments)" in dashboard

    publication = json.loads(publication_path.read_text())
    publication["measurement_class"] = "diagnostic"
    publication_path.write_text(json.dumps(publication))
    with pytest.raises(ValidationError):
        TREND_RENDERER.render([publication_path], [decision_path])
    publication["measurement_class"] = "decision-quality"
    publication["records"][-1]["measurement"]["throughput"]["unit"] = "items/second"
    publication_path.write_text(json.dumps(publication))
    with pytest.raises(ValueError, match="throughput units"):
        TREND_RENDERER.render([publication_path], [decision_path])
