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
