from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_required_ci_runs_for_stacked_child_bases():
    workflow = (ROOT / ".github/workflows/ci.yml").read_text()
    pull_request_trigger = workflow.split("  pull_request:", 1)[1].split("\nenv:", 1)[0]

    assert "branches:" not in pull_request_trigger
    assert "name: CI Required" in workflow


def test_playground_changes_run_and_build_in_js_ci():
    workflow = (ROOT / ".github/workflows/ci.yml").read_text()

    assert "pkg-wrapper/|playground/|scripts/build_wasm" in workflow
    assert "npm run build --prefix playground" in workflow
