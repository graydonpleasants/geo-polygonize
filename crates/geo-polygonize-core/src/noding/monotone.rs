//! Research-only monotone-chain candidate prototype.

use crate::index::{IndexedEnvelope, RStarBackend};
use crate::types::{Line3D, SourceChainKind, SourceLineString};
use crate::{PolygonizeError, Result};
use rstar::AABB;

#[derive(Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn from_line(line: Line3D) -> Self {
        Self {
            min_x: line.start.x.min(line.end.x),
            min_y: line.start.y.min(line.end.y),
            max_x: line.start.x.max(line.end.x),
            max_y: line.start.y.max(line.end.y),
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self.max_x >= other.min_x
            && self.min_x <= other.max_x
            && self.max_y >= other.min_y
            && self.min_y <= other.max_y
    }

    fn aabb(self) -> AABB<[f64; 2]> {
        AABB::from_corners([self.min_x, self.min_y], [self.max_x, self.max_y])
    }
}

#[derive(Clone, Copy)]
struct MonotoneChain {
    start: usize,
    end: usize,
    bounds: Bounds,
}

/// Benchmark-only compatibility entrypoint for detached source ranges.
///
/// The crate's hidden `noding` module keeps this compiler-public for the
/// native benchmark; production candidate dispatch uses source-chain identity.
#[doc(hidden)]
pub fn enumerate_candidates(
    lines: &[Line3D],
    source_ranges: &[(usize, usize)],
) -> Result<Vec<(usize, usize)>> {
    enumerate_candidates_from_ranges(lines, source_ranges)
}

/// Enumerate deterministic segment-envelope candidates for original source chains.
///
/// This is a benchmark-only prototype. It requires finite coordinates and
/// sorted, disjoint ranges; it has no execution-policy accounting or
/// cancellation. Candidates are envelope overlaps, not exact intersections.
pub(crate) fn enumerate_source_chain_candidates(
    lines: &[Line3D],
    source_chains: &[SourceLineString],
) -> Result<Vec<(usize, usize)>> {
    let source_ranges: Vec<_> = source_chains
        .iter()
        .filter(|chain| chain.kind == SourceChainKind::Original)
        .map(|chain| (chain.segment_start, chain.segment_count))
        .collect();
    enumerate_candidates_from_ranges(lines, &source_ranges)
}

