use geo::Coord;
use wide::f64x4;
use wide::CmpGt;

pub struct SimdRing {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    len: usize,
}

impl SimdRing {
    pub fn new(coords: &[Coord<f64>]) -> Self {
        let len = coords.len();

        let mut x = Vec::with_capacity(len + 3);
        let mut y = Vec::with_capacity(len + 3);

        for c in coords {
            x.push(c.x);
            y.push(c.y);
        }

        while x.len() % 4 != 0 {
            x.push(x.last().cloned().unwrap_or(0.0));
            y.push(y.last().cloned().unwrap_or(0.0));
        }

        Self { x, y, len }
    }

    pub fn contains(&self, point: Coord<f64>) -> bool {
        if self.len == 0 {
            return false;
        }

        let px = f64x4::splat(point.x);
        let py = f64x4::splat(point.y);

        let n = self.len - 1; // Number of segments

        let mut i = 0;
        let mut crossings = 0;

        while i < n {
            let remaining = n - i;
            if remaining >= 4 {
                let xi = f64x4::from(&self.x[i..i + 4]);
                let yi = f64x4::from(&self.y[i..i + 4]);

                let xj = f64x4::from(&self.x[i + 1..i + 5]);
                let yj = f64x4::from(&self.y[i + 1..i + 5]);

                let yi_gt_py = yi.cmp_gt(py);
                let yj_gt_py = yj.cmp_gt(py);
                let in_range = yi_gt_py ^ yj_gt_py;

                let num = (xj - xi) * (py - yi);
                let den = yj - yi;

                let intersect_x = (num / den) + xi;
                let x_cond = intersect_x.cmp_gt(px);

                let is_crossing = in_range & x_cond;

                crossings += is_crossing.move_mask().count_ones();

                i += 4;
            } else {
                let p1x = self.x[i];
                let p1y = self.y[i];
                let p2x = self.x[i + 1];
                let p2y = self.y[i + 1];

                if ((p1y > point.y) != (p2y > point.y))
                    && (point.x < (p2x - p1x) * (point.y - p1y) / (p2y - p1y) + p1x)
                {
                    crossings += 1;
                }
                i += 1;
            }
        }

        crossings % 2 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Coord;

    #[test]
    fn test_simd_ring_square() {
        let coords = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 0.0, y: 0.0 },
        ];
        let ring = SimdRing::new(&coords);

        // Inside
        assert!(ring.contains(Coord { x: 5.0, y: 5.0 }));

        // Outside
        assert!(!ring.contains(Coord { x: 15.0, y: 5.0 }));
        assert!(!ring.contains(Coord { x: 5.0, y: 15.0 }));
        assert!(!ring.contains(Coord { x: -5.0, y: 5.0 }));
        assert!(!ring.contains(Coord { x: 5.0, y: -5.0 }));
    }

    #[test]
    fn test_simd_ring_triangle() {
        let coords = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 0.0, y: 0.0 },
        ];
        let ring = SimdRing::new(&coords);

        // Inside
        assert!(ring.contains(Coord { x: 2.0, y: 2.0 }));
        // Outside
        assert!(!ring.contains(Coord { x: 10.0, y: 10.0 })); // Outside bounding box corner
        assert!(!ring.contains(Coord { x: 5.0, y: 6.0 })); // Outside hypotenuse
    }

    #[test]
    fn test_simd_ring_complex() {
        // A "U" shape polygon
        // 0,0 -> 10,0 -> 10,10 -> 8,10 -> 8,2 -> 2,2 -> 2,10 -> 0,10 -> 0,0
        let coords = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
            Coord { x: 8.0, y: 10.0 },
            Coord { x: 8.0, y: 2.0 },
            Coord { x: 2.0, y: 2.0 },
            Coord { x: 2.0, y: 10.0 },
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 0.0, y: 0.0 },
        ];
        let ring = SimdRing::new(&coords);

        // Inside the U base
        assert!(ring.contains(Coord { x: 5.0, y: 1.0 }));
        // Inside the U arms
        assert!(ring.contains(Coord { x: 1.0, y: 5.0 }));
        assert!(ring.contains(Coord { x: 9.0, y: 5.0 }));

        // Inside the "hole" of the U (concave part)
        // x=5, y=5 is in the empty space between arms
        assert!(!ring.contains(Coord { x: 5.0, y: 5.0 }));

        // Outside bounding box
        assert!(!ring.contains(Coord { x: 12.0, y: 5.0 }));
    }

    #[test]
    fn test_simd_ring_empty() {
        let coords: Vec<Coord<f64>> = vec![];
        let ring = SimdRing::new(&coords);
        assert!(!ring.contains(Coord { x: 0.0, y: 0.0 }));

        let coords_single = vec![Coord { x: 0.0, y: 0.0 }];
        let ring_single = SimdRing::new(&coords_single);
        assert!(!ring_single.contains(Coord { x: 0.0, y: 0.0 }));
    }
}
