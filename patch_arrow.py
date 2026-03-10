import sys

for file_path in ['crates/geo-polygonize-core/src/arrow_api.rs', 'crates/geo-polygonize-core/src/ffi.rs']:
    with open(file_path, 'r') as f:
        content = f.read()

    content = content.replace(
        'pub extract_only_polygonal: bool,',
        'pub extract_only_polygonal: bool,\n    pub report_mode: bool,'
    )

    content = content.replace(
        'polygonizer.extract_only_polygonal = opts.extract_only_polygonal;',
        'polygonizer.extract_only_polygonal = opts.extract_only_polygonal;\n        polygonizer.diagnostics_options.enabled = opts.report_mode;\n        polygonizer.diagnostics_options.report_mode = opts.report_mode;'
    )

    with open(file_path, 'w') as f:
        f.write(content)
