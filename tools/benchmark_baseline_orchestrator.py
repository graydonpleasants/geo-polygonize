#!/usr/bin/env python3
"""Dispatch, resume, validate, and report a seven-publication baseline."""

import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path


WORKLOADS = (
    ("osm-california-highways-1k-v1", "production-network-1k"),
    ("osm-california-highways-10k-v1", "production-network-10k"),
    ("osm-california-highways-100k-v1", "production-network-100k"),
)
ARTIFACT_PREFIX = "benchmark-publication-"


def gh(*args, check=True):
    result = subprocess.run(
        ["gh", *args],
        check=False,
        capture_output=True,
        text=True,
        env=os.environ.copy(),
    )
    if check and result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    return result.stdout.strip()


def save_state(path, state):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")


def load_state(path):
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else {"runs": {}}


def run_view(repo, run_id):
    return json.loads(gh("run", "view", str(run_id), "--repo", repo, "--json", "status,conclusion,headSha,jobs,url"))


def failure_detail(repo, run):
    for job in run.get("jobs", []):
        failed = [step for step in job.get("steps", []) if step.get("conclusion") in {"failure", "cancelled"}]
        if failed:
            log = gh("run", "view", "--job", str(job["databaseId"]), "--repo", repo, "--log-failed", check=False)
            return {"job": job["name"], "step": failed[0]["name"], "log": log[-8000:]}
    return {"job": None, "step": None, "log": "workflow ended without a failed step"}


def wait_for_run(repo, run_id, expected_sha, poll_seconds):
    while True:
        run = run_view(repo, run_id)
        if run["status"] == "completed":
            if run.get("headSha") != expected_sha:
                raise RuntimeError(f"run {run_id} used {run.get('headSha')}, expected {expected_sha}")
            if run.get("conclusion") != "success":
                raise RuntimeError(json.dumps({"run": run, "failure": failure_detail(repo, run)}, indent=2))
            return run
        time.sleep(poll_seconds)


def dispatch_publication(repo, ref, workload, lane, manifest_path):
    output = gh(
        "workflow", "run", "benchmark-publication.yml", "--repo", repo, "--ref", ref,
        "-f", f"workload={workload}", "-f", f"lane={lane}", "-f", f"manifest_path={manifest_path}",
    )
    urls = [line.strip() for line in output.splitlines() if "/actions/runs/" in line]
    if not urls:
        raise RuntimeError(f"workflow dispatch returned no run URL: {output}")
    url = urls[-1].rstrip("/")
    return {"id": url.rsplit("/", 1)[-1], "url": url}


def artifact_name(workload, lane, sha):
    return f"{ARTIFACT_PREFIX}{workload}-{lane}-{sha}"


def validate_publication(path):
    publication = json.loads(path.read_text(encoding="utf-8"))
    if publication.get("runner_class") != "dedicated":
        raise ValueError(f"{path}: runner_class is not dedicated")
    if publication.get("warmup_iterations", 0) < 5 or publication.get("process_repetitions", 0) < 5:
        raise ValueError(f"{path}: insufficient warmups or process repetitions")
    records = publication.get("records", [])
    if len(records) != 5:
        raise ValueError(f"{path}: expected five records, got {len(records)}")
    for record in records:
        measurement = record["measurement"]
        if measurement.get("samples", 0) < 30:
            raise ValueError(f"{path}: record has fewer than 30 samples")
        if record["correctness_gate"]["status"] != "passed":
            raise ValueError(f"{path}: correctness gate failed")
    return publication


def median(records, getter):
    return statistics.median(getter(record) for record in records)


def summarize(path):
    publication = validate_publication(path)
    records = publication["records"]
    baseline = records[0]
    component = baseline["work"]["component_memory"]
    input_segments = baseline["work"]["input_segments"]
    return {
        "publication_id": publication.get("publication_id"),
        "workload_id": baseline["workload_id"],
        "lane": baseline["lane"],
        "input_segments": input_segments,
        "median_p50_ms": median(records, lambda r: r["measurement"]["p50_ms"]),
        "median_p95_ms": median(records, lambda r: r["measurement"]["p95_ms"]),
        "median_allocated_bytes": median(records, lambda r: r["measurement"]["allocations"]["bytes"]),
        "median_peak_rss_bytes": median(records, lambda r: r["measurement"]["peak_rss_bytes"]),
        "component_count": component["component_count"],
        "largest_component_fraction": component["largest_component_edge_count"] / input_segments if input_segments else 0,
        "partition_capacities": {
            "node": component["partition_node_capacity"],
            "edge": component["partition_edge_capacity"],
        },
        "scratch_instances": component["scratch_instance_count"],
        "worker_count": component["execution_worker_count"],
        "scratch_capacities": {
            key: value for key, value in component.items() if key.startswith("max_scratch_")
        },
        "component_memory": component,
        "environment": baseline["environment"],
    }


