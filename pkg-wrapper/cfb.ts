import type { PolygonizerOptions } from "./bindings/PolygonizerOptions";

export const cfbRobustOptions: PolygonizerOptions = {
    target: "Native",
    node_input: true,
    snap_grid_size: 0.5,
    extract_only_polygonal: false,
    snap_strategy: "GeosCompat",
    noding: {
        backend: "Snap",
        snap_mode: "FloatEpsilonDedup",
    },
    containment: {
        touch_policy: "AllowPointTouchDisallowEdgeShare",
        index_backend: "RStar",
    },
    z: {
        policy: "InterpolateAlongEdge",
    },
    determinism: {
        canonical_sort: true,
        canonical_ring_rotation: true,
        stable_tie_breaks: true,
    },
    diagnostics: {
        enabled: true,
        report_mode: true,
    },
    provenance: {
        enabled: true,
        include_boundary_line_ids: true,
    },
    input_profile_id: "cfb_robust_v1",
};
