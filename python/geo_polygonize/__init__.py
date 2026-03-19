from .types import SimplePolygon
import numpy as np

import json

try:
    from .geo_polygonize_core import polygonize as _polygonize_impl
    from .geo_polygonize_core import polygonize_with_options as _polygonize_with_options_impl
except ImportError:
    from .cffi_wrapper import polygonize as _polygonize_impl

def polygonize_with_options(lines=None, coords=None, offsets=None, options=None, stride=None, line_ids=None, return_polygons=False):
    """
    Polygonize a set of lines with full canonical options.

    Args:
        lines: iterable of LineStrings (e.g. Shapely LineStrings) or coordinate lists. If provided, overrides coords and offsets.
        coords: float64 array of shape (N, 2) or (N, 3) or flattened.
        offsets: uint32 array of start indices in coords.
        options: A dictionary matching the `PolygonizerOptions` schema.
        stride: stride of coordinates (2 or 3). If None, defaults to 2 unless coords is (N, 3).
        line_ids: optional uint32 array of line IDs.
        return_polygons: if True, returns Shapely Polygon objects instead of a dictionary.
    """
    options_dict = options or {}

    if lines is not None:
        flat_coords = []
        parsed_offsets = []
        current_offset = 0

        for line in lines:
            if hasattr(line, 'coords'):
                pts = list(line.coords)
            else:
                pts = line

            if not pts:
                continue

            parsed_offsets.append(current_offset)

            if stride is None:
                stride = len(pts[0])

            for pt in pts:
                flat_coords.extend(pt[:stride])
                current_offset += 1

        coords = np.array(flat_coords, dtype=np.float64)
        offsets = np.array(parsed_offsets, dtype=np.uint32)

    elif coords is None or offsets is None:
        raise ValueError("Either 'lines' or both 'coords' and 'offsets' must be provided.")

    coords = np.ascontiguousarray(coords, dtype=np.float64)
    offsets = np.ascontiguousarray(offsets, dtype=np.uint32)

    if stride is None:
        if coords.ndim == 2:
            stride = coords.shape[1]
        else:
            stride = 2

    if stride not in (2, 3):
        raise ValueError("stride must be 2 or 3")

    if coords.ndim == 2:
        if coords.shape[1] != stride:
             if stride == 2 and coords.shape[1] == 3:
                 coords = coords[:, :2]
                 coords = np.ascontiguousarray(coords, dtype=np.float64)
             else:
                 raise ValueError(f"Input shape {coords.shape} does not match stride {stride}")
        coords = coords.ravel()

    if coords.size % stride != 0:
        raise ValueError(f"Coordinates array length must be multiple of {stride}.")

    if line_ids is not None:
        line_ids = np.ascontiguousarray(line_ids, dtype=np.uint32)

    options_json = None if options is None else json.dumps(options)

    try:
        result = _polygonize_with_options_impl(coords, offsets, stride=stride, options_json=options_json, line_ids=line_ids)
    except NameError:
        # Fallback for CFFI which does not support options mapping fully yet
        result = _polygonize_impl(coords, offsets, node=options_dict.get("node_input", False), snap=options_dict.get("snap_grid_size", 1e-10), extract_only_polygonal=options_dict.get("extract_only_polygonal", False), stride=stride, line_ids=line_ids)

    if return_polygons:
        try:
            from shapely.geometry import Polygon
        except ImportError:
            raise ImportError("return_polygons=True requires 'shapely' to be installed.")

        shapely_polys = []
        if 'polygons' in result:
            for sp in result['polygons']:
                poly = Polygon(sp.shell, sp.holes)
                if hasattr(sp, 'provenance') and sp.provenance is not None:
                    # Shapely Polygon objects do not support arbitrary attributes,
                    # so we don't try to attach it directly to the C-extension object if it fails.
                    # Or we can just leave it up to the caller to use `return_polygons=False`.
                    pass
                shapely_polys.append(poly)

        return shapely_polys

    return result

