use crate::Polygonizer;
use geo_types::{Coord, Line, LineString, Polygon};
use std::slice;

#[repr(C)]
pub struct PolygonizerOptions {
    pub node_input: bool,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: bool,
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
    pub dangles: Vec<LineString<f64>>,
    pub invalid_rings: Vec<LineString<f64>>,
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

    #[allow(clippy::manual_is_multiple_of)]
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
            dangles: Vec::new(),
            invalid_rings: Vec::new(),
            status: CPolygonStatus::Success,
        }));
    }

    let mut lines = Vec::new();

    // Iterate through linestrings defined by offsets
    for i in 0..offsets_len - 1 {
        // Offsets are indices of POINTS (pairs of f64), so multiply by 2 to get float index
        // Use saturating_mul to avoid overflow panics
        let start_point_idx = offsets[i] as usize;
        let end_point_idx = offsets[i + 1] as usize;

        let _start_idx = start_point_idx.saturating_mul(2);
        let end_idx = end_point_idx.saturating_mul(2);

        if start_point_idx > end_point_idx {
            // Invalid offset range
            return Box::into_raw(Box::new(CPolygonResult {
                polygons: Vec::new(),
                dangles: Vec::new(),
                invalid_rings: Vec::new(),
                status: CPolygonStatus::InvalidInput,
            }));
        }

        if start_point_idx == end_point_idx {
            continue;
        }

        // Check bounds: indices refer to points, each point is 2 f64s
        if end_idx > coords_len {
            return Box::into_raw(Box::new(CPolygonResult {
                polygons: Vec::new(),
                dangles: Vec::new(),
                invalid_rings: Vec::new(),
                status: CPolygonStatus::InvalidInput,
            }));
        }

        // Iterate through POINTS in the linestring
        for j in start_point_idx..end_point_idx - 1 {
            // j is point index. Access coords at 2*j and 2*j+1
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

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = options.node_input;
    polygonizer.snap_grid_size = options.snap_grid_size;
    polygonizer.extract_only_polygonal = options.extract_only_polygonal;
    polygonizer.add_lines(lines);

    match polygonizer.polygonize() {
        Ok(result) => {
            let res = CPolygonResult {
                polygons: result.polygons,
                dangles: result.dangles,
                invalid_rings: result.invalid_rings,
                status: CPolygonStatus::Success,
            };
            Box::into_raw(Box::new(res))
        }
        Err(_) => Box::into_raw(Box::new(CPolygonResult {
            polygons: Vec::new(),
            dangles: Vec::new(),
            invalid_rings: Vec::new(),
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

/// Get shell point count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_shell_point_count(
    res: *const CPolygonResult,
    poly_idx: usize,
) -> usize {
    if res.is_null() {
        return 0;
    }
    let polys = unsafe { &(*res).polygons };
    if poly_idx >= polys.len() {
        return 0;
    }
    polys[poly_idx].exterior().0.len()
}

/// Get shell points
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `2 * point_count` doubles.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_shell_points(
    res: *const CPolygonResult,
    poly_idx: usize,
    buffer: *mut f64,
) {
    if res.is_null() || buffer.is_null() {
        return;
    }
    let polys = unsafe { &(*res).polygons };
    if poly_idx >= polys.len() {
        return;
    }

    let shell = polys[poly_idx].exterior();
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, shell.0.len() * 2) };
    for (i, coord) in shell.0.iter().enumerate() {
        buffer_slice[2 * i] = coord.x;
        buffer_slice[2 * i + 1] = coord.y;
    }
}

/// Get dangle count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_dangle_count(res: *const CPolygonResult) -> usize {
    if res.is_null() {
        return 0;
    }
    unsafe { (*res).dangles.len() }
}

/// Get dangle point count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_dangle_point_count(
    res: *const CPolygonResult,
    dangle_idx: usize,
) -> usize {
    if res.is_null() {
        return 0;
    }
    let dangles = unsafe { &(*res).dangles };
    if dangle_idx >= dangles.len() {
        return 0;
    }
    dangles[dangle_idx].0.len()
}

/// Get dangle points
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `2 * point_count` doubles.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_dangle_points(
    res: *const CPolygonResult,
    dangle_idx: usize,
    buffer: *mut f64,
) {
    if res.is_null() || buffer.is_null() {
        return;
    }
    let dangles = unsafe { &(*res).dangles };
    if dangle_idx >= dangles.len() {
        return;
    }

    let dangle = &dangles[dangle_idx];
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, dangle.0.len() * 2) };
    for (i, coord) in dangle.0.iter().enumerate() {
        buffer_slice[2 * i] = coord.x;
        buffer_slice[2 * i + 1] = coord.y;
    }
}

/// Get hole count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_hole_count(
    res: *const CPolygonResult,
    poly_idx: usize,
) -> usize {
    if res.is_null() {
        return 0;
    }
    let polys = unsafe { &(*res).polygons };
    if poly_idx >= polys.len() {
        return 0;
    }
    polys[poly_idx].interiors().len()
}

/// Get hole point count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_hole_point_count(
    res: *const CPolygonResult,
    poly_idx: usize,
    hole_idx: usize,
) -> usize {
    if res.is_null() {
        return 0;
    }
    let polys = unsafe { &(*res).polygons };
    if poly_idx >= polys.len() {
        return 0;
    }
    let holes = polys[poly_idx].interiors();
    if hole_idx >= holes.len() {
        return 0;
    }
    holes[hole_idx].0.len()
}

/// Get hole points
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `2 * point_count` doubles.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_hole_points(
    res: *const CPolygonResult,
    poly_idx: usize,
    hole_idx: usize,
    buffer: *mut f64,
) {
    if res.is_null() || buffer.is_null() {
        return;
    }
    let polys = unsafe { &(*res).polygons };
    if poly_idx >= polys.len() {
        return;
    }
    let holes = polys[poly_idx].interiors();
    if hole_idx >= holes.len() {
        return;
    }

    let hole = &holes[hole_idx];
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, hole.0.len() * 2) };
    for (i, coord) in hole.0.iter().enumerate() {
        buffer_slice[2 * i] = coord.x;
        buffer_slice[2 * i + 1] = coord.y;
    }
}

/// Get invalid ring count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_invalid_ring_count(
    res: *const CPolygonResult,
) -> usize {
    if res.is_null() {
        return 0;
    }
    unsafe { (*res).invalid_rings.len() }
}

/// Get invalid ring point count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_invalid_ring_point_count(
    res: *const CPolygonResult,
    ring_idx: usize,
) -> usize {
    if res.is_null() {
        return 0;
    }
    let rings = unsafe { &(*res).invalid_rings };
    if ring_idx >= rings.len() {
        return 0;
    }
    rings[ring_idx].0.len()
}

/// Get invalid ring points
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `2 * point_count` doubles.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_invalid_ring_points(
    res: *const CPolygonResult,
    ring_idx: usize,
    buffer: *mut f64,
) {
    if res.is_null() || buffer.is_null() {
        return;
    }
    let rings = unsafe { &(*res).invalid_rings };
    if ring_idx >= rings.len() {
        return;
    }

    let ring = &rings[ring_idx];
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, ring.0.len() * 2) };
    for (i, coord) in ring.0.iter().enumerate() {
        buffer_slice[2 * i] = coord.x;
        buffer_slice[2 * i + 1] = coord.y;
    }
}
