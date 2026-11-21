"""Hunt sides: big alive counts above a floor, extra alive, pools and skill."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gen_missions as g


class HuntSideTests(unittest.TestCase):
    def test_unlisted_hunt_plays_the_default_on_every_tier(self):
        with patch.dict(g.HUNT_ALIVE, {}, clear=True), \
                patch.dict(g.HUNT_ALIVE_EXTRA, {}, clear=True):
            self.assertEqual(g.hunt_alive_rows('bf2_end1'),
                             [(g.HUNT_ALIVE_DEFAULT,) * 2] * 4)

    def test_extra_alive_adds_on_top_of_the_side_count(self):
        with patch.dict(g.HUNT_ALIVE, {'x': [(24, 24), (24, 24), (32, 32), (32, 32)]}), \
                patch.dict(g.HUNT_ALIVE_EXTRA, {'x': [(0, 1), (0, 2), (2, 0), (0, 0)]}):
            self.assertEqual(g.hunt_alive_rows('x'),
                             [(24, 25), (24, 26), (34, 32), (32, 32)])

    def test_every_tier_keeps_both_sides_at_or_above_the_floor(self):
        floor = g.HUNT_ALIVE_FLOOR
        self.assertEqual(floor, 8)
        self.assertTrue(g.hunt_alive_ok([(floor, 40)] * 4))
        self.assertFalse(g.hunt_alive_ok([(floor - 1, 40)] + [(floor, floor)] * 3))
        # Above the disc's units is allowed: no ceiling.
        self.assertTrue(g.hunt_alive_ok([(64, 64)] * 4))
        self.assertFalse(g.hunt_alive_ok([(16, 16)] * 3))

    def test_extra_skill_and_pool_bounds(self):
        top = g.HUNT_EXTRA_MAX
        self.assertTrue(g.hunt_extra_ok([(0, top)] * 4))
        self.assertFalse(g.hunt_extra_ok([(0, top + 1)] * 4))
        self.assertFalse(g.hunt_extra_ok([(-1, 0)] * 4))
        self.assertTrue(g.hunt_skill_ok([(100, 30)] * 4))
        self.assertFalse(g.hunt_skill_ok([(101, 100)] * 4))
        rows = [(16, 16), (16, 18), (24, 16), (16, 16)]
        self.assertTrue(g.hunt_units_ok((24, 18), rows))
        self.assertFalse(g.hunt_units_ok((23, 40), rows))

    def test_the_generator_refuses_a_skill_below_30_percent(self):
        self.assertEqual(g.HUNT_SKILL_FLOOR, 30)
        self.assertFalse(g.hunt_skill_ok([(29, 100)] + [(100, 100)] * 3))
        self.assertFalse(g.hunt_skill_ok([(100, 1)] * 4))
        for key, rows in g.HUNT_SKILL_PCT.items():
            self.assertTrue(g.hunt_skill_ok(rows), key)

    def test_the_rejected_low_caps_are_gone(self):
        for key in set(g.HUNT_POINT_TARGETS) | set(g.HUNT_ALIVE):
            self.assertTrue(g.hunt_alive_ok(g.hunt_alive_rows(key)), key)


if __name__ == '__main__':
    unittest.main()
