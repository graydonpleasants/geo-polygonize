import json

def get_default_options():
    return {
        "target": "Native",
        "node_input": False,
        "snap_grid_size": 1e-10,
        "extract_only_polygonal": False,
        "snap_strategy": "Grid",
        "noding": {
            "backend": "Snap",
            "snap_mode": "FloatEpsilonDedup"
        },
        "containment": {
            "touch_policy": "AllowPointTouchDisallowEdgeShare",
            "index_backend": "RStar"
        },
        "tiling": None,
        "z": {
            "policy": "Ignore"
        },
        "determinism": {
            "canonical_sort": True,
            "canonical_ring_rotation": True,
            "stable_tie_breaks": True
        },
        "diagnostics": {
            "enabled": False,
            "report_mode": False
        },
        "provenance": {
            "enabled": False,
            "include_boundary_line_ids": False
        },
        "input_profile_id": None
    }
