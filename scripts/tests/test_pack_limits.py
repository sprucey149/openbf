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
    def generation_command(self, space, bf2_kind="ps2-disc"):
        class AtMissionGeneration(Exception):
            pass

        captured = []

        def run(command, **kwargs):
            if any(str(arg).endswith("gen_missions.py") for arg in command):
                captured.append(command)
                raise AtMissionGeneration

        games = {game: {"gameplay": {"kind": "ps2-extracted", "path": "extracted"}, "visual": []}
                 for game in ("bf1", "bf2")}
        if bf2_kind == "pc-install":
            games["bf2"]["gameplay"] = {"kind": bf2_kind, "path": "pc-bf2"}
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
                 mock.patch.object(pack, "need"), mock.patch.object(pack, "need_ffmpeg"), mock.patch.object(pack, "run", side_effect=run), \
                 mock.patch.object(pack, "console_lua_root", return_value=str(lua)), \
                 mock.patch.object(pack, "git_tracked_missions", return_value=False), \
                 mock.patch.object(pack, "GAMEPLAY_TREES", {}), \
                 mock.patch.object(pack, "MISSIONS", str(Path(folder) / "missions")), \
                 mock.patch.object(pack, "BAKED", str(Path(folder) / "assets")), \
                 mock.patch.object(pack, "EXTRACTED", str(Path(folder) / "extracted")), \
                 mock.patch.object(pack, "MISSION_STAMP", str(Path(folder) / "mission.stamp")):
                with self.assertRaises(AtMissionGeneration):
                    pack.main()
        self.assertEqual(len(captured), 1)
        return captured[0]

    def test_space_missions_are_generated_by_default(self):
        for value in (None, "1", "true"):
            with self.subTest(value=value):
                self.assertIn("--space", self.generation_command(value))

    def test_space_opt_out_keeps_ground_missions(self):
        self.assertNotIn("--space", self.generation_command("0"))

    def test_mixed_pc_bf2_uses_table_only_missions_for_bf2(self):
        command = self.generation_command(None, bf2_kind="pc-install")
        self.assertIn("--gameplay-source", command)
        self.assertIn("bf2=pc-bf2", command)
        self.assertEqual(command.count("--table-only-game"), 1, command)
        self.assertEqual(command[command.index("--table-only-game") + 1], "bf2")
        self.assertNotIn("--table-only-game", self.generation_command(None))


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


