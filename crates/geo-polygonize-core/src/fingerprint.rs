//! Versioned, exact conformance values for adapter test harnesses.
//!
//! This is intentionally `#[doc(hidden)]`: it is a shared contract for the
//! repository's adapters, not an additional stable polygonization entrypoint.

use crate::utils::canonical_coordinate_bits;
use crate::{
    Coord3D, NodingValidationKind, PolygonizeError, PolygonizerOptions, PolygonizerResult,
};
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// The current topology fingerprint schema version.
pub const TOPOLOGY_FINGERPRINT_V1_SCHEMA_VERSION: u32 = 1;

/// A versioned, structured canonical representation of a successful run.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct TopologyFingerprintV1 {
    pub schema_version: u32,
    /// The serialized canonical options object is part of the semantic contract.
    #[ts(type = "unknown")]
    pub options: Value,
    pub polygons: Vec<PolygonFingerprintV1>,
    pub dangles: Vec<Vec<CoordinateFingerprintV1>>,
    pub cut_edges: Vec<Vec<CoordinateFingerprintV1>>,
    pub invalid_rings: Vec<Vec<CoordinateFingerprintV1>>,
    pub diagnostics: Option<TopologyDiagnosticsFingerprintV1>,
}

/// An exact finite coordinate. Every component is an IEEE-754 bit pattern.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, TS)]
#[ts(export)]
pub struct CoordinateFingerprintV1 {
    pub x: String,
    pub y: String,
    pub z: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct PolygonFingerprintV1 {
    pub exterior: Vec<CoordinateFingerprintV1>,
    pub interiors: Vec<RingFingerprintV1>,
    /// Per-edge representative input IDs, encoded as fixed-width hex strings.
    pub exterior_edge_ids: Vec<String>,
    pub provenance: Option<ProvenanceFingerprintV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct RingFingerprintV1 {
    pub coordinates: Vec<CoordinateFingerprintV1>,
    pub edge_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ProvenanceFingerprintV1 {
    /// Complete source sets, never JSON numbers, so JavaScript cannot lose IDs.
    pub boundary_line_ids: Vec<String>,
    pub input_profile_id: Option<String>,
}

/// Only topology-level diagnostics. Timings and work counters are deliberately excluded.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct TopologyDiagnosticsFingerprintV1 {
    pub dangle_count: usize,
    pub cut_edge_count: usize,
    pub ring_count: usize,
    pub shell_count: usize,
    pub hole_count: usize,
    pub unassigned_hole_count: usize,
    pub unassigned_hole_area: String,
    pub invalid_ring_count: usize,
    pub z_conflict_node_count: usize,
    pub z_conflicting_line_ids: Vec<String>,
}

/// A structured, stable error equality contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct NormalizedPolygonizeErrorV1 {
    pub schema_version: u32,
    pub family: String,
    pub code: String,
    pub stage: String,
    pub field: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub limit: Option<String>,
    pub observed: Option<String>,
    pub witness: Option<ErrorWitnessV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ErrorWitnessV1 {
    pub ids: Vec<String>,
    pub coordinate: Option<CoordinateFingerprintV1>,
}

/// A first, field-level difference suitable for terse CI failures.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct FingerprintDiffV1 {
    pub path: String,
    #[ts(type = "unknown")]
    pub expected: Value,
    #[ts(type = "unknown")]
    pub actual: Value,
}

impl TopologyFingerprintV1 {
    /// Builds a canonical, exact representation. Inputs must have finite coordinates.
    pub fn try_from_result(
        result: &PolygonizerResult,
        options: &PolygonizerOptions,
    ) -> crate::Result<Self> {
        let mut polygons: Vec<_> = result
            .polygons
            .iter()
            .map(|polygon| {
                let mut interiors: Vec<_> = polygon
                    .interiors
                    .iter()
                    .zip(&polygon.interiors_ids)
                    .map(|(coordinates, ids)| {
                        let (coordinates, edge_ids) = canonical_ring_with_ids(coordinates, ids)?;
                        Ok(RingFingerprintV1 {
                            coordinates,
                            edge_ids,
                        })
                    })
                    .collect::<crate::Result<_>>()?;
                interiors.sort_by_key(sort_key);
                let (exterior, exterior_edge_ids) =
                    canonical_ring_with_ids(&polygon.exterior, &polygon.exterior_ids)?;
                Ok(PolygonFingerprintV1 {
                    exterior,
                    interiors,
                    exterior_edge_ids,
                    provenance: polygon.provenance.as_ref().map(|provenance| {
                        let mut boundary_line_ids: Vec<_> = provenance
                            .boundary_line_ids
                            .iter()
                            .copied()
                            .map(id64)
                            .collect();
                        boundary_line_ids.sort();
                        boundary_line_ids.dedup();
                        ProvenanceFingerprintV1 {
                            boundary_line_ids,
                            input_profile_id: provenance.input_profile_id.clone(),
                        }
                    }),
                })
            })
            .collect::<crate::Result<_>>()?;
        polygons.sort_by_key(sort_key);

        Ok(Self {
            schema_version: TOPOLOGY_FINGERPRINT_V1_SCHEMA_VERSION,
            options: serde_json::to_value(options).expect("validated options serialize"),
            polygons,
            dangles: canonical_open_lines(&result.dangles)?,
            cut_edges: canonical_open_lines(&result.cut_edges)?,
            invalid_rings: canonical_rings(&result.invalid_rings)?,
            diagnostics: result.diagnostics.as_ref().map(|diagnostics| {
                let mut z_conflicting_line_ids: Vec<_> = diagnostics
                    .z_conflicts
                    .contributing_line_ids
                    .iter()
                    .copied()
                    .map(id32)
                    .collect();
                z_conflicting_line_ids.sort();
                z_conflicting_line_ids.dedup();
                TopologyDiagnosticsFingerprintV1 {
                    dangle_count: diagnostics.dangle_count,
                    cut_edge_count: diagnostics.cut_edge_count,
                    ring_count: diagnostics.ring_count,
                    shell_count: diagnostics.shell_count,
                    hole_count: diagnostics.hole_count,
                    unassigned_hole_count: diagnostics.unassigned_hole_count,
                    unassigned_hole_area: float_bits(diagnostics.unassigned_hole_area)
                        .expect("pipeline diagnostics are finite"),
                    invalid_ring_count: diagnostics.invalid_ring_count,
                    z_conflict_node_count: diagnostics.z_conflicts.conflict_node_count,
                    z_conflicting_line_ids,
                }
            }),
        })
    }

