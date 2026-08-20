#!/usr/bin/env python3
"""Verify published release artifacts and smoke-test public registry installs.

The publish workflows are intentionally independent.  This verifier is the
post-publication contract: it checks the public registries, records the tag
workflow runs, installs exact released artifacts outside the repository, and
emits one machine-readable report suitable for a release asset.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


REPOSITORY = "graydonpleasants/geo-polygonize"
REPORT_SCHEMA_VERSION = 1
USER_AGENT = "geo-polygonize-release-verifier/1.0 (+https://github.com/graydonpleasants/geo-polygonize)"
CRATE_PACKAGES = (
    "geo-polygonize-core",
    "geo-polygonize-arrow",
    "geo-polygonize-flatgeobuf",
    "geo-polygonize-wasm",
)
PUBLISH_WORKFLOWS = {
    "publish.yml": "Publish to crates.io",
    "publish-npm.yml": "Publish to npm",
    "publish-python.yml": "Publish Python Package",
}
VERSION_RE = re.compile(r"^(?:geo-polygonize-v|v)(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)$")


class VerificationError(RuntimeError):
    """A bounded verification failure with a user-actionable message."""


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def version_from_tag(tag: str) -> str:
    match = VERSION_RE.fullmatch(tag)
    if not match:
        raise VerificationError(f"tag {tag!r} is not a supported geo-polygonize release tag")
    return match.group(1)


def tag_for_version(version: str) -> str:
    return f"geo-polygonize-v{version}"


def request_json(url: str, token: str | None = None) -> object:
    headers = {"Accept": "application/json", "User-Agent": USER_AGENT}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        raise VerificationError(f"GET {url} returned HTTP {error.code}: {body[:400]}") from error
    except urllib.error.URLError as error:
        raise VerificationError(f"GET {url} failed: {error.reason}") from error


def request_json_with_retries(url: str, attempts: int, delay_seconds: float) -> object:
    last_error: VerificationError | None = None
    for attempt in range(1, attempts + 1):
        try:
            return request_json(url)
        except VerificationError as error:
            last_error = error
            if attempt < attempts:
                time.sleep(delay_seconds)
    assert last_error is not None
    raise last_error


def run(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> str:
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=900,
    )
    if result.returncode:
        output = result.stdout.strip()
        raise VerificationError(
            f"command {' '.join(command)} failed with exit {result.returncode}: {output[-4000:]}"
        )
    return result.stdout


def run_stdout(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> str:
    """Run a command while keeping machine-readable stdout free of cargo logs."""
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=900,
    )
    if result.returncode:
        output = (result.stdout + result.stderr).strip()
        raise VerificationError(
            f"command {' '.join(command)} failed with exit {result.returncode}: {output[-4000:]}"
        )
    return result.stdout


def registry_record(registry: str, package: str, version: str, attempts: int, delay_seconds: float) -> dict:
    if registry == "crates.io":
        encoded = urllib.parse.quote(package, safe="")
        data = request_json_with_retries(
            f"https://crates.io/api/v1/crates/{encoded}/{version}", attempts, delay_seconds
        )
        published = data.get("version", {})
        if published.get("num") != version or published.get("yanked"):
            raise VerificationError(f"crates.io has no non-yanked {package} {version}")
        return {
            "registry": registry,
            "package": package,
            "version": version,
            "status": "available",
            "published_at": published.get("created_at"),
            "artifact": {
                "download_url": f"https://crates.io{published['dl_path']}",
                "sha256": published.get("checksum"),
                "size_bytes": published.get("crate_size"),
            },
            "platforms": ["source"],
        }

    if registry == "npm":
        encoded = urllib.parse.quote(package, safe="")
        data = request_json_with_retries(
            f"https://registry.npmjs.org/{encoded}", attempts, delay_seconds
        )
        published = data.get("versions", {}).get(version)
        if not published:
            raise VerificationError(f"npm has no {package} {version}")
        dist = published.get("dist", {})
        return {
            "registry": registry,
            "package": package,
            "version": version,
            "status": "available",
            "published_at": data.get("time", {}).get(version),
            "artifact": {
                "tarball": dist.get("tarball"),
                "shasum": dist.get("shasum"),
                "integrity": dist.get("integrity"),
            },
            "platforms": ["node", "wasm-scalar", "wasm-slim"],
        }

    if registry == "pypi":
        encoded = urllib.parse.quote(package, safe="")
        data = request_json_with_retries(
            f"https://pypi.org/pypi/{encoded}/{version}/json", attempts, delay_seconds
        )
        files = data.get("urls", [])
        if not files:
            raise VerificationError(f"PyPI has no files for {package} {version}")
        return {
            "registry": registry,
            "package": package,
            "version": version,
            "status": "available",
            "published_at": min(file.get("upload_time_iso", file.get("upload_time")) for file in files),
            "artifact": {
                "files": [
                    {
                        "filename": file.get("filename"),
                        "packagetype": file.get("packagetype"),
                        "python_version": file.get("python_version"),
                        "digests": file.get("digests", {}),
                    }
                    for file in files
                ]
            },
            "platforms": sorted({file.get("filename", "").split("-")[-1] for file in files}),
        }

    raise VerificationError(f"unsupported registry {registry}")


def verify_registries(version: str, attempts: int, delay_seconds: float) -> list[dict]:
    records: list[dict] = []
    for package in CRATE_PACKAGES:
        try:
            records.append(registry_record("crates.io", package, version, attempts, delay_seconds))
        except VerificationError as error:
            records.append(
                {"registry": "crates.io", "package": package, "version": version, "status": "missing", "error": str(error)}
            )
    for registry, package in (("npm", "geo-polygonize"), ("pypi", "geo-polygonize-py")):
        try:
            records.append(registry_record(registry, package, version, attempts, delay_seconds))
        except VerificationError as error:
            records.append(
                {"registry": registry, "package": package, "version": version, "status": "missing", "error": str(error)}
            )
    return records


def github_runs(repository: str, token: str, tag: str) -> list[dict]:
    data = request_json(
        f"https://api.github.com/repos/{repository}/actions/runs?event=push&per_page=100", token
    )
    runs = []
    for run_info in data.get("workflow_runs", []):
        if run_info.get("head_branch") != tag:
            continue
        workflow_name = run_info.get("name")
        if workflow_name not in PUBLISH_WORKFLOWS.values():
            continue
        runs.append(
            {
                "workflow": workflow_name,
                "run_id": run_info.get("id"),
                "status": run_info.get("status"),
                "conclusion": run_info.get("conclusion"),
                "head_sha": run_info.get("head_sha"),
                "created_at": run_info.get("created_at"),
                "updated_at": run_info.get("updated_at"),
                "url": run_info.get("html_url"),
            }
        )
    return sorted(runs, key=lambda value: (value["workflow"], value["created_at"] or ""))


def smoke_rust(version: str) -> dict:
    with tempfile.TemporaryDirectory(prefix="geo-polygonize-release-rust-") as directory:
        root = Path(directory)
        (root / "src").mkdir()
        (root / "Cargo.toml").write_text(
            f'''[package]\nname = "registry-smoke"\nversion = "0.0.0"\nedition = "2021"\n\n[dependencies]\ngeo-polygonize-core = "={version}"\n'''
        )
        (root / "src/main.rs").write_text(
            '''use geo_polygonize_core::{normalize_polygonize_error, polygonize, Coord3D, Line3D, PolygonizerOptions};

fn line(start: (f64, f64), end: (f64, f64), id: u32) -> Line3D {
    Line3D::new(Coord3D::new(start.0, start.1, 0.0), Coord3D::new(end.0, end.1, 0.0), id)
}

fn main() {
    let square = [
        line((0.0, 0.0), (1.0, 0.0), 1),
        line((1.0, 0.0), (1.0, 1.0), 2),
        line((1.0, 1.0), (0.0, 1.0), 3),
        line((0.0, 1.0), (0.0, 0.0), 4),
    ];
    let result = polygonize(square, &PolygonizerOptions::default()).expect("canonical square");
    assert_eq!(result.polygons.len(), 1);

    let invalid = PolygonizerOptions { pre_snap_tolerance: 1.0, ..Default::default() };
    let error = polygonize([line((0.0, 0.0), (1.0, 0.0), 1)], &invalid).expect_err("invalid options");
    let normalized = normalize_polygonize_error(&error);
    assert_eq!(normalized.family, "invalid_argument");
    assert_eq!(normalized.code, "unsupported_option_combination");
    println!("version={}", env!("CARGO_PKG_VERSION"));
    println!("polygons={}", result.polygons.len());
    println!("error={}", normalized.code);
}
'''
        )
        cargo_home = root / "cargo-home"
        env = os.environ.copy()
        env["CARGO_HOME"] = str(cargo_home)
        env.pop("RUSTFLAGS", None)
        # The temporary consumer has no lockfile on its first invocation. The
        # first check resolves the exact dependency, after which every command
        # is locked and the metadata check rejects path/workspace resolution.
        run(["cargo", "check"], root, env)
        metadata = json.loads(run_stdout(["cargo", "metadata", "--format-version", "1", "--locked"], root, env))
        packages = [
            package
            for package in metadata["packages"]
            if package["name"] == "geo-polygonize-core" and package["version"] == version
        ]
        if len(packages) != 1 or not packages[0].get("source", "").startswith("registry+"):
            raise VerificationError("Rust smoke dependency did not resolve to the public registry")
        output = run(["cargo", "run", "--locked", "--quiet"], root, env)
        if "polygons=1" not in output or "error=unsupported_option_combination" not in output:
            raise VerificationError(f"Rust smoke output did not contain the canonical contract: {output[-1000:]}")
    return {"status": "passed", "package": "geo-polygonize-core", "version": version, "platform": "native-registry-consumer"}


def smoke_npm(version: str) -> dict:
    with tempfile.TemporaryDirectory(prefix="geo-polygonize-release-npm-") as directory:
        root = Path(directory)
        run(["npm", "init", "-y"], root)
        run(
            [
                "npm",
                "install",
                "--ignore-scripts",
                "--no-package-lock",
                "--no-save",
                "--no-audit",
                "--no-fund",
                f"geo-polygonize@{version}",
            ],
            root,
        )
        (root / "smoke.mjs").write_text(
            '''import { readFile } from "node:fs/promises";
import { join } from "node:path";
import * as standard from "geo-polygonize";
import * as slim from "geo-polygonize/slim";

const square = JSON.stringify({ type: "LineString", coordinates: [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]] });
const invalidOptions = { node_input: false, pre_snap_tolerance: 1 };
const packageJson = JSON.parse(await readFile(join(process.cwd(), "node_modules/geo-polygonize/package.json"), "utf8"));
if (packageJson.version !== process.env.EXPECTED_VERSION) throw new Error(`package version ${packageJson.version}`);

async function check(name, module) {
  const report = JSON.parse(module.polygonizeReportWithOptions(square, {}));
  if (report.schema_version !== 1 || report.polygons.length !== 1) throw new Error(`${name} canonical result failed`);
  try {
    module.polygonizeReportWithOptions(square, invalidOptions);
    throw new Error(`${name} accepted invalid options`);
  } catch (error) {
    if (error.normalized?.family !== "invalid_argument" || error.normalized?.code !== "unsupported_option_combination") throw error;
  }
}

await standard.default();
await check("standard", standard);
const wasm = await readFile(join(process.cwd(), "node_modules/geo-polygonize/dist/geo_polygonize.wasm"));
const slimExports = await slim.initBest({ module: wasm });
await check("slim", slimExports);
console.log(`version=${packageJson.version}`);
console.log("standard=passed");
console.log("slim=passed");
'''
        )
        env = os.environ.copy()
        env["EXPECTED_VERSION"] = version
        run(["node", "smoke.mjs"], root, env)
    return {"status": "passed", "package": "geo-polygonize", "version": version, "platform": "node-standard-slim-wasm"}


def smoke_python(version: str) -> dict:
    with tempfile.TemporaryDirectory(prefix="geo-polygonize-release-python-") as directory:
        root = Path(directory)
        venv = root / "venv"
        run([sys.executable, "-m", "venv", str(venv)], root)
        python = venv / "bin/python"
        run(
            [
                str(python),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-cache-dir",
                "--only-binary=:all:",
                f"geo-polygonize-py=={version}",
            ],
            root,
        )
        (root / "smoke.py").write_text(
            '''import importlib.metadata
import json
import numpy as np
import geo_polygonize

version = importlib.metadata.version("geo-polygonize-py")
assert version == __import__("os").environ["EXPECTED_VERSION"], version
coords = np.asarray([0, 0, 1, 0, 1, 1, 0, 1, 0, 0], dtype=np.float64)
offsets = np.asarray([0], dtype=np.uint32)
result = geo_polygonize.polygonize_with_options(coords=coords, offsets=offsets, options={})
assert len(result["polygons"]) == 1
try:
    geo_polygonize.polygonize_with_options(
        coords=coords,
        offsets=offsets,
        options={"node_input": False, "pre_snap_tolerance": 1.0},
    )
    raise AssertionError("invalid options were accepted")
except Exception as error:
    normalized = json.loads(error.normalized)
    assert normalized["family"] == "invalid_argument", normalized
    assert normalized["code"] == "unsupported_option_combination", normalized
print(f"version={version}")
print("polygons=1")
print("error=unsupported_option_combination")
'''
        )
        env = os.environ.copy()
        env["EXPECTED_VERSION"] = version
        env.pop("PYTHONPATH", None)
        run([str(python), "smoke.py"], root, env)
    return {"status": "passed", "package": "geo-polygonize-py", "version": version, "platform": "cp-abi3-registry-wheel"}


def smoke_registry_installs(version: str) -> dict:
    results = {}
    for name, function in (("rust", smoke_rust), ("npm", smoke_npm), ("python", smoke_python)):
        try:
            results[name] = function(version)
        except (OSError, VerificationError, subprocess.SubprocessError) as error:
            results[name] = {"status": "failed", "error": str(error)}
    return results


def report_complete(report: dict) -> bool:
    registries = report.get("registries", [])
    expected = len(CRATE_PACKAGES) + 2
    return (
        len(registries) == expected
        and all(record.get("status") == "available" for record in registries)
        and all(record.get("conclusion") == "success" for record in report.get("workflows", []))
        and len(report.get("workflows", [])) == len(PUBLISH_WORKFLOWS)
        and all(result.get("status") == "passed" for result in report.get("smoke", {}).values())
    )


def release_report(
    version: str,
    tag: str,
    repository: str,
    token: str,
    attempts: int,
    delay_seconds: float,
) -> dict:
    report = {
        "schema_version": REPORT_SCHEMA_VERSION,
        "release_version": version,
        "tag": tag,
        "verified_at": utc_now(),
        "repository": repository,
        "registries": verify_registries(version, attempts, delay_seconds),
        "workflows": github_runs(repository, token, tag),
        "smoke": smoke_registry_installs(version),
        "complete": False,
    }
    report["complete"] = report_complete(report)
    return report


def github_release(repository: str, token: str, tag: str) -> dict:
    encoded = urllib.parse.quote(tag, safe="")
    return request_json(f"https://api.github.com/repos/{repository}/releases/tags/{encoded}", token)


def semver_key(tag: str) -> tuple[int, int, int, str] | None:
    match = VERSION_RE.fullmatch(tag)
    if not match:
        return None
    version = match.group(1).split("+", 1)[0]
    core, _, prerelease = version.partition("-")
    major, minor, patch = (int(part) for part in core.split("."))
    return major, minor, patch, prerelease


def latest_release_tag(repository: str, token: str) -> str:
    tags = request_json(f"https://api.github.com/repos/{repository}/tags?per_page=100", token)
    candidates = [tag["name"] for tag in tags if semver_key(tag["name"]) is not None]
    if not candidates:
        raise VerificationError(f"no geo-polygonize release tags found in {repository}")
    return max(candidates, key=lambda tag: semver_key(tag))


def check_previous_report(repository: str, token: str, root: Path) -> None:
    tag = latest_release_tag(repository, token)
    version = version_from_tag(tag)
    report: dict | None = None
    try:
        release = github_release(repository, token, tag)
        asset = next(
            (
                asset
                for asset in release.get("assets", [])
                if asset.get("name") in {"publication-report.json", "release-publication-report.json"}
            ),
            None,
        )
        if asset:
            report = request_json(asset["browser_download_url"], token)
    except VerificationError:
        report = None
    if report is None:
        path = root / "release" / "reports" / f"{tag}.json"
        if path.is_file():
            report = json.loads(path.read_text())
    if not report or report.get("release_version") != version or report.get("tag") != tag or not report.get("complete"):
        raise VerificationError(
            f"previous release {tag} has no complete publication report; repair/verify it before publishing another version"
        )
    print(f"previous release gate passed: {tag}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version")
    parser.add_argument("--tag")
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", REPOSITORY))
    parser.add_argument("--github-token", default=os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN"))
    parser.add_argument("--report", type=Path)
    parser.add_argument("--attempts", type=int, default=6)
    parser.add_argument("--delay-seconds", type=float, default=30.0)
    parser.add_argument("--check-previous", action="store_true")
    args = parser.parse_args()

    try:
        if args.check_previous:
            if not args.github_token:
                raise VerificationError("--check-previous requires --github-token or GH_TOKEN")
            check_previous_report(args.repository, args.github_token, Path.cwd())
            return 0
        if args.attempts < 1 or args.delay_seconds < 0:
            raise VerificationError("attempts must be positive and delay-seconds must be non-negative")
        version = args.version or (version_from_tag(args.tag) if args.tag else None)
        if not version:
            raise VerificationError("provide --version or --tag")
        tag = args.tag or tag_for_version(version)
        if version_from_tag(tag) != version:
            raise VerificationError(f"version {version} does not match tag {tag}")
        if not args.github_token:
            raise VerificationError("release verification requires --github-token or GH_TOKEN")
        report = release_report(version, tag, args.repository, args.github_token, args.attempts, args.delay_seconds)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
        return 0 if report["complete"] else 1
    except (VerificationError, OSError, ValueError, json.JSONDecodeError) as error:
        if args.report:
            failure = {
                "schema_version": REPORT_SCHEMA_VERSION,
                "release_version": args.version,
                "tag": args.tag,
                "verified_at": utc_now(),
                "repository": args.repository,
                "complete": False,
                "error": str(error),
            }
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(failure, indent=2) + "\n")
        print(f"release verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
