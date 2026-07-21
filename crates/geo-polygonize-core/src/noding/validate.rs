use crate::error::{PolygonizeError, Result};
use crate::index::{IndexedEnvelope, RStarBackend};
use crate::types::Line3D;
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};
use geo_types::Coord;
use rstar::AABB;

/// Independently verifies that segment linework is fully noded and normalized.
#[derive(Debug, Default)]
pub struct ValidatingNoder;

impl ValidatingNoder {
    pub fn new() -> Self {
        Self
    }

    /// Returns the first pair containing an interior intersection or overlap.
    pub fn validate(&self, lines: &[Line3D]) -> Result<()> {
        let index = RStarBackend::new(
            lines
                .iter()
                .enumerate()
                .map(|(index, line)| IndexedEnvelope {
                    aabb: envelope(line),
                    index,
                })
                .collect(),
        );

        for (first_index, first) in lines.iter().enumerate() {
            if first.start.x == first.end.x && first.start.y == first.end.y {
                return failure(
                    first_index,
                    first_index,
                    "zero-length segment is not normalized".to_string(),
                );
            }

            let mut candidates: Vec<_> = index
                .locate_in_envelope_intersecting(&envelope(first))
                .filter(|second_index| *second_index > first_index)
                .collect();
            candidates.sort_unstable();
            for second_index in candidates {
                let second = &lines[second_index];
                if same_segment_xy(first, second) {
                    continue;
                }
                match line_intersection(first.to_line_2d(), second.to_line_2d()) {
                    Some(LineIntersection::SinglePoint {
                        intersection,
                        is_proper,
                    }) if is_proper
                        || !is_endpoint(first, intersection)
                        || !is_endpoint(second, intersection) =>
                    {
                        return failure(
                            first_index,
                            second_index,
                            format!(
                                "intersection ({}, {}) is not an endpoint of both segments",
                                intersection.x, intersection.y
                            ),
                        );
                    }
                    Some(LineIntersection::Collinear { intersection }) => {
                        return failure(
                            first_index,
                            second_index,
                            format!(
                                "collinear overlap from ({}, {}) to ({}, {}) is not normalized",
                                intersection.start.x,
                                intersection.start.y,
                                intersection.end.x,
                                intersection.end.y
                            ),
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

fn same_segment_xy(first: &Line3D, second: &Line3D) -> bool {
    ((first.start.x == second.start.x && first.start.y == second.start.y)
        && (first.end.x == second.end.x && first.end.y == second.end.y))
        || ((first.start.x == second.end.x && first.start.y == second.end.y)
            && (first.end.x == second.start.x && first.end.y == second.start.y))
}

fn envelope(line: &Line3D) -> AABB<[f64; 2]> {
    AABB::from_corners(
        [line.start.x.min(line.end.x), line.start.y.min(line.end.y)],
        [line.start.x.max(line.end.x), line.start.y.max(line.end.y)],
    )
}

fn is_endpoint(line: &Line3D, point: Coord<f64>) -> bool {
    (line.start.x == point.x && line.start.y == point.y)
        || (line.end.x == point.x && line.end.y == point.y)
}

fn failure(first_segment: usize, second_segment: usize, reason: String) -> Result<()> {
    Err(PolygonizeError::NodingValidationFailure {
        first_segment,
        second_segment,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Coord3D;

    fn line(start: (f64, f64), end: (f64, f64)) -> Line3D {
        Line3D::new(
            Coord3D::new(start.0, start.1, 0.0),
            Coord3D::new(end.0, end.1, 0.0),
            0,
        )
    }

    #[test]
    fn accepts_fully_noded_crossing() {
        let lines = [
            line((-1.0, 0.0), (0.0, 0.0)),
            line((0.0, 0.0), (1.0, 0.0)),
            line((0.0, -1.0), (0.0, 0.0)),
            line((0.0, 0.0), (0.0, 1.0)),
        ];
        ValidatingNoder::new().validate(&lines).unwrap();
    }

    #[test]
    fn rejects_proper_intersection() {
        let lines = [line((-1.0, 0.0), (1.0, 0.0)), line((0.0, -1.0), (0.0, 1.0))];
        assert!(matches!(
            ValidatingNoder::new().validate(&lines),
            Err(PolygonizeError::NodingValidationFailure { .. })
        ));
    }

    #[test]
    fn rejects_endpoint_in_segment_interior() {
        let lines = [line((-1.0, 0.0), (1.0, 0.0)), line((0.0, 0.0), (0.0, 1.0))];
        assert!(ValidatingNoder::new().validate(&lines).is_err());
    }

    #[test]
    fn rejects_collinear_overlap() {
        let lines = [line((0.0, 0.0), (2.0, 0.0)), line((1.0, 0.0), (3.0, 0.0))];
        assert!(ValidatingNoder::new().validate(&lines).is_err());
    }

    #[test]
    fn accepts_coincident_segments_as_source_carriers() {
        let lines = [line((0.0, 0.0), (2.0, 0.0)), line((2.0, 0.0), (0.0, 0.0))];
        ValidatingNoder::new().validate(&lines).unwrap();
    }
}
