import importlib.util
from pathlib import Path

import pytest


PATH = Path(__file__).resolve().parents[1] / "benchmarks/reference_geos.py"
SPEC = importlib.util.spec_from_file_location("reference_geos", PATH)
REFERENCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REFERENCE)


def test_ring_and_line_canonicalization_preserve_negative_zero_contract():
    ring = [(1.0, -0.0), (0.0, 1.0), (-0.0, 0.0), (1.0, -0.0)]
    assert REFERENCE.canonical_ring(ring) == [
        {"x": "0x0000000000000000", "y": "0x0000000000000000"},
        {"x": "0x0000000000000000", "y": "0x3ff0000000000000"},
        {"x": "0x3ff0000000000000", "y": "0x0000000000000000"},
        {"x": "0x0000000000000000", "y": "0x0000000000000000"},
    ]
    assert REFERENCE.canonical_line([(1.0, 0.0), (0.0, 0.0)])[0]["x"] == (
        "0x0000000000000000"
    )


@pytest.mark.parametrize(
    ("workload", "lane"),
    [
        ("already-noded-coverage-v1", "already-noded"),
        ("network-linework-v1", "floating"),
    ],
)
def test_reference_generation_is_deterministic(workload, lane):
    root = PATH.parents[1]
    _, lines = REFERENCE.load_workload(root, workload)
    first = REFERENCE.canonical_topology(lines, lane)
    second = REFERENCE.canonical_topology(list(reversed(lines)), lane)
    assert REFERENCE.canonical_json(first) == REFERENCE.canonical_json(second)


def test_reference_loader_accepts_an_external_manifest():
    root = PATH.parents[1]
    manifest = root / "crates/geo-polygonize-core/tests/workloads/manifest-v1.json"
    default_workload, default_lines = REFERENCE.load_workload(root, "network-linework-v1")
    external_workload, external_lines = REFERENCE.load_workload(
        root, "network-linework-v1", manifest
    )
    assert external_workload == default_workload
    assert len(external_lines) == len(default_lines)
