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
        offsets = np.array([0, 2, 4], dtype=np.uint32)
        line_ids = np.array([1, 2, 3], dtype=np.uint32)

        result = geo_polygonize.polygonize(coords, offsets, line_ids=line_ids)

        polys = result.get('polygons')
        self.assertIsNotNone(polys)
        self.assertEqual(len(polys), 1)

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
        offsets = np.array([0, 2, 4, 6, 8, 10], dtype=np.uint32)
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
