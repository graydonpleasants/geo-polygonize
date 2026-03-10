import sys

with open('crates/geo-polygonize-core/src/python.rs', 'r') as f:
    content = f.read()

content = content.replace(
    '#[pyo3(signature = (coords, offsets, node=false, snap=1e-10, extract_only_polygonal=false, stride=2, line_ids=None))]',
    '#[pyo3(signature = (coords, offsets, node=false, snap=1e-10, extract_only_polygonal=false, stride=2, line_ids=None, report_mode=false))]'
)

content = content.replace(
    '    stride: u8,\n    line_ids: Option<PyReadonlyArray1<\'py, u32>>,\n) -> PyResult<PyObject> {',
    '    stride: u8,\n    line_ids: Option<PyReadonlyArray1<\'py, u32>>,\n    report_mode: bool,\n) -> PyResult<PyObject> {'
)

# Apply report mode options to Polygonizer
content = content.replace(
    '    let mut polygonizer = Polygonizer::new();',
    '    let mut polygonizer = Polygonizer::new();\n    polygonizer.diagnostics_options.enabled = report_mode;\n    polygonizer.diagnostics_options.report_mode = report_mode;'
)

# Add diagnostic result to PyDict
idx = content.find('let dangles_list = PyList::new(')
if idx != -1:
    content = content[:idx] + """    if let Some(diag) = result.diagnostics {
        let diag_dict = PyDict::new(py);
        diag_dict.set_item("input_segment_count", diag.input_segment_count)?;
        diag_dict.set_item("noded_segment_count", diag.noded_segment_count)?;
        diag_dict.set_item("dangle_count", diag.dangle_count)?;
        diag_dict.set_item("cut_edge_count", diag.cut_edge_count)?;
        diag_dict.set_item("ring_count", diag.ring_count)?;
        diag_dict.set_item("shell_count", diag.shell_count)?;
        diag_dict.set_item("hole_count", diag.hole_count)?;
        diag_dict.set_item("invalid_ring_count", diag.invalid_ring_count)?;
        diag_dict.set_item("flat_line_count", diag.flat_line_count)?;

        let times_dict = PyDict::new(py);
        times_dict.set_item("ingest_and_node_ms", diag.phase_times.ingest_and_node.as_secs_f64() * 1000.0)?;
        times_dict.set_item("graph_build_ms", diag.phase_times.graph_build.as_secs_f64() * 1000.0)?;
        times_dict.set_item("ring_extraction_ms", diag.phase_times.ring_extraction.as_secs_f64() * 1000.0)?;
        times_dict.set_item("containment_ms", diag.phase_times.containment.as_secs_f64() * 1000.0)?;
        times_dict.set_item("output_flatten_ms", diag.phase_times.output_flatten.as_secs_f64() * 1000.0)?;

        diag_dict.set_item("phase_times", times_dict)?;
        result_dict.set_item("diagnostics", diag_dict)?;
    }

    """ + content[idx:]

with open('crates/geo-polygonize-core/src/python.rs', 'w') as f:
    f.write(content)
