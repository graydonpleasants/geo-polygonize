#!/usr/bin/env python3
"""Local diagnostic benchmark, not a publish_benchmark decision-quality record.

Every sample is validated after its timer stops. Failure aborts before writing
results. Full and lean calls use identical core options and input source IDs.
GEOS timing is included only for the exact-equivalent noding grid lane; it does
not supply native provenance or all native diagnostic metadata.
"""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time

import numpy as np
import shapely
from shapely.geometry import LineString, Polygon, box
from shapely.ops import polygonize as geos_polygonize, polygonize_full, unary_union

from geo_polygonize import cfb_robust_options, polygonize_with_options
from geo_polygonize import geo_polygonize_core as native_extension


def require(condition, detail):
    if not condition:
        raise RuntimeError(f"Correctness gate failed: {detail}")


def stable(value):
    if isinstance(value, np.ndarray):
        return {"dtype": value.dtype.str, "shape": value.shape, "bytes": value.tobytes().hex()}
    if isinstance(value, float):
        return value.hex()
    if hasattr(value, "shell"):
        return stable(vars(value))
    if isinstance(value, dict):
        return {k: stable(v) for k, v in value.items()}
    if isinstance(value, (tuple, list)):
        return [stable(v) for v in value]
    return value


def semantic_digest(report):
    semantic = {k: v for k, v in report.items()
                if k not in ("binding_timings", "binding_profile", "topology_fingerprint")}
    if "diagnostics" in semantic:
        semantic["diagnostics"] = {k: v for k, v in semantic["diagnostics"].items() if k != "phase_times"}
    return hashlib.sha256(json.dumps(stable(semantic), sort_keys=True).encode()).hexdigest()


def geometry_signature(polygons):
    require(all(p.is_valid for p in polygons), "Invalid output geometry")
    return sorted(shapely.normalize(p).wkb_hex for p in polygons)


def decode_packed(report):
    coords = report["flat_coords"].reshape(-1, report["stride"])
    rings = np.append(report["ring_offsets"], len(coords))
    polys = np.append(report["polygon_offsets"], len(rings) - 1)
    return shapely.from_ragged_array(shapely.GeometryType.POLYGON, coords, (rings, polys))


def workload(kind, n):
    if kind == "grid":
        lines = [LineString([(x, 0), (x, n)]) for x in range(n + 1)]
        lines += [LineString([(0, y), (n, y)]) for y in range(n + 1)]
        expected = [box(x, y, x + 1, y + 1) for x in range(n) for y in range(n)]
    elif kind == "noded_grid":
        lines = [LineString([(x, y), (x + 1, y)]) for x in range(n) for y in range(n + 1)]
        lines += [LineString([(x, y), (x, y + 1)]) for x in range(n + 1) for y in range(n)]
        expected = [box(x, y, x + 1, y + 1) for x in range(n) for y in range(n)]
    else:
        lines, expected = [], []
        for x in range(n):
            for y in range(n):
                shell = box(5*x, 5*y, 5*x+4, 5*y+4)
                inner = box(5*x+1, 5*y+1, 5*x+3, 5*y+3)
                lines += [LineString(shell.exterior.coords), LineString(inner.exterior.coords)]
                expected += [Polygon(shell.exterior.coords, [inner.exterior.coords]), inner]
    return np.asarray(lines, dtype=object), expected


def call(lines, ids, options, route, mode):
    started = time.perf_counter()
    if route == "packed":
        _, coords, offsets = shapely.to_ragged_array(lines)
        kwargs = dict(coords=coords, offsets=offsets[0][:-1].astype(np.uint32, copy=False))
    else:
        kwargs = dict(lines=lines)
    input_done = time.perf_counter()
    report = polygonize_with_options(**kwargs, line_ids=ids, options=options,
                                     output=mode, profile_binding=True)
    native_done = time.perf_counter()
    if route == "packed":
        polygons = decode_packed(report)
    else:
        polygons = [Polygon(p.shell, p.holes) for p in report["polygons"]]
    ended = time.perf_counter()
    return report, polygons, {
        "input_pack_ms": (input_done - started)*1000,
        "wrapper_call_ms": (native_done - input_done)*1000,
        "shapely_output_ms": (ended - native_done)*1000,
        "pipeline_ms": (ended - started)*1000,
        **report["binding_profile"],
    }


def diagnostic_signature(report):
    return {key: sorted(shapely.normalize(LineString(line)).wkb_hex for line in report[key])
            for key in ("dangles", "cut_edges", "invalid_rings")}


def geos_call(lines):
    started = time.perf_counter()
    noded = unary_union(lines)
    noded_done = time.perf_counter()
    polygons, cuts, dangles, invalid = polygonize_full(noded)
    polygonized = time.perf_counter()
    polys = list(polygons.geoms)
    diagnostics = {"dangles": list(dangles.geoms), "cut_edges": list(cuts.geoms), "invalid_rings": list(invalid.geoms)}
    ended = time.perf_counter()
    signature = {key: sorted(shapely.normalize(line).wkb_hex for line in values) for key, values in diagnostics.items()}
    return polys, signature, {"noding_ms": (noded_done-started)*1000,
                              "polygonize_ms": (polygonized-noded_done)*1000,
                              "geometry_wrapper_ms": (ended-polygonized)*1000,
                              "pipeline_ms": (ended-started)*1000}


