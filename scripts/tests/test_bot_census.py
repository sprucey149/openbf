"""bot_census.py parsing, metrics and Wilson intervals; synthetic lines only, no rounds."""
from pathlib import Path
import os
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import bot_census as bc  # noqa: E402

TXT = ("tat1       team 1 wins at tick 10800 (360 s)  tickets 1/56  alive 1/24  y 11..48  cps 6  stuck_max 360  "
       "hash 4871b3fe30099849\n"
       "bf1_yav2   no outcome after 60000 ticks  tickets 3/3\n")
ERR = """recycle tat1: seed 42 tick 1 unit 2 gen 0 team 0 class 1 pos (0.0,0.0,0.0) leg x locus H1 floor_dy 0.0 steer idle objective none
relocate tat1: seed 42 tick 5 unit 3 team 1 from (0.0,0.0,0.0) to (1.0,0.0,0.0) locus H1 2.0 m
shares tat1: unit-s 100 | walk-in-place 0 (0.0 %) | stand20 60 in 6000 foot-s (1.00 %) [travel 60] | seated 0 (0.0 %) foot 6000 | top
behave tat1: foot-s 6000 | travel 3000 fight 1200 evade 0 capture 0 guard 0 board 0 follow 0 recover 120 pickup 0 none 0 overwatch 0 | B12 cover 60 at-position 600 of fight 1200 (5.0 %) modes none 0
hyst tat1: flips 40 kept 10 | wakes hit 1 near 2 seen 3 | wake changes []
deaths tat1: team0 kill 30 region 0 fall 0 vehicle 0 blast 0 | team1 kill 30 region 0 fall 0 vehicle 0 blast 0
d1 tat1 normal: thinks 1000 | human 0 far 0 (0.0 %) takes 0 far 0 | bot 400 far 8 (2.0 %) takes 40 far 1
levers tat1 normal: foot-min 100.0 thinks 1000 | reaction-wait 50 scope 5 | events flinch 20
think tat1: calls 9000 p50 1.5 p99 40.0 max 900.0 us | thinks 1000 p50 7.0 p99 160.0 | proxy calls 0 p50 0.0 p99 0.0
"""
DC = """d1 tat1 hard vs-human: thinks 100 | human 20 far 2 (10.0 %) takes 4 far 1 | bot 50 far 1 (2.0 %) takes 5 far 0
d1 tat1 normal: thinks 90 | human 0 far 0 (0.0 %) takes 0 far 0 | bot 40 far 4 (10.0 %) takes 4 far 0
d1 tat1 proxy normal: thinks 9 | human 0 far 0 (0.0 %) takes 0 far 0 | bot 9 far 9 (100.0 %) takes 1 far 1
levers tat1 hard vs-human: foot-min 10.0 thinks 100 | pick 5 | events flinch 3
think tat1: calls 10 p50 2.0 p99 30.0 max 50.0 us | thinks 5 p50 9.0 p99 99.0 | proxy calls 1 p50 3.0 p99 3.0
round tat1 s1 hard winner 0 margin -5 tickets 5/0  proxy 0 good kills 9 deaths 3 shots 1 hits 1 acc 1.000 life_s 9.0
round tat1 s2 hard winner 0 margin -5 tickets 5/0  proxy 1 good kills 9 deaths 3 shots 1 hits 1 acc 1.000 life_s 9.0
"""


