use crate::types::{Coord3D, Line3D};
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CandidatePair {
    pub(crate) first: usize,
    pub(crate) second: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CandidateIntersectionTrace {
    Point(Coord3D),
    Collinear(Coord3D, Coord3D),
}

/// The exact outcome shared by every floating candidate backend.
#[derive(Clone, Copy)]
pub(crate) struct ExactCandidate {
    pub(crate) pair: CandidatePair,
    pub(crate) first: Line3D,
    pub(crate) second: Line3D,
    pub(crate) intersection: Option<LineIntersection<f64>>,
}

impl ExactCandidate {
    pub(crate) fn evaluate(lines: &[Line3D], pair: CandidatePair) -> Self {
        let first = lines[pair.first];
        let second = lines[pair.second];
        Self {
            pair,
            first,
            second,
            intersection: line_intersection(first.to_line_2d(), second.to_line_2d()),
        }
    }

    pub(crate) fn witness(self) -> Option<CandidateIntersectionTrace> {
        self.intersection.map(|intersection| match intersection {
            LineIntersection::SinglePoint { intersection, .. } => {
                CandidateIntersectionTrace::Point(Coord3D::new(intersection.x, intersection.y, 0.0))
            }
            LineIntersection::Collinear { intersection } => CandidateIntersectionTrace::Collinear(
                Coord3D::new(intersection.start.x, intersection.start.y, 0.0),
                Coord3D::new(intersection.end.x, intersection.end.y, 0.0),
            ),
        })
    }
}

pub mod grid;
pub mod hot_pixel;
#[allow(dead_code)] // Research-only candidate prototype; production dispatch does not call it yet.
pub mod monotone;
pub mod snap;
pub mod sweep;
pub mod validate;
