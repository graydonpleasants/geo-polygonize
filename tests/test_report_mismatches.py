import unittest
import numpy as np
import geo_polygonize

def generate_winding_chaos():
    """Identical boundaries drawn in opposite directions and fragmented."""
    lines = []
    # Square CW
    lines.extend([(0,0), (0,10), (10,10), (10,0), (0,0)])
    # Square CCW
    lines.extend([(0,0), (10,0), (10,10), (0,10), (0,0)])

    # Fragmented overlapping lines
    lines.extend([(0,0), (5,0)])
    lines.extend([(10,0), (2,0)]) # Reversed and overlapping

    return lines

class TestReportMismatches(unittest.TestCase):
    def test_cfb_robust_report_mode_matches_itself(self):
        coords = np.array([
            0.0, 0.0, 10.0, 0.0,
            10.0, 0.0, 10.0, 10.0,
            10.0, 10.0, 0.0, 10.0,
            0.0, 10.0, 0.0, 0.0
        ], dtype=np.float64)
        offsets = np.array([0, 2, 4, 6], dtype=np.uint32)
        options = geo_polygonize.cfb_robust_options()

        result_a = geo_polygonize.polygonize_with_options(coords=coords, offsets=offsets, options=options)
        result_b = geo_polygonize.polygonize_with_options(coords=coords, offsets=offsets, options=options)
        result_a["options"] = options
        result_b["options"] = options

        mismatch_info = geo_polygonize.explain_mismatch(result_a, result_b)
        self.assertTrue(mismatch_info["is_match"], mismatch_info["mismatches"])

    def test_options_mismatch(self):
        coords = np.array([
            0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0,
            0.0, 0.0, 10.0, 10.0
        ], dtype=np.float64)
        offsets = np.array([0, 5], dtype=np.uint32)

        options_a = {            "node_input": True,
            "snap_grid_size": 1e-10,
            "extract_only_polygonal": False,
            "snap_strategy": "Grid",
            "noding": {"backend": "Snap"},
            "containment": {"touch_policy": "AllowPointTouchDisallowEdgeShare"},            "determinism": {"canonical_sort": True, "canonical_ring_rotation": True, "stable_tie_breaks": True},
            "diagnostics": {"enabled": True, "report_mode": True},
            "provenance": {"enabled": False, "include_boundary_line_ids": False}
        }
        options_b = {            "node_input": False,
            "snap_grid_size": 1e-6,
            "extract_only_polygonal": False,
            "snap_strategy": "Grid",
            "noding": {"backend": "Snap"},
            "containment": {"touch_policy": "AllowPointTouchDisallowEdgeShare"},            "determinism": {"canonical_sort": True, "canonical_ring_rotation": True, "stable_tie_breaks": True},
            "diagnostics": {"enabled": True, "report_mode": True},
            "provenance": {"enabled": False, "include_boundary_line_ids": False}
        }

        result_a = geo_polygonize.polygonize_with_options(coords=coords, offsets=offsets, options=options_a)
        result_b = geo_polygonize.polygonize_with_options(coords=coords, offsets=offsets, options=options_b)

        # Manually add options dict back into results for checking
        result_a["options"] = options_a
        result_b["options"] = options_b

        mismatch_info = geo_polygonize.explain_mismatch(result_a, result_b)
        self.assertFalse(mismatch_info["is_match"])

        mismatches_str = " ".join(mismatch_info["mismatches"])
        self.assertIn("node_input", mismatches_str)
        self.assertIn("snap_grid_size", mismatches_str)

    def test_topology_mismatch(self):
        # Result A: Square with diagonal (noded) -> 2 triangles
        coords_a = np.array([
            0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0,
            0.0, 0.0, 10.0, 10.0
        ], dtype=np.float64)
        offsets_a = np.array([0, 5], dtype=np.uint32)

        # Result B: Square without diagonal -> 1 square
        coords_b = np.array([
            0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0, 0.0, 0.0
        ], dtype=np.float64)
        offsets_b = np.array([0], dtype=np.uint32)

        options = {            "node_input": True,
            "snap_grid_size": 1e-10,
            "extract_only_polygonal": False,
            "snap_strategy": "Grid",
            "noding": {"backend": "Snap"},
            "containment": {"touch_policy": "AllowPointTouchDisallowEdgeShare"},            "determinism": {"canonical_sort": True, "canonical_ring_rotation": True, "stable_tie_breaks": True},
            "diagnostics": {"enabled": True, "report_mode": True},
            "provenance": {"enabled": False, "include_boundary_line_ids": False}
        }

        result_a = geo_polygonize.polygonize_with_options(coords=coords_a, offsets=offsets_a, options=options)
        result_b = geo_polygonize.polygonize_with_options(coords=coords_b, offsets=offsets_b, options=options)

        mismatch_info = geo_polygonize.explain_mismatch(result_a, result_b)
        self.assertFalse(mismatch_info["is_match"])

        mismatches_str = " ".join(mismatch_info["mismatches"])
        self.assertIn("Polygon count mismatch", mismatches_str)

    def test_provenance_mismatch(self):
        # A simple square, but we pass different line_ids
        coords = np.array([
            [0.0, 0.0], [10.0, 0.0],
            [10.0, 0.0], [10.0, 10.0],
            [10.0, 10.0], [0.0, 10.0],
            [0.0, 10.0], [0.0, 0.0]
        ], dtype=np.float64)
        offsets = np.array([0, 2, 4, 6], dtype=np.uint32)

        line_ids_a = np.array([1, 2, 3, 4], dtype=np.uint32)
        line_ids_b = np.array([5, 6, 7, 8], dtype=np.uint32)

        options = {            "node_input": False,
            "snap_grid_size": 1e-10,
            "extract_only_polygonal": False,
            "snap_strategy": "Grid",
            "noding": {"backend": "Snap"},
            "containment": {"touch_policy": "AllowPointTouchDisallowEdgeShare"},            "determinism": {"canonical_sort": True, "canonical_ring_rotation": True, "stable_tie_breaks": True},
            "diagnostics": {"enabled": True, "report_mode": True},
            "provenance": {"enabled": True, "include_boundary_line_ids": True}
        }

        result_a = geo_polygonize.polygonize_with_options(coords=coords, offsets=offsets, options=options, line_ids=line_ids_a)
        result_b = geo_polygonize.polygonize_with_options(coords=coords, offsets=offsets, options=options, line_ids=line_ids_b)

        mismatch_info = geo_polygonize.explain_mismatch(result_a, result_b)
        self.assertFalse(mismatch_info["is_match"])

        mismatches_str = " ".join(mismatch_info["mismatches"])
        self.assertIn("Provenance mismatch", mismatches_str)


if __name__ == '__main__':
    unittest.main()
