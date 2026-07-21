#![cfg(feature = "arrow")]

use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
use geo_polygonize_core::ffi::{polygonize_ffi, PolygonizerOptions};

#[test]
fn test_ffi_null_pointers() {
    let mut array = FFI_ArrowArray::empty();
    let mut schema = FFI_ArrowSchema::empty();
    let options = PolygonizerOptions {
        node_input: 0,
        snap_grid_size: 1e-10,
        extract_only_polygonal: 0,
        report_mode: 0,
    };

    let status = unsafe {
        polygonize_ffi(
            std::ptr::null_mut(),
            &mut schema,
            &mut array,
            &mut schema,
            &options,
        )
    };
    assert_eq!(status, 1);
}
