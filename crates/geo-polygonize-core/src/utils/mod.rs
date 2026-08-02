use geo_types::Coord;
use robust::{orient2d, Coord as RobustCoord};
use std::cmp::Ordering;

pub mod parallel;
pub mod simd;
pub mod soa;

#[inline]
pub(crate) fn canonical_coordinate_bits(value: f64) -> u64 {
    if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}

pub(crate) fn minimum_rotation_index<T: Ord>(ring: &[T]) -> usize {
    let n = ring.len();
    if n < 2 {
        return 0;
    }
    let (mut left, mut right, mut offset) = (0, 1, 0);
    while left < n && right < n && offset < n {
        match ring[(left + offset) % n].cmp(&ring[(right + offset) % n]) {
            Ordering::Equal => offset += 1,
            Ordering::Less => {
                right += offset + 1;
                if right == left {
                    right += 1;
                }
                offset = 0;
            }
            Ordering::Greater => {
                left += offset + 1;
                if left == right {
                    left += 1;
                }
                offset = 0;
            }
        }
    }
    left.min(right)
}

/// Computes a Z-order curve (Morton code) index for a 2D coordinate.
/// Maps floating point coordinates to a 64-bit integer index.
/// This preserves locality: points close in 2D space are likely close in Z-order.
pub fn z_order_index(c: Coord<f64>) -> u64 {
    let x = sortable_float(c.x);
    let y = sortable_float(c.y);
    // Use the most significant 32 bits for Z-order index computation
    // since `sortable_float` maps the float into a 64-bit unsigned int
    // where the sign, exponent, and top mantissa are in the high bits.
    part1by1(x >> 32) | (part1by1(y >> 32) << 1)
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
    n = (n | (n << 8)) & 0x00FF00FF00FF00FF;
    n = (n | (n << 4)) & 0x0F0F0F0F0F0F0F0F;
    n = (n | (n << 2)) & 0x3333333333333333;
    n = (n | (n << 1)) & 0x5555555555555555;
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
    let c = RobustCoord {
        x: center.x,
        y: center.y,
    };
    let a = RobustCoord {
        x: target_a.x,
        y: target_a.y,
    };
    let b = RobustCoord {
        x: target_b.x,
        y: target_b.y,
    };

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
        let dx_a = target_a.x - center.x;
        let dy_a = target_a.y - center.y;
        let dist_a = dx_a * dx_a + dy_a * dy_a;

        let dx_b = target_b.x - center.x;
        let dy_b = target_b.y - center.y;
        let dist_b = dx_b * dx_b + dy_b * dy_b;

        dist_a.partial_cmp(&dist_b).unwrap_or(Ordering::Equal)
    }
}

fn quadrant(c: Coord<f64>, t: Coord<f64>) -> u8 {
    let dx = t.x - c.x;
    let dy = t.y - c.y;

    if dx > 0.0 && dy >= 0.0 {
        0
    } else if dx <= 0.0 && dy > 0.0 {
        1
    } else if dx < 0.0 && dy <= 0.0 {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sortable_float() {
        // Test ordering preservation for typical values
        let values = vec![
            f64::NEG_INFINITY,
            -1000.0,
            -2.0,
            -1.0,
            -0.0,
            0.0,
            1.0,
            2.0,
            1000.0,
            f64::INFINITY,
        ];

        for w in values.windows(2) {
            let a = w[0];
            let b = w[1];
            assert!(
                sortable_float(a) < sortable_float(b),
                "Expected sortable_float({}) < sortable_float({}), but got {} >= {}",
                a,
                b,
                sortable_float(a),
                sortable_float(b)
            );
        }

        // Test NaN behavior (NaN bits are preserved but ordering is tricky,
        // generally we just want to ensure it doesn't crash and maps predictably)
        let nan_mapped = sortable_float(f64::NAN);
        assert!(nan_mapped > 0);
    }

    #[test]
    fn test_part1by1() {
        // Interleave lower 32 bits, separating them with zeros.
        // 0b1 -> 0b1
        assert_eq!(part1by1(0b1), 0b1);
        // 0b11 -> 0b0101
        assert_eq!(part1by1(0b11), 0b0101);
        // 0b101 -> 0b0001_0000_0001
        assert_eq!(part1by1(0b101), 0b10001);

        // 0xFFFFFFFF (32 ones) -> 0x5555555555555555 (64 alternating ones and zeros)
        assert_eq!(part1by1(0xFFFFFFFF), 0x5555555555555555);

        // Higher bits are ignored by the `n &= 0x00000000FFFFFFFF;` line
        assert_eq!(part1by1(0x1_FFFFFFFF), 0x5555555555555555);
    }

    #[test]
    fn test_z_order_index_locality() {
        // Points that are close in 2D space should have similar Z-order indices.
        let p1 = Coord { x: 0.0, y: 0.0 };
        let p2 = Coord {
            x: 0.00001,
            y: 0.00001,
        };
        let p3 = Coord { x: 100.0, y: 100.0 };

        let z1 = z_order_index(p1);
        let z2 = z_order_index(p2);
        let z3 = z_order_index(p3);

        // Calculate absolute differences
        let diff12 = z1.abs_diff(z2);
        let diff13 = z1.abs_diff(z3);

        assert!(
            diff12 < diff13,
            "Expected points closer in 2D space to have closer Z-order indices. \
            diff12: {}, diff13: {}",
            diff12,
            diff13
        );
    }

    #[test]
    fn test_z_order_index_quadrants() {
        // Test points in different quadrants around the origin.
        // For a standard Z-order curve with unsigned integers, interleaving X and Y
        // puts Y on the even bits and X on the odd bits (or vice versa).
        // Here we map negative floats to smaller unsigned integers, so the relative
        // ordering should follow a Z-pattern across quadrants.

        let p_bl = Coord { x: -1.0, y: -1.0 }; // Bottom-left (smallest x, smallest y)
        let p_br = Coord { x: 1.0, y: -1.0 }; // Bottom-right (largest x, smallest y)
        let p_tl = Coord { x: -1.0, y: 1.0 }; // Top-left (smallest x, largest y)
        let p_tr = Coord { x: 1.0, y: 1.0 }; // Top-right (largest x, largest y)

        let z_bl = z_order_index(p_bl);
        let z_br = z_order_index(p_br);
        let z_tl = z_order_index(p_tl);
        let z_tr = z_order_index(p_tr);

        // Verify the basic Z-pattern structure:
        // bottom-left is the smallest overall because both components are negative
        assert!(z_bl < z_br);
        assert!(z_bl < z_tl);

        // top-right is the largest overall because both components are positive
        assert!(z_br < z_tr);
        assert!(z_tl < z_tr);

        // Given `part1by1(y) << 1`, Y provides the more significant bits locally
        // between a horizontal pair vs vertical pair, though with 64-bit interleaved
        // it applies strictly to every bit. We mainly just care that the function
        // evaluates cleanly without panics and gives distinct values.
        assert_ne!(z_bl, z_br);
        assert_ne!(z_bl, z_tl);
        assert_ne!(z_bl, z_tr);
        assert_ne!(z_br, z_tl);
        assert_ne!(z_br, z_tr);
        assert_ne!(z_tl, z_tr);
    }
}
