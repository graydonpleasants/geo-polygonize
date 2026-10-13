import importlib.util
import json
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import pytest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "verify_release", ROOT / "scripts/verify_release.py"
)
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


def canonical_report():
    return json.loads((ROOT / "release/reports/geo-polygonize-v1.0.0.json").read_text())


def publication_run(workflow, run_id=1, **changes):
    return {
        "name": workflow,
        "id": run_id,
        "head_branch": "geo-polygonize-v1.0.0",
        "head_sha": canonical_report()["workflows"][0]["head_sha"],
        "event": "push",
        "status": "completed",
        "conclusion": "success",
        "created_at": "2026-08-03T20:36:15Z",
        **changes,
    }


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


@pytest.mark.parametrize("asset_name", ["publication-report.json", "release-publication-report.json"])
def test_previous_report_accepts_workflow_asset_name(monkeypatch, tmp_path, asset_name):
    report = canonical_report()
    tag = report["tag"]
    monkeypatch.setattr(VERIFY, "latest_release_tag", lambda repository, token: tag)
    monkeypatch.setattr(VERIFY, "github_tag_sha", lambda *args: report["workflows"][0]["head_sha"])
    monkeypatch.setattr(
        VERIFY,
        "github_release",
        lambda repository, token, release_tag: {
            "assets": [{"name": asset_name, "browser_download_url": "report"}]
        },
    )
    monkeypatch.setattr(
        VERIFY,
        "request_json",
        lambda url, token: report,
    )

    VERIFY.check_previous_report(VERIFY.REPOSITORY, "token", tmp_path)


@pytest.mark.parametrize("version", ["1.0.0", "1.1.0"])
def test_historical_report_layout_does_not_require_new_head_sha(version):
    report = canonical_report()
    report["release_version"] = version
    report["tag"] = VERIFY.tag_for_version(version)
    for record in report["registries"] + list(report["smoke"].values()):
        record["version"] = version
    assert "head_sha" not in report
    assert VERIFY.report_complete(report, report["workflows"][0]["head_sha"])


@pytest.mark.parametrize(
    ("section", "field", "wrong"),
    [
        ("registries", "registry", "wrong"),
        ("registries", "package", "wrong"),
        ("registries", "version", "1.2.0"),
        ("registries", "status", "missing"),
        ("workflows", "workflow", "wrong"),
        ("workflows", "status", "in_progress"),
        ("workflows", "conclusion", "failure"),
        ("workflows", "head_sha", "a" * 40),
    ],
)
def test_report_rejects_wrong_evidence(section, field, wrong):
    report = canonical_report()
    report[section][0][field] = wrong
    assert not VERIFY.report_complete(report)


@pytest.mark.parametrize("section", ["registries", "workflows"])
def test_report_rejects_duplicate_identity_at_correct_count(section):
    report = canonical_report()
    report[section][0] = dict(report[section][1])
    assert not VERIFY.report_complete(report)


@pytest.mark.parametrize("field", ["status", "package", "version"])
def test_report_rejects_wrong_smoke_identity(field):
    report = canonical_report()
    report["smoke"]["npm"][field] = "wrong"
    assert not VERIFY.report_complete(report)


@pytest.mark.parametrize("smoke", [{}, {"npm": {"status": "passed"}}, None, {"rust": None}])
def test_report_requires_all_smoke_results(smoke):
    report = canonical_report()
    report["smoke"] = smoke
    assert not VERIFY.report_complete(report)


@pytest.mark.parametrize("tag", ["main", "geo-polygonize-v1.1.0", None])
def test_report_rejects_wrong_release_tag(tag):
    report = canonical_report()
    report["tag"] = tag
    assert not VERIFY.report_complete(report)


def test_report_requires_resolved_sha_when_supplied():
    report = canonical_report()
    sha = report["workflows"][0]["head_sha"]
    report["head_sha"] = sha
    assert VERIFY.report_complete(report)
    assert not VERIFY.report_complete(report, "a" * 40)
    report["head_sha"] = "a" * 40
    assert not VERIFY.report_complete(report)
    report["head_sha"] = None
    assert not VERIFY.report_complete(report)


@pytest.mark.parametrize("section", ["registries", "workflows"])
@pytest.mark.parametrize("malformed", [None, {}, [None]])
def test_report_rejects_malformed_evidence(section, malformed):
    report = canonical_report()
    report[section] = malformed
    assert not VERIFY.report_complete(report)


