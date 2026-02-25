from .types import SimplePolygon
import numpy as np

try:
    from .geo_polygonize_core import polygonize as _polygonize_impl
except ImportError:
    from .cffi_wrapper import polygonize as _polygonize_impl

def polygonize(coords, offsets, node=False, snap=1e-10, extract_only_polygonal=False):
    """
    Polygonize a set of lines.

    Args:
        coords: float64 array of shape (N, 2) or (N, 3) or flattened.
        offsets: uint32 array of start indices in coords.
        node: whether to node the input.
        snap: snap grid size.
        extract_only_polygonal: whether to extract only disjoint, outer-most polygonal shells.

    Returns:
        Dict with keys 'polygons' (List[SimplePolygon]) and 'dangles' (List[tuple of coords]).
    """
    # Ensure coords is a numpy array
    coords = np.ascontiguousarray(coords, dtype=np.float64)

    # Handle (N, 3) case: slice out Z
    if coords.ndim == 2 and coords.shape[1] == 3:
        coords = coords[:, :2]
        coords = np.ascontiguousarray(coords, dtype=np.float64)

    # Flatten if 2D (N, 2)
    if coords.ndim == 2:
        coords = coords.ravel()

    # Check for odd length (must be pairs of XY)
    if coords.size % 2 != 0:
        raise ValueError("Coordinates array must have an even number of elements (XY pairs).")

    return _polygonize_impl(coords, offsets, node=node, snap=snap, extract_only_polygonal=extract_only_polygonal)
