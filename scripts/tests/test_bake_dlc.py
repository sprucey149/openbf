"""Scoped DLC orchestration tests; source recognition has its own tests."""
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bake_dlc import bake_dlc
from dlc_inputs import DlcSource


class BakeDlc(unittest.TestCase):
    def test_absent_is_noop_even_without_reference(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch("bake_dlc.detect_dlc", return_value=[]), patch("bake_dlc.subprocess.run") as run:
                self.assertIsNone(bake_dlc(root / "missing", "missing", root / "out", "bake"))
                run.assert_not_called()
            self.assertFalse((root / "out").exists())

    def test_ps2_gameplay_and_pc_texture_donor_materialized(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pc = root / "pc.lvl"
            pc.write_bytes(b"PC texture donor")
            image = root / "renamed.image"
            image.write_bytes(b"prefixhero-bytesunused-map-script")
            preferred = DlcSource(image, "ps2", 10, "renamed/side.data", 6)
            sources = [DlcSource(pc, "pc", pc.stat().st_size), preferred]
            ingame = root / "reference.lvl"
            ingame.write_bytes(b"reference")
            output = root / "out"
            staged = []

            def run(command, check):
                self.assertTrue(check)
                self.assertEqual(command[:2], [str(root / "bake"), "dlc-heroes"])
                lvl = Path(command[2])
                staged.append(lvl)
                self.assertNotIn(output, lvl.parents)
                self.assertEqual(lvl.read_bytes(), b"hero-bytes")
                self.assertEqual(command[3:5], [str(ingame), str(output)])
                self.assertEqual(len(command), 6)
                self.assertEqual(Path(command[5]).read_bytes(), b"PC texture donor")
                staged.append(Path(command[5]))

            with patch("bake_dlc.detect_dlc", return_value=sources), patch("bake_dlc.subprocess.run", side_effect=run):
                self.assertEqual(bake_dlc(image, ingame, output, root / "bake"), preferred)
            self.assertTrue(all(not path.exists() for path in staged))
            self.assertEqual(image.read_bytes(), b"prefixhero-bytesunused-map-script")

    def sound_folder(self, root):
        source = root / "dlc"
        source.mkdir()
        side, ingame = source / "side.lvl", root / "game/INGAME.LVL"
        bank = source / "anything.dat"
        side.write_bytes(b"side")
        ingame.parent.mkdir()
        ingame.write_bytes(b"reference")
        (ingame.parent / "CORE.LVL").write_bytes(b"core")
        bank.write_bytes(b"ucfb" + struct.pack("<I", 2186927242))
        return source, ingame, bank, DlcSource(side, "pc", 4)

    def test_the_heroes_sound_bank_follows_with_the_games_core(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, ingame, bank, side = self.sound_folder(root)
            commands = []
            with patch("bake_dlc.detect_dlc", return_value=[side]), \
                    patch("bake_dlc.subprocess.run", side_effect=lambda command, check: commands.append(command)):
                bake_dlc(source, ingame, root / "out", root / "bake")
            self.assertEqual(commands[-1], [str(root / "bake"), "dlc-sounds", str(bank), str(ingame.parent / "CORE.LVL"),
                                            str(root / "out")])

    def test_an_unreadable_sound_bank_does_not_stop_the_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, ingame, _, side = self.sound_folder(root)

            def run(command, check):
                if command[1] == "dlc-sounds":
                    raise subprocess.CalledProcessError(1, command)

            with patch("bake_dlc.detect_dlc", return_value=[side]), patch("bake_dlc.subprocess.run", side_effect=run):
                self.assertEqual(bake_dlc(source, ingame, root / "out", root / "bake"), side)

    def test_output_input_and_symlink_guards(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_dir = root / "inputs"
            source_dir.mkdir()
            path = source_dir / "side"
            path.write_bytes(b"data")
            source = DlcSource(path, "pc", 4)
            link = root / "shared"
            link.symlink_to(root / "target", target_is_directory=True)
            with patch("bake_dlc.detect_dlc", return_value=[source]), patch("bake_dlc.subprocess.run") as run:
                for output in (source_dir, source_dir / "out", root / "unmade" / ".." / "inputs" / "out", link / "out", Path("/")):
                    with self.assertRaisesRegex(ValueError, "output"):
                        bake_dlc(source_dir, "missing", output, "bake")
                run.assert_not_called()

    def test_missing_reference_and_bake_failure_propagate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "side"
            path.write_bytes(b"data")
            source = DlcSource(path, "pc", 4)
            with patch("bake_dlc.detect_dlc", return_value=[source]), patch("bake_dlc.subprocess.run") as run:
                with self.assertRaisesRegex(ValueError, "PS2 BF2 INGAME"):
                    bake_dlc(path, root / "missing", root / "out", "bake")
                run.assert_not_called()
                run.side_effect = subprocess.CalledProcessError(1, "bake")
                with self.assertRaises(subprocess.CalledProcessError):
                    bake_dlc(path, path, root / "out", "bake")
                self.assertEqual(len(run.call_args.args[0]), 5, "PC-only needs no donor")


if __name__ == "__main__":
    unittest.main()
