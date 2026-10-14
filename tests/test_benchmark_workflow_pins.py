from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def workflow(name):
    return (ROOT / ".github" / "workflows" / name).read_text()


def test_comparison_workflows_pin_toolchains_and_references():
    evidence = workflow("benchmark-evidence.yml")
    cross_arch = workflow("cross-architecture-benchmarks.yml")
    maintenance = workflow("maintenance.yml")
    requirements = (ROOT / "benchmarks" / "reference-requirements.txt").read_text()
    pom = (ROOT / "benchmarks" / "jts-reference" / "pom.xml").read_text()

    assert evidence.count('toolchain: "1.96.1"') == 2
    assert 'python-version: "3.12.11"' in evidence
    assert "Shapely==2.1.2" in requirements
    assert "<jts.version>1.20.0</jts.version>" in pom

    assert cross_arch.count('toolchain: "1.96.1"') == 2
    assert 'toolchain: "nightly-2026-07-15"' in cross_arch
    assert 'node-version: "22.17.1"' in cross_arch

    assert "toolchain: '1.96.1'" in maintenance
    assert "python-version: '3.12.11'" in maintenance
    assert "Shapely==2.1.2 numpy==2.3.2" in maintenance
    assert "node-version: '22.17.1'" in maintenance
    assert "wasm-pack@0.13.1" in maintenance


def test_router_feature_is_only_enabled_by_the_research_benchmark_job():
    cross_arch = workflow("cross-architecture-benchmarks.yml")
    wasm_job = cross_arch.split("  wasm:", 1)[1].split("  attach-to-release:", 1)[0]
    assert 'WASM_ROUTER_BENCHMARKS: "1"' in wasm_job
    assert "THREADS_TOOLCHAIN: nightly-2026-07-15" in wasm_job
    assert "node scripts/test_wasm_router_exports.mjs pkg-scalar research" in wasm_job
    assert '- "scripts/test_wasm_router_exports.mjs"' in cross_arch
    for name in ["ci.yml", "publish-npm.yml", "docs-pages.yml"]:
        assert "WASM_ROUTER_BENCHMARKS" not in workflow(name)
