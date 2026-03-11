use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolygonizerOptions {
    pub target: TargetProfile,
    pub node_input: bool,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: bool,
    pub snap_strategy: SnapStrategy,
    pub noding: NodingOptions,
    pub containment: ContainmentOptions,
    pub tiling: Option<TilingOptions>,
    pub z: ZOptions,
    pub determinism: DeterminismOptions,
    pub diagnostics: DiagnosticsOptions,
    pub provenance: ProvenanceOptions,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TargetProfile {
    Native,
    WasmSingleThread,
    WasmThreads,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SnapStrategy {
    Grid,
    GeosCompat,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SnapMode {
    FloatExact,
    FloatEpsilonDedup,
    IntegerGrid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ZPolicy {
    Ignore,
    InterpolateAlongEdge,
    PreferNearestEndpoint,
    ErrorOnConflict { max_delta: f64 },
}

impl Default for ZPolicy {
    fn default() -> Self {
        ZPolicy::Ignore
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TouchPolicy {
    AllowPointTouchDisallowEdgeShare,
    TreatAnyTouchAsDisjoint,
    AllowEdgeShare,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TileOwnershipPolicy {
    Centroid,
    RepresentativePointInsidePolygon,
    LexicographicMinVertex,
    CanonicalBoundaryHash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodingBackend {
    Snap,
    // placeholders for future backends
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum IndexBackend {
    RStar,
    // placeholders for future backends
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagnosticsOptions {
    pub enabled: bool,
    pub report_mode: bool,
}

impl Default for DiagnosticsOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            report_mode: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvenanceOptions {
    pub enabled: bool,
    pub include_boundary_line_ids: bool,
}

impl Default for ProvenanceOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            include_boundary_line_ids: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TilingOptions {
    pub ownership_policy: TileOwnershipPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZOptions {
    pub policy: ZPolicy,
}

impl Default for ZOptions {
    fn default() -> Self {
        Self {
            policy: ZPolicy::default(),
        }
    }
}
