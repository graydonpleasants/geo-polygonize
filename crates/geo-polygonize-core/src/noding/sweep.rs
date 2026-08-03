//! Research-only exact intersection prototype backed by `geo::Intersections`.

use geo::algorithm::line_intersection::LineIntersection;
use geo::algorithm::sweep::{Cross, Intersections};
use geo_types::Line;
use std::iter::FromIterator;

use crate::types::Line3D;

#[derive(Clone, Copy)]
struct IndexedLine {
    index: usize,
    line: Line<f64>,
}

impl Cross for IndexedLine {
    type Scalar = f64;

    fn line(&self) -> Line<Self::Scalar> {
        self.line
    }
}

/// Enumerate intersecting input pairs in deterministic input-index order.
///
/// This is an exact-hit prototype, not a replacement noding backend: it does
/// not expose envelope-overlap candidates, execution-policy accounting, or
/// cancellation. Collinear overlaps are retained as intersection results.
pub fn enumerate_intersections(lines: &[Line3D]) -> Vec<((usize, usize), LineIntersection<f64>)> {
    let mut intersections: Vec<_> =
        Intersections::from_iter(lines.iter().enumerate().map(|(index, line)| IndexedLine {
            index,
            line: line.to_line_2d(),
        }))
        .map(|(first, second, intersection)| {
            let pair = if first.index < second.index {
                (first.index, second.index)
            } else {
                (second.index, first.index)
            };
            (pair, intersection)
        })
        .collect();

    intersections.sort_unstable_by_key(|(pair, _)| *pair);
    intersections
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Coord3D;
    use geo::algorithm::line_intersection::line_intersection;

    fn line(start: (f64, f64), end: (f64, f64)) -> Line3D {
        Line3D::new(
            Coord3D::new(start.0, start.1, 0.0),
            Coord3D::new(end.0, end.1, 0.0),
            0,
        )
    }

    #[test]
    fn matches_pairwise_intersections_and_retains_overlaps() {
        let lines = [
            line((0.0, 0.0), (10.0, 0.0)),
            line((5.0, -1.0), (5.0, 1.0)),
            line((2.0, 0.0), (4.0, 0.0)),
            line((20.0, 0.0), (21.0, 0.0)),
            line((10.0, 0.0), (10.0, 3.0)),
        ];

        let expected: Vec<_> = lines
            .iter()
            .enumerate()
            .flat_map(|(first, line)| {
                lines
                    .iter()
                    .enumerate()
                    .skip(first + 1)
                    .filter_map(move |(second, other)| {
                        line_intersection(line.to_line_2d(), other.to_line_2d())
                            .map(|intersection| ((first, second), intersection))
                    })
            })
            .collect();
        let actual = enumerate_intersections(&lines);

        assert_eq!(
            actual.iter().map(|(pair, _)| *pair).collect::<Vec<_>>(),
            expected.iter().map(|(pair, _)| *pair).collect::<Vec<_>>()
        );
        assert!(matches!(actual[1].1, LineIntersection::Collinear { .. }));
    }
}