class PythonVersionGate(unittest.TestCase):
    """macOS's own python3 is 3.9: pack.py must say so before a 3.10-only module fails to import."""

    def test_python_version_message_names_the_fix(self):
        self.assertIsNone(pack.python_too_old((3, 10, 0)))
        self.assertIsNone(pack.python_too_old((3, 14, 2)))
        message = pack.python_too_old((3, 9, 6), platform="darwin")
        self.assertIn("Python 3.9.6", message)
        self.assertIn("brew install python", message)
        self.assertNotIn("brew", pack.python_too_old((3, 9, 6), platform="linux"))

    def test_python_version_gate_runs_before_the_build_modules(self):
        # -I keeps scripts/ off sys.path, so any local build-module import placed before the gate fails this test
        # with ModuleNotFoundError; a 3.9-only failure after the gate is the AST test's job below.
        script = ("import runpy, sys; sys.version_info = (3, 9, 6, 'final', 0); "
                  f"runpy.run_path({str(ROOT / 'scripts' / 'pack.py')!r}, run_name='__main__')")
        result = subprocess.run([sys.executable, "-I", "-c", script, "--help"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("needs Python 3.10 or newer", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_python_version_pack_path_parses_and_imports_on_39(self):
        """pack.py and every local module it imports must at least parse as 3.9, and evaluate no `X | Y` annotation
        (a TypeError on 3.9 at import), so the gate above is what a 3.9 user sees."""
        import ast
        scripts = ROOT / "scripts"
        seen, todo, problems = set(), ["pack"], []
        while todo:
            name = todo.pop()
            if name in seen or not (scripts / f"{name}.py").is_file():
                continue
            seen.add(name)
            tree = ast.parse((scripts / f"{name}.py").read_text(encoding="utf-8"), feature_version=(3, 9))
            future = any(isinstance(n, ast.ImportFrom) and n.module == "__future__" and
                         any(a.name == "annotations" for a in n.names) for n in tree.body)
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    todo.extend(a.name for a in node.names)
                elif isinstance(node, ast.ImportFrom) and node.module and not node.level:
                    todo.append(node.module)
                annotations = []
                if isinstance(node, ast.AnnAssign):
                    annotations.append(node.annotation)
                elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    annotations.append(node.returns)
                    arguments = node.args
                    for arg in [*arguments.posonlyargs, *arguments.args, *arguments.kwonlyargs, arguments.vararg, arguments.kwarg]:
                        annotations.append(arg and arg.annotation)
                for annotation in filter(None, annotations):
                    if not future and any(isinstance(n, ast.BinOp) and isinstance(n.op, ast.BitOr) for n in ast.walk(annotation)):
                        problems.append(f"{name}.py:{annotation.lineno}")
        self.assertIn("dlc_inputs", seen)
        self.assertEqual(problems, [])


class VorbisCheck(unittest.TestCase):
    """The bake encodes with libvorbis; plain Homebrew ffmpeg has only ffmpeg's own `vorbis` encoder."""
    LIBVORBIS = " A....D libvorbis            libvorbis (codec vorbis)\n"
    NATIVE = " A..X.D vorbis               Vorbis\n"

    def problem(self, stdout, platform="linux", which="/usr/bin/ffmpeg"):
        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return mock.Mock(stdout="Encoders:\n V..... = Video\n ------\n" + stdout, returncode=0)

        problem = pack.ffmpeg_problem(which=lambda tool: which, run=run, platform=platform)
        return problem, calls

    def test_vorbis_libvorbis_is_accepted(self):
        problem, calls = self.problem(self.NATIVE + self.LIBVORBIS)
        self.assertIsNone(problem)
        self.assertEqual(calls, [["/usr/bin/ffmpeg", "-hide_banner", "-encoders"]])

    def test_vorbis_native_encoder_alone_is_refused_never_used(self):
        problem, _ = self.problem(self.NATIVE)
        self.assertIn("has no libvorbis encoder", problem)
        problem, _ = self.problem(self.NATIVE, platform="darwin")
        self.assertIn("brew install ffmpeg-full", problem)
        self.assertIn("$(brew --prefix ffmpeg-full)/bin", problem)

    def test_vorbis_decoder_line_or_name_in_text_is_not_an_encoder(self):
        problem, _ = self.problem(" V....D libvorbis_fake  video\n other   uses libvorbis somewhere\n")
        self.assertIsNotNone(problem)

    def test_vorbis_missing_ffmpeg_is_refused(self):
        problem, calls = self.problem("", platform="darwin", which=None)
        self.assertIn("not on PATH", problem)
        self.assertIn("ffmpeg-full", problem)
        self.assertEqual(calls, [])

    def test_vorbis_is_checked_before_the_first_cargo_build(self):
        plan = {"games": {}, "refusals": [], "dlc": {"used": False}}
        with tempfile.TemporaryDirectory() as folder, \
             mock.patch.object(sys, "argv", ["pack.py", "--out", str(Path(folder) / "openbf")]), \
             mock.patch.object(pack.input_scan, "scan", return_value=plan), \
             mock.patch.object(pack.input_scan, "summary_lines", return_value=[]), \
             mock.patch.object(pack.input_scan, "write_summary"), \
             mock.patch.object(pack, "ffmpeg_problem", return_value="pack: no libvorbis"), \
             mock.patch.object(pack, "run") as run, \
             mock.patch.object(pack, "EXTRACTED", str(Path(folder) / "extracted")), \
             mock.patch.object(pack, "BAKED", str(Path(folder) / "assets")), \
             mock.patch.object(pack, "MISSIONS", str(Path(folder) / "missions")):
            with self.assertRaisesRegex(SystemExit, "no libvorbis"):
                pack.main()
            run.assert_not_called()


class MacWarnings(unittest.TestCase):
    HOME = "/u/b"  # mac_warnings takes home as a parameter; a neutral path keeps the public export scrub quiet
    HEALTHY = dict(python_machine="arm64", arm64_hardware=True, translated=False, rustc_host="aarch64-apple-darwin",
                   free_bytes=200 * 10**9, ancestors=["python3", "caffeinate", "zsh"])

    def warnings(self, root="/u/b/openbf", **changes):
        return pack.mac_warnings(root, self.HOME, **dict(self.HEALTHY, **changes))

    def test_mac_warning_none_for_a_healthy_setup(self):
        self.assertEqual(self.warnings(), [])
        self.assertEqual(self.warnings(python_machine="x86_64", arm64_hardware=False,
                                       rustc_host="x86_64-apple-darwin"), [])  # an Intel Mac is not Rosetta

    def test_mac_warning_rosetta(self):
        for changes in (dict(python_machine="x86_64"), dict(translated=True)):
            with self.subTest(**changes):
                (warning,) = self.warnings(**changes)
                self.assertIn("Rosetta", warning)
        (warning,) = self.warnings(rustc_host="x86_64-apple-darwin")
        self.assertIn("rustup", warning)

    def test_mac_warning_synced_or_external_folder(self):
        for root in ("/u/b/Desktop/openbf", "/u/b/Documents/games/openbf",
                     "/u/b/Library/Mobile Documents/com~apple~CloudDocs/openbf"):
            with self.subTest(root=root):
                (warning,) = self.warnings(root)
                self.assertIn("iCloud", warning)
        (warning,) = self.warnings(os.path.join(os.sep, "Volumes", "Backup", "openbf"))
        self.assertIn("external", warning)
        self.assertEqual(self.warnings("/u/b/Desktopper/openbf"), [])

    def test_mac_warning_space_and_caffeinate(self):
        (warning,) = self.warnings(free_bytes=12 * 10**9)
        self.assertIn("12 GB free", warning)
        self.assertEqual(self.warnings(free_bytes=None), [])
        (warning,) = self.warnings(ancestors=["python3", "zsh", "login"])
        self.assertIn("caffeinate -i python3 scripts/pack.py", warning)

    def test_mac_warning_never_refuses_and_stays_off_elsewhere(self):
        everything = dict(python_machine="x86_64", translated=True, rustc_host="x86_64-apple-darwin",
                          free_bytes=1, ancestors=[])
        self.assertEqual(len(self.warnings("/u/b/Desktop/openbf", **everything)), 5)
        with mock.patch.object(pack.sys, "platform", "linux"), mock.patch.object(pack, "mac_facts") as facts:
            pack.warn_mac()
            facts.assert_not_called()
        with mock.patch.object(pack.sys, "platform", "darwin"), \
             mock.patch.object(pack, "mac_facts", return_value=dict(self.HEALTHY, free_bytes=1)), \
             mock.patch("builtins.print") as printed:
            pack.warn_mac()
        self.assertIn("macOS warning", printed.call_args.args[0])


class StepTimes(unittest.TestCase):
    def test_step_times_clock_and_totals(self):
        self.assertIsNone(pack.StepClock().totals())
        ticks = iter([100.0, 130.5, 3830.0])
        clock = pack.StepClock(now=lambda: next(ticks))
        self.assertEqual(clock.begin("1/6 build bake + game (release)"), 0)
        self.assertEqual(pack.clock(clock.begin("2/6 the discs")), "00:30")
        self.assertEqual(clock.totals(), "step times: 1/6 00:30, 2/6 61:39; total 62:10")
        self.assertEqual(pack.clock(5712), "95:12")

    def test_step_times_printed_when_a_step_fails(self):
        printed = []
        plan = {"games": {}, "refusals": [], "dlc": {"used": False}}
        with tempfile.TemporaryDirectory() as folder, \
             mock.patch.object(sys, "argv", ["pack.py", "--out", str(Path(folder) / "openbf")]), \
             mock.patch.object(pack.input_scan, "scan", return_value=plan), \
             mock.patch.object(pack.input_scan, "summary_lines", return_value=[]), \
             mock.patch.object(pack.input_scan, "write_summary"), \
             mock.patch.object(pack, "need_ffmpeg"), mock.patch.object(pack, "warn_mac"), mock.patch.object(pack, "need"), \
             mock.patch.object(pack, "run", side_effect=SystemExit("pack: failed (101): cargo build")), \
             mock.patch.object(pack, "EXTRACTED", str(Path(folder) / "extracted")), \
             mock.patch.object(pack, "BAKED", str(Path(folder) / "assets")), \
             mock.patch.object(pack, "MISSIONS", str(Path(folder) / "missions")), \
             mock.patch("builtins.print", side_effect=lambda *a, **k: printed.append(" ".join(map(str, a)))):
            with self.assertRaisesRegex(SystemExit, "cargo build"):
                pack.main()
        self.assertTrue(any(line.startswith("\n== [00:00] 1/6 build") for line in printed), printed)
        self.assertTrue(printed[-1].startswith("\nstep times: 1/6 "), printed)


if __name__ == "__main__":
    unittest.main()
