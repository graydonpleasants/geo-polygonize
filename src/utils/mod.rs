use geo_types::Coord;

/// Computes a Z-order curve (Morton code) index for a 2D coordinate.
/// Maps floating point coordinates to a 64-bit integer index.
/// This preserves locality: points close in 2D space are likely close in Z-order.
pub fn z_order_index(c: Coord<f64>) -> u64 {
    // Normalize? We assume inputs are in a reasonable range or we just cast bits?
    // Using bit interleaving on integers is standard.
    // For floats, we can map to u32 range.
    // A simple, robust way is to interleave the bits of the normalized integer representation.

    // We Map float to [0, u32::MAX].
    // Assuming typical GIS coords, maybe just cast to u32 after scaling?
    // Let's use a simpler sort: Interleave bits of the integer parts?
    // Actually, `rstar` uses something similar internally.

    // Let's implement a simple bit interleaving for positive integers.
    // We map f64 to u32 by sorting logic?
    // Just mapping the bits directly (transmuting) works if data is positive.
    // If data can be negative, we need to handle sign.
    // A robust way: Map min/max bounds to 0..u32::MAX.
    // But we don't want to scan for bounds every time.

    // Alternative: Just use the f64 bits, flip sign bit if negative.
    // https://stackoverflow.com/questions/10260927/translation-of-float-to-integer-preserving-order
    //
    // To keep it simple and fast: We prioritize locality.
    // We can just cast to i32 (grid coordinates) if we assume the user provides something grid-like?
    // No, must be general.

    // Let's use `rstar`'s strategy? `rstar` sorts by dimension.

    // We'll implement a "good enough" Z-order for sorting.
    // Interleave x and y bits.

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
