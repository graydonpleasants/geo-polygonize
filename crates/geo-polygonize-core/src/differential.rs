//! Internal differential-test minimization helpers.

use crate::Line3D;

/// Delta-debug a line set while the caller's exact mismatch predicate holds.
///
/// Retained segments are copied byte-for-byte, including source IDs and Z.
/// The caller captures the selected options and recomputes both sides of the
/// differential comparison for every candidate.
#[doc(hidden)]
pub fn minimize_line_set<F>(lines: Vec<Line3D>, mut reproduces: F) -> Option<Vec<Line3D>>
where
    F: FnMut(&[Line3D]) -> bool,
{
    if !reproduces(&lines) {
        return None;
    }
    if reproduces(&[]) {
        return Some(Vec::new());
    }

    let mut current = lines;
    let mut partitions = 2;
    while current.len() >= 2 {
        let chunk_size = current.len().div_ceil(partitions);
        let mut reduced = false;
        for start in (0..current.len()).step_by(chunk_size) {
            let end = (start + chunk_size).min(current.len());
            let mut candidate = Vec::with_capacity(current.len() - (end - start));
            candidate.extend_from_slice(&current[..start]);
            candidate.extend_from_slice(&current[end..]);
            if reproduces(&candidate) {
                current = candidate;
                partitions = partitions.saturating_sub(1).max(2);
                reduced = true;
                break;
            }
        }
        if !reduced {
            if partitions >= current.len() {
                break;
            }
            partitions = partitions.saturating_mul(2).min(current.len());
        }
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coord3D, PolygonizerOptions};

    fn line(id: u32, z: f64) -> Line3D {
        Line3D::new(
            Coord3D::new(id as f64, 0.0, z),
            Coord3D::new(id as f64, 1.0, z + 0.5),
            id,
        )
    }

    #[test]
    fn minimizes_to_the_required_segments_without_rewriting_identity_or_z() {
        let lines: Vec<_> = (1..=8).map(|id| line(id, id as f64 * 10.0)).collect();
        let expected = [lines[2], lines[6]];
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };

        let minimized = minimize_line_set(lines, |candidate| {
            options.node_input
                && expected.iter().all(|required| {
                    candidate
                        .iter()
                        .any(|line| line.line_id == required.line_id)
                })
        })
        .unwrap();

        assert_eq!(minimized.len(), 2);
        for (actual, expected) in minimized.iter().zip(expected) {
            assert_eq!(actual.line_id, expected.line_id);
            assert_eq!(actual.start, expected.start);
            assert_eq!(actual.end, expected.end);
        }
    }

    #[test]
    fn rejects_a_non_reproducing_input_and_can_reduce_options_only_failures_to_empty() {
        let lines = vec![line(1, 3.0)];
        assert!(minimize_line_set(lines.clone(), |_| false).is_none());
        assert_eq!(
            minimize_line_set(lines, |_| true).unwrap(),
            Vec::<Line3D>::new()
        );
    }
}
