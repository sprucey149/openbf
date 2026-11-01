"""The build's layouts: one file below about 3.9 GiB, an executable plus openbf.pak above, the same on every OS.

The end-to-end test packs a small fixture with the production `pak` code at a low split size; no game data needed.
"""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import pack


class InstallAndSummary(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        d = Path(self.dir.name)
        self.out, self.pak = str(d / "openbf"), str(d / pack.PAK_NAME)
        self.new, self.new_pak = self.out + ".new", self.pak + ".new"

    def write(self, path, data=b"x"):
        Path(path).write_bytes(data)

    def test_a_split_build_installs_both_files(self):
        self.write(self.new)
        self.write(self.new_pak)
        self.assertTrue(pack.install_pack(self.new, self.new_pak, self.out, self.pak))
        self.assertTrue(os.path.exists(self.out) and os.path.exists(self.pak))
        self.assertFalse(os.path.exists(self.new) or os.path.exists(self.new_pak))

    def test_a_one_file_build_drops_the_archive_its_old_executable_used(self):
        self.write(self.out, b"small executable with no footer" * 4)
        self.write(self.pak)
        self.write(self.new, b"x" * 32 + pack.PACK_MAGIC + b"\0" * 24)
        self.assertFalse(pack.install_pack(self.new, self.new_pak, self.out, self.pak))
        self.assertFalse(os.path.exists(self.pak))

    def test_a_one_file_build_keeps_another_builds_archive(self):
        self.write(self.out, b"x" * 32 + pack.PACK_MAGIC + b"\0" * 24)
        self.write(self.pak)
        self.write(self.new, b"y" * 32 + pack.PACK_MAGIC + b"\0" * 24)
        self.assertFalse(pack.install_pack(self.new, self.new_pak, self.out, self.pak))
        self.assertTrue(os.path.exists(self.pak))
        self.write(self.new, b"z" * 64)
        self.assertFalse(pack.install_pack(self.new, self.new_pak, str(Path(self.out).with_name("other")), self.pak))
        self.assertTrue(os.path.exists(self.pak))

    def test_the_summary_warns_about_fat32_only_when_there_are_two_files(self):
        self.write(self.out)
        self.write(self.pak)
        one = "\n".join(pack.pack_summary(self.out, self.pak, False))
        self.assertIn("nothing else is needed", one)
        self.assertNotIn("FAT32", one)
        two = "\n".join(pack.pack_summary(self.out, self.pak, True))
        self.assertIn("same folder", two)
        self.assertIn("FAT32 cannot hold a file of 4 GiB or more", two)

    def test_the_summary_names_an_archive_past_the_fat32_limit(self):
        self.write(self.out)
        with open(self.pak, "wb") as sparse:
            sparse.truncate(pack.FAT32_LIMIT)
        two = "\n".join(pack.pack_summary(self.out, self.pak, True))
        self.assertIn("openbf.pak is 4 GiB or more", two)


@unittest.skipUnless(shutil.which("cargo"), "needs cargo")
class EndToEnd(unittest.TestCase):
    """A fixture packed and read by real processes, from a folder that holds no data."""

    @classmethod
    def setUpClass(cls):
        engine = ROOT / "engine"
        subprocess.run(["cargo", "build", "-q", "-p", "pak", "--example", "pack_load"], cwd=engine, check=True)
        target = Path(os.environ.get("CARGO_TARGET_DIR") or engine / "target")
        cls.tool = target / "debug" / "examples" / ("pack_load" + pack.EXE)

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        d = Path(self.dir.name)
        self.data = d / "fixture"
        (self.data / "sub").mkdir(parents=True)
        (self.data / "sub" / "a.ron").write_text("(hello: 1)\n" * 300)
        (self.data / "b.bin").write_bytes(bytes(range(256)))
        self.game = d / "built" / ("openbf" + pack.EXE)
        self.game.parent.mkdir()
        self.elsewhere = d / "elsewhere"
        self.elsewhere.mkdir()

    def pack(self, *args):
        result = subprocess.run([self.tool, "write", self.game, *args, f"assets={self.data}"], check=True,
                                capture_output=True, text=True)
        return result.stdout.strip()

    def read(self, key):
        return subprocess.run([self.game, "read", key], cwd=self.elsewhere, capture_output=True)

    def assert_reads_the_fixture(self):
        got = self.read("assets/sub/a.ron")
        self.assertEqual(got.returncode, 0, got.stderr)
        self.assertEqual(got.stdout, (self.data / "sub" / "a.ron").read_bytes())
        self.assertEqual(self.read("assets/b.bin").stdout, bytes(range(256)))

    def test_below_the_limit_one_file_carries_everything(self):
        self.assertEqual(self.pack(), "SingleFile")
        self.assertFalse((self.game.parent / pack.PAK_NAME).exists())
        self.assertTrue(pack.carries_pack(self.game))
        self.assert_reads_the_fixture()

    def test_above_the_limit_a_small_executable_reads_the_archive_beside_it(self):
        self.assertEqual(self.pack("--split-at", "1"), "Beside")
        pak = self.game.parent / pack.PAK_NAME
        self.assertTrue(pak.exists())
        self.assertFalse(pack.carries_pack(self.game))
        self.assertEqual(self.game.stat().st_size, self.tool.stat().st_size)
        self.assert_reads_the_fixture()
        # The two travel together, and only together.
        moved = self.game.parent.parent / "moved"
        moved.mkdir()
        shutil.move(self.game, moved / self.game.name)
        self.game = moved / self.game.name
        self.assertEqual(self.read("assets/b.bin").returncode, 3)
        shutil.move(pak, moved / pack.PAK_NAME)
        self.assert_reads_the_fixture()

    def test_the_executables_own_data_wins_over_an_archive_beside_it(self):
        self.pack()
        stale = self.game.parent.parent / "stale"
        (stale / "x").mkdir(parents=True)
        (stale / "x" / "only_in_stale").write_text("stale")
        subprocess.run([self.tool, "write", stale / "unused", "--split-at", "1", "--pak",
                        self.game.parent / pack.PAK_NAME, f"assets={stale / 'x'}"], check=True, capture_output=True)
        self.assert_reads_the_fixture()
        self.assertNotEqual(self.read("assets/only_in_stale").returncode, 0)


if __name__ == "__main__":
    unittest.main()