def cpu_affinity():
    try:
        return sorted(os.sched_getaffinity(0))
    except (AttributeError, OSError):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sizes", type=int, nargs="+", default=[16, 48])
    parser.add_argument("--kinds", nargs="+", choices=["grid", "noded_grid", "holes"], default=["grid", "noded_grid", "holes"])
    parser.add_argument("--routes", nargs="+", choices=["packed", "wrapper"], default=["packed", "wrapper"])
    parser.add_argument("--samples", type=int, default=9)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.samples < 1 or args.warmups < 0 or any(n < 1 for n in args.sizes):
        parser.error("positive sizes/samples and nonnegative warmups required")
    output = {
        "classification": "local-diagnostic-only; no production or decision-quality speed claim",
        "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "git_diff_sha256": hashlib.sha256(subprocess.check_output(["git", "diff", "HEAD"])).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "native_extension_path": native_extension.__file__,
        "native_extension_sha256": hashlib.sha256(Path(native_extension.__file__).read_bytes()).hexdigest(),
        "python": platform.python_version(), "platform": platform.platform(),
        "cpu_count": os.cpu_count(), "cpu_affinity": cpu_affinity(), "rayon_num_threads": os.getenv("RAYON_NUM_THREADS"),
        "versions": {p: importlib.metadata.version(p) for p in ["numpy", "shapely", "geo-polygonize-py"]},
        "geos_version": shapely.geos_version_string,
        "samples": args.samples, "warmups": args.warmups, "cases": [],
        "scope": "Input packing, native binding, Shapely reconstruction. No repair/filter/reconciliation/grading/I/O. Input fixture generation, oracle and validation excluded from timers.",
    }
    for kind in args.kinds:
        for n in args.sizes:
            lines, expected = workload(kind, n)
            ids = np.arange(1, len(lines) + 1, dtype=np.uint32)
            options = cfb_robust_options()
            if kind in ("noded_grid", "holes"):
                # Explicit already-noded lane; not comparable to noding+polygonize.
                options.update(node_input=False, pre_snap_tolerance=0., precision_model={"type": "floating"})
                options["input_profile_id"] = "lean_benchmark_already_noded"
            oracle = geometry_signature(expected)
            require(geometry_signature(list(geos_polygonize(unary_union(lines)))) == oracle, "GEOS versus analytical oracle")
            baseline, baseline_polys, _ = call(lines, ids, options, "packed", "report")
            require(geometry_signature(baseline_polys) == oracle, "Native versus analytical oracle")
            expected_semantics = semantic_digest(baseline)
            fingerprint = baseline["topology_fingerprint"]
            case = {"kind": kind, "size": n, "input_lines": len(lines), "polygon_count": len(oracle),
                    "options": options, "semantic_sha256": expected_semantics,
                    "geometry_sha256": hashlib.sha256(json.dumps(oracle).encode()).hexdigest(), "routes": {}}
            expected_diagnostics = diagnostic_signature(baseline)
            del baseline, baseline_polys
            for route in args.routes:
                rows = {"report": [], "lean_report": []}
                for iteration in range(args.warmups + args.samples):
                    # Alternate order so one mode does not always receive the warmer allocator.
                    order = ("report", "lean_report") if iteration % 2 == 0 else ("lean_report", "report")
                    for mode in order:
                        report, polygons, row = call(lines, ids, options, route, mode)
                        require(semantic_digest(report) == expected_semantics, (kind, n, route, mode, "report mismatch"))
                        require(geometry_signature(polygons) == oracle, (kind, n, route, mode, "geometry mismatch"))
                        if mode == "report":
                            require(report["topology_fingerprint"] == fingerprint, "Fingerprint changed")
                        else:
                            require("topology_fingerprint" not in report, "Lean constructed fingerprint")
                            require(row["fingerprint_build_ms"] == row["fingerprint_json_ms"] == 0., "Lean performed fingerprint work")
                        if iteration >= args.warmups:
                            rows[mode].append(row)
                        del report, polygons
                summaries = {mode: {field: statistics.median(r[field] for r in records) for field in records[0]}
                             for mode, records in rows.items()}
                ratio = summaries["report"]["pipeline_ms"] / summaries["lean_report"]["pipeline_ms"]
                case["routes"][route] = {"median_ms": summaries, "full_over_lean_pipeline_ratio": ratio, "raw_samples_ms": rows}
                print(f"{kind} {n} {route}: report={summaries['report']['pipeline_ms']:.3f}ms lean={summaries['lean_report']['pipeline_ms']:.3f}ms ratio={ratio:.3f}", flush=True)
            if kind == "grid":
                # This lane includes noding on both sides. Native additionally does
                # fixed-grid/pre-snap policy work, provenance and richer diagnostics.
                # Gate only genuinely shared outputs, never pretend GEOS supplies IDs.
                geos_rows = []
                for iteration in range(args.warmups + args.samples):
                    polys, diagnostics, row = geos_call(lines)
                    require(geometry_signature(polys) == oracle, "Timed GEOS geometry mismatch")
                    require(diagnostics == expected_diagnostics, "Timed GEOS diagnostic geometry mismatch")
                    if iteration >= args.warmups:
                        geos_rows.append(row)
                case["geos_noding_pipeline"] = {
                    "scope": "unary_union + polygonize_full + Python wrappers for all geometry families; no source provenance or native work counters",
                    "median_ms": {field: statistics.median(r[field] for r in geos_rows) for field in geos_rows[0]},
                    "raw_samples_ms": geos_rows,
                }
                print(f"grid {n} GEOS noding pipeline: {case['geos_noding_pipeline']['median_ms']['pipeline_ms']:.3f}ms", flush=True)
            output["cases"].append(case)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
