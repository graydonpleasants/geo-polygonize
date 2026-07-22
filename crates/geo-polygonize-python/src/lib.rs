use numpy::{PyArray1, PyReadonlyArray1};
use polygonize_core::{
    normalize_polygonize_error, polygonize as polygonize_lines, Coord3D, Line3D, PolygonizeError,
    PolygonizerOptions, PrecisionModel, TopologyFingerprintV1,
};
use pyo3::create_exception;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

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
        PolygonizeError::InvalidBufferShape { reason } => PolygonizeTypeError::new_err(reason),
        PolygonizeError::ResourceLimitExceeded { .. } => PyRuntimeError::new_err(err.to_string()),
        PolygonizeError::UnsupportedOptionCombination { reason } => {
            PolygonizeOptionsError::new_err(reason)
        }
        PolygonizeError::TopologyFailure { reason } => PolygonizeTopologyError::new_err(reason),
        err @ PolygonizeError::ZConflict { .. } => {
            PolygonizeTopologyError::new_err(err.to_string())
        }
        err @ PolygonizeError::NodingValidationFailure { .. } => {
            PolygonizeTopologyError::new_err(err.to_string())
        }
        PolygonizeError::InternalInvariantViolation { reason } => PyRuntimeError::new_err(reason),
        PolygonizeError::ArrowError(msg) => PyValueError::new_err(msg),
        PolygonizeError::NullPointer(msg) => PyValueError::new_err(msg),
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

#[pyfunction]
#[pyo3(signature = (coords, offsets, stride=2, options_json=None, line_ids=None))]
#[allow(clippy::too_many_arguments)]
fn polygonize_with_options<'py>(
    py: Python<'py>,
    coords: PyReadonlyArray1<'py, f64>,
    offsets: PyReadonlyArray1<'py, u32>,
    stride: u8,
    options_json: Option<&str>,
    line_ids: Option<PyReadonlyArray1<'py, u32>>,
) -> PyResult<Py<PyAny>> {
    let options: PolygonizerOptions = if let Some(json) = options_json {
        serde_json::from_str(json)
            .map_err(|e| PolygonizeOptionsError::new_err(format!("Invalid options json: {}", e)))?
    } else {
        PolygonizerOptions::default()
    };

    polygonize_internal(py, coords, offsets, stride, options, line_ids)
}

#[pyfunction]
#[pyo3(signature = (coords, offsets, node=false, snap=1e-10, extract_only_polygonal=false, stride=2, line_ids=None, report_mode=false))]
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

    polygonize_internal(py, coords, offsets, stride, options, line_ids)
}

