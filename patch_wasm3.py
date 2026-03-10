import sys
with open('crates/geo-polygonize-wasm/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'pub fn polygonize(\n    geojson_str: &str,\n    node_input: Option<bool>,\n    snap_grid_size: Option<f64>,\n    extract_only_polygonal: Option<bool>,\n) -> Result<String, JsValue> {',
    'pub fn polygonize(\n    geojson_str: &str,\n    node_input: Option<bool>,\n    snap_grid_size: Option<f64>,\n    extract_only_polygonal: Option<bool>,\n    report_mode: Option<bool>,\n) -> Result<String, JsValue> {'
)
content = content.replace('if let Some(rm) = options.report_mode {', 'if let Some(rm) = report_mode {')

# Also fix polygonize_buffers and polygonize_geoarrow
content = content.replace(
    'pub fn polygonize_buffers(\n    coords: js_sys::Float64Array,\n    offsets: js_sys::Uint32Array,\n    stride: Option<u8>,\n    node_input: Option<bool>,\n    snap_grid_size: Option<f64>,\n    extract_only_polygonal: Option<bool>,\n) -> Result<js_sys::Float64Array, JsValue> {',
    'pub fn polygonize_buffers(\n    coords: js_sys::Float64Array,\n    offsets: js_sys::Uint32Array,\n    stride: Option<u8>,\n    node_input: Option<bool>,\n    snap_grid_size: Option<f64>,\n    extract_only_polygonal: Option<bool>,\n    report_mode: Option<bool>,\n) -> Result<js_sys::Float64Array, JsValue> {'
)

content = content.replace(
    'pub fn polygonize_geoarrow(\n    arrow_ipc_bytes: js_sys::Uint8Array,\n    node_input: Option<bool>,\n    snap_grid_size: Option<f64>,\n    extract_only_polygonal: Option<bool>,\n) -> Result<js_sys::Uint8Array, JsValue> {',
    'pub fn polygonize_geoarrow(\n    arrow_ipc_bytes: js_sys::Uint8Array,\n    node_input: Option<bool>,\n    snap_grid_size: Option<f64>,\n    extract_only_polygonal: Option<bool>,\n    report_mode: Option<bool>,\n) -> Result<js_sys::Uint8Array, JsValue> {'
)

# And options struct
content = content.replace(
    'pub struct PolygonizeOptions {\n    #[wasm_bindgen(js_name = nodeInput)]\n    pub node_input: Option<bool>,\n    #[wasm_bindgen(js_name = snapGridSize)]\n    pub snap_grid_size: Option<f64>,\n    #[wasm_bindgen(js_name = extractOnlyPolygonal)]\n    pub extract_only_polygonal: Option<bool>,\n}',
    'pub struct PolygonizeOptions {\n    #[wasm_bindgen(js_name = nodeInput)]\n    pub node_input: Option<bool>,\n    #[wasm_bindgen(js_name = snapGridSize)]\n    pub snap_grid_size: Option<f64>,\n    #[wasm_bindgen(js_name = extractOnlyPolygonal)]\n    pub extract_only_polygonal: Option<bool>,\n    #[wasm_bindgen(js_name = reportMode)]\n    pub report_mode: Option<bool>,\n}'
)

with open('crates/geo-polygonize-wasm/src/lib.rs', 'w') as f:
    f.write(content)
