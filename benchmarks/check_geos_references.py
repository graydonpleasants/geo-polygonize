#!/usr/bin/env python3
"""Check every GEOS-comparable parity workload without recording timings."""

import argparse
import contextlib
import filecmp
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import jsonschema

import reference_geos


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--serial-binary", type=Path)
    parser.add_argument("--validation-output-dir", type=Path)
    parser.add_argument("--repeat", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    binary = args.binary or root / "target/release/examples/benchmark_record"
    serial_binary = args.serial_binary
    if not binary.is_absolute():
        binary = root / binary
    if serial_binary and not serial_binary.is_absolute():
        serial_binary = root / serial_binary
    if not binary.is_file():
        raise SystemExit(f"build {binary} before checking references")
    if serial_binary and not serial_binary.is_file():
        raise SystemExit(f"build {serial_binary} before checking serial equality")
    manifest_path = args.manifest or (
        root / "crates/geo-polygonize-core/tests/workloads/manifest-v1.json"
    )
    if not manifest_path.is_absolute():
        manifest_path = root / manifest_path
    manifest = json.loads(manifest_path.read_text())
    schema = json.loads(
        (root / "benchmarks/reference-result-v1.schema.json").read_text()
    )
    validation_schema = json.loads(
        (root / "benchmarks/production-validation-v1.schema.json").read_text()
    )

    checked = 0
    directory = (
        contextlib.nullcontext(args.output_dir)
        if args.output_dir
        else tempfile.TemporaryDirectory()
    )
    if args.output_dir:
        args.output_dir.mkdir(parents=True, exist_ok=True)
    if args.validation_output_dir:
        args.validation_output_dir.mkdir(parents=True, exist_ok=True)
    with directory as temporary:
        temporary = Path(temporary)
        validation_dir = args.validation_output_dir or (
            temporary / "validation" if serial_binary or args.repeat else None
        )
        if validation_dir:
            validation_dir.mkdir(parents=True, exist_ok=True)
        for workload in manifest["workloads"]:
            if workload["compatibility_class"] != "parity":
                continue
            for lane in reference_geos.LANES:
                if lane not in workload["permitted_profiles"]:
                    continue
                reference_path = temporary / f"{workload['id']}-{lane}.json"
                subprocess.run(
                    [
                        sys.executable,
                        str(root / "benchmarks/reference_geos.py"),
                        "--lane",
                        lane,
                        "--workload",
                        workload["id"],
                        "--manifest",
                        str(manifest_path),
                        "--output",
                        str(reference_path),
                    ],
                    check=True,
                )
                jsonschema.validate(json.loads(reference_path.read_text()), schema)
                check_command = [
                    str(binary),
                    "--lane",
                    lane,
                    "--workload",
                    workload["id"],
                    "--manifest",
                    str(manifest_path),
                    "--reference-result",
                    str(reference_path),
                    "--check-only",
                ]
                validation_path = None
                if validation_dir:
                    validation_path = validation_dir / f"{workload['id']}-{lane}.json"
                    check_command += ["--check-only-output", str(validation_path)]
                subprocess.run(check_command, check=True)
                if validation_path:
                    jsonschema.validate(json.loads(validation_path.read_text()), validation_schema)
                if serial_binary:
                    serial_path = validation_dir / f"{workload['id']}-{lane}-serial.json"
                    subprocess.run(
                        [
                            str(serial_binary),
                            "--lane",
                            lane,
                            "--workload",
                            workload["id"],
                            "--manifest",
                            str(manifest_path),
                            "--reference-result",
                            str(reference_path),
                            "--check-only",
                            "--check-only-output",
                            str(serial_path),
                        ],
                        check=True,
                    )
                    if not filecmp.cmp(validation_path, serial_path, shallow=False):
                        raise SystemExit(
                            f"serial/parallel validation differs for {workload['id']} {lane}"
                        )
                if args.repeat:
                    repeat_path = validation_dir / f"{workload['id']}-{lane}-repeat.json"
                    subprocess.run(
                        check_command[:-2]
                        + ["--check-only-output", str(repeat_path)],
                        check=True,
                    )
                    if not filecmp.cmp(validation_path, repeat_path, shallow=False):
                        raise SystemExit(
                            f"repeated validation differs for {workload['id']} {lane}"
                        )
                checked += 1
    qualifiers = []
    if serial_binary:
        qualifiers.append("serial/parallel equality")
    if args.repeat:
        qualifiers.append("repeated-run determinism")
    suffix = f" ({', '.join(qualifiers)})" if qualifiers else ""
    print(f"validated {checked} GEOS reference workload/lane combinations{suffix}")


if __name__ == "__main__":
    main()
