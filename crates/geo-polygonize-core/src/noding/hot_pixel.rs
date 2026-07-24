use crate::diagnostics::NodingWorkStats;
use crate::error::{PolygonizeError, Result};
use crate::index::{IndexedEnvelope, RStarBackend};
use crate::noding::snap::SnapNoder;
use crate::noding::validate::ValidatingNoder;
use crate::options::{ExecutionPolicy, ZPolicy};
use crate::types::{Coord3D, IPoint, Line3D};
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};
use rstar::AABB;

const MAX_EXACT_GRID_INDEX: f64 = (1_u64 << 52) as f64;

/// Fixed-precision hot-pixel snap-rounding noder.
pub struct HotPixelNoder {
    grid_size: f64,
    z_policy: ZPolicy,
}

impl HotPixelNoder {
    pub fn new(grid_size: f64) -> Result<Self> {
        if !grid_size.is_finite() || grid_size <= 0.0 {
            return Err(PolygonizeError::InvalidArgumentType {
                field: "grid_size".to_string(),
                expected: "a finite positive number".to_string(),
                actual: grid_size.to_string(),
            });
        }
        Ok(Self {
            grid_size,
            z_policy: ZPolicy::InterpolateAlongEdge,
        })
    }

    pub fn with_z_policy(mut self, z_policy: ZPolicy) -> Self {
        self.z_policy = z_policy;
        self
    }

    pub fn node(&self, lines: Vec<Line3D>) -> Result<Vec<Line3D>> {
        self.node_with_stats_impl(lines, None)
            .map(|(lines, _, _)| lines)
    }

    pub(crate) fn node_with_stats(
        &self,
        lines: Vec<Line3D>,
    ) -> Result<(Vec<Line3D>, usize, NodingWorkStats)> {
        self.node_with_stats_impl(lines, None)
    }

    pub(crate) fn node_with_execution_policy(
        &self,
        lines: Vec<Line3D>,
        execution_policy: &ExecutionPolicy,
    ) -> Result<Vec<Line3D>> {
        self.node_with_stats_impl(lines, Some(execution_policy))
            .map(|(lines, _, _)| lines)
    }

    pub(crate) fn node_with_stats_and_execution_policy(
        &self,
        lines: Vec<Line3D>,
        execution_policy: &ExecutionPolicy,
    ) -> Result<(Vec<Line3D>, usize, NodingWorkStats)> {
        self.node_with_stats_impl(lines, Some(execution_policy))
    }

