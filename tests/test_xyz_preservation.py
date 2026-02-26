import numpy as np
from geo_polygonize import polygonize


def test_xyz_preservation_and_interpolation():
    coords = np.array([
        # square boundary (single linestring)
        0.0, 0.0, 0.0,
        10.0, 0.0, 10.0,
        10.0, 10.0, 20.0,
        0.0, 10.0, 30.0,
        0.0, 0.0, 0.0,
        # horizontal splitter crossing the square
        -1.0, 5.0, 0.0,
        11.0, 5.0, 12.0,
    ], dtype=np.float64)
    offsets = np.array([0, 5, 7], dtype=np.uint32)

    out = polygonize(coords, offsets, node=True, stride=3)
    flat = out["flat_coords"].reshape(-1, 3)

    assert len(flat) > 0

    # Original vertices preserved
    assert any(np.allclose(pt, [0.0, 0.0, 0.0]) for pt in flat)
    assert any(np.allclose(pt, [10.0, 0.0, 10.0]) for pt in flat)

    # Interpolated z at boundary intersections of splitter
    assert any(np.allclose(pt, [0.0, 5.0, 15.0], atol=1e-6) for pt in flat)
    assert any(np.allclose(pt, [10.0, 5.0, 15.0], atol=1e-6) for pt in flat)
