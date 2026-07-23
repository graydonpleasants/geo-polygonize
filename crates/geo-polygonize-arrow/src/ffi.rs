use crate::polygonize_arrow;
use arrow::array::Array;
use arrow::datatypes::Field;
use arrow::error::ArrowError;
use arrow::ffi::{from_ffi_and_data_type, FFI_ArrowArray, FFI_ArrowSchema};
use geo_polygonize_core::{
    normalize_polygonize_error, PolygonizeError, PolygonizerOptions as CoreOptions, PrecisionModel,
};
use geoarrow::array::IntoArrow;
use std::convert::TryFrom;
use std::ffi::{c_char, CString};

pub const POLYGONIZE_FFI_ABI_VERSION: u32 = 1;

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolygonizeFfiStatus {
    Success = 0,
    InvalidArgument = 1,
    InvalidArrowCData = 2,
    SchemaExport = 4,
    InvalidBufferShape = 5,
    InvalidOption = 6,
    InvalidGeometry = 7,
    Topology = 8,
    UnsupportedOptionCombination = 9,
    InternalInvariant = 10,
    Arrow = 11,
    Unknown = 12,
    Panic = 99,
}

#[repr(C)]
pub struct PolygonizeFfiLastError {
    pub status: i32,
    pub family: *const c_char,
    pub stage: *const c_char,
    pub message: *const c_char,
    pub witness: *const c_char,
}

struct LastErrorStorage {
    _family: CString,
    _stage: CString,
    _message: CString,
    _witness: CString,
    value: PolygonizeFfiLastError,
}

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<LastErrorStorage>> = const { std::cell::RefCell::new(None) };
}

fn c_string(value: &str) -> CString {
    CString::new(value).unwrap_or_else(|_| CString::new("invalid error text").unwrap())
}

fn clear_last_error() {
    LAST_ERROR.with(|last| *last.borrow_mut() = None);
}

fn set_last_error(
    status: PolygonizeFfiStatus,
    family: &str,
    stage: &str,
    message: &str,
    witness: &str,
) -> i32 {
    let family = c_string(family);
    let stage = c_string(stage);
    let message = c_string(message);
    let witness = c_string(witness);
    let value = PolygonizeFfiLastError {
        status: status as i32,
        family: family.as_ptr(),
        stage: stage.as_ptr(),
        message: message.as_ptr(),
        witness: witness.as_ptr(),
    };
    LAST_ERROR.with(|last| {
        *last.borrow_mut() = Some(LastErrorStorage {
            _family: family,
            _stage: stage,
            _message: message,
            _witness: witness,
            value,
        });
    });
    status as i32
}

fn set_static_error(status: PolygonizeFfiStatus, family: &str, stage: &str, message: &str) -> i32 {
    set_last_error(status, family, stage, message, "")
}

fn set_polygonize_error(error: &PolygonizeError) -> i32 {
    let normalized = normalize_polygonize_error(error);
    let status = match error {
        PolygonizeError::InvalidBufferShape { .. } => PolygonizeFfiStatus::InvalidBufferShape,
        PolygonizeError::ResourceLimitExceeded { .. } => PolygonizeFfiStatus::Unknown,
        PolygonizeError::Cancelled { .. } => PolygonizeFfiStatus::Unknown,
        PolygonizeError::InvalidArgumentType { .. } => PolygonizeFfiStatus::InvalidOption,
        PolygonizeError::InvalidGeometry { .. } => PolygonizeFfiStatus::InvalidGeometry,
        PolygonizeError::TopologyFailure { .. }
        | PolygonizeError::ZConflict { .. }
        | PolygonizeError::NodingValidationFailure { .. } => PolygonizeFfiStatus::Topology,
        PolygonizeError::UnsupportedOptionCombination { .. } => {
            PolygonizeFfiStatus::UnsupportedOptionCombination
        }
        PolygonizeError::InternalInvariantViolation { .. } => {
            PolygonizeFfiStatus::InternalInvariant
        }
        PolygonizeError::ArrowError(_) => PolygonizeFfiStatus::Arrow,
        PolygonizeError::NullPointer(_) => PolygonizeFfiStatus::InvalidArgument,
        PolygonizeError::Panic(_) => PolygonizeFfiStatus::Panic,
    };
    let witness = normalized
        .witness
        .map(|value| serde_json::to_string(&value).unwrap())
        .unwrap_or_default();
    set_last_error(
        status,
        &normalized.family,
        &normalized.stage,
        &error.to_string(),
        &witness,
    )
}