    fn node_with_stats_impl(
        &self,
        mut lines: Vec<Line3D>,
        execution_policy: Option<&ExecutionPolicy>,
    ) -> Result<(Vec<Line3D>, usize, NodingWorkStats)> {
        for line in &mut lines {
            line.start = self.snap(line.start)?;
            line.end = self.snap(line.end)?;
        }
        lines.retain(|line| line.start.x != line.end.x || line.start.y != line.end.y);

        let snapper = SnapNoder::new(self.grid_size).with_z_policy(self.z_policy);
        snapper.normalize_and_dedup(&mut lines);

        let mut hot_pixels = Vec::with_capacity(lines.len() * 2);
        for line in &lines {
            hot_pixels.push(self.grid_point(line.start)?);
            hot_pixels.push(self.grid_point(line.end)?);
        }

        let segment_index = RStarBackend::new(
            lines
                .iter()
                .enumerate()
                .map(|(index, line)| IndexedEnvelope {
                    aabb: line_envelope(line),
                    index,
                })
                .collect(),
        );
        let mut intersections = 0;
        let mut work = NodingWorkStats::default();
        for (first_index, first) in lines.iter().enumerate() {
            let mut candidates: Vec<_> = segment_index
                .locate_in_envelope_intersecting(&line_envelope(first))
                .filter(|second_index| *second_index > first_index)
                .collect();
            candidates.sort_unstable();
            for second_index in candidates {
                work.candidate_pairs = work.candidate_pairs.checked_add(1).ok_or_else(|| {
                    PolygonizeError::InternalInvariantViolation {
                        reason: "candidate-pair counter overflow".to_string(),
                    }
                })?;
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled("candidate_enumeration")?;
                    execution_policy
                        .check_noding_work(work.candidate_pairs, work.exact_intersection_calls)?;
                }
                work.exact_intersection_calls = work
                    .exact_intersection_calls
                    .checked_add(1)
                    .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                        reason: "exact-intersection counter overflow".to_string(),
                    })?;
                if let Some(execution_policy) = execution_policy {
                    execution_policy
                        .check_noding_work(work.candidate_pairs, work.exact_intersection_calls)?;
                }
                match line_intersection(first.to_line_2d(), lines[second_index].to_line_2d()) {
                    Some(LineIntersection::SinglePoint { intersection, .. }) => {
                        intersections += 1;
                        hot_pixels.push(self.grid_point(Coord3D::from(intersection))?);
                    }
                    Some(LineIntersection::Collinear { intersection }) => {
                        intersections += 1;
                        hot_pixels.push(self.grid_point(Coord3D::from(intersection.start))?);
                        hot_pixels.push(self.grid_point(Coord3D::from(intersection.end))?);
                    }
                    None => {}
                }
            }
        }
        hot_pixels.sort_unstable();
        hot_pixels.dedup();

        let hot_pixel_index = RStarBackend::new(
            hot_pixels
                .iter()
                .enumerate()
                .map(|(index, point)| IndexedEnvelope {
                    aabb: AABB::from_point([point.x as f64, point.y as f64]),
                    index,
                })
                .collect(),
        );
        let mut output = Vec::new();
        for line in lines {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled("split_application")?;
            }
            let start = self.grid_point(line.start)?;
            let end = self.grid_point(line.end)?;
            let query = AABB::from_corners(
                [
                    start.x.min(end.x) as f64 - 1.0,
                    start.y.min(end.y) as f64 - 1.0,
                ],
                [
                    start.x.max(end.x) as f64 + 1.0,
                    start.y.max(end.y) as f64 + 1.0,
                ],
            );
            let mut candidates: Vec<_> = hot_pixel_index
                .locate_in_envelope_intersecting(&query)
                .collect();
            candidates.sort_unstable();

            let mut points = vec![line.start, line.end];
            for (candidate_index, hot_pixel_index) in candidates.into_iter().enumerate() {
                if let Some(execution_policy) = execution_policy {
                    execution_policy.check_cancelled_every("split_application", candidate_index)?;
                }
                let hot_pixel = hot_pixels[hot_pixel_index];
                if hot_pixel == start
                    || hot_pixel == end
                    || segment_intersects_hot_pixel(start, end, hot_pixel)
                {
                    let point = self.coordinate(hot_pixel);
                    points.push(Coord3D::new(
                        point.x,
                        point.y,
                        snapper.interpolate_z(point.to_coord_2d(), line),
                    ));
                }
            }

            let dx = end.x - start.x;
            let dy = end.y - start.y;
            points.sort_unstable_by(|left, right| {
                let left_grid = self.grid_point(*left).expect("validated grid coordinate");
                let right_grid = self.grid_point(*right).expect("validated grid coordinate");
                let projection = |point: IPoint| {
                    (point.x - start.x) as i128 * dx as i128
                        + (point.y - start.y) as i128 * dy as i128
                };
                projection(left_grid).cmp(&projection(right_grid))
            });
            points.dedup_by(|left, right| left.x == right.x && left.y == right.y);
            work.split_events = work
                .split_events
                .checked_add(points.len().saturating_sub(2))
                .ok_or_else(|| PolygonizeError::InternalInvariantViolation {
                    reason: "split-event counter overflow".to_string(),
                })?;
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_split_events(work.split_events)?;
            }
            for (replacement_index, pair) in points.windows(2).enumerate() {
                if let Some(execution_policy) = execution_policy {
                    execution_policy
                        .check_cancelled_every("split_application", replacement_index)?;
                }
                if pair[0].x != pair[1].x || pair[0].y != pair[1].y {
                    output.push(Line3D::new(pair[0], pair[1], line.line_id));
                }
            }
        }

        snapper.normalize_and_dedup(&mut output);
        ValidatingNoder::new().validate(&output)?;
        Ok((output, intersections, work))
    }

    fn grid_point(&self, coordinate: Coord3D) -> Result<IPoint> {
        let index = |value: f64| {
            let value = (value / self.grid_size).round();
            if !value.is_finite() || value.abs() > MAX_EXACT_GRID_INDEX {
                Err(PolygonizeError::InvalidGeometry {
                    reason: "coordinate exceeds the exact range of the configured fixed grid"
                        .to_string(),
                })
            } else {
                Ok(value as i64)
            }
        };
        Ok(IPoint::new(index(coordinate.x)?, index(coordinate.y)?))
    }

    fn coordinate(&self, point: IPoint) -> Coord3D {
        Coord3D::new(
            point.x as f64 * self.grid_size,
            point.y as f64 * self.grid_size,
            0.0,
        )
    }

    fn snap(&self, coordinate: Coord3D) -> Result<Coord3D> {
        if !coordinate.z.is_finite() {
            return Err(PolygonizeError::InvalidGeometry {
                reason: "coordinates must contain only finite XYZ values".to_string(),
            });
        }
        let point = self.coordinate(self.grid_point(coordinate)?);
        Ok(Coord3D::new(point.x, point.y, coordinate.z))
    }
}

