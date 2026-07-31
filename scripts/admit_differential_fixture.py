#!/usr/bin/env python3
"""Validate a persisted differential fixture before admitting it to the corpus."""

import argparse
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CASE_ID = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
CLASSIFICATIONS = ("expected_parity", "expected_divergence", "invalid_ambiguous")
PRODUCERS = {"adapter_differential": ("one_shot", "workspace")}


def reviewed_fixture(candidate, case_id, classification):
    if not CASE_ID.fullmatch(case_id):
        raise ValueError("candidate has an invalid case_id")
    if classification not in CLASSIFICATIONS:
        raise ValueError("candidate has an invalid classification")
    expected_keys = {
        "schema_version",
        "producer",
        "input",
        "options",
        "versions",
        "baseline",
        "comparison",
    }
    if set(candidate) != expected_keys or candidate["schema_version"] != 1:
        raise ValueError("expected an exact DifferentialMismatchCandidateV1")
    for run in (candidate["baseline"], candidate["comparison"]):
        if not isinstance(run, dict) or set(run) != {"implementation", "outcome"}:
            raise ValueError("expected exact differential runs")
        outcome = run["outcome"]
        if (
            not isinstance(outcome, dict)
            or set(outcome) != {"status", "value"}
            or outcome["status"] not in ("success", "error")
            or not isinstance(outcome["value"], dict)
        ):
            raise ValueError("expected exact normalized outcomes")
    labels = PRODUCERS.get(candidate["producer"])
    actual_labels = (
        candidate["baseline"].get("implementation"),
        candidate["comparison"].get("implementation"),
    )
    if labels is None or actual_labels != labels:
        raise ValueError("unknown producer or implementation labels")
    if candidate["baseline"].get("outcome") == candidate["comparison"].get("outcome"):
        raise ValueError("candidate outcomes must differ")
    if not isinstance(candidate["input"], list) or not isinstance(candidate["options"], dict):
        raise ValueError("candidate input and options are required")
    if not isinstance(candidate["versions"], dict) or not candidate["versions"]:
        raise ValueError("candidate versions are required")
    return {
        "schema_version": 2,
        "case_id": case_id,
        "classification": classification,
        "candidate": candidate,
    }


def write_fixture(path, payload):
    with path.open("x") as output:
        json.dump(payload, output, indent=2, allow_nan=False)
        output.write("\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--case-id")
    parser.add_argument("--classification", choices=CLASSIFICATIONS)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()

    candidate = args.candidate.resolve(strict=True)
    payload = json.loads(candidate.read_text())
    if not isinstance(payload, dict):
        raise SystemExit("candidate must be a JSON object")
    is_mismatch_candidate = "producer" in payload
    if is_mismatch_candidate:
        if not args.case_id or not args.classification:
            raise SystemExit("candidate review requires --case-id and --classification")
        try:
            payload = reviewed_fixture(payload, args.case_id, args.classification)
        except (KeyError, TypeError, ValueError) as error:
            raise SystemExit(str(error)) from error
        case_id = args.case_id
        test = "persisted_differential_v2"
        env_name = "PERSISTED_DIFFERENTIAL_V2_CANDIDATE"
        default_output_dir = ROOT / "fixtures/differential-v2"
    else:
        if args.case_id or args.classification:
            raise SystemExit("review flags require a DifferentialMismatchCandidateV1")
        case_id = payload.get("case_id", "")
        if not CASE_ID.fullmatch(case_id):
            raise SystemExit("candidate has an invalid case_id")
        test = "persisted_differential"
        env_name = "PERSISTED_DIFFERENTIAL_CANDIDATE"
        default_output_dir = ROOT / "fixtures/differential"

    with tempfile.TemporaryDirectory() as temporary:
        reviewed = Path(temporary) / f"{case_id}.json"
        reviewed.write_text(json.dumps(payload, indent=2, allow_nan=False) + "\n")
        env = os.environ.copy()
        env[env_name] = str(reviewed)
        subprocess.run(
            ["cargo", "test", "-p", "geo-polygonize-core", "--test", test],
            cwd=ROOT,
            env=env,
            check=True,
        )
    if args.check_only:
        return

    output_dir = args.output_dir or default_output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    destination = output_dir / f"{case_id}.json"
    write_fixture(destination, payload)
    print(destination)


if __name__ == "__main__":
    main()
