"""DLC detection accepts structural hero metadata, never names or loose strings."""
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from dlc_inputs import HERO_CLASSES, detect_dlc, detect_hero_sounds


def chunk(tag, body):
    return tag + struct.pack("<I", len(body)) + body + bytes(-len(body) % 4)


def hero_lvl(platform=b"VBUF", heroes=HERO_CLASSES):
    classes = b"".join(chunk(b"entc", chunk(b"TYPE", name)) for name in sorted(heroes))
    geom = chunk(b"modl", chunk(b"segm", chunk(platform, bytes(4))))
    return chunk(b"ucfb", chunk(b"lvl_", bytes(8) + classes + geom))


def record(name, lba, size, directory=False):
    raw = name.encode("ascii")
    data = bytearray(33 + len(raw) + (len(raw) % 2 == 0))
    data[0], data[25], data[32] = len(data), 2 if directory else 0, len(raw)
    struct.pack_into("<I", data, 2, lba)
    struct.pack_into("<I", data, 10, size)
    data[33:33 + len(raw)] = raw
    return data


def image(lvl):
    data = bytearray(24 * 2048)
    data[16 * 2048:16 * 2048 + 7] = b"\1CD001\1"
    struct.pack_into("<I", data, 16 * 2048 + 158, 20)
    struct.pack_into("<I", data, 16 * 2048 + 166, 2048)
    entry = record("RENAMED", 21, 2048, True)
    data[20 * 2048:20 * 2048 + len(entry)] = entry
    entry = record("HEROES.ANY;1", 22, len(lvl))
    data[21 * 2048:21 * 2048 + len(entry)] = entry
    data[22 * 2048:22 * 2048 + len(lvl)] = lvl
    return data


class DlcInputs(unittest.TestCase):
    def test_renamed_folder_and_file_and_read(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "unrelated-name" / "no-extension"
            path.parent.mkdir()
            data = hero_lvl()
            path.write_bytes(data)
            found = detect_dlc(directory)
            self.assertEqual(len(found), 1)
            self.assertEqual((found[0].platform, found[0].member), ("pc", None))
            self.assertEqual(found[0].read_bytes(), data)
            path.write_bytes(b"gone")
            with self.assertRaisesRegex(ValueError, "truncated"):
                found[0].read_bytes()

    def test_hero_sounds_are_the_smallest_bank_naming_them_whatever_it_is_called(self):
        wanted = struct.pack("<I", 2186927242)  # the hash the game looks hero_weap_energyBall_throw up by
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertIsNone(detect_hero_sounds(root))
            self.assertIsNone(detect_hero_sounds(root / "missing"))
            (root / "text.lvl").write_bytes(b"ucfb" + b"hero_weap_energyBall_throw")
            (root / "a").mkdir()
            (root / "a/map.lvl").write_bytes(b"ucfb" + wanted + bytes(4096))
            (root / "not_a_level.bin").write_bytes(wanted)
            self.assertEqual(detect_hero_sounds(root), root / "a/map.lvl")
            (root / "a/z.dat").write_bytes(b"ucfb" + wanted + bytes(16))
            self.assertEqual(detect_hero_sounds(root), root / "a/z.dat")
            os.mkfifo(root / "pipe")  # would wait for a writer
            self.assertEqual(detect_hero_sounds(root), root / "a/z.dat")
            self.assertEqual(len(detect_dlc(root)), 0)

    def test_absence_and_retail_classes(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(detect_dlc(directory), [])
            self.assertEqual(detect_dlc(Path(directory) / "missing"), [])
            path = Path(directory) / "dlc.lvl"
            path.write_bytes(hero_lvl(heroes={b"rep_hero_anakin\0"}))
            self.assertEqual(detect_dlc(directory), [])

    def test_loose_strings_and_one_hero_do_not_count(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dlc.lvl"
            for data in (b"".join(sorted(HERO_CLASSES)),
                         chunk(b"ucfb", chunk(b"tex_", b"".join(sorted(HERO_CLASSES)))),
                         hero_lvl(heroes={b"dlc_hero_fisto\0"})):
                path.write_bytes(data)
                self.assertEqual(detect_dlc(path), [])

    def test_unknown_and_mixed_geometry_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dlc.lvl"
            path.write_bytes(hero_lvl(b"XBOX"))
            self.assertEqual(detect_dlc(path), [])
            body = hero_lvl()[8:] + chunk(b"modl", chunk(b"segm", chunk(b"_PS2", b"")))
            path.write_bytes(chunk(b"ucfb", body))
            self.assertEqual(detect_dlc(path), [])

    def test_ps2_iso_and_member_names_are_irrelevant(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "arbitrary.dat"
            data = hero_lvl(b"_PS2")
            path.write_bytes(image(data))
            found = detect_dlc(path)
            self.assertEqual(len(found), 1)
            self.assertEqual(found[0].platform, "ps2")
            self.assertEqual(found[0].member, "RENAMED/HEROES.ANY")
            self.assertEqual(found[0].offset, 22 * 2048)
            self.assertEqual(found[0].read_bytes(), data)

    def test_truncated_and_hostile_sources(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "arbitrary.dat"
            for data in (b"", b"ucfb\xff\xff\xff\xff", hero_lvl()[:-1], image(hero_lvl())[:21 * 2048]):
                path.write_bytes(data)
                self.assertEqual(detect_dlc(path), [])
            for offset in (16 * 2048 + 166, 20 * 2048 + 2, 21 * 2048 + 10):
                data = image(hero_lvl())
                struct.pack_into("<I", data, offset, 0xffffffff)
                path.write_bytes(data)
                self.assertEqual(detect_dlc(path), [])
            # Chunk bounds are checked against their parent, not just the file.
            data = bytearray(hero_lvl())
            struct.pack_into("<I", data, 12, len(data))
            path.write_bytes(data)
            self.assertEqual(detect_dlc(path), [])


if __name__ == "__main__":
    unittest.main()
