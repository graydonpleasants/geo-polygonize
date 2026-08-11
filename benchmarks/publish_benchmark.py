#!/usr/bin/env python3
"""Validate repeated benchmark records before creating a publication artifact."""

import argparse
import json
import statistics
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).parent


def load(name):
    return json.loads((ROOT / name).read_text())


def publish(record_paths, runner_class, warmup_iterations):
    policy = load("benchmark-decision-policy-v1.json")
    quality = policy["measurement_classes"]["decision_quality"]
    if runner_class not in quality["allowed_runner_classes"]:
        raise ValueError(f"{runner_class} is not a decision-quality runner")
    if warmup_iterations < quality["warmup_iterations"]:
        raise ValueError("insufficient warmup iterations")
    if len(record_paths) < quality["minimum_process_repetitions"]:
        raise ValueError("insufficient process repetitions")

    record_schema = load("benchmark-record-v1.schema.json")
    record_validator = Draft202012Validator(record_schema)
    records = [json.loads(Path(path).read_text()) for path in record_paths]
    for record in records:
        record_validator.validate(record)
        if record["measurement"]["samples"] < quality["minimum_samples_per_process"]:
            raise ValueError("insufficient samples per process")

    if len({record["record_id"] for record in records}) != len(records):
        raise ValueError("record IDs must be unique")
    stable_fields = [
        "workload_id",
        "artifact_sha256",
        "lane",
        "implementation",
        "correctness_gate",
        "topology",
        "work",
        "environment",
    ]
    baseline = records[0]
    if any(
        record[field] != baseline[field]
        for record in records[1:]
        for field in stable_fields
    ):
        raise ValueError(
            "records do not describe one commit, environment, workload, and artifact"
        )

    p50_values = [record["measurement"]["p50_ms"] for record in records]
    median = statistics.median(p50_values)
    if median <= 0:
        raise ValueError("decision-quality p50 must be positive")
    relative_mad = (
        statistics.median(abs(value - median) for value in p50_values) / median * 100
    )
    if relative_mad > quality["maximum_relative_mad_percent"]:
        raise ValueError("p50 dispersion exceeds the decision-quality limit")

    records.sort(key=lambda record: record["record_id"])
    publication = {
        "schema_version": 1,
        "publication_id": (
            f"{baseline['workload_id']}-{baseline['environment']['commit_sha'][:12]}-"
            f"{baseline['lane']}"
        ),
        "policy_id": policy["policy_id"],
        "measurement_class": "decision-quality",
        "runner_class": runner_class,
        "warmup_iterations": warmup_iterations,
        "process_repetitions": len(records),
        "p50_relative_mad_percent": relative_mad,
        "records": records,
    }
    publication_schema = load("benchmark-publication-v1.schema.json")
    registry = Registry().with_resource(
        record_schema["$id"], Resource.from_contents(record_schema)
    )
    Draft202012Validator(publication_schema, registry=registry).validate(publication)
    return publication


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--record", action="append", required=True)
    parser.add_argument("--runner-class", required=True)
    parser.add_argument("--warmup-iterations", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    publication = publish(args.record, args.runner_class, args.warmup_iterations)
    args.output.write_text(json.dumps(publication, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
