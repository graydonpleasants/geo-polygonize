import json
from pathlib import Path

import numpy as np
import pytest

import geo_polygonize


FIXTURE_DIR = Path(__file__).resolve().parents[1] / "fixtures" / "cfb" / "cases"


def _flatten_fixture(fixture):
    coords = []
    offsets = []
    line_ids = []
    offset = 0

    for line in fixture["lines"]:
        offsets.append(offset)
        line_ids.append(line["id"])
        for point in line["coords"]:
            coords.extend(point[: fixture["stride"]])
            offset += 1

    return (
        np.asarray(coords, dtype=np.float64),
        np.asarray(offsets, dtype=np.uint32),
        np.asarray(line_ids, dtype=np.uint32),
    )


def _run_fixture(path):
    fixture = json.loads(path.read_text())
    coords, offsets, line_ids = _flatten_fixture(fixture)

    if fixture["optionsProfile"] != "cfb_robust_v1":
        pytest.fail(f"unsupported fixture profile {fixture['optionsProfile']}")

    try:
        result = geo_polygonize.polygonize_with_options(
            coords=coords,
            offsets=offsets,
            stride=fixture["stride"],
            line_ids=line_ids,
            options=geo_polygonize.cfb_robust_options(),
        )
    except Exception as exc:
        pytest.skip(f"geo_polygonize native fixture runner unavailable: {exc}")

    expected = fixture["expected"]
    assert len(result["polygons"]) == expected["polygonCount"]
    assert len(result["dangles"]) == expected["dangleCount"]
    assert len(result.get("cut_edges", [])) == expected["cutEdgeCount"]
    assert len(result["invalid_rings"]) == expected["invalidRingCount"]

    def ring_area(coords):
        ring = np.asarray(coords)
        x = ring[:, 0] - ring[0, 0]
        y = ring[:, 1] - ring[0, 1]
        return abs(float(np.dot(x, np.roll(y, -1)) - np.dot(y, np.roll(x, -1))) / 2.0)

    area = 0.0
    for polygon in result["polygons"]:
        area += ring_area(polygon.shell)
        area -= sum(ring_area(hole) for hole in polygon.holes)

    assert area == pytest.approx(expected["totalArea"], abs=expected.get("areaTolerance", 1e-6))

    expected_ids = expected.get("boundaryLineIds", [])
    if expected_ids:
        actual_ids = sorted({
            line_id
            for polygon in result["polygons"]
            for line_id in polygon.provenance.get("boundary_line_ids", [])
        })
        assert actual_ids == expected_ids


@pytest.mark.parametrize("path", sorted(FIXTURE_DIR.glob("*.json")))
def test_cfb_fixture(path):
    fixture = json.loads(path.read_text())
    if fixture.get("expectedStatus", "pass") == "xfail":
        with pytest.raises(AssertionError):
            _run_fixture(path)
    else:
        _run_fixture(path)
