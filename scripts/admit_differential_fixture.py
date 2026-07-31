#!/usr/bin/env python3
"""Validate a persisted differential fixture before admitting it to the corpus."""

import argparse
import json
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CASE_ID = re.compile(r"^[a-z0-9][a-z0-9._-]*$")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "fixtures/differential")
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()

    candidate = args.candidate.resolve(strict=True)
    payload = json.loads(candidate.read_text())
    case_id = payload.get("case_id", "")
    if not CASE_ID.fullmatch(case_id):
        raise SystemExit("candidate has an invalid case_id")

    env = os.environ.copy()
    env["PERSISTED_DIFFERENTIAL_CANDIDATE"] = str(candidate)
    subprocess.run(
        ["cargo", "test", "-p", "geo-polygonize-core", "--test", "persisted_differential"],
        cwd=ROOT,
        env=env,
        check=True,
    )
    if args.check_only:
        return

    args.output_dir.mkdir(parents=True, exist_ok=True)
    destination = args.output_dir / f"{case_id}.json"
    with destination.open("x") as output:
        json.dump(payload, output, indent=2, allow_nan=False)
        output.write("\n")
    print(destination)


if __name__ == "__main__":
    main()
