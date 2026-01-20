use geo::Line;
use wide::f64x4;
use wide::CmpLe;
use wide::CmpGe;

pub struct SoALines {
    pub start_x: Vec<f64>,
    pub start_y: Vec<f64>,
    pub end_x: Vec<f64>,
    pub end_y: Vec<f64>,
}

impl SoALines {
    pub fn new(lines: &[Line<f64>]) -> Self {
        let len = lines.len();
        // Reserve memory + padding
        let mut sx = Vec::with_capacity(len + 3);
        let mut sy = Vec::with_capacity(len + 3);
        let mut ex = Vec::with_capacity(len + 3);
        let mut ey = Vec::with_capacity(len + 3);

        for line in lines {
            sx.push(line.start.x);
            sy.push(line.start.y);
            ex.push(line.end.x);
            ey.push(line.end.y);
        }

        // Pad with NaNs so that comparisons always fail (return false)
        // preventing false positives at the end of the array.
        while sx.len() % 4 != 0 {
            sx.push(f64::NAN);
            sy.push(f64::NAN);
            ex.push(f64::NAN);
            ey.push(f64::NAN);
        }

        Self { start_x: sx, start_y: sy, end_x: ex, end_y: ey }
    }

    pub fn len(&self) -> usize {
        self.start_x.len()
    }

    /// Checks a single query line against 4 stored lines simultaneously.
    /// Returns a bitmask (u8) where bits 0-3 represent intersection candidates.
    ///
    /// Bit 0 = index
    /// Bit 1 = index + 1
    /// ...
    #[inline]
    pub fn intersects_bbox_batch(&self, query: Line<f64>, index: usize) -> u8 {
        // 1. Prepare Query BBox (Splat to all 4 lanes)
        // We pre-calculate min/max for the query line once.
        let q_min_x_val = query.start.x.min(query.end.x);
        let q_max_x_val = query.start.x.max(query.end.x);
        let q_min_y_val = query.start.y.min(query.end.y);
        let q_max_y_val = query.start.y.max(query.end.y);

        let q_min_x = f64x4::splat(q_min_x_val);
        let q_max_x = f64x4::splat(q_max_x_val);
        let q_min_y = f64x4::splat(q_min_y_val);
        let q_max_y = f64x4::splat(q_max_y_val);

        // 2. Load Targets (4 at a time)
        // Ideally, ensure `index` is valid. We rely on the caller stepping by 4.
        let t_sx = f64x4::from(&self.start_x[index..index+4]);
        let t_sy = f64x4::from(&self.start_y[index..index+4]);
        let t_ex = f64x4::from(&self.end_x[index..index+4]);
        let t_ey = f64x4::from(&self.end_y[index..index+4]);

        // 3. Calculate Target BBoxes (Parallel Min/Max)
        let t_min_x = t_sx.min(t_ex);
        let t_max_x = t_sx.max(t_ex);
        let t_min_y = t_sy.min(t_ey);
        let t_max_y = t_sy.max(t_ey);

        // 4. Perform Intersection Check
        // Logic: Overlap exists if (RectA.min < RectB.max) && (RectA.max > RectB.min)
        // We check X axis AND Y axis.

        // Note: wide cmp methods return a mask where all bits are 1 for true.
        let overlap_x = q_min_x.cmp_le(t_max_x) & q_max_x.cmp_ge(t_min_x);
        let overlap_y = q_min_y.cmp_le(t_max_y) & q_max_y.cmp_ge(t_min_y);

        let overlap = overlap_x & overlap_y;

        // 5. Pack result to u8
        // move_mask() extracts the sign bit of each lane.
        // For f64x4, it returns an i32 (only bottom 4 bits matter).
        overlap.move_mask() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Coord, Line};

