import os
import glob
import cffi
import numpy as np
from .types import SimplePolygon

ffi = cffi.FFI()
ffi.cdef("""
    typedef struct { uint8_t node_input; double snap_grid_size; uint8_t extract_only_polygonal; } PolygonizerOptions;
    typedef struct CPolygonResult CPolygonResult;

    void polygonize_result_free(CPolygonResult* res);

    CPolygonResult* polygonize_ffi(
        const double* coords, size_t coords_len,
        uint8_t stride,
        const uint32_t* offsets, size_t offsets_len,
        const PolygonizerOptions* options
    );

    int polygonize_result_get_status(const CPolygonResult* res);
    size_t polygonize_result_get_count(const CPolygonResult* res);

    size_t polygonize_result_get_shell_point_count(const CPolygonResult* res, size_t poly_idx);
    void polygonize_result_get_shell_points(const CPolygonResult* res, size_t poly_idx, double* buffer);

    size_t polygonize_result_get_hole_count(const CPolygonResult* res, size_t poly_idx);
    size_t polygonize_result_get_hole_point_count(const CPolygonResult* res, size_t poly_idx, size_t hole_idx);
    void polygonize_result_get_hole_points(const CPolygonResult* res, size_t poly_idx, size_t hole_idx, double* buffer);

    size_t polygonize_result_get_dangle_count(const CPolygonResult* res);
    size_t polygonize_result_get_dangle_point_count(const CPolygonResult* res, size_t dangle_idx);
    void polygonize_result_get_dangle_points(const CPolygonResult* res, size_t dangle_idx, double* buffer);

    size_t polygonize_result_get_invalid_ring_count(const CPolygonResult* res);
    size_t polygonize_result_get_invalid_ring_point_count(const CPolygonResult* res, size_t ring_idx);
    void polygonize_result_get_invalid_ring_points(const CPolygonResult* res, size_t ring_idx, double* buffer);
""")

# Locate the library
def find_library():
    base_dir = os.path.dirname(os.path.abspath(__file__))

    possible_names = [
        "libgeo_polygonize_core.so",
        "geo_polygonize_core.dll",
        "libgeo_polygonize_core.dylib",
    ]

    # 1. Check in the package directory (for installed wheel)
    for name in possible_names:
        path = os.path.join(base_dir, name)
        if os.path.exists(path):
            return path

    # 2. Check in development build directory
    for name in possible_names:
        path = os.path.join(base_dir, "../../target/release", name)
        if os.path.exists(path):
            return path

    # 3. Check for PyO3 extension module (as shared lib)
    extensions = ["*.so", "*.pyd", "*.dylib"]
    for ext in extensions:
        pattern = os.path.join(base_dir, "geo_polygonize_core" + ext)
        matches = glob.glob(pattern)
        if matches:
            return matches[0]

    raise FileNotFoundError("Could not find geo_polygonize_core shared library")

lib_path = find_library()
lib = ffi.dlopen(lib_path)

def polygonize(coords_array: np.ndarray, offsets_array: np.ndarray, node: bool = False, snap: float = 1e-10, extract_only_polygonal: bool = False, stride: int = 2):
    """
    Polygonize a set of lines.

    Args:
        coords_array: contiguous float64 array of shape (N, 2) or flattened (2*N,).
        offsets_array: contiguous uint32 array of start indices in coords_array.
                       Indices refer to points (tuples of doubles), NOT individual doubles.
        node: whether to node the input.
        snap: snap grid size.
        extract_only_polygonal: whether to extract only disjoint, outer-most polygonal shells.
        stride: 2 for XY, 3 for XYZ.

    Returns:
        Dict with 'polygons' (List[SimplePolygon]), 'dangles', and 'invalid_rings'.
    """
    # Ensure contiguous C-order arrays
    coords = np.ascontiguousarray(coords_array, dtype=np.float64)
    offsets = np.ascontiguousarray(offsets_array, dtype=np.uint32)

    if coords.ndim == 2:
        coords = coords.ravel()

    coords_ptr = ffi.cast("double*", coords.ctypes.data)
    offsets_ptr = ffi.cast("uint32_t*", offsets.ctypes.data)

    options_val = {
        'node_input': 1 if node else 0,
        'snap_grid_size': snap,
        'extract_only_polygonal': 1 if extract_only_polygonal else 0
    }
    options_ptr = ffi.new("PolygonizerOptions*", options_val)

    res_ptr = lib.polygonize_ffi(
        coords_ptr, coords.size,
        stride,
        offsets_ptr, offsets.size,
        options_ptr
    )

    if res_ptr == ffi.NULL:
        raise RuntimeError("Polygonization failed (returned NULL)")

    try:
        status = lib.polygonize_result_get_status(res_ptr)
        if status != 0:
             if status == 1:
                 raise ValueError("Invalid input provided to polygonize")
             elif status == 2:
                 raise RuntimeError("Internal error during polygonization")
             else:
                 raise RuntimeError(f"Unknown error status: {status}")

        count = lib.polygonize_result_get_count(res_ptr)
        polygons = []

        for i in range(count):
            # Shell (3D)
            shell_pts_count = lib.polygonize_result_get_shell_point_count(res_ptr, i)
            shell_buffer = np.zeros(shell_pts_count * 3, dtype=np.float64)
            lib.polygonize_result_get_shell_points(
                res_ptr, i,
                ffi.cast("double*", shell_buffer.ctypes.data)
            )
            shell_coords = tuple(map(tuple, shell_buffer.reshape(-1, 3).tolist()))

            # Holes
            hole_count = lib.polygonize_result_get_hole_count(res_ptr, i)
            holes = []
            for j in range(hole_count):
                hole_pts_count = lib.polygonize_result_get_hole_point_count(res_ptr, i, j)
                hole_buffer = np.zeros(hole_pts_count * 3, dtype=np.float64)
                lib.polygonize_result_get_hole_points(
                    res_ptr, i, j,
                    ffi.cast("double*", hole_buffer.ctypes.data)
                )
                hole_coords = tuple(map(tuple, hole_buffer.reshape(-1, 3).tolist()))
                holes.append(hole_coords)

            polygons.append(SimplePolygon(shell_coords, holes))

        # Dangles
        dangle_count = lib.polygonize_result_get_dangle_count(res_ptr)
        dangles = []
        for i in range(dangle_count):
            pts_count = lib.polygonize_result_get_dangle_point_count(res_ptr, i)
            buffer = np.zeros(pts_count * 3, dtype=np.float64)
            lib.polygonize_result_get_dangle_points(
                res_ptr, i,
                ffi.cast("double*", buffer.ctypes.data)
            )
            coords = tuple(map(tuple, buffer.reshape(-1, 3).tolist()))
            dangles.append(coords)

        # Invalid Rings
        invalid_count = lib.polygonize_result_get_invalid_ring_count(res_ptr)
        invalid_rings = []
        for i in range(invalid_count):
            pts_count = lib.polygonize_result_get_invalid_ring_point_count(res_ptr, i)
            buffer = np.zeros(pts_count * 3, dtype=np.float64)
            lib.polygonize_result_get_invalid_ring_points(
                res_ptr, i,
                ffi.cast("double*", buffer.ctypes.data)
            )
            coords = tuple(map(tuple, buffer.reshape(-1, 3).tolist()))
            invalid_rings.append(coords)

        return {'polygons': polygons, 'dangles': dangles, 'invalid_rings': invalid_rings}

    finally:
        lib.polygonize_result_free(res_ptr)
