use crate::types::{Coord3D, Line3D};
use crate::Polygonizer;
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;
use pyo3::types::PyDict;

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

    let mut lines = Vec::new();
    let stride_usize = stride as usize;

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

    let result = polygonizer.polygonize().map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Polygonization error: {}", e))
    })?;

    // Flatten logic
    let mut flat_coords = Vec::new();
    let mut ring_offsets = Vec::new();
    let mut polygon_offsets = Vec::new();
    let mut flat_line_ids = Vec::new();

    for poly in result.polygons {
        polygon_offsets.push(ring_offsets.len() as u32);

        let exterior = poly.exterior;
        let interiors = poly.interiors;

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

    let dict = PyDict::new_bound(py);
    dict.set_item("flat_coords", PyArray1::from_vec_bound(py, flat_coords))?;
    dict.set_item("ring_offsets", PyArray1::from_vec_bound(py, ring_offsets))?;
    dict.set_item(
        "polygon_offsets",
        PyArray1::from_vec_bound(py, polygon_offsets),
    )?;
    dict.set_item("flat_line_ids", PyArray1::from_vec_bound(py, flat_line_ids))?;
    dict.set_item("stride", stride)?;

    Ok(dict.into())
}

#[pymodule]
fn geo_polygonize_core(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(polygonize, m)?)?;
    Ok(())
}
