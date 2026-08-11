import importlib.util
import json
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "verify_release", ROOT / "scripts/verify_release.py"
)
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


def test_release_report_is_complete_and_versioned():
    report = json.loads(
        (ROOT / "release/reports/geo-polygonize-v1.0.0.json").read_text()
    )
    assert report["schema_version"] == 1
    assert report["complete"] is True
    assert VERIFY.report_complete(report)

    report["smoke"]["npm"]["status"] = "failed"
    assert not VERIFY.report_complete(report)


@pytest.mark.parametrize(
    ("tag", "version"),
    [
        ("geo-polygonize-v1.0.0", "1.0.0"),
        ("v2.3.4", "2.3.4"),
        ("geo-polygonize-v1.2.3-rc.1", "1.2.3-rc.1"),
    ],
)
def test_release_tag_parsing(tag, version):
    assert VERIFY.version_from_tag(tag) == version


def test_release_tag_parsing_rejects_non_release_tag():
    with pytest.raises(VERIFY.VerificationError):
        VERIFY.version_from_tag("main")


def test_registry_records_capture_public_artifact_metadata(monkeypatch):
    payloads = {
        "crates.io": {
            "version": {
                "num": "1.0.0",
                "yanked": False,
                "created_at": "2026-08-03T20:36:46Z",
                "dl_path": "/api/v1/crates/example/1.0.0/download",
                "checksum": "a" * 64,
                "crate_size": 12,
            }
        },
        "npm": {
            "time": {"1.0.0": "2026-08-03T20:45:56Z"},
            "versions": {
                "1.0.0": {
                    "dist": {
                        "tarball": "https://registry.npmjs.org/example.tgz",
                        "shasum": "b" * 40,
                    }
                }
            },
        },
        "pypi": {
            "urls": [
                {
                    "filename": "example-1.0.0-py3-none-any.whl",
                    "packagetype": "bdist_wheel",
                    "python_version": "py3",
                    "upload_time_iso": "2026-08-03T20:39:23Z",
                    "digests": {"sha256": "c" * 64},
                }
            ]
        },
    }

    def fake_request(url, attempts, delay):
        if "crates.io" in url:
            return payloads["crates.io"]
        if "npmjs.org" in url:
            return payloads["npm"]
        return payloads["pypi"]

    monkeypatch.setattr(VERIFY, "request_json_with_retries", fake_request)
    assert VERIFY.registry_record("crates.io", "example", "1.0.0", 1, 0)["status"] == "available"
    assert VERIFY.registry_record("npm", "example", "1.0.0", 1, 0)["artifact"]["shasum"] == "b" * 40
    assert VERIFY.registry_record("pypi", "example", "1.0.0", 1, 0)["artifact"]["files"][0]["filename"].endswith(".whl")


def test_report_requires_all_publish_workflows():
    report = json.loads(
        (ROOT / "release/reports/geo-polygonize-v1.0.0.json").read_text()
    )
    report["workflows"] = report["workflows"][:2]
    assert not VERIFY.report_complete(report)
