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


    pub fn new_3d(coords: &[crate::types::Coord3D]) -> Self {
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
    use geo::Contains;
    use geo::{Coord, LineString, Polygon};
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

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

    #[test]
    fn test_simd_ring_alignment_fallback() {
        // Test varying sizes to exercise SIMD + scalar tail logic
        // A square 0,0 to 10,10.
        // 5 points: 0,0 -> 10,0 -> 10,10 -> 0,10 -> 0,0.
        // len=5. n=4. 1 iteration of SIMD (4). No scalar tail.

        // Let's create a polygon with more points on the edges to force scalar tail.
        // 0,0 -> 5,0 -> 10,0 -> 10,5 -> 10,10 -> 5,10 -> 0,10 -> 0,5 -> 0,0.
        // 9 points. len=9. n=8.
        // 2 iterations of SIMD. No scalar tail.

        // Add one more point: 0,0 -> ... -> 0,2 -> 0,0.
        // 10 points. len=10. n=9.
        // 2 iterations of SIMD (8). 1 scalar tail (9th segment).

        let coords = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 5.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 10.0, y: 5.0 },
            Coord { x: 10.0, y: 10.0 },
            Coord { x: 5.0, y: 10.0 },
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 0.0, y: 5.0 },
            Coord { x: 0.0, y: 2.0 },
            Coord { x: 0.0, y: 0.0 },
        ];
        let ring = SimdRing::new(&coords);

        // Inside
        assert!(ring.contains(Coord { x: 5.0, y: 5.0 }));
        // Outside
        assert!(!ring.contains(Coord { x: 15.0, y: 5.0 }));
    }

    #[test]
    fn test_simd_ring_large_coords() {
        // Test with large coordinates to check numerical stability
        let offset = 1_000_000.0;
        let coords = vec![
            Coord {
                x: offset,
                y: offset,
            },
            Coord {
                x: offset + 10.0,
                y: offset,
            },
            Coord {
                x: offset + 10.0,
                y: offset + 10.0,
            },
            Coord {
                x: offset,
                y: offset + 10.0,
            },
            Coord {
                x: offset,
                y: offset,
            },
        ];
        let ring = SimdRing::new(&coords);

        assert!(ring.contains(Coord {
            x: offset + 5.0,
            y: offset + 5.0
        }));
        assert!(!ring.contains(Coord {
            x: offset + 15.0,
            y: offset + 5.0
        }));
    }

    #[test]
    fn test_simd_ring_vs_geo_random() {
        let mut rng = StdRng::seed_from_u64(42);

        for _ in 0..100 {
            // Generate a random triangle
            let p1 = Coord {
                x: rng.gen_range(0.0..100.0),
                y: rng.gen_range(0.0..100.0),
            };
            let p2 = Coord {
                x: rng.gen_range(0.0..100.0),
                y: rng.gen_range(0.0..100.0),
            };
            let p3 = Coord {
                x: rng.gen_range(0.0..100.0),
                y: rng.gen_range(0.0..100.0),
            };

            let coords = vec![p1, p2, p3, p1];
            let ring = SimdRing::new(&coords);
            let poly = Polygon::new(LineString::new(coords), vec![]);

            // Test random points
            for _ in 0..10 {
                let test_point = Coord {
                    x: rng.gen_range(0.0..100.0),
                    y: rng.gen_range(0.0..100.0),
                };

                // Geo's contains is strict (false for boundary).
                // SimdRing is strict-ish (false for most boundary, true for some vertices).
                // We skip points too close to boundary to avoid flaky tests on undefined behavior.
                // Convert test_point to Point for distance check
                // let point_geo = geo::Point(test_point);
                // Check distance to boundary
                let boundary = poly.exterior();

                let mut min_dist = f64::MAX;
                for i in 0..boundary.0.len() - 1 {
                    let a = boundary.0[i];
                    let b = boundary.0[i + 1];
                    let p = test_point;

                    let l2 = (b.x - a.x) * (b.x - a.x) + (b.y - a.y) * (b.y - a.y);
                    let d = if l2 == 0.0 {
                        ((p.x - a.x) * (p.x - a.x) + (p.y - a.y) * (p.y - a.y)).sqrt()
                    } else {
                        let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2;
                        let t = t.clamp(0.0, 1.0);
                        let proj_x = a.x + t * (b.x - a.x);
                        let proj_y = a.y + t * (b.y - a.y);
                        ((p.x - proj_x) * (p.x - proj_x) + (p.y - proj_y) * (p.y - proj_y)).sqrt()
                    };

                    if d < min_dist {
                        min_dist = d;
                    }
                }

                // Check distance using Euclidean distance
                if min_dist < 1e-5 {
                    continue;
                }

                let simd_contains = ring.contains(test_point);
                let geo_contains = poly.contains(&test_point);

                assert_eq!(
                    simd_contains, geo_contains,
                    "Mismatch for point {:?} in triangle {:?}, {:?}, {:?}",
                    test_point, p1, p2, p3
                );
            }
        }
    }

    #[test]
    fn test_simd_ring_boundary_documented_behavior() {
        // Documenting the specific behavior of the current implementation for boundary points.
        // Logic: Ray casting to +infinity X.
        // Vertical edges: Ignored if x != px. If x == px, ray runs along edge (degenerate).
        // If ray passes through a vertex:
        //  - If edges go Up-Up or Down-Down: 2 crossings (even) -> Outside.
        //  - If edges go Up-Down or Down-Up: 1 crossing (odd) -> Inside.
        //  - Vertices with y == py are treated as "below" the ray (strict > check).

        let coords = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 0.0, y: 0.0 },
        ];
        let ring = SimdRing::new(&coords);

        // (0,0): Vertex.
        // Edge (0,10)->(0,0) comes down to y=0. (10 > 0 is True).
        // Edge (0,0)->(10,0) goes right at y=0. (0 > 0 is False).
        // Crossing check: `true != false` -> 1 crossing. Inside.
        assert!(ring.contains(Coord { x: 0.0, y: 0.0 }));

        // (10,0): Vertex.
        // Edge (0,0)->(10,0) comes to y=0.
        // Edge (10,0)->(10,10) goes up from y=0.
        // Crossing check: `false != true`. 1 crossing.
        // BUT point.x < intersect_x.
        // Ray starts at x=10. Intersect is at x=10.
        // 10 < 10 is False. No crossing counted. Outside.
        assert!(!ring.contains(Coord { x: 10.0, y: 0.0 }));

        // (10,10): Vertex.
        // Edge (10,0)->(10,10) comes up to y=10. (10 > 10 is False).
        // Edge (10,10)->(0,10) goes left at y=10. (10 > 10 is False).
        // Crossing check: `false != false`. 0 crossings. Outside.
        assert!(!ring.contains(Coord { x: 10.0, y: 10.0 }));

        // (0,10): Vertex.
        // Edge (10,10)->(0,10) comes to y=10.
        // Edge (0,10)->(0,0) goes down from y=10.
        // Crossing check: `false != true`. 1 crossing.
        // intersect_x = 0. 0 < 0 False. Outside.
        assert!(!ring.contains(Coord { x: 0.0, y: 10.0 }));

        // Midpoint of bottom edge (5,0).
        // Ray hits (10,0)-(10,10) at x=10. 5 < 10. 1 crossing. Inside.
        assert!(ring.contains(Coord { x: 5.0, y: 0.0 }));

        // Midpoint of top edge (5,10).
        // Ray hits nothing to the right (except parallel edge (10,10)-(0,10) which is ignored).
        // Outside.
        assert!(!ring.contains(Coord { x: 5.0, y: 10.0 }));

        // Midpoint of left edge (0,5).
        // Ray hits right edge at x=10. 0 < 10. Inside.
        assert!(ring.contains(Coord { x: 0.0, y: 5.0 }));

        // Midpoint of right edge (10,5).
        // Ray starts at x=10. Hits right edge at x=10. 10 < 10 False. Outside.
        assert!(!ring.contains(Coord { x: 10.0, y: 5.0 }));
    }
}
