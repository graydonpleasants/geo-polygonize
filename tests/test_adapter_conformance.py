import json
from pathlib import Path

import numpy as np

import geo_polygonize


FIXTURE = (
    Path(__file__).resolve().parents[1]
    / "crates/geo-polygonize-core/tests/fixtures/conformance/axis_aligned_ring_v1.json"
)


def test_python_canonical_options_matches_shared_fingerprint_fixture():
    fixture = json.loads(FIXTURE.read_text())
    result = geo_polygonize.polygonize_with_options(
        coords=np.asarray(fixture["coords"], dtype=np.float64),
        offsets=np.asarray(fixture["offsets"], dtype=np.uint32),
        stride=fixture["stride"],
        line_ids=np.asarray(fixture["line_ids"], dtype=np.uint32),
        options=fixture["options"],
    )
    assert result["topology_fingerprint"] == fixture["expected_fingerprint"]
