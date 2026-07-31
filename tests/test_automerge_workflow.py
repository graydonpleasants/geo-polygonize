from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_automerge_excludes_native_stacks():
    workflow = (ROOT / ".github/workflows/automerge.yml").read_text()

    assert "github.event.pull_request.base.ref == github.event.repository.default_branch" in workflow
    assert "github.event.pull_request.stack == null" in workflow