fn line_envelope(line: &Line3D) -> AABB<[f64; 2]> {
    AABB::from_corners(
        [line.start.x.min(line.end.x), line.start.y.min(line.end.y)],
        [line.start.x.max(line.end.x), line.start.y.max(line.end.y)],
    )
}

fn segment_intersects_hot_pixel(start: IPoint, end: IPoint, hot_pixel: IPoint) -> bool {
    let start_x = start.x as f64;
    let start_y = start.y as f64;
    let dx = (end.x - start.x) as f64;
    let dy = (end.y - start.y) as f64;
    let mut lower = 0.0_f64;
    let mut upper = 1.0_f64;

    let mut clip = |origin: f64, delta: f64, center: i64| {
        let min = center as f64 - 0.5;
        let max = center as f64 + 0.5;
        if delta == 0.0 {
            return origin > min && origin < max;
        }
        let first = (min - origin) / delta;
        let second = (max - origin) / delta;
        lower = lower.max(first.min(second));
        upper = upper.min(first.max(second));
        lower < upper
    };

    clip(start_x, dx, hot_pixel.x) && clip(start_y, dy, hot_pixel.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start: (f64, f64), end: (f64, f64), line_id: u32) -> Line3D {
        Line3D::new(
            Coord3D::new(start.0, start.1, 0.0),
            Coord3D::new(end.0, end.1, 0.0),
            line_id,
        )
    }

    #[test]
    fn snaps_every_segment_through_a_hot_pixel() {
        let lines = vec![
            line((-2.0, 0.0), (3.0, 1.0), 1),
            line((0.0, 0.0), (0.0, -2.0), 2),
        ];
        let output = HotPixelNoder::new(1.0).unwrap().node(lines).unwrap();

        assert!(
            output
                .iter()
                .filter(|line| line.start.x == 0.0 && line.start.y == 0.0
                    || line.end.x == 0.0 && line.end.y == 0.0)
                .count()
                >= 3
        );
        ValidatingNoder::new().validate(&output).unwrap();
    }

    #[test]
    fn rejects_coordinates_outside_the_exact_grid_range() {
        let lines = vec![line((0.0, 0.0), (1e16, 1.0), 1)];
        assert!(HotPixelNoder::new(1.0).unwrap().node(lines).is_err());
    }

    #[test]
    fn normalizes_crossings_and_overlaps_on_the_grid() {
        let lines = vec![
            line((-3.0, 0.0), (3.0, 1.0), 1),
            line((0.0, -3.0), (0.0, 3.0), 2),
            line((-2.0, 2.0), (2.0, -2.0), 3),
            line((-2.0, 2.0), (0.0, 0.0), 4),
        ];
        let output = HotPixelNoder::new(1.0).unwrap().node(lines).unwrap();

        ValidatingNoder::new().validate(&output).unwrap();
        assert!(output.iter().all(|line| [line.start, line.end]
            .iter()
            .all(|point| point.x == point.x.round() && point.y == point.y.round())));
    }
}
