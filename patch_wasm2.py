import sys
with open('crates/geo-polygonize-wasm/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'pub struct PolygonizeOptions {',
    'pub struct PolygonizeOptions {\n    #[wasm_bindgen(js_name = reportMode)]\n    pub report_mode: Option<bool>,'
)

content = content.replace(
    '    if let Some(eop) = extract_only_polygonal {\n        polygonizer.extract_only_polygonal = eop;\n    }',
    '    if let Some(eop) = extract_only_polygonal {\n        polygonizer.extract_only_polygonal = eop;\n    }\n    if let Some(rm) = options.report_mode {\n        polygonizer.diagnostics_options.enabled = rm;\n        polygonizer.diagnostics_options.report_mode = rm;\n    }'
)

with open('crates/geo-polygonize-wasm/src/lib.rs', 'w') as f:
    f.write(content)
