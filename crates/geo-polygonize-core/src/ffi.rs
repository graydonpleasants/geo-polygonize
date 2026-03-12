use crate::arrow_api::{polygonize_arrow, PolygonizerOptions as ArrowOptions};
use arrow::datatypes::DataType;
use arrow::datatypes::Field;
use arrow::error::ArrowError;
use arrow::ffi::{from_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use geoarrow::array::GeoArrowArray;
use std::convert::TryFrom;

#[repr(C)]
pub struct PolygonizerOptions {
    pub node_input: u8,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: u8,
    pub report_mode: u8,
}

/// # Safety
/// This is a stub for the legacy CFFI bindings, as we migrated to an Arrow-only C API.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_free() {}

pub trait SchemaExporter {
    fn try_export(data_type: &DataType) -> Result<FFI_ArrowSchema, ArrowError>;
}

pub struct RealSchemaExporter;

impl SchemaExporter for RealSchemaExporter {
    fn try_export(data_type: &DataType) -> Result<FFI_ArrowSchema, ArrowError> {
        FFI_ArrowSchema::try_from(data_type)
    }
}

#[cfg(not(test))]
type DefaultSchemaExporter = RealSchemaExporter;
#[cfg(test)]
type DefaultSchemaExporter = MockSchemaExporter;

#[cfg(test)]
thread_local! {
    pub static MOCK_SCHEMA_ERROR: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
}

#[cfg(test)]
pub struct MockSchemaExporter;

#[cfg(test)]
impl SchemaExporter for MockSchemaExporter {
    fn try_export(data_type: &DataType) -> Result<FFI_ArrowSchema, ArrowError> {
        let should_error = MOCK_SCHEMA_ERROR.with(|f| *f.borrow());
        if should_error {
            Err(ArrowError::CDataInterface(
                "Mocked schema export error".to_string(),
            ))
        } else {
            FFI_ArrowSchema::try_from(data_type)
        }
    }
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
    std::panic::catch_unwind(|| {
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

        let array_val = std::ptr::replace(input_array, FFI_ArrowArray::empty());
        let arrow_data = match from_ffi(array_val, &*input_schema) {
            Ok(data) => data,
            Err(_) => return 2,
        };
        let array = arrow::array::make_array(arrow_data);

        let opts = &*options;
        let arrow_opts = ArrowOptions {
            node_input: opts.node_input != 0,
            snap_grid_size: opts.snap_grid_size,
            extract_only_polygonal: opts.extract_only_polygonal != 0,
            report_mode: opts.report_mode != 0,
        };

        match polygonize_arrow(array.as_ref(), &field, arrow_opts) {
            Ok(polygon_array) => {
                let array_ref = polygon_array.into_array_ref();
                let data = array_ref.to_data();

                let ffi_array = FFI_ArrowArray::new(&data);
                std::ptr::write(output_array, ffi_array);

                let ffi_schema = match DefaultSchemaExporter::try_export(data.data_type()) {
                    Ok(s) => s,
                    Err(_) => return 4,
                };
                std::ptr::write(output_schema, ffi_schema);

                0
            }
            Err(e) => match e {
                crate::error::PolygonizeError::InvalidBufferShape { .. } => 5,
                crate::error::PolygonizeError::InvalidArgumentType { .. } => 6,
                crate::error::PolygonizeError::InvalidGeometry { .. } => 7,
                crate::error::PolygonizeError::TopologyFailure { .. } => 8,
                crate::error::PolygonizeError::UnsupportedOptionCombination { .. } => 9,
                crate::error::PolygonizeError::InternalInvariantViolation { .. } => 10,
                crate::error::PolygonizeError::ArrowError(_) => 11,
                _ => 12,
            },
        }
    })
    .unwrap_or(99)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
    use geoarrow::array::GeoArrowArray;
    use geoarrow::datatypes::{Dimension, LineStringType};
    use std::sync::Arc;

    struct MockGuard;

    impl Drop for MockGuard {
        fn drop(&mut self) {
            MOCK_SCHEMA_ERROR.with(|f| *f.borrow_mut() = false);
        }
    }

    fn set_mock_error() -> MockGuard {
        MOCK_SCHEMA_ERROR.with(|f| *f.borrow_mut() = true);
        MockGuard
    }

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

    #[test]
    fn test_ffi_schema_export_error() {
        // Set mock to return error and use a drop guard to ensure it resets
        let _guard = set_mock_error();

        let typ = LineStringType::new(Dimension::XY, Arc::new(Default::default()));
        let builder = geoarrow::array::LineStringBuilder::new(typ);
        let input_arrow_array = builder.finish();

        let array_ref = input_arrow_array.into_array_ref();
        let (input_array, input_schema) = arrow::ffi::to_ffi(&array_ref.to_data()).unwrap();
        let mut input_array_ffi = std::mem::ManuallyDrop::new(input_array);
        let mut input_schema_ffi = std::mem::ManuallyDrop::new(input_schema);

        let mut output_array = FFI_ArrowArray::empty();
        let mut output_schema = FFI_ArrowSchema::empty();
        let options = PolygonizerOptions {
            node_input: 0,
            snap_grid_size: 1e-10,
            extract_only_polygonal: 0,
            report_mode: 0,
        };

        let status = unsafe {
            polygonize_ffi(
                &mut *input_array_ffi,
                &mut *input_schema_ffi,
                &mut output_array,
                &mut output_schema,
                &options,
            )
        };

        assert_eq!(status, 4);
    }
}
