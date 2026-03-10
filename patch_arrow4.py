import sys
with open('crates/geo-polygonize-core/src/ffi.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'pub extract_only_polygonal: u8,',
    'pub extract_only_polygonal: u8,\n    pub report_mode: u8,'
)

with open('crates/geo-polygonize-core/src/ffi.rs', 'w') as f:
    f.write(content)
