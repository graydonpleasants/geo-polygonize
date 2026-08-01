use numpy::{PyArray1, PyReadonlyArray1};
use polygonize_core::{
    normalize_polygonize_error, polygonize_with_execution_policy, CancellationToken, Coord3D,
    ExecutionPolicy, Line3D, PolygonizeError, PolygonizerOptions, PolygonizerResult,
    PrecisionModel, TopologyFingerprintV1,
};
use pyo3::create_exception;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use serde::Deserialize;
use std::sync::mpsc::{sync_channel, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

const SIGNAL_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PYTHON_CONVERSION_SIGNAL_INTERVAL: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Buffers,
    Objects,
    Report,
}

impl OutputMode {
    fn parse(value: &str) -> PyResult<Self> {
        match value {
            "buffers" => Ok(Self::Buffers),
            "objects" => Ok(Self::Objects),
            "report" => Ok(Self::Report),
            _ => Err(PolygonizeOptionsError::new_err(
                "output must be 'buffers', 'objects', or 'report'",
            )),
        }
    }

    fn needs_buffers(self) -> bool {
        matches!(self, Self::Buffers | Self::Report)
    }

    fn needs_objects(self) -> bool {
        matches!(self, Self::Objects | Self::Report)
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PythonExecutionLimits {
    max_input_line_strings: Option<usize>,
    max_input_segments: Option<usize>,
    max_input_coordinates: Option<usize>,
    max_noded_segments: Option<usize>,
    max_candidate_pairs: Option<usize>,
    max_exact_intersection_calls: Option<usize>,
    max_split_events: Option<usize>,
    max_noding_iterations: Option<usize>,
    max_graph_nodes: Option<usize>,
    max_graph_edges: Option<usize>,
    max_rings: Option<usize>,
    max_output_polygons: Option<usize>,
    max_output_coordinates: Option<usize>,
}

impl PythonExecutionLimits {
    fn into_policy(self, cancellation_token: CancellationToken) -> ExecutionPolicy {
        ExecutionPolicy {
            cancellation_token: Some(cancellation_token),
            max_input_line_strings: self.max_input_line_strings,
            max_input_segments: self.max_input_segments,
            max_input_coordinates: self.max_input_coordinates,
            max_noded_segments: self.max_noded_segments,
            max_candidate_pairs: self.max_candidate_pairs,
            max_exact_intersection_calls: self.max_exact_intersection_calls,
            max_split_events: self.max_split_events,
            max_noding_iterations: self.max_noding_iterations,
            max_graph_nodes: self.max_graph_nodes,
            max_graph_edges: self.max_graph_edges,
            max_rings: self.max_rings,
            max_output_polygons: self.max_output_polygons,
            max_output_coordinates: self.max_output_coordinates,
        }
    }
}

#[derive(Default)]
struct OwnedBuffers {
    flat_coords: Vec<f64>,
    ring_offsets: Vec<u32>,
    polygon_offsets: Vec<u32>,
    flat_line_ids: Vec<u32>,
}

#[derive(Default)]
struct BindingTimings {
    thread_spawn: Duration,
    buffer_conversion: Duration,
    fingerprint: Duration,
    python_objects: Duration,
}

struct WorkerOutput {
    result: PolygonizerResult,
    fingerprint: Option<TopologyFingerprintV1>,
    buffers: Option<OwnedBuffers>,
    timings: BindingTimings,
}

create_exception!(
    geo_polygonize_core,
    PolygonizeTypeError,
    pyo3::exceptions::PyException
);
create_exception!(
    geo_polygonize_core,
    PolygonizeGeometryError,
    pyo3::exceptions::PyException
);
create_exception!(
    geo_polygonize_core,
    PolygonizeOptionsError,
    pyo3::exceptions::PyException
);
create_exception!(
    geo_polygonize_core,
    PolygonizeTopologyError,
    pyo3::exceptions::PyException
);

fn to_py_err(err: PolygonizeError) -> PyErr {
    let normalized = serde_json::to_string(&normalize_polygonize_error(&err))
        .expect("normalized errors serialize");
    let py_err = match err {
        PolygonizeError::InvalidArgumentType {
            field,
            expected,
            actual,
        } => PolygonizeTypeError::new_err(format!(
            "Invalid argument type for {field}: expected {expected}, got {actual}"
        )),
        PolygonizeError::InvalidGeometry { reason } => PolygonizeGeometryError::new_err(reason),
        PolygonizeError::NonFiniteCoordinate { reason } => PolygonizeGeometryError::new_err(reason),
        PolygonizeError::InvalidBufferShape { reason } => PolygonizeTypeError::new_err(reason),
        PolygonizeError::ResourceLimitExceeded { .. } => PyRuntimeError::new_err(err.to_string()),
        PolygonizeError::Cancelled { .. } => PyRuntimeError::new_err(err.to_string()),
        PolygonizeError::UnsupportedOptionCombination { reason } => {
            PolygonizeOptionsError::new_err(reason)
        }
        err @ PolygonizeError::ZConflict { .. } => {
            PolygonizeTopologyError::new_err(err.to_string())
        }
        err @ PolygonizeError::NodingValidationFailure { .. } => {
            PolygonizeTopologyError::new_err(err.to_string())
        }
        PolygonizeError::InternalInvariantViolation { reason } => PyRuntimeError::new_err(reason),
        PolygonizeError::ArrowError(msg) => PyValueError::new_err(msg),
        PolygonizeError::Panic(msg) => PyRuntimeError::new_err(msg),
    };
    Python::try_attach(|py| {
        py_err
            .value(py)
            .setattr("normalized", normalized)
            .expect("exceptions accept attributes");
    })
    .expect("Python exception conversion runs while attached");
    py_err
}

fn polygonize_without_gil(
    py: Python<'_>,
    lines: Vec<Line3D>,
    options: PolygonizerOptions,
    execution_limits: PythonExecutionLimits,
    output_mode: OutputMode,
    stride: usize,
) -> PyResult<WorkerOutput> {
    py.check_signals()?;
    let token = CancellationToken::new();
    let policy = execution_limits.into_policy(token.clone());
    let (sender, receiver) = sync_channel(1);
    let spawn_started = Instant::now();
    let worker = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = polygonize_with_execution_policy(lines, &options, &policy)?;
            let buffer_started = Instant::now();
            let buffers = output_mode
                .needs_buffers()
                .then(|| flatten_result(&result, stride))
                .transpose()?;
            let buffer_conversion = buffer_started.elapsed();

            let fingerprint_started = Instant::now();
            let fingerprint = (output_mode == OutputMode::Report)
                .then(|| TopologyFingerprintV1::try_from_result(&result, &options))
                .transpose()?;
            let fingerprint_duration = fingerprint_started.elapsed();
            Ok(WorkerOutput {
                result,
                fingerprint,
                buffers,
                timings: BindingTimings {
                    buffer_conversion,
                    fingerprint: fingerprint_duration,
                    ..Default::default()
                },
            })
        }))
        .unwrap_or_else(|_| {
            Err(PolygonizeError::Panic(
                "Panic occurred in Rust core".to_string(),
            ))
        });
        let _ = sender.send(result);
    });
    let thread_spawn = spawn_started.elapsed();

    let result = py.detach(move || {
        let result = loop {
            match receiver.recv_timeout(SIGNAL_POLL_INTERVAL) {
                Ok(result) => break Ok(result),
                Err(RecvTimeoutError::Timeout) => {
                    if let Err(signal) = Python::attach(|py| py.check_signals()) {
                        token.cancel();
                        break Err(signal);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    break Ok(Err(PolygonizeError::Panic(
                        "Rust worker exited before returning a result".to_string(),
                    )));
                }
            }
        };
        if result.is_err() {
            let _ = receiver.recv();
        }
        worker
            .join()
            .map_err(|_| PyRuntimeError::new_err("Rust worker panicked"))?;
        result
    });

    let mut output = result?.map_err(to_py_err)?;
    output.timings.thread_spawn = thread_spawn;
    Ok(output)
}

