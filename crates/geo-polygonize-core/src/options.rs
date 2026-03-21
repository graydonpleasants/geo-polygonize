use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PolygonizerOptions {
    pub target: TargetProfile,
    pub node_input: bool,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: bool,
    pub snap_strategy: SnapStrategy,
    pub noding: NodingOptions,
    pub containment: ContainmentOptions,
    #[ts(optional)]
    pub tiling: Option<TilingOptions>,
    pub z: ZOptions,
    pub determinism: DeterminismOptions,
    pub diagnostics: DiagnosticsOptions,
    pub provenance: ProvenanceOptions,
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

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TargetProfile {
    Native,
    WasmSingleThread,
    WasmThreads,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
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
