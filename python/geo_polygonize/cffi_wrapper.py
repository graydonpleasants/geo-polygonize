import os
import glob
import cffi
import numpy as np
from .types import SimplePolygon

ffi = cffi.FFI()
ffi.cdef("""
    typedef struct { bool node_input; double snap_grid_size; bool extract_only_polygonal; } PolygonizerOptions;
    typedef struct CPolygonResult CPolygonResult;

    void polygonize_result_free(CPolygonResult* res);

    CPolygonResult* polygonize_ffi(
        const double* coords, size_t coords_len,
        const uint32_t* offsets, size_t offsets_len,
        uint8_t stride,
        PolygonizerOptions options
    );

    int polygonize_result_get_status(const CPolygonResult* res);
    uint8_t polygonize_result_get_stride(const CPolygonResult* res);
    size_t polygonize_result_get_flat_coords_len(const CPolygonResult* res);
    void polygonize_result_copy_flat_coords(const CPolygonResult* res, double* buffer);
    size_t polygonize_result_get_ring_offsets_len(const CPolygonResult* res);
    void polygonize_result_copy_ring_offsets(const CPolygonResult* res, uint32_t* buffer);
    size_t polygonize_result_get_polygon_offsets_len(const CPolygonResult* res);
    void polygonize_result_copy_polygon_offsets(const CPolygonResult* res, uint32_t* buffer);
""")


def find_library():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    possible_names = ["libgeo_polygonize_core.so", "geo_polygonize_core.dll", "libgeo_polygonize_core.dylib"]
    for name in possible_names:
        path = os.path.join(base_dir, name)
        if os.path.exists(path):
            return path
    for name in possible_names:
        path = os.path.join(base_dir, "../../target/release", name)
        if os.path.exists(path):
            return path
    for ext in ["*.so", "*.pyd", "*.dylib"]:
        pattern = os.path.join(base_dir, "geo_polygonize_core" + ext)
        matches = glob.glob(pattern)
        if matches:
            return matches[0]
    raise FileNotFoundError("Could not find geo_polygonize_core shared library")


lib = ffi.dlopen(find_library())


def polygonize(coords_array: np.ndarray, offsets_array: np.ndarray, node: bool = False, snap: float = 1e-10, extract_only_polygonal: bool = False, stride: int = 2):
    coords = np.ascontiguousarray(coords_array, dtype=np.float64)
    offsets = np.ascontiguousarray(offsets_array, dtype=np.uint32)

    if coords.ndim == 2:
        coords = coords.ravel()

    if stride not in (2, 3):
        raise ValueError("stride must be 2 (XY) or 3 (XYZ)")

    coords_ptr = ffi.cast("double*", coords.ctypes.data)
    offsets_ptr = ffi.cast("uint32_t*", offsets.ctypes.data)
    options = {'node_input': node, 'snap_grid_size': snap, 'extract_only_polygonal': extract_only_polygonal}

    res_ptr = lib.polygonize_ffi(coords_ptr, coords.size, offsets_ptr, offsets.size, stride, options)
    if res_ptr == ffi.NULL:
        raise RuntimeError("Polygonization failed (returned NULL)")

    try:
        status = lib.polygonize_result_get_status(res_ptr)
        if status != 0:
            if status == 1:
                raise ValueError("Invalid input provided to polygonize")
            raise RuntimeError("Internal error during polygonization")

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
        for p_idx, ring_start in enumerate(poly_offsets):
            ring_end = poly_offsets[p_idx + 1] if p_idx + 1 < len(poly_offsets) else len(ring_offsets)
            shell = None
            holes = []
            for r in range(ring_start, ring_end):
                point_start = ring_offsets[r]
                point_end = ring_offsets[r + 1] if r + 1 < len(ring_offsets) else (len(flat) // out_stride)
                ring = flat[point_start * out_stride: point_end * out_stride].reshape(-1, out_stride)
                coords_tuples = tuple(map(tuple, ring.tolist()))
                if shell is None:
                    shell = coords_tuples
                else:
                    holes.append(coords_tuples)
            polygons.append(SimplePolygon(shell or tuple(), holes))

        return {'polygons': polygons, 'flat_coords': flat, 'ring_offsets': ring_offsets, 'polygon_offsets': poly_offsets, 'stride': int(out_stride)}
    finally:
        lib.polygonize_result_free(res_ptr)
