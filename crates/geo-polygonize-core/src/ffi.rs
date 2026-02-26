use crate::types::{Coord3D, Line3D, Polygon3D};
use crate::Polygonizer;
use std::slice;

#[repr(C)]
pub struct PolygonizerOptions {
    pub node_input: u8,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: u8,
}

#[repr(i32)]
#[derive(Clone, Copy)]
pub enum CPolygonStatus {
    Success = 0,
    InvalidInput = 1,
    InternalError = 2,
}

pub struct CPolygonResult {
    pub polygons: Vec<Polygon3D>,
    pub dangles: Vec<Vec<Coord3D>>,
    pub invalid_rings: Vec<Vec<Coord3D>>,
    pub polygon_coords: Vec<f64>,
    pub ring_offsets: Vec<u32>,
    pub polygon_offsets: Vec<u32>,
    pub stride: u8,
    pub status: CPolygonStatus,
}

fn flatten_polygons(polygons: &[Polygon3D], stride: u8) -> (Vec<f64>, Vec<u32>, Vec<u32>) {
    let mut coords = Vec::new();
    let mut ring_offsets = Vec::new();
    let mut polygon_offsets = Vec::new();

    for poly in polygons {
        polygon_offsets.push(ring_offsets.len() as u32);
        ring_offsets.push((coords.len() / stride as usize) as u32);
        for coord in &poly.exterior {
            coords.push(coord.x);
            coords.push(coord.y);
            if stride == 3 {
                coords.push(coord.z);
            }
        }

        for ring in &poly.interiors {
            ring_offsets.push((coords.len() / stride as usize) as u32);
            for coord in ring {
                coords.push(coord.x);
                coords.push(coord.y);
                if stride == 3 {
                    coords.push(coord.z);
                }
            }
        }
    }

    (coords, ring_offsets, polygon_offsets)
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
    stride: u8,
    options: *const PolygonizerOptions,
) -> *mut CPolygonResult {
    if (coords_ptr.is_null() && coords_len > 0)
        || (offsets_ptr.is_null() && offsets_len > 0)
        || options.is_null()
    {
        return std::ptr::null_mut();
    }

    if stride != 2 && stride != 3 {
        return std::ptr::null_mut();
    }

    #[allow(clippy::manual_is_multiple_of)]
    if coords_len % (stride as usize) != 0 {
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

    let opts = unsafe { &*options };

    if offsets_len < 2 {
        // No lines can be defined with < 2 offsets
        return Box::into_raw(Box::new(CPolygonResult {
            polygons: Vec::new(),
            dangles: Vec::new(),
            invalid_rings: Vec::new(),
            polygon_coords: Vec::new(),
            ring_offsets: Vec::new(),
            polygon_offsets: Vec::new(),
            stride,
            status: CPolygonStatus::Success,
        }));
    }

    let mut lines = Vec::new();
    let stride = stride as usize;

    // Iterate through linestrings defined by offsets
    for i in 0..offsets_len - 1 {
        // Offsets are indices of POINTS (tuples of f64), so multiply by stride
        let start_point_idx = offsets[i] as usize;
        let end_point_idx = offsets[i + 1] as usize;

        let end_idx = end_point_idx.saturating_mul(stride);

        if start_point_idx > end_point_idx {
            // Invalid offset range
            return Box::into_raw(Box::new(CPolygonResult {
                polygons: Vec::new(),
                dangles: Vec::new(),
                invalid_rings: Vec::new(),
                polygon_coords: Vec::new(),
                ring_offsets: Vec::new(),
                polygon_offsets: Vec::new(),
                stride: stride as u8,
                status: CPolygonStatus::InvalidInput,
            }));
        }

        if start_point_idx == end_point_idx {
            continue;
        }

        // Check bounds
        if end_idx > coords_len {
            return Box::into_raw(Box::new(CPolygonResult {
                polygons: Vec::new(),
                dangles: Vec::new(),
                invalid_rings: Vec::new(),
                polygon_coords: Vec::new(),
                ring_offsets: Vec::new(),
                polygon_offsets: Vec::new(),
                stride: stride as u8,
                status: CPolygonStatus::InvalidInput,
            }));
        }

        // Iterate through POINTS in the linestring
        for j in start_point_idx..end_point_idx - 1 {
            let idx1 = j * stride;
            let idx2 = (j + 1) * stride;

            let p1 = if stride == 2 {
                Coord3D::new(coords[idx1], coords[idx1 + 1], 0.0)
            } else {
                Coord3D::new(coords[idx1], coords[idx1 + 1], coords[idx1 + 2])
            };

            let p2 = if stride == 2 {
                Coord3D::new(coords[idx2], coords[idx2 + 1], 0.0)
            } else {
                Coord3D::new(coords[idx2], coords[idx2 + 1], coords[idx2 + 2])
            };

            lines.push(Line3D::new(p1, p2));
        }
    }

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = opts.node_input != 0;
    polygonizer.snap_grid_size = opts.snap_grid_size;
    polygonizer.extract_only_polygonal = opts.extract_only_polygonal != 0;
    polygonizer.add_lines(lines);

    match polygonizer.polygonize() {
        Ok(result) => {
            let (polygon_coords, ring_offsets, polygon_offsets) =
                flatten_polygons(&result.polygons, stride as u8);
            Box::into_raw(Box::new(CPolygonResult {
                polygons: result.polygons,
                dangles: result.dangles,
                invalid_rings: result.invalid_rings,
                polygon_coords,
                ring_offsets,
                polygon_offsets,
                stride: stride as u8,
                status: CPolygonStatus::Success,
            }))
        }
        Err(_) => Box::into_raw(Box::new(CPolygonResult {
            polygons: Vec::new(),
            dangles: Vec::new(),
            invalid_rings: Vec::new(),
            polygon_coords: Vec::new(),
            ring_offsets: Vec::new(),
            polygon_offsets: Vec::new(),
            stride: stride as u8,
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

/// Get shell point count (3D)
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
    polys[poly_idx].exterior.len()
}

/// Get shell points (3D, interleaved [x,y,z,...])
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `3 * point_count` doubles.
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

    let shell = &polys[poly_idx].exterior;
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, shell.len() * 3) };
    for (i, coord) in shell.iter().enumerate() {
        buffer_slice[3 * i] = coord.x;
        buffer_slice[3 * i + 1] = coord.y;
        buffer_slice[3 * i + 2] = coord.z;
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
    dangles[dangle_idx].len()
}

/// Get dangle points (3D)
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `3 * point_count` doubles.
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
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, dangle.len() * 3) };
    for (i, coord) in dangle.iter().enumerate() {
        buffer_slice[3 * i] = coord.x;
        buffer_slice[3 * i + 1] = coord.y;
        buffer_slice[3 * i + 2] = coord.z;
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
    polys[poly_idx].interiors.len()
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
    let holes = &polys[poly_idx].interiors;
    if hole_idx >= holes.len() {
        return 0;
    }
    holes[hole_idx].len()
}

/// Get hole points (3D)
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `3 * point_count` doubles.
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
    let holes = &polys[poly_idx].interiors;
    if hole_idx >= holes.len() {
        return;
    }

    let hole = &holes[hole_idx];
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, hole.len() * 3) };
    for (i, coord) in hole.iter().enumerate() {
        buffer_slice[3 * i] = coord.x;
        buffer_slice[3 * i + 1] = coord.y;
        buffer_slice[3 * i + 2] = coord.z;
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
    rings[ring_idx].len()
}

