import json
from pathlib import Path

import numpy as np
import pytest

import geo_polygonize


FIXTURE = (
    Path(__file__).resolve().parents[1]
    / "crates/geo-polygonize-core/tests/fixtures/conformance/axis_aligned_ring_v1.json"
)


def test_python_report_entrypoints_match_shared_fingerprint_fixture():
    fixture = json.loads(FIXTURE.read_text())
    canonical = geo_polygonize.polygonize_with_options(
        coords=np.asarray(fixture["coords"], dtype=np.float64),
        offsets=np.asarray(fixture["offsets"], dtype=np.uint32),
        stride=fixture["stride"],
        line_ids=np.asarray(fixture["line_ids"], dtype=np.uint32),
        options=fixture["options"],
    )
    legacy = geo_polygonize.polygonize(
        coords=np.asarray(fixture["coords"], dtype=np.float64),
        offsets=np.asarray(fixture["offsets"], dtype=np.uint32),
        stride=fixture["stride"],
        line_ids=np.asarray(fixture["line_ids"], dtype=np.uint32),
    )

    assert canonical["topology_fingerprint"] == fixture["expected_fingerprint"]
    assert legacy["topology_fingerprint"] == fixture["expected_fingerprint"]


def test_python_entrypoints_expose_the_same_normalized_core_error():
    fixture = json.loads(FIXTURE.read_text())
    arguments = {
        "coords": np.asarray(fixture["coords"], dtype=np.float64),
        "offsets": np.asarray(fixture["offsets"], dtype=np.uint32),
        "stride": fixture["stride"],
        "line_ids": np.asarray(fixture["line_ids"], dtype=np.uint32),
        "execution_limits": {"max_input_segments": 0},
    }

    normalized = []
    for entrypoint in (
        geo_polygonize.polygonize_with_options,
        geo_polygonize.polygonize,
    ):
        with pytest.raises(RuntimeError) as caught:
            entrypoint(**arguments)
        normalized.append(json.loads(caught.value.normalized))

    assert normalized[0] == normalized[1] == {
        "schema_version": 1,
        "family": "resource_limit",
        "code": "resource_limit_exceeded",
        "stage": "input_segments",
        "field": None,
        "expected": None,
        "actual": None,
        "limit": "0",
        "observed": "1",
        "witness": None,
    }
