#!/usr/bin/env python3
"""Check every GEOS-comparable parity workload without recording timings."""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import jsonschema

import reference_geos


def main():
    root = Path(__file__).resolve().parents[1]
    binary = root / "target/release/examples/benchmark_record"
    if not binary.is_file():
        raise SystemExit(f"build {binary} before checking references")
    manifest = json.loads(
        (
            root / "crates/geo-polygonize-core/tests/workloads/manifest-v1.json"
        ).read_text()
    )
    schema = json.loads(
        (root / "benchmarks/reference-result-v1.schema.json").read_text()
    )

    checked = 0
    with tempfile.TemporaryDirectory() as temporary:
        for workload in manifest["workloads"]:
            if workload["compatibility_class"] != "parity":
                continue
            for lane in reference_geos.LANES:
                if lane not in workload["permitted_profiles"]:
                    continue
                reference_path = Path(temporary) / f"{workload['id']}-{lane}.json"
                subprocess.run(
                    [
                        sys.executable,
                        str(root / "benchmarks/reference_geos.py"),
                        "--lane",
                        lane,
                        "--workload",
                        workload["id"],
                        "--output",
                        str(reference_path),
                    ],
                    check=True,
                )
                jsonschema.validate(json.loads(reference_path.read_text()), schema)
                subprocess.run(
                    [
                        str(binary),
                        "--lane",
                        lane,
                        "--workload",
                        workload["id"],
                        "--reference-result",
                        str(reference_path),
                        "--check-only",
                    ],
                    check=True,
                )
                checked += 1
    print(f"validated {checked} GEOS reference workload/lane combinations")


if __name__ == "__main__":
    main()
