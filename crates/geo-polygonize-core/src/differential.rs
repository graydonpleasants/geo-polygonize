//! Internal differential-test minimization helpers.

use crate::fingerprint::{coordinate_fingerprint, float_bits};
use crate::{
    CoordinateFingerprintV1, FingerprintDiffV1, Line3D, PolygonizerOptions, TopologyFingerprintV1,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// The standalone differential reproduction bundle schema version.
pub const REPRO_BUNDLE_V1_SCHEMA_VERSION: u32 = 1;

/// Exact, portable input segment for a differential reproduction bundle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReproLineV1 {
    pub start: CoordinateFingerprintV1,
    pub end: CoordinateFingerprintV1,
    pub line_id: String,
}

/// Reference topology metrics recorded with a differential mismatch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReferenceMetricsV1 {
    pub polygon_count: usize,
    pub dangle_count: usize,
    pub cut_edge_count: usize,
    pub invalid_ring_count: usize,
    pub total_area: String,
}

impl ReferenceMetricsV1 {
    pub fn new(
        polygon_count: usize,
        dangle_count: usize,
        cut_edge_count: usize,
        invalid_ring_count: usize,
        total_area: f64,
    ) -> crate::Result<Self> {
        Ok(Self {
            polygon_count,
            dangle_count,
            cut_edge_count,
            invalid_ring_count,
            total_area: float_bits(total_area)?,
        })
    }
}

/// Versioned, standalone evidence needed to reproduce a differential mismatch.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReproBundleV1 {
    pub schema_version: u32,
    pub input: Vec<ReproLineV1>,
    pub options: serde_json::Value,
    pub versions: BTreeMap<String, String>,
    pub fingerprint: TopologyFingerprintV1,
    pub reference_metrics: ReferenceMetricsV1,
    pub witness: FingerprintDiffV1,
}

impl ReproBundleV1 {
    pub fn new(
        input: &[Line3D],
        options: &PolygonizerOptions,
        mut versions: BTreeMap<String, String>,
        fingerprint: TopologyFingerprintV1,
        reference_metrics: ReferenceMetricsV1,
        witness: FingerprintDiffV1,
    ) -> crate::Result<Self> {
        versions
            .entry("geo-polygonize-core".to_string())
            .or_insert_with(|| env!("CARGO_PKG_VERSION").to_string());
        Ok(Self {
            schema_version: REPRO_BUNDLE_V1_SCHEMA_VERSION,
            input: input
                .iter()
                .map(|line| {
                    Ok(ReproLineV1 {
                        start: coordinate_fingerprint(line.start)?,
                        end: coordinate_fingerprint(line.end)?,
                        line_id: format!("0x{:08x}", line.line_id),
                    })
                })
                .collect::<crate::Result<_>>()?,
            options: serde_json::to_value(options).expect("validated options serialize"),
            versions,
            fingerprint,
            reference_metrics,
            witness,
        })
    }