    /// Returns the first deterministic structural mismatch.
    pub fn diff(&self, actual: &Self) -> Option<FingerprintDiffV1> {
        diff_value(
            "$",
            &serde_json::to_value(self).expect("fingerprint serializes"),
            &serde_json::to_value(actual).expect("fingerprint serializes"),
        )
    }
}

/// Normalizes a failure without making user-facing message text part of equality.
pub fn normalize_polygonize_error(error: &PolygonizeError) -> NormalizedPolygonizeErrorV1 {
    let base = |family: &str, code: &str, stage: &str| NormalizedPolygonizeErrorV1 {
        schema_version: TOPOLOGY_FINGERPRINT_V1_SCHEMA_VERSION,
        family: family.to_string(),
        code: code.to_string(),
        stage: stage.to_string(),
        field: None,
        expected: None,
        actual: None,
        limit: None,
        observed: None,
        witness: None,
    };
    match error {
        PolygonizeError::InvalidArgumentType {
            field,
            expected,
            actual,
        } => NormalizedPolygonizeErrorV1 {
            field: Some(field.clone()),
            expected: Some(expected.clone()),
            actual: Some(actual.clone()),
            ..base("invalid_argument", "invalid_argument_type", "options")
        },
        PolygonizeError::InvalidGeometry { .. } => {
            base("invalid_geometry", "invalid_geometry", "input_validation")
        }
        PolygonizeError::NonFiniteCoordinate { .. } => base(
            "invalid_geometry",
            "non_finite_coordinate",
            "input_validation",
        ),
        PolygonizeError::InvalidBufferShape { .. } => base(
            "invalid_argument",
            "invalid_buffer_shape",
            "input_validation",
        ),
        PolygonizeError::ResourceLimitExceeded {
            stage,
            limit,
            observed,
        } => NormalizedPolygonizeErrorV1 {
            limit: Some(limit.to_string()),
            observed: Some(observed.to_string()),
            ..base("resource_limit", "resource_limit_exceeded", stage)
        },
        PolygonizeError::Cancelled { stage } => base("cancelled", "cancelled", stage),
        PolygonizeError::UnsupportedOptionCombination { .. } => base(
            "invalid_argument",
            "unsupported_option_combination",
            "options",
        ),
        PolygonizeError::ZConflict { x, y, line_ids } => {
            let mut ids: Vec<_> = line_ids.iter().copied().map(id32).collect();
            ids.sort();
            ids.dedup();
            NormalizedPolygonizeErrorV1 {
                witness: Some(ErrorWitnessV1 {
                    ids,
                    coordinate: Some(CoordinateFingerprintV1 {
                        x: float_bits(*x).expect("validated coordinate"),
                        y: float_bits(*y).expect("validated coordinate"),
                        z: float_bits(0.0).expect("zero is finite"),
                    }),
                }),
                ..base("topology", "z_conflict", "z_reconciliation")
            }
        }
        PolygonizeError::NodingValidationFailure {
            first_segment,
            second_segment,
            kind,
            ..
        } => NormalizedPolygonizeErrorV1 {
            witness: Some(ErrorWitnessV1 {
                ids: vec![id_usize(*first_segment), id_usize(*second_segment)],
                coordinate: None,
            }),
            ..base(
                "topology",
                match kind {
                    NodingValidationKind::ZeroLengthSegment => "zero_length_segment",
                    NodingValidationKind::CollinearOverlap => "collinear_overlap",
                    NodingValidationKind::InteriorIntersection => "interior_intersection",
                },
                "noding_validation",
            )
        },
        PolygonizeError::InternalInvariantViolation { .. } => {
            base("internal", "invariant_violation", "internal")
        }
        PolygonizeError::ArrowError { .. } => base("adapter", "arrow_error", "adapter"),
        PolygonizeError::Panic { .. } => base("internal", "panic", "boundary"),
    }
}

fn canonical_open_lines(
    lines: &[Vec<Coord3D>],
) -> crate::Result<Vec<Vec<CoordinateFingerprintV1>>> {
    let mut result: Vec<_> = lines
        .iter()
        .map(|line| {
            let forward = coordinates(line)?;
            let mut reverse = forward.clone();
            reverse.reverse();
            Ok(forward.min(reverse))
        })
        .collect::<crate::Result<_>>()?;
    result.sort_by_key(sort_key);
    Ok(result)
}

fn canonical_rings(rings: &[Vec<Coord3D>]) -> crate::Result<Vec<Vec<CoordinateFingerprintV1>>> {
    let mut result: Vec<_> = rings
        .iter()
        .map(|ring| canonical_ring(ring))
        .collect::<crate::Result<_>>()?;
    result.sort_by_key(sort_key);
    Ok(result)
}

fn canonical_ring(ring: &[Coord3D]) -> crate::Result<Vec<CoordinateFingerprintV1>> {
    let mut ring = coordinates(ring)?;
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.is_empty() {
        return Ok(ring);
    }
    let forward = rotate_minimum(&ring);
    let mut backwards = ring;
    backwards.reverse();
    let backwards = rotate_minimum(&backwards);
    let mut canonical = forward.min(backwards);
    canonical.push(canonical[0].clone());
    Ok(canonical)
}

fn canonical_ring_with_ids(
    ring: &[Coord3D],
    ids: &[u32],
) -> crate::Result<(Vec<CoordinateFingerprintV1>, Vec<String>)> {
    let mut coordinates = coordinates(ring)?;
    if coordinates.len() > 1 && coordinates.first() == coordinates.last() {
        coordinates.pop();
    }
    if coordinates.is_empty() {
        return Ok((coordinates, Vec::new()));
    }
    if coordinates.len() != ids.len() {
        return Err(PolygonizeError::InternalInvariantViolation {
            reason: format!(
                "fingerprint ring has {} edges but {} representative IDs",
                coordinates.len(),
                ids.len()
            ),
        });
    }

    let forward: Vec<_> = coordinates
        .iter()
        .cloned()
        .zip(ids.iter().copied().map(id32))
        .collect();
    let forward = rotate_minimum(&forward);

    coordinates.reverse();
    let mut reversed_ids: Vec<_> = ids.iter().rev().copied().map(id32).collect();
    reversed_ids.rotate_left(1);
    let backwards: Vec<_> = coordinates.into_iter().zip(reversed_ids).collect();
    let backwards = rotate_minimum(&backwards);

    let canonical = forward.min(backwards);
    let (mut coordinates, edge_ids): (Vec<_>, Vec<_>) = canonical.into_iter().unzip();
    coordinates.push(coordinates[0].clone());
    Ok((coordinates, edge_ids))
}

fn rotate_minimum<T: Clone + Ord>(ring: &[T]) -> Vec<T> {
    let start = minimum_rotation_index(ring);
    ring[start..]
        .iter()
        .chain(&ring[..start])
        .cloned()
        .collect()
}

// Booth's algorithm: linear comparisons and one final linear clone.
fn minimum_rotation_index<T: Ord>(ring: &[T]) -> usize {
    let n = ring.len();
    if n < 2 {
        return 0;
    }
    let (mut left, mut right, mut offset) = (0, 1, 0);
    while left < n && right < n && offset < n {
        match ring[(left + offset) % n].cmp(&ring[(right + offset) % n]) {
            std::cmp::Ordering::Equal => offset += 1,
            std::cmp::Ordering::Less => {
                right += offset + 1;
                if right == left {
                    right += 1;
                }
                offset = 0;
            }
            std::cmp::Ordering::Greater => {
                left += offset + 1;
                if left == right {
                    left += 1;
                }
                offset = 0;
            }
        }
    }
    left.min(right)
}

fn coordinates(points: &[Coord3D]) -> crate::Result<Vec<CoordinateFingerprintV1>> {
    points.iter().copied().map(coordinate_fingerprint).collect()
}

pub(crate) fn coordinate_fingerprint(point: Coord3D) -> crate::Result<CoordinateFingerprintV1> {
    Ok(CoordinateFingerprintV1 {
        x: float_bits(point.x)?,
        y: float_bits(point.y)?,
        z: float_bits(point.z)?,
    })
}

pub(crate) fn float_bits(value: f64) -> crate::Result<String> {
    if !value.is_finite() {
        return Err(PolygonizeError::InvalidGeometry {
            reason: "fingerprint coordinates must be finite".to_string(),
        });
    }
    Ok(format!("0x{:016x}", canonical_coordinate_bits(value)))
}

fn id32(value: u32) -> String {
    format!("0x{value:08x}")
}

fn id64(value: u64) -> String {
    format!("0x{value:016x}")
}

fn id_usize(value: usize) -> String {
    format!("0x{value:016x}")
}

fn sort_key<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("fingerprint serializes")
}

