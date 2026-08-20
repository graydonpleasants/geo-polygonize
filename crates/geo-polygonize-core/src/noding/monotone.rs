//! Research-only MCIndex-style monotone-chain candidate prototype.

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

    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }
}

#[derive(Clone, Copy)]
struct MonotoneChainNode {
    start: usize,
    end: usize,
    bounds: Bounds,
    left: Option<usize>,
    right: Option<usize>,
}

#[derive(Clone, Copy)]
struct MonotoneChainRoot {
    node: usize,
    bounds: Bounds,
}

struct MonotoneChainTree {
    nodes: Vec<MonotoneChainNode>,
    roots: Vec<MonotoneChainRoot>,
}

impl MonotoneChainTree {
    fn from_ranges(segment_bounds: &[Bounds], ranges: &[(usize, usize)]) -> Self {
        let mut tree = Self {
            nodes: Vec::new(),
            roots: Vec::with_capacity(ranges.len()),
        };
        for &(start, end) in ranges {
            let node = tree.push_node(segment_bounds, start, end);
            tree.roots.push(MonotoneChainRoot {
                node,
                bounds: tree.nodes[node].bounds,
            });
        }
        tree
    }

    fn push_node(&mut self, segment_bounds: &[Bounds], start: usize, end: usize) -> usize {
        let node = self.nodes.len();
        self.nodes.push(MonotoneChainNode {
            start,
            end,
            bounds: segment_bounds[start],
            left: None,
            right: None,
        });
        if end - start > 1 {
            let middle = start + (end - start) / 2;
            let left = self.push_node(segment_bounds, start, middle);
            let right = self.push_node(segment_bounds, middle, end);
            self.nodes[node] = MonotoneChainNode {
                start,
                end,
                bounds: self.nodes[left].bounds.union(self.nodes[right].bounds),
                left: Some(left),
                right: Some(right),
            };
        }
        node
    }
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

/// Enumerate deterministic MCIndex-style segment-envelope candidates for retained original chains.
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
    let chain_ranges = build_chain_ranges(lines, source_ranges)?;
    let tree = MonotoneChainTree::from_ranges(&segment_bounds, &chain_ranges);
    let chain_index = RStarBackend::new(
        tree.roots
            .iter()
            .enumerate()
            .map(|(index, root)| IndexedEnvelope {
                aabb: root.bounds.aabb(),
                index,
            })
            .collect(),
    );
    let mut candidates = Vec::new();

    for (first_chain_index, first_root) in tree.roots.iter().enumerate() {
        collect_within_tree(&tree, first_root.node, &mut candidates);
        for second_chain_index in chain_index
            .locate_in_envelope_intersecting(&first_root.bounds.aabb())
            .filter(|&index| index > first_chain_index)
        {
            collect_between_nodes(
                &tree,
                first_root.node,
                tree.roots[second_chain_index].node,
                &mut candidates,
            );
        }
    }

    candidates.sort_unstable();
    candidates.dedup();
    Ok(candidates)
}

fn build_chain_ranges(
    lines: &[Line3D],
    source_ranges: &[(usize, usize)],
) -> Result<Vec<(usize, usize)>> {
    let mut ranges = Vec::new();
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
                ranges.push((chain_start, segment));
                chain_start = segment;
                direction = next_direction;
            }
        }
        ranges.push((chain_start, end));
    }
    Ok(ranges)
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

fn collect_within_tree(
    tree: &MonotoneChainTree,
    node_index: usize,
    candidates: &mut Vec<(usize, usize)>,
) {
    let node = tree.nodes[node_index];
    let (Some(left), Some(right)) = (node.left, node.right) else {
        return;
    };
    collect_between_nodes(tree, left, right, candidates);
    collect_within_tree(tree, left, candidates);
    collect_within_tree(tree, right, candidates);
}