fn checked_u32(value: usize, field: &str) -> polygonize_core::Result<u32> {
    u32::try_from(value).map_err(|_| PolygonizeError::ResourceLimitExceeded {
        stage: field.to_string(),
        limit: u32::MAX as usize,
        observed: value,
    })
}

fn flatten_result(
    result: &PolygonizerResult,
    stride: usize,
) -> polygonize_core::Result<OwnedBuffers> {
    let num_points = result.polygons.iter().try_fold(0usize, |count, polygon| {
        polygon.interiors.iter().try_fold(
            count.checked_add(polygon.exterior.len()).ok_or_else(|| {
                PolygonizeError::InternalInvariantViolation {
                    reason: "Python output point count overflowed usize".to_string(),
                }
            })?,
            |count, ring| {
                count.checked_add(ring.len()).ok_or_else(|| {
                    PolygonizeError::InternalInvariantViolation {
                        reason: "Python output point count overflowed usize".to_string(),
                    }
                })
            },
        )
    })?;
    let num_rings = result.polygons.iter().try_fold(0usize, |count, polygon| {
        count
            .checked_add(1 + polygon.interiors.len())
            .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                reason: "Python output ring count overflowed usize".to_string(),
            })
    })?;
    let coordinate_capacity = num_points.checked_mul(stride).ok_or_else(|| {
        PolygonizeError::InternalInvariantViolation {
            reason: "Python output coordinate count overflowed usize".to_string(),
        }
    })?;
    let mut buffers = OwnedBuffers {
        flat_coords: Vec::with_capacity(coordinate_capacity),
        ring_offsets: Vec::with_capacity(num_rings),
        polygon_offsets: Vec::with_capacity(result.polygons.len()),
        flat_line_ids: Vec::with_capacity(num_points),
    };

    for polygon in &result.polygons {
        buffers.polygon_offsets.push(checked_u32(
            buffers.ring_offsets.len(),
            "python_polygon_offsets",
        )?);
        buffers.ring_offsets.push(checked_u32(
            buffers.flat_coords.len() / stride,
            "python_ring_offsets",
        )?);
        append_ring(
            &mut buffers,
            &polygon.exterior,
            &polygon.exterior_ids,
            stride,
        );
        for (ring, ids) in polygon.interiors.iter().zip(&polygon.interiors_ids) {
            buffers.ring_offsets.push(checked_u32(
                buffers.flat_coords.len() / stride,
                "python_ring_offsets",
            )?);
            append_ring(&mut buffers, ring, ids, stride);
        }
    }
    Ok(buffers)
}

