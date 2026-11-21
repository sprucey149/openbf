"""Focused global-sound transaction/dependency and extraction regressions."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import global_sound_cache as cache
import pack_cache


def versions_json(global_sound=1):
    groups = dict.fromkeys(("tat1", "tat2", "maps", "common", "side", "dlc", "classes", "sounds", "global-sound",
                            "load", "beep", "extract-iso"), 1)
    groups["global-sound"] = global_sound
    return json.dumps(dict(schema=1, target="test-os", groups=groups))


VERSIONS = versions_json()


def fake_bake(path, global_sound=1, note=""):
    """A bake that answers only `format-versions`."""
    path.write_text(f"#!{sys.executable}\n# {note}\nprint({versions_json(global_sound)!r})\n")
    path.chmod(0o755)


class GlobalSoundCache(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.store = self.root / "cache"
        self.out = self.root / "assets/audio"
        self.roots = {}
        for game, names in (("bf1", ("CW", "GCW", "TEST", "NAB")), ("bf2", ("GLOBAL", "NAB"))):
            self.roots[game] = self.root / game
            sound = self.roots[game] / "sound"
            sound.mkdir(parents=True)
            for name in names:
                (sound / (name + ".LvL")).write_bytes(name.encode())
        self.tools = {"schema": 1, "binary": "fixture"}
        patch = mock.patch.object(cache, "identity", return_value=self.tools)
        patch.start()
        self.addCleanup(patch.stop)
        self.calls = []
        patch = mock.patch.object(cache.subprocess, "run", side_effect=self.bake)
        patch.start()
        self.addCleanup(patch.stop)

    def bake(self, command, **kwargs):
        self.calls.append(command)
        output = Path(command[3])
        self.assertEqual(list(output.iterdir()), [], "every bake starts empty")
        files = command[2].split(",") + command[4].split(",")
        content = [[Path(p).name, cache.digest(p)] for p in files if p]
        (output / "global.ron").write_text(json.dumps(content))
        (output / "nested").mkdir()
        (output / "nested/voice.ogg").write_bytes(b"voice")

    def build(self, game="bf1", fresh=False):
        return cache.build(game, self.roots[game], self.out / ("global_" + game), "bake", {}, self.tools, self.store, fresh)

    def test_cold_warm_and_independent_same_size_replacement(self):
        for game in self.roots:
            self.assertEqual(self.build(game)["action"], "rebuilt")
        with mock.patch.object(cache.shutil, "copytree", side_effect=AssertionError("rewrote current output")):
            for game in self.roots:
                self.assertEqual(self.build(game)["action"], "reused")
        (self.roots["bf1"] / "sound/NAB.LvL").write_bytes(b"new")
        result = self.build()
        self.assertEqual(result["action"], "rebuilt")
        self.assertEqual(result["reason"], "changed reference banks")
        self.assertEqual(self.build("bf2")["action"], "reused")
        self.assertEqual(len(self.calls), 3)

    def test_reference_add_remove_and_order(self):
        initial = self.build()["key"]
        extra = self.roots["bf1"] / "sound/AAA.lvl"
        extra.write_bytes(b"new")
        self.assertNotEqual(self.build()["key"], initial)
        self.assertEqual([Path(p).name for p in self.calls[-1][4].split(",")], ["AAA.lvl", "NAB.LvL"])
        extra.unlink()
        self.assertEqual(self.build()["key"], initial)
        self.assertEqual([Path(p).name for p in self.calls[-1][2].split(",")], ["CW.LvL", "GCW.LvL"])

    def test_excluded_references_and_unrelated_asset_do_not_invalidate(self):
        initial = self.build()["key"]
        (self.roots["bf1"] / "sound/TEST.LvL").write_bytes(b"changed")
        (self.roots["bf1"] / "other.asset").write_bytes(b"changed")
        self.assertEqual(self.build()["action"], "reused")
        self.assertEqual(self.build()["key"], initial)

    def test_absent_required_source_is_keyed_and_does_not_publish(self):
        self.build()
        (self.roots["bf1"] / "sound/CW.LvL").unlink()
        key = cache.key_for("bf1", self.roots["bf1"], self.tools)[0]
        with self.assertRaises(FileNotFoundError):
            self.build()
        self.assertFalse(cache.valid(self.store / "bf1", key)[0])

    def test_relocated_inputs_reuse_logical_recipe(self):
        self.build()
        moved = self.root / "moved"
        shutil.copytree(self.roots["bf1"], moved)
        self.roots["bf1"] = moved
        self.assertEqual(self.build()["action"], "reused")
        sound = moved / "sound"
        (sound / "NAB.LvL").rename(sound / "renamed.lvl")
        self.assertEqual(self.build()["action"], "rebuilt")

    def test_missing_damaged_added_output_and_old_cache_repair(self):
        self.build()
        entry = self.store / "bf1"
        for damage in ("missing", "content", "extra", "old", "json", "old-file"):
            with self.subTest(damage=damage):
                if damage == "missing":
                    (entry / "tree/nested/voice.ogg").unlink()
                elif damage == "content":
                    (entry / "tree/nested/voice.ogg").write_bytes(b"other")
                elif damage == "extra":
                    (entry / "tree/extra").touch()
                elif damage == "old":
                    meta = json.loads((entry / "result.json").read_text())
                    meta["schema"] = 0
                    cache.write_json(entry / "result.json", meta)
                elif damage == "old-file":
                    shutil.rmtree(entry)
                    entry.write_text("old cache format")
                else:
                    (entry / "result.json").write_text("{")
                self.assertEqual(self.build()["action"], "rebuilt")
                self.assertEqual(self.build()["action"], "reused")

    def test_materialization_repairs_stale_and_missing_asset_tree(self):
        self.build()
        output = self.out / "global_bf1"
        (output / "stale").touch()
        (output / "global.ron").unlink()
        self.assertEqual(self.build()["action"], "reused")
        self.assertEqual(cache.inventory(output), cache.inventory(self.store / "bf1/tree"))

    def test_warm_pack_recovers_after_output_rename_before_backup_removal(self):
        self.build()
        previous = self.out / "global_bf1.previous"
        previous.mkdir()
        (previous / "stale").touch()
        self.assertEqual(self.build()["action"], "reused")
        self.assertFalse(previous.exists())
        self.assertEqual(len(self.calls), 1)

    def test_staging_is_outside_the_packed_tree(self):
        self.build()
        self.assertFalse(Path(self.calls[-1][3]).is_relative_to(self.out))

    def test_fresh_never_hits_and_replaces_complete_inventory(self):
        self.build()
        (self.out / "global_bf1/stale").touch()
        self.assertEqual(self.build(fresh=True)["reason"], "explicit fresh reference")
        self.assertFalse((self.out / "global_bf1/stale").exists())
        self.assertEqual(len(self.calls), 2)

    def test_retail_toggle_and_tool_identity_invalidate(self):
        self.build()
        self.tools["restoration"] = "retail"
        self.assertEqual(self.build()["action"], "rebuilt")
        self.tools["binary"] = "new"
        self.assertEqual(self.build()["action"], "rebuilt")

    def test_failed_bake_does_not_publish(self):
        with mock.patch.object(cache.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "bake")):
            with self.assertRaises(subprocess.CalledProcessError):
                self.build()
        self.assertFalse((self.store / "bf1").exists())
        self.assertEqual(self.build()["action"], "rebuilt")

    def test_interrupted_and_failed_metadata_publication_are_repairable(self):
        for error in (OSError("disk full"), KeyboardInterrupt()):
            with self.subTest(error=type(error).__name__):
                with mock.patch.object(cache, "write_json", side_effect=error):
                    with self.assertRaises(type(error)):
                        self.build()
                self.assertFalse((self.store / "bf1").exists())
        self.assertEqual(self.build()["action"], "rebuilt")

    def test_interruption_between_directory_renames_repairs_on_retry(self):
        self.build()
        entry = self.store / "bf1"
        entry.rename(self.store / "bf1.previous")
        abandoned = self.store / "publish-interrupted"
        abandoned.mkdir()
        (abandoned / "result.json").write_text("{")
        self.assertEqual(self.build()["action"], "rebuilt")
        self.assertEqual(self.build()["action"], "reused")
        self.assertFalse((self.store / "bf1.previous").exists())

    def test_failed_output_sync_never_publishes_a_cache_result(self):
        with mock.patch.object(cache.os, "fsync", side_effect=OSError("disk full")):
            with self.assertRaises(OSError):
                self.build()
        self.assertFalse((self.store / "bf1").exists())
        self.assertEqual(self.build()["action"], "rebuilt")

    def test_stream_replacement_with_same_size_invalidates_only_its_unit(self):
        self.build()
        self.build("bf2")
        (self.roots["bf1"] / "sound/CW.LvL").write_bytes(b"XX")
        self.assertEqual(self.build()["action"], "rebuilt")
        self.assertEqual(self.build("bf2")["action"], "reused")

    def test_inputs_changed_during_bake_do_not_publish(self):
        original = self.bake
        def changed(command, **kwargs):
            original(command, **kwargs)
            (self.roots["bf1"] / "sound/NAB.LvL").write_bytes(b"new")
        with mock.patch.object(cache.subprocess, "run", side_effect=changed):
            with self.assertRaisesRegex(ValueError, "inputs changed"):
                self.build()
        self.assertFalse((self.store / "bf1").exists())

    def test_failed_replace_restores_previous_tree(self):
        old = self.root / "destination"
        old.mkdir()
        (old / "kept").touch()
        with self.assertRaises(FileNotFoundError):
            cache.replace_tree(self.root / "absent", old)
        self.assertTrue((old / "kept").is_file())

    def test_symlink_output_and_cache_payload_refused(self):
        self.build()
        output = self.out / "global_bf1"
        shutil.rmtree(output)
        output.symlink_to(self.store / "bf1/tree", target_is_directory=True)
        with self.assertRaises(ValueError):
            self.build()
        (self.store / "bf1/tree/link").symlink_to(self.root)
        self.assertFalse(cache.valid(self.store / "bf1", self.build_key())[0])

    def build_key(self):
        return cache.key_for("bf1", self.roots["bf1"], self.tools)[0]

    def test_lock_rejects_concurrent_writer(self):
        with cache.lock(self.root / "lock"):
            with self.assertRaises(RuntimeError):
                with cache.lock(self.root / "lock"):
                    pass

    def test_legacy_root_recipe_and_comma_refusal(self):
        primary = self.build_key()
        self.roots["bf1"] = self.roots["bf2"]
        self.assertNotEqual(primary, self.build_key())
        (self.roots["bf1"] / "sound/bad,name.lvl").touch()
        with self.assertRaises(ValueError):
            self.build_key()


class Extraction(unittest.TestCase):
    def test_unattested_same_size_replaced_image_and_damaged_tree(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            image, bake, output = root / "image", root / "bake", root / "extracted"
            image.write_bytes(b"old")
            bake.write_bytes(b"tool")
            output.mkdir()
            (output / "data").write_bytes(b"bad")
            calls = []
            def run(command):
                calls.append(command)
                stage = Path(command[-1])
                self.assertEqual(list(stage.iterdir()), [])
                (stage / "data").write_bytes(image.read_bytes())
            def extract():
                return pack_cache.extract_image(str(bake), "extract-iso", str(image), output, run)
            self.assertTrue(extract())
            self.assertEqual((output / "data").read_bytes(), b"old")
            self.assertFalse(extract())
            image.write_bytes(b"new")
            self.assertTrue(extract())
            self.assertEqual((output / "data").read_bytes(), b"new")
            (output / "data").write_bytes(b"bad")
            self.assertTrue(extract())
            self.assertEqual(len(calls), 3)

    def test_failed_extraction_never_attests_old_tree(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name in ("image", "bake"):
                (root / name).write_bytes(b"x")
            def run(command):
                raise OSError("interrupted")
            with self.assertRaises(OSError):
                pack_cache.extract_image(str(root / "bake"), "extract-iso", str(root / "image"), root / "tree", run)
            self.assertFalse((root / "tree.image.json").exists())


@unittest.skipUnless(shutil.which("ffmpeg") and os.name != "nt", "POSIX shell fixture; native Windows remains separate")
class ShellCompatibility(unittest.TestCase):
    def test_scratch_runner_uses_selected_legacy_roots_and_binary_with_spaces(self):
        with tempfile.TemporaryDirectory(prefix="global sound ") as folder:
            root = Path(folder)
            sandbox = root / "sandbox"
            sandbox.mkdir()
            (sandbox / "scripts").symlink_to(ROOT / "scripts", target_is_directory=True)
            (sandbox / "engine").symlink_to(ROOT / "engine", target_is_directory=True)
            for game, names in (("bf1", ("CW", "GCW", "NAB")), ("bf2", ("GLOBAL", "NAB"))):
                for location, content in (("lvl", "primary"), ("sound", "legacy")):
                    sound = sandbox / "extracted" / game / location / "DATA/_LVL_PS2/SOUND"
                    sound.mkdir(parents=True)
                    for name in names:
                        (sound / (name + ".LVL")).write_text(content)
            bake = root / "fake bake"
            bake.write_text("#!" + sys.executable + "\n" +
                            "import json, pathlib, sys\n"
                            f"if sys.argv[1] == 'format-versions': print({VERSIONS!r}); sys.exit()\n"
                            "out = pathlib.Path(sys.argv[3]); out.mkdir(parents=True, exist_ok=True)\n"
                            "paths = sys.argv[2].split(',') + sys.argv[4].split(',')\n"
                            "(out / 'global.ron').write_text(json.dumps([pathlib.Path(p).read_text() for p in paths if p]))\n")
            bake.chmod(0o755)
            output = root / "output"
            env = {k: v for k, v in os.environ.items() if not k.startswith(("OPENBF_", "PC", "XB", "BF2_"))}
            env.update(ROOT=str(sandbox), OUT=str(output), BAKE=str(bake), OPENBF_BAKE_BIN=str(bake),
                       OPENBF_INPUTS_CHOSEN="1", OPENBF_PYTHON=sys.executable)
            def run():
                done = subprocess.run(["bash", str(ROOT / "scripts/scratch_bake.sh"), "global-sound"], env=env, capture_output=True, text=True)
                self.assertEqual(done.returncode, 0, done.stderr)
                return [json.loads((output / "audio" / ("global_" + game) / "global.ron").read_text()) for game in ("bf1", "bf2")]
            self.assertEqual(run(), [["legacy"] * 3, ["legacy"] * 2])
            env.update(OPENBF_IMAGE_BF1="1", OPENBF_IMAGE_BF2="1")
            self.assertEqual(run(), [["primary"] * 3, ["primary"] * 2])
            env["OPENBF_BF1_TREE"] = str(sandbox / "extracted/bf1/sound/DATA/_LVL_PS2")
            self.assertEqual(run()[0], ["legacy"] * 3)
            self.assertFalse(any(p.suffix == ".json" for p in output.rglob('*')))


@unittest.skipUnless(os.name != "nt", "POSIX executable fixture")
class ToolIdentity(unittest.TestCase):
    def test_version_helper_ffmpeg_interpreter_and_restoration_identity(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            bake, worker, ffmpeg = root / "bake", root / "worker.py", root / "ffmpeg"
            fake_bake(bake, note="A")
            worker.write_text("# worker A\n")
            ffmpeg.write_text("#!/bin/sh\necho version-A\n")
            ffmpeg.chmod(0o755)
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ["PATH"])
            retail = cache.identity(bake, env)
            self.assertIsNone(retail["restoration"])
            with self.assertRaises(OSError):
                cache.identity(bake, dict(env, OPENBF_AUDIO_RESTORE_SCRIPT=""))
            self.assertEqual(retail["helper"], cache.digest(cache.__file__))
            env.update(OPENBF_AUDIO_RESTORE_SCRIPT=str(worker), OPENBF_PYTHON=sys.executable)
            restored = cache.identity(bake, env)
            self.assertNotEqual(restored, retail)
            self.assertEqual(restored["restoration"][1], cache.digest(sys.executable))
            worker.write_text("# worker B\n")
            self.assertNotEqual(cache.identity(bake, env), restored)
            ffmpeg.write_text("#!/bin/sh\necho version-B\n")
            self.assertNotEqual(cache.identity(bake, env)["ffmpeg"], restored["ffmpeg"])
            ffmpeg.write_text("#!/bin/sh\necho version-A\n")
            restored = cache.identity(bake, env)
            # A rebuilt bake with the same global-sound version keeps the cache; a raised version does not.
            fake_bake(bake, note="B")
            self.assertEqual(cache.identity(bake, env), restored)
            self.assertNotIn("bake", restored)
            self.assertNotIn("code", restored)
            fake_bake(bake, global_sound=2)
            self.assertEqual(cache.identity(bake, env)["version"], 2)


class PublicationGuard(unittest.TestCase):
    def test_concurrent_pack_cannot_overwrite_the_input_receipt(self):
        import pack
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder, 'openbf')
            with pack_cache.outputs_locked((out, out.parent / '.openbf-pack')), \
                 mock.patch.dict(os.environ, {}, clear=True), \
                 mock.patch.object(sys, 'argv', ['pack.py', '--scan-only', '--out', str(out)]), \
                 mock.patch.object(pack.visual_packs, 'discover', return_value=([], {})), \
                 mock.patch.object(pack.input_scan, 'scan', return_value={'refusals': []}), \
                 mock.patch.object(pack.input_scan, 'summary_lines', return_value=[]), \
                 mock.patch.object(pack.input_scan, 'write_summary') as write:
                with self.assertRaises(RuntimeError):
                    pack.main()
                write.assert_not_called()
