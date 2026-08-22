//! Research-only MCIndex-style monotone-chain candidate prototype.

use crate::diagnostics::{ExecutionWorkTracker, NodingWorkStats};
use crate::index::{IndexedEnvelope, RStarBackend};
use crate::noding::snap::SnapNoder;
use crate::noding::validate::ValidatingNoder;
use crate::noding::{CandidatePair, ExactCandidate};
use crate::options::ExecutionPolicy;
use crate::types::{Line3D, SourceChainKind, SourceLineString, SourceSegmentIdentity};
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

/// Run one source-chain-preserving MCIndex experiment through the shared split,
/// normalization, and validation path.
///
/// The input must already be the immutable segment representation described by
/// `source_chains`; preprocessing that reorders or replaces segments belongs
/// outside this research-only adapter until its identity mapping is explicit.
pub(crate) fn node_hybrid_source_chains(
    lines: Vec<Line3D>,
    source_chains: &[SourceLineString],
    snap_noder: &SnapNoder,
    execution_policy: &ExecutionPolicy,
) -> Result<(Vec<Line3D>, NodingWorkStats)> {
    let coverage = source_chain_coverage(lines.len(), source_chains)?;
    if coverage.segment_identities.iter().any(Option::is_none) {
        return Err(PolygonizeError::InvalidGeometry {
            reason: "MCIndex experiment requires complete source-chain coverage".to_string(),
        });
    }

    let mut events = Vec::new();
    let mut work_stats = NodingWorkStats::default();
    {
        let mut tracker = ExecutionWorkTracker::new(Some(execution_policy), Some(&mut work_stats));
        visit_hybrid_exact_candidates(&lines, source_chains, &mut tracker, |exact| {
            snap_noder.append_exact_candidate_splits(exact, &mut events);
            Ok(())
        })?;
    }

    let (noded, split_events) = if events.is_empty() {
        (lines, 0)
    } else {
        snap_noder.apply_split_events_for_research(&lines, events, execution_policy)?
    };
    work_stats.split_events = split_events;
    ValidatingNoder::new().validate_with_execution_policy(&noded, execution_policy)?;
    Ok((noded, work_stats))
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
    let coverage = source_chain_coverage(lines.len(), source_chains)?;
    enumerate_candidates_from_ranges(lines, &coverage.original_ranges)
}

/// Stream MCIndex envelope candidates through the shared candidate callback shape.
///
/// This remains a benchmark-only prototype: only original source chains are indexed,
/// and the callback must still route candidates through the shared exact path.
pub(crate) fn visit_source_chain_candidates<F>(
    lines: &[Line3D],
    source_chains: &[SourceLineString],
    tracker: &mut ExecutionWorkTracker<'_>,
    visit: F,
) -> Result<()>
where
    F: FnMut(CandidatePair) -> Result<()>,
{
    let coverage = source_chain_coverage(lines.len(), source_chains)?;
    visit_candidates_from_ranges(lines, &coverage.original_ranges, tracker, visit)
}

/// Stream the hybrid research experiment: MCIndex for original chains and a
/// fallback scan for every pair involving synthetic or unavailable segments.
pub(crate) fn visit_hybrid_source_chain_candidates<F>(
    lines: &[Line3D],
    source_chains: &[SourceLineString],
    tracker: &mut ExecutionWorkTracker<'_>,
    mut visit: F,
) -> Result<()>
where
    F: FnMut(CandidatePair) -> Result<()>,
{
    let coverage = source_chain_coverage(lines.len(), source_chains)?;
    visit_candidates_from_ranges(lines, &coverage.original_ranges, tracker, &mut visit)?;
    let bounds = build_segment_bounds(lines)?;
    for first in 0..lines.len() {
        for second in first + 1..lines.len() {
            if coverage.original_segments[first] && coverage.original_segments[second] {
                continue;
            }
            let overlaps = bounds[first].overlaps(bounds[second]);
            tracker.candidate(overlaps)?;
            if overlaps {
                visit(CandidatePair { first, second })?;
            }
        }
    }
    Ok(())
}

