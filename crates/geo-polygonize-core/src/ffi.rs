use crate::Polygonizer;
use geo_types::{Coord, Line, Polygon};
use std::slice;

#[repr(C)]
pub struct PolygonizerOptions {
    pub node_input: bool,
    pub snap_grid_size: f64,
}

#[repr(i32)]
#[derive(Clone, Copy)]
pub enum CPolygonStatus {
    Success = 0,
    InvalidInput = 1,
    InternalError = 2,
}

pub struct CPolygonResult {
    pub polygons: Vec<Polygon<f64>>,
    pub status: CPolygonStatus,
}

/// Helper to ingest raw data and run polygonization
///
/// `coords_ptr`: Pointer to flat array of f64 coordinates [x0, y0, x1, y1, ...]
/// `coords_len`: Number of f64 values (should be even)
/// `offsets_ptr`: Pointer to u32 offsets defining linestrings.
/// `offsets_len`: Number of offsets.
///
/// This assumes Arrow-like offsets: `offsets` has length `N+1` for `N` linestrings.
/// The `i`-th linestring consists of points from index `offsets[i]` to `offsets[i+1]`.
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that `coords_ptr` and `offsets_ptr` are valid for the given lengths.
#[no_mangle]
pub unsafe extern "C" fn polygonize_ffi(
    coords_ptr: *const f64,
    coords_len: usize,
    offsets_ptr: *const u32,
    offsets_len: usize,
    options: PolygonizerOptions,
) -> *mut CPolygonResult {
    if (coords_ptr.is_null() && coords_len > 0) || (offsets_ptr.is_null() && offsets_len > 0) {
        return std::ptr::null_mut();
    }

    if coords_len % 2 != 0 {
        return std::ptr::null_mut();
    }

    // Safety: The caller must ensure pointers are valid for the given lengths.
    // For empty buffers, allow null pointers (common FFI convention).
    let coords = if coords_len == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(coords_ptr, coords_len) }
    };
    let offsets = if offsets_len == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(offsets_ptr, offsets_len) }
    };

    if offsets_len < 2 {
        // No lines can be defined with < 2 offsets
        return Box::into_raw(Box::new(CPolygonResult {
            polygons: Vec::new(),
            status: CPolygonStatus::Success,
        }));
    }

    let mut lines = Vec::new();
    let mut status = CPolygonStatus::Success;

    // Iterate through linestrings defined by offsets
    for i in 0..offsets_len - 1 {
        let start_idx = offsets[i] as usize;
        let end_idx = offsets[i + 1] as usize;

        if start_idx > end_idx {
            return std::ptr::null_mut();
        }

        if start_idx == end_idx {
            continue;
        }

        // Check bounds: indices refer to points, each point is 2 f64s
        if end_idx * 2 > coords_len {
            return std::ptr::null_mut();
        }

        // Create segments for this linestring
        for j in start_idx..end_idx - 1 {
            let p1 = Coord {
                x: coords[2 * j],
                y: coords[2 * j + 1],
            };
            let p2 = Coord {
                x: coords[2 * (j + 1)],
                y: coords[2 * (j + 1) + 1],
            };
            lines.push(Line::new(p1, p2));
        }
    }

    if let CPolygonStatus::InvalidInput = status {
        return Box::into_raw(Box::new(CPolygonResult {
            polygons: Vec::new(),
            status,
        }));
    }

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = options.node_input;
    polygonizer.snap_grid_size = options.snap_grid_size;
    polygonizer.add_lines(lines);

    match polygonizer.polygonize() {
        Ok(polygons) => {
            let res = CPolygonResult {
                polygons,
                status: CPolygonStatus::Success,
            };
            Box::into_raw(Box::new(res))
        }
        Err(_) => Box::into_raw(Box::new(CPolygonResult {
            polygons: Vec::new(),
            status: CPolygonStatus::InternalError,
        })),
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer.
/// The caller must ensure that `res` is a valid pointer to a `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_count(res: *const CPolygonResult) -> usize {
    if res.is_null() {
        return 0;
    }
    unsafe { (*res).polygons.len() }
}

/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer.
/// The caller must ensure that `res` is a valid pointer to a `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_status(res: *const CPolygonResult) -> i32 {
    if res.is_null() {
        return -1; // Null pointer error
    }
    unsafe { (*res).status as i32 }
}

/// # Safety
///
/// This function is unsafe because it dereferences and drops a raw pointer.
/// The caller must ensure that `res` is a valid pointer obtained from `polygonize_ffi`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_free(res: *mut CPolygonResult) {
    if !res.is_null() {
        unsafe { drop(Box::from_raw(res)) };
    }
}
