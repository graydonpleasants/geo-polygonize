import sys
import os
import numpy as np

# Add python directory to path to import geo_polygonize
sys.path.append(os.path.join(os.path.dirname(__file__), "../python"))

from geo_polygonize import polygonize

def test_3d_interpolation():
    # Line 1: (0,0,0) -> (10,10,10). Z increases with dist.
    # Line 2: (0,10,20) -> (10,0,30). Z increases.
    # Intersection at (5,5).
    # Z1 = 5.
    # Z2 = 25.

    # Coords: 3D
    coords = np.array([
        0.0, 0.0, 0.0,
        10.0, 10.0, 10.0,
        0.0, 10.0, 20.0,
        10.0, 0.0, 30.0
    ], dtype=np.float64)

    offsets = np.array([0, 2, 4], dtype=np.uint32)

    # We need to node
    result = polygonize(coords, offsets, node=True, snap=1e-10, stride=3)

    dangles = result['dangles']
    print(f"Found {len(dangles)} dangles")

    # Collect all points
    points = []
    for d in dangles:
        points.extend(d)

    # Find points near (5,5)
    center_points = [p for p in points if abs(p[0] - 5.0) < 1e-5 and abs(p[1] - 5.0) < 1e-5]

    print("Center points found:", center_points)

    z_values = sorted([p[2] for p in center_points])

    assert len(z_values) > 0
    # Check that we have interpolated values.
    # We expect close to 5.0 or 25.0
    matched = False
    for z in z_values:
        if abs(z - 5.0) < 1e-5 or abs(z - 25.0) < 1e-5:
            matched = True
            break
    assert matched, f"No expected Z values found in {z_values}"

    print("Test 3D interpolation passed!")

def test_3d_polygon_preservation():
    # A simple triangle: (0,0,10) -> (10,0,10) -> (0,10,10) -> (0,0,10).
    # Should result in a polygon with Z=10 everywhere.

    coords = np.array([
        0.0, 0.0, 10.0,
        10.0, 0.0, 10.0,
        0.0, 10.0, 10.0,
        0.0, 0.0, 10.0
    ], dtype=np.float64)

    offsets = np.array([0, 4], dtype=np.uint32)

    result = polygonize(coords, offsets, node=False, stride=3)
    polys = result['polygons']
    assert len(polys) == 1

    poly = polys[0]
    # Check Z
    for pt in poly.shell:
        assert abs(pt[2] - 10.0) < 1e-6, f"Expected Z=10, got {pt[2]}"

    print("Test 3D polygon preservation passed!")

if __name__ == "__main__":
    test_3d_interpolation()
    test_3d_polygon_preservation()