/// Evaluate streamed hybrid candidates with the shared floating exact path.
pub(crate) fn visit_hybrid_exact_candidates<F>(
    lines: &[Line3D],
    source_chains: &[SourceLineString],
    tracker: &mut ExecutionWorkTracker<'_>,
    mut visit: F,
) -> Result<()>
where
    F: FnMut(ExactCandidate) -> Result<()>,
{
    visit_hybrid_source_chain_candidates(lines, source_chains, tracker, |candidate| {
        visit(ExactCandidate::evaluate(lines, candidate))
    })
}

fn enumerate_candidates_from_ranges(
    lines: &[Line3D],
    source_ranges: &[(usize, usize)],
) -> Result<Vec<(usize, usize)>> {
    let mut tracker = ExecutionWorkTracker::new(None, None);
    let mut candidates = Vec::new();
    visit_candidates_from_ranges(lines, source_ranges, &mut tracker, |candidate| {
        candidates.push((candidate.first, candidate.second));
        Ok(())
    })?;
    candidates.sort_unstable();
    candidates.dedup();
    Ok(candidates)
}

struct SourceChainCoverage {
    original_ranges: Vec<(usize, usize)>,
    original_segments: Vec<bool>,
    segment_identities: Vec<Option<SourceSegmentIdentity>>,
}

fn source_chain_coverage(
    line_count: usize,
    source_chains: &[SourceLineString],
) -> Result<SourceChainCoverage> {
    let mut original_ranges = Vec::new();
    let mut original_segments = vec![false; line_count];
    let mut segment_identities = vec![None; line_count];
    let mut previous_end = 0;
    for (chain_index, chain) in source_chains.iter().enumerate() {
        if chain.segment_count == 0 {
            continue;
        }
        let end = chain
            .segment_start
            .checked_add(chain.segment_count)
            .ok_or_else(|| invalid_range(chain.segment_start, chain.segment_count))?;
        if chain.segment_start >= line_count
            || end > line_count
            || chain.segment_start < previous_end
        {
            return Err(invalid_range(chain.segment_start, chain.segment_count));
        }
        previous_end = end;
        for segment_index in 0..chain.segment_count {
            segment_identities[chain.segment_start + segment_index] = Some(SourceSegmentIdentity {
                source_id: chain.source_id,
                chain_index,
                segment_index,
                chain_segment_count: chain.segment_count,
                kind: chain.kind,
            });
        }
        if chain.kind == SourceChainKind::Original {
            original_ranges.push((chain.segment_start, chain.segment_count));
            original_segments[chain.segment_start..end].fill(true);
        }
    }
    Ok(SourceChainCoverage {
        original_ranges,
        original_segments,
        segment_identities,
    })
}

fn build_segment_bounds(lines: &[Line3D]) -> Result<Vec<Bounds>> {
    lines
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
        .collect()
}

fn visit_candidates_from_ranges<F>(
    lines: &[Line3D],
    source_ranges: &[(usize, usize)],
    tracker: &mut ExecutionWorkTracker<'_>,
    mut visit: F,
) -> Result<()>
where
    F: FnMut(CandidatePair) -> Result<()>,
{
    tracker.check_cancelled()?;
    let segment_bounds = build_segment_bounds(lines)?;
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
    for (first_chain_index, first_root) in tree.roots.iter().enumerate() {
        collect_within_tree(&tree, first_root.node, tracker, &mut visit)?;
        for second_chain_index in chain_index
            .locate_in_envelope_intersecting(&first_root.bounds.aabb())
            .filter(|&index| index > first_chain_index)
        {
            collect_between_nodes(
                &tree,
                first_root.node,
                tree.roots[second_chain_index].node,
                tracker,
                &mut visit,
            )?;
        }
    }
    Ok(())
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
    tracker: &mut ExecutionWorkTracker<'_>,
    visit: &mut impl FnMut(CandidatePair) -> Result<()>,
) -> Result<()> {
    let node = tree.nodes[node_index];
    let (Some(left), Some(right)) = (node.left, node.right) else {
        return Ok(());
    };
    collect_between_nodes(tree, left, right, tracker, visit)?;
    collect_within_tree(tree, left, tracker, visit)?;
    collect_within_tree(tree, right, tracker, visit)
}

