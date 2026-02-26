use geo_polygonize_core::ffi::{
    polygonize_ffi, polygonize_result_free, polygonize_result_get_count,
    polygonize_result_get_shell_point_count, polygonize_result_get_shell_points, CPolygonStatus,
    PolygonizerOptions,
};
use std::slice;

#[test]
fn test_ffi_simple_square() {
    let coords = vec![0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0];
    let offsets = vec![0, 5]; // 1 linestring with 5 points

    let options = PolygonizerOptions {
        node_input: 0,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            2, // Stride
            offsets.as_ptr(),
            offsets.len(),
            &options,
        )
    };

    assert!(!result_ptr.is_null());

    let count = unsafe { polygonize_result_get_count(result_ptr) };
    assert_eq!(count, 1);

    let shell_pts = unsafe { polygonize_result_get_shell_point_count(result_ptr, 0) };
    assert_eq!(shell_pts, 5);

    let mut buffer = vec![0.0; shell_pts * 3]; // 3D
    unsafe {
        polygonize_result_get_shell_points(result_ptr, 0, buffer.as_mut_ptr());
    }

    // Verify first point (0,0,0)
    assert_eq!(buffer[0], 0.0);
    assert_eq!(buffer[1], 0.0);
    assert_eq!(buffer[2], 0.0);

    unsafe {
        polygonize_result_free(result_ptr);
    }
}

#[test]
fn test_ffi_noding() {
    // Frame + Cross
    let coords = vec![
        0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0, // Frame
        0.0, 0.0, 10.0, 10.0, // Diag 1
        0.0, 10.0, 10.0, 0.0, // Diag 2
    ];
    let offsets = vec![0, 5, 7, 9]; // 3 lines

    let options = PolygonizerOptions {
        node_input: 1,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            2, // Stride
            offsets.as_ptr(),
            offsets.len(),
            &options,
        )
    };

    assert!(!result_ptr.is_null());
    let count = unsafe { polygonize_result_get_count(result_ptr) };
    assert_eq!(count, 4);

    unsafe {
        polygonize_result_free(result_ptr);
    }
}

#[test]
fn test_ffi_invalid_input() {
    // Even length but invalid linestring (1 point)
    let coords = vec![0.0, 0.0];
    let offsets = vec![0, 1]; // 1 point

    let options = PolygonizerOptions {
        node_input: 0,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            2, // Stride
            offsets.as_ptr(),
            offsets.len(),
            &options,
        )
    };

    assert!(!result_ptr.is_null());
    let count = unsafe { polygonize_result_get_count(result_ptr) };
    assert_eq!(count, 0);

    unsafe {
        polygonize_result_free(result_ptr);
    }
}

#[test]
fn test_ffi_null_pointers() {
    let options = PolygonizerOptions {
        node_input: 0,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
    };

    let result_ptr =
        unsafe { polygonize_ffi(std::ptr::null(), 0, 2, std::ptr::null(), 0, &options) };
    assert!(!result_ptr.is_null());
    assert_eq!(unsafe { polygonize_result_get_count(result_ptr) }, 0);
    unsafe { polygonize_result_free(result_ptr) };
}

#[test]
fn test_ffi_odd_coordinates() {
    let coords = vec![0.0, 0.0, 10.0]; // 3 coords?
    let offsets = vec![0, 1];

    let options = PolygonizerOptions {
        node_input: 0,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            2,
            offsets.as_ptr(),
            offsets.len(),
            &options,
        )
    };

    // Should be null due to stride check
    assert!(result_ptr.is_null());
}
