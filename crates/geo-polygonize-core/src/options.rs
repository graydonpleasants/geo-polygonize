use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
/// The canonical configuration object for the `geo-polygonize` engine.
///
/// This struct controls every aspect of the polygonization pipeline, including
/// topological robustness, feature output, containment policies, noding, and determinism.
pub struct PolygonizerOptions {
    /// Determines the execution environment profile (e.g., Native, WasmSingleThread, WasmThreads).
    ///
    /// Default: `TargetProfile::Native`
    pub target: TargetProfile,

    /// Whether to robustly node the input before polygonization.
    ///
    /// Enable this for real-world linework where segment intersections may not
    /// already exist as explicit vertices. This is slower than the fast path but
    /// avoids missing faces and unresolved crossings.
    ///
    /// Default: `false`
    pub node_input: bool,

    /// The snapping grid size used for vertex deduplication and noding operations.
    ///
    /// Vertices falling within the same grid cell are coalesced. A size of `0.0`
    /// indicates exact floating-point evaluation without grid snapping.
    ///
    /// Default: `1e-10`
    pub snap_grid_size: f64,

    /// If `true`, only pure, outermost polygonal shells are returned.
    ///
    /// Floating dangles, internal cut-lines, or invalid rings will be discarded.
    ///
    /// Default: `false`
    pub extract_only_polygonal: bool,

    /// The underlying strategy to apply when snapping coordinate geometries.
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

    /// Optional configuration for tiled, distributed execution across huge datasets.
    #[ts(optional)]
    pub tiling: Option<TilingOptions>,

    /// Configures Z-axis coordinate handling.
    pub z: ZOptions,

    /// Configuration for enforcing exact topological determinism.
    pub determinism: DeterminismOptions,

    /// Options for capturing diagnostic topology failures.
    pub diagnostics: DiagnosticsOptions,

    /// Options for mapping final faces back to original input geometry IDs.
    pub provenance: ProvenanceOptions,

    /// An optional identifier for the input dataset.
    #[ts(optional)]
    pub input_profile_id: Option<String>,
}

impl Default for PolygonizerOptions {
    fn default() -> Self {
        Self {
            target: TargetProfile::Native,
            node_input: false,
            snap_grid_size: 1e-10,
            extract_only_polygonal: false,
            snap_strategy: SnapStrategy::Grid,
            noding: NodingOptions::default(),
            containment: ContainmentOptions::default(),
            tiling: None,
            z: ZOptions::default(),
            determinism: DeterminismOptions::default(),
            diagnostics: DiagnosticsOptions::default(),
            provenance: ProvenanceOptions::default(),
            input_profile_id: None,
        }
    }
}

impl PolygonizerOptions {
    pub fn cfb_robust_v1() -> Self {
        Self {
            target: TargetProfile::Native,
            node_input: true,
            snap_grid_size: 0.5,
            extract_only_polygonal: false,
            snap_strategy: SnapStrategy::GeosCompat,
            noding: NodingOptions {
                backend: NodingBackend::Snap,
                snap_mode: SnapMode::FloatEpsilonDedup,
            },
            containment: ContainmentOptions {
                touch_policy: TouchPolicy::AllowPointTouchDisallowEdgeShare,
                index_backend: IndexBackend::RStar,
            },
            tiling: None,
            z: ZOptions {
                policy: ZPolicy::InterpolateAlongEdge,
            },
            determinism: DeterminismOptions {
                canonical_sort: true,
                canonical_ring_rotation: true,
                stable_tie_breaks: true,
            },
            diagnostics: DiagnosticsOptions {
                enabled: true,
                report_mode: true,
            },
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            input_profile_id: Some("cfb_robust_v1".to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TargetProfile {
    Native,
    WasmSingleThread,
    WasmThreads,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
/// Strategy for snapping coordinates to a grid.
///
/// `Grid` applies a standard round-to-nearest integer grid policy `(coord / size).round() * size`.
/// `GeosCompat` attempts to emulate GEOS / Shapely's `set_precision` behavior, which uses
/// C++ std::round (rounding halfway cases away from zero).
///
/// **Scale Guidance:**
/// Use `Grid` for native Rust applications where absolute topological determinism and
/// intuitive rounding is preferred.
/// Use `GeosCompat` if you are using `geo-polygonize` to replace a Shapely / GEOS pipeline
/// and require exact parity for edge cases at precision boundaries.
///
/// **Expected Parity:**
/// Rust's native `f64::round()` rounds halfway cases away from zero. This perfectly
/// matches C++ `std::round` behavior, meaning that `Grid` and `GeosCompat` currently
/// map identical values for basic single-coordinate precision points (e.g., `-0.5` => `-1.0`
/// and `0.5` => `1.0`).
///
/// **Expected Divergence:**
/// The divergence between `Grid` and `GeosCompat` strategies typically arises under complex
/// topological scaling sequences rather than simple single point coordinate rounding, but
/// the option ensures the library remains semantically compatible with Python/C++ GEOS pipelines.
pub enum SnapStrategy {
    Grid,
    GeosCompat,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SnapMode {
    FloatExact,
    FloatEpsilonDedup,
    IntegerGrid,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ZPolicy {
    #[default]
    Ignore,
    InterpolateAlongEdge,
    PreferNearestEndpoint,
    ErrorOnConflict {
        max_delta: f64,
    },
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
    Centroid,
    RepresentativePointInsidePolygon,
    LexicographicMinVertex,
    CanonicalBoundaryHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum NodingBackend {
    Snap,
    Advanced,
    // placeholders for future backends
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum IndexBackend {
    RStar,
    PackedNative,
    // placeholders for future backends
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NodingOptions {
    pub backend: NodingBackend,
    pub snap_mode: SnapMode,
}

impl Default for NodingOptions {
    fn default() -> Self {
        Self {
            backend: NodingBackend::Snap,
            snap_mode: SnapMode::FloatEpsilonDedup,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ContainmentOptions {
    pub touch_policy: TouchPolicy,
    pub index_backend: IndexBackend,
}

impl Default for ContainmentOptions {
    fn default() -> Self {
        Self {
            touch_policy: TouchPolicy::AllowPointTouchDisallowEdgeShare,
            index_backend: IndexBackend::RStar,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
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
#[ts(export)]
pub struct DiagnosticsOptions {
    pub enabled: bool,
    pub report_mode: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProvenanceOptions {
    pub enabled: bool,
    pub include_boundary_line_ids: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DedupPolicy {
    #[default]
    KeepAll,
    CanonicalRingHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TilingOptions {
    pub ownership_policy: TileOwnershipPolicy,
    #[serde(default)]
    pub dedup_policy: DedupPolicy,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ZOptions {
    pub policy: ZPolicy,
}
