import sys
import os
import numpy as np
import shapely
from shapely.geometry import shape
from unittest.mock import patch

# Add python directory to path
sys.path.append(os.path.join(os.path.dirname(__file__)))

from geo_polygonize import polygonize

def test_square():
    print("Testing square polygonization...")
    coords = np.array([
        0.0, 0.0, 10.0, 0.0,
        10.0, 0.0, 10.0, 10.0,
        10.0, 10.0, 0.0, 10.0,
        0.0, 10.0, 0.0, 0.0
    ], dtype=np.float64)
    # Offsets in POINTS (pairs of doubles)
    # Fix: Added 8 to close the last segment
    offsets = np.array([0, 2, 4, 6, 8], dtype=np.uint32)

    polys = polygonize(coords, offsets, return_polygons=True)
    print(f"Result count: {len(polys)}")
    assert len(polys) == 1

    shapely_poly = polys[0]
    print(f"Shapely area: {shapely_poly.area}")
    assert abs(shapely_poly.area - 100.0) < 1e-6
    print("Square test passed!")

def test_two_squares():
    print("\nTesting two disjoint squares...")
    coords = np.array([
        # Square 1
        0.0, 0.0, 10.0, 0.0,
        10.0, 0.0, 10.0, 10.0,
        10.0, 10.0, 0.0, 10.0,
        0.0, 10.0, 0.0, 0.0,
        # Square 2
        20.0, 0.0, 30.0, 0.0,
        30.0, 0.0, 30.0, 10.0,
        30.0, 10.0, 20.0, 10.0,
        20.0, 10.0, 20.0, 0.0
    ], dtype=np.float64)

    # Fix: Added 16 to close the last segment
    offsets = np.array([0, 2, 4, 6, 8, 10, 12, 14, 16], dtype=np.uint32)

    polys = polygonize(coords, offsets, return_polygons=True)
    print(f"Result count: {len(polys)}")
    assert len(polys) == 2
    print("Two squares test passed!")

def test_square_with_hole():
    print("\nTesting square with hole...")
    coords = np.array([
        # Outer
        0.0, 0.0, 10.0, 0.0,
        10.0, 0.0, 10.0, 10.0,
        10.0, 10.0, 0.0, 10.0,
        0.0, 10.0, 0.0, 0.0,
        # Hole
        2.0, 2.0, 2.0, 8.0,
        2.0, 8.0, 8.0, 8.0,
        8.0, 8.0, 8.0, 2.0,
        8.0, 2.0, 2.0, 2.0
    ], dtype=np.float64)

    # 32 floats = 16 points
    # Fix: Added 16 (range to 18) to close the last segment
    offsets = np.arange(0, 18, 2, dtype=np.uint32)

    polys = polygonize(coords, offsets, return_polygons=True)
    print(f"Result count: {len(polys)}")

    # Polygonizer returns all possible polygons.
    # 1. The outer donut (100 - 36 = 64)
    # 2. The inner hole filled (36)
    assert len(polys) == 2

    areas = []
    for p in polys:
        areas.append(p.area)

    areas.sort()
    print(f"Areas: {areas}")

    assert abs(areas[0] - 36.0) < 1e-6
    assert abs(areas[1] - 64.0) < 1e-6
    print("Square with hole test passed!")