def polygonize(coords=None, offsets=None, lines=None, node=False, snap=1e-10, extract_only_polygonal=False, stride=None, line_ids=None, return_polygons=False):
    """
    Polygonize a set of lines.

    Args:
        coords: float64 array of shape (N, 2) or (N, 3) or flattened.
        offsets: uint32 array of start indices in coords.
        lines: iterable of LineStrings (e.g. Shapely LineStrings) or coordinate lists. If provided, overrides coords and offsets.
        node: whether to node the input.
        snap: snap grid size.
        extract_only_polygonal: whether to extract only disjoint, outer-most polygonal shells.
        stride: stride of coordinates (2 or 3). If None, defaults to 2 unless coords is (N, 3).
        line_ids: optional uint32 array of line IDs.
        return_polygons: if True, returns Shapely Polygon objects instead of a dictionary.

    Returns:
        Dict with keys 'polygons' (List[SimplePolygon]), 'dangles', 'invalid_rings', and 'flat_line_ids'.
        If return_polygons=True, returns a List of shapely.geometry.Polygon.
    """
    if lines is not None:
        flat_coords = []
        parsed_offsets = []
        current_offset = 0

        for line in lines:
            # Handle shapely objects
            if hasattr(line, 'coords'):
                pts = list(line.coords)
            else:
                pts = line

            if not pts:
                continue

            parsed_offsets.append(current_offset)

            # infer stride from first point if not set
            if stride is None:
                stride = len(pts[0])

            for pt in pts:
                flat_coords.extend(pt[:stride])
                current_offset += 1

        coords = np.array(flat_coords, dtype=np.float64)
        offsets = np.array(parsed_offsets, dtype=np.uint32)

    elif coords is None or offsets is None:
        raise ValueError("Either 'lines' or both 'coords' and 'offsets' must be provided.")

    # Ensure coords is a numpy array
    coords = np.ascontiguousarray(coords, dtype=np.float64)
    offsets = np.ascontiguousarray(offsets, dtype=np.uint32)

    if stride is None:
        if coords.ndim == 2:
            stride = coords.shape[1]
        else:
            stride = 2

    if stride not in (2, 3):
        raise ValueError("stride must be 2 or 3")

    if coords.ndim == 2:
        if coords.shape[1] != stride:
             # If explicit stride is given but shape mismatches, user intent is ambiguous.
             # However, we might want to slice if stride=2 and shape=3?
             if stride == 2 and coords.shape[1] == 3:
                 coords = coords[:, :2]
                 coords = np.ascontiguousarray(coords, dtype=np.float64)
             else:
                 raise ValueError(f"Input shape {coords.shape} does not match stride {stride}")
        coords = coords.ravel()

    # Check length
    if coords.size % stride != 0:
        raise ValueError(f"Coordinates array length must be multiple of {stride}.")

    if line_ids is not None:
        line_ids = np.ascontiguousarray(line_ids, dtype=np.uint32)

    # Use the new options API as a wrapper
    options = {
        "target": "Native",
        "node_input": node,
        "snap_grid_size": snap,
        "extract_only_polygonal": extract_only_polygonal,
        "snap_strategy": "Grid",
        "noding": {
            "backend": "Snap",
            "snap_mode": "FloatEpsilonDedup"
        },
        "containment": {
            "touch_policy": "AllowPointTouchDisallowEdgeShare",
            "index_backend": "RStar"
        },
        "tiling": None,
        "z": {
            "policy": "Ignore"
        },
        "determinism": {
            "canonical_sort": True,
            "canonical_ring_rotation": True,
            "stable_tie_breaks": True
        },
        "diagnostics": {
            "enabled": False,
            "report_mode": False
        },
        "provenance": {
            "enabled": False,
            "include_boundary_line_ids": False
        },
        "input_profile_id": None
    }

    result = polygonize_with_options(coords=coords, offsets=offsets, options=options, stride=stride, line_ids=line_ids)

    if return_polygons:
        try:
            from shapely.geometry import Polygon
        except ImportError:
            raise ImportError("return_polygons=True requires 'shapely' to be installed.")

        shapely_polys = []
        if 'polygons' in result:
            for sp in result['polygons']:
                poly = Polygon(sp.shell, sp.holes)
                if hasattr(sp, 'provenance') and sp.provenance is not None:
                    pass
                shapely_polys.append(poly)

        return shapely_polys

    return result
