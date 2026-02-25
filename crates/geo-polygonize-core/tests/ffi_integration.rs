use geo_polygonize_core::ffi::{
    polygonize_ffi, polygonize_result_free, polygonize_result_get_count, PolygonizerOptions,
};

#[test]
fn test_ffi_simple_square() {
    // Square: (0,0), (10,0), (10,10), (0,10), (0,0)
    let coords: Vec<f64> = vec![0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0];
    let offsets: Vec<u32> = vec![0, 5]; // One linestring with 5 points

    let options = PolygonizerOptions {
        node_input: false,
        snap_grid_size: 1e-10,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            offsets.as_ptr(),
            offsets.len(),
            options,
        )
    };

    assert!(!result_ptr.is_null());

    let count = unsafe { polygonize_result_get_count(result_ptr) };
    assert_eq!(count, 1);

    unsafe { polygonize_result_free(result_ptr) };
}

#[test]
fn test_ffi_invalid_bounds() {
    // Only 2 points (4 doubles) provided
    let coords: Vec<f64> = vec![0.0, 0.0, 10.0, 0.0];
    // Offsets claims 5 points (index 0 to 5)
    let offsets: Vec<u32> = vec![0, 5];

    let options = PolygonizerOptions {
        node_input: false,
        snap_grid_size: 1e-10,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            offsets.as_ptr(),
            offsets.len(),
            options,
        )
    };

    assert!(result_ptr.is_null());
}

#[test]
fn test_ffi_two_squares_touching() {
    // Two squares sharing an edge
    // Square 1: (0,0)-(10,0)-(10,10)-(0,10)-(0,0)
    // Square 2: (10,0)-(20,0)-(20,10)-(10,10)-(10,0)

    // We can pass them as 2 linestrings
    let coords: Vec<f64> = vec![
        // Square 1
        0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0, // Square 2
        10.0, 0.0, 20.0, 0.0, 20.0, 10.0, 10.0, 10.0, 10.0, 0.0,
    ];
    let offsets: Vec<u32> = vec![0, 5, 10]; // 2 linestrings, each 5 points

    let options = PolygonizerOptions {
        node_input: true, // Should dedup shared edge
        snap_grid_size: 1e-10,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            offsets.as_ptr(),
            offsets.len(),
            options,
        )
    };

    assert!(!result_ptr.is_null());

    let count = unsafe { polygonize_result_get_count(result_ptr) };
    // Should result in 2 polygons (squares)
    assert_eq!(count, 2);

    unsafe { polygonize_result_free(result_ptr) };
}

#[test]
fn test_ffi_accepts_null_empty_buffers() {
    let options = PolygonizerOptions {
        node_input: false,
        snap_grid_size: 1e-10,
    };

    let result_ptr = unsafe { polygonize_ffi(std::ptr::null(), 0, std::ptr::null(), 0, options) };

    assert!(!result_ptr.is_null());
    let count = unsafe { polygonize_result_get_count(result_ptr) };
    assert_eq!(count, 0);
    unsafe { polygonize_result_free(result_ptr) };
}

#[test]
fn test_ffi_rejects_out_of_bounds_offsets() {
    // One bad linestring (offset points past coords length)
    let coords: Vec<f64> = vec![0.0, 0.0, 1.0, 1.0];
    let offsets: Vec<u32> = vec![0, 3];

    let options = PolygonizerOptions {
        node_input: false,
        snap_grid_size: 1e-10,
    };

    let result_ptr = unsafe {
        polygonize_ffi(
            coords.as_ptr(),
            coords.len(),
            offsets.as_ptr(),
            offsets.len(),
            options,
        )
    };

    assert!(result_ptr.is_null());
}
