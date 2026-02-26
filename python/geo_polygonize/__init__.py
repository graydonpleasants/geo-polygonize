from .types import SimplePolygon
import numpy as np

try:
    from .geo_polygonize_core import polygonize as _polygonize_impl
except ImportError:
    from .cffi_wrapper import polygonize as _polygonize_impl


def polygonize(coords, offsets, node=False, snap=1e-10, extract_only_polygonal=False, stride=None):
    coords = np.ascontiguousarray(coords, dtype=np.float64)

    if coords.ndim == 2:
        if stride is None:
            stride = coords.shape[1]
        coords = coords.ravel()

    if stride is None:
        stride = 2

    if stride not in (2, 3):
        raise ValueError("stride must be 2 or 3")

    if coords.size % stride != 0:
        raise ValueError("Coordinates length must be divisible by stride")

    return _polygonize_impl(
        coords,
        offsets,
        node=node,
        snap=snap,
        extract_only_polygonal=extract_only_polygonal,
        stride=stride,
    )