#[no_mangle]
pub extern "C" fn polygonize_ffi_abi_version() -> u32 {
    POLYGONIZE_FFI_ABI_VERSION
}

/// Returns the current thread's most recent FFI error, or null after success.
///
/// The returned pointer remains valid until the next polygonize FFI call on the
/// same thread. Callers must copy any strings they need to retain.
#[no_mangle]
pub extern "C" fn polygonize_ffi_last_error() -> *const PolygonizeFfiLastError {
    LAST_ERROR.with(|last| {
        last.borrow()
            .as_ref()
            .map(|error| &error.value as *const PolygonizeFfiLastError)
            .unwrap_or(std::ptr::null())
    })
}

#[repr(C)]
pub struct PolygonizerOptions {
    pub node_input: u8,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: u8,
    pub report_mode: u8,
}

/// # Safety
/// This is a stub for the legacy CFFI bindings, as we migrated to an Arrow-only C API.
/// This function is currently a no-op, but it is marked unsafe to match legacy FFI signatures.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_free() {}

pub trait SchemaExporter {
    fn try_export(field: &Field) -> Result<FFI_ArrowSchema, ArrowError>;
}

pub struct RealSchemaExporter;

impl SchemaExporter for RealSchemaExporter {
    fn try_export(field: &Field) -> Result<FFI_ArrowSchema, ArrowError> {
        FFI_ArrowSchema::try_from(field)
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
    fn try_export(field: &Field) -> Result<FFI_ArrowSchema, ArrowError> {
        let should_error = MOCK_SCHEMA_ERROR.with(|f| *f.borrow());
        if should_error {
            Err(ArrowError::CDataInterface(
                "Mocked schema export error".to_string(),
            ))
        } else {
            FFI_ArrowSchema::try_from(field)
        }
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
///
/// **Invariants & Rationale:**
/// - `input_array` and `input_schema` must be valid, non-null pointers to valid Arrow C Data Interface structures.
/// - `output_array` and `output_schema` must be valid, non-null pointers to allocated but uninitialized or safe-to-overwrite Arrow C Data Interface structures.
/// - `options` must be a valid, non-null pointer to a `PolygonizerOptions` struct.
/// - A valid input array is consumed after its schema is accepted; output ownership transfers only on success.
/// - We use `std::panic::catch_unwind` at this boundary to ensure panics don't cross the FFI boundary, returning a defined error code (99) instead.
///
/// See `docs/C_ABI.md` for the complete ABI and ownership contract.
#[no_mangle]
pub unsafe extern "C" fn polygonize_ffi(
    input_array: *mut FFI_ArrowArray,
    input_schema: *mut FFI_ArrowSchema,
    output_array: *mut FFI_ArrowArray,
    output_schema: *mut FFI_ArrowSchema,
    options: *const PolygonizerOptions,
) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        if options.is_null() {
            return set_static_error(
                PolygonizeFfiStatus::InvalidArgument,
                "invalid_argument",
                "options",
                "options must not be null",
            );
        }
        let opts = &*options;
        let node_input = opts.node_input != 0;
        let mut arrow_opts = CoreOptions {
            node_input,
            precision_model: if node_input {
                PrecisionModel::from_grid_size(opts.snap_grid_size)
            } else {
                PrecisionModel::Floating
            },
            extract_only_polygonal: opts.extract_only_polygonal != 0,
            ..Default::default()
        };
        arrow_opts.diagnostics.enabled = opts.report_mode != 0;
        arrow_opts.diagnostics.report_mode = opts.report_mode != 0;

        polygonize_ffi_internal(
            input_array,
            input_schema,
            output_array,
            output_schema,
            arrow_opts,
        )
    }))
    .unwrap_or_else(|_| {
        set_static_error(
            PolygonizeFfiStatus::Panic,
            "internal",
            "boundary",
            "panic crossed the FFI boundary",
        )
    })
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
///
/// `options_json` is borrowed for this call. A valid input array is consumed
/// after schema validation, while outputs transfer only on success.
///
/// We use `std::panic::catch_unwind` at this boundary to ensure panics don't cross the FFI boundary, returning a defined error code (99) instead.
#[no_mangle]
pub unsafe extern "C" fn polygonize_with_options_ffi(
    input_array: *mut FFI_ArrowArray,
    input_schema: *mut FFI_ArrowSchema,
    output_array: *mut FFI_ArrowArray,
    output_schema: *mut FFI_ArrowSchema,
    options_json: *const u8,
    options_json_len: usize,
) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        if options_json.is_null() {
            return set_static_error(
                PolygonizeFfiStatus::InvalidArgument,
                "invalid_argument",
                "options",
                "options_json must not be null",
            );
        }

        let slice = std::slice::from_raw_parts(options_json, options_json_len);
        let options_str = match std::str::from_utf8(slice) {
            Ok(s) => s,
            Err(_) => {
                return set_static_error(
                    PolygonizeFfiStatus::InvalidArgument,
                    "invalid_argument",
                    "options",
                    "options_json must be valid UTF-8",
                )
            }
        };

        let arrow_opts: CoreOptions = match serde_json::from_str(options_str) {
            Ok(o) => o,
            Err(_) => {
                return set_static_error(
                    PolygonizeFfiStatus::InvalidArgument,
                    "invalid_argument",
                    "options",
                    "options_json must match PolygonizerOptions",
                )
            }
        };

        polygonize_ffi_internal(
            input_array,
            input_schema,
            output_array,
            output_schema,
            arrow_opts,
        )
    }))
    .unwrap_or_else(|_| {
        set_static_error(
            PolygonizeFfiStatus::Panic,
            "internal",
            "boundary",
            "panic crossed the FFI boundary",
        )
    })
}