fn diff_value(path: &str, expected: &Value, actual: &Value) -> Option<FingerprintDiffV1> {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            let keys: std::collections::BTreeSet<_> =
                expected.keys().chain(actual.keys()).collect();
            for key in keys {
                let next = format!("{path}.{key}");
                match (expected.get(key), actual.get(key)) {
                    (Some(expected), Some(actual)) => {
                        if let Some(diff) = diff_value(&next, expected, actual) {
                            return Some(diff);
                        }
                    }
                    _ => {
                        return Some(FingerprintDiffV1 {
                            path: next,
                            expected: expected.get(key).cloned().unwrap_or(Value::Null),
                            actual: actual.get(key).cloned().unwrap_or(Value::Null),
                        })
                    }
                }
            }
            None
        }
        (Value::Array(expected), Value::Array(actual)) => {
            for index in 0..expected.len().max(actual.len()) {
                let next = format!("{path}[{index}]");
                match (expected.get(index), actual.get(index)) {
                    (Some(expected), Some(actual)) => {
                        if let Some(diff) = diff_value(&next, expected, actual) {
                            return Some(diff);
                        }
                    }
                    _ => {
                        return Some(FingerprintDiffV1 {
                            path: next,
                            expected: expected.get(index).cloned().unwrap_or(Value::Null),
                            actual: actual.get(index).cloned().unwrap_or(Value::Null),
                        })
                    }
                }
            }
            None
        }
        _ if expected == actual => None,
        _ => Some(FingerprintDiffV1 {
            path: path.to_string(),
            expected: expected.clone(),
            actual: actual.clone(),
        }),
    }
}
