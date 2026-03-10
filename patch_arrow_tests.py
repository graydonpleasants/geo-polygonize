import sys
import glob

for pattern in ['crates/geo-polygonize-core/tests/*.rs', 'crates/geo-polygonize-core/src/ffi.rs']:
    for file_path in glob.glob(pattern):
        with open(file_path, 'r') as f:
            content = f.read()

        if 'extract_only_polygonal: 0,' in content:
            content = content.replace('extract_only_polygonal: 0,', 'extract_only_polygonal: 0,\n            report_mode: 0,')
        if 'extract_only_polygonal: false,' in content:
            content = content.replace('extract_only_polygonal: false,', 'extract_only_polygonal: false,\n            report_mode: false,')

        with open(file_path, 'w') as f:
            f.write(content)