fn append_ring(buffers: &mut OwnedBuffers, ring: &[Coord3D], ids: &[u32], stride: usize) {
    for (index, coordinate) in ring.iter().enumerate() {
        buffers.flat_coords.extend([coordinate.x, coordinate.y]);
        if stride == 3 {
            buffers.flat_coords.push(coordinate.z);
        }
        buffers
            .flat_line_ids
            .push(ids.get(index).copied().unwrap_or_default());
    }
}

#[pyfunction]
#[pyo3(signature = (coords, offsets, stride=2, options_json=None, line_ids=None, output="report", execution_json=None))]
#[allow(clippy::too_many_arguments)]
fn polygonize_with_options<'py>(
    py: Python<'py>,
    coords: PyReadonlyArray1<'py, f64>,
    offsets: PyReadonlyArray1<'py, u32>,
    stride: u8,
    options_json: Option<&str>,
    line_ids: Option<PyReadonlyArray1<'py, u32>>,
    output: &str,
    execution_json: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let options: PolygonizerOptions = if let Some(json) = options_json {
        serde_json::from_str(json)
            .map_err(|e| PolygonizeOptionsError::new_err(format!("Invalid options json: {}", e)))?
    } else {
        PolygonizerOptions::default()
    };

    polygonize_internal(
        py,
        coords,
        offsets,
        stride,
        options,
        line_ids,
        OutputMode::parse(output)?,
        parse_execution_limits(execution_json)?,
    )
}

