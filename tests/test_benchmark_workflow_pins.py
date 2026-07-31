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
