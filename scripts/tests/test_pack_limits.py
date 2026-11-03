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
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import pack


class MissionSelection(unittest.TestCase):
    def generation_command(self, space):
        class AtMissionGeneration(Exception):
            pass

        captured = []

        def run(command, **kwargs):
            if any(str(arg).endswith("gen_missions.py") for arg in command):
                captured.append(command)
                raise AtMissionGeneration

        games = {game: {"gameplay": {"kind": "ps2-disc", "path": "disc.iso"}, "visual": []}
                 for game in ("bf1", "bf2")}
        plan = {"games": games, "refusals": [], "dlc": {"used": False}}
        with tempfile.TemporaryDirectory() as folder, mock.patch.dict(os.environ, {}, clear=True):
            if space is not None:
                os.environ["OPENBF_BAKE_SPACE"] = space
            lua = Path(folder) / "lua"
            for game, groups in pack.LUA.items():
                for group in groups:
                    (lua / game / group).mkdir(parents=True)
                    (lua / game / group / ".complete").touch()
            with mock.patch.object(sys, "argv", ["pack.py", "--out", str(Path(folder) / "openbf")]), \
                 mock.patch.object(pack.input_scan, "scan", return_value=plan), \
                 mock.patch.object(pack.input_scan, "summary_lines", return_value=[]), \
                 mock.patch.object(pack.input_scan, "write_summary"), \
                 mock.patch.object(pack.input_scan, "discs_to_extract", return_value=[]), \
                 mock.patch.object(pack, "need"), mock.patch.object(pack, "run", side_effect=run), \
                 mock.patch.object(pack, "console_lua_root", return_value=str(lua)), \
                 mock.patch.object(pack, "git_tracked_missions", return_value=False), \
                 mock.patch.object(pack, "MISSIONS", str(Path(folder) / "missions")):
                with self.assertRaises(AtMissionGeneration):
                    pack.main()
        self.assertEqual(len(captured), 1)
        return captured[0]

    def test_explicit_space_bake_generates_space_missions(self):
        self.assertIn("--space", self.generation_command("1"))

    def test_default_and_non_one_flags_keep_ground_missions(self):
        for value in (None, "0", "true"):
            with self.subTest(value=value):
                self.assertNotIn("--space", self.generation_command(value))


class ScratchBake(unittest.TestCase):
    def test_it_still_finds_the_lines_it_rewrites_in_rebake(self):
        with tempfile.TemporaryDirectory() as d:
            env = dict(os.environ, OUT=str(Path(d) / "out"), BAKE=shutil.which("true"), OPENBF_INPUTS_CHOSEN="1")
            run = subprocess.run(["bash", str(ROOT / "scripts" / "scratch_bake.sh"), "no-such-group"], env=env,
                                 capture_output=True, text=True, timeout=120)
        self.assertNotIn("no longer has the four expected lines", run.stderr)
        self.assertIn("scratch_bake:", run.stdout)


class CmpBake(unittest.TestCase):
    def test_a_map_that_lost_its_foliage_fails_though_no_shared_file_differs(self):
        with tempfile.TemporaryDirectory() as folder:
            a, b, fake = Path(folder, "a"), Path(folder, "b"), Path(folder, "bake")
            for tree in (a, b):
                (tree / "m").mkdir(parents=True)
                (tree / "m/manifest.ron").write_text("same")
            (b / "m/foliage.bin").write_text("placed")
            fake.write_text("#!/bin/sh\nexit 0\n")
            fake.chmod(0o755)
            script = Path(__file__).resolve().parents[1] / "cmp_bake.sh"
            env = dict(os.environ, BAKE=str(fake))
            done = subprocess.run(["bash", str(script), str(a), str(b)], env=env, capture_output=True, text=True)
            self.assertEqual(done.returncode, 1, done.stdout)
            self.assertIn("FOLIAGE_LOST m", done.stdout)
            (a / "m/foliage.bin").write_text("placed")
            done = subprocess.run(["bash", str(script), str(a), str(b)], env=env, capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stdout)
            # Two trees that agree still fail when the PS2's counts say otherwise.
            fake.write_text("#!/bin/sh\nexit 1\n")
            done = subprocess.run(["bash", str(script), str(a), str(b)], env=env, capture_output=True, text=True)
            self.assertEqual(done.returncode, 1, done.stdout)


class ShadowSidecars(unittest.TestCase):
    def test_a_bake_without_any_bf1_shadow_is_called_out(self):
        with tempfile.TemporaryDirectory() as d:
            side = Path(d) / "side_bf1" / "vehicles" / "all"
            side.mkdir(parents=True)
            kept, pack.BAKED = pack.BAKED, d
            self.addCleanup(setattr, pack, "BAKED", kept)
            self.assertEqual(pack.missing_shadows(), [], "no side baked yet is not a missing shadow")
            (side / "vehicles.ron").write_text("()")
            self.assertEqual(pack.missing_shadows(), [str(side / "vehicles.ron")])
            (side / "shadows").mkdir()
            (side / "shadows" / "all_fly_xwing.ron").write_text("[]")
            self.assertEqual(pack.missing_shadows(), [])


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

    def test_a_one_file_build_ignores_an_archive_left_by_a_killed_run(self):
        self.write(self.new, b"x" * 32 + pack.PACK_MAGIC + b"\0" * 24)
        self.write(self.new_pak, b"half an archive")
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
