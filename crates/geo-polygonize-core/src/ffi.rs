use crate::arrow_api::{polygonize_arrow, PolygonizerOptions as ArrowOptions};
use arrow::datatypes::Field;
use arrow::ffi::{from_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use geoarrow::array::GeoArrowArray;
use std::convert::TryFrom;

#[repr(C)]
pub struct PolygonizerOptions {
    pub node_input: u8,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: u8,
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
#[no_mangle]
pub unsafe extern "C" fn polygonize_ffi(
    input_array: *mut FFI_ArrowArray,
    input_schema: *mut FFI_ArrowSchema,
    output_array: *mut FFI_ArrowArray,
    output_schema: *mut FFI_ArrowSchema,
    options: *const PolygonizerOptions,
) -> i32 {
    if input_array.is_null()
        || input_schema.is_null()
        || output_array.is_null()
        || output_schema.is_null()
        || options.is_null()
    {
        return 1;
    }

    let field = match Field::try_from(&*input_schema) {
        Ok(f) => f,
        Err(_) => return 2,
    };

    let arrow_data = match from_ffi(std::ptr::read(input_array), &*input_schema) {
        Ok(data) => data,
        Err(_) => return 2,
    };
    let array = arrow::array::make_array(arrow_data);

    let opts = &*options;
    let arrow_opts = ArrowOptions {
        node_input: opts.node_input != 0,
        snap_grid_size: opts.snap_grid_size,
        extract_only_polygonal: opts.extract_only_polygonal != 0,
    };

    match polygonize_arrow(array.as_ref(), &field, arrow_opts) {
        Ok(polygon_array) => {
            let array_ref = polygon_array.into_array_ref();
            let data = array_ref.to_data();

            let ffi_array = FFI_ArrowArray::new(&data);
            std::ptr::write(output_array, ffi_array);

            let ffi_schema = match FFI_ArrowSchema::try_from(data.data_type()) {
                Ok(s) => s,
                Err(_) => return 4,
            };
            std::ptr::write(output_schema, ffi_schema);

            0
        }
        Err(_) => 5,
    }
}
