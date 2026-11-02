"""The committed receipt stays small and name-only, and the doc quotes it exactly."""

import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import version_report as report

RECEIPT = ROOT / "scripts/data/version-diff/measurements.json"
DOC = ROOT / "analysis/version-diff.md"


class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.receipt = json.loads(RECEIPT.read_text())

    def test_doc_tables_are_exactly_the_receipt(self):
        self.assertEqual(DOC.read_text(), report.render_doc(DOC.read_text(), self.receipt))

    def test_doc_links_resolve(self):
        import re

        for link in re.findall(r"\]\((\.\./[^)#]+|[a-z-]+\.md)\)", DOC.read_text()):
            self.assertTrue((DOC.parent / link).exists(), link)

    def test_tool_hashes_are_the_tools_in_this_tree(self):
        import hashlib

        # gen_missions.py and extract_lua.py belong to the bake and change with it. The receipt records which ones it
        # used; failing here when another branch touches them would say nothing about these tools.
        recorded_only = {"version_report.py", "gen_missions.py", "extract_lua.py"}
        for name, digest in self.receipt["tool_sha256"].items():
            if name in recorded_only:
                continue
            self.assertEqual(hashlib.sha256((ROOT / "scripts" / name).read_bytes()).hexdigest(), digest, name)

    def test_a_terrain_class_table_adds_up(self):
        for terrains in self.receipt["terrain_play"].values():
            for terrain in terrains:
                total = sum(c["samples"] for c in terrain["classes"].values())
                self.assertEqual(total, terrain["total_changed"], terrain["file"])
                self.assertLessEqual(terrain["in_play_exposed"]["samples"] + terrain["outside_boundary"]["samples"], total)

    def test_receipt_has_no_lists_of_content(self):
        self.assertLess(RECEIPT.stat().st_size, 256 << 10)
        pair = self.receipt["comparisons"]["bf2_ps2_vs_bf2_xbox"]
        self.assertNotIn("scoped_unmatched_names", pair)
        self.assertNotIn("left_only", pair["content_name_sets"])
        self.assertNotIn("left_only", pair["geometry"]["collision"])

    def test_the_receipt_records_the_name_registry_it_used(self):
        for source in self.receipt["sources"].values():
            self.assertRegex(source["animation_registry_sha256"], "^[0-9a-f]{64}$")

    def test_unknown_and_missing_blocks_refuse(self):
        with self.assertRaises(ValueError):
            report.render_doc("<!-- generated:nope -->x<!-- /generated -->", self.receipt)
        with self.assertRaises(ValueError):
            report.render_doc("no blocks at all", self.receipt)


class GateTests(unittest.TestCase):
    @staticmethod
    def source(*, terrain=("a/a1.lvl", "b/b1.lvl"), meshes=(), routes=()):
        return {"geometry": {
            "terrain": [{"file": f, "scope": [], "name": f, "heights_sha256": "h", "cutters_sha256": "c"} for f in terrain],
            "collision": [{"file": f, "scope": [], "name": n, "node": n, "surface_sha256": s} for f, n, s in meshes],
            "routes": [{"file": f, "scope": [], "name": "path", "sha256": s} for f, s in routes],
        }}

    def test_a_map_the_other_edition_has_no_terrain_for_fails(self):
        gate = report.geometry_gate(self.source(), self.source(terrain=("a/a1.lvl",)))
        self.assertEqual(gate["same_shapes"], ["a/a1.lvl"])
        self.assertEqual(gate["other_ground"], ["b/b1.lvl"])

    def test_shared_lvls_are_reported_and_do_not_pass_silently(self):
        left = self.source(meshes=(("side/all.lvl", "gun", "x"), ("side/all.lvl", "turret", "x"), ("spa/sky.lvl", "sky", "x"),
                                   ("ingame.lvl", "icon", "x")))
        right = self.source(meshes=(("side/all.lvl", "gun", "y"), ("spa/sky.lvl", "sky", "y")))
        gate = report.geometry_gate(left, right)
        self.assertEqual(gate["side_meshes_with_other_surfaces"], {"side/all.lvl": 1})
        self.assertEqual(gate["side_meshes_missing"], {"side/all.lvl": 1})
        self.assertEqual(gate["other_shared_meshes_with_other_surfaces"], {"spa/sky.lvl": 1})
        self.assertEqual(gate["other_shared_meshes_missing"], {"ingame.lvl": 1})

    def test_a_passing_map_with_other_routes_is_named(self):
        gate = report.geometry_gate(self.source(routes=(("a/a1.lvl", "x"), ("b/b1.lvl", "x"))),
                                    self.source(routes=(("a/a1.lvl", "y"), ("b/b1.lvl", "x"))))
        self.assertEqual(gate["same_shapes"], ["a/a1.lvl", "b/b1.lvl"])
        self.assertEqual(gate["same_shapes_other_routes"], ["a/a1.lvl"])


if __name__ == "__main__":
    unittest.main()
