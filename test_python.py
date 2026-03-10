import sys
import numpy as np

try:
    from geo_polygonize import polygonize
    print("Successfully imported geo_polygonize")
except Exception as e:
    print(f"Error importing geo_polygonize: {e}")
    sys.exit(1)
