use geo_types::{Coord, Line, LineString, Polygon};
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct DeterminismOptions {
    pub canonical_sort: bool,
    pub canonical_ring_rotation: bool,
    pub stable_tie_breaks: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coord3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Coord3D {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn to_coord_2d(self) -> Coord<f64> {
        Coord {
            x: self.x,
            y: self.y,
        }
    }
}

impl From<Coord<f64>> for Coord3D {
    fn from(c: Coord<f64>) -> Self {
        Self {
            x: c.x,
            y: c.y,
            z: 0.0, // Default Z
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Line3D {
    pub start: Coord3D,
    pub end: Coord3D,
    pub line_id: u32,
}

impl PartialEq for Line3D {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end == other.end
    }
}

impl Line3D {
    pub fn new(start: Coord3D, end: Coord3D, line_id: u32) -> Self {
        Self {
            start,
            end,
            line_id,
        }
    }

    pub fn to_line_2d(self) -> Line<f64> {
        Line::new(self.start.to_coord_2d(), self.end.to_coord_2d())
    }
}

impl From<Line<f64>> for Line3D {
    fn from(l: Line<f64>) -> Self {
        Self {
            start: l.start.into(),
            end: l.end.into(),
            line_id: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Polygon3D {
    pub exterior: Vec<Coord3D>,
    pub interiors: Vec<Vec<Coord3D>>,
    pub exterior_ids: Vec<u32>,
    pub interiors_ids: Vec<Vec<u32>>,
}

impl Polygon3D {
    pub fn new(
        exterior: Vec<Coord3D>,
        interiors: Vec<Vec<Coord3D>>,
        exterior_ids: Vec<u32>,
        interiors_ids: Vec<Vec<u32>>,
    ) -> Self {
        Self {
            exterior,
            interiors,
            exterior_ids,
            interiors_ids,
        }
    }

    pub fn to_polygon_2d(&self) -> Polygon<f64> {
        let ext = LineString(self.exterior.iter().map(|c| c.to_coord_2d()).collect());
        let ints = self
            .interiors
            .iter()
            .map(|ring| LineString(ring.iter().map(|c| c.to_coord_2d()).collect()))
            .collect();
        Polygon::new(ext, ints)
    }

    /// Computes the signed 2D area directly without allocating intermediate geometry.
    /// This assumes standard winding order (exterior CCW, interior CW) where interior areas are implicitly negative.
    pub fn signed_area_2d(&self) -> f64 {
        let mut area = Self::ring_signed_area_2d(&self.exterior);
        for hole in &self.interiors {
            area += Self::ring_signed_area_2d(hole);
        }
        area
    }

    /// Computes the unsigned 2D area directly without allocating intermediate geometry.
    /// The unsigned area subtracts the absolute areas of the interiors from the absolute area of the exterior.
    pub fn unsigned_area_2d(&self) -> f64 {
        let mut area = Self::ring_signed_area_2d(&self.exterior).abs();
        for hole in &self.interiors {
            area -= Self::ring_signed_area_2d(hole).abs();
        }
        area
    }

    /// Computes the 2D centroid directly without allocating intermediate geometry.
    pub fn centroid_2d(&self) -> Option<geo_types::Point<f64>> {
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut total_area = 0.0;

        let (ext_area, ext_cx, ext_cy) = Self::ring_area_and_centroid_2d(&self.exterior);
        let ext_abs_area = ext_area.abs();
        if ext_abs_area < 1e-12 {
            return None;
        }
        total_area += ext_abs_area;
        cx += ext_cx * ext_abs_area;
        cy += ext_cy * ext_abs_area;

        for hole in &self.interiors {
            let (hole_area, hole_cx, hole_cy) = Self::ring_area_and_centroid_2d(hole);
            let hole_abs_area = hole_area.abs();
            // Subtract holes based on their structural role, independent of winding order.
            total_area -= hole_abs_area;
            cx -= hole_cx * hole_abs_area;
            cy -= hole_cy * hole_abs_area;
        }

        if total_area.abs() < 1e-12 {
            None
        } else {
            Some(geo_types::Point::new(cx / total_area, cy / total_area))
        }
    }

    #[inline]
    pub fn ring_signed_area_2d(coords: &[Coord3D]) -> f64 {
        if coords.len() < 3 {
            return 0.0;
        }
        let mut twice_area = 0.0;
        let mut prev = coords[coords.len() - 1];
        for curr in coords {
            twice_area += (prev.x - curr.x) * (prev.y + curr.y);
            prev = *curr;
        }
        twice_area / 2.0
    }

    #[inline]
    pub fn ring_area_and_centroid_2d(coords: &[Coord3D]) -> (f64, f64, f64) {
        if coords.len() < 3 {
            return (0.0, 0.0, 0.0);
        }
        let origin_x = coords[0].x;
        let origin_y = coords[0].y;
        let mut twice_area = 0.0;
        let mut cx = 0.0;
        let mut cy = 0.0;
        let prev = coords[coords.len() - 1];
        let mut p1_x = prev.x - origin_x;
        let mut p1_y = prev.y - origin_y;

        for curr in coords {
            let p2_x = curr.x - origin_x;
            let p2_y = curr.y - origin_y;
            let f = p1_x * p2_y - p2_x * p1_y;
            twice_area += f;
            cx += (p1_x + p2_x) * f;
            cy += (p1_y + p2_y) * f;
            p1_x = p2_x;
            p1_y = p2_y;
        }
        let area = twice_area / 2.0;
        if area == 0.0 {
            return (0.0, 0.0, 0.0);
        }
        (
            area,
            cx / (3.0 * twice_area) + origin_x,
            cy / (3.0 * twice_area) + origin_y,
        )
    }
}

// Implement basic arithmetic for interpolation
impl Add for Coord3D {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Coord3D {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Mul<f64> for Coord3D {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::Coord;

    #[test]
    fn test_coord3d_new() {
        let c = Coord3D::new(1.0, 2.0, 3.0);
        assert_eq!(c.x, 1.0);
        assert_eq!(c.y, 2.0);
        assert_eq!(c.z, 3.0);
    }

    #[test]
    fn test_coord3d_to_coord_2d() {
        let c = Coord3D::new(1.0, 2.0, 3.0);
        let c2d = c.to_coord_2d();
        assert_eq!(c2d.x, 1.0);
        assert_eq!(c2d.y, 2.0);
    }

    #[test]
    fn test_coord3d_from_coord_2d() {
        let c2d = Coord { x: 1.0, y: 2.0 };
        let c3d: Coord3D = c2d.into();
        assert_eq!(c3d.x, 1.0);
        assert_eq!(c3d.y, 2.0);
        assert_eq!(c3d.z, 0.0);
    }

    #[test]
    fn test_coord3d_add() {
        let c1 = Coord3D::new(1.0, 2.0, 3.0);
        let c2 = Coord3D::new(4.0, 5.0, 6.0);
        assert_eq!(c1 + c2, Coord3D::new(5.0, 7.0, 9.0));

        let c3 = Coord3D::new(-1.0, 0.0, -3.0);
        assert_eq!(c1 + c3, Coord3D::new(0.0, 2.0, 0.0));
    }

    #[test]
    fn test_coord3d_sub() {
        let c1 = Coord3D::new(1.0, 2.0, 3.0);
        let c2 = Coord3D::new(4.0, 5.0, 6.0);
        assert_eq!(c2 - c1, Coord3D::new(3.0, 3.0, 3.0));

        let c3 = Coord3D::new(1.0, 2.0, 3.0);
        assert_eq!(c1 - c3, Coord3D::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_coord3d_mul() {
        let c1 = Coord3D::new(1.0, 2.0, 3.0);

        // Positive scalar
        assert_eq!(c1 * 2.0, Coord3D::new(2.0, 4.0, 6.0));

        // Negative scalar
        assert_eq!(c1 * -1.5, Coord3D::new(-1.5, -3.0, -4.5));

        // Zero scalar
        assert_eq!(c1 * 0.0, Coord3D::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_coord3d_arithmetic_chain() {
        let c1 = Coord3D::new(1.0, 2.0, 3.0);
        let c2 = Coord3D::new(3.0, 2.0, 1.0);
        let c3 = Coord3D::new(1.0, 1.0, 1.0);

        // (c1 + c2) * 0.5 - c3
        // (4, 4, 4) * 0.5 - (1, 1, 1) = (2, 2, 2) - (1, 1, 1) = (1, 1, 1)
        let result = (c1 + c2) * 0.5 - c3;
        assert_eq!(result, Coord3D::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn test_centroid_winding_independence() {
        // Exterior is CCW (positive area)
        let ext = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];

        // Hole is also CCW (usually holes are CW, but we want to test independence)
        let hole = vec![
            Coord3D::new(2.0, 2.0, 0.0),
            Coord3D::new(8.0, 2.0, 0.0),
            Coord3D::new(8.0, 8.0, 0.0),
            Coord3D::new(2.0, 8.0, 0.0),
            Coord3D::new(2.0, 2.0, 0.0),
        ];

        let poly = Polygon3D::new(ext, vec![hole], vec![], vec![vec![]]);
        let Some(centroid) = poly.centroid_2d() else {
            panic!("Centroid calculation failed and returned None");
        };
        // Since it's a symmetric hole in a symmetric square, the centroid should be exactly at the center (5, 5).
        // If winding independence failed, it might add instead of subtract or produce a wildly wrong result.
        assert!((centroid.x() - 5.0).abs() < 1e-6);
        assert!((centroid.y() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_centroid_numeric_stability_at_large_offsets() {
        let offset = 10_000_000.0;
        let ext = vec![
            Coord3D::new(offset, offset, 0.0),
            Coord3D::new(offset + 0.001, offset, 0.0),
            Coord3D::new(offset + 0.001, offset + 0.001, 0.0),
            Coord3D::new(offset, offset + 0.001, 0.0),
            Coord3D::new(offset, offset, 0.0),
        ];

        let poly = Polygon3D::new(ext, vec![], vec![], vec![]);
        let Some(centroid) = poly.centroid_2d() else {
            panic!("Centroid calculation failed and returned None");
        };

        // Centroid should be exactly at the center of the small square
        let expected_x = offset + 0.0005;
        let expected_y = offset + 0.0005;

        // If there is catastrophic cancellation, the error will be large relative to the small dimensions.
        println!(
            "centroid = {:?}, expected = {:?}",
            centroid,
            (expected_x, expected_y)
        );
        assert!((centroid.x() - expected_x).abs() < 1e-8);
        assert!((centroid.y() - expected_y).abs() < 1e-8);
    }

    #[test]
    fn test_centroid_empty_exterior() {
        let poly = Polygon3D::new(vec![], vec![], vec![], vec![]);
        assert_eq!(poly.centroid_2d(), None);
    }

    #[test]
    fn test_centroid_empty_polygon() {
        let poly = Polygon3D::new(vec![], vec![vec![]], vec![], vec![vec![]]);
        assert_eq!(poly.centroid_2d(), None);
    }
}