/// Get invalid ring points (3D)
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
/// `buffer` must point to a valid memory region large enough to hold `3 * point_count` doubles.
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
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, ring.len() * 3) };
    for (i, coord) in ring.iter().enumerate() {
        buffer_slice[3 * i] = coord.x;
        buffer_slice[3 * i + 1] = coord.y;
        buffer_slice[3 * i + 2] = coord.z;
    }
}

/// Return the coordinate stride (2 or 3) used in this result.
///
/// # Safety
///
/// `res` must be either null or a valid pointer returned by `polygonize_ffi`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_stride(res: *const CPolygonResult) -> u8 {
    if res.is_null() {
        0
    } else {
        unsafe { (*res).stride }
    }
}

/// Return the number of `f64` values in the flat polygon coordinate buffer.
///
/// # Safety
///
/// `res` must be either null or a valid pointer returned by `polygonize_ffi`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_flat_coords_len(
    res: *const CPolygonResult,
) -> usize {
    if res.is_null() {
        0
    } else {
        unsafe { (*res).polygon_coords.len() }
    }
}

/// Copy flat polygon coordinates into a caller-provided buffer.
///
/// # Safety
///
/// `res` must be a valid pointer returned by `polygonize_ffi` (or null, in which case this is a no-op).
/// `buffer` must be valid for writes of `polygonize_result_get_flat_coords_len(res)` `f64` values.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_copy_flat_coords(
    res: *const CPolygonResult,
    buffer: *mut f64,
) {
    if res.is_null() || buffer.is_null() {
        return;
    }

    let coords = unsafe { &(*res).polygon_coords };
    let out = unsafe { slice::from_raw_parts_mut(buffer, coords.len()) };
    out.copy_from_slice(coords);
}

/// Return the number of entries in the ring-offset buffer.
///
/// # Safety
///
/// `res` must be either null or a valid pointer returned by `polygonize_ffi`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_ring_offsets_len(
    res: *const CPolygonResult,
) -> usize {
    if res.is_null() {
        0
    } else {
        unsafe { (*res).ring_offsets.len() }
    }
}

/// Copy ring offsets into a caller-provided buffer.
///
/// # Safety
///
/// `res` must be a valid pointer returned by `polygonize_ffi` (or null, in which case this is a no-op).
/// `buffer` must be valid for writes of `polygonize_result_get_ring_offsets_len(res)` `u32` values.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_copy_ring_offsets(
    res: *const CPolygonResult,
    buffer: *mut u32,
) {
    if res.is_null() || buffer.is_null() {
        return;
    }

    let offsets = unsafe { &(*res).ring_offsets };
    let out = unsafe { slice::from_raw_parts_mut(buffer, offsets.len()) };
    out.copy_from_slice(offsets);
}

/// Return the number of entries in the polygon-offset buffer.
///
/// # Safety
///
/// `res` must be either null or a valid pointer returned by `polygonize_ffi`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_polygon_offsets_len(
    res: *const CPolygonResult,
) -> usize {
    if res.is_null() {
        0
    } else {
        unsafe { (*res).polygon_offsets.len() }
    }
}

/// Copy polygon offsets into a caller-provided buffer.
///
/// # Safety
///
/// `res` must be a valid pointer returned by `polygonize_ffi` (or null, in which case this is a no-op).
/// `buffer` must be valid for writes of `polygonize_result_get_polygon_offsets_len(res)` `u32` values.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_copy_polygon_offsets(
    res: *const CPolygonResult,
    buffer: *mut u32,
) {
    if res.is_null() || buffer.is_null() {
        return;
    }

    let offsets = unsafe { &(*res).polygon_offsets };
    let out = unsafe { slice::from_raw_parts_mut(buffer, offsets.len()) };
    out.copy_from_slice(offsets);
}
