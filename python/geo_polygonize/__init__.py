from .types import SimplePolygon
import numpy as np

try:
    from .geo_polygonize_core import polygonize as _polygonize_impl
except ImportError:
    from .cffi_wrapper import polygonize as _polygonize_impl

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

    result = _polygonize_impl(coords, offsets, node=node, snap=snap, extract_only_polygonal=extract_only_polygonal, stride=stride, line_ids=line_ids)

    if return_polygons:
        try:
            from shapely.geometry import Polygon
        except ImportError:
            raise ImportError("return_polygons=True requires 'shapely' to be installed.")

        # PyO3 bindings return dict but cffi bindings return a custom SimplePolygon objects.
        # We need to handle both cases to construct Shapely polygons.
        shapely_polys = []
        if 'polygons' in result and all(isinstance(p, SimplePolygon) for p in result['polygons']):
            for sp in result['polygons']:
                shapely_polys.append(Polygon(sp.shell, sp.holes))
        else:
            # We are using native PyO3 core returned dict.
            flat_coords = result["flat_coords"]
            ring_offsets = result["ring_offsets"]
            polygon_offsets = result["polygon_offsets"]
            out_stride = result["stride"]

            for p_idx in range(len(polygon_offsets)):
                ring_start = polygon_offsets[p_idx]
                ring_end = polygon_offsets[p_idx+1] if p_idx + 1 < len(polygon_offsets) else len(ring_offsets)

                shell = None
                holes = []

                for r in range(ring_start, ring_end):
                    point_start = ring_offsets[r]
                    point_end = ring_offsets[r+1] if r + 1 < len(ring_offsets) else (len(flat_coords) // out_stride)

                    ring_coords = []
                    for i in range(point_start, point_end):
                        idx = i * out_stride
                        if out_stride == 3:
                            ring_coords.append((flat_coords[idx], flat_coords[idx+1], flat_coords[idx+2]))
                        else:
                            ring_coords.append((flat_coords[idx], flat_coords[idx+1]))

                    if shell is None:
                        shell = ring_coords
                    else:
                        holes.append(ring_coords)

                if shell:
                    shapely_polys.append(Polygon(shell, holes))

        return shapely_polys

    return result
