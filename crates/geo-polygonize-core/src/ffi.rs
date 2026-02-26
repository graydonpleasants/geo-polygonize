use crate::Polygonizer;
use geo_types::{Coord, Line, LineString, Polygon};
use std::slice;

#[derive(Clone, Copy)]
struct Line3D {
    start: [f64; 3],
    end: [f64; 3],
}

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
    pub polygon_coords: Vec<f64>,
    pub ring_offsets: Vec<u32>,
    pub polygon_offsets: Vec<u32>,
    pub stride: u8,
    pub status: CPolygonStatus,
}

fn interpolate_z(point: Coord<f64>, seg: Line3D) -> Option<f64> {
    let dx = seg.end[0] - seg.start[0];
    let dy = seg.end[1] - seg.start[1];
    let len2 = dx * dx + dy * dy;
    if len2 <= 1e-24 {
        return None;
    }

    let t = if dx.abs() >= dy.abs() {
        (point.x - seg.start[0]) / dx
    } else {
        (point.y - seg.start[1]) / dy
    };

    if !(-1e-8..=1.0 + 1e-8).contains(&t) {
        return None;
    }

    let px = seg.start[0] + t * dx;
    let py = seg.start[1] + t * dy;
    if (px - point.x).abs() > 1e-6 || (py - point.y).abs() > 1e-6 {
        return None;
    }

    Some(seg.start[2] + t * (seg.end[2] - seg.start[2]))
}

fn lookup_z(point: Coord<f64>, segments: &[Line3D]) -> f64 {
    for seg in segments {
        if let Some(z) = interpolate_z(point, *seg) {
            return z;
        }
    }
    0.0
}

fn flatten_polygons(
    polygons: &[Polygon<f64>],
    segments: &[Line3D],
    stride: u8,
) -> (Vec<f64>, Vec<u32>, Vec<u32>) {
    let mut coords = Vec::new();
    let mut ring_offsets = Vec::new();
    let mut polygon_offsets = Vec::new();

    for poly in polygons {
        polygon_offsets.push(ring_offsets.len() as u32);
        ring_offsets.push((coords.len() / stride as usize) as u32);
        for coord in &poly.exterior().0 {
            coords.push(coord.x);
            coords.push(coord.y);
            if stride == 3 {
                coords.push(lookup_z(*coord, segments));
            }
        }

        for ring in poly.interiors() {
            ring_offsets.push((coords.len() / stride as usize) as u32);
            for coord in &ring.0 {
                coords.push(coord.x);
                coords.push(coord.y);
                if stride == 3 {
                    coords.push(lookup_z(*coord, segments));
                }
            }
        }
    }

    (coords, ring_offsets, polygon_offsets)
}

/// Polygonize linework from flat coordinate and offset buffers.
///
/// Coordinates are provided as interleaved tuples with `stride` components per point
/// (`2` for XY, `3` for XYZ). Offsets follow Arrow-style semantics over point indices.
///
/// # Safety
///
/// If `coords_len > 0`, `coords_ptr` must be valid for reading `coords_len` `f64` values.
/// If `offsets_len > 0`, `offsets_ptr` must be valid for reading `offsets_len` `u32` values.
/// Pointers may be null only when their corresponding length is zero.
#[no_mangle]
pub unsafe extern "C" fn polygonize_ffi(
    coords_ptr: *const f64,
    coords_len: usize,
    offsets_ptr: *const u32,
    offsets_len: usize,
    stride: u8,
    options: PolygonizerOptions,
) -> *mut CPolygonResult {
    if (coords_ptr.is_null() && coords_len > 0) || (offsets_ptr.is_null() && offsets_len > 0) {
        return std::ptr::null_mut();
    }

    if stride != 2 && stride != 3 {
        return std::ptr::null_mut();
    }

    if !coords_len.is_multiple_of(stride as usize) {
        return std::ptr::null_mut();
    }

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
    let mut lines_3d = Vec::new();

    for i in 0..offsets_len - 1 {
        let start_point_idx = offsets[i] as usize;
        let end_point_idx = offsets[i + 1] as usize;

        if start_point_idx > end_point_idx {
            return Box::into_raw(Box::new(CPolygonResult {
                polygons: Vec::new(),
                dangles: Vec::new(),
                invalid_rings: Vec::new(),
                polygon_coords: Vec::new(),
                ring_offsets: Vec::new(),
                polygon_offsets: Vec::new(),
                stride,
                status: CPolygonStatus::InvalidInput,
            }));
        }

        if end_point_idx.saturating_mul(stride as usize) > coords_len {
            return Box::into_raw(Box::new(CPolygonResult {
                polygons: Vec::new(),
                dangles: Vec::new(),
                invalid_rings: Vec::new(),
                polygon_coords: Vec::new(),
                ring_offsets: Vec::new(),
                polygon_offsets: Vec::new(),
                stride,
                status: CPolygonStatus::InvalidInput,
            }));
        }

        if start_point_idx == end_point_idx {
            continue;
        }

        for j in start_point_idx..end_point_idx - 1 {
            let s = j * stride as usize;
            let e = (j + 1) * stride as usize;
            let p1 = Coord {
                x: coords[s],
                y: coords[s + 1],
            };
            let p2 = Coord {
                x: coords[e],
                y: coords[e + 1],
            };
            lines.push(Line::new(p1, p2));

            let z1 = if stride == 3 { coords[s + 2] } else { 0.0 };
            let z2 = if stride == 3 { coords[e + 2] } else { 0.0 };
            lines_3d.push(Line3D {
                start: [p1.x, p1.y, z1],
                end: [p2.x, p2.y, z2],
            });
        }
    }

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = options.node_input;
    polygonizer.snap_grid_size = options.snap_grid_size;
    polygonizer.extract_only_polygonal = options.extract_only_polygonal;
    polygonizer.add_lines(lines);

    match polygonizer.polygonize() {
        Ok(result) => {
            let (polygon_coords, ring_offsets, polygon_offsets) =
                flatten_polygons(&result.polygons, &lines_3d, stride);
            Box::into_raw(Box::new(CPolygonResult {
                polygons: result.polygons,
                dangles: result.dangles,
                invalid_rings: result.invalid_rings,
                polygon_coords,
                ring_offsets,
                polygon_offsets,
                stride,
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
            stride,
            status: CPolygonStatus::InternalError,
        })),
    }
}

/// Return the number of polygon records in the result.
///
/// # Safety
///
/// `res` must be either null or a valid pointer returned by `polygonize_ffi`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_count(res: *const CPolygonResult) -> usize {
    if res.is_null() {
        0
    } else {
        unsafe { (*res).polygons.len() }
    }
}

/// Return the status code for the result.
///
/// # Safety
///
/// `res` must be either null or a valid pointer returned by `polygonize_ffi`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_status(res: *const CPolygonResult) -> i32 {
    if res.is_null() {
        -1
    } else {
        unsafe { (*res).status as i32 }
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

/// Free a result allocated by `polygonize_ffi`.
///
/// # Safety
///
/// `res` must be either null or a pointer previously returned by `polygonize_ffi`
/// that has not already been freed.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_free(res: *mut CPolygonResult) {
    if !res.is_null() {
        unsafe { drop(Box::from_raw(res)) };
    }
}
