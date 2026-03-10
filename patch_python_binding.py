import sys
with open('python/geo_polygonize/__init__.py', 'r') as f:
    content = f.read()

content = content.replace(
    '        coords: npt.NDArray[np.float64],\n        offsets: npt.NDArray[np.uint32],\n        node: bool = False,\n        snap: float = 1e-10,\n        extract_only_polygonal: bool = False,\n        stride: int = 2,\n        line_ids: Optional[npt.NDArray[np.uint32]] = None,\n    ) -> Dict[str, Any]:',
    '        coords: npt.NDArray[np.float64],\n        offsets: npt.NDArray[np.uint32],\n        node: bool = False,\n        snap: float = 1e-10,\n        extract_only_polygonal: bool = False,\n        stride: int = 2,\n        line_ids: Optional[npt.NDArray[np.uint32]] = None,\n        report_mode: bool = False,\n    ) -> Dict[str, Any]:'
)

content = content.replace(
    '            return geo_polygonize_core.polygonize(\n                coords, offsets, node, snap, extract_only_polygonal, stride, line_ids\n            )',
    '            return geo_polygonize_core.polygonize(\n                coords, offsets, node, snap, extract_only_polygonal, stride, line_ids, report_mode\n            )'
)

with open('python/geo_polygonize/__init__.py', 'w') as f:
    f.write(content)