    #[test]
    fn test_soa_bbox_batch_simd() {
        // Setup 4 lines to test against a query
        // Query Line: (0,0) -> (10,10). BBox: [0,0, 10,10]
        let query = Line::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 });

        let lines = vec![
            // 0. Inside Query BBox (Should Match)
            Line::new(Coord { x: 1.0, y: 1.0 }, Coord { x: 2.0, y: 2.0 }),

            // 1. Completely Outside to the Right (No Match)
            // BBox: [12,0, 14,10] -> MinX(12) > QueryMaxX(10)
            Line::new(Coord { x: 12.0, y: 0.0 }, Coord { x: 14.0, y: 10.0 }),

            // 2. Overlapping Boundary (Touching) (Should Match)
            // BBox: [10,5, 15,5]. MinX(10) <= QueryMaxX(10)
            Line::new(Coord { x: 10.0, y: 5.0 }, Coord { x: 15.0, y: 5.0 }),

            // 3. Diagonal Crossing (Should Match)
            Line::new(Coord { x: 0.0, y: 10.0 }, Coord { x: 10.0, y: 0.0 }),
        ];

        let soa = SoALines::new(&lines);

        // Run the SIMD check
        let mask = soa.intersects_bbox_batch(query, 0);

        // Expected bits:
        // Index 0: Match -> 1
        // Index 1: No    -> 0
        // Index 2: Match -> 1
        // Index 3: Match -> 1
        // Result binary: 1101 (Little Endian order: bit0=idx0, bit3=idx3)
        // 1 + 0 + 4 + 8 = 13
        assert_eq!(mask, 0b1101, "Mask should match expected intersections");
    }

    #[test]
    fn test_soa_padding_safety() {
        // Test that the padding NaNs don't cause false positives
        let query = Line::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 });

        // Only 1 line provided. 3 slots will be padded with NaN.
        let lines = vec![
            Line::new(Coord { x: 1.0, y: 1.0 }, Coord { x: 2.0, y: 2.0 }),
        ];

        let soa = SoALines::new(&lines);

        // Ensure we allocated enough for SIMD width
        assert!(soa.start_x.len() >= 4);

        let mask = soa.intersects_bbox_batch(query, 0);

        // Index 0 is a match.
        // Index 1, 2, 3 are NaN padding.
        // Comparisons with NaN (e.g. NaN <= 10.0) return False.
        // So mask should be 0001 -> 1.
        assert_eq!(mask, 1, "Padding slots should never return true");
    }

    #[test]
    fn test_empty_soa() {
        // Edge case: Empty input
        let lines: Vec<Line<f64>> = vec![];
        let soa = SoALines::new(&lines);

        assert_eq!(soa.start_x.len(), 0);
    }

    #[test]
    fn test_crossing_scenario() {
        // Reproduction of test_noding_crossing_lines structure
        let lines = vec![
            Line::new(Coord{x:0.,y:0.}, Coord{x:10.,y:0.}), // 0
            Line::new(Coord{x:10.,y:0.}, Coord{x:10.,y:10.}), // 1
            Line::new(Coord{x:10.,y:10.}, Coord{x:0.,y:10.}), // 2
            Line::new(Coord{x:0.,y:10.}, Coord{x:0.,y:0.}), // 3
            Line::new(Coord{x:0.,y:0.}, Coord{x:10.,y:10.}), // 4
            Line::new(Coord{x:0.,y:10.}, Coord{x:10.,y:0.}), // 5
        ];

        let soa = SoALines::new(&lines);

        // Check 4 vs 5
        // 5 is at index 5.
        // Block starting at 4 covers 4, 5, 6, 7.
        // We query line 4.
        let mask = soa.intersects_bbox_batch(lines[4], 4);

        // Expected:
        // Index 4 (Self): Match
        // Index 5 (Cross): Match
        // Index 6 (NaN): No
        // Index 7 (NaN): No
        // Mask: 0011 -> 3

        assert_eq!(mask & 2, 2, "Line 4 should intersect Line 5 (bit 1)");
    }
}
