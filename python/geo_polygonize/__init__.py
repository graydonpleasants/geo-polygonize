from .types import SimplePolygon

try:
    from .geo_polygonize_core import polygonize
except ImportError:
    from .cffi_wrapper import polygonize