fn enumerate_candidates_from_ranges(
    lines: &[Line3D],
    source_ranges: &[(usize, usize)],
) -> Result<Vec<(usize, usize)>> {
    let segment_bounds: Vec<_> = lines
        .iter()
        .copied()
        .map(|line| {
            if line.start.x.is_finite()
                && line.start.y.is_finite()
                && line.end.x.is_finite()
                && line.end.y.is_finite()
            {
                Ok(Bounds::from_line(line))
            } else {
                Err(PolygonizeError::InvalidGeometry {
                    reason: "monotone-chain prototype requires finite coordinates".to_string(),
                })
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let chains = build_chains(lines, &segment_bounds, source_ranges)?;
    let chain_index = RStarBackend::new(
        chains
            .iter()
            .enumerate()
            .map(|(index, chain)| IndexedEnvelope {
                aabb: chain.bounds.aabb(),
                index,
            })
            .collect(),
    );
    let mut candidates = Vec::new();

    for (first_chain_index, first_chain) in chains.iter().enumerate() {
        collect_within_chain(
            first_chain.start,
            first_chain.end,
            &segment_bounds,
            &mut candidates,
        );
        for second_chain_index in chain_index
            .locate_in_envelope_intersecting(&first_chain.bounds.aabb())
            .filter(|&index| index > first_chain_index)
        {
            let second_chain = chains[second_chain_index];
            collect_between_chains(
                first_chain.start,
                first_chain.end,
                second_chain.start,
                second_chain.end,
                &segment_bounds,
                &mut candidates,
            );
        }
    }

    candidates.sort_unstable();
    candidates.dedup();
    Ok(candidates)
}

fn build_chains(
    lines: &[Line3D],
    segment_bounds: &[Bounds],
    source_ranges: &[(usize, usize)],
) -> Result<Vec<MonotoneChain>> {
    let mut chains = Vec::new();
    let mut previous_end = 0;

    for &(start, count) in source_ranges {
        if count == 0 {
            continue;
        }
        let end = start
            .checked_add(count)
            .ok_or_else(|| invalid_range(start, count))?;
        if start >= lines.len() || end > lines.len() || start < previous_end {
            return Err(invalid_range(start, count));
        }
        previous_end = end;

        let mut chain_start = start;
        let mut direction = segment_direction(lines[start]);
        for (segment, line) in lines.iter().enumerate().take(end).skip(start + 1) {
            let next_direction = segment_direction(*line);
            if next_direction != direction {
                chains.push(make_chain(segment_bounds, chain_start, segment));
                chain_start = segment;
                direction = next_direction;
            }
        }
        chains.push(make_chain(segment_bounds, chain_start, end));
    }
    Ok(chains)
}

fn make_chain(segment_bounds: &[Bounds], start: usize, end: usize) -> MonotoneChain {
    MonotoneChain {
        start,
        end,
        bounds: range_bounds(segment_bounds, start, end),
    }
}

fn segment_direction(line: Line3D) -> (i8, i8) {
    (
        sign(line.end.x - line.start.x),
        sign(line.end.y - line.start.y),
    )
}

fn sign(value: f64) -> i8 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn range_bounds(segment_bounds: &[Bounds], start: usize, end: usize) -> Bounds {
    segment_bounds[start..end]
        .iter()
        .copied()
        .reduce(|first, second| Bounds {
            min_x: first.min_x.min(second.min_x),
            min_y: first.min_y.min(second.min_y),
            max_x: first.max_x.max(second.max_x),
            max_y: first.max_y.max(second.max_y),
        })
        .expect("monotone chain range is non-empty")
}

fn collect_within_chain(
    start: usize,
    end: usize,
    segment_bounds: &[Bounds],
    candidates: &mut Vec<(usize, usize)>,
) {
    if end - start < 2 {
        return;
    }
    let middle = start + (end - start) / 2;
    collect_between_chains(start, middle, middle, end, segment_bounds, candidates);
    collect_within_chain(start, middle, segment_bounds, candidates);
    collect_within_chain(middle, end, segment_bounds, candidates);
}

fn collect_between_chains(
    first_start: usize,
    first_end: usize,
    second_start: usize,
    second_end: usize,
    segment_bounds: &[Bounds],
    candidates: &mut Vec<(usize, usize)>,
) {
    if !range_bounds(segment_bounds, first_start, first_end).overlaps(range_bounds(
        segment_bounds,
        second_start,
        second_end,
    )) {
        return;
    }
    let first_len = first_end - first_start;
    let second_len = second_end - second_start;
    if first_len == 1 && second_len == 1 {
        let first = first_start;
        let second = second_start;
        if first != second && segment_bounds[first].overlaps(segment_bounds[second]) {
            candidates.push(if first < second {
                (first, second)
            } else {
                (second, first)
            });
        }
        return;
    }

    if first_len >= second_len && first_len > 1 {
        let middle = first_start + first_len / 2;
        collect_between_chains(
            first_start,
            middle,
            second_start,
            second_end,
            segment_bounds,
            candidates,
        );
        collect_between_chains(
            middle,
            first_end,
            second_start,
            second_end,
            segment_bounds,
            candidates,
        );
    } else {
        let middle = second_start + second_len / 2;
        collect_between_chains(
            first_start,
            first_end,
            second_start,
            middle,
            segment_bounds,
            candidates,
        );
        collect_between_chains(
            first_start,
            first_end,
            middle,
            second_end,
            segment_bounds,
            candidates,
        );
    }
}

fn invalid_range(start: usize, count: usize) -> PolygonizeError {
    PolygonizeError::InvalidGeometry {
        reason: format!("invalid monotone-chain source range ({start}, {count})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Coord3D, SourceChainKind};

    fn line(start: (f64, f64), end: (f64, f64)) -> Line3D {
        Line3D::new(
            Coord3D::new(start.0, start.1, 0.0),
            Coord3D::new(end.0, end.1, 0.0),
            0,
        )
    }

    #[test]
    fn recursively_prunes_long_chain_candidates_without_misses() {
        let lines = [
            line((0.0, 0.0), (1.0, 0.0)),
            line((1.0, 0.0), (2.0, 0.0)),
            line((2.0, 0.0), (3.0, 0.0)),
            line((3.0, 0.0), (4.0, 0.0)),
            line((2.5, -1.0), (2.5, 1.0)),
        ];
        let chains = [
            SourceLineString {
                segment_start: 0,
                segment_count: 4,
                source_id: Some(1),
                kind: SourceChainKind::Original,
            },
            SourceLineString {
                segment_start: 4,
                segment_count: 1,
                source_id: Some(2),
                kind: SourceChainKind::Original,
            },
        ];
        let actual = enumerate_source_chain_candidates(&lines, &chains).unwrap();

        assert_eq!(actual, vec![(0, 1), (1, 2), (2, 3), (2, 4)]);
    }

    #[test]
    fn rejects_overlapping_source_ranges() {
        let lines = [line((0.0, 0.0), (1.0, 0.0)), line((1.0, 0.0), (2.0, 0.0))];
        assert!(matches!(
            enumerate_source_chain_candidates(
                &lines,
                &[
                    SourceLineString {
                        segment_start: 0,
                        segment_count: 2,
                        source_id: Some(1),
                        kind: SourceChainKind::Original,
                    },
                    SourceLineString {
                        segment_start: 1,
                        segment_count: 1,
                        source_id: Some(2),
                        kind: SourceChainKind::Original,
                    },
                ],
            ),
            Err(PolygonizeError::InvalidGeometry { .. })
        ));
    }

    #[test]
    fn ignores_synthetic_and_unavailable_chains() {
        let lines = [line((0.0, 0.0), (1.0, 1.0)), line((0.0, 1.0), (1.0, 0.0))];
        let chains = [
            SourceLineString {
                segment_start: 0,
                segment_count: 1,
                source_id: Some(1),
                kind: SourceChainKind::Synthetic,
            },
            SourceLineString {
                segment_start: 1,
                segment_count: 1,
                source_id: None,
                kind: SourceChainKind::Unavailable,
            },
        ];

        assert!(enumerate_source_chain_candidates(&lines, &chains)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn long_sparse_workload_matches_bruteforce_envelope_candidates() {
        let long_segments = 2_048;
        let mut lines = Vec::with_capacity(long_segments + long_segments / 32);
        let mut source_ranges = vec![(0, long_segments)];
        for index in 0..long_segments {
            lines.push(line((index as f64, 0.0), (index as f64 + 1.0, 0.0)));
        }
        for index in (0..long_segments).step_by(32) {
            let segment_index = lines.len();
            let x = index as f64 + 0.5;
            lines.push(line((x, -1.0), (x, 1.0)));
            source_ranges.push((segment_index, 1));
        }

        let mut expected = Vec::new();
        let bounds: Vec<_> = lines.iter().copied().map(Bounds::from_line).collect();
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                if bounds[first].overlaps(bounds[second]) {
                    expected.push((first, second));
                }
            }
        }

        assert_eq!(
            enumerate_candidates(&lines, &source_ranges).unwrap(),
            expected
        );
    }
}
