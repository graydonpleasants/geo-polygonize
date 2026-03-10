import sys
with open('crates/geo-polygonize-core/src/ffi.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'extract_only_polygonal: (*options).extract_only_polygonal,',
    'extract_only_polygonal: (*options).extract_only_polygonal,\n            report_mode: (*options).report_mode,'
)

with open('crates/geo-polygonize-core/src/ffi.rs', 'w') as f:
    f.write(content)