unsafe fn polygonize_ffi_internal(
    input_array: *mut FFI_ArrowArray,
    input_schema: *mut FFI_ArrowSchema,
    output_array: *mut FFI_ArrowArray,
    output_schema: *mut FFI_ArrowSchema,
    arrow_opts: CoreOptions,
) -> i32 {
    if input_array.is_null()
        || input_schema.is_null()
        || output_array.is_null()
        || output_schema.is_null()
    {
        return set_static_error(
            PolygonizeFfiStatus::InvalidArgument,
            "invalid_argument",
            "ffi",
            "input and output pointers must not be null",
        );
    }

    let field = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Field::try_from(&*input_schema)
    })) {
        Ok(Ok(field)) => field,
        Ok(Err(_)) | Err(_) => {
            return set_static_error(
                PolygonizeFfiStatus::InvalidArrowCData,
                "arrow_c_data",
                "input",
                "input schema is invalid",
            )
        }
    };

    let array_val = std::ptr::replace(input_array, FFI_ArrowArray::empty());
    let arrow_data = match from_ffi_and_data_type(array_val, field.data_type().clone()) {
        Ok(data) => data,
        Err(_) => {
            return set_static_error(
                PolygonizeFfiStatus::InvalidArrowCData,
                "arrow_c_data",
                "input",
                "input array is invalid",
            )
        }
    };
    let array = arrow::array::make_array(arrow_data);

    match polygonize_arrow(array.as_ref(), &field, arrow_opts) {
        Ok(polygon_array) => {
            let extension_type = polygon_array.extension_type().clone();
            let arrow_array = polygon_array.into_arrow();
            let field = Field::new("geometry", arrow_array.data_type().clone(), true)
                .with_extension_type(extension_type);
            let data = arrow_array.into_data();

            let ffi_array = FFI_ArrowArray::new(&data);
            let ffi_schema = match DefaultSchemaExporter::try_export(&field) {
                Ok(s) => s,
                Err(_) => {
                    return set_static_error(
                        PolygonizeFfiStatus::SchemaExport,
                        "arrow_c_data",
                        "output",
                        "output schema export failed",
                    )
                }
            };

            std::ptr::write(output_array, ffi_array);
            std::ptr::write(output_schema, ffi_schema);

            clear_last_error();
            PolygonizeFfiStatus::Success as i32
        }
        Err(e) => set_polygonize_error(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
    use geoarrow::array::GeoArrowArray;
    use geoarrow::datatypes::{Dimension, LineStringType};
    use std::ffi::CStr;
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
        assert_eq!(status, PolygonizeFfiStatus::InvalidArgument as i32);
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
        let output_array_before = format!("{output_array:?}");
        let output_schema_before = format!("{output_schema:?}");
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

        assert_eq!(status, PolygonizeFfiStatus::SchemaExport as i32);
        assert_eq!(
            format!("{:?}", &*input_array_ffi),
            format!("{:?}", FFI_ArrowArray::empty())
        );
        assert_eq!(format!("{output_array:?}"), output_array_before);
        assert_eq!(format!("{output_schema:?}"), output_schema_before);
    }

    #[test]
    fn test_ffi_invalid_schema_keeps_input_and_outputs() {
        let typ = LineStringType::new(Dimension::XY, Arc::new(Default::default()));
        let input_arrow_array = geoarrow::array::LineStringBuilder::new(typ).finish();
        let array_ref = input_arrow_array.into_array_ref();
        let (mut input_array, _) = arrow::ffi::to_ffi(&array_ref.to_data()).unwrap();
        let mut input_schema = FFI_ArrowSchema::empty();
        let input_array_before = format!("{input_array:?}");
        let mut output_array = FFI_ArrowArray::empty();
        let mut output_schema = FFI_ArrowSchema::empty();
        let output_array_before = format!("{output_array:?}");
        let output_schema_before = format!("{output_schema:?}");
        let options = PolygonizerOptions {
            node_input: 0,
            snap_grid_size: 1e-10,
            extract_only_polygonal: 0,
            report_mode: 0,
        };

        let status = unsafe {
            polygonize_ffi(
                &mut input_array,
                &mut input_schema,
                &mut output_array,
                &mut output_schema,
                &options,
            )
        };

        assert_eq!(status, PolygonizeFfiStatus::InvalidArrowCData as i32);
        assert_eq!(format!("{input_array:?}"), input_array_before);
        assert_eq!(format!("{output_array:?}"), output_array_before);
        assert_eq!(format!("{output_schema:?}"), output_schema_before);
    }

    #[test]
    fn test_ffi_version_and_last_error() {
        assert_eq!(polygonize_ffi_abi_version(), POLYGONIZE_FFI_ABI_VERSION);

        let status = unsafe {
            polygonize_with_options_ffi(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
            )
        };
        assert_eq!(status, PolygonizeFfiStatus::InvalidArgument as i32);

        let error = unsafe { &*polygonize_ffi_last_error() };
        assert_eq!(error.status, status);
        assert_eq!(
            unsafe { CStr::from_ptr(error.family) }.to_str().unwrap(),
            "invalid_argument"
        );
        assert_eq!(
            unsafe { CStr::from_ptr(error.stage) }.to_str().unwrap(),
            "options"
        );
    }

    #[test]
    fn test_ffi_last_error_includes_noding_witness() {
        set_polygonize_error(&PolygonizeError::NodingValidationFailure {
            first_segment: 3,
            second_segment: 8,
            reason: "intersection".to_string(),
        });

        let error = unsafe { &*polygonize_ffi_last_error() };
        assert_eq!(error.status, PolygonizeFfiStatus::Topology as i32);
        assert_eq!(
            unsafe { CStr::from_ptr(error.family) }.to_str().unwrap(),
            "topology"
        );
        assert_eq!(
            unsafe { CStr::from_ptr(error.witness) }.to_str().unwrap(),
            r#"{"ids":["0x0000000000000003","0x0000000000000008"],"coordinate":null}"#
        );
    }
}
