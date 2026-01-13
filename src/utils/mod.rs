use geo_types::Coord;
use robust::{orient2d, Coord as RobustCoord};
use std::cmp::Ordering;

pub mod parallel;
pub mod simd;

/// Computes a Z-order curve (Morton code) index for a 2D coordinate.
/// Maps floating point coordinates to a 64-bit integer index.
/// This preserves locality: points close in 2D space are likely close in Z-order.
pub fn z_order_index(c: Coord<f64>) -> u64 {
    let x = sortable_float(c.x);
    let y = sortable_float(c.y);
    part1by1(x as u64) | (part1by1(y as u64) << 1)
}

#[inline]
fn sortable_float(f: f64) -> u64 {
    let bits = f.to_bits();
    if bits & 0x8000000000000000 != 0 {
        !bits
    } else {
        bits ^ 0x8000000000000000
    }
}

// Interleave lower 32 bits to 64 bits
#[inline]
fn part1by1(mut n: u64) -> u64 {
    n &= 0x00000000FFFFFFFF;
    n = (n | (n << 16)) & 0x0000FFFF0000FFFF;
    n = (n | (n << 8))  & 0x00FF00FF00FF00FF;
    n = (n | (n << 4))  & 0x0F0F0F0F0F0F0F0F;
    n = (n | (n << 2))  & 0x3333333333333333;
    n = (n | (n << 1))  & 0x5555555555555555;
    n
}

/// Robust comparator for angular sorting of edges around a center point.
/// Replaces the need for `pseudo_angle`.
///
/// Sorts vectors `u` and `v` starting at `center` in counter-clockwise order
/// starting from the positive X-axis.
///
/// Returns `Ordering` such that a < b if a comes before b in CCW order.
pub fn compare_angular(center: Coord<f64>, target_a: Coord<f64>, target_b: Coord<f64>) -> Ordering {
    if target_a == target_b {
        return Ordering::Equal;
    }

    // Determine quadrants
    // 0: [0, 90)   (x>0, y>=0)
    // 1: [90, 180) (x<=0, y>0)
    // 2: [180, 270) (x<0, y<=0)
    // 3: [270, 360) (x>=0, y<0)
    let quad_a = quadrant(center, target_a);
    let quad_b = quadrant(center, target_b);

    if quad_a != quad_b {
        return quad_a.cmp(&quad_b);
    }

    // Same quadrant: use robust orientation check
    // If orient2d(center, a, b) > 0, then b is Left of a (CCW).
    // So a < b.
    let c = RobustCoord { x: center.x, y: center.y };
    let a = RobustCoord { x: target_a.x, y: target_a.y };
    let b = RobustCoord { x: target_b.x, y: target_b.y };

    let orient = orient2d(c, a, b);

    if orient > 0.0 {
        Ordering::Less // a is before b (b is CCW of a)
    } else if orient < 0.0 {
        Ordering::Greater // b is before a (a is CCW of b)
    } else {
        // Collinear rays
        // Sort by distance (shorter first? longer first?)
        // For simple polygonization, dedup usually handles this.
        // Let's pick: Farthest first?
        let dist_a = (target_a.x - center.x).powi(2) + (target_a.y - center.y).powi(2);
        let dist_b = (target_b.x - center.x).powi(2) + (target_b.y - center.y).powi(2);
        dist_a.partial_cmp(&dist_b).unwrap_or(Ordering::Equal)
    }
}

fn quadrant(c: Coord<f64>, t: Coord<f64>) -> u8 {
    let dx = t.x - c.x;
    let dy = t.y - c.y;

    if dx > 0.0 && dy >= 0.0 { 0 }
    else if dx <= 0.0 && dy > 0.0 { 1 }
    else if dx < 0.0 && dy <= 0.0 { 2 }
    else { 3 }
}
