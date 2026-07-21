use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{PolygonizeError, Result};

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
/// The canonical configuration object for the `geo-polygonize` engine.
///
/// This struct controls every aspect of the polygonization pipeline, including
/// topological robustness, feature output, containment policies, noding, and determinism.
pub struct PolygonizerOptions {
    /// Whether to robustly node the input before polygonization.
    ///
    /// Enable this for real-world linework where segment intersections may not
    /// already exist as explicit vertices. This is slower than the fast path but
    /// avoids missing faces and unresolved crossings.
    ///
    /// Default: `false`
    pub node_input: bool,

    /// Coordinate precision used for topology and noding.
    ///
    /// Default: `PrecisionModel::Floating`
    pub precision_model: PrecisionModel,

    /// Snap input segments to nearby vertices from exact-noded input linework before grid noding.
    ///
    /// A value of `0.0` disables pre-snap. This mirrors the CFB/Shapely
    /// `snap(line, unary_union(all_lines), tolerance)` step closely enough to
    /// close small CAD gaps before polygonization.
    ///
    /// Default: `0.0`
    #[serde(default)]
    pub pre_snap_tolerance: f64,

    /// If `true`, only pure, outermost polygonal shells are returned.
    ///
    /// Floating dangles, internal cut-lines, or invalid rings will be discarded.
    ///
    /// Default: `false`
    pub extract_only_polygonal: bool,

    /// Controls robust snap noding and output coordinate handling.
    ///
    /// See `SnapStrategy` for differences between strict `Grid` snapping and
    /// Shapely/GEOS `GeosCompat` strategies.
    ///
    /// Default: `SnapStrategy::Grid`
    pub snap_strategy: SnapStrategy,

    /// Configures the noding engine backend and behavior.
    pub noding: NodingOptions,

    /// Configures how topological relationships (containment) are calculated
    /// during face formation.
    pub containment: ContainmentOptions,

    /// Configuration for enforcing exact topological determinism.
    pub determinism: DeterminismOptions,

    /// Options for capturing diagnostic topology failures.
    pub diagnostics: DiagnosticsOptions,

    /// Options for mapping final faces back to original input geometry IDs.
    pub provenance: ProvenanceOptions,

    /// Controls Z reconstruction and same-XY conflict handling.
    pub z: ZOptions,

    /// Optional application-level filtering applied after topology is established.
    pub output_filter: OutputFilterOptions,

    /// An optional identifier for the input dataset.
    #[ts(optional)]
    pub input_profile_id: Option<String>,
}

impl Default for PolygonizerOptions {
    fn default() -> Self {
        Self {
            node_input: false,
            precision_model: PrecisionModel::Floating,
            pre_snap_tolerance: 0.0,
            extract_only_polygonal: false,
            snap_strategy: SnapStrategy::Grid,
            noding: NodingOptions::default(),
            containment: ContainmentOptions::default(),
            determinism: DeterminismOptions::default(),
            diagnostics: DiagnosticsOptions::default(),
            provenance: ProvenanceOptions::default(),
            z: ZOptions::default(),
            output_filter: OutputFilterOptions::default(),
            input_profile_id: None,
        }
    }
}

