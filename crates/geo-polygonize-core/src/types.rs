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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line3D {
    pub start: Coord3D,
    pub end: Coord3D,
}

impl Line3D {
    pub fn new(start: Coord3D, end: Coord3D) -> Self {
        Self { start, end }
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
        }
    }
}

#[derive(Clone, Debug)]
pub struct Polygon3D {
    pub exterior: Vec<Coord3D>,
    pub interiors: Vec<Vec<Coord3D>>,
}

impl Polygon3D {
    pub fn new(exterior: Vec<Coord3D>, interiors: Vec<Vec<Coord3D>>) -> Self {
        Self {
            exterior,
            interiors,
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

    pub fn into_inner(self) -> (Vec<Coord3D>, Vec<Vec<Coord3D>>) {
        (self.exterior, self.interiors)
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
