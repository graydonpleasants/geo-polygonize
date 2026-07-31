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
