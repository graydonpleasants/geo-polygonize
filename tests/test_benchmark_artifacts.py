import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_correctness_references_are_persistable_and_retained():
    help_text = subprocess.run(
        [sys.executable, ROOT / "benchmarks" / "check_geos_references.py", "--help"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    assert "--output-dir" in help_text

    workflow = (ROOT / ".github/workflows/benchmark-evidence.yml").read_text()
    assert "--output-dir target/geos-reference" in workflow
    assert "path: target/geos-reference/*.json" in workflow
    assert "path: target/jts-reference/*.json" in workflow
    assert workflow.count("retention-days: 30") == 2


def test_publication_requires_a_dedicated_runner_and_retains_gated_outputs():
    workflow = (ROOT / ".github/workflows/benchmark-publication.yml").read_text()
    assert "runs-on: [self-hosted, benchmark-dedicated, linux, x64]" in workflow
    assert workflow.count("--repetition") == 1
    assert "for repetition in 1 2 3 4 5" in workflow
    assert "--runner-class dedicated" in workflow
    assert "certified-fixed" in workflow
    assert "geo-polygonize-jts-reference-1.0.0.jar" in workflow
    assert "maven:3.9.11-eclipse-temurin-21@sha256:" in workflow
    assert "--publication target/benchmark-publication/publication.json" in workflow
    assert 'cat target/benchmark-publication/trends.md >> "$GITHUB_STEP_SUMMARY"' in workflow
    assert "retention-days: 90" in workflow
    assert "ubuntu-latest" not in workflow
