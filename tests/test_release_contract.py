"""Static guards for the release compatibility check."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_release_contract_checks_latest_published_baseline():
    workflow = (ROOT / ".github/workflows/ci.yml").read_text()
    release_contract = workflow.split("  release-contract:\n", 1)[1].split(
        "  supply-chain:\n", 1
    )[0]
    # Keep this pinned to the last completed release, not the pending version.
    assert "baseline-rev: geo-polygonize-v1.2.0" in release_contract
    assert "baseline-rev: geo-polygonize-v1.1.0" not in release_contract
    assert "package: geo-polygonize-core" in release_contract
    assert "release-type: minor" in release_contract
    assert "fetch-depth: 0" in release_contract
