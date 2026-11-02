"""Focused format invariants for the portable geometry census."""

import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import version_geometry as g


def chunk(tag, p):
    return tag + struct.pack("<I", len(p)) + p + b"\0" * ((-len(p)) % 4)


class GeometryTests(unittest.TestCase):
    def test_collision_detects_shape_change_with_equal_counts_and_bounds(self):
        def mesh(mid):
            return b"".join(
                (
                    chunk(b"NAME", b"wall\0"),
                    chunk(b"NODE", b"root\0"),
                    chunk(b"INFO", struct.pack("<4I6f", 4, 0, 1, 4, 0, 0, 0, 1, 1, 1)),
                    chunk(
                        b"POSI",
                        struct.pack("<12f", 0, 0, 0, 1, 0, 0, mid, 1, 1, 0, 1, 0),
                    ),
                    chunk(
                        b"TREE",
                        chunk(
                            b"LEAF", b"\4" + b"\0" * 6 + struct.pack("<4H", 0, 1, 2, 3)
                        ),
                    ),
                )
            )

        a, b = g.collision(mesh(1))[0], g.collision(mesh(0.75))[0]
        self.assertEqual(a["fan_triangles"], 2)
        self.assertEqual(a["bounds"], b["bounds"])
        self.assertNotEqual(a["positions_sha256"], b["positions_sha256"])

    def test_surface_digest_ignores_ordering_but_sees_shape_and_winding(self):
        a, b, c, d = (0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 1.0, 0.0)
        base = g.surface_digest([a, b, c, d], [[0, 1, 2], [0, 2, 3]])
        renumbered = g.surface_digest([d, c, b, a], [[3, 0, 1], [1, 2, 3]])
        self.assertEqual(base, g.surface_digest([a, b, c, d], [[0, 2, 3], [1, 2, 0]]))
        self.assertNotEqual(base, g.surface_digest([a, b, c, d], [[2, 1, 0], [0, 2, 3]]))
        self.assertNotEqual(base, g.surface_digest([a, b, (1.0, 1.0, 0.5), d], [[0, 1, 2], [0, 2, 3]]))
        self.assertEqual(renumbered, g.surface_digest([d, c, b, a], [[1, 2, 3], [3, 0, 1]]))

    def test_renamed_collision_nodes_pair_only_within_unique_mesh_scope(self):
        key = ("a.lvl", (1,), "wall", "ps2_node", 0)
        renamed = ("a.lvl", (1,), "wall", "pc_node", 0)
        a, b, pairs = g.pair_collision_indexes({key: 1}, {renamed: 2})
        self.assertEqual((a, b, pairs), ({key: 1}, {key: 2}, [(key, renamed)]))
        wrong_scope = ("a.lvl", (2,), "wall", "pc_node", 0)
        self.assertEqual(g.pair_collision_indexes({key: 1}, {wrong_scope: 2})[2], [])
        ambiguous = ("a.lvl", (1,), "wall", "pc_other", 0)
        self.assertEqual(g.pair_collision_indexes({key: 1}, {renamed: 2, ambiguous: 3})[2], [])
        # Keep exact node matches before trying a unique remainder.
        self.assertEqual(g.pair_collision_indexes({key: 1}, {key: 2, renamed: 3})[2], [])

    def test_collision_changes_split_by_what_differs(self):
        def row(name, positions, surface, vertices, bounds=(0.0, 1.0)):
            return {"name": name, "positions_sha256": positions, "topology_sha256": "t", "surface_sha256": surface,
                    "vertex_set_sha256": vertices, "bounds": list(bounds)}

        a = {("f", "same"): row("same", "p", "s", "v"), ("f", "order"): row("order", "p", "s", "v"),
             ("f", "poly"): row("poly", "p", "s", "v"), ("f", "moved"): row("moved", "p", "s", "v")}
        b = {("f", "same"): row("same", "p", "s", "v"), ("f", "order"): row("order", "q", "s", "v"),
             ("f", "poly"): row("poly", "q", "other", "v"),
             ("f", "moved"): row("moved", "q", "other", "w", (0.0, 3.0))}
        out = g.collision_changes(a, b, sorted(a))
        self.assertEqual((out["order_only"], out["same_vertices_other_polygons"], out["different_vertices"]), (1, 1, 1))
        self.assertEqual(out["bounds_changed"], 1)
        self.assertEqual(out["largest_bounds_moves"], [["f", "moved", 2.0]])

    def test_primitive_inherits_mask_but_records_own_mask(self):
        body = chunk(b"INFO", b"crate\0" + struct.pack("<I", 2))
        for i in range(2):
            body += chunk(b"NAME", f"p{i}\0".encode())
            if i == 0:
                body += chunk(b"MASK", b"\x03")
            body += chunk(b"DATA", struct.pack("<I3f", 4, 1, 2, 3))
        rows = g.primitives(body)
        self.assertEqual([r["mask"] for r in rows], [3, 3])
        self.assertEqual([r["mask_own"] for r in rows], [1, 0])

    def test_rle_bounds_and_animation_signed_wrap(self):
        self.assertEqual(g.rle(b"\x81\x05\x00\x07", 3), b"\5\5\7")
        with self.assertRaises(g.GeometryError):
            g.rle(b"\xff\0", 16)
        self.assertEqual(
            g.stream(struct.pack("<h", 32767) + b"\x01\x80\x02\x81\0\0", 0, 5),
            [32767, -32768, -32768, -32768, 0],
        )
        with self.assertRaises(g.GeometryError):
            g.stream(b"\0\0\x80\0", 0, 2)

    def test_height_grid_patch_order_and_scale(self):
        info = struct.pack("<4f6H", 8, 0.5, 0, 10, 32, 8, 16, 9, 16, 0)
        h8 = bytes([1] * 256 + [2] * 256 + [3] * 256 + [4] * 256)
        body = chunk(b"NAME", b"map\0") + chunk(b"INFO", info)
        body += chunk(
            b"HEXP", b"".join(struct.pack("<BBhhh", 1, 0, 0, 0, 0) for _ in range(4))
        )
        body += chunk(
            b"HGT8", b"".join(b"\xff" + bytes([v]) for v in (1, 1, 2, 2, 3, 3, 4, 4))
        )
        row = g.terrain(body, retain_samples=True)[0]
        self.assertEqual(row["samples"], 1024)
        self.assertEqual(
            [
                row["_samples"][z * 32 + x]
                for x, z in [(0, 0), (16, 0), (0, 16), (16, 16)]
            ],
            [1, 2, 3, 4],
        )
        self.assertNotIn("_samples", g.terrain(body)[0])

    def test_animation_root_stream_and_duration(self):
        # A two-frame DummyRoot translation, x moves from 1 to 3 metres.
        mina = struct.pack("<QIHH", 1, 123, 2, 1)
        track = struct.pack(
            "<8I4f", g.crc32_lower("DummyRoot"), 0, 0, 0, 0, 0, 3, 3, 1, 0, 0, 2
        )
        tada = struct.pack("<h", 0) + b"\1" + struct.pack("<h", 0) + b"\0"
        data = b"SMNA" + struct.pack("<4IH2x", 0, 8, 1, len(tada), 1)
        data += chunk(b"MINA", mina) + chunk(b"TNJA", track) + chunk(b"TADA", tada)
        body = chunk(b"NAME", b"human\0") + chunk(b"BIN_", data)
        a = g.animations(body, "bf1")[0]
        b = g.animations(body, "bf2")[0]
        self.assertEqual(a["roots"][0]["delta"], [2, 0, 0])
        self.assertEqual(a["duration_seconds"], 2 / 30)
        self.assertEqual(b["duration_seconds"], 1 / 30)

    def test_unnamed_placement_reordering_does_not_invent_differences(self):
        def row(cls, x):
            return {
                "file": "a.lvl",
                "scope": [],
                "world": "map",
                "kind": "inst",
                "name": "",
                "class": cls,
                "transform": [x] * 12,
                "properties": [],
            }

        a, b = row("crate", 1), row("wall", 2)
        result = g.compare_geometry({"world": [a, b]}, {"world": [b, a]})["world"]
        self.assertEqual((result["compared"], result["changed"]), (2, 0))
        result = g.compare_geometry({"world": [a, b]}, {"world": [b, row("crate", 3)]})[
            "world"
        ]
        self.assertEqual((result["compared"], result["changed"]), (1, 0))
        self.assertEqual((len(result["left_only"]), len(result["right_only"])), (1, 1))

    def test_height_comparison_uses_world_coordinates(self):
        common = {
            "file": "map.lvl",
            "scope": [],
            "name": "map",
            "heightfield": 1,
            "grid_unit": 1,
        }
        a = dict(common, grid_size=2, _samples=[1, 2, 3, 4])
        b = dict(common, grid_size=4, _samples=[0] * 16)
        b["_samples"][5:7] = [1, 2]
        b["_samples"][9:11] = [3, 4]
        result = g.compare_height_samples([a], [b])[0]
        self.assertEqual(
            (result["compared"], result["changed"], result["right_only"]), (4, 0, 12)
        )


if __name__ == "__main__":
    unittest.main()
