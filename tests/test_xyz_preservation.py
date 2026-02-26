import sys
import os
import numpy as np

# Add python directory to path to import geo_polygonize
sys.path.append(os.path.join(os.path.dirname(__file__), "../python"))

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

    # Check flat coords directly if available, else infer from polygons
    if 'flat_coords' in out:
        flat = out["flat_coords"].reshape(-1, 3)
    else:
        # Reconstruct from polygons and dangles for test
        flat = []
        for p in out['polygons']:
            flat.extend(p.shell)
            for h in p.holes:
                flat.extend(h)
        for d in out['dangles']:
            flat.extend(d)
        flat = np.array(flat)

    assert len(flat) > 0

    # Original vertices preserved (approximate check as noding might shift slightly or dedup)
    # (0,0,0)
    assert any(np.allclose(pt, [0.0, 0.0, 0.0]) for pt in flat)
    # (10,0,10)
    assert any(np.allclose(pt, [10.0, 0.0, 10.0]) for pt in flat)

    # Interpolated z at boundary intersections of splitter
    # Splitter is Y=5. Square vertical edges are at X=0 and X=10.

    # Intersection 1: X=0, Y=5.
    # On Square edge (0,0,0)->(0,10,30) (Wait, order was 0,0->10,0->10,10->0,10->0,0)
    # Edge 4: (0,10,30) -> (0,0,0).
    # Midpoint Y=5. Z should be (30+0)/2 = 15.
    # Splitter: (-1,5,0) -> (11,5,12). Length 12. X goes -1 to 11.
    # At X=0, dist=1/12. Z = 0 + (1/12)*12 = 1.
    # Noder will create intersection.
    # Depending on implementation, it might create TWO points at same location with different Z if lines don't merge?
    # But SnapNoder snaps to same point.
    # Z-interpolation behavior:
    # If we snap, we need to decide Z.
    # Current implementation interpolates Z from the line segment being split.
    # If both lines split at same XY, we get two events.
    # So we should see points with Z=15 (from box edge) and Z=1 (from splitter).

    found_z15 = any(np.allclose(pt, [0.0, 5.0, 15.0], atol=1e-6) for pt in flat)
    found_z1 = any(np.allclose(pt, [0.0, 5.0, 1.0], atol=1e-6) for pt in flat)

    # Intersection 2: X=10, Y=5.
    # On Square edge (10,0,10)->(10,10,20).
    # Midpoint Y=5. Z = (10+20)/2 = 15.
    # Splitter at X=10: dist=11/12. Z = 11.

    found_z_box_2 = any(np.allclose(pt, [10.0, 5.0, 15.0], atol=1e-6) for pt in flat)
    found_z_split_2 = any(np.allclose(pt, [10.0, 5.0, 11.0], atol=1e-6) for pt in flat)

    print(f"Found Z=15 at (0,5): {found_z15}")
    print(f"Found Z=1 at (0,5): {found_z1}")
    print(f"Found Z=15 at (10,5): {found_z_box_2}")
    print(f"Found Z=11 at (10,5): {found_z_split_2}")

    assert found_z15 or found_z1
    assert found_z_box_2 or found_z_split_2

    print("Test XYZ preservation passed!")

if __name__ == "__main__":
    test_xyz_preservation_and_interpolation()
