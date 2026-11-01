"""Distances between two editions' collision meshes."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import version_collision as v


class CollisionTests(unittest.TestCase):
    def test_nearest_gap_finds_the_worst_vertex(self):
        left = [(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)]
        right = [(0.0, 0.0, 0.0), (10.0, 3.0, 4.0), (-7.0, 0.0, 0.0)]
        self.assertAlmostEqual(v.nearest_gap(left, right), 5.0)
        self.assertAlmostEqual(v.nearest_gap(right, left), 7.0)
        self.assertEqual(v.nearest_gap(left, left), 0.0)
        self.assertEqual(v.nearest_gap([], left), float("inf"))

    def test_rigid_offset_only_for_one_translation(self):
        a = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)]
        self.assertEqual(v.rigid_offset(a, [(1.0, 1.0, 0.0), (0.0, 1.0, 0.0)]), [0.0, 1.0, 0.0])
        self.assertIsNone(v.rigid_offset(a, [(0.0, 1.0, 0.0), (1.0, 3.0, 0.0)]))
        self.assertIsNone(v.rigid_offset(a, a[:1]))


if __name__ == "__main__":
    unittest.main()
