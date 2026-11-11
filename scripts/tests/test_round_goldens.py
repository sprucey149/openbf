"""round_goldens.py verdicts and parsing; no game executable, data or rounds."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import round_goldens as rg  # noqa: E402

SPACE_OUT = "\n".join(f"state {t} {t:016x}" for t in range(1795, 9001)) + "\ntrace: 9000 ticks -> x\n"
GROUND_OUT = ("    setup tat1 combatants 16/16 tickets 250/250\n"
              "    checkpoint tat1 3000 00000000000000aa\n"
              "    checkpoint tat1 6000 00000000000000bb\n"
              "tat1       team 0 wins at tick 7406 (247 s)  tickets 4/1  alive 4/1  y 12..18  cps 6  stuck_max 180  "
              "hash 00000000000000cc\n"
              "sweep: finished 1/1  stuck_max median 180 (finished-only 180)  >10k 0 maps  sum 180\n")


class Args:
    game = "/missing/game"
    rto = "/missing/round_to_outcome"
    space_assets = None
    space_missions = None
    ground_assets = "/missing/assets"
    ground_missions = "/missing/missions"


class RoundGoldens(unittest.TestCase):
    def golden(self, kind):
        return next(g for g in rg.goldens(Args()) if g.kind == kind)

    def test_space_keeps_only_the_checkpoints(self):
        got, _ = rg.parse_space(SPACE_OUT)
        self.assertEqual(sorted(got, key=int), [str(t) for t in rg.SPACE_CHECKPOINTS])
        self.assertEqual(got["1800"], f"{1800:016x}")

    def test_ground_reads_checkpoints_end_and_result(self):
        got, result = rg.parse_ground(GROUND_OUT, "tat1")
        self.assertEqual(got, {"3000": "00000000000000aa", "6000": "00000000000000bb", "end": "00000000000000cc"})
        self.assertEqual(result, "team 0 wins at tick 7406")
        self.assertEqual(rg.parse_ground(GROUND_OUT, "bf1_nab2"), ({}, ""))

    def test_missing_inputs_fail_never_skip(self):
        a = Args()
        miss = rg.check_one(self.golden("space"), None, a, "/unused", {})
        self.assertEqual(miss["verdict"], "FAIL")
        self.assertIn("space inputs unset", miss["value"])
        miss = rg.check_one(self.golden("ground"), None, a, "/unused", {})
        self.assertEqual(miss["verdict"], "FAIL")
        self.assertIn("/missing/round_to_outcome", miss["value"])

    def test_verdicts(self):
        g = self.golden("ground")
        got, result = rg.parse_ground(GROUND_OUT, "tat1")
        run = (got, result, 0, "log")
        pin = {"fingerprint": "f", "hashes": dict(got)}
        self.assertEqual(rg.judge(g, pin, "f", "", [run], False)["verdict"], "PASS")
        self.assertIn("re-bless", rg.judge(g, pin, "other", "", [run], False)["value"])
        self.assertEqual(rg.judge(g, None, "f", "", [run], False)["verdict"], "FAIL")
        moved = dict(pin, hashes=dict(got, end="0000000000000000"))
        self.assertEqual(rg.judge(g, moved, "f", "", [run], False)["verdict"], "RERUN")
        row = rg.judge(g, moved, "f", "", [run, run], False)
        self.assertEqual(row["verdict"], "MOVED")
        self.assertIn("from tick end", row["value"])
        other = (dict(got, **{"6000": "1111111111111111"}), result, 0, "log")
        row = rg.judge(g, moved, "f", "", [run, other], False)
        self.assertEqual(row["verdict"], "FAIL")
        self.assertIn("nondeterministic", row["value"])
        self.assertEqual(rg.judge(g, None, "f", "", [({}, "", 1, "log")], False)["verdict"], "FAIL")

    def test_first_moved_is_in_tick_order(self):
        old = {"3000": "a", "12000": "b", "end": "c"}
        new = {"3000": "a", "12000": "x", "end": "y"}
        self.assertEqual(rg.first_moved(old, new), ("12000", "b", "x"))


if __name__ == "__main__":
    unittest.main()
