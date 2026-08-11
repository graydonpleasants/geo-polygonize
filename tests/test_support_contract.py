import json
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_REPOSITORY = "https://github.com/graydonpleasants/geo-polygonize"
EXPECTED_AUTHOR = "Graydon Pleasants <graydonpleasants@gmail.com>"
EXPECTED_LICENSE = "MIT OR Apache-2.0"


def test_published_metadata_declares_the_same_support_contract():
    for path in sorted((ROOT / "crates").glob("geo-polygonize-*/Cargo.toml")):
        package = tomllib.loads(path.read_text())["package"]
        assert package["rust-version"] == "1.87"
        assert package["license"] == EXPECTED_LICENSE
        assert package["authors"] == [EXPECTED_AUTHOR]
        assert package["repository"] == EXPECTED_REPOSITORY
        assert package["homepage"] == EXPECTED_REPOSITORY

    package = json.loads((ROOT / "package.json").read_text())
    assert package["author"] == EXPECTED_AUTHOR
    assert package["license"] == EXPECTED_LICENSE
    assert package["bugs"]["url"] == f"{EXPECTED_REPOSITORY}/issues"

    project = tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]
    assert project["authors"] == [
        {"name": "Graydon Pleasants", "email": "graydonpleasants@gmail.com"}
    ]
    assert project["license"]["text"] == EXPECTED_LICENSE
    assert not any("PyPy" in classifier for classifier in project["classifiers"])
    assert project["urls"]["Repository"] == EXPECTED_REPOSITORY


def test_ci_has_an_exact_msrv_job_in_the_required_aggregate():
    ci = (ROOT / ".github/workflows/ci.yml").read_text()
    assert "name: MSRV (Rust 1.87.0)" in ci
    assert "uses: dtolnay/rust-toolchain@1.87.0" in ci
    assert "needs: [changes, rust, msrv," in ci
    assert '[ "${{ needs.msrv.result }}" = "success" ]' in ci
