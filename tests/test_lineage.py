import unittest
import numpy as np
import geo_polygonize

class TestLineage(unittest.TestCase):
    def test_triangle(self):
        # Triangle
        coords = np.array([
            [0.0, 0.0], [10.0, 0.0],
            [10.0, 0.0], [0.0, 10.0],
            [0.0, 10.0], [0.0, 0.0]
        ])
        offsets = np.array([0, 2, 4, 6], dtype=np.uint32)
        line_ids = np.array([1, 2, 3], dtype=np.uint32)

        result = geo_polygonize.polygonize(coords, offsets, line_ids=line_ids)

        # The native implementation returns a dict with 'flat_line_ids'.
        # 'polygons' is not directly populated by PyO3 impl, it seems I need to reconstruct it
        # OR fix the PyO3 impl to return 'polygons'.
        # Wait, the `cffi_wrapper.py` constructs `polygons`.
        # But `geo_polygonize_core` (PyO3) only returns the raw arrays (see python.rs).
        # Ah! `python.rs` returns `dict` but it does NOT construct `SimplePolygon` objects.

        # The `cffi_wrapper` does construct them.
        # This inconsistency needs to be addressed or I should adapt the test.
        # Ideally, `python.rs` should also return `polygons`.
        # However, constructing Python objects in Rust via PyO3 is possible but maybe the original author intended
        # the wrapper to do it?
        # Checking `python/geo_polygonize/__init__.py`: it just calls `_polygonize_impl`.

        # If `_polygonize_impl` is the PyO3 module, it returns what `python.rs` defines.
        # `python.rs` returns: `flat_coords`, `ring_offsets`, `polygon_offsets`, `flat_line_ids`, `stride`.
        # It misses `polygons`, `dangles`, `invalid_rings`.

        # I should probably update `python/geo_polygonize/__init__.py` to construct `polygons` if they are missing
        # OR update `python.rs` to return them.
        # Given I can't easily import `SimplePolygon` in Rust without more PyO3 boilerplate,
        # I will update `__init__.py` to hydrate the result if needed.

        flat_line_ids = result.get('flat_line_ids')
        self.assertIsNotNone(flat_line_ids)
        self.assertTrue(len(flat_line_ids) > 0)

        # Verify IDs presence in flat array
        self.assertTrue(1 in flat_line_ids)
        self.assertTrue(2 in flat_line_ids)
        self.assertTrue(3 in flat_line_ids)

    def test_splitting(self):
        # Cross + Box
        # L1: (0,0)->(10,10) ID=10
        # L2: (0,10)->(10,0) ID=20
        # Box: (0,0)-(10,0)-(10,10)-(0,10)-(0,0). IDs 100, 101, 102, 103.

        coords_list = [
            [0.0, 0.0], [10.0, 10.0], # L1
            [0.0, 10.0], [10.0, 0.0], # L2
            [0.0, 0.0], [10.0, 0.0], # B1
            [10.0, 0.0], [10.0, 10.0], # B2
            [10.0, 10.0], [0.0, 10.0], # B3
            [0.0, 10.0], [0.0, 0.0], # B4
        ]
        coords = np.array(coords_list)
        offsets = np.array([0, 2, 4, 6, 8, 10, 12], dtype=np.uint32)
        line_ids = np.array([10, 20, 100, 101, 102, 103], dtype=np.uint32)

        result = geo_polygonize.polygonize(coords, offsets, node=True, line_ids=line_ids)

        flat_line_ids = result.get('flat_line_ids')
        self.assertIsNotNone(flat_line_ids)

        # Check presence of box edge and diagonal parts
        has_box = any(i >= 100 for i in flat_line_ids)
        has_l1 = 10 in flat_line_ids
        has_l2 = 20 in flat_line_ids

        self.assertTrue(has_box)
        self.assertTrue(has_l1)
        self.assertTrue(has_l2)

if __name__ == '__main__':
    unittest.main()