class BotCensus(unittest.TestCase):
    def test_census_metrics(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d, "sweep_s42.txt").write_text(TXT)
            Path(d, "sweep_s42.err").write_text(ERR)
            m = bc.metrics(bc.merge(bc.load_dir(d, 60000).values()))
        self.assertEqual((m["recycles"], m["relocations"], m["stand20 foot-s"]), (1, 1, 60))
        self.assertAlmostEqual(m["recover % of foot"], 2.0)
        self.assertAlmostEqual(m["B12 cover % of fight"], 5.0)
        # 60 kills over 10800 + 60000 ticks (an unfinished round runs to the limit) = 39.33 min.
        self.assertAlmostEqual(m["kills/min"], 60 / (70800 / 1800))
        self.assertAlmostEqual(m["kills/foot-min"], 0.6)
        self.assertAlmostEqual(m["flips/fight-min"], 2.0)
        self.assertAlmostEqual(m["d1 bot far %"], 2.0)
        self.assertEqual(m["think p99 us/think"], 160.0)

    def test_wilson(self):
        lo, hi = bc.wilson(50, 192)
        self.assertAlmostEqual(lo, 20.3, places=1)
        self.assertAlmostEqual(hi, 32.7, places=1)

    def test_wilson_is_rounded_once(self):
        # 98.85 and 19.9455 exactly: rounding through two decimals first gave 98.9 and 20.0.
        self.assertEqual(bc.interval(46, 48), "86.0-98.8")
        self.assertEqual(bc.interval(15, 48), "19.9-45.3")

    def test_band_is_the_two_block_difference(self):
        # sd 1 over the base seeds, 6 against 6: 2 * sqrt(1/6 + 1/6), twice the old 2 sd / sqrt(12).
        vals = [-1.0, 1.0] * 6
        sd = bc.statistics.stdev(vals)
        self.assertAlmostEqual(bc.band(vals, 6, 6), 2 * sd * (1 / 3) ** 0.5)
        self.assertAlmostEqual(bc.band(vals, 6, 6), 2 * (2 * sd / 12 ** 0.5))
        self.assertAlmostEqual(bc.band(vals, 12, 12), 2 * sd * (1 / 6) ** 0.5)

    def test_selftest_holds_about_95_percent_of_rerolls(self):
        report, ok = bc.selftest(trials=2000, seed=7)
        self.assertTrue(ok, report)

    def test_compare_uses_the_candidate_block_size(self):
        def block(d, recycles_by_seed):
            for seed, n in recycles_by_seed.items():
                Path(d, f"sweep_s{seed}.txt").write_text(TXT)
                Path(d, f"sweep_s{seed}.err").write_text("recycle tat1: seed 1\n" * n)
        with tempfile.TemporaryDirectory() as base, tempfile.TemporaryDirectory() as other, \
                tempfile.TemporaryDirectory() as near, tempfile.TemporaryDirectory() as far:
            block(base, dict(zip(range(42, 48), [0, 2, 0, 2, 0, 2])))
            block(other, dict(zip(range(48, 54), [1, 1, 0, 2, 2, 0])))
            # Per-seed sd over the 12 base seeds is 0.953; the band for 6 against 6 is 1.10 recycles a seed.
            block(near, dict(zip(range(42, 48), [2, 2, 1, 2, 2, 2])))   # +0.83: inside the band
            block(far, dict(zip(range(42, 48), [3, 2, 2, 2, 2, 3])))    # +1.33: outside it
            text, bad = bc.compare(base, near, other, 60000)
            self.assertIn("| recycles | 1 | 1.83 | +0.833 | 1.10 | inside |", text)
            text, bad = bc.compare(base, far, other, 60000)
            self.assertIn("| recycles | 1 | 2.33 | +1.333 | 1.10 | OUTSIDE |", text)

    def test_timing_needs_the_band_clear_of_the_budget(self):
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            f.write("main 1 wall 30 s user 100.0 s\nbranch 1 wall 30 s user 101.0 s\n"
                    "main 2 wall 30 s user 104.0 s\nbranch 2 wall 30 s user 102.0 s\n")
        try:
            text, verdict = bc.timing(f.name, "main", "branch", 5.0)
            self.assertTrue(verdict.startswith("unresolved"), text)
            with open(f.name, "a") as g:
                for n in range(3, 9):
                    g.write(f"main {n} wall 30 s user 102.0 s\nbranch {n} wall 30 s user 101.5 s\n")
            text, verdict = bc.timing(f.name, "main", "branch", 5.0)
            self.assertEqual(verdict, "within budget", text)
        finally:
            os.unlink(f.name)

    def test_dc_keeps_the_census_with_its_round_and_splits_by_side(self):
        with tempfile.NamedTemporaryFile("w", suffix=".err", delete=False) as f:
            f.write(DC)
        try:
            rs = bc.load_dc(f.name)
            self.assertEqual([(r["side"], r["won"]) for r in rs], [(0, True), (1, False)])
            self.assertEqual(len(rs[0]["lines"]), 5)
            self.assertEqual(rs[1]["lines"], [])
            out = bc.dc(f.name, None, None)
        finally:
            os.unlink(f.name)
        self.assertIn("| hard | 1/2 |", out)
        # Facing = the vs-human group only; the proxy's own line is left out.
        self.assertIn("| hard | 20 | 10.00 | 25.00 | 50 | 2.00 | 0.000 |", out)


if __name__ == "__main__":
    unittest.main()
