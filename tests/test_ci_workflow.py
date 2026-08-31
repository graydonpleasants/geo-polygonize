import re
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
    assert workflow.index("npm ci --prefix playground") < workflow.index(
        "npm run build --prefix playground"
    )
    assert "npm run build --prefix playground" in workflow


def test_supported_feature_and_python_abi_matrix_is_required():
    workflow = (ROOT / ".github/workflows/ci.yml").read_text()

    assert (
        "cargo test --locked -p geo-polygonize-core --no-default-features --lib --tests"
        in workflow
    )
    assert (
        "cargo check --locked -p geo-polygonize-core --all-targets --all-features"
        in workflow
    )
    assert "variant: [scalar, simd, threads]" in workflow
    assert "python-version: ['3.8', '3.x']" in workflow
    assert "from geo_polygonize.geo_polygonize_core import polygonize_with_options" in workflow
    assert "needs.python-abi-build.result" in workflow
    assert "needs.python-abi.result" in workflow


def test_benchmark_only_changes_do_not_fan_out_to_unrelated_pr_checks():
    ci = (ROOT / ".github/workflows/ci.yml").read_text()
    docs = (ROOT / ".github/workflows/docs-pages.yml").read_text()
    differential = (ROOT / ".github/workflows/python-tests.yml").read_text()
    unsafe = (ROOT / ".github/workflows/unsafe-boundaries.yml").read_text()
    maintenance = (ROOT / ".github/workflows/maintenance.yml").read_text()
    perf = (ROOT / ".github/workflows/perf.yml").read_text()
    filters = [
        line.split("grep -Eq '", 1)[1].split("'; then", 1)[0]
        for line in ci.splitlines()
        if "grep -Eq '" in line
    ]

    assert len(filters) == 2
    assert all(
        re.search(pattern, "crates/geo-polygonize-core/benches/polygonize_bench.rs")
        is None
        for pattern in filters
    )
    assert all(
        re.search(pattern, "crates/geo-polygonize-core/src/lib.rs")
        for pattern in filters
    )
    assert "cargo test --release --locked -p geo-polygonize-arrow" not in ci
    assert "crates/geo-polygonize-core/**" not in docs
    assert "crates/geo-polygonize-core/**" not in differential
    assert "crates/geo-polygonize-core/**" not in unsafe
    assert "pull_request:" not in maintenance
    assert "pull_request:" not in perf
    assert "workflow_dispatch:" in perf


def test_scheduled_differential_fuzzing_retains_review_candidates():
    workflow = (ROOT / ".github/workflows/fuzz.yml").read_text()

    assert 'cron: "0 6 * * 1"' in workflow
    assert "prepare_adapter_differential_candidate" in workflow
    assert "fuzz/artifacts/adapter_differential/*" in workflow
    assert "target/differential-fuzz-candidates/" in workflow
    assert "retention-days: 30" in workflow


def test_miri_uses_a_pinned_no_default_feature_slice():
    workflow = (ROOT / ".github/workflows/unsafe-boundaries.yml").read_text()

    assert 'toolchain: "nightly-2026-07-15"' in workflow
    assert "components: miri, rust-src" in workflow
    assert "cargo miri test --locked -p geo-polygonize-core" in workflow
    assert "--no-default-features --test execution_policy" in workflow


def test_address_sanitizer_covers_the_arrow_c_ownership_boundary():
    workflow = (ROOT / ".github/workflows/unsafe-boundaries.yml").read_text()

    assert "address-sanitizer-arrow-ffi:" in workflow
    assert "-Zsanitizer=address" in workflow
    assert "--target x86_64-unknown-linux-gnu" in workflow
    assert "--test arrow_integration --test test_ffi_errors" in workflow
