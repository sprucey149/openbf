"""Per-group bake keys (bake_cache.py), their landing guard (bake_versions.py) and the disc extraction receipt."""
import contextlib
import io
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
import bake_cache
import bake_versions
import pack
import pack_cache


def versions(**bumped):
    groups = dict.fromkeys(bake_cache.GROUPS + ("extract-iso",), 1)
    groups.update({name.replace("_", "-"): number for name, number in bumped.items()})
    return dict(schema=1, target="x86_64-test", groups=groups)


INPUTS = dict(gameplay={"bf1": "disc one", "bf2": "disc two", "pc_only": False}, pc={}, xbox={}, space=True,
              maps={}, menu_font=None, sound_trees={}, dlc=[None, None], dlc_scripts=["a", "b"], lua="lua")
AUDIO = dict(ffmpeg=["ffmpeg digest", "ffmpeg version 7.1"], restoration=["restore digest", "3.14"])


def concrete(pattern):
    return pattern.replace("bf[12]_*", "bf1_map").replace("*", "out")


def write_outputs(baked, group):
    for pattern in bake_cache.OUTPUTS.get(group, ()):
        path = Path(baked, concrete(pattern))
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.suffix:
            path.write_text(group)
        else:
            path.mkdir(exist_ok=True)


class GroupKeys(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.baked = self.root / "assets_baked"
        self.baked.mkdir()
        self.store = bake_cache.KeyStore(self.baked)
        self.calls = []

    def keys(self, vers=None, inputs=None, audio=None):
        return bake_cache.group_keys(vers or versions(), inputs or INPUTS, audio or AUDIO)

    def bake(self, keys, rebake=False, fail_at=None):
        rows = bake_cache.plan(self.store, keys, rebake=rebake)

        def run(group):
            if group == fail_at:
                raise KeyboardInterrupt
            self.calls.append(group)
            write_outputs(self.baked, group)
        start = len(self.calls)
        bake_cache.run_groups(self.store, keys, rows, run)
        return self.calls[start:], rows

    def test_warm_second_pack_bakes_nothing(self):  # case 1
        done, _ = self.bake(self.keys())
        self.assertEqual(done, [g for g in bake_cache.ORDER if g != "dlc"], "side's run makes the DLC heroes")
        done, rows = self.bake(self.keys())
        self.assertEqual(done, [])
        self.assertTrue(all(action == "hit" for _, action, _ in rows))

    def test_one_version_bump_bakes_that_group_and_what_reads_it(self):  # case 2
        self.bake(self.keys())
        done, rows = self.bake(self.keys(versions(sounds=2)))
        self.assertEqual(done, ["sounds", "catalog"])
        self.assertIn(("sounds", "bake", "version 1 -> 2"), rows)
        self.assertIn("baked again: version 1 -> 2", "\n".join(bake_cache.explain_lines(rows)))
        # sound-names reads the baked classes: their new version reaches sounds too, nothing else.
        done, rows = self.bake(self.keys(versions(sounds=2, classes=2)))
        self.assertEqual(done, ["classes", "sounds", "catalog"])
        self.assertIn(("sounds", "bake", "changed the classes it reads"), rows)

    def test_changed_input_bakes_only_the_groups_that_read_it(self):  # case 3
        self.bake(self.keys())
        cases = {"menu_font": ("abc", ["common"]), "dlc": (["dlc tree", None], ["dlc", "sounds"]),
                 "pc": ({"bf2": "install"}, ["tat1", "tat2", "common", "side", "sounds", "maps"]),
                 "lua": ("other lua", ["sounds"]), "space": (False, ["sounds", "maps"])}
        for name, (value, groups) in cases.items():
            with self.subTest(input=name):
                done, _ = self.bake(self.keys(inputs=dict(INPUTS, **{name: value})))
                self.assertEqual(done, groups + ["catalog"])
                self.bake(self.keys())  # back to the reference inputs
        done, _ = self.bake(self.keys(inputs=dict(INPUTS, gameplay=dict(INPUTS["gameplay"], bf1="other disc"))))
        self.assertEqual(sorted(done), sorted(g for g in bake_cache.ORDER if g != "dlc"))

    def test_a_pack_runs_every_rebake_group(self):
        script = Path(bake_cache.__file__).with_name("rebake.sh").read_text(encoding="utf-8")
        line = next(row for row in script.splitlines() if row.startswith("REBAKE_GROUPS="))
        self.assertEqual(sorted(line.split('"')[1].split()), sorted(bake_cache.GROUPS))

    def test_a_bake_without_a_catalog_bakes_only_the_catalog(self):
        self.bake(self.keys())
        self.store.drop("catalog")
        shutil.rmtree(self.baked / "catalog")
        done, _ = self.bake(self.keys())
        self.assertEqual(done, ["catalog"])

    def test_the_catalog_is_written_from_an_empty_folder(self):
        self.bake(self.keys())
        (self.baked / "catalog/bf2_pc.ron").write_text("a PC copy since removed")
        self.bake(self.keys(versions(maps=2)))
        self.assertFalse((self.baked / "catalog/bf2_pc.ron").exists())

    def test_a_run_cut_off_before_the_catalog_leaves_it_stale(self):
        keys = self.keys()
        self.bake(keys)
        rows = bake_cache.plan(self.store, keys, {"side": "no BF1 vehicle shadows"})
        self.assertIn(("catalog", "bake", "it lists what the other groups wrote"), rows)

        def run(group):
            raise KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt):
            bake_cache.run_groups(self.store, keys, rows, run)
        self.assertIsNone(self.store.read("catalog"))

    def test_keys_never_hold_bake_source_or_binary(self):  # case 4
        keys = self.keys()
        text = json.dumps(keys)
        for word in ("engine/crates", ".rs", "bake binary"):
            self.assertNotIn(word, text)
        self.assertEqual(keys, self.keys())

    def test_ffmpeg_or_restoration_change_bakes_only_the_audio_groups(self):  # case 5
        self.bake(self.keys())
        done, rows = self.bake(self.keys(audio=dict(AUDIO, ffmpeg=["other", "ffmpeg version 8.0"])))
        self.assertEqual(done, ["common", "dlc", "sounds", "beep", "catalog"])
        self.assertIn(("beep", "bake", "changed ffmpeg"), rows)
        self.bake(self.keys())
        done, _ = self.bake(self.keys(audio=dict(AUDIO, restoration=None)))
        self.assertEqual(done, ["common", "dlc", "sounds", "beep", "catalog"])

    def test_moved_input_folder_with_the_same_content_is_all_hits(self):  # case 6
        first = self.root / "iso" / "BF2 PC"
        (first / "SIDE").mkdir(parents=True)
        (first / "SIDE/ALL.LVL").write_bytes(b"side")
        (first / "CORE.LVL").write_bytes(b"core")
        memo = bake_cache.DigestMemo(self.root / "memo.json")
        before = memo.tree(first)
        moved = self.root / "elsewhere" / "pc"
        moved.parent.mkdir()
        shutil.move(str(first), moved)
        self.assertEqual(memo.tree(moved), before)
        self.bake(self.keys(inputs=dict(INPUTS, pc={"bf2": before})))
        done, _ = self.bake(self.keys(inputs=dict(INPUTS, pc={"bf2": memo.tree(moved)})))
        self.assertEqual(done, [])
        (moved / "CORE.LVL").write_bytes(b"CORE")
        self.assertNotEqual(memo.tree(moved), before)

    def test_digest_memo_reads_an_unchanged_file_once(self):
        path = self.root / "image.iso"
        path.write_bytes(b"disc")
        memo = bake_cache.DigestMemo(self.root / "memo.json")
        first = memo.digest(path)
        memo.save()
        again = bake_cache.DigestMemo(self.root / "memo.json")
        with mock.patch.object(bake_cache.hashlib, "sha256", side_effect=AssertionError("read again")):
            self.assertEqual(again.digest(path), first)
        path.write_bytes(b"DISC")
        self.assertNotEqual(again.digest(path), first)
        self.assertNotEqual(bake_cache.DigestMemo(self.root / "memo.json", fresh=True).digest(path), first)

    def test_two_output_folders_keep_their_own_keys(self):  # case 7
        other = bake_cache.KeyStore(self.root / "profile" / "assets_baked")
        (self.root / "profile" / "assets_baked").mkdir(parents=True)
        self.bake(self.keys())
        rows = bake_cache.plan(other, self.keys())
        self.assertTrue(all(action != "hit" for _, action, _ in rows))
        self.assertNotEqual(other.folder, self.store.folder)

    def test_killed_key_write_leaves_no_partial_key(self):  # case 8
        keys = self.keys()
        with mock.patch.object(bake_cache.os, "replace", side_effect=OSError("killed")):
            with self.assertRaises(OSError):
                self.store.write("maps", keys["maps"])
        self.assertIsNone(self.store.read("maps"))
        self.store.write("maps", keys["maps"])
        (self.store.folder / "maps.json.new").write_text("{half")
        self.assertEqual(self.store.read("maps")["key"], keys["maps"]["key"])

    def test_killed_run_resumes_after_the_groups_that_finished(self):  # case 13
        keys = self.keys()
        with self.assertRaises(KeyboardInterrupt):
            self.bake(keys, fail_at="classes")
        self.assertEqual(self.calls, ["tat1", "tat2", "common", "side"])
        self.assertIsNone(self.store.read("classes"), "a group cut off has no key")
        done, rows = self.bake(keys)
        self.assertEqual(done, ["classes", "sounds", "load", "maps", "beep", "catalog"])
        self.assertEqual([g for g, action, _ in rows if action == "hit"], ["tat1", "tat2", "common", "side", "dlc"])

    def test_key_with_missing_output_or_forced_reason_bakes_again(self):
        self.bake(self.keys())
        shutil.rmtree(self.baked / "core_bf2")
        rows = bake_cache.plan(self.store, self.keys(), {"side": "no BF1 vehicle shadows"})
        self.assertIn(("sounds", "bake", "its output is missing"), rows)
        self.assertIn(("side", "bake", "no BF1 vehicle shadows"), rows)
        self.assertIn(("dlc", "with side", "baked again by side"), rows)

    def test_sound_names_follow_the_dlc_classes(self):
        # sound-names reads classes/bf2_dlc.ron, which the dlc group writes: a DLC change reaches sounds.
        self.bake(self.keys())
        done, rows = self.bake(self.keys(versions(dlc=2)))
        self.assertEqual(done, ["dlc", "sounds", "catalog"])
        self.assertIn(("sounds", "bake", "changed the DLC classes it reads"), rows)

    def test_a_group_baked_over_a_matching_key_drops_it_first(self):
        # A forced or output-missing group still holds a matching key; a run cut off inside it must not leave that key
        # (nor its carried dlc key) over half-written output.
        keys = self.keys()
        self.bake(keys)
        self.assertIsNotNone(self.store.read("side"))
        rows = bake_cache.plan(self.store, keys, {"side": "no BF1 vehicle shadows"})

        def run(group):
            raise KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt):
            bake_cache.run_groups(self.store, keys, rows, run)
        self.assertIsNone(self.store.read("side"))
        self.assertIsNone(self.store.read("dlc"))
        self.assertIsNotNone(self.store.read("tat1"), "groups not run keep their keys")

    def test_keys_sit_beside_the_resolved_output(self):  # case 15 (part)
        real = self.root / "main" / "assets_baked"
        real.mkdir(parents=True)
        link = self.root / "worktree" / "assets_baked"
        link.parent.mkdir()
        link.symlink_to(real, target_is_directory=True)
        self.assertEqual(bake_cache.keys_folder(link), self.root / "main" / "assets_baked.groups")