def test_3d_coordinates():
    print("\nTesting 3D coordinates (expecting Z to be ignored)...")
    # A square in 3D: (0,0,100), (10,0,101), (10,10,102), (0,10,103)
    coords = np.array([
        [0.0, 0.0, 100.0],
        [10.0, 0.0, 101.0],
        [10.0, 10.0, 102.0],
        [0.0, 10.0, 103.0],
        [0.0, 0.0, 100.0]  # Closed ring
    ], dtype=np.float64)

    # Offsets in POINTS (indices into the N rows)
    # 5 points.
    # We want one ring.
    # Note: polygonize usually takes multiple lines.
    # If we pass a single closed linestring, it should work if it forms a polygon.
    # We need to include the last point (index 4) to close the loop P3->P4.
    # Offsets define range [start, end). The loop runs for j in start..end-1.
    # To get segments P0-P1, P1-P2, P2-P3, P3-P4, we need j=0,1,2,3.
    # So end-1 = 4 => end = 5.
    offsets = np.array([0, 5], dtype=np.uint32)

    polys = polygonize(coords, offsets, return_polygons=True)
    print(f"Result count: {len(polys)}")
    assert len(polys) == 1

    shapely_poly = polys[0]
    print(f"Shapely area: {shapely_poly.area}")
    assert abs(shapely_poly.area - 100.0) < 1e-6
    print("3D coordinates test passed!")

def test_odd_length_coordinates():
    print("\nTesting odd length coordinates...")
    # Flat array with 3 elements (1.5 points)
    coords = np.array([0.0, 0.0, 1.0], dtype=np.float64)
    offsets = np.array([0], dtype=np.uint32)

    try:
        polygonize(coords, offsets)
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"Caught expected error: {e}")
        assert "Coordinates array length must be multiple of" in str(e)
    print("Odd length coordinates test passed!")

@patch('os.path.exists')
@patch('glob.glob')
def test_library_not_found(mock_glob, mock_exists):
    print("\nTesting missing shared library...")
    # Mock exists to always return False and glob to return empty list
    mock_exists.return_value = False
    mock_glob.return_value = []

    import sys
    if 'geo_polygonize.cffi_wrapper' in sys.modules:
        del sys.modules['geo_polygonize.cffi_wrapper']

    try:
        import geo_polygonize.cffi_wrapper as cffi_wrapper
        assert False, "Should have raised FileNotFoundError"
    except FileNotFoundError as e:
        print(f"Caught expected error: {e}")
        assert "Could not find geo_polygonize_core shared library" in str(e)
    print("Missing library test passed!")

@patch('geo_polygonize.cffi_wrapper.lib', create=True)
def test_invalid_input_status(mock_lib):
    print("\nTesting invalid input status (status == 1)...")
    import geo_polygonize.cffi_wrapper as cw

    # Mock the return values
    mock_lib.polygonize_ffi.return_value = "mock_res_ptr"
    mock_lib.polygonize_result_get_status.return_value = 1
    mock_lib.polygonize_result_free.return_value = None

    # Valid-looking arrays to pass python-side checks
    coords = np.array([0.0, 0.0, 1.0, 1.0], dtype=np.float64)
    offsets = np.array([0, 2], dtype=np.uint32)

    try:
        cw.polygonize(coords, offsets)
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"Caught expected error: {e}")
        assert "Invalid input provided to polygonize" in str(e)
    print("Invalid input status test passed!")

@patch('geo_polygonize.cffi_wrapper.lib', create=True)
def test_internal_error_status(mock_lib):
    print("\nTesting internal error status (status == 2)...")
    import geo_polygonize.cffi_wrapper as cw

    # Mock the return values
    mock_lib.polygonize_ffi.return_value = "mock_res_ptr"
    mock_lib.polygonize_result_get_status.return_value = 2
    mock_lib.polygonize_result_free.return_value = None

    coords = np.array([0.0, 0.0, 1.0, 1.0], dtype=np.float64)
    offsets = np.array([0, 2], dtype=np.uint32)

    try:
        cw.polygonize(coords, offsets)
        assert False, "Should have raised RuntimeError"
    except RuntimeError as e:
        print(f"Caught expected error: {e}")
        assert "Internal error during polygonization: 2" in str(e)
    print("Internal error status test passed!")


if __name__ == "__main__":
    test_square()
    test_two_squares()
    test_square_with_hole()
    test_3d_coordinates()
    test_odd_length_coordinates()
    test_library_not_found()
    test_invalid_input_status()
    test_internal_error_status()