    pub fn to_pretty_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

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

/// Simplify shared X/Y values while the caller's exact mismatch predicate holds.
///
/// Every occurrence of a coordinate value is replaced together so connected
/// endpoints stay connected. Source IDs and Z values are never changed.
#[doc(hidden)]
pub fn minimize_xy_coordinates<F>(lines: Vec<Line3D>, mut reproduces: F) -> Option<Vec<Line3D>>
where
    F: FnMut(&[Line3D]) -> bool,
{
    if !reproduces(&lines) {
        return None;
    }

    let mut current = lines;
    for axis in [Axis::X, Axis::Y] {
        let mut values = Vec::new();
        for line in &current {
            values.push(axis.get(line.start).to_bits());
            values.push(axis.get(line.end).to_bits());
        }
        values.sort_unstable();
        values.dedup();

        for bits in values {
            let value = f64::from_bits(bits);
            for replacement in [0.0, value.signum(), value.trunc()] {
                if !replacement.is_finite() || replacement.to_bits() == bits {
                    continue;
                }
                let mut candidate = current.clone();
                for line in &mut candidate {
                    axis.replace(&mut line.start, bits, replacement);
                    axis.replace(&mut line.end, bits, replacement);
                }
                if reproduces(&candidate) {
                    current = candidate;
                    break;
                }
            }
        }
    }
    Some(current)
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn get(self, coord: crate::Coord3D) -> f64 {
        match self {
            Self::X => coord.x,
            Self::Y => coord.y,
        }
    }

    fn replace(self, coord: &mut crate::Coord3D, expected: u64, replacement: f64) {
        let value = match self {
            Self::X => &mut coord.x,
            Self::Y => &mut coord.y,
        };
        if value.to_bits() == expected {
            *value = replacement;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{polygonize, Coord3D};
    use serde_json::json;

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

    #[test]
    fn simplifies_shared_xy_without_changing_ids_or_z_conflicts() {
        let lines = vec![
            Line3D::new(
                Coord3D::new(123.5, 456.75, 10.0),
                Coord3D::new(789.25, 456.75, 20.0),
                7,
            ),
            Line3D::new(
                Coord3D::new(789.25, 456.75, 30.0),
                Coord3D::new(789.25, 999.5, 40.0),
                9,
            ),
        ];
        let original_z: Vec<_> = lines
            .iter()
            .flat_map(|line| [line.start.z, line.end.z])
            .collect();

        let minimized = minimize_xy_coordinates(lines, |candidate| {
            candidate.len() == 2
                && candidate[0].line_id == 7
                && candidate[1].line_id == 9
                && candidate[0].end.x == candidate[1].start.x
                && candidate[0].end.y == candidate[1].start.y
                && candidate
                    .iter()
                    .all(|line| line.start.to_coord_2d() != line.end.to_coord_2d())
                && candidate[0].end.z != candidate[1].start.z
        })
        .unwrap();

        assert_eq!(minimized[0].start.to_coord_2d(), (0.0, 0.0).into());
        assert_eq!(minimized[0].end.to_coord_2d(), (1.0, 0.0).into());
        assert_eq!(minimized[1].start.to_coord_2d(), (1.0, 0.0).into());
        assert_eq!(minimized[1].end.to_coord_2d(), (1.0, 1.0).into());
        assert_eq!(
            minimized
                .iter()
                .flat_map(|line| [line.start.z, line.end.z])
                .collect::<Vec<_>>(),
            original_z
        );
    }

    #[test]
    fn exports_an_exact_standalone_repro_bundle() {
        let lines = vec![
            Line3D::new(
                Coord3D::new(-0.0, 0.0, 10.0),
                Coord3D::new(1.0, 0.0, 20.0),
                7,
            ),
            Line3D::new(
                Coord3D::new(1.0, 0.0, 30.0),
                Coord3D::new(0.0, 1.0, 40.0),
                9,
            ),
        ];
        let options = PolygonizerOptions::default();
        let result = polygonize(lines.iter().copied(), &options).unwrap();
        let fingerprint = TopologyFingerprintV1::try_from_result(&result, &options).unwrap();
        let witness = FingerprintDiffV1 {
            path: "$.polygons".to_string(),
            expected: json!(1),
            actual: json!(0),
        };
        let bundle = ReproBundleV1::new(
            &lines,
            &options,
            BTreeMap::from([("reference".to_string(), "GEOS 3.13.1".to_string())]),
            fingerprint,
            ReferenceMetricsV1::new(1, 0, 0, 0, 0.5).unwrap(),
            witness,
        )
        .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&bundle.to_pretty_json().unwrap()).unwrap();

        assert_eq!(json["schema_version"], 1);
        assert_eq!(
            json["versions"]["geo-polygonize-core"],
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(json["versions"]["reference"], "GEOS 3.13.1");
        assert_eq!(json["input"][0]["start"]["x"], "0x0000000000000000");
        assert_eq!(json["input"][0]["line_id"], "0x00000007");
        assert_eq!(
            json["reference_metrics"]["total_area"],
            "0x3fe0000000000000"
        );
        assert_eq!(json["witness"]["path"], "$.polygons");
        assert!(json["options"].is_object());
        assert!(json["fingerprint"].is_object());
    }
}
