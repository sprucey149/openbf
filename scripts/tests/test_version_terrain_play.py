"""The playable-ground classifier: its height query, its frame and each of its exclusions."""

import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import version_terrain_play as play


def row(n=16, unit=8.0, heights=None):
    return {"grid_size": n, "grid_unit": unit, "_samples": list(heights if heights is not None else [0.0] * (n * n))}


def glb(low, high):
    doc = {"meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
           "accessors": [{"min": low, "max": high}]}
    body = json.dumps(doc).encode()
    body += b" " * (-len(body) % 4)
    return struct.pack("<4sII", b"glTF", 2, 20 + len(body)) + struct.pack("<I4s", len(body), b"JSON") + body


class RonTests(unittest.TestCase):
    def test_reads_what_the_bake_writes(self):
        text = '''(
            map: "tat1", // a comment
            terrain_name: Some("tat1"), sky: None,
            nodes: [((1.5, -2.0, 3e2), (0.0, -0.0, 1.0, -0.0))],
            props: [(2734521612, "1")], flag: true,
            water: Some((level: -1.0)),
        )'''
        value = play.parse_ron(text)
        self.assertEqual(value["map"], "tat1")
        self.assertEqual(value["terrain_name"], "tat1")
        self.assertIsNone(value["sky"])
        self.assertEqual(value["nodes"][0][0], [1.5, -2.0, 300.0])
        self.assertEqual(int(value["props"][0][0]), 2734521612)
        self.assertIs(value["flag"], True)
        self.assertEqual(value["water"], {"level": -1.0})


    def test_non_finite_floats_stay_floats(self):
        value = play.parse_ron("(a: inf, b: -inf, c: NaN, infinite: true, NaNny: 1.0)")
        self.assertEqual(value["a"], float("inf"))
        self.assertEqual(value["b"], float("-inf"))
        self.assertNotEqual(value["c"], value["c"])
        self.assertIs(value["infinite"], True)
        self.assertEqual(value["NaNny"], 1.0)


class GridTests(unittest.TestCase):
    def test_world_frame_has_the_one_cell_z_offset(self):
        grid = play.Grid(row())
        self.assertEqual(grid.world(0, 0), (-64.0, -56.0))
        self.assertEqual(grid.world(8, 7), (0.0, 0.0))
        self.assertTrue(grid.covers(0.0, 0.0))
        self.assertFalse(grid.covers(-65.0, 0.0))

    def test_height_query_takes_the_triangle_of_the_row_parity(self):
        # A single raised corner at (ix=1, iz=1): which cell half sees it depends on the row's diagonal.
        n, heights = 16, [0.0] * 256
        heights[1 * n + 1] = 8.0
        grid = play.Grid(row(heights=heights))
        x0, z0 = grid.world(0, 0)
        # Row 0 is even: its diagonal runs (0,0)-(1,1), so the raised corner reaches both halves of that cell.
        self.assertAlmostEqual(grid.height(x0 + 4.0, z0 + 2.0), 2.0)
        self.assertAlmostEqual(grid.height(x0 + 2.0, z0 + 4.0), 2.0)
        # Row 1 is odd: its diagonal runs (1,1)-(0,2), and the far half of cell (1,1) still holds the corner.
        x1, z1 = grid.world(1, 1)
        self.assertAlmostEqual(grid.height(x1, z1), 8.0)
        self.assertAlmostEqual(grid.height(x1 + 4.0, z1 + 4.0), 0.0)
        self.assertAlmostEqual(grid.height(x1 + 2.0, z1 + 2.0), 4.0)

    def test_cutter_needs_every_plane_negative_and_the_sphere_only_rejects(self):
        box = [[1, 0, 0, -1], [-1, 0, 0, -1], [0, 1, 0, -1], [0, -1, 0, -1], [0, 0, 1, -1], [0, 0, -1, -1]]
        self.assertTrue(play.is_cut([([0, 0, 0, 0], box)], 0.0, 0.0, 0.0))
        self.assertFalse(play.is_cut([([0, 0, 0, 0], box)], 2.0, 0.0, 0.0))
        self.assertFalse(play.is_cut([([0.5, 9.0, 9.0, 9.0], box)], 0.0, 0.0, 0.0))

    def test_a_vertex_on_a_patch_edge_is_painted_by_either_side(self):
        masks = [0] * 4
        masks[1] = 1  # second render patch of the first row
        self.assertTrue(play.painted(masks, 16, 8, 0))
        self.assertTrue(play.painted(masks, 16, 12, 2))
        self.assertFalse(play.painted(masks, 16, 2, 2))


