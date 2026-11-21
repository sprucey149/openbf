"""The application icon set: one master, every platform's container, and the check that
catches a master redrawn without regenerating the assets (branding/, scripts/make_icons.py).
"""
import io
import struct
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import make_icons


def master(size=200):
    """A deterministic master with alpha, so the test needs no committed artwork."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    px = img.load()
    for y in range(size):
        for x in range(size):
            px[x, y] = ((x * 255) // (size - 1), (y * 255) // (size - 1), 128, min(255, x * 2))
    return img


class Containers(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.files = make_icons.assets(master())

    def test_ico_header_and_the_sizes_windows_asks_for(self):
        data = self.files["branding/windows/openbf.ico"]
        self.assertEqual(struct.unpack("<HHH", data[:6]), (0, 1, len(make_icons.ICO_SIZES)))
        entries = make_icons.ico_entries(data)
        self.assertEqual([px for px, _ in entries], list(make_icons.ICO_SIZES))
        for px, body in entries:
            self.assertEqual(body[:8], b"\x89PNG\r\n\x1a\n")
            self.assertEqual(Image.open(io.BytesIO(body)).size, (px, px))
        # The directory comes first, and 256 px is stored as the byte 0.
        self.assertEqual(struct.unpack("<BBBBHHII", data[6:22])[:2], (16, 16))
        last = struct.unpack("<BBBBHHII", data[6 + 16 * (len(entries) - 1):22 + 16 * (len(entries) - 1)])
        self.assertEqual((last[0], last[1]), (0, 0))
        self.assertEqual(len(data), 6 + 16 * len(entries) + sum(len(body) for _, body in entries))

    def test_icns_container(self):
        data = self.files["branding/macos/openbf.icns"]
        self.assertEqual(data[:4], b"icns")
        entries = make_icons.icns_entries(data)
        self.assertEqual(sorted(entries), sorted(key for key, _ in make_icons.ICNS_TYPES))
        for key, px in make_icons.ICNS_TYPES:
            self.assertEqual(Image.open(io.BytesIO(entries[key])).size, (px, px))

    def test_an_icns_that_lies_about_its_length_is_not_read_as_one(self):
        data = bytearray(self.files["branding/macos/openbf.icns"])
        data[4:8] = (len(data) + 1).to_bytes(4, "big")
        with self.assertRaises(ValueError):
            make_icons.icns_entries(bytes(data))

    def test_iconset_names_and_sizes(self):
        self.assertEqual(len(make_icons.ICONSET), 10)
        for name, px in make_icons.ICONSET:
            body = self.files["branding/macos/openbf.iconset/" + name]
            self.assertEqual(Image.open(io.BytesIO(body)).size, (px, px))

    def test_hicolor_sizes(self):
        for px in make_icons.HICOLOR_SIZES:
            rel = f"branding/linux/hicolor/{px}x{px}/apps/openbf.png"
            self.assertIn(rel, self.files)
            self.assertEqual(Image.open(io.BytesIO(self.files[rel])).size, (px, px))

    def test_the_desktop_entry_names_the_game_and_the_icon(self):
        text = self.files["branding/linux/openbf.desktop.template"].decode("utf-8")
        self.assertIn("[Desktop Entry]", text)
        self.assertIn("Exec=@EXE@", text)
        self.assertIn("Icon=openbf", text)
        self.assertIn("StartupWMClass=openbf", text)
        self.assertIn("Categories=Game;", text)

    def test_the_same_master_gives_the_same_assets(self):
        self.assertEqual(make_icons.assets(master()), self.files)

    def test_the_assets_are_the_whole_set(self):
        self.assertEqual(len(self.files),
                         1 + len(make_icons.ICONSET) + 1 + len(make_icons.HICOLOR_SIZES) + 1)


class Check(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.tree = Path(self.dir.name)
        self.files = make_icons.assets(master())

    def test_a_written_tree_is_not_stale(self):
        make_icons.write(self.files, self.tree)
        self.assertEqual(make_icons.check(self.files, self.tree), [])

    def test_writing_twice_writes_the_files_once(self):
        self.assertEqual(len(make_icons.write(self.files, self.tree)), len(self.files))
        self.assertEqual(make_icons.write(self.files, self.tree), [])

    def test_a_replaced_asset_is_stale(self):
        make_icons.write(self.files, self.tree)
        stale = self.tree / "branding" / "linux" / "hicolor" / "64x64" / "apps" / "openbf.png"
        stale.write_bytes(make_icons.png_bytes(Image.new("RGBA", (64, 64), (255, 0, 0, 255))))
        notes = make_icons.check(self.files, self.tree)
        self.assertEqual(len(notes), 1)
        self.assertIn("64x64", notes[0])

    def test_a_missing_asset_is_reported(self):
        make_icons.write(self.files, self.tree)
        (self.tree / "branding" / "macos" / "openbf.icns").unlink()
        notes = make_icons.check(self.files, self.tree)
        self.assertTrue(any("openbf.icns" in note and "missing" in note for note in notes))

    def test_a_different_encoder_is_not_stale(self):
        make_icons.write(self.files, self.tree)
        path = self.tree / "branding" / "linux" / "hicolor" / "32x32" / "apps" / "openbf.png"
        img = Image.open(io.BytesIO(path.read_bytes()))
        out = io.BytesIO()
        img.save(out, "PNG", compress_level=6)  # a different encode settings, the same pixels
        path.write_bytes(out.getvalue())
        self.assertEqual(make_icons.check(self.files, self.tree), [])

    def test_the_committed_assets_match_the_master(self):
        committed = make_icons.load_master(make_icons.MASTER)
        self.assertEqual(committed.size[0], committed.size[1])
        self.assertEqual(make_icons.check(make_icons.assets(committed)), [])


if __name__ == "__main__":
    unittest.main()