def write_component_evidence(summaries, output):
    evidence = {
        "schema_version": 1,
        "evidence_type": "component-memory",
        "measurement_class": "decision-quality",
        "runner_class": "dedicated",
        "publication_count": len(summaries),
        "environment": summaries[0]["environment"],
        "publications": [
            {
                "publication_id": summary["publication_id"],
                "workload_id": summary["workload_id"],
                "lane": summary["lane"],
                "input_segments": summary["input_segments"],
                "largest_component_fraction": summary["largest_component_fraction"],
                "component_memory": summary["component_memory"],
            }
            for summary in summaries
        ],
    }
    output.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY"), required=False)
    parser.add_argument("--benchmark-ref", required=True)
    parser.add_argument("--benchmark-sha", required=True)
    parser.add_argument("--baseline-ref", required=True)
    parser.add_argument("--manifest-path", required=True)
    parser.add_argument("--existing-runs", required=True, help="JSON object of artifact key to successful run ID")
    parser.add_argument("--california-runs", default="{}", help="JSON object of California workload:lane to successful run ID")
    parser.add_argument("--poll-seconds", type=int, default=30)
    parser.add_argument("--root", type=Path, default=Path("target/benchmark-orchestrator"))
    parser.add_argument("--resume-state", type=Path)
    args = parser.parse_args()
    if not args.repo:
        parser.error("--repo or GITHUB_REPOSITORY is required")

    root = args.root.resolve()
    publications = root / "publications"
    state_path = root / "orchestrator-state.json"
    state = load_state(args.resume_state) if args.resume_state and args.resume_state.exists() else load_state(state_path)
    state.update({"repo": args.repo, "benchmark_ref": args.benchmark_ref, "benchmark_sha": args.benchmark_sha})
    state.setdefault("runs", {})
    save_state(state_path, state)

    for key, run_id in json.loads(args.existing_runs).items():
        entry = state["runs"].setdefault(key, {"id": str(run_id)})
        run = wait_for_run(args.repo, entry["id"], args.benchmark_sha, args.poll_seconds)
        entry.update({"id": str(run["databaseId"]) if "databaseId" in run else str(entry["id"]), "url": run["url"], "status": "success"})
        save_state(state_path, state)

    expected_california = {f"{workload}:floating" for workload, _ in WORKLOADS}
    california_runs = json.loads(args.california_runs)
    unknown_california = set(california_runs) - expected_california
    if unknown_california:
        raise RuntimeError(f"unknown California run keys: {sorted(unknown_california)}")
    for key, run_id in california_runs.items():
        entry = state["runs"].setdefault(key, {})
        entry.update({"id": str(run_id), "status": "pending"})
        save_state(state_path, state)
        run = wait_for_run(args.repo, entry["id"], args.benchmark_sha, args.poll_seconds)
        entry.update({"id": str(run["databaseId"]) if "databaseId" in run else str(entry["id"]), "url": run["url"], "status": "success"})
        save_state(state_path, state)

    for workload, tier in WORKLOADS:
        key = f"{workload}:floating"
        entry = state["runs"].get(key)
        if not entry or entry.get("status") != "success":
            entry = dispatch_publication(args.repo, args.benchmark_ref, workload, "floating", args.manifest_path)
            state["runs"][key] = entry
            save_state(state_path, state)
            run = wait_for_run(args.repo, entry["id"], args.benchmark_sha, args.poll_seconds)
            entry.update({"url": run["url"], "status": "success"})
            save_state(state_path, state)

        destination = publications / tier
        destination.mkdir(parents=True, exist_ok=True)
        name = artifact_name(workload, "floating", args.benchmark_sha)
        if not (destination / "publication.json").exists():
            gh("run", "download", entry["id"], "--repo", args.repo, "-n", name, "-D", str(destination))

    existing = json.loads(args.existing_runs)
    existing_artifacts = {
        "coverage": "benchmark-publication-already-noded-coverage-v1-already-noded-" + args.benchmark_sha,
        "nested": "benchmark-publication-disconnected-nested-rings-v1-already-noded-" + args.benchmark_sha,
        "floating": "benchmark-publication-dense-crossings-v1-floating-" + args.benchmark_sha,
        "certified-fixed": "benchmark-publication-dense-crossings-v1-certified-fixed-" + args.benchmark_sha,
    }
    for key, run_id in existing.items():
        destination = publications / key
        destination.mkdir(parents=True, exist_ok=True)
        if not (destination / "publication.json").exists():
            gh("run", "download", str(run_id), "--repo", args.repo, "-n", existing_artifacts[key], "-D", str(destination))

    publication_paths = sorted(publications.rglob("publication.json"))
    if len(publication_paths) != 7:
        raise RuntimeError(f"expected seven publication.json files, got {len(publication_paths)}")

    summaries = [summarize(path) for path in publication_paths]
    baseline = root / "production-baseline-evidence-v1.json"
    subprocess.run(
        [sys.executable, "benchmarks/validate_baseline_suite.py", "--suite", "benchmarks/production-baseline-suite-v1.json",
         *sum((["--publication", str(path)] for path in publication_paths), []), "--output", str(baseline)],
        check=True,
    )
    write_component_evidence(summaries, root / "component-memory-evidence-v1.json")
    report = {
        "schema_version": 1,
        "benchmark_sha": args.benchmark_sha,
        "benchmark_ref": args.benchmark_ref,
        "baseline_ref": args.baseline_ref,
        "publication_count": len(publication_paths),
        "runs": state["runs"],
        "publications": summaries,
        "evidence": str(baseline),
    }
    report_path = root / "benchmark-baseline-report-v1.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        Path(summary_path).write_text("# Benchmark baseline orchestrator\n\n" + json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"ERROR: {error}", file=sys.stderr)
        raise
