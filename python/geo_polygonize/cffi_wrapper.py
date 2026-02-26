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

def polygonize(coords_array: np.ndarray, offsets_array: np.ndarray, node: bool = False, snap: float = 1e-10, extract_only_polygonal: bool = False, stride: int = 2):
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
        offsets_ptr, offsets.size,
        stride,
        options_ptr
    )

    if res_ptr == ffi.NULL:
        raise RuntimeError("Polygonization failed (returned NULL)")

    try:
        status = lib.polygonize_result_get_status(res_ptr)
        if status != 0:
             if status == 1:
                 raise ValueError("Invalid input provided to polygonize")
             else:
                 raise RuntimeError(f"Internal error during polygonization: {status}")

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

        polygons = []
        for p_idx in range(len(poly_offsets)):
            ring_start = poly_offsets[p_idx]
            ring_end = poly_offsets[p_idx+1] if p_idx + 1 < len(poly_offsets) else len(ring_offsets)

            shell = None
            holes = []
            for r in range(ring_start, ring_end):
                point_start = ring_offsets[r]
                point_end = ring_offsets[r+1] if r + 1 < len(ring_offsets) else (len(flat) // out_stride)

                ring = flat[point_start*out_stride : point_end*out_stride].reshape(-1, out_stride)
                coords_tuples = tuple(map(tuple, ring.tolist()))

                if shell is None:
                    shell = coords_tuples
                else:
                    holes.append(coords_tuples)

            polygons.append(SimplePolygon(shell or tuple(), holes))

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
            # Reshape based on stride?
            # Rust side `polygonize_result_get_dangle_points` writes 3 coords (3D) always?
            # Let's check ffi.rs. Yes, it writes 3 doubles per point.
            # But python `stride` might be 2.
            # If stride=2, we should probably slice?
            # Or should we respect the output stride?
            # The prompt said "Adopt the Coord3D ... and expose it seamlessly to Python ... via interleaved flat arrays."
            # But dangles are handled via separate accessors in this implementation.
            # If I use `stride=2`, do I expect 2D output?
            # The FFI `get_dangle_points` hardcodes 3 doubles per point.
            # So I should reshape to (-1, 3).
            # And then if stride is 2, slice?
            # Or should FFI respect stride?
            # The previous FFI implementation I wrote for `get_dangle_points` used `slice::from_raw_parts_mut(buffer, dangle.len() * 3)`.
            # So it always outputs 3D.
            # Python side should adapt.
            coords = buffer.reshape(-1, 3)
            if stride == 2:
                coords = coords[:, :2]
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
            invalid_rings.append(tuple(map(tuple, coords.tolist())))

        return {'polygons': polygons, 'flat_coords': flat, 'ring_offsets': ring_offsets, 'polygon_offsets': poly_offsets, 'stride': int(out_stride), 'dangles': dangles, 'invalid_rings': invalid_rings}

    finally:
        lib.polygonize_result_free(res_ptr)