fn polygonize_internal<'py>(
    py: Python<'py>,
    coords: PyReadonlyArray1<'py, f64>,
    offsets: PyReadonlyArray1<'py, u32>,
    stride: u8,
    options: PolygonizerOptions,
    line_ids: Option<PyReadonlyArray1<'py, u32>>,
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
            if end * stride_usize > coords_slice.len() {
                return Err(to_py_err(PolygonizeError::InvalidBufferShape {
                    reason: format!(
                        "Invalid offsets: calculated end offset {} exceeds coordinate capacity {} for stride {}",
                        end * stride_usize, coords_slice.len(), stride_usize
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

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        polygonize_lines(lines, &options)
    }))
    .unwrap_or_else(|_| {
        Err(PolygonizeError::Panic(
            "Panic occurred in Rust core".to_string(),
        ))
    })
    .map_err(to_py_err)?;
    let topology_fingerprint =
        TopologyFingerprintV1::try_from_result(&result, &options).map_err(to_py_err)?;

    // Flatten logic
    let mut num_points = 0;
    let mut num_rings = 0;
    let num_polygons = result.polygons.len();

    for poly in &result.polygons {
        num_points += poly.exterior.len();
        num_rings += 1 + poly.interiors.len();
        for ring in &poly.interiors {
            num_points += ring.len();
        }
    }

    let mut flat_coords = Vec::with_capacity(num_points * stride_usize);
    let mut ring_offsets = Vec::with_capacity(num_rings);
    let mut polygon_offsets = Vec::with_capacity(num_polygons);
    let mut flat_line_ids = Vec::with_capacity(num_points);

    for poly in &result.polygons {
        polygon_offsets.push(ring_offsets.len() as u32);

        let exterior = &poly.exterior;
        let interiors = &poly.interiors;

        ring_offsets.push((flat_coords.len() / stride_usize) as u32);
        for (k, c) in exterior.iter().enumerate() {
            flat_coords.push(c.x);
            flat_coords.push(c.y);
            if stride == 3 {
                flat_coords.push(c.z);
            }
            if k < poly.exterior_ids.len() {
                flat_line_ids.push(poly.exterior_ids[k]);
            } else {
                flat_line_ids.push(0);
            }
        }

        for (h_idx, ring) in interiors.iter().enumerate() {
            ring_offsets.push((flat_coords.len() / stride_usize) as u32);
            for (k, c) in ring.iter().enumerate() {
                flat_coords.push(c.x);
                flat_coords.push(c.y);
                if stride == 3 {
                    flat_coords.push(c.z);
                }
                if k < poly.interiors_ids[h_idx].len() {
                    flat_line_ids.push(poly.interiors_ids[h_idx][k]);
                } else {
                    flat_line_ids.push(0);
                }
            }
        }
    }

    // We will build Python polygons via the SimplePolygon class from geo_polygonize.types.
    // If geo_polygonize.types isn't available, we fallback to a simple dict representation?
    // The issue asks to "construct and return SimplePolygon objects directly".
    let py_polygons = PyList::empty(py);

    // Attempt to import the Python class `SimplePolygon`
    let simple_polygon_cls = py
        .import("geo_polygonize.types")?
        .getattr("SimplePolygon")?;

    for poly in &result.polygons {
        // Construct exterior tuples
        let exterior_pts = PyList::empty(py);
        for c in &poly.exterior {
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

    // Construct dangles
    let py_dangles = PyList::empty(py);
    for dangle in &result.dangles {
        let dangle_pts = PyList::empty(py);
        for c in dangle {
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
    for cut_edge in &result.cut_edges {
        let cut_edge_pts = PyList::empty(py);
        for c in cut_edge {
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
    for invalid_ring in &result.invalid_rings {
        let invalid_pts = PyList::empty(py);
        for c in invalid_ring {
            if stride == 3 {
                invalid_pts.append(PyTuple::new(py, [c.x, c.y, c.z])?)?;
            } else {
                invalid_pts.append(PyTuple::new(py, [c.x, c.y])?)?;
            }
        }
        py_invalid_rings.append(PyTuple::new(py, invalid_pts)?)?;
    }

    let dict = PyDict::new(py);
    dict.set_item("flat_coords", PyArray1::from_vec(py, flat_coords))?;
    dict.set_item("ring_offsets", PyArray1::from_vec(py, ring_offsets))?;
    dict.set_item("polygon_offsets", PyArray1::from_vec(py, polygon_offsets))?;
    dict.set_item("flat_line_ids", PyArray1::from_vec(py, flat_line_ids))?;
    dict.set_item("stride", stride)?;

    dict.set_item("polygons", py_polygons)?;
    dict.set_item("dangles", py_dangles)?;
    dict.set_item("cut_edges", py_cut_edges)?;
    dict.set_item("invalid_rings", py_invalid_rings)?;
    let json_module = py.import("json")?;
    let loads_func = json_module.getattr("loads")?;
    dict.set_item(
        "topology_fingerprint",
        loads_func.call1((serde_json::to_string(&topology_fingerprint).unwrap(),))?,
    )?;

    if let Some(ref diag) = result.diagnostics {
        // Map diagnostics using serde to a Python dict
        if let Ok(diag_json) = serde_json::to_string(diag) {
            let py_diag = loads_func.call1((diag_json,))?;
            dict.set_item("diagnostics", py_diag)?;
        }
    }

    Ok(dict.into())
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
