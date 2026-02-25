use crate::Polygonizer;
use geo_types::{Coord, LineString, Polygon};
use std::slice;

#[repr(C)]
pub struct PolygonizerOptions {
    pub node_input: bool,
    pub snap_grid_size: f64,
}

pub struct CPolygonResult {
    pub polygons: Vec<Polygon<f64>>,
}

/// Helper to free result
///
/// # Safety
///
/// This function is unsafe because it takes a raw pointer to a `CPolygonResult`.
/// The caller must ensure that:
/// - `res` was allocated by `polygonize_ffi` (or compatible allocation).
/// - `res` has not already been freed.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_free(res: *mut CPolygonResult) {
    if !res.is_null() {
        let _ = Box::from_raw(res);
    }
}

/// Main entry point
///
/// # Safety
///
/// This function is unsafe because it takes raw pointers.
/// The caller must ensure that:
/// - `coords` points to a valid array of `f64` with length `coords_len`.
/// - `offsets` points to a valid array of `u32` with length `offsets_len`.
/// - The memory ranges do not overlap in a way that violates Rust's aliasing rules (though they are const here).
#[no_mangle]
pub unsafe extern "C" fn polygonize_ffi(
    coords: *const f64,
    coords_len: usize,
    offsets: *const u32,
    offsets_len: usize,
    options: PolygonizerOptions,
) -> *mut CPolygonResult {
    if coords.is_null() || offsets.is_null() {
        return std::ptr::null_mut();
    }

    // Ensure coords length is even (pairs of x,y)
    #[allow(clippy::manual_is_multiple_of)]
    if coords_len % 2 != 0 {
        return std::ptr::null_mut();
    }

    let coords_slice = slice::from_raw_parts(coords, coords_len);
    let offsets_slice = slice::from_raw_parts(offsets, offsets_len);

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = options.node_input;
    polygonizer.snap_grid_size = options.snap_grid_size;

    // Parse lines
    for i in 0..offsets_len {
        // Offsets are indices of POINTS (pairs of f64), so multiply by 2 to get float index
        let start = (offsets_slice[i] as usize).saturating_mul(2);

        let end = if i + 1 < offsets_len {
            (offsets_slice[i + 1] as usize).saturating_mul(2)
        } else {
            coords_len
        };

        // Ensure valid range
        if start > coords_len || end > coords_len || start >= end {
            continue;
        }

        let line_coords: Vec<Coord<f64>> = coords_slice[start..end]
            .chunks(2)
            .map(|chunk| Coord {
                x: chunk[0],
                y: chunk[1],
            })
            .collect();

        if line_coords.len() >= 2 {
            polygonizer.add_geometry(geo_types::Geometry::LineString(LineString::new(
                line_coords,
            )));
        }
    }

    let result = polygonizer.polygonize().unwrap_or_default();

    Box::into_raw(Box::new(CPolygonResult { polygons: result }))
}

/// Get polygon count
///
/// # Safety
///
/// `res` must be a valid pointer to `CPolygonResult`.
#[no_mangle]
pub unsafe extern "C" fn polygonize_result_get_count(res: *const CPolygonResult) -> usize {
    if res.is_null() {
        0
    } else {
        (*res).polygons.len()
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
    let polys = &(*res).polygons;
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
    let polys = &(*res).polygons;
    if poly_idx >= polys.len() {
        return;
    }

    let shell = polys[poly_idx].exterior();
    let buffer_slice = slice::from_raw_parts_mut(buffer, shell.0.len() * 2);
    for (i, coord) in shell.0.iter().enumerate() {
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
    let polys = &(*res).polygons;
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
    let polys = &(*res).polygons;
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
    let polys = &(*res).polygons;
    if poly_idx >= polys.len() {
        return;
    }
    let holes = polys[poly_idx].interiors();
    if hole_idx >= holes.len() {
        return;
    }

    let hole = &holes[hole_idx];
    let buffer_slice = slice::from_raw_parts_mut(buffer, hole.0.len() * 2);
    for (i, coord) in hole.0.iter().enumerate() {
        buffer_slice[2 * i] = coord.x;
        buffer_slice[2 * i + 1] = coord.y;
    }
}
