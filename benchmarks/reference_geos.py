#!/usr/bin/env python3
"""Emit a GEOS/Shapely topology reference for one public workload."""

import argparse
import hashlib
import json
import struct
from pathlib import Path

import shapely
from shapely.geometry import LineString, shape
from shapely.ops import polygonize_full, unary_union


LANES = {
    "already-noded": "already-noded-polygonization",
    "floating": "floating-noding-plus-polygonization",
}


def bits(value):
    if value == 0:
        value = 0.0
    return f"0x{struct.unpack('>Q', struct.pack('>d', value))[0]:016x}"


def coordinate(value):
    return {"x": bits(value[0]), "y": bits(value[1])}


def coordinate_key(value):
    return value["x"], value["y"]


def minimum_rotation(values):
    size = len(values)
    if size < 2:
        return values
    left, right, offset = 0, 1, 0
    while left < size and right < size and offset < size:
        first = coordinate_key(values[(left + offset) % size])
        second = coordinate_key(values[(right + offset) % size])
        if first == second:
            offset += 1
        elif first < second:
            right += offset + 1
            if right == left:
                right += 1
            offset = 0
        else:
            left += offset + 1
            if left == right:
                left += 1
            offset = 0
    start = min(left, right)
    return values[start:] + values[:start]


def canonical_ring(values):
    values = [coordinate(value) for value in values]
    if len(values) > 1 and values[0] == values[-1]:
        values.pop()
    if not values:
        return []
    forward = minimum_rotation(values)
    backward = minimum_rotation(list(reversed(values)))
    result = min(forward, backward, key=canonical_json)
    return result + [result[0]]


def canonical_line(values):
    values = [coordinate(value) for value in values]
    backward = list(reversed(values))
    return min(values, backward, key=canonical_json)


def canonical_json(value):
    return json.dumps(value, separators=(",", ":"), sort_keys=False)


def geometries(collection):
    return list(collection.geoms)


def canonical_topology(lines, lane):
    source = lines if lane == "already-noded" else unary_union(lines)
    polygons, cuts, dangles, invalid = polygonize_full(source)
    polygon_values = []
    for polygon in geometries(polygons):
        interiors = [canonical_ring(ring.coords) for ring in polygon.interiors]
        interiors.sort(key=canonical_json)
        polygon_values.append(
            {
                "exterior": canonical_ring(polygon.exterior.coords),
                "interiors": interiors,
            }
        )
    polygon_values.sort(key=canonical_json)

    def linestrings(collection, ring=False):
        canonicalize = canonical_ring if ring else canonical_line
        result = [canonicalize(line.coords) for line in geometries(collection)]
        result.sort(key=canonical_json)
        return result

    return {
        "polygons": polygon_values,
        "dangles": linestrings(dangles),
        "cut_edges": linestrings(cuts),
        "invalid_rings": linestrings(invalid, ring=True),
    }


def load_workload(root, workload_id, manifest_path=None):
    manifest_path = Path(manifest_path) if manifest_path else (
        root / "crates/geo-polygonize-core/tests/workloads/manifest-v1.json"
    )
    if not manifest_path.is_absolute():
        manifest_path = root / manifest_path
    manifest = json.loads(manifest_path.read_text())
    workload = next(
        (value for value in manifest["workloads"] if value["id"] == workload_id), None
    )
    if workload is None:
        raise ValueError(f"unknown workload {workload_id}")
    clip = manifest_path.parent / workload["artifact"]["clip_path"]
    collection = json.loads(clip.read_text())
    lines = []
    for feature in collection["features"]:
        geometry = shape(feature["geometry"])
        if geometry.geom_type == "LineString":
            source_lines = [geometry]
        elif geometry.geom_type == "MultiLineString":
            source_lines = geometry.geoms
        else:
            raise ValueError("workload geometry must contain line strings")
        for line in source_lines:
            coordinates = list(line.coords)
            lines.extend(
                LineString(coordinates[index : index + 2])
                for index in range(len(coordinates) - 1)
            )
    return workload, lines


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--lane", choices=LANES, required=True)
    parser.add_argument("--workload", required=True)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    workload, lines = load_workload(root, args.workload, args.manifest)
    if workload["compatibility_class"] != "parity":
        raise ValueError(f"{args.workload} is not a parity-class workload")
    if args.lane not in workload["permitted_profiles"]:
        raise ValueError(f"{args.workload} does not permit {args.lane}")

    topology = canonical_topology(lines, args.lane)
    result = {
        "schema_version": 1,
        "workload_id": args.workload,
        "lane": LANES[args.lane],
        "implementation": {
            "name": "shapely",
            "version": shapely.__version__,
            "dependencies": {"geos": shapely.geos_version_string},
        },
        "fingerprint_sha256": hashlib.sha256(
            canonical_json(topology).encode()
        ).hexdigest(),
        "topology": topology,
    }
    output = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(output)
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
