import sys
with open('crates/geo-polygonize-core/src/ffi.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'extract_only_polygonal: opts.extract_only_polygonal != 0,',
    'extract_only_polygonal: opts.extract_only_polygonal != 0,\n            report_mode: opts.report_mode != 0,'
)
content = content.replace('pub report_mode: bool,', 'pub report_mode: u8,')

with open('crates/geo-polygonize-core/src/ffi.rs', 'w') as f:
    f.write(content)