fn collect_between_nodes(
    tree: &MonotoneChainTree,
    first_index: usize,
    second_index: usize,
    tracker: &mut ExecutionWorkTracker<'_>,
    visit: &mut impl FnMut(CandidatePair) -> Result<()>,
) -> Result<()> {
    let first = tree.nodes[first_index];
    let second = tree.nodes[second_index];
    if !first.bounds.overlaps(second.bounds) {
        tracker.candidate(false)?;
        return Ok(());
    }
    if first.left.is_none() && second.left.is_none() {
        tracker.candidate(true)?;
        visit(CandidatePair {
            first: first.start.min(second.start),
            second: first.start.max(second.start),
        })?;
        return Ok(());
    }

    let first_len = first.end - first.start;
    let second_len = second.end - second.start;
    if first.left.is_some() && (second.right.is_none() || first_len >= second_len) {
        let (Some(left), Some(right)) = (first.left, first.right) else {
            unreachable!("internal monotone-chain node has two children")
        };
        collect_between_nodes(tree, left, second_index, tracker, visit)?;
        collect_between_nodes(tree, right, second_index, tracker, visit)?;
    } else {
        let (Some(left), Some(right)) = (second.left, second.right) else {
            unreachable!("internal monotone-chain node has two children")
        };
        collect_between_nodes(tree, first_index, left, tracker, visit)?;
        collect_between_nodes(tree, first_index, right, tracker, visit)?;
    }
    Ok(())
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
    use crate::{CancellationToken, ExecutionPolicy};

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
    fn candidate_visitor_matches_materialized_output_and_checks_cancellation() {
        let (lines, chains) =
            original_chains(&[&[(0.0, 0.0), (2.0, 0.0)], &[(1.0, -1.0), (1.0, 1.0)]]);
        let mut tracker = ExecutionWorkTracker::new(None, None);
        let mut visited = Vec::new();
        visit_source_chain_candidates(&lines, &chains, &mut tracker, |candidate| {
            visited.push((candidate.first, candidate.second));
            Ok(())
        })
        .unwrap();
        visited.sort_unstable();

        assert_eq!(
            visited,
            enumerate_source_chain_candidates(&lines, &chains).unwrap()
        );

        let token = CancellationToken::new();
        token.cancel();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };
        let mut tracker = ExecutionWorkTracker::new(Some(&policy), None);
        assert!(matches!(
            visit_source_chain_candidates(&lines, &chains, &mut tracker, |_| Ok(())),
            Err(PolygonizeError::Cancelled { stage }) if stage == "candidate_enumeration"
        ));
    }

    #[test]
    fn hybrid_visitor_covers_non_original_pairs_without_duplicates() {
        let lines = [
            line((0.0, 0.0), (2.0, 0.0)),
            line((1.0, -1.0), (1.0, 1.0)),
            line((0.0, 0.0), (2.0, 0.0)),
            line((1.0, -1.0), (1.0, 1.0)),
        ];
        let chains = [
            SourceLineString {
                segment_start: 0,
                segment_count: 1,
                source_id: Some(1),
                kind: SourceChainKind::Original,
            },
            SourceLineString {
                segment_start: 1,
                segment_count: 1,
                source_id: Some(2),
                kind: SourceChainKind::Original,
            },
            SourceLineString {
                segment_start: 2,
                segment_count: 1,
                source_id: Some(3),
                kind: SourceChainKind::Synthetic,
            },
            SourceLineString {
                segment_start: 3,
                segment_count: 1,
                source_id: None,
                kind: SourceChainKind::Unavailable,
            },
        ];
        let coverage = source_chain_coverage(lines.len(), &chains).unwrap();
        assert_eq!(
            coverage.segment_identities[1],
            Some(SourceSegmentIdentity {
                source_id: Some(2),
                chain_index: 1,
                segment_index: 0,
                chain_segment_count: 1,
                kind: SourceChainKind::Original,
            })
        );
        assert_eq!(
            coverage.segment_identities[3],
            Some(SourceSegmentIdentity {
                source_id: None,
                chain_index: 3,
                segment_index: 0,
                chain_segment_count: 1,
                kind: SourceChainKind::Unavailable,
            })
        );
        let mut tracker = ExecutionWorkTracker::new(None, None);
        let mut actual = Vec::new();
        visit_hybrid_source_chain_candidates(&lines, &chains, &mut tracker, |candidate| {
            actual.push((candidate.first, candidate.second));
            Ok(())
        })
        .unwrap();
        actual.sort_unstable();

        let mut expected = Vec::new();
        let bounds: Vec<_> = lines.iter().copied().map(Bounds::from_line).collect();
        for first in 0..lines.len() {
            for second in first + 1..lines.len() {
                if bounds[first].overlaps(bounds[second]) {
                    expected.push((first, second));
                }
            }
        }
        expected.sort_unstable();

        assert_eq!(actual, expected);
        assert_eq!(
            actual.windows(2).filter(|pair| pair[0] == pair[1]).count(),
            0
        );
    }

    #[test]
    fn hybrid_exact_visitor_uses_shared_intersection_evaluation() {
        let lines = [line((0.0, 0.0), (2.0, 2.0)), line((0.0, 2.0), (2.0, 0.0))];
        let chains = [
            SourceLineString {
                segment_start: 0,
                segment_count: 1,
                source_id: Some(1),
                kind: SourceChainKind::Original,
            },
            SourceLineString {
                segment_start: 1,
                segment_count: 1,
                source_id: Some(2),
                kind: SourceChainKind::Original,
            },
        ];
        let mut tracker = ExecutionWorkTracker::new(None, None);
        let mut exact = Vec::new();
        visit_hybrid_exact_candidates(&lines, &chains, &mut tracker, |candidate| {
            exact.push(candidate);
            Ok(())
        })
        .unwrap();

        assert_eq!(exact.len(), 1);
        assert!(exact[0].intersection.is_some());
        assert_eq!(
            exact[0].pair,
            CandidatePair {
                first: 0,
                second: 1
            }
        );
    }

    #[test]
    fn hybrid_experiment_uses_shared_split_and_validation_path() {
        let lines = [
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(2.0, 0.0, 10.0), 1),
            Line3D::new(
                Coord3D::new(0.0, -1.0, 20.0),
                Coord3D::new(2.0, 1.0, 40.0),
                2,
            ),
            Line3D::new(
                Coord3D::new(0.0, 1.0, 60.0),
                Coord3D::new(2.0, -1.0, 80.0),
                3,
            ),
        ];
        let chains = [
            SourceLineString {
                segment_start: 0,
                segment_count: 1,
                source_id: Some(1),
                kind: SourceChainKind::Original,
            },
            SourceLineString {
                segment_start: 1,
                segment_count: 1,
                source_id: Some(2),
                kind: SourceChainKind::Synthetic,
            },
            SourceLineString {
                segment_start: 2,
                segment_count: 1,
                source_id: None,
                kind: SourceChainKind::Unavailable,
            },
        ];
        let snap_noder = SnapNoder::new(0.0);
        let expected = snap_noder.node(lines.to_vec());

        let (actual, stats) = node_hybrid_source_chains(
            lines.to_vec(),
            &chains,
            &snap_noder,
            &ExecutionPolicy::default(),
        )
        .unwrap();

        assert_eq!(actual, expected);
        assert_eq!(stats.candidate_pairs, 3);
        assert_eq!(stats.exact_intersection_calls, 3);
        assert_eq!(stats.split_events, 3);
        assert!(actual.iter().any(|segment| segment.start.z == 5.0));
        assert!(actual.iter().any(|segment| segment.start.z == 30.0));
        assert!(actual.iter().any(|segment| segment.start.z == 70.0));
    }

    #[test]
    fn hybrid_experiment_rejects_incomplete_source_identity() {
        let lines = [line((0.0, 0.0), (1.0, 0.0)), line((0.0, 1.0), (1.0, 1.0))];
        let chains = [SourceLineString {
            segment_start: 0,
            segment_count: 1,
            source_id: Some(1),
            kind: SourceChainKind::Original,
        }];

        assert!(matches!(
            node_hybrid_source_chains(
                lines.to_vec(),
                &chains,
                &SnapNoder::new(0.0),
                &ExecutionPolicy::default(),
            ),
            Err(PolygonizeError::InvalidGeometry { reason })
                if reason.contains("complete source-chain coverage")
        ));
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
