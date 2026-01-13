use wide::f64x4;
use wide::CmpGt;
use geo::Coord;

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
        let px = f64x4::splat(point.x);
        let py = f64x4::splat(point.y);

        let n = self.len - 1; // Number of segments

        let mut i = 0;
        let mut crossings = 0;

        while i < n {
            let remaining = n - i;
            if remaining >= 4 {
                let xi = f64x4::from(&self.x[i..i+4]);
                let yi = f64x4::from(&self.y[i..i+4]);

                let xj = f64x4::from(&self.x[i+1..i+5]);
                let yj = f64x4::from(&self.y[i+1..i+5]);

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
                let p2x = self.x[i+1];
                let p2y = self.y[i+1];

                if ((p1y > point.y) != (p2y > point.y)) &&
                   (point.x < (p2x - p1x) * (point.y - p1y) / (p2y - p1y) + p1x) {
                    crossings += 1;
                }
                i += 1;
            }
        }

        crossings % 2 != 0
    }
}