def test_github_tag_resolves_annotated_tag_to_commit(monkeypatch):
    sha = canonical_report()["workflows"][0]["head_sha"]
    calls = []

    def request(url, token):
        calls.append(url)
        return {"sha": sha}

    monkeypatch.setattr(VERIFY, "request_json", request)
    assert VERIFY.github_tag_sha(VERIFY.REPOSITORY, "token", "geo-polygonize-v1.0.0") == sha
    assert calls == [f"https://api.github.com/repos/{VERIFY.REPOSITORY}/commits/geo-polygonize-v1.0.0"]


@pytest.mark.parametrize("response", [None, {}, {"sha": "not-a-sha"}])
def test_github_tag_rejects_invalid_commit_response(monkeypatch, response):
    monkeypatch.setattr(VERIFY, "request_json", lambda *args: response)
    with pytest.raises(VERIFY.VerificationError, match="no valid commit SHA"):
        VERIFY.github_tag_sha("repo", "token", "geo-polygonize-v1.0.0")


def test_github_runs_are_tag_sha_event_scoped_and_paginated(monkeypatch):
    report = canonical_report()
    sha = report["workflows"][0]["head_sha"]
    calls = []

    def request(url, token):
        parsed = urlparse(url)
        filename = parsed.path.split("/")[-2]
        query = parse_qs(parsed.query)
        assert query["branch"] == [report["tag"]]
        assert query["head_sha"] == [sha]
        assert query["event"] == ["push"]
        page = int(query["page"][0])
        calls.append((filename, page))
        name = VERIFY.PUBLISH_WORKFLOWS[filename]
        if page == 1:
            wrong = [
                publication_run(name, head_branch="main"),
                publication_run(name, head_sha="a" * 40),
                publication_run(name, event="workflow_dispatch"),
                publication_run("Unrelated workflow"),
            ]
            return {"workflow_runs": wrong * 25, "total_count": 101}
        return {"workflow_runs": [publication_run(name)], "total_count": 101}

    monkeypatch.setattr(VERIFY, "request_json", request)
    runs = VERIFY.github_runs(VERIFY.REPOSITORY, "token", report["tag"], sha)
    assert {run["workflow"] for run in runs} == set(VERIFY.PUBLISH_WORKFLOWS.values())
    assert len(calls) == 6


@pytest.mark.parametrize("status,conclusion", [("completed", "failure"), ("in_progress", None), ("completed", "success")])
def test_github_runs_choose_latest_exact_tag_run(monkeypatch, status, conclusion):
    report = canonical_report()
    sha = report["workflows"][0]["head_sha"]

    def request(url, token):
        name = VERIFY.PUBLISH_WORKFLOWS[urlparse(url).path.split("/")[-2]]
        return {"workflow_runs": [
            publication_run(name),
            publication_run(name, 2, created_at="2026-08-04T00:00:00Z", status=status, conclusion=conclusion),
        ]}

    monkeypatch.setattr(VERIFY, "request_json", request)
    runs = VERIFY.github_runs(VERIFY.REPOSITORY, "token", report["tag"], sha)
    assert all(run["run_id"] == 2 and run["status"] == status and run["conclusion"] == conclusion for run in runs)


def test_github_runs_fail_closed_at_pagination_limit(monkeypatch):
    name = VERIFY.PUBLISH_WORKFLOWS["publish.yml"]
    calls = []

    def request(url, token):
        calls.append(url)
        return {"workflow_runs": [publication_run(name)] * 100, "total_count": 1001}

    monkeypatch.setattr(VERIFY, "request_json", request)
    with pytest.raises(VERIFY.VerificationError, match="exceeds 1000"):
        VERIFY.github_runs(VERIFY.REPOSITORY, "token", "geo-polygonize-v1.0.0", "a" * 40)
    assert len(calls) == 10


def test_publishers_wait_for_same_run_rerun_to_finish(monkeypatch):
    complete = canonical_report()["workflows"]
    pending = [dict(run, status="in_progress", conclusion=None) for run in complete]
    responses = iter([pending, complete])
    sleeps = []
    monkeypatch.setattr(VERIFY, "github_runs", lambda *args: next(responses))
    monkeypatch.setattr(VERIFY.time, "sleep", sleeps.append)
    assert VERIFY.wait_for_publishers("repo", "token", "tag", "sha", 2, 30) == complete
    assert sleeps == [30]


@pytest.mark.parametrize("runs", [[], [dict(canonical_report()["workflows"][0], status="in_progress", conclusion=None)]])
def test_publishers_timeout_is_bounded(monkeypatch, runs):
    calls, sleeps = [], []

    def discover(*args):
        calls.append(args)
        return runs

    monkeypatch.setattr(VERIFY, "github_runs", discover)
    monkeypatch.setattr(VERIFY.time, "sleep", sleeps.append)
    assert VERIFY.wait_for_publishers("repo", "token", "tag", "sha", 3, 30) == runs
    assert len(calls) == 3
    assert sleeps == [30, 30]