class Symlinks(unittest.TestCase):
    def test_a_linked_bake_folder_is_never_removed(self):  # case 15
        with tempfile.TemporaryDirectory() as temp:
            real = Path(temp, "main")
            real.mkdir()
            (real / "kept.ron").write_text("other checkout")
            link = Path(temp, "assets_baked")
            link.symlink_to(real, target_is_directory=True)
            with mock.patch.object(pack.shutil, "rmtree", side_effect=AssertionError("rmtree")):
                with self.assertRaisesRegex(SystemExit, "link"):
                    pack.clear_baked(str(link))
            self.assertTrue((real / "kept.ron").is_file())


class PackRuns(unittest.TestCase):
    """pack.main with every tool faked: what each run asks rebake.sh to bake."""

    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = root = Path(temp.name)
        self.engine = root / "engine"
        self.baked, self.missions = self.engine / "assets_baked", self.engine / "missions"
        self.missions.mkdir(parents=True)
        (self.missions / "one.ron").write_text("tracked mission")
        (root / "iso").mkdir()
        (root / "scripts").mkdir()
        (root / "scripts/rebake.sh").write_text('BF2_MAPS="MUS/MUS1"\n')
        for game, lvls in pack.LUA.items():
            lvl = root / "extracted" / game / "lvl/DATA/_LVL_PS2"
            lvl.mkdir(parents=True)
            (lvl / "CORE.LVL").write_bytes(game.encode())
            for library in lvls.values():
                (lvl / library).parent.mkdir(parents=True, exist_ok=True)
                (lvl / library).write_bytes(library.encode())
        (self.engine / "target/release").mkdir(parents=True)
        (self.engine / "target/release" / ("game" + pack.EXE)).write_bytes(b"binary")
        self.out = root / "dist/game"
        self.versions = versions()
        self.groups = []
        self.fail_at = None

    def run_tool(self, command, **kwargs):
        if "--write-pack" in command:
            Path(command[2]).write_bytes(b"packed")
        elif command[-1] == "global-sound":
            result = Path(kwargs["env"]["OPENBF_GLOBAL_RESULT"])
            result.write_text(json.dumps([{"game": g, "action": "reused", "reason": "fixture"} for g in pack.LUA]))
        elif any(str(part).endswith("rebake.sh") for part in command):
            group = command[-1]
            if group == self.fail_at:
                raise KeyboardInterrupt
            self.groups.append(group)
            write_outputs(self.baked, group)
        elif any(str(part).endswith("extract_lua.py") for part in command):
            out = Path(command[command.index("--out") + 1])
            (out / "one.lua").write_text("lua")

    def pack(self, *extra):
        values = dict(ROOT=str(self.root), ENGINE=str(self.engine), EXTRACTED=str(self.root / "extracted"),
                      BAKED=str(self.baked), MISSIONS=str(self.missions), TARGET=str(self.engine / "target"),
                      MISSION_STAMP=str(self.root / "extracted/mission.sha"))
        start = len(self.groups)
        output = io.StringIO()
        with mock.patch.multiple(pack, **values), mock.patch.object(sys, "argv", ["pack.py", "--out", str(self.out), *extra]), \
                mock.patch.object(pack, "need"), mock.patch.object(pack, "need_ffmpeg"), \
                mock.patch.object(pack, "run", side_effect=self.run_tool), \
                mock.patch.object(pack, "find_bash", return_value="bash"), \
                mock.patch.object(pack, "group_versions", side_effect=lambda bake: self.versions), \
                mock.patch.object(pack, "missing_music", return_value=[]), \
                mock.patch.object(pack, "check_foliage", return_value=[]), \
                mock.patch.object(pack.bake_cache, "ffmpeg_identity", side_effect=lambda env, memo: self.ffmpeg), \
                mock.patch.object(pack.shutil, "which", return_value=None), \
                mock.patch.object(pack.subprocess, "run", return_value=mock.Mock(stdout="tracked", returncode=0)), \
                mock.patch.dict(os.environ, {"OPENBF_BAKE_SPACE": "1"}), \
                contextlib.redirect_stdout(output):
            pack.main()
        self.output = output.getvalue()
        return self.groups[start:]

    ffmpeg = ["ffmpeg", "7.1"]

    def test_reference_warm_bump_and_resume(self):
        everything = [g for g in bake_cache.ORDER if g != "dlc"]
        self.assertEqual(self.pack("--rebake"), everything)
        report = json.loads(Path(str(self.out) + ".sources.json").read_text())
        self.assertEqual({row["action"] for row in report["bake_groups"]}, {"bake", "with side"})
        # Warm: nothing bakes, and the bake folder is left as it was.
        (self.baked / "classes/extra.ron").write_text("kept")
        self.assertEqual(self.pack(), [])
        self.assertTrue((self.baked / "classes/extra.ron").is_file())
        self.assertIn("bake groups: 0 to bake, 11 reused", self.output)
        # A raised sounds version bakes only the sounds, and --explain says why.
        self.versions = versions(sounds=2)
        self.assertEqual(self.pack("--explain"), ["sounds", "catalog"])
        self.assertIn("sounds       baked again: version 1 -> 2", self.output)
        self.assertIn("maps         reused", self.output)
        self.assertTrue((self.baked / "classes/extra.ron").is_file(), "a partial bake never empties the folder")
        # A new ffmpeg bakes the groups that encode audio.
        self.ffmpeg = ["ffmpeg", "8.0"]
        self.assertEqual(self.pack(), ["common", "dlc", "sounds", "beep", "catalog"])

    def test_interrupted_reference_resumes(self):
        self.fail_at = "sounds"
        with self.assertRaises(KeyboardInterrupt):
            self.pack()
        self.fail_at = None
        self.assertEqual(self.pack(), ["sounds", "load", "maps", "beep", "catalog"])
        self.assertEqual(self.pack(), [])

    def test_moved_checkout_inputs_and_bake_rebuild_bake_nothing(self):
        self.pack()
        # The bake binary rebuilt (other bytes, same versions): no group, no extraction.
        (self.engine / "target/release" / ("game" + pack.EXE)).write_bytes(b"other binary")
        self.assertEqual(self.pack(), [])


