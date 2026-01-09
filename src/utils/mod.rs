use geo_types::Coord;

/// Computes a Z-order curve (Morton code) index for a 2D coordinate.
/// Maps floating point coordinates to a 64-bit integer index.
/// This preserves locality: points close in 2D space are likely close in Z-order.
pub fn z_order_index(c: Coord<f64>) -> u64 {
    let x = sortable_float(c.x);
    let y = sortable_float(c.y);
    part1by1(x as u64) | (part1by1(y as u64) << 1)
}

fn sortable_float(f: f64) -> u64 {
    let bits = f.to_bits();
    if bits & 0x8000000000000000 != 0 {
        !bits
    } else {
        bits ^ 0x8000000000000000
    }
}

// Interleave lower 32 bits to 64 bits
fn part1by1(mut n: u64) -> u64 {
    n &= 0x00000000FFFFFFFF;
    n = (n | (n << 16)) & 0x0000FFFF0000FFFF;
    n = (n | (n << 8))  & 0x00FF00FF00FF00FF;
    n = (n | (n << 4))  & 0x0F0F0F0F0F0F0F0F;
    n = (n | (n << 2))  & 0x3333333333333333;
    n = (n | (n << 1))  & 0x5555555555555555;
    n
}