class CoverTests(unittest.TestCase):
    def test_rotation_uses_the_columns_of_instance_matrix(self):
        cover = play.Cover({}, Path("."))
        # Same column-major 90-degree yaw as game::math::instance_matrix.
        # Local x=2..4,z=0..1 lands at world x=10..11,z=16..18.
        cover.place({"rotation": [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
                     "translation": [10, 0, 20]}, [2, -1, 0], [4, 1, 1])
        self.assertTrue(cover.covers(10.5, 17, 0))
        self.assertFalse(cover.covers(9.5, 23, 0))


class AnalyseTests(unittest.TestCase):
    def manifest(self, cutters=()):
        grid = play.Grid(row(n=32))
        x, z = grid.world(16, 16)
        return {
            "map": "test", "source_game": "bf1",
            "terrain_cutters": list(cutters),
            "paths": [{"name": "boundary", "nodes": [([x - 40, 0.0, z - 40], [1, 0, 0, 0]), ([x + 40, 0.0, z - 40], [1, 0, 0, 0]),
                                                     ([x + 40, 0.0, z + 40], [1, 0, 0, 0]), ([x - 40, 0.0, z + 40], [1, 0, 0, 0])]},
                      {"name": "CP1SpawnPath", "nodes": [([x, 0.0, z], [1, 0, 0, 0])]}],
            "instances": [], "regions": [], "classes": [], "collision": [],
            "nav": {"plan": {"hubs": [{"pos": [x, 0.0, z], "radius": 8.0, "name": "h", "arcs": []}], "arcs": []}},
        }, grid

    def run_case(self, change, manifest, masks=None):
        n = 32
        left = [0.0] * (n * n)
        right = list(left)
        for ix, iz, delta in change:
            right[iz * n + ix] = delta
        with tempfile.TemporaryDirectory() as folder:
            return play.analyse("t.lvl", (row(n=n, heights=left), masks or [1] * 16), (row(n=n, heights=right), None),
                                manifest, Path(folder))

    def test_each_exclusion(self):
        manifest, grid = self.manifest()
        report = self.run_case([(16, 16, 3.0), (17, 16, 0.5), (1, 1, 20.0)], manifest)
        self.assertEqual(report["total_changed"], 3)
        self.assertEqual(report["classes"]["on_nav"]["samples"], 2)
        self.assertEqual(report["outside_boundary"]["samples"], 1)
        self.assertEqual(report["outside_boundary"]["max_abs_metres"], 20.0)
        self.assertEqual(report["in_play_exposed"]["samples"], 2)
        self.assertEqual(report["in_play_exposed"]["max_abs_metres"], 3.0)

    def test_undrawn_ground_is_not_in_play(self):
        manifest, _ = self.manifest()
        report = self.run_case([(16, 16, 3.0)], manifest, masks=[0] * 16)
        self.assertEqual(report["classes"]["unpainted"]["samples"], 1)
        self.assertEqual(report["in_play_exposed"]["samples"], 0)

    def test_a_hole_in_both_versions_does_not_count_and_a_hole_in_one_does(self):
        manifest, grid = self.manifest()
        x, z = grid.world(16, 16)
        # Inside for y below 2: the reference (0 m) always; the other only when its change stays under 2.
        box = {"sphere": [0, 0, 0, 0], "planes": [[1, 0, 0, -x - 1], [-1, 0, 0, x - 1], [0, 1, 0, -2], [0, -1, 0, -5],
                                                   [0, 0, 1, -z - 1], [0, 0, -1, z - 1]]}
        manifest["terrain_cutters"] = [box]
        both = self.run_case([(16, 16, 1.0)], manifest)
        self.assertEqual(both["classes"]["cut_both"]["samples"], 1)
        self.assertEqual(both["in_play_exposed"]["samples"], 0)
        one = self.run_case([(16, 16, 3.0)], manifest)
        self.assertEqual(one["classes"]["cut_one"]["samples"], 1)
        self.assertEqual(one["in_play_exposed"]["samples"], 1)

    def test_a_placed_object_hides_the_ground_it_stands_on(self):
        manifest, grid = self.manifest()
        x, z = grid.world(16, 16)
        manifest["classes"] = [{"name": "bunker", "geometry": "bunker_mesh"}]
        manifest["collision"] = [{"geometry": "bunker_mesh", "glb": "collision/b.glb", "prims": []}]
        manifest["instances"] = [{"class": "Bunker", "name": "b", "props": [],
                                  "rotation": [[1, 0, 0], [0, 1, 0], [0, 0, 1]], "translation": [x, 0.0, z]}]
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / "collision").mkdir()
            (Path(folder) / "collision/b.glb").write_bytes(glb([-4.0, -1.0, -4.0], [4.0, 5.0, 4.0]))
            n = 32
            left = [0.0] * (n * n)
            right = list(left)
            right[16 * n + 16] = 3.0
            report = play.analyse("t.lvl", (row(n=n, heights=left), [1] * 16), (row(n=n, heights=right), None),
                                  manifest, Path(folder))
        self.assertEqual(report["classes"]["on_nav"]["under_object"], 1)
        self.assertEqual(report["in_play_exposed"]["samples"], 0)

    def test_arcs_and_points_report_the_query_not_the_vertices(self):
        manifest, grid = self.manifest()
        x, z = grid.world(16, 16)
        hubs = manifest["nav"]["plan"]["hubs"]
        hubs.append({"pos": [x + 16.0, 0.0, z], "radius": 8.0, "name": "h2", "arcs": []})
        manifest["nav"]["plan"]["arcs"] = [{"a": 0, "b": 1, "type_mask": 1, "name": "a", "flags": 0},
                                           {"a": 0, "b": 1, "type_mask": 32, "name": "fly", "flags": 0}]
        report = self.run_case([(17, 16, 2.0)], manifest)
        lines = report["play_lines"]
        self.assertEqual(lines["soldier_arcs"]["arcs"], 1)
        self.assertEqual(lines["soldier_arcs"]["above_1m"], 1)
        self.assertEqual(lines["soldier_arcs"]["exposed"]["above_1m"], 1)
        self.assertEqual(lines["vehicle_only_arcs"]["arcs"], 0)
        self.assertEqual(report["instrument"]["hubs_inside_boundary"], 2)

    def test_output_holds_no_grid(self):
        manifest, _ = self.manifest()
        report = self.run_case([(16, 16, 3.0)], manifest)
        self.assertNotIn("_samples", json.dumps(report))
        self.assertLess(len(json.dumps(report)), 8000)


if __name__ == "__main__":
    unittest.main()