class Extraction(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.image, self.bake, self.output = self.root / "image", self.root / "bake", self.root / "extracted"
        self.image.write_bytes(b"disc")
        self.bake.write_bytes(b"tool A")
        self.calls = []

    def run_extract(self, command):
        self.calls.append(command)
        (Path(command[-1]) / "data").write_bytes(self.image.read_bytes())

    def extract(self, version=1, fresh=False):
        return pack_cache.extract_image(str(self.bake), "extract-iso", str(self.image), self.output, self.run_extract,
                                        version=version, fresh=fresh)

    def test_rebuilt_bake_keeps_the_extraction(self):  # cases 11 and 14
        self.assertTrue(self.extract())
        self.bake.write_bytes(b"tool B, rebuilt")
        self.assertFalse(self.extract())
        self.assertTrue(self.extract(version=2), "a raised extract-iso version extracts again")
        self.assertEqual(len(self.calls), 2)

    def test_warm_run_does_not_read_the_image_again(self):
        self.extract()
        with mock.patch.object(pack_cache, "digest", side_effect=AssertionError("image read again")):
            self.assertFalse(self.extract())
        with mock.patch.object(pack_cache, "digest", wraps=pack_cache.digest) as read:
            self.assertFalse(self.extract(fresh=True))
            read.assert_called_once()

    def test_receipt_from_before_extraction_versions_is_kept(self):
        self.extract()
        receipt = self.output.with_name("extracted.image.json")
        value = json.loads(receipt.read_text())
        value["identity"] = dict(schema=1, image=value["identity"]["image"], bake="old binary", command="extract-iso")
        del value["image_stat"]
        receipt.write_text(json.dumps(value))
        self.assertFalse(self.extract())
        self.assertEqual(json.loads(receipt.read_text())["identity"]["extract"], 1)
        self.assertEqual(len(self.calls), 1)


VERSIONS_RS = '''pub const GROUP_VERSIONS: &[(&str, u32)] = &[
''' + "".join(f'    ("{g}", 1),\n' for g in bake_cache.GROUPS + ("extract-iso",)) + "];\n"

LOCKFILE = '''version = 3

[[package]]
name = "bake"
version = "0.1.0"
dependencies = [
 "mungefs",
 "serde",
]

[[package]]
name = "game"
version = "0.1.0"
dependencies = [
 "wgpu",
]

[[package]]
name = "mungefs"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.0"

[[package]]
name = "wgpu"
version = "22.0.0"
'''


class Guard(unittest.TestCase):
    """bake_versions.py on a small fixture tree (case 12)."""

    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = root = Path(temp.name)
        files = {
            "engine/Cargo.toml": "[workspace]\n", "engine/Cargo.lock": LOCKFILE,
            "engine/crates/bake/Cargo.toml": "[package]\nname = \"bake\"\n",
            "engine/crates/bake/src/main.rs": "fn main() {}\n",
            "engine/crates/bake/src/lib.rs": "pub mod map;\npub mod sounds;\npub mod ogg;\npub mod loadscreen;\npub mod versions;\n",
            "engine/crates/bake/src/versions.rs": VERSIONS_RS,
            "engine/crates/bake/src/map.rs": "pub fn bake() {}\n",
            "engine/crates/bake/src/sounds.rs": "use crate::ogg;\npub fn bake() {}\n",
            "engine/crates/bake/src/sounds_tests.rs": "#[test] fn t() {}\n",
            "engine/crates/bake/src/ogg.rs": "pub fn enc() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { let _ = 1; }\n}\n",
            "engine/crates/bake/src/loadscreen.rs": "pub fn bake() {}\n",
            "engine/crates/bake/src/xbox.rs": "use crate::map;\npub fn extract() {}\n",
            "engine/crates/bake/examples/tool.rs": "fn main() {}\n",
            "engine/crates/mungefs/src/lib.rs": "// munge\n", "engine/crates/manifest/src/lib.rs": "// manifest\n",
            "engine/crates/audio/src/lib.rs": "// audio\n", "engine/crates/pak/src/lib.rs": "// pak\n",
            "scripts/rebake.sh": f'REBAKE_GROUPS="{" ".join(bake_cache.GROUPS)}"\n',
            "scripts/bake_dlc.py": "# dlc\n", "scripts/dlc_inputs.py": "# dlc inputs\n",
        }
        for name, text in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        shutil.copy(ROOT / "scripts/gate_policy.py", root / "scripts/gate_policy.py")
        self.assertEqual(self.cli("neutral", "all"), 0)
        self.assertEqual(bake_versions.problems(root), [])

    def cli(self, *args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()) as err:
            code = bake_versions.main(["--root", str(self.root), *args])
        self.err = err.getvalue()
        return code

    def edit(self, name, text):
        (self.root / name).write_text(text)

    def drifted(self):
        found = bake_versions.problems(self.root)
        if not found:
            return set()
        names = found[0].split("the code behind ", 1)[1].split(" changed", 1)[0]
        return set(names.split(", "))

    def test_visual_code_change_needs_a_bump_or_a_neutral_record(self):
        self.edit("engine/crates/bake/src/map.rs", "pub fn bake() { draw(); }\n")
        self.assertEqual(self.drifted(), set(bake_versions.VISUAL))
        self.assertEqual(self.cli("check"), 1)
        self.assertIn("bump", self.err)
        self.assertEqual(self.cli("bump", "maps"), 0)
        self.assertEqual(bake_versions.read_versions(self.root)["maps"], 2)
        self.assertEqual(self.drifted(), set(bake_versions.VISUAL) - {"maps"})
        self.assertEqual(self.cli("neutral", *sorted(set(bake_versions.VISUAL) - {"maps"})), 0)
        self.assertEqual(self.cli("check"), 0)

    def test_audio_module_change_reaches_only_its_groups(self):
        self.edit("engine/crates/bake/src/ogg.rs", "pub fn enc() { louder(); }\n")
        self.assertEqual(self.drifted(), set(bake_versions.VISUAL) | {"sounds"})

    def test_xbox_extractor_change_reaches_extract_iso(self):
        # `bake extract-xiso` is bake::xbox::extract: its file is in extract-iso's closure, its other modules are not.
        self.edit("engine/crates/bake/src/xbox.rs", "use crate::map;\npub fn extract() { other(); }\n")
        self.assertEqual(self.drifted(), set(bake_versions.VISUAL) | {"extract-iso"})
        self.assertEqual(self.cli("neutral", "all"), 0)
        self.edit("engine/crates/bake/src/map.rs", "pub fn bake() { draw(); }\n")
        self.assertNotIn("extract-iso", self.drifted())

    def test_every_guarded_path_runs_the_bake_gate(self):
        # A `neutral` record is proven by the gates.py bake gate, which runs only for BAKE_CODE paths.
        import gates
        paths = [*bake_versions.SHARED, bake_versions.BAKE_SRC + "/map.rs",
                 *(path for extra in bake_versions.EXTRA.values() for path in extra)]
        for path in paths:
            probe = path if "." in path.rsplit("/", 1)[-1] else path + "/src/lib.rs"
            with self.subTest(path=probe):
                self.assertTrue(gates.touches_bake([probe]))
        probe = "engine/Cargo.lock"
        self.assertTrue(gates.touches_bake([probe]))

    def test_test_only_and_line_ending_edits_change_nothing(self):  # case 4
        self.edit("engine/crates/bake/src/sounds_tests.rs", "#[test] fn other() {}\n")
        self.edit("engine/crates/bake/examples/tool.rs", "fn main() { other(); }\n")
        ogg = self.root / "engine/crates/bake/src/ogg.rs"
        ogg.write_text(ogg.read_text().replace("let _ = 1;", "let _ = 2;").replace("\n", "\r\n"))
        self.assertEqual(bake_versions.problems(self.root), [])

    def test_only_the_bakes_part_of_cargo_lock_counts(self):
        lock = self.root / "engine/Cargo.lock"
        lock.write_text(LOCKFILE.replace('"22.0.0"', '"23.0.0"'))
        self.assertEqual(bake_versions.problems(self.root), [])
        lock.write_text(LOCKFILE.replace('"1.0.0"', '"1.0.1"'))
        self.assertEqual(self.drifted(), set(bake_versions.VISUAL) | set(bake_versions.ENTRY))

    def test_hand_edited_version_must_be_recorded(self):
        bake_versions.write_versions(self.root, {"load": 5})
        found = bake_versions.problems(self.root)
        self.assertTrue(found and "disagree on load" in found[0], found)

    @unittest.skipUnless(shutil.which("git"), "needs git")
    def test_declared_rebake_must_raise_the_version_once_per_branch(self):
        git = lambda *a: subprocess.run(["git", "-C", str(self.root), *a], check=True, capture_output=True)
        git("init", "-q", "-b", "main")
        git("-c", "user.name=t", "-c", "user.email=t@t", "add", ".")
        git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base")
        self.assertEqual(self.cli("check", "--base", "main", "--declares", "rebake=sounds"), 1)
        self.assertIn("did not rise", self.err)
        self.assertEqual(self.cli("bump", "--declares", "rebake=sounds;sweep"), 0)
        self.assertEqual(self.cli("bump", "--declares", "rebake=sounds"), 0)
        self.assertEqual(bake_versions.read_versions(self.root)["sounds"], 2, "bumped once against main")
        self.assertEqual(self.cli("check", "--base", "main", "--declares", "rebake=sounds"), 0)
        # rebake=maps implies tat1 and tat2, as the bake gate reads it.
        self.assertEqual(self.cli("bump", "--declares", "rebake=maps"), 0)
        self.assertEqual({g: bake_versions.read_versions(self.root)[g] for g in ("maps", "tat1", "tat2")},
                         {"maps": 2, "tat1": 2, "tat2": 2})

    def test_real_tree_is_recorded(self):
        """The committed lock matches this checkout's bake code and versions.rs."""
        self.assertEqual(bake_versions.problems(ROOT), [])


@unittest.skipUnless(os.name != "nt" and shutil.which("bash"), "POSIX shell")
class RebakeByHand(unittest.TestCase):
    def test_hand_run_drops_the_keys_of_the_groups_it_rebakes(self):
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp, "out")
            out.mkdir()
            keys = bake_cache.keys_folder(out)
            keys.mkdir()
            for group in ("beep", "sounds"):
                (keys / f"{group}.json").write_text("{}")
            fake = Path(temp, "bake")
            fake.write_text("#!/bin/sh\nexit 0\n")
            fake.chmod(0o755)
            env = {k: v for k, v in os.environ.items() if not k.startswith(("OPENBF_", "PC", "XB", "BF2_"))}
            env.update(OPENBF_INPUTS_CHOSEN="0", OPENBF_BAKE_BIN=str(fake), REBAKE_OUT=str(out), OPENBF_PYTHON=sys.executable)
            done = subprocess.run(["bash", str(ROOT / "scripts/rebake.sh"), "beep"], env=env, capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertFalse((keys / "beep.json").exists())
            self.assertTrue((keys / "sounds.json").exists())


if __name__ == "__main__":
    unittest.main()