impl PolygonizerOptions {
    pub fn cfb_robust_v1() -> Self {
        Self {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 0.1 },
            pre_snap_tolerance: 0.5,
            extract_only_polygonal: false,
            snap_strategy: SnapStrategy::GeosCompat,
            noding: NodingOptions {
                backend: NodingBackend::Snap,
                guarantee: NodingGuarantee::Unchecked,
            },
            containment: ContainmentOptions {
                touch_policy: TouchPolicy::AllowPointTouchDisallowEdgeShare,
            },
            determinism: DeterminismOptions {
                canonical_sort: true,
                canonical_ring_rotation: true,
                stable_tie_breaks: true,
            },
            diagnostics: DiagnosticsOptions {
                enabled: true,
                report_mode: true,
                timings: false,
            },
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            z: ZOptions::default(),
            output_filter: OutputFilterOptions::default(),
            input_profile_id: Some("cfb_robust_v1".to_string()),
        }
    }

    pub fn validate(&self) -> Result<()> {
        for (field, value) in [("pre_snap_tolerance", self.pre_snap_tolerance)] {
            if !value.is_finite() || value < 0.0 {
                return Err(PolygonizeError::InvalidArgumentType {
                    field: field.to_string(),
                    expected: "a finite non-negative number".to_string(),
                    actual: value.to_string(),
                });
            }
        }

        if let PrecisionModel::FixedGrid { grid_size } = self.precision_model {
            if !grid_size.is_finite() || grid_size <= 0.0 {
                return Err(PolygonizeError::InvalidArgumentType {
                    field: "precision_model.grid_size".to_string(),
                    expected: "a finite positive number".to_string(),
                    actual: grid_size.to_string(),
                });
            }
            if self.node_input && matches!(self.noding.backend, NodingBackend::Advanced) {
                return Err(PolygonizeError::UnsupportedOptionCombination {
                    reason: "the Advanced compatibility noder supports floating precision only"
                        .to_string(),
                });
            }
        }

        if self.pre_snap_tolerance > 0.0 && !self.node_input {
            return Err(PolygonizeError::UnsupportedOptionCombination {
                reason: "pre_snap_tolerance requires node_input=true".to_string(),
            });
        }

        if matches!(
            self.noding.guarantee,
            NodingGuarantee::CertifiedFixedPrecision
        ) && (!self.node_input
            || !matches!(self.precision_model, PrecisionModel::FixedGrid { .. })
            || !matches!(self.noding.backend, NodingBackend::Snap)
            || !matches!(self.snap_strategy, SnapStrategy::Grid))
        {
            return Err(PolygonizeError::UnsupportedOptionCombination {
                reason: "CertifiedFixedPrecision requires node_input=true, FixedGrid precision, the Snap backend, and the Grid snap strategy".to_string(),
            });
        }

        if let Some(value) = self.output_filter.minimum_face_area {
            if !value.is_finite() || value < 0.0 {
                return Err(PolygonizeError::InvalidArgumentType {
                    field: "output_filter.minimum_face_area".to_string(),
                    expected: "a finite non-negative number".to_string(),
                    actual: value.to_string(),
                });
            }
        }

        if !self.z.conflict_tolerance.is_finite() || self.z.conflict_tolerance < 0.0 {
            return Err(PolygonizeError::InvalidArgumentType {
                field: "z.conflict_tolerance".to_string(),
                expected: "a finite non-negative number".to_string(),
                actual: self.z.conflict_tolerance.to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
/// Coordinate precision used by the topology pipeline.
pub enum PrecisionModel {
    /// Preserve input coordinates and compute intersections in floating point.
    #[default]
    Floating,
    /// Round topology coordinates to a fixed grid of positive cell size.
    FixedGrid { grid_size: f64 },
}

impl PrecisionModel {
    pub fn grid_size(self) -> f64 {
        match self {
            Self::Floating => 0.0,
            Self::FixedGrid { grid_size } => grid_size,
        }
    }

    pub fn from_grid_size(grid_size: f64) -> Self {
        if grid_size == 0.0 {
            Self::Floating
        } else {
            Self::FixedGrid { grid_size }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
/// Strategy for robust snap noding and output coordinates.
///
/// `Grid` uses the precision grid for both topology and output coordinates.
/// `GeosCompat` uses the grid for topology, then restores one deterministic nearest source XY
/// coordinate per snapped node. This targets Shapely-style `snap` followed by full-precision
/// noding and polygonization; it does not emulate `set_precision` output.
///
/// **Scale Guidance:**
/// Use `Grid` when an explicit precision model is the desired output contract.
/// Use `GeosCompat` when the grid is a robustness aid but source-coordinate fidelity matters.
/// Both strategies are deterministic; exact GEOS/Shapely parity is not guaranteed for
/// degenerate or many-to-one snaps.
pub enum SnapStrategy {
    Grid,
    GeosCompat,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TouchPolicy {
    AllowPointTouchDisallowEdgeShare,
    TreatAnyTouchAsDisjoint,
    AllowEdgeShare,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TileOwnershipPolicy {
    /// Fast ownership using the polygon centroid, which may lie outside a concave polygon.
    Centroid,
    /// Ownership using a point guaranteed to intersect the polygon interior when one exists.
    RepresentativePointInsidePolygon,
    /// Ownership using the smallest boundary vertex in XY order.
    LexicographicMinVertex,
    /// Legacy deterministic ownership policy; now uses a safe interior point.
    CanonicalBoundaryHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum NodingBackend {
    /// Snap-rounding noder using the configured precision grid.
    Snap,
    /// Deprecated compatibility alias for exact (`grid_size = 0`) snap noding.
    ///
    /// The experimental sweep-line implementation was retired because it did not
    /// maintain the invariants required for complete intersection enumeration.
    Advanced,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum NodingGuarantee {
    /// Trust the selected noder without checking its output.
    #[default]
    Unchecked,
    /// Verify that the resulting segments are fully noded and normalized.
    Validate,
    /// Use hot-pixel snap rounding and verify the fixed-grid result.
    CertifiedFixedPrecision,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct NodingOptions {
    pub backend: NodingBackend,
    pub guarantee: NodingGuarantee,
}

impl Default for NodingOptions {
    fn default() -> Self {
        Self {
            backend: NodingBackend::Snap,
            guarantee: NodingGuarantee::Unchecked,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ContainmentOptions {
    pub touch_policy: TouchPolicy,
}

impl Default for ContainmentOptions {
    fn default() -> Self {
        Self {
            touch_policy: TouchPolicy::AllowPointTouchDisallowEdgeShare,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct DeterminismOptions {
    pub canonical_sort: bool,
    pub canonical_ring_rotation: bool,
    pub stable_tie_breaks: bool,
}

impl Default for DeterminismOptions {
    fn default() -> Self {
        Self {
            canonical_sort: true,
            canonical_ring_rotation: true,
            stable_tie_breaks: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct DiagnosticsOptions {
    pub enabled: bool,
    pub report_mode: bool,
    // Collect phase timings without enabling the more expensive work counters.
    #[serde(default)]
    pub timings: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ProvenanceOptions {
    pub enabled: bool,
    pub include_boundary_line_ids: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ZPolicy {
    /// Ignore input Z and emit `0.0`.
    Ignore,
    /// Interpolate split-vertex Z along each source edge before graph reconciliation.
    #[default]
    InterpolateAlongEdge,
    /// Use the nearest source endpoint's Z for each split vertex.
    PreferNearestEndpoint,
    /// Interpolate Z, then fail if one XY node receives conflicting values.
    ErrorOnConflict,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ZOptions {
    pub policy: ZPolicy,
    /// Z values at one XY node conflict when their difference exceeds this value.
    pub conflict_tolerance: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct OutputFilterOptions {
    /// Keep faces whose area is greater than or equal to this value.
    #[ts(optional)]
    pub minimum_face_area: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DedupPolicy {
    #[default]
    KeepAll,
    CanonicalRingHash,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_uses_defaults() {
        let options: PolygonizerOptions = serde_json::from_str(
            r#"{"diagnostics":{"enabled":true},"output_filter":{"minimum_face_area":2.0}}"#,
        )
        .unwrap();

        assert!(options.diagnostics.enabled);
        assert!(!options.diagnostics.report_mode);
        assert_eq!(options.precision_model, PrecisionModel::Floating);
        assert_eq!(options.output_filter.minimum_face_area, Some(2.0));
        assert_eq!(options.z, ZOptions::default());

        let fixed: PolygonizerOptions =
            serde_json::from_str(r#"{"precision_model":{"type":"fixed_grid","grid_size":0.25}}"#)
                .unwrap();
        assert_eq!(
            fixed.precision_model,
            PrecisionModel::FixedGrid { grid_size: 0.25 }
        );

        let validated: PolygonizerOptions =
            serde_json::from_str(r#"{"noding":{"guarantee":"Validate"}}"#).unwrap();
        assert_eq!(validated.noding.guarantee, NodingGuarantee::Validate);

        let z: PolygonizerOptions =
            serde_json::from_str(r#"{"z":{"policy":"ErrorOnConflict"}}"#).unwrap();
        assert_eq!(z.z.policy, ZPolicy::ErrorOnConflict);
        assert_eq!(z.z.conflict_tolerance, 0.0);
    }

    #[test]
    fn legacy_snap_grid_size_is_rejected_instead_of_ignored() {
        let error =
            serde_json::from_str::<PolygonizerOptions>(r#"{"snap_grid_size":0.1}"#).unwrap_err();
        assert!(error.to_string().contains("unknown field `snap_grid_size`"));
    }

    #[test]
    fn validation_rejects_invalid_options() {
        for value in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let options = PolygonizerOptions {
                node_input: true,
                pre_snap_tolerance: value,
                ..Default::default()
            };
            assert!(options.validate().is_err());

            let options = PolygonizerOptions {
                output_filter: OutputFilterOptions {
                    minimum_face_area: Some(value),
                },
                ..Default::default()
            };
            assert!(options.validate().is_err());

            let options = PolygonizerOptions {
                z: ZOptions {
                    conflict_tolerance: value,
                    ..Default::default()
                },
                ..Default::default()
            };
            assert!(options.validate().is_err());
        }

        for grid_size in [-1.0, 0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let options = PolygonizerOptions {
                precision_model: PrecisionModel::FixedGrid { grid_size },
                ..Default::default()
            };
            assert!(options.validate().is_err());
        }

        let options = PolygonizerOptions {
            pre_snap_tolerance: 1.0,
            ..Default::default()
        };
        assert!(matches!(
            options.validate(),
            Err(PolygonizeError::UnsupportedOptionCombination { .. })
        ));

        let options = PolygonizerOptions {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
            noding: NodingOptions {
                backend: NodingBackend::Advanced,
                guarantee: NodingGuarantee::Unchecked,
            },
            ..Default::default()
        };
        assert!(matches!(
            options.validate(),
            Err(PolygonizeError::UnsupportedOptionCombination { .. })
        ));

        let certified = PolygonizerOptions {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
            noding: NodingOptions {
                guarantee: NodingGuarantee::CertifiedFixedPrecision,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(certified.validate().is_ok());
        assert!(PolygonizerOptions {
            node_input: false,
            ..certified.clone()
        }
        .validate()
        .is_err());
        assert!(PolygonizerOptions {
            precision_model: PrecisionModel::Floating,
            ..certified
        }
        .validate()
        .is_err());

        assert!(PolygonizerOptions::default().validate().is_ok());
    }
}