#[pyfunction]
#[pyo3(signature = (coords, offsets, node=false, snap=1e-10, extract_only_polygonal=false, stride=2, line_ids=None, report_mode=false, output="report", execution_json=None))]
#[allow(clippy::too_many_arguments)]
fn polygonize<'py>(
    py: Python<'py>,
    coords: PyReadonlyArray1<'py, f64>,
    offsets: PyReadonlyArray1<'py, u32>,
    node: bool,
    snap: f64,
    extract_only_polygonal: bool,
    stride: u8,
    line_ids: Option<PyReadonlyArray1<'py, u32>>,
    report_mode: bool,
    output: &str,
    execution_json: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let mut options = PolygonizerOptions::default();
    options.diagnostics.enabled = report_mode;
    options.diagnostics.report_mode = report_mode;
    options.node_input = node;
    options.precision_model = if node {
        PrecisionModel::from_grid_size(snap)
    } else {
        PrecisionModel::Floating
    };
    options.extract_only_polygonal = extract_only_polygonal;

    polygonize_internal(
        py,
        coords,
        offsets,
        stride,
        options,
        line_ids,
        OutputMode::parse(output)?,
        parse_execution_limits(execution_json)?,
    )
}

fn parse_execution_limits(value: Option<&str>) -> PyResult<PythonExecutionLimits> {
    value.map_or_else(
        || Ok(PythonExecutionLimits::default()),
        |json| {
            serde_json::from_str(json).map_err(|error| {
                PolygonizeOptionsError::new_err(format!("Invalid execution limits json: {error}"))
            })
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn polygonize_internal<'py>(
    py: Python<'py>,
    coords: PyReadonlyArray1<'py, f64>,
    offsets: PyReadonlyArray1<'py, u32>,
    stride: u8,
    options: PolygonizerOptions,
    line_ids: Option<PyReadonlyArray1<'py, u32>>,
    output_mode: OutputMode,
    execution_limits: PythonExecutionLimits,
) -> PyResult<Py<PyAny>> {
    let coords_slice = coords.as_slice()?;
    let offsets_slice = offsets.as_slice()?;

    if stride != 2 && stride != 3 {
        return Err(to_py_err(PolygonizeError::InvalidBufferShape {
            reason: "stride must be 2 or 3".to_string(),
        }));
    }

    let stride_usize = stride as usize;

    if coords_slice.len() % stride_usize != 0 {
        return Err(to_py_err(PolygonizeError::InvalidBufferShape {
            reason: format!(
                "Coordinates array length {} is not a multiple of stride {}",
                coords_slice.len(),
                stride_usize
            ),
        }));
    }

    if let Some(ref ids) = line_ids {
        let ids_slice = ids.as_slice()?;
        if !offsets_slice.is_empty() && ids_slice.len() != offsets_slice.len() {
            return Err(to_py_err(PolygonizeError::InvalidBufferShape {
                reason: format!(
                    "line_ids length {} does not match line count {}",
                    ids_slice.len(),
                    offsets_slice.len()
                ),
            }));
        }
    }

    let mut lines = Vec::with_capacity(coords_slice.len() / stride_usize);

    if !offsets_slice.is_empty() {
        for i in 0..offsets_slice.len() {
            let start = offsets_slice[i] as usize;
            let end = if i + 1 < offsets_slice.len() {
                offsets_slice[i + 1] as usize
            } else {
                coords_slice.len() / stride_usize
            };

            if start > end {
                return Err(to_py_err(PolygonizeError::InvalidBufferShape {
                    reason: format!(
                        "Invalid offsets: start offset ({}) is greater than end offset ({}) at index {}",
                        start, end, i
                    ),
                }));
            }
            let coordinate_end = end.checked_mul(stride_usize).ok_or_else(|| {
                to_py_err(PolygonizeError::InvalidBufferShape {
                    reason: "Invalid offsets: coordinate index overflow".to_string(),
                })
            })?;
            if coordinate_end > coords_slice.len() {
                return Err(to_py_err(PolygonizeError::InvalidBufferShape {
                    reason: format!(
                        "Invalid offsets: calculated end offset {} exceeds coordinate capacity {} for stride {}",
                        coordinate_end, coords_slice.len(), stride_usize
                    ),
                }));
            }

            // Get line ID if provided
            let line_id = if let Some(ref ids) = line_ids {
                let ids_slice = ids.as_slice()?;
                if i < ids_slice.len() {
                    ids_slice[i]
                } else {
                    0
                }
            } else {
                0
            };

            for j in start..end.saturating_sub(1) {
                let idx = j * stride_usize;
                let jdx = (j + 1) * stride_usize;

                let z1 = if stride == 3 {
                    coords_slice[idx + 2]
                } else {
                    0.0
                };
                let z2 = if stride == 3 {
                    coords_slice[jdx + 2]
                } else {
                    0.0
                };

                let p1 = Coord3D::new(coords_slice[idx], coords_slice[idx + 1], z1);
                let p2 = Coord3D::new(coords_slice[jdx], coords_slice[jdx + 1], z2);
                lines.push(Line3D::new(p1, p2, line_id));
            }
        }
    }

    let mut output = polygonize_without_gil(
        py,
        lines,
        options,
        execution_limits,
        output_mode,
        stride_usize,
    )?;
    let result = &output.result;
    let python_objects_started = Instant::now();
    let mut converted_items = 0usize;

    let py_polygons = PyList::empty(py);
    if output_mode.needs_objects() {
        let simple_polygon_cls = py
            .import("geo_polygonize.types")?
            .getattr("SimplePolygon")?;
        for poly in &result.polygons {
            check_python_signal(py, &mut converted_items)?;
            // Construct exterior tuples
            let exterior_pts = PyList::empty(py);
            for c in &poly.exterior {
                check_python_signal(py, &mut converted_items)?;
                if stride == 3 {
                    exterior_pts.append(PyTuple::new(py, [c.x, c.y, c.z])?)?;
                } else {
                    exterior_pts.append(PyTuple::new(py, [c.x, c.y])?)?;
                }
            }
            let shell = PyTuple::new(py, exterior_pts)?;

            let shell_ids = PyTuple::new(py, &poly.exterior_ids)?;

            // Construct interiors
            let holes = PyList::empty(py);
            let holes_ids = PyList::empty(py);

            for (h_idx, ring) in poly.interiors.iter().enumerate() {
                let ring_pts = PyList::empty(py);
                for c in ring {
                    check_python_signal(py, &mut converted_items)?;
                    if stride == 3 {
                        ring_pts.append(PyTuple::new(py, [c.x, c.y, c.z])?)?;
                    } else {
                        ring_pts.append(PyTuple::new(py, [c.x, c.y])?)?;
                    }
                }
                holes.append(PyTuple::new(py, ring_pts)?)?;

                let r_ids = PyTuple::new(py, &poly.interiors_ids[h_idx])?;
                holes_ids.append(r_ids)?;
            }

            let py_provenance = if let Some(ref prov) = poly.provenance {
                let prov_dict = PyDict::new(py);
                let b_ids = PyTuple::new(py, &prov.boundary_line_ids)?;
                prov_dict.set_item("boundary_line_ids", b_ids)?;
                if let Some(ref prof_id) = prov.input_profile_id {
                    prov_dict.set_item("input_profile_id", prof_id)?;
                } else {
                    prov_dict.set_item("input_profile_id", py.None())?;
                }
                prov_dict.into_any()
            } else {
                py.None().into_bound(py)
            };

            let py_poly =
                simple_polygon_cls.call1((shell, holes, shell_ids, holes_ids, py_provenance))?;
            py_polygons.append(py_poly)?;
        }
    }

    // Construct dangles
    let py_dangles = PyList::empty(py);
    for dangle in result
        .dangles
        .iter()
        .filter(|_| output_mode.needs_objects())
    {
        let dangle_pts = PyList::empty(py);
        for c in dangle {
            check_python_signal(py, &mut converted_items)?;
            if stride == 3 {
                dangle_pts.append(PyTuple::new(py, [c.x, c.y, c.z])?)?;
            } else {
                dangle_pts.append(PyTuple::new(py, [c.x, c.y])?)?;
            }
        }
        py_dangles.append(PyTuple::new(py, dangle_pts)?)?;
    }

    // Construct cut edges
    let py_cut_edges = PyList::empty(py);
    for cut_edge in result
        .cut_edges
        .iter()
        .filter(|_| output_mode.needs_objects())
    {
        let cut_edge_pts = PyList::empty(py);
        for c in cut_edge {
            check_python_signal(py, &mut converted_items)?;
            if stride == 3 {
                cut_edge_pts.append(PyTuple::new(py, [c.x, c.y, c.z])?)?;
            } else {
                cut_edge_pts.append(PyTuple::new(py, [c.x, c.y])?)?;
            }
        }
        py_cut_edges.append(PyTuple::new(py, cut_edge_pts)?)?;
    }

    // Construct invalid rings
    let py_invalid_rings = PyList::empty(py);
    for invalid_ring in result
        .invalid_rings
        .iter()
        .filter(|_| output_mode.needs_objects())
    {
        let invalid_pts = PyList::empty(py);
        for c in invalid_ring {
            check_python_signal(py, &mut converted_items)?;
            if stride == 3 {
                invalid_pts.append(PyTuple::new(py, [c.x, c.y, c.z])?)?;
            } else {
                invalid_pts.append(PyTuple::new(py, [c.x, c.y])?)?;
            }
        }
        py_invalid_rings.append(PyTuple::new(py, invalid_pts)?)?;
    }
    output.timings.python_objects = python_objects_started.elapsed();

    let dict = PyDict::new(py);
    dict.set_item("stride", stride)?;
    if let Some(buffers) = output.buffers {
        dict.set_item("flat_coords", PyArray1::from_vec(py, buffers.flat_coords))?;
        dict.set_item("ring_offsets", PyArray1::from_vec(py, buffers.ring_offsets))?;
        dict.set_item(
            "polygon_offsets",
            PyArray1::from_vec(py, buffers.polygon_offsets),
        )?;
        dict.set_item(
            "flat_line_ids",
            PyArray1::from_vec(py, buffers.flat_line_ids),
        )?;
    }
    if output_mode.needs_objects() {
        dict.set_item("polygons", py_polygons)?;
        dict.set_item("dangles", py_dangles)?;
        dict.set_item("cut_edges", py_cut_edges)?;
        dict.set_item("invalid_rings", py_invalid_rings)?;
    }
    if output_mode == OutputMode::Report {
        let json_module = py.import("json")?;
        let loads = json_module.getattr("loads")?;
        if let Some(fingerprint) = output.fingerprint {
            dict.set_item(
                "topology_fingerprint",
                loads.call1((
                    serde_json::to_string(&fingerprint).expect("fingerprint serializes"),
                ))?,
            )?;
        }
        if let Some(ref diagnostics) = result.diagnostics {
            dict.set_item(
                "diagnostics",
                loads
                    .call1((serde_json::to_string(diagnostics).expect("diagnostics serialize"),))?,
            )?;
        }
        let timings = PyDict::new(py);
        timings.set_item(
            "thread_spawn_ms",
            output.timings.thread_spawn.as_secs_f64() * 1_000.0,
        )?;
        timings.set_item(
            "buffer_conversion_ms",
            output.timings.buffer_conversion.as_secs_f64() * 1_000.0,
        )?;
        timings.set_item(
            "fingerprint_ms",
            output.timings.fingerprint.as_secs_f64() * 1_000.0,
        )?;
        timings.set_item(
            "python_objects_ms",
            output.timings.python_objects.as_secs_f64() * 1_000.0,
        )?;
        dict.set_item("binding_timings", timings)?;
    }

    Ok(dict.into())
}

fn check_python_signal(py: Python<'_>, converted_items: &mut usize) -> PyResult<()> {
    *converted_items = converted_items.saturating_add(1);
    if converted_items.is_multiple_of(PYTHON_CONVERSION_SIGNAL_INTERVAL) {
        py.check_signals()?;
    }
    Ok(())
}

#[pymodule]
fn geo_polygonize_core(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("PolygonizeTypeError", py.get_type::<PolygonizeTypeError>())?;
    m.add(
        "PolygonizeGeometryError",
        py.get_type::<PolygonizeGeometryError>(),
    )?;
    m.add(
        "PolygonizeOptionsError",
        py.get_type::<PolygonizeOptionsError>(),
    )?;
    m.add(
        "PolygonizeTopologyError",
        py.get_type::<PolygonizeTopologyError>(),
    )?;
    m.add_function(wrap_pyfunction!(polygonize, m)?)?;
    m.add_function(wrap_pyfunction!(polygonize_with_options, m)?)?;
    Ok(())
}
