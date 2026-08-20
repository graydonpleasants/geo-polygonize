import json
import subprocess
import sys
from pathlib import Path

from jsonschema import validate


ROOT = Path(__file__).resolve().parents[1]


def test_production_corpus_is_schema_valid_and_source_pinned():
    manifest = ROOT / "benchmarks" / "production-corpus-v1.json"
    schema = ROOT / "benchmarks" / "production-corpus-v1.schema.json"
    corpus = json.loads(manifest.read_text())
    validate(corpus, json.loads(schema.read_text()))

    source_artifact = corpus["sources"][0]["artifact"]
    assert source_artifact["filename"] == "california-260801.osm.pbf"
    assert source_artifact["byte_length"] == 1322245000
    assert source_artifact["checksum"]["value"] == "3be0a7bdf02572622c791b89063638a0"

    sources = {source["id"]: source for source in corpus["sources"]}
    assert len(sources) == len(corpus["sources"])
    workload_ids = {workload["id"] for workload in corpus["workloads"]}
    assert len(workload_ids) == len(corpus["workloads"])
    for workload in corpus["workloads"]:
        source = sources[workload["source_id"]]
        assert workload["minimum_source_bytes"] <= source["artifact"]["byte_length"]
    for source in sources.values():
        artifact = source["artifact"]
        assert artifact["byte_length"] >= 1 << 30
        assert "latest" not in artifact["filename"]
        assert "latest" not in artifact["download_url"]
        assert artifact["download_url"].startswith("https://")
        assert artifact["checksum"]["publisher_url"].startswith("https://")


def test_correctness_references_are_persistable_and_retained():
    help_text = subprocess.run(
        [sys.executable, ROOT / "benchmarks" / "check_geos_references.py", "--help"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    assert "--output-dir" in help_text
    assert "--manifest" in help_text
    assert "--validation-output-dir" in help_text
    assert "--serial-binary" in help_text

    workflow = (ROOT / ".github/workflows/benchmark-evidence.yml").read_text()
    assert "--output-dir target/geos-reference" in workflow
    assert "path: target/geos-reference/*.json" in workflow
    assert "path: target/jts-reference/*.json" in workflow
    assert workflow.count("retention-days: 30") == 2


def test_publication_requires_a_dedicated_runner_and_retains_gated_outputs():
    workflow = (ROOT / ".github/workflows/benchmark-publication.yml").read_text()
    assert "runs-on: [self-hosted, benchmark-dedicated, linux, x64]" in workflow
    assert "manifest_path:" in workflow
    assert "MANIFEST_PATH: ${{ inputs.manifest_path }}" in workflow
    assert 'test -f "$MANIFEST_PATH"' in workflow
    assert 'manifest_args+=(--manifest "$MANIFEST_PATH")' in workflow
    assert 'jts_mount_args+=(-v "$manifest_directory:/external-manifest:ro")' in workflow
    assert 'jts_manifest_args+=(--manifest "/external-manifest/$manifest_name")' in workflow
    assert workflow.count("--repetition") == 1
    assert "for repetition in 1 2 3 4 5" in workflow
    assert "--runner-class dedicated" in workflow
    assert "certified-fixed" in workflow
    assert "geo-polygonize-jts-reference-1.0.0.jar" in workflow
    assert "maven:3.9.11-eclipse-temurin-21@sha256:" in workflow
    assert "python:3.12.11-bookworm@sha256:" in workflow
    assert "Prepare pinned Python environment" in workflow
    assert "actions/setup-python@v5" not in workflow
    assert "--publication target/benchmark-publication/publication.json" in workflow
    assert 'cat target/benchmark-publication/trends.md >> "$GITHUB_STEP_SUMMARY"' in workflow
    assert "retention-days: 90" in workflow
    assert "ubuntu-latest" not in workflow


def test_baseline_suite_validation_is_dedicated_and_fail_closed():
    workflow = (ROOT / ".github/workflows/benchmark-baseline-suite.yml").read_text()
    assert "runs-on: [self-hosted, benchmark-dedicated, linux, x64]" in workflow
    assert "publication_dir:" in workflow
    assert 'test -d "$PUBLICATION_DIR"' in workflow
    assert 'find "$PUBLICATION_DIR" -type f -name publication.json | sort' in workflow
    assert "benchmarks/validate_baseline_suite.py" in workflow
    assert "benchmarks/production-baseline-suite-v1.json" in workflow
    assert "production-baseline-evidence-v1.json" in workflow
    assert "python:3.12.11-bookworm@sha256:" in workflow
    assert "Prepare pinned Python environment" in workflow
    assert "actions/setup-python@v5" not in workflow
    assert "retention-days: 90" in workflow
    assert "ubuntu-latest" not in workflow
