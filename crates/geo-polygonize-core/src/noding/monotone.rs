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

    let (mut noded, split_events) = if events.is_empty() {
        (lines, 0)
    } else {
        snap_noder.apply_split_events_for_research(&lines, events, execution_policy)?
    };
    noded.retain(|line| line.start.to_coord_2d() != line.end.to_coord_2d());
    snap_noder.normalize_and_dedup(&mut noded);
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
    use crate::{
        normalize_polygonize_error, polygonize, CancellationToken, DiagnosticsOptions,
        ExecutionPolicy, PolygonizerOptions, ProvenanceOptions, TopologyFingerprintV1,
    };
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use serde::Deserialize;

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

    #[derive(Deserialize)]
    struct Fixture {
        options: Option<PolygonizerOptions>,
        profile_id: Option<String>,
        inputs: Vec<FixtureLine>,
    }

    #[derive(Deserialize)]
    struct FixtureLine {
        start: FixtureCoordinate,
        end: FixtureCoordinate,
        id: u32,
    }

    #[derive(Deserialize)]
    struct FixtureCoordinate {
        x: f64,
        y: f64,
        z: f64,
    }

    fn assert_hybrid_matches_fixture_fingerprint(
        source: &str,
        expected_polygon_count: usize,
        expected_dangle_count: usize,
    ) {
        let fixture: Fixture = serde_json::from_str(source).unwrap();
        let mut options = fixture.options.unwrap_or_default();
        options.input_profile_id = fixture.profile_id;
        options.diagnostics = DiagnosticsOptions {
            enabled: true,
            ..Default::default()
        };
        options.provenance = ProvenanceOptions {
            enabled: true,
            include_boundary_line_ids: true,
        };
        let lines: Vec<_> = fixture
            .inputs
            .into_iter()
            .map(|input| {
                Line3D::new(
                    Coord3D::new(input.start.x, input.start.y, input.start.z),
                    Coord3D::new(input.end.x, input.end.y, input.end.z),
                    input.id,
                )
            })
            .collect();
        let chains: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(segment_start, line)| SourceLineString {
                segment_start,
                segment_count: 1,
                source_id: Some(line.line_id),
                kind: SourceChainKind::Original,
            })
            .collect();
        let snap_noder = SnapNoder::new(0.0);
        let expected = snap_noder.node(lines.clone());
        let (actual, _) =
            node_hybrid_source_chains(lines, &chains, &snap_noder, &ExecutionPolicy::default())
                .unwrap();
        let expected_result = polygonize(expected, &options).unwrap();
        let actual_result = polygonize(actual, &options).unwrap();
        let expected_fingerprint =
            TopologyFingerprintV1::try_from_result(&expected_result, &options).unwrap();
        let actual_fingerprint =
            TopologyFingerprintV1::try_from_result(&actual_result, &options).unwrap();

        assert_eq!(actual_result.polygons.len(), expected_polygon_count);
        assert_eq!(actual_result.dangles.len(), expected_dangle_count);
        assert!(actual_result.cut_edges.is_empty());
        assert!(actual_result.invalid_rings.is_empty());
        assert_eq!(actual_fingerprint, expected_fingerprint);
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

    fn assert_hybrid_matches_snap(
        lines: Vec<Line3D>,
        source_chains: &[SourceLineString],
    ) -> NodingWorkStats {
        let snap_noder = SnapNoder::new(0.0);
        let expected = snap_noder.node(lines.clone());
        let (actual, stats) = node_hybrid_source_chains(
            lines,
            source_chains,
            &snap_noder,
            &ExecutionPolicy::default(),
        )
        .unwrap();
        assert_eq!(actual, expected);
        stats
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
    fn hybrid_experiment_preserves_source_ids_z_and_shared_path() {
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
        let mut source_line_ids: Vec<_> = actual.iter().map(|segment| segment.line_id).collect();
        source_line_ids.sort_unstable();
        assert_eq!(source_line_ids, vec![1, 1, 2, 2, 3, 3]);
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
    fn hybrid_experiment_is_deterministic_under_input_permutation() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(4.0, 4.0, 0.0), 10),
            Line3D::new(Coord3D::new(0.0, 4.0, 0.0), Coord3D::new(4.0, 0.0, 0.0), 11),
            Line3D::new(
                Coord3D::new(2.0, -1.0, 0.0),
                Coord3D::new(2.0, 5.0, 0.0),
                12,
            ),
            Line3D::new(
                Coord3D::new(-1.0, 2.0, 0.0),
                Coord3D::new(5.0, 2.0, 0.0),
                13,
            ),
        ];
        let chains: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(segment_start, line)| SourceLineString {
                segment_start,
                segment_count: 1,
                source_id: Some(line.line_id),
                kind: SourceChainKind::Original,
            })
            .collect();
        let permutation = [2, 0, 3, 1];
        let permuted_lines: Vec<_> = permutation.iter().map(|&index| lines[index]).collect();
        let permuted_chains: Vec<_> = permuted_lines
            .iter()
            .enumerate()
            .map(|(segment_start, line)| SourceLineString {
                segment_start,
                segment_count: 1,
                source_id: Some(line.line_id),
                kind: SourceChainKind::Original,
            })
            .collect();
        let snap_noder = SnapNoder::new(0.0);

        let (expected, _) = node_hybrid_source_chains(
            lines.clone(),
            &chains,
            &snap_noder,
            &ExecutionPolicy::default(),
        )
        .unwrap();
        let (actual, _) = node_hybrid_source_chains(
            permuted_lines.clone(),
            &permuted_chains,
            &snap_noder,
            &ExecutionPolicy::default(),
        )
        .unwrap();

        assert_eq!(expected, snap_noder.node(lines));
        assert_eq!(actual, snap_noder.node(permuted_lines));
        assert_eq!(actual, expected);
    }

    #[test]
    fn hybrid_experiment_normalizes_boundary_errors() {
        let lines = vec![line((0.0, 0.0), (1.0, 0.0))];
        let invalid_range = node_hybrid_source_chains(
            lines.clone(),
            &[SourceLineString {
                segment_start: 1,
                segment_count: 1,
                source_id: Some(1),
                kind: SourceChainKind::Original,
            }],
            &SnapNoder::new(0.0),
            &ExecutionPolicy::default(),
        )
        .unwrap_err();
        let incomplete_identity = node_hybrid_source_chains(
            lines,
            &[],
            &SnapNoder::new(0.0),
            &ExecutionPolicy::default(),
        )
        .unwrap_err();

        let invalid_range = normalize_polygonize_error(&invalid_range);
        let incomplete_identity = normalize_polygonize_error(&incomplete_identity);
        assert_eq!(invalid_range, incomplete_identity);
        assert_eq!(invalid_range.family, "invalid_geometry");
        assert_eq!(invalid_range.code, "invalid_geometry");
        assert_eq!(invalid_range.stage, "input_validation");
    }

    #[test]
    fn hybrid_experiment_matches_differential_conformance_corpus() {
        let (self_intersecting, self_intersecting_chains) =
            original_chains(&[&[(0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)]]);
        let self_stats = assert_hybrid_matches_snap(self_intersecting, &self_intersecting_chains);
        assert!(self_stats.split_events > 0);

        let (road_and_contours, road_and_contour_chains) = original_chains(&[
            &[(0.0, 0.0), (3.0, 0.0), (6.0, 0.0)],
            &[(1.0, -1.0), (1.0, 1.0), (1.0, 3.0)],
            &[(0.0, 10.0), (2.0, 11.0), (4.0, 10.0), (6.0, 11.0)],
            &[(0.0, 12.0), (2.0, 13.0), (4.0, 12.0), (6.0, 13.0)],
        ]);
        let road_stats = assert_hybrid_matches_snap(road_and_contours, &road_and_contour_chains);
        assert!(road_stats.candidate_pairs >= road_stats.exact_intersection_calls);

        let duplicate_and_reversed = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(2.0, 0.0, 0.0), 10),
            Line3D::new(Coord3D::new(2.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 11),
            Line3D::new(
                Coord3D::new(1.0, -1.0, 0.0),
                Coord3D::new(1.0, 1.0, 0.0),
                12,
            ),
        ];
        let duplicate_and_reversed_chains = [
            SourceLineString {
                segment_start: 0,
                segment_count: 2,
                source_id: Some(10),
                kind: SourceChainKind::Original,
            },
            SourceLineString {
                segment_start: 2,
                segment_count: 1,
                source_id: Some(12),
                kind: SourceChainKind::Synthetic,
            },
        ];
        let duplicate_stats =
            assert_hybrid_matches_snap(duplicate_and_reversed, &duplicate_and_reversed_chains);
        assert!(duplicate_stats.exact_intersection_calls > 0);
    }

    #[test]
    fn hybrid_experiment_matches_canonical_feature_build_fingerprint() {
        let (lines, chains) = original_chains(&[
            &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)],
            &[(0.0, 0.0), (4.0, 4.0)],
            &[(0.0, 4.0), (4.0, 0.0)],
        ]);
        let snap_noder = SnapNoder::new(0.0);
        let expected = snap_noder.node(lines.clone());
        let (actual, _) =
            node_hybrid_source_chains(lines, &chains, &snap_noder, &ExecutionPolicy::default())
                .unwrap();
        let options = PolygonizerOptions::default();
        let expected_result = polygonize(expected, &options).unwrap();
        let actual_result = polygonize(actual, &options).unwrap();
        let expected_fingerprint =
            TopologyFingerprintV1::try_from_result(&expected_result, &options).unwrap();
        let actual_fingerprint =
            TopologyFingerprintV1::try_from_result(&actual_result, &options).unwrap();

        assert_eq!(actual_fingerprint, expected_fingerprint);
    }

    #[test]
    fn hybrid_experiment_matches_square_with_hole_fixture_fingerprint() {
        assert_hybrid_matches_fixture_fingerprint(
            include_str!("../../tests/fixtures/basic/square_with_hole.json"),
            2,
            0,
        );
    }

    #[test]
    fn hybrid_experiment_matches_bowtie_fixture_fingerprint() {
        assert_hybrid_matches_fixture_fingerprint(
            include_str!("../../tests/fixtures/dirty/bowtie.json"),
            2,
            0,
        );
    }

    #[test]
    fn hybrid_experiment_matches_floating_microfaces_fixture_fingerprint() {
        assert_hybrid_matches_fixture_fingerprint(
            include_str!("../../tests/fixtures/compat/floating_microfaces.json"),
            2,
            4,
        );
    }

    #[test]
    fn hybrid_experiment_matches_zero_length_fixture_fingerprint() {
        assert_hybrid_matches_fixture_fingerprint(
            include_str!("../../tests/fixtures/compat/zero_length_segment.json"),
            0,
            0,
        );
    }

    #[test]
    fn hybrid_experiment_matches_provenance_fixture_fingerprint() {
        assert_hybrid_matches_fixture_fingerprint(
            include_str!("../../tests/fixtures/provenance/mixed_boundary_with_profile.json"),
            2,
            0,
        );
    }

    #[test]
    fn hybrid_experiment_matches_z_ignore_fixture_fingerprint() {
        assert_hybrid_matches_fixture_fingerprint(
            include_str!("../../tests/fixtures/z/ignore_conflicts.json"),
            1,
            0,
        );
    }

    #[test]
    fn hybrid_experiment_matches_overlap_and_nested_ring_cases() {
        let (overlapping, overlapping_chains) = original_chains(&[
            &[(0.0, 0.0), (4.0, 0.0)],
            &[(1.0, 0.0), (3.0, 0.0)],
            &[(2.0, -1.0), (2.0, 1.0)],
        ]);
        let overlap_stats = assert_hybrid_matches_snap(overlapping, &overlapping_chains);
        assert!(overlap_stats.split_events > 0);

        let (nested_rings, nested_ring_chains) = original_chains(&[
            &[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ],
            &[(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0), (2.0, 2.0)],
        ]);
        let nested_stats = assert_hybrid_matches_snap(nested_rings, &nested_ring_chains);
        assert_eq!(nested_stats.split_events, 0);
    }

    #[test]
    fn hybrid_experiment_preserves_bounded_error_contracts() {
        let (lines, chains) = original_chains(&[
            &[(0.0, 0.0), (2.0, 0.0)],
            &[(1.0, -1.0), (1.0, 1.0)],
            &[(0.0, -1.0), (2.0, 1.0)],
        ]);
        let policy = ExecutionPolicy {
            max_candidate_pairs: Some(1),
            ..Default::default()
        };

        assert!(matches!(
            node_hybrid_source_chains(
                lines,
                &chains,
                &SnapNoder::new(0.0),
                &policy,
            ),
            Err(PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 1,
                ..
            }) if stage == "candidate_pairs"
        ));
    }

    #[test]
    fn hybrid_experiment_normalizes_operational_errors() {
        let (lines, chains) = original_chains(&[
            &[(0.0, 0.0), (2.0, 0.0)],
            &[(1.0, -1.0), (1.0, 1.0)],
            &[(0.0, -1.0), (2.0, 1.0)],
        ]);
        let limit_error = node_hybrid_source_chains(
            lines.clone(),
            &chains,
            &SnapNoder::new(0.0),
            &ExecutionPolicy {
                max_candidate_pairs: Some(1),
                ..Default::default()
            },
        )
        .unwrap_err();
        let limit = normalize_polygonize_error(&limit_error);
        assert_eq!(limit.family, "resource_limit");
        assert_eq!(limit.code, "resource_limit_exceeded");
        assert_eq!(limit.stage, "candidate_pairs");
        assert_eq!(limit.limit.as_deref(), Some("1"));
        assert!(limit.observed.is_some());

        let token = CancellationToken::new();
        token.cancel();
        let cancelled_error = node_hybrid_source_chains(
            lines,
            &chains,
            &SnapNoder::new(0.0),
            &ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            },
        )
        .unwrap_err();
        let cancelled = normalize_polygonize_error(&cancelled_error);
        assert_eq!(cancelled.family, "cancelled");
        assert_eq!(cancelled.code, "cancelled");
        assert_eq!(cancelled.stage, "candidate_enumeration");
    }

    #[test]
    fn hybrid_experiment_matches_bounded_differential_fuzz_corpus() {
        let mut rng = StdRng::seed_from_u64(0x1287_2026);
        for case_index in 0..12 {
            let segment_count = 24 + case_index % 4 * 8;
            let mut lines = Vec::with_capacity(segment_count);
            let mut chains = Vec::with_capacity(segment_count);
            for segment_index in 0..segment_count {
                let start = (rng.gen_range(-100.0..100.0), rng.gen_range(-100.0..100.0));
                let mut end = (rng.gen_range(-100.0..100.0), rng.gen_range(-100.0..100.0));
                if start == end {
                    end.0 += 1.0;
                }
                lines.push(Line3D::new(
                    Coord3D::new(start.0, start.1, 0.0),
                    Coord3D::new(end.0, end.1, 0.0),
                    segment_index as u32,
                ));
                chains.push(SourceLineString {
                    segment_start: segment_index,
                    segment_count: 1,
                    source_id: Some(segment_index as u32),
                    kind: SourceChainKind::Original,
                });
            }

            let stats = assert_hybrid_matches_snap(lines, &chains);
            assert!(stats.exact_intersection_calls <= stats.candidate_pairs);
        }
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
