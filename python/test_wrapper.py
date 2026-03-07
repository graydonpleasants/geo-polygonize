import pytest
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

def test_missing_args():
    print("\nTesting missing arguments...")
    coords = np.array([0.0, 0.0, 1.0, 1.0], dtype=np.float64)
    offsets = np.array([0, 2], dtype=np.uint32)

    with pytest.raises(ValueError, match="Either 'lines' or both 'coords' and 'offsets' must be provided."):
        polygonize()

    with pytest.raises(ValueError, match="Either 'lines' or both 'coords' and 'offsets' must be provided."):
        polygonize(coords=coords)

    with pytest.raises(ValueError, match="Either 'lines' or both 'coords' and 'offsets' must be provided."):
        polygonize(offsets=offsets)

    print("Missing arguments test passed!")

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

def test_import_error_fallback():
    print("\nTesting ImportError fallback in __init__.py...")
    import importlib
    import builtins
    import geo_polygonize

    real_import = builtins.__import__

    def mock_import(name, globals=None, locals=None, fromlist=(), level=0):
        if 'geo_polygonize_core' in name or (fromlist and 'geo_polygonize_core' in fromlist):
            raise ImportError("Mocked ImportError for testing")
        return real_import(name, globals, locals, fromlist, level)

    with patch('builtins.__import__', side_effect=mock_import):
        importlib.reload(geo_polygonize)

        import geo_polygonize.cffi_wrapper as cffi_wrapper
        assert geo_polygonize._polygonize_impl is cffi_wrapper.polygonize
        print("ImportError fallback test passed!")

    # Reload again to restore the module state for other tests
    importlib.reload(geo_polygonize.cffi_wrapper)
    importlib.reload(geo_polygonize)

def test_return_polygons_without_shapely():
    print("\nTesting ImportError fallback in return_polygons...")
    coords = np.array([
        0.0, 0.0, 10.0, 0.0,
        10.0, 0.0, 10.0, 10.0,
        10.0, 10.0, 0.0, 10.0,
        0.0, 10.0, 0.0, 0.0
    ], dtype=np.float64)
    offsets = np.array([0, 2, 4, 6, 8], dtype=np.uint32)

    # Mock shapely.geometry not being available
    import sys
    import builtins
    import unittest.mock

    real_import = builtins.__import__
    def mock_import(name, globals=None, locals=None, fromlist=(), level=0):
        if name == 'shapely.geometry' or name == 'shapely':
            raise ImportError("Mocked ImportError")
        return real_import(name, globals, locals, fromlist, level)

    with unittest.mock.patch('builtins.__import__', side_effect=mock_import):
        try:
            polygonize(coords, offsets, return_polygons=True)
            assert False, "Should have raised ImportError"
        except ImportError as e:
            print(f"Caught expected error: {e}")
            assert "return_polygons=True requires 'shapely' to be installed." in str(e)
    print("ImportError fallback test passed!")
    
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

def test_invalid_shape_mismatch():
    print("\nTesting invalid shape mismatch...")
    # 2D coords with shape (1, 4) but stride=2
    coords = np.array([[0.0, 0.0, 1.0, 1.0]], dtype=np.float64)
    offsets = np.array([0], dtype=np.uint32)

    with pytest.raises(ValueError, match=r"Input shape \(1, 4\) does not match stride 2"):
        polygonize(coords, offsets, stride=2)
    print("Invalid shape mismatch test passed!")

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
def test_null_result_pointer(mock_lib):
    print("\nTesting null result pointer...")
    import geo_polygonize.cffi_wrapper as cw

    # Mock the return values
    mock_lib.polygonize_ffi.return_value = cw.ffi.NULL

    coords = np.array([0.0, 0.0, 1.0, 1.0], dtype=np.float64)
    offsets = np.array([0, 2], dtype=np.uint32)

    try:
        cw.polygonize(coords, offsets)
        assert False, "Should have raised RuntimeError"
    except RuntimeError as e:
        print(f"Caught expected error: {e}")
        assert "Polygonization failed (returned NULL)" in str(e)
    print("Null result pointer test passed!")

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
    test_missing_args()
    test_odd_length_coordinates()
    test_import_error_fallback()
    test_return_polygons_without_shapely()
    test_library_not_found()
    test_invalid_shape_mismatch()
    test_invalid_input_status()
    test_internal_error_status()
    test_null_result_pointer()
