use crate::types::{Coord3D, Line3D};
use crate::Polygonizer;
use crate::error::PolygonizerError;
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use pyo3::exceptions::{PyValueError, PyRuntimeError};
use pyo3::create_exception;

create_exception!(geo_polygonize_core, TopologyError, pyo3::exceptions::PyException);
create_exception!(geo_polygonize_core, InvalidGeometryError, pyo3::exceptions::PyException);
create_exception!(geo_polygonize_core, NodingError, pyo3::exceptions::PyException);

impl std::convert::From<PolygonizerError> for PyErr {
    fn from(err: PolygonizerError) -> PyErr {
        match err {
            PolygonizerError::TopologyError(msg) => TopologyError::new_err(msg),
            PolygonizerError::InvalidGeometry(msg) => InvalidGeometryError::new_err(msg),
            PolygonizerError::NodingError(msg) => NodingError::new_err(msg),
            PolygonizerError::ArrowError(msg) => PyValueError::new_err(msg),
            PolygonizerError::NullPointer(msg) => PyValueError::new_err(msg),
            PolygonizerError::Panic(msg) => PyRuntimeError::new_err(msg),
        }
    }
}

#[pyfunction]
#[pyo3(signature = (coords, offsets, node=false, snap=1e-10, extract_only_polygonal=false, stride=2, line_ids=None))]
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
) -> PyResult<PyObject> {
    let coords_slice = coords.as_slice()?;
    let offsets_slice = offsets.as_slice()?;

    if stride != 2 && stride != 3 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "stride must be 2 or 3",
        ));
    }

    let stride_usize = stride as usize;
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
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Invalid offsets: start offset ({}) is greater than end offset ({}) at index {}",
                    start, end, i
                )));
            }
            if end * stride_usize > coords_slice.len() {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Invalid offsets: calculated end offset {} exceeds coordinate capacity {} for stride {}",
                    end * stride_usize, coords_slice.len(), stride_usize
                )));
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

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = node;
    polygonizer.snap_grid_size = snap;
    polygonizer.extract_only_polygonal = extract_only_polygonal;
    polygonizer.add_lines(lines);

    let result = polygonizer.polygonize()?;

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
    let py_polygons = PyList::empty_bound(py);

    // Attempt to import the Python class `SimplePolygon`
    let simple_polygon_cls = py
        .import_bound("geo_polygonize.types")?
        .getattr("SimplePolygon")?;

    for poly in &result.polygons {
        // Construct exterior tuples
        let exterior_pts = PyList::empty_bound(py);
        for c in &poly.exterior {
            if stride == 3 {
                exterior_pts.append(PyTuple::new_bound(py, &[c.x, c.y, c.z]))?;
            } else {
                exterior_pts.append(PyTuple::new_bound(py, &[c.x, c.y]))?;
            }
        }
        let shell = PyTuple::new_bound(py, exterior_pts);

        let shell_ids = PyTuple::new_bound(py, &poly.exterior_ids);

        // Construct interiors
        let holes = PyList::empty_bound(py);
        let holes_ids = PyList::empty_bound(py);

        for (h_idx, ring) in poly.interiors.iter().enumerate() {
            let ring_pts = PyList::empty_bound(py);
            for c in ring {
                if stride == 3 {
                    ring_pts.append(PyTuple::new_bound(py, &[c.x, c.y, c.z]))?;
                } else {
                    ring_pts.append(PyTuple::new_bound(py, &[c.x, c.y]))?;
                }
            }
            holes.append(PyTuple::new_bound(py, ring_pts))?;

            let r_ids = PyTuple::new_bound(py, &poly.interiors_ids[h_idx]);
            holes_ids.append(r_ids)?;
        }

        let py_poly = simple_polygon_cls.call1((shell, holes, shell_ids, holes_ids))?;
        py_polygons.append(py_poly)?;
    }

    // Construct dangles
    let py_dangles = PyList::empty_bound(py);
    for dangle in &result.dangles {
        let dangle_pts = PyList::empty_bound(py);
        for c in dangle {
            if stride == 3 {
                dangle_pts.append(PyTuple::new_bound(py, &[c.x, c.y, c.z]))?;
            } else {
                dangle_pts.append(PyTuple::new_bound(py, &[c.x, c.y]))?;
            }
        }
        py_dangles.append(PyTuple::new_bound(py, dangle_pts))?;
    }

    // Construct invalid rings
    let py_invalid_rings = PyList::empty_bound(py);
    for invalid_ring in &result.invalid_rings {
        let invalid_pts = PyList::empty_bound(py);
        for c in invalid_ring {
            if stride == 3 {
                invalid_pts.append(PyTuple::new_bound(py, &[c.x, c.y, c.z]))?;
            } else {
                invalid_pts.append(PyTuple::new_bound(py, &[c.x, c.y]))?;
            }
        }
        py_invalid_rings.append(PyTuple::new_bound(py, invalid_pts))?;
    }

    let dict = PyDict::new_bound(py);
    dict.set_item("flat_coords", PyArray1::from_vec_bound(py, flat_coords))?;
    dict.set_item("ring_offsets", PyArray1::from_vec_bound(py, ring_offsets))?;
    dict.set_item(
        "polygon_offsets",
        PyArray1::from_vec_bound(py, polygon_offsets),
    )?;
    dict.set_item("flat_line_ids", PyArray1::from_vec_bound(py, flat_line_ids))?;
    dict.set_item("stride", stride)?;

    dict.set_item("polygons", py_polygons)?;
    dict.set_item("dangles", py_dangles)?;
    dict.set_item("invalid_rings", py_invalid_rings)?;

    Ok(dict.into())
}

#[pymodule]
fn geo_polygonize_core(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("TopologyError", py.get_type_bound::<TopologyError>())?;
    m.add("InvalidGeometryError", py.get_type_bound::<InvalidGeometryError>())?;
    m.add("NodingError", py.get_type_bound::<NodingError>())?;
    m.add_function(wrap_pyfunction!(polygonize, m)?)?;
    Ok(())
}
