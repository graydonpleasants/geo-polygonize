#!/usr/bin/env python3
"""Fail closed unless a complete dedicated baseline suite is present."""

import argparse
import hashlib
import json
import statistics
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parent
RECORD_SCHEMA_NAME = "benchmark-record-v1.schema.json"
PUBLICATION_SCHEMA_NAME = "benchmark-publication-v1.schema.json"
SUITE_SCHEMA_NAME = "production-baseline-suite-v1.schema.json"
EVIDENCE_SCHEMA_NAME = "production-baseline-evidence-v1.schema.json"
QUALITY = {
    "runner_class": "dedicated",
    "measurement_class": "decision-quality",
    "minimum_samples_per_process": 30,
    "minimum_process_repetitions": 5,
    "minimum_warmup_iterations": 5,
    "maximum_relative_mad_percent": 3.0,
}
STABLE_RECORD_FIELDS = (
    "workload_id",
    "artifact_sha256",
    "lane",
    "implementation",
    "correctness_gate",
    "topology",
    "work",
    "environment",
)
ENVIRONMENT_FIELDS = ("architecture", "os", "compiler", "commit_sha")


def load_json(path):
    return json.loads(Path(path).read_text())


def load_schema(name):
    return load_json(ROOT / name)


def stable_record_value(record, field):
    """Return the portion of a record that identifies its workload context.

    Component-memory evidence includes allocator and worker-scheduling
    effects. Keep it in the published record, but do not treat it as record
    identity.
    """
    value = record[field]
    if field != "work":
        return value
    work = dict(value)
    work.pop("component_memory", None)
    return work


def validate_document(document, schema, name, registry=None):
    validator = (
        Draft202012Validator(schema)
        if registry is None
        else Draft202012Validator(schema, registry=registry)
    )
    errors = sorted(validator.iter_errors(document), key=lambda error: list(error.path))
    if errors:
        error = errors[0]
        location = ".".join(str(part) for part in error.path) or "document"
        raise ValueError(f"{name} schema validation failed at {location}: {error.message}")


def validate_suite_document(path):
    suite = load_json(path)
    schema = load_schema(SUITE_SCHEMA_NAME)
    validate_document(suite, schema, "baseline suite")
    entries = suite["required_publications"]
    entry_ids = [entry["entry_id"] for entry in entries]
    if len(set(entry_ids)) != len(entry_ids):
        raise ValueError("baseline suite entry IDs must be unique")
    if any(entry["lane"] not in entry["coverage"] for entry in entries):
        raise ValueError("baseline suite coverage must include each publication lane")
    keys = [(entry["workload_id"], entry["lane"]) for entry in entries]
    if len(set(keys)) != len(keys):
        raise ValueError("baseline suite workload/lane pairs must be unique")
    return suite


def _publication_validator():
    record_schema = load_schema(RECORD_SCHEMA_NAME)
    publication_schema = load_schema(PUBLICATION_SCHEMA_NAME)
    registry = Registry().with_resource(
        record_schema["$id"], Resource.from_contents(record_schema)
    )
    return publication_schema, registry


def _environment_identity(environment):
    return tuple(
        json.dumps(environment[field], sort_keys=True) if isinstance(environment[field], dict)
        else environment[field]
        for field in ENVIRONMENT_FIELDS
    )


def _validate_record_set(publication, path):
    records = publication["records"]
    if publication["process_repetitions"] != len(records):
        raise ValueError(f"{path}: process_repetitions does not match records")
    if len({record["record_id"] for record in records}) != len(records):
        raise ValueError(f"{path}: record IDs must be unique")

    baseline = records[0]
    for record in records[1:]:
        if any(
            stable_record_value(record, field)
            != stable_record_value(baseline, field)
            for field in STABLE_RECORD_FIELDS
        ):
            raise ValueError(f"{path}: records do not describe one stable workload and environment")

    for record in records:
        gate = record["correctness_gate"]
        if gate["status"] != "passed" or gate["validation"]["result"] != "passed":
            raise ValueError(f"{path}: every record must pass the correctness gate")
        if gate["compatibility"]["observed"] != "equal":
            raise ValueError(f"{path}: every baseline record must have equal reference topology")
        if gate["fingerprint"]["outcome"] != "equal":
            raise ValueError(f"{path}: every baseline record must have an equal fingerprint")

        measurement = record["measurement"]
        if measurement["samples"] < QUALITY["minimum_samples_per_process"]:
            raise ValueError(f"{path}: insufficient samples per process")
        if measurement["p50_ms"] <= 0 or measurement["p95_ms"] < measurement["p50_ms"]:
            raise ValueError(f"{path}: invalid timing percentiles")
        if measurement["throughput"]["value"] <= 0:
            raise ValueError(f"{path}: throughput must be positive")
        if measurement["throughput"]["unit"] != "input-segments/second":
            raise ValueError(f"{path}: unsupported throughput unit")

    if publication["p50_relative_mad_percent"] > QUALITY["maximum_relative_mad_percent"]:
        raise ValueError(f"{path}: p50 dispersion exceeds the decision-quality limit")
    return baseline, records


