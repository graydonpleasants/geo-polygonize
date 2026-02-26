use crate::ffi::{
    polygonize_ffi, polygonize_result_copy_flat_coords, polygonize_result_copy_flat_line_ids,
    polygonize_result_copy_polygon_offsets, polygonize_result_copy_ring_offsets,
    polygonize_result_free, polygonize_result_get_flat_coords_len,
    polygonize_result_get_flat_line_ids_len, polygonize_result_get_polygon_offsets_len,
    polygonize_result_get_ring_offsets_len, polygonize_result_get_status, PolygonizerOptions,
};
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

    let (line_ids_ptr, line_ids_len) = if let Some(arr) = line_ids {
        let slice = arr.as_slice()?;
        (slice.as_ptr(), slice.len())
    } else {
        (std::ptr::null(), 0)
    };

    let options = PolygonizerOptions {
        node_input: if node { 1 } else { 0 },
        snap_grid_size: snap,
        extract_only_polygonal: if extract_only_polygonal { 1 } else { 0 },
    };

    let res_ptr = unsafe {
        polygonize_ffi(
            coords_slice.as_ptr(),
            coords_slice.len(),
            offsets_slice.as_ptr(),
            offsets_slice.len(),
            line_ids_ptr,
            line_ids_len,
            stride,
            &options,
        )
    };

    if res_ptr.is_null() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "polygonize_ffi returned NULL",
        ));
    }

    let status = unsafe { polygonize_result_get_status(res_ptr) };
    if status != 0 {
        unsafe { polygonize_result_free(res_ptr) };
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "polygonization failed",
        ));
    }

    let coords_len = unsafe { polygonize_result_get_flat_coords_len(res_ptr) };
    let ring_len = unsafe { polygonize_result_get_ring_offsets_len(res_ptr) };
    let poly_len = unsafe { polygonize_result_get_polygon_offsets_len(res_ptr) };
    let line_ids_len = unsafe { polygonize_result_get_flat_line_ids_len(res_ptr) };

    let mut flat = vec![0.0; coords_len];
    let mut ring_offsets = vec![0u32; ring_len];
    let mut polygon_offsets = vec![0u32; poly_len];
    let mut flat_line_ids = vec![0u32; line_ids_len];

    unsafe {
        polygonize_result_copy_flat_coords(res_ptr, flat.as_mut_ptr());
        polygonize_result_copy_ring_offsets(res_ptr, ring_offsets.as_mut_ptr());
        polygonize_result_copy_polygon_offsets(res_ptr, polygon_offsets.as_mut_ptr());
        polygonize_result_copy_flat_line_ids(res_ptr, flat_line_ids.as_mut_ptr());
        polygonize_result_free(res_ptr);
    }

    let dict = PyDict::new_bound(py);

    dict.set_item("flat_coords", PyArray1::from_vec_bound(py, flat))?;
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
