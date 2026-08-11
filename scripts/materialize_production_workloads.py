#!/usr/bin/env python3
"""Materialize deterministic, out-of-tree workloads from a pinned source.

The raw PBF and derived GeoJSON files belong under ``target/`` (or another
caller-owned directory), never in Git.  The checked-in source manifest is the
authority for the input bytes; this script records the converter, selection
rule, structure descriptors, and output checksums needed to reproduce a run.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
from collections import Counter
from pathlib import Path
from statistics import mean


SCHEMA_VERSION = 1
DEFAULT_TARGETS = (1_000, 10_000, 100_000)
GRID_SIZE = 16


def canonical_json(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def digest_file(path, algorithm):
    digest = hashlib.new(algorithm)
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def percentile(values, fraction):
    ordered = sorted(values)
    if not ordered:
        return 0
    index = min(len(ordered) - 1, max(0, math.ceil(fraction * len(ordered)) - 1))
    return ordered[index]


def feature_key(feature):
    value = feature.get("properties", {}).get("osm_id")
    try:
        return (0, int(value))
    except (TypeError, ValueError):
        return (1, str(value or ""))


def parse_seq_feature(raw, line_number):
    record = raw.lstrip("\x1e").strip()
    if not record:
        return None
    try:
        feature = json.loads(record)
    except json.JSONDecodeError as error:
        raise ValueError(f"invalid GeoJSONSeq record at line {line_number}: {error}") from error
    geometry = feature.get("geometry") or {}
    if geometry.get("type") != "LineString":
        raise ValueError(
            f"source line {line_number} is {geometry.get('type')!r}; expected LineString"
        )
    coordinates = geometry.get("coordinates") or []
    if len(coordinates) < 2:
        return None
    properties = feature.get("properties") or {}
    if properties.get("highway") is None:
        raise ValueError(f"source line {line_number} has no highway tag after filtering")
    return {
        "type": "Feature",
        "properties": {
            "osm_id": str(properties.get("osm_id", "")),
            "highway": str(properties["highway"]),
        },
        "geometry": {"type": "LineString", "coordinates": coordinates},
    }


def iter_features(path):
    previous_key = None
    with path.open("r", encoding="utf-8") as stream:
        for line_number, raw in enumerate(stream, 1):
            feature = parse_seq_feature(raw, line_number)
            if feature is None:
                continue
            key = feature_key(feature)
            if previous_key is not None and key < previous_key:
                raise ValueError(
                    "GeoJSONSeq feature order is not monotonic by osm_id; "
                    "refuse to derive a parser-order-dependent workload"
                )
            previous_key = key
            yield feature


def coordinate_key(coordinate):
    x, y = coordinate[:2]
    return (0.0 if x == 0 else x, 0.0 if y == 0 else y)


def segment_key(start, end):
    first, second = coordinate_key(start), coordinate_key(end)
    return (first, second) if first <= second else (second, first)


def geometry_coordinates(features):
    return [coordinate for feature in features for coordinate in feature["geometry"]["coordinates"]]


def bbox(features):
    coordinates = geometry_coordinates(features)
    xs = [coordinate[0] for coordinate in coordinates]
    ys = [coordinate[1] for coordinate in coordinates]
    return [min(xs), min(ys), max(xs), max(ys)]


def structure_descriptor(features):
    chain_lengths = [len(feature["geometry"]["coordinates"]) - 1 for feature in features]
    coordinates = geometry_coordinates(features)
    minimum_x, minimum_y, maximum_x, maximum_y = bbox(features)
    width = max(maximum_x - minimum_x, 1e-15)
    height = max(maximum_y - minimum_y, 1e-15)

    endpoint_ids = {}
    parent = []
    def endpoint_id(endpoint):
        key = coordinate_key(endpoint)
        if key not in endpoint_ids:
            endpoint_ids[key] = len(parent)
            parent.append(len(parent))
        return endpoint_ids[key]

    def find(value):
        while parent[value] != value:
            parent[value] = parent[parent[value]]
            value = parent[value]
        return value

    def union(left, right):
        left, right = find(left), find(right)
        if left != right:
            parent[right] = left

    duplicate_segments = Counter()
    occupied = Counter()
    for feature in features:
        line = feature["geometry"]["coordinates"]
        line_ids = [endpoint_id(endpoint) for endpoint in line]
        for start_id, end_id in zip(line_ids, line_ids[1:]):
            union(start_id, end_id)
        for start, end in zip(line, line[1:]):
            duplicate_segments[segment_key(start, end)] += 1
            midpoint_x = (start[0] + end[0]) / 2
            midpoint_y = (start[1] + end[1]) / 2
            column = min(GRID_SIZE - 1, max(0, int((midpoint_x - minimum_x) / width * GRID_SIZE)))
            row = min(GRID_SIZE - 1, max(0, int((midpoint_y - minimum_y) / height * GRID_SIZE)))
            occupied[(row, column)] += 1

    component_counts = Counter()
    for feature in features:
        root = find(endpoint_id(feature["geometry"]["coordinates"][0]))
        component_counts[root] += len(feature["geometry"]["coordinates"]) - 1
    component_sizes = sorted(component_counts.values(), reverse=True)
    duplicate_count = sum(count - 1 for count in duplicate_segments.values() if count > 1)

    return {
        "line_strings": len(features),
        "segments": sum(chain_lengths),
        "coordinates": len(coordinates),
        "chain_lengths": {
            "minimum": min(chain_lengths),
            "maximum": max(chain_lengths),
            "mean": mean(chain_lengths),
            "p50": percentile(chain_lengths, 0.50),
            "p95": percentile(chain_lengths, 0.95),
        },
        "connected_components": {
            "definition": "components connected by exact shared line-string vertices",
            "count": len(component_sizes),
            "largest_segments": component_sizes[0],
            "p50_segments": percentile(component_sizes, 0.50),
            "p95_segments": percentile(component_sizes, 0.95),
        },
        "envelope_grid_occupancy": {
            "grid_size": GRID_SIZE,
            "occupied_cells": len(occupied),
            "total_cells": GRID_SIZE * GRID_SIZE,
            "maximum_segments_per_cell": max(occupied.values()),
        },
        "duplicate_incidence": {
            "exact_duplicate_segments": duplicate_count,
            "exact_duplicate_ratio": duplicate_count / max(sum(chain_lengths), 1),
            "collinear_overlap": "not measured; requires an exact line-overlap pass",
        },
    }


def write_feature_collection(path, features):
    payload = {"type": "FeatureCollection", "features": features}
    encoded = (json.dumps(payload, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded)
    return hashlib.sha256(encoded).hexdigest(), len(encoded)


def run_converter(ogr2ogr, source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    command = converter_command(ogr2ogr, source, destination)
    subprocess.run(command, check=True)
    version = subprocess.run([ogr2ogr, "--version"], check=True, capture_output=True, text=True)
    return command, version.stdout.strip()


def converter_command(ogr2ogr, source, destination):
    return [
        ogr2ogr,
        "-f",
        "GeoJSONSeq",
        str(destination),
        str(source),
        "lines",
        "-where",
        "highway IS NOT NULL",
        "-lco",
        "RS=YES",
        "-lco",
        "COORDINATE_PRECISION=17",
    ]


def source_record(manifest, source_id):
    sources = {source["id"]: source for source in manifest["sources"]}
    try:
        return sources[source_id]
    except KeyError as error:
        raise ValueError(f"unknown source {source_id}") from error


def parser():
    root = Path(__file__).resolve().parents[1]
    command = argparse.ArgumentParser(description=__doc__)
    command.add_argument(
        "--source-manifest",
        type=Path,
        default=root / "benchmarks/production-corpus-v1.json",
    )
    command.add_argument("--source-id", default="geofabrik-california-2026-08-01")
    command.add_argument("--source", type=Path, required=True)
    command.add_argument("--output-dir", type=Path, required=True)
    command.add_argument("--ogr2ogr", default="ogr2ogr")
    command.add_argument("--lines", type=Path, help="reuse a converted GeoJSONSeq file")
    command.add_argument("--acquired-on", required=True, help="UTC acquisition date (YYYY-MM-DD)")
    command.add_argument("--target", type=int, action="append")
    command.add_argument("--include-million", action="store_true")
    command.add_argument(
        "--validation-dir",
        type=Path,
        help="attach passed benchmark_record --check-only-output files",
    )
    return command


def main():
    args = parser().parse_args()
    targets = list(args.target or DEFAULT_TARGETS)
    if args.include_million and 1_000_000 not in targets:
        targets.append(1_000_000)
    if any(target < 1 for target in targets) or len(set(targets)) != len(targets):
        raise SystemExit("targets must be positive and unique")
    targets.sort()

    manifest = json.loads(args.source_manifest.read_text())
    source = source_record(manifest, args.source_id)
    artifact = source["artifact"]
    if args.source.stat().st_size != artifact["byte_length"]:
        raise SystemExit(f"source byte length mismatch for {args.source}")
    actual_md5 = digest_file(args.source, "md5")
    if actual_md5 != artifact["checksum"]["value"]:
        raise SystemExit(f"source MD5 mismatch: expected {artifact['checksum']['value']}, got {actual_md5}")

    output_dir = args.output_dir
    lines_path = args.lines or output_dir / "intermediate" / f"{args.source_id}-highways.geojsonl"
    if args.lines is None:
        command, converter_version = run_converter(args.ogr2ogr, args.source, lines_path)
    else:
        command = converter_command(args.ogr2ogr, args.source, lines_path)
        version = subprocess.run(
            [args.ogr2ogr, "--version"], check=True, capture_output=True, text=True
        )
        converter_version = version.stdout.strip()
    if not lines_path.is_file():
        raise SystemExit(f"missing converted linework: {lines_path}")

    selected = []
    target_records = {}
    segment_count = 0
    for feature in iter_features(lines_path):
        selected.append(feature)
        segment_count += len(feature["geometry"]["coordinates"]) - 1
        for target in targets:
            if target not in target_records and segment_count >= target:
                target_records[target] = list(selected)
        if len(target_records) == len(targets):
            break
    if len(target_records) != len(targets):
        raise SystemExit(f"converted source ended at {segment_count} segments before {targets[-1]}")

    source_sha256 = digest_file(args.source, "sha256")
    workload_records = []
    for target in targets:
        features = target_records[target]
        workload_id = f"osm-california-highways-{target // 1000}k-v1" if target < 1_000_000 else "osm-california-highways-1m-v1"
        relative_path = Path("clips") / f"{workload_id}.geojson"
        artifact_path = output_dir / relative_path
        sha256, byte_length = write_feature_collection(artifact_path, features)
        structure = structure_descriptor(features)
        record = {
            "id": workload_id,
            "description": f"Deterministic OSM highway linework tier at or above {target} input segments.",
            "target_segments": target,
            "source_selection": {
                "rule": "ascending osm_id order emitted by the pinned GDAL OSM driver; retain complete ways until the target is reached",
                "selected_feature_count": len(features),
                "first_osm_id": features[0]["properties"]["osm_id"],
                "last_osm_id": features[-1]["properties"]["osm_id"],
                "bbox_wgs84": bbox(features),
            },
            "artifact": {
                "path": relative_path.as_posix(),
                "sha256": sha256,
                "byte_length": byte_length,
            },
            "structure": structure,
            "candidate_split_density": {
                "status": "pending-correctness-gated-run",
                "candidate_pairs_per_input_segment": None,
                "exact_predicates_per_input_segment": None,
                "split_events_per_input_segment": None,
            },
            "contract": {
                "coordinate_reference": "WGS 84 (EPSG:4326)",
                "units": "degrees",
                "compatibility_class": "parity",
                "permitted_profiles": ["floating"],
                "retained_result_families": [
                    "polygons",
                    "dangles",
                    "cut-edges",
                    "invalid-rings",
                    "provenance",
                    "diagnostics",
                ],
                "z_behavior": "source is two-dimensional; Z is absent",
            },
        }
        if args.validation_dir is not None:
            validation_path = args.validation_dir / f"{workload_id}.json"
            if not validation_path.is_file():
                validation_path = args.validation_dir / f"{workload_id}-floating.json"
            if not validation_path.is_file():
                raise SystemExit(f"missing validation output: {validation_path}")
            validation = json.loads(validation_path.read_text())
            if validation.get("status") != "passed":
                raise SystemExit(f"validation did not pass: {validation_path}")
            validation_work = validation["work"]
            input_segments = validation_work["input_segments"]
            record["candidate_split_density"] = {
                "status": "measured-by-benchmark-record-check-only",
                "candidate_pairs_per_input_segment": validation_work["candidate_pairs"] / input_segments,
                "exact_predicates_per_input_segment": validation_work["exact_predicate_calls"] / input_segments,
                "split_events_per_input_segment": validation_work["split_events"] / input_segments,
            }
            record["validation"] = validation
        workload_records.append(record)

    derivation = {
        "generator": "scripts/materialize_production_workloads.py",
        "generator_version": SCHEMA_VERSION,
        "converter": "GDAL ogr2ogr OSM to GeoJSONSeq",
        "converter_version": converter_version,
        "converter_command": command,
        "source_layer": "lines",
        "where": "highway IS NOT NULL",
        "output_options": ["RS=YES", "COORDINATE_PRECISION=17"],
        "selection_order": "ascending osm_id, complete source ways",
    }
    report = {
        "schema_version": SCHEMA_VERSION,
        "source": {
            "id": source["id"],
            "filename": artifact["filename"],
            "download_url": artifact["download_url"],
            "metadata_url": source["metadata_url"],
            "license": source["license"],
            "license_url": source["license_url"],
            "attribution": source["attribution"],
            "acquired_on": args.acquired_on,
            "byte_length": artifact["byte_length"],
            "md5": actual_md5,
            "sha256": source_sha256,
        },
        "derivation": derivation,
        "workloads": workload_records,
        "domain_coverage": {
            "network": {"status": "materialized", "workload_ids": [record["id"] for record in workload_records], "reason": "Road-network tiers were materialized from the pinned OSM source."},
            "coverage": {"status": "procedural-fixture", "workload_ids": ["already-noded-coverage-v1"], "reason": "No public cadastral source was authorized in this run."},
            "hydrographic": {"status": "existing-public-fixture", "workload_ids": ["hydrographic-boundary-v1"], "reason": "Existing Natural Earth boundary remains the legal public fixture; no contour source was acquired."},
            "cad": {"status": "procedural-fixture", "workload_ids": ["dirty-cad-v1"], "reason": "Customer/Civil 3D coordinates are not authorized for this corpus."},
        },
    }
    output_dir.mkdir(parents=True, exist_ok=True)
    report_path = output_dir / "production-workloads-v1.json"
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")

    runner_workloads = []
    for record in workload_records:
        runner_workloads.append(
            {
                "id": record["id"],
                "description": record["description"],
                "domain": "road-network",
                "source_url": source["metadata_url"],
                "license": source["license"],
                "attribution": source["attribution"],
                "artifact": {"clip_path": record["artifact"]["path"], "sha256": record["artifact"]["sha256"]},
                "coordinate_reference": "WGS 84 (EPSG:4326)",
                "units": "degrees",
                "compatibility_class": record["contract"]["compatibility_class"],
                "permitted_profiles": ["floating"],
                "options": [{"node_input": True}],
                "retained_result_families": record["contract"]["retained_result_families"],
                "size": {
                    "line_strings": record["structure"]["line_strings"],
                    "segments": record["structure"]["segments"],
                    "coordinates": record["structure"]["coordinates"],
                    "expected_candidate_class": "dense"
                    if record["target_segments"] >= 100_000
                    else "moderate",
                },
            }
        )
    (output_dir / "runner-manifest-v1.json").write_text(
        json.dumps({"schema_version": 1, "workloads": runner_workloads}, indent=2) + "\n"
    )
    print(f"materialized {len(workload_records)} workloads under {output_dir}")


if __name__ == "__main__":
    main()