def _load_publication(path):
    path = Path(path)
    publication = load_json(path)
    publication_schema, registry = _publication_validator()
    validate_document(publication, publication_schema, str(path), registry)
    if publication["policy_id"] != "benchmark-decision-v1":
        raise ValueError(f"{path}: unexpected policy")
    if publication["measurement_class"] != QUALITY["measurement_class"]:
        raise ValueError(f"{path}: publication is not decision-quality")
    if publication["runner_class"] != QUALITY["runner_class"]:
        raise ValueError(f"{path}: publication is not from a dedicated runner")
    if publication["warmup_iterations"] < QUALITY["minimum_warmup_iterations"]:
        raise ValueError(f"{path}: insufficient warmup iterations")
    if publication["process_repetitions"] < QUALITY["minimum_process_repetitions"]:
        raise ValueError(f"{path}: insufficient process repetitions")

    baseline, records = _validate_record_set(publication, path)
    return {
        "path": path,
        "publication": publication,
        "baseline": baseline,
        "records": records,
        "publication_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def _median(records, field):
    return statistics.median(record["measurement"][field] for record in records)


def validate_baseline_suite(suite_path, publication_paths):
    suite = validate_suite_document(suite_path)
    expected = {
        (entry["workload_id"], entry["lane"]): entry
        for entry in suite["required_publications"]
    }
    if len(publication_paths) != len(expected):
        raise ValueError(
            f"baseline suite requires {len(expected)} publications, got {len(publication_paths)}"
        )

    contexts = [_load_publication(path) for path in publication_paths]
    seen = set()
    for context in contexts:
        baseline = context["baseline"]
        key = (baseline["workload_id"], baseline["lane"])
        if key not in expected:
            raise ValueError(f"unexpected publication for {key[0]} / {key[1]}")
        if key in seen:
            raise ValueError(f"duplicate publication for {key[0]} / {key[1]}")
        seen.add(key)

        entry = expected[key]
        if baseline["work"]["input_segments"] < entry["minimum_input_segments"]:
            raise ValueError(
                f"{entry['entry_id']}: input has fewer than {entry['minimum_input_segments']} segments"
            )
        for field in entry["required_work_fields"]:
            if field not in baseline["work"]:
                raise ValueError(f"{entry['entry_id']}: work.{field} is required")

    missing = set(expected) - seen
    if missing:
        formatted = ", ".join(f"{workload} / {lane}" for workload, lane in sorted(missing))
        raise ValueError(f"missing baseline publications: {formatted}")

    first = contexts[0]["baseline"]
    suite_environment = _environment_identity(first["environment"])
    implementation = first["implementation"]
    for context in contexts[1:]:
        baseline = context["baseline"]
        if _environment_identity(baseline["environment"]) != suite_environment:
            raise ValueError("baseline publications must use one architecture, OS, compiler, and commit")
        if baseline["implementation"] != implementation:
            raise ValueError("baseline publications must use one implementation")

    summaries = []
    for context in contexts:
        baseline = context["baseline"]
        records = context["records"]
        entry = expected[(baseline["workload_id"], baseline["lane"])]
        summaries.append(
            {
                "entry_id": entry["entry_id"],
                "workload_id": baseline["workload_id"],
                "lane": baseline["lane"],
                "publication_id": context["publication"]["publication_id"],
                "publication_sha256": context["publication_sha256"],
                "artifact_sha256": baseline["artifact_sha256"],
                "record_count": len(records),
                "input_segments": baseline["work"]["input_segments"],
                "median_p50_ms": _median(records, "p50_ms"),
                "median_p95_ms": _median(records, "p95_ms"),
                "median_throughput": statistics.median(
                    record["measurement"]["throughput"]["value"] for record in records
                ),
                "median_allocated_bytes": statistics.median(
                    record["measurement"]["allocations"]["bytes"] for record in records
                ),
                "median_peak_rss_bytes": statistics.median(
                    record["measurement"]["peak_rss_bytes"] for record in records
                ),
            }
        )
    summaries.sort(key=lambda summary: summary["entry_id"])

    evidence = {
        "schema_version": 1,
        "suite_id": suite["suite_id"],
        "policy_id": suite["policy_id"],
        "measurement_class": QUALITY["measurement_class"],
        "runner_class": QUALITY["runner_class"],
        "publication_count": len(summaries),
        "environment": {
            field: first["environment"][field] for field in ENVIRONMENT_FIELDS
        },
        "publications": summaries,
    }
    validate_document(evidence, load_schema(EVIDENCE_SCHEMA_NAME), "baseline evidence")
    return evidence


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", type=Path, default=ROOT / "production-baseline-suite-v1.json")
    parser.add_argument("--publication", type=Path, action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    evidence = validate_baseline_suite(args.suite, args.publication)
    args.output.write_text(json.dumps(evidence, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
