import os
import glob
import cffi
import numpy as np
from .types import SimplePolygon

ffi = cffi.FFI()
ffi.cdef("""
    typedef struct { bool node_input; double snap_grid_size; } PolygonizerOptions;
    typedef struct CPolygonResult CPolygonResult;

    void polygonize_result_free(CPolygonResult* res);

    CPolygonResult* polygonize_ffi(
        const double* coords, size_t coords_len,
        const uint32_t* offsets, size_t offsets_len,
        PolygonizerOptions options
    );

    int polygonize_result_get_status(const CPolygonResult* res);
    size_t polygonize_result_get_count(const CPolygonResult* res);

    size_t polygonize_result_get_shell_point_count(const CPolygonResult* res, size_t poly_idx);
    void polygonize_result_get_shell_points(const CPolygonResult* res, size_t poly_idx, double* buffer);

    size_t polygonize_result_get_hole_count(const CPolygonResult* res, size_t poly_idx);
    size_t polygonize_result_get_hole_point_count(const CPolygonResult* res, size_t poly_idx, size_t hole_idx);
    void polygonize_result_get_hole_points(const CPolygonResult* res, size_t poly_idx, size_t hole_idx, double* buffer);
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
    # Path: ../../target/release/
    # This assumes we are running from python/geo_polygonize/
    # So ../../ reaches python/.. which is repo root.
    # Wait, python/geo_polygonize/../.. => python/.. => root.
    # Then target/release/ => correct.
    for name in possible_names:
        path = os.path.join(base_dir, "../../target/release", name)
        if os.path.exists(path):
            return path

    # 3. Check for PyO3 extension module (as shared lib)
    # The extension name is geo_polygonize_core.
    # It might have a suffix like .cpython-39-x86_64-linux-gnu.so or .abi3.so
    extensions = ["*.so", "*.pyd", "*.dylib"]
    for ext in extensions:
        pattern = os.path.join(base_dir, "geo_polygonize_core" + ext)
        matches = glob.glob(pattern)
        if matches:
            return matches[0]

    raise FileNotFoundError("Could not find geo_polygonize_core shared library")

lib_path = find_library()
lib = ffi.dlopen(lib_path)

def polygonize(coords_array: np.ndarray, offsets_array: np.ndarray, node: bool = False, snap: float = 1e-10):
    """
    Polygonize a set of lines.

    Args:
        coords_array: contiguous float64 array of shape (N, 2) or flattened (2*N,).
        offsets_array: contiguous uint32 array of start indices in coords_array.
                       Indices refer to points (pairs of doubles), NOT individual doubles.
                       E.g., if coords_array has 4 points (8 doubles), offsets=[0, 2]
                       means first line starts at point 0, second at point 2.
        node: whether to node the input.
        snap: snap grid size.

    Returns:
        List of SimplePolygon objects.
    """
    # Ensure contiguous C-order arrays
    coords = np.ascontiguousarray(coords_array, dtype=np.float64)
    offsets = np.ascontiguousarray(offsets_array, dtype=np.uint32)

    if coords.ndim == 2:
        coords = coords.ravel()

    coords_ptr = ffi.cast("double*", coords.ctypes.data)
    offsets_ptr = ffi.cast("uint32_t*", offsets.ctypes.data)

    options = {'node_input': node, 'snap_grid_size': snap}

    res_ptr = lib.polygonize_ffi(
        coords_ptr, coords.size,
        offsets_ptr, offsets.size,
        options
    )

    if res_ptr == ffi.NULL:
        raise RuntimeError("Polygonization failed (returned NULL)")

    try:
        status = lib.polygonize_result_get_status(res_ptr)
        if status != 0:
             # 0 = Success, 1 = InvalidInput, 2 = InternalError
             if status == 1:
                 raise ValueError("Invalid input provided to polygonize")
             elif status == 2:
                 raise RuntimeError("Internal error during polygonization")
             else:
                 raise RuntimeError(f"Unknown error status: {status}")

        count = lib.polygonize_result_get_count(res_ptr)
        polygons = []

        for i in range(count):
            # Shell
            shell_pts_count = lib.polygonize_result_get_shell_point_count(res_ptr, i)
            shell_buffer = np.zeros(shell_pts_count * 2, dtype=np.float64)
            lib.polygonize_result_get_shell_points(
                res_ptr, i,
                ffi.cast("double*", shell_buffer.ctypes.data)
            )
            shell_coords = tuple(map(tuple, shell_buffer.reshape(-1, 2).tolist()))

            # Holes
            hole_count = lib.polygonize_result_get_hole_count(res_ptr, i)
            holes = []
            for j in range(hole_count):
                hole_pts_count = lib.polygonize_result_get_hole_point_count(res_ptr, i, j)
                hole_buffer = np.zeros(hole_pts_count * 2, dtype=np.float64)
                lib.polygonize_result_get_hole_points(
                    res_ptr, i, j,
                    ffi.cast("double*", hole_buffer.ctypes.data)
                )
                hole_coords = tuple(map(tuple, hole_buffer.reshape(-1, 2).tolist()))
                holes.append(hole_coords)

            polygons.append(SimplePolygon(shell_coords, holes))

        return polygons

    finally:
        lib.polygonize_result_free(res_ptr)
