"""python3 -m unittest discover -s scripts/tests   (absence-claim receipts)"""
import json, os, subprocess, sys, tempfile, unittest

S = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, S)
LOOPDIR = tempfile.mkdtemp(prefix="absence_test_")
os.environ["OPENBF_LOOP_DIR"] = LOOPDIR  # before the imports read it
os.environ["OPENBF_LEAD_TMP"] = ""
import re_check  # noqa: E402


class Absence(unittest.TestCase):
    def setUp(self):
        d = tempfile.mkdtemp(prefix="absence_fx_")
        self.d = d
        open(os.path.join(d, "a.c"), "w").write("/* ==== 00100000 FUN_a ==== */\nx = DAT_1;\n/* ==== 00100010 FUN_b ==== */\ny = DAT_2;\nz = DAT_1;\n")
        open(os.path.join(d, "blob.bin"), "wb").write(b"\0\0needle\0\0needle")
        re_check.ROOT = d
        re_check.CORP = {"b1": "a.c", "b2": "a.c"}
        re_check.ELF = {"b1": "blob.bin", "b2": "blob.bin"}
        re_check._cache.clear()

    def test_receipt_contents_and_file(self):
        rec = re_check.search(["DAT_1", "nothere"], srcs=["blob.bin"], quiet=True)
        self.assertEqual(rec["total_hits"], 2 * 2 + 0)  # DAT_1 twice, in each of two corpora
        rid = re_check.write_receipt(rec, "CARD1")
        f = os.path.join(LOOPDIR, "receipts", f"CARD1_{rid}.json")
        r = json.load(open(f))
        self.assertEqual((r["card"], r["terms"], r["version"]), ("CARD1", ["DAT_1", "nothere"], re_check.VERSION))
        self.assertEqual(r["sources"][0]["bytes"], 16)
        self.assertTrue(r["timestamp"])
        self.assertTrue(re_check.receipt_exists(rid))
        # raw + elf scanning
        rec = re_check.search(["needle"], elf=True, quiet=True)
        self.assertEqual([s["hits"] for s in rec["sources"]], [2, 2])

    def test_check(self):
        rec = re_check.search(["DAT_2"], quiet=True)
        rid = re_check.write_receipt(rec, "C")
        doc = os.path.join(self.d, "re-x.md")
        open(doc, "w").write(
            f"# t\nNo class authors DAT_9.\n\nAbsent from both ELFs [receipt:{rid}].\n\n"
            f"Never authored anywhere.\n  see [receipt:deadbeef]\n\n```\nnone of the classes absent\n```\nfine text\n")
        bad, rep = re_check.check_absence(doc)
        self.assertEqual(bad, 2, rep)  # line 2 (no tag) and line 6 (tag names no receipt); the fence and the cited line are fine
        self.assertIn(":2:", rep[0])
        self.assertIn("deadbeef", rep[1])

    def test_only_added_lines(self):
        doc = os.path.join(self.d, "r.md")
        open(doc, "w").write("old: no class authors it.\n")
        g = lambda *a: subprocess.run(["git", "-C", self.d, *a], capture_output=True, text=True)
        g("init", "-q", "-b", "main"); g("config", "user.email", "t@t"); g("config", "user.name", "t")
        g("add", "r.md"); g("commit", "-q", "-m", "i")
        open(doc, "a").write("new: absent everywhere.\n")
        bad, rep = re_check.check_absence(doc, "HEAD", self.d)
        self.assertEqual(bad, 1)
        self.assertIn(":2:", rep[0])  # the committed claim on line 1 is not re-checked



if __name__ == "__main__":
    unittest.main()
