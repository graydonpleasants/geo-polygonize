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
        const uint32_t* offsets, size_t offsets_len,
        const uint32_t* line_ids, size_t line_ids_len,
        uint8_t stride,
        const PolygonizerOptions* options
    );

    int polygonize_result_get_status(const CPolygonResult* res);

    uint8_t polygonize_result_get_stride(const CPolygonResult* res);
    size_t polygonize_result_get_flat_coords_len(const CPolygonResult* res);
    void polygonize_result_copy_flat_coords(const CPolygonResult* res, double* buffer);

    size_t polygonize_result_get_ring_offsets_len(const CPolygonResult* res);
    void polygonize_result_copy_ring_offsets(const CPolygonResult* res, uint32_t* buffer);

    size_t polygonize_result_get_polygon_offsets_len(const CPolygonResult* res);
    void polygonize_result_copy_polygon_offsets(const CPolygonResult* res, uint32_t* buffer);

    size_t polygonize_result_get_flat_line_ids_len(const CPolygonResult* res);
    void polygonize_result_copy_flat_line_ids(const CPolygonResult* res, uint32_t* buffer);

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

def polygonize(coords_array: np.ndarray, offsets_array: np.ndarray, node: bool = False, snap: float = 1e-10, extract_only_polygonal: bool = False, stride: int = 2, line_ids: np.ndarray = None):
    """
    Polygonize a set of lines.

    Args:
        coords_array: contiguous float64 array of shape (N, stride) or flattened.
        offsets_array: contiguous uint32 array of start indices in coords_array.
                       Indices refer to points (tuples of doubles), NOT individual doubles.
        node: whether to node the input.
        snap: snap grid size.
        extract_only_polygonal: whether to extract only disjoint, outer-most polygonal shells.
        stride: 2 for XY, 3 for XYZ.
        line_ids: optional contiguous uint32 array of line IDs (length = num_linestrings).

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

    line_ids_ptr = ffi.NULL
    line_ids_len = 0
    if line_ids is not None:
        line_ids = np.ascontiguousarray(line_ids, dtype=np.uint32)
        line_ids_ptr = ffi.cast("uint32_t*", line_ids.ctypes.data)
        line_ids_len = line_ids.size

    options_val = {
        'node_input': 1 if node else 0,
        'snap_grid_size': snap,
        'extract_only_polygonal': 1 if extract_only_polygonal else 0
    }
    options_ptr = ffi.new("PolygonizerOptions*", options_val)

    res_ptr = lib.polygonize_ffi(
        coords_ptr, coords.size,
        offsets_ptr, offsets.size,
        line_ids_ptr, line_ids_len,
        stride,
        options_ptr
    )

    # To use our defined error types
    # Since CFFI wrapper is a fallback, we need to import these classes dynamically.
    # To avoid circular imports, we assume they are defined in __init__.py when CFFI fallback happens.
    import sys
    if 'geo_polygonize' in sys.modules:
        PolygonizeTypeError = sys.modules['geo_polygonize'].PolygonizeTypeError
        PolygonizeTopologyError = sys.modules['geo_polygonize'].PolygonizeTopologyError
    else:
        PolygonizeTypeError = ValueError
        PolygonizeTopologyError = RuntimeError

    if res_ptr == ffi.NULL:
        raise PolygonizeTopologyError("Polygonization failed (returned NULL)")

    try:
        # Note: if this C library no longer exports `polygonize_result_get_status`,
        # this will fail gracefully or we can just catch the attribute error.
        try:
            status = lib.polygonize_result_get_status(res_ptr)
            if status != 0:
                 if status == 1:
                     raise PolygonizeTypeError("Invalid input provided to polygonize")
                 else:
                     raise PolygonizeTopologyError(f"Internal error during polygonization: {status}")
        except AttributeError:
            pass

        out_stride = lib.polygonize_result_get_stride(res_ptr)
        flat_len = lib.polygonize_result_get_flat_coords_len(res_ptr)
        flat = np.zeros(flat_len, dtype=np.float64)
        lib.polygonize_result_copy_flat_coords(res_ptr, ffi.cast("double*", flat.ctypes.data))

        ring_len = lib.polygonize_result_get_ring_offsets_len(res_ptr)
        ring_offsets = np.zeros(ring_len, dtype=np.uint32)
        lib.polygonize_result_copy_ring_offsets(res_ptr, ffi.cast("uint32_t*", ring_offsets.ctypes.data))

        poly_len = lib.polygonize_result_get_polygon_offsets_len(res_ptr)
        poly_offsets = np.zeros(poly_len, dtype=np.uint32)
        lib.polygonize_result_copy_polygon_offsets(res_ptr, ffi.cast("uint32_t*", poly_offsets.ctypes.data))

        line_ids_out_len = lib.polygonize_result_get_flat_line_ids_len(res_ptr)
        flat_line_ids = np.zeros(line_ids_out_len, dtype=np.uint32)
        lib.polygonize_result_copy_flat_line_ids(res_ptr, ffi.cast("uint32_t*", flat_line_ids.ctypes.data))

        polygons = []
        for p_idx in range(len(poly_offsets)):
            ring_start = poly_offsets[p_idx]
            ring_end = poly_offsets[p_idx+1] if p_idx + 1 < len(poly_offsets) else len(ring_offsets)

            shell = None
            shell_ids = None
            holes = []
            holes_ids = []

            for r in range(ring_start, ring_end):
                point_start = ring_offsets[r]
                point_end = ring_offsets[r+1] if r + 1 < len(ring_offsets) else (len(flat) // out_stride)

                # Use .copy() to ensure SimplePolygon owns its data,
                # as flat is a view into C-allocated memory that will be freed.
                ring = flat[point_start*out_stride : point_end*out_stride].reshape(-1, out_stride).copy()
                coords_data = ring

                # Extract IDs
                r_ids = flat_line_ids[point_start:point_end].copy()
                ids_data = r_ids

                if shell is None:
                    shell = coords_data
                    shell_ids = ids_data
                else:
                    holes.append(coords_data)
                    holes_ids.append(ids_data)

            polygons.append(SimplePolygon(shell if shell is not None else np.empty((0, out_stride)), holes, shell_ids, holes_ids))

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
            coords = buffer.reshape(-1, 3)
            if stride == 2:
                coords = coords[:, :2]
            # Keep dangles as tuples for backward compatibility (breaking change avoided)
            dangles.append(tuple(map(tuple, coords.tolist())))

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
            coords = buffer.reshape(-1, 3)
            if stride == 2:
                coords = coords[:, :2]
            # Keep invalid_rings as tuples for backward compatibility
            invalid_rings.append(tuple(map(tuple, coords.tolist())))

        return {
            'polygons': polygons,
            'flat_coords': flat,
            'ring_offsets': ring_offsets,
            'polygon_offsets': poly_offsets,
            'flat_line_ids': flat_line_ids,
            'stride': int(out_stride),
            'dangles': dangles,
            'invalid_rings': invalid_rings
        }

    finally:
        lib.polygonize_result_free(res_ptr)
