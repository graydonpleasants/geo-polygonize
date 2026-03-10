import sys
with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'invalid_rings,\n            diagnostics: None,',
    'invalid_rings,\n            diagnostics: diag,'
)

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
