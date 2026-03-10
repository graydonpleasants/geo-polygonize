import sys
with open('crates/geo-polygonize-wasm/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace(
    '        extract_only_polygonal,\n    };',
    '        extract_only_polygonal,\n        report_mode: false,\n    };'
)

with open('crates/geo-polygonize-wasm/src/lib.rs', 'w') as f:
    f.write(content)
