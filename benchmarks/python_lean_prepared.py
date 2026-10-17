#!/usr/bin/env python3
"""Hash-pinned, already-noded prepared XY inputs; private captures stay external.

Input manifest: list of {name, file, sha256, line_count, coordinate_count}.
Each .json.gz input contains wkbs (hex LineStrings) and line_ids. Source IDs here
belong to the prepared inputs; they are not assumed to be original CAD handles.
These timings exclude original snapping/noding/preparation and all grading.
"""
import argparse
import gzip
import hashlib
import importlib.metadata
import json
from pathlib import Path
import platform
import statistics
import subprocess
import time

import numpy as np
import shapely
from shapely.ops import polygonize_full

import python_lean_report as bench


def geos_prepared(lines):
    started = time.perf_counter()
    polygons, cuts, dangles, invalid = polygonize_full(lines)
    kernel_done = time.perf_counter()
    polys = list(polygons.geoms)
    diagnostics = {"dangles": list(dangles.geoms), "cut_edges": list(cuts.geoms), "invalid_rings": list(invalid.geoms)}
    ended = time.perf_counter()
    signature = {key: sorted(shapely.normalize(line).wkb_hex for line in values) for key, values in diagnostics.items()}
    return polys, signature, {"polygonize_ms": (kernel_done-started)*1000,
                              "geometry_wrapper_ms": (ended-kernel_done)*1000,
                              "pipeline_ms": (ended-started)*1000}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--input-root", type=Path, required=True)
    parser.add_argument("--names", nargs="*")
    parser.add_argument("--routes", nargs="+", choices=["packed", "wrapper"], default=["packed", "wrapper"])
    parser.add_argument("--samples", type=int, default=9)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.samples < 1 or args.warmups < 0:
        parser.error("positive samples and nonnegative warmups required")
    manifest = json.loads(args.manifest.read_text())
    names = [e["name"] for e in manifest]
    bench.require(len(names) == len(set(names)), "Duplicate manifest names")
    entries = [e for e in manifest if args.names is None or e["name"] in args.names]
    bench.require(entries and (args.names is None or {e["name"] for e in entries} == set(args.names)), "Requested inputs missing")
    output = {
        "classification": "local diagnostic only; already-noded prepared-input extraction/binding pipeline",
        "scope": "Shapely LineString inputs -> packing -> polygonization -> Shapely output; original snapping/noding/preparation, validation, I/O and grading excluded",
        "native_extra_product": "representative IDs, complete prepared-source provenance, native diagnostics; GEOS does not produce those fields",
        "source_ids": "provided prepared-input IDs, not independently mapped to original drawing entities",
        "manifest_sha256": hashlib.sha256(args.manifest.read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "shared_harness_sha256": hashlib.sha256(Path(bench.__file__).read_bytes()).hexdigest(),
        "native_extension_path": bench.native_extension.__file__,
        "native_extension_sha256": hashlib.sha256(Path(bench.native_extension.__file__).read_bytes()).hexdigest(),
        "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "git_diff_sha256": hashlib.sha256(subprocess.check_output(["git", "diff", "HEAD"])).hexdigest(),
        "rayon_num_threads": bench.os.getenv("RAYON_NUM_THREADS"),
        "python": platform.python_version(), "platform": platform.platform(), "cpu_affinity": bench.cpu_affinity(),
        "versions": {p: importlib.metadata.version(p) for p in ["numpy", "shapely", "geo-polygonize-py"]},
        "geos_version": shapely.geos_version_string, "samples": args.samples, "warmups": args.warmups, "cases": [],
    }
    for entry in entries:
        path = args.input_root / Path(entry["file"]).name
        raw = path.read_bytes()
        bench.require(hashlib.sha256(raw).hexdigest() == entry["sha256"], (entry["name"], "Input checksum mismatch"))
        data = json.loads(gzip.decompress(raw))
        lines = shapely.from_wkb(data["wkbs"])
        bench.require(len(lines) == entry["line_count"], "Input line count changed")
        bench.require(np.all(shapely.get_type_id(lines) == 1), "Prepared inputs must be LineStrings")
        bench.require(int(np.sum(shapely.get_num_coordinates(lines))) == entry["coordinate_count"], "Input coordinate count changed")
        bench.require(np.all(shapely.get_coordinate_dimension(lines) == 2), "XY-only prepared lane")
        bench.require(len(data["line_ids"]) == len(lines), "Source-ID count mismatch")
        bench.require(all(isinstance(i, int) and not isinstance(i, bool) and 0 <= i <= np.iinfo(np.uint32).max for i in data["line_ids"]), "Invalid source ID")
        ids = np.asarray(data["line_ids"], dtype=np.uint32)
        options = bench.cfb_robust_options()
        options.update(node_input=False, pre_snap_tolerance=0., precision_model={"type": "floating"},
                       input_profile_id="lean_benchmark_already_noded")
        baseline, polys, _ = bench.call(lines, ids, options, "packed", "report")
        oracle = bench.geometry_signature(polys)
        diagnostics = bench.diagnostic_signature(baseline)
        expected_semantics = bench.semantic_digest(baseline)
        fingerprint = baseline["topology_fingerprint"]
        geos_polys, geos_diagnostics, _ = geos_prepared(lines)
        bench.require(bench.geometry_signature(geos_polys) == oracle, (entry["name"], "GEOS geometry mismatch"))
        bench.require(geos_diagnostics == diagnostics, (entry["name"], "GEOS diagnostic geometry mismatch"))
        case = {"name": entry["name"], "input_sha256": entry["sha256"], "input_lines": len(lines),
                "source_contract": data.get("contract"), "options": options, "polygon_count": len(polys),
                "hole_count": sum(len(p.interiors) for p in polys),
                "diagnostic_counts": {k: len(v) for k, v in diagnostics.items()},
                "semantic_sha256": expected_semantics, "geometry_sha256": hashlib.sha256(json.dumps(oracle).encode()).hexdigest(),
                "routes": {}}
        del baseline, polys, geos_polys
        for route in args.routes:
            rows = {"report": [], "lean_report": [], "geos": []}
            modes = list(rows)
            for iteration in range(args.samples + args.warmups):
                order = modes[iteration % 3:] + modes[:iteration % 3]
                for mode in order:
                    if mode == "geos":
                        polys, diag, row = geos_prepared(lines)
                        bench.require(diag == diagnostics, "Timed GEOS diagnostics mismatch")
                    else:
                        report, polys, row = bench.call(lines, ids, options, route, mode)
                        bench.require(bench.semantic_digest(report) == expected_semantics, "Timed native report mismatch")
                        if mode == "report":
                            bench.require(report["topology_fingerprint"] == fingerprint, "Fingerprint changed")
                        else:
                            bench.require("topology_fingerprint" not in report, "Lean constructed fingerprint")
                            bench.require(row["fingerprint_build_ms"] == row["fingerprint_json_ms"] == 0., "Lean performed fingerprint work")
                        del report
                    bench.require(bench.geometry_signature(polys) == oracle, "Timed geometry mismatch")
                    if iteration >= args.warmups:
                        rows[mode].append(row)
                    del polys
            summaries = {mode: {k: statistics.median(r[k] for r in values) for k in values[0]} for mode, values in rows.items()}
            case["routes"][route] = {"median_ms": summaries, "raw_samples_ms": rows,
                                      "full_over_lean_pipeline_ratio": summaries["report"]["pipeline_ms"] / summaries["lean_report"]["pipeline_ms"]}
            print(f"{entry['name']} {route}: report={summaries['report']['pipeline_ms']:.3f}ms lean={summaries['lean_report']['pipeline_ms']:.3f}ms GEOS={summaries['geos']['pipeline_ms']:.3f}ms", flush=True)
        output["cases"].append(case)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
