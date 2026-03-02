use geo_types::{Coord, Line, LineString, Polygon};
use std::ops::{Add, Mul, Sub};

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

    #[inline]
    fn ring_signed_area_2d(coords: &[Coord3D]) -> f64 {
        if coords.len() < 3 {
            return 0.0;
        }
        let mut twice_area = 0.0;
        let mut j = coords.len() - 1;
        for i in 0..coords.len() {
            twice_area += (coords[j].x - coords[i].x) * (coords[j].y + coords[i].y);
            j = i;
        }
        twice_area / 2.0
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
    fn test_coord3d_arithmetic() {
        let c1 = Coord3D::new(1.0, 2.0, 3.0);
        let c2 = Coord3D::new(4.0, 5.0, 6.0);

        // Add
        let sum = c1 + c2;
        assert_eq!(sum, Coord3D::new(5.0, 7.0, 9.0));

        // Sub
        let diff = c2 - c1;
        assert_eq!(diff, Coord3D::new(3.0, 3.0, 3.0));

        // Mul
        let scaled = c1 * 2.0;
        assert_eq!(scaled, Coord3D::new(2.0, 4.0, 6.0));

        // Negative and zero
        let c3 = Coord3D::new(-1.0, 0.0, 1.0);
        assert_eq!(c1 + c3, Coord3D::new(0.0, 2.0, 4.0));
        assert_eq!(c3 * -1.0, Coord3D::new(1.0, 0.0, -1.0));
    }

    #[test]
    fn test_line3d() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let end = Coord3D::new(1.0, 1.0, 1.0);
        let line = Line3D::new(start, end);

        let line2d = line.to_line_2d();
        assert_eq!(line2d.start.x, 0.0);
        assert_eq!(line2d.start.y, 0.0);
        assert_eq!(line2d.end.x, 1.0);
        assert_eq!(line2d.end.y, 1.0);

        let geo_line = geo_types::Line::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 });
        let line3d_from: Line3D = geo_line.into();
        assert_eq!(line3d_from.start.z, 0.0);
        assert_eq!(line3d_from.end.z, 0.0);
    }

    #[test]
    fn test_polygon3d() {
        let exterior = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        let interior = vec![
            Coord3D::new(2.0, 2.0, 1.0),
            Coord3D::new(2.0, 8.0, 1.0),
            Coord3D::new(8.0, 8.0, 1.0),
            Coord3D::new(8.0, 2.0, 1.0),
            Coord3D::new(2.0, 2.0, 1.0),
        ];

        let poly = Polygon3D::new(exterior, vec![interior]);
        let poly2d = poly.to_polygon_2d();

        assert_eq!(poly2d.exterior().0.len(), 5);
        assert_eq!(poly2d.interiors().len(), 1);
        assert_eq!(poly2d.interiors()[0].0.len(), 5);
    }
}
