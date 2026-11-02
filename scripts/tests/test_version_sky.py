"""Raw fog/range slot decoding, including the platform-specific argument counts."""
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import extract_lua
import version_sky as sky


def chunk(tag, data):
    return tag + struct.pack("<I", len(data)) + data + bytes(-len(data) % 4)


def record(name, values):
    return chunk(b"DATA", struct.pack("<IB", extract_lua.fnv1a(name), len(values))
                 + struct.pack("<" + "f" * len(values), *values) + bytes(4))


class SkyTests(unittest.TestCase):
    def test_preserves_pc_four_argument_ranges_instead_of_truncating_to_ps2(self):
        payload = chunk(b"NAME", struct.pack("<I", 0xa0975b56)) + record("SkyInfo", [])
        payload += chunk(b"SCOP", record("NearSceneRange", [20, 30, 60, 80])
                         + record("FarSceneRange", [300, 1000]) + record("FogColor", [95, 111, 126]))
        row = sky.parse(payload)
        self.assertEqual(row["name"], "hash_a0975b56")
        self.assertEqual(row["sky_info"]["near_scene_range"], [20, 30, 60, 80])
        self.assertEqual(row["sky_info"]["far_scene_range"], [300, 1000])
        self.assertNotIn("fog_range", row["sky_info"])

    def test_absent_values_are_not_zero_and_nonfinite_values_refuse(self):
        payload = chunk(b"NAME", b"sky\0") + record("SkyInfo", [])
        self.assertEqual(sky.parse(payload)["sky_info"], {})
        with self.assertRaises(ValueError):
            sky.parse(payload + chunk(b"SCOP", record("FogRange", [0, float("nan")])))

    def test_truncated_scopes_and_argument_arrays_refuse(self):
        with self.assertRaises(ValueError):
            sky.config_tree(chunk(b"SCOP", record("FogRange", [0, 100])))
        with self.assertRaises(ValueError):
            sky.config_tree(chunk(b"DATA", struct.pack("<IBf", 1, 2, 1)))


if __name__ == "__main__":
    unittest.main()
