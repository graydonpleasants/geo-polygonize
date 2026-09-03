#!/usr/bin/env python3
"""Validate repeated benchmark records before creating a publication artifact."""

import argparse
import json
import statistics
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).parent


def stable_record_value(record, field):
    """Return the portion of a record that identifies its workload context.

    Component-memory evidence includes allocator and worker-scheduling
    effects. Keep it in the published record, but do not treat it as record
    identity.
    """
    value = record.get(field)
    if field in ("partition_router", "stitching") and value is not None:
        value = dict(value)
        value.pop("measurement")
    if field != "work":
        return value
    work = dict(value)
    work.pop("component_memory", None)
    return work


def load(name):
    return json.loads((ROOT / name).read_text())


def relative_mad_percent(values, label):
    median = statistics.median(values)
    if median <= 0:
        raise ValueError(f"decision-quality {label} p50 must be positive")
    relative_mad = (
        statistics.median(abs(value - median) for value in values) / median * 100
    )
    return relative_mad


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
        "partition_router",
        "stitching",
    ]
    baseline = records[0]
    if any(
        stable_record_value(record, field) != stable_record_value(baseline, field)
        for record in records[1:]
        for field in stable_fields
    ):
        raise ValueError(
            "records do not describe one commit, environment, workload, and artifact"
        )

    relative_mad = relative_mad_percent(
        [record["measurement"]["p50_ms"] for record in records], "end-to-end"
    )
    if relative_mad > quality["maximum_relative_mad_percent"]:
        raise ValueError("p50 dispersion exceeds the decision-quality limit")

    router_relative_mad = None
    if baseline.get("partition_router") is not None:
        router_relative_mad = relative_mad_percent(
            [record["partition_router"]["measurement"]["p50_ms"] for record in records],
            "partition router",
        )
        if router_relative_mad > quality["maximum_relative_mad_percent"]:
            raise ValueError(
                "partition router p50 dispersion exceeds the decision-quality limit"
            )

    stitching_relative_mad = None
    if baseline.get("stitching") is not None:
        stitching_relative_mad = relative_mad_percent(
            [record["stitching"]["measurement"]["p50_ms"] for record in records],
            "stitched output",
        )
        if stitching_relative_mad > quality["maximum_relative_mad_percent"]:
            raise ValueError(
                "stitched output p50 dispersion exceeds the decision-quality limit"
            )

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
    if router_relative_mad is not None:
        publication["partition_router_p50_relative_mad_percent"] = router_relative_mad
    if stitching_relative_mad is not None:
        publication["stitching_p50_relative_mad_percent"] = stitching_relative_mad
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