@pytest.mark.parametrize("conclusion", ["failure", "cancelled", "skipped"])
def test_publishers_stop_on_terminal_failure(monkeypatch, conclusion):
    failed = [dict(canonical_report()["workflows"][0], conclusion=conclusion)]
    monkeypatch.setattr(VERIFY, "github_runs", lambda *args: failed)
    monkeypatch.setattr(VERIFY.time, "sleep", lambda _: pytest.fail("terminal failure must not wait"))
    assert VERIFY.wait_for_publishers("repo", "token", "tag", "sha", 3, 30) == failed


def test_release_report_records_resolved_sha_and_final_publishers(monkeypatch):
    canonical = canonical_report()
    sha = canonical["workflows"][0]["head_sha"]
    pending = [dict(run, status="in_progress", conclusion=None) for run in canonical["workflows"]]
    responses = iter([pending, canonical["workflows"]])
    monkeypatch.setattr(VERIFY, "github_tag_sha", lambda *args: sha)
    monkeypatch.setattr(VERIFY, "verify_registries", lambda *args: canonical["registries"])
    monkeypatch.setattr(VERIFY, "github_runs", lambda *args: next(responses))
    monkeypatch.setattr(VERIFY.time, "sleep", lambda _: None)
    monkeypatch.setattr(VERIFY, "smoke_registry_installs", lambda *args: canonical["smoke"])
    report = VERIFY.release_report("1.0.0", canonical["tag"], VERIFY.REPOSITORY, "token", 2, 30)
    assert report["head_sha"] == sha
    assert report["workflows"] == canonical["workflows"]
    assert report["complete"] is True


def test_release_report_waits_before_registry_snapshot(monkeypatch):
    canonical = canonical_report()
    sha = canonical["workflows"][0]["head_sha"]
    events = []

    def resolve(*args):
        events.append("tag")
        return sha

    def wait(*args):
        assert events == ["tag"]
        events.append("publishers")
        return canonical["workflows"]

    def registries(*args):
        assert events == ["tag", "publishers"]
        events.append("registries")
        return canonical["registries"]

    def smoke(*args):
        assert events == ["tag", "publishers", "registries"]
        events.append("smoke")
        return canonical["smoke"]

    monkeypatch.setattr(VERIFY, "github_tag_sha", resolve)
    monkeypatch.setattr(VERIFY, "wait_for_publishers", wait)
    monkeypatch.setattr(VERIFY, "verify_registries", registries)
    monkeypatch.setattr(VERIFY, "smoke_registry_installs", smoke)
    report = VERIFY.release_report("1.0.0", canonical["tag"], VERIFY.REPOSITORY, "token", 2, 30)
    assert events == ["tag", "publishers", "registries", "smoke"]
    assert report["complete"] is True


@pytest.mark.parametrize("invalid", ["complete_only", "repository", "sha", "smoke", "tag"])
def test_previous_gate_revalidates_claimed_complete_report(monkeypatch, tmp_path, invalid):
    report = canonical_report()
    sha, tag = report["workflows"][0]["head_sha"], report["tag"]
    if invalid == "complete_only":
        report = {"complete": True, "release_version": "1.0.0", "tag": tag}
    elif invalid == "repository":
        report["repository"] = "owner/other"
    elif invalid == "sha":
        for run in report["workflows"]:
            run["head_sha"] = "a" * 40
    elif invalid == "smoke":
        report["smoke"] = {}
    else:
        report["tag"] = "geo-polygonize-v1.1.0"
    monkeypatch.setattr(VERIFY, "latest_release_tag", lambda *args: tag)
    monkeypatch.setattr(VERIFY, "github_tag_sha", lambda *args: sha)
    monkeypatch.setattr(VERIFY, "github_release", lambda *args: {
        "assets": [{"name": "release-publication-report.json", "browser_download_url": "report"}]
    })
    monkeypatch.setattr(VERIFY, "request_json", lambda *args: report)
    with pytest.raises(VERIFY.VerificationError, match="no complete publication report"):
        VERIFY.check_previous_report(VERIFY.REPOSITORY, "token", tmp_path)


def test_previous_gate_accepts_canonical_file_repair(monkeypatch):
    report = canonical_report()
    monkeypatch.setattr(VERIFY, "latest_release_tag", lambda *args: report["tag"])
    monkeypatch.setattr(VERIFY, "github_tag_sha", lambda *args: report["workflows"][0]["head_sha"])
    monkeypatch.setattr(VERIFY, "github_release", lambda *args: {"assets": []})
    VERIFY.check_previous_report(VERIFY.REPOSITORY, "token", ROOT)
