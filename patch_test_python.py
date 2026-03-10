import sys
with open('python/test_wrapper.py', 'r') as f:
    content = f.read()

content = content.replace(
    'from geo_polygonize_core import InvalidGeometryError',
    'from geo_polygonize.geo_polygonize_core import InvalidGeometryError'
)

with open('python/test_wrapper.py', 'w') as f:
    f.write(content)