fn collect_between_nodes(
    tree: &MonotoneChainTree,
    first_index: usize,
    second_index: usize,
    candidates: &mut Vec<(usize, usize)>,
) {
    let first = tree.nodes[first_index];
    let second = tree.nodes[second_index];
    if !first.bounds.overlaps(second.bounds) {
        return;
    }
    if first.left.is_none() && second.left.is_none() {
        candidates.push(if first.start < second.start {
            (first.start, second.start)
        } else {
            (second.start, first.start)
        });
        return;
    }

    let first_len = first.end - first.start;
    let second_len = second.end - second.start;
    if first.left.is_some() && (second.right.is_none() || first_len >= second_len) {
        let (Some(left), Some(right)) = (first.left, first.right) else {
            unreachable!("internal monotone-chain node has two children")
        };
        collect_between_nodes(tree, left, second_index, candidates);
        collect_between_nodes(tree, right, second_index, candidates);
    } else {
        let (Some(left), Some(right)) = (second.left, second.right) else {
            unreachable!("internal monotone-chain node has two children")
        };
        collect_between_nodes(tree, first_index, left, candidates);
        collect_between_nodes(tree, first_index, right, candidates);
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

    fn original_chains(points: &[&[(f64, f64)]]) -> (Vec<Line3D>, Vec<SourceLineString>) {
        let mut lines = Vec::new();
        let mut chains = Vec::with_capacity(points.len());

        for (source_id, points) in points.iter().enumerate() {
            let segment_start = lines.len();
            lines.extend(points.windows(2).map(|window| line(window[0], window[1])));
            chains.push(SourceLineString {
                segment_start,
                segment_count: points.len().saturating_sub(1),
                source_id: Some(source_id as u32),
                kind: SourceChainKind::Original,
            });
        }
        (lines, chains)
    }

    fn assert_matches_envelope_oracle(lines: &[Line3D], chains: &[SourceLineString]) {
        let mut eligible = vec![false; lines.len()];
        for chain in chains
            .iter()
            .filter(|chain| chain.kind == SourceChainKind::Original)
        {
            eligible[chain.segment_start..chain.segment_start + chain.segment_count].fill(true);
        }

        let bounds: Vec<_> = lines.iter().copied().map(Bounds::from_line).collect();
        let mut expected = Vec::new();
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                if eligible[first] && eligible[second] && bounds[first].overlaps(bounds[second]) {
                    expected.push((first, second));
                }
            }
        }

        assert_eq!(
            enumerate_source_chain_candidates(lines, chains).unwrap(),
            expected
        );
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
        let mut chains = vec![SourceLineString {
            segment_start: 0,
            segment_count: long_segments,
            source_id: Some(0),
            kind: SourceChainKind::Original,
        }];
        for index in 0..long_segments {
            lines.push(line((index as f64, 0.0), (index as f64 + 1.0, 0.0)));
        }
        for index in (0..long_segments).step_by(32) {
            let segment_index = lines.len();
            let x = index as f64 + 0.5;
            lines.push(line((x, -1.0), (x, 1.0)));
            chains.push(SourceLineString {
                segment_start: segment_index,
                segment_count: 1,
                source_id: Some(chains.len() as u32),
                kind: SourceChainKind::Original,
            });
        }

        assert_matches_envelope_oracle(&lines, &chains);
    }

    #[test]
    fn mcindex_matches_road_and_contour_chain_envelopes() {
        let (lines, chains) = original_chains(&[
            &[(0.0, 0.0), (3.0, 0.0), (6.0, 0.0)],
            &[(1.0, -1.0), (1.0, 1.0), (1.0, 3.0)],
            &[(5.0, -1.0), (5.0, 1.0)],
            &[(0.0, 10.0), (2.0, 11.0), (4.0, 10.0), (6.0, 11.0)],
            &[(0.0, 12.0), (2.0, 13.0), (4.0, 12.0), (6.0, 13.0)],
        ]);

        assert_matches_envelope_oracle(&lines, &chains);
    }

    #[test]
    fn mcindex_matches_mixed_and_collinear_chain_envelopes() {
        let (lines, chains) = original_chains(&[
            &[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (6.0, 0.0)],
            &[(1.0, -1.0), (1.0, 1.0)],
            &[(5.0, -1.0), (5.0, 1.0)],
            &[(2.0, 0.0), (5.0, 0.0)],
            &[(3.0, 0.0), (7.0, 0.0)],
        ]);

        assert_matches_envelope_oracle(&lines, &chains);
    }

    #[test]
    fn mcindex_matches_self_intersecting_and_component_local_chain_envelopes() {
        let (lines, chains) = original_chains(&[
            &[(0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)],
            &[(10.0, 10.0), (12.0, 10.0), (11.0, 12.0), (10.0, 10.0)],
            &[
                (100.0, 100.0),
                (102.0, 100.0),
                (101.0, 102.0),
                (100.0, 100.0),
            ],
        ]);

        assert_matches_envelope_oracle(&lines, &chains);
    }
}
