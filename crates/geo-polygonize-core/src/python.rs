use crate::Polygonizer;
use geo_types::{Coord, Line};
use numpy::PyReadonlyArray1;
use pyo3::prelude::*;

#[pyfunction]
#[pyo3(signature = (coords, offsets, node=false, snap=1e-10))]
fn polygonize<'py>(
    py: Python<'py>,
    coords: PyReadonlyArray1<'py, f64>,
    offsets: PyReadonlyArray1<'py, u32>,
    node: bool,
    snap: f64,
) -> PyResult<Vec<PyObject>> {
    let coords_slice = coords.as_slice()?;
    let offsets_slice = offsets.as_slice()?;

    if coords_slice.len() % 2 != 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "Coords array must have even length",
        ));
    }

    if offsets_slice.len() < 2 {
        return Ok(Vec::new());
    }

    let mut lines = Vec::new();
    for i in 0..offsets_slice.len() - 1 {
        let start_idx = offsets_slice[i] as usize;
        let end_idx = offsets_slice[i + 1] as usize;

        if start_idx > end_idx {
            continue;
        }

        // Bounds check
        if end_idx.saturating_mul(2) > coords_slice.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Offset index out of bounds",
            ));
        }

        if start_idx == end_idx {
            continue;
        }

        for j in start_idx..end_idx - 1 {
            let p1 = Coord {
                x: coords_slice[2 * j],
                y: coords_slice[2 * j + 1],
            };
            let p2 = Coord {
                x: coords_slice[2 * (j + 1)],
                y: coords_slice[2 * (j + 1) + 1],
            };
            lines.push(Line::new(p1, p2));
        }
    }

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = node;
    polygonizer.snap_grid_size = snap;
    polygonizer.add_lines(lines);

    let polygons = polygonizer
        .polygonize()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    // Import SimplePolygon class
    let types_mod = PyModule::import_bound(py, "geo_polygonize.types")?;
    let simple_polygon_cls = types_mod.getattr("SimplePolygon")?;

    let mut results = Vec::with_capacity(polygons.len());

    for poly in polygons {
        let exterior = poly.exterior();
        let shell_coords: Vec<(f64, f64)> = exterior.0.iter().map(|c| (c.x, c.y)).collect();

        let mut holes_list = Vec::new();
        for interior in poly.interiors() {
            let hole_coords: Vec<(f64, f64)> = interior.0.iter().map(|c| (c.x, c.y)).collect();
            holes_list.push(hole_coords);
        }

        let instance = simple_polygon_cls.call1((shell_coords, holes_list))?;
        results.push(instance.into());
    }

    Ok(results)
}

#[pymodule]
fn geo_polygonize_core(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(polygonize, m)?)?;
    Ok(())
}
