"""Input-policy regressions: no real assets are required by these fixtures."""
import contextlib
import io
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import console_input as inputs
import pack


def chunk(tag, body):
    return tag + struct.pack('<I', len(body)) + body + bytes(-len(body) % 4)


def geometry(platform):
    return chunk(b'ucfb', chunk(b'modl', chunk(b'segm', chunk(platform, bytes(4)))))


def console_tree(root):
    root.mkdir(parents=True, exist_ok=True)
    for name in ('CORE.LVL', 'SHELL.LVL'):
        (root / name).write_bytes(chunk(b'ucfb', b''))
    (root / 'INGAME.LVL').write_bytes(geometry(b'_PS2'))


def pc_tree(root):
    console_tree(root)
    (root / 'INGAME.LVL').write_bytes(geometry(b'VBUF'))


def record(name, lba, size, directory=False):
    name = name.encode('ascii')
    data = bytearray(33 + len(name) + (len(name) % 2 == 0))
    data[0] = len(data)
    struct.pack_into('<I', data, 2, lba)
    struct.pack_into('<I', data, 10, size)
    data[25], data[32] = 2 if directory else 0, len(name)
    data[33:33 + len(name)] = name
    return data


def iso(path):
    data = bytearray(24 * 2048)
    data[16 * 2048:16 * 2048 + 7] = b'\1CD001\1'
    data[16 * 2048 + 8:16 * 2048 + 40] = b'PLAYSTATION'.ljust(32)
    struct.pack_into('<I', data, 16 * 2048 + 158, 20)
    struct.pack_into('<I', data, 16 * 2048 + 166, 2048)
    root = record('DATA', 21, 2048, True) + record('SYSTEM.CNF;1', 23, 1) + record('SWBFRNT2.ELF;1', 23, 1)
    data[20 * 2048:20 * 2048 + len(root)] = root
    sub = record('_LVL_PS2', 22, 2048, True)
    data[21 * 2048:21 * 2048 + len(sub)] = sub
    files = b''.join(record(name + ';1', 23, 1) for name in ('CORE.LVL', 'INGAME.LVL', 'SHELL.LVL'))
    data[22 * 2048:22 * 2048 + len(files)] = files
    path.write_bytes(data)
    return data


class InputPolicy(unittest.TestCase):
    def test_console_content_and_pc_refusal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pc_tree(root)
            self.assertEqual(inputs.pc_game(root), 'bf2')
            self.assertFalse(inputs.is_ps2_bf2_tree(root))
            with self.assertRaisesRegex(ValueError, 'add a PS2 BF2 disc'):
                inputs.require_console_bf2(True, root)
            console_tree(root)
            self.assertTrue(inputs.is_ps2_bf2_tree(root))
            inputs.require_console_bf2(True, root)
            self.assertIsNone(inputs.pc_game(root))

    def test_iso_identified_by_contents_and_hostile_extents(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'not-a-game-name.dat'
            original = iso(path)
            self.assertTrue(inputs.ps2_bf2_disc(path))
            for offset, word in ((16 * 2048 + 166, 0xffffffff), (20 * 2048 + 10, 0xffffffff), (20 * 2048 + 2, 0xffffffff)):
                data = original[:]
                struct.pack_into('<I', data, offset, word)
                path.write_bytes(data)
                self.assertFalse(inputs.ps2_bf2_disc(path))
            for size in (0, 16 * 2048, 16 * 2048 + 156, 21 * 2048, len(original) - 2048):
                path.write_bytes(original[:size])
                self.assertFalse(inputs.ps2_bf2_disc(path))
            path.write_bytes(b'XBOX_DVD_LAYOUT_TOOL_SIG')
            self.assertFalse(inputs.ps2_bf2_disc(path))

    def test_truncated_lvl_and_tag_inside_payload_are_not_platform_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'ingame.lvl'
            for data in (b'_PS2' + bytes(4), chunk(b'ucfb', chunk(b'tex_', b'_PS2')), geometry(b'_PS2')[:-1], b'ucfb\xff\xff\xff\xff'):
                path.write_bytes(data)
                self.assertFalse(inputs.is_ps2_lvl(path))

    def test_pack_refuses_before_build_even_with_cached_assets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pc = root / 'GameData/Data/_LVL_PC'
            pc_tree(pc)
            args = ['pack.py', '--pc2', str(pc), '--iso2', str(root / 'missing.iso')]
            with patch.object(pack, 'EXTRACTED', str(root / 'extracted')), patch.object(sys, 'argv', args), patch.object(pack, 'run') as run, contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(SystemExit, 'add a PS2 BF2 disc'):
                    pack.main()
                run.assert_not_called()

    def test_changing_pc_sources_invalidates_bake_cache(self):
        with tempfile.TemporaryDirectory() as directory:
            stamp = Path(directory) / "stamp"
            source = {"bf2": "first/install"}
            stamp.write_text(pack.inputs_hash([], source) + "\n")
            self.assertIsNone(pack.stale(stamp, [], source))
            self.assertIsNotNone(pack.stale(stamp, [], {"bf2": "second/install"}))
            self.assertIsNotNone(pack.stale(stamp, [], {}))

    def test_console_lua_cache_does_not_inherit_old_pc_scripts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / 'bf1/lvl/DATA/_LVL_PS2/MISSION.LVL'
            library.parent.mkdir(parents=True)
            library.write_bytes(b'console mission fixture')
            old = root / 'bf1/lua/mission'
            old.mkdir(parents=True)
            (old / 'pc_only.luac').write_bytes(b'PC')
            with patch.object(pack, 'EXTRACTED', str(root)), patch.object(pack, 'LUA', {'bf1': {'mission': 'MISSION.LVL'}}):
                cache = Path(pack.console_lua_root())
                self.assertNotEqual(cache / 'bf1/mission', old)
                (old / 'pc_only.luac').write_bytes(b'different PC')
                self.assertEqual(Path(pack.console_lua_root()), cache)
                library.write_bytes(b'changed console mission')
                self.assertNotEqual(Path(pack.console_lua_root()), cache)

    def test_missing_bf1_reference_is_explicit(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, 'BF1 PS2 gameplay reference'):
                inputs.require_ps2_bf1_reference(True, directory)

    def test_rebake_uses_console_ui_without_pc_alternatives(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'scripts').mkdir()
            for name in ('rebake.sh', 'console_input.py'):
                shutil.copyfile(ROOT / 'scripts' / name, root / 'scripts' / name)
            (root / 'engine').mkdir()
            console = root / 'extracted/bf2/lvl/DATA/_LVL_PS2'
            console_tree(console)
            pc = root / 'pc/GameData/Data/_LVL_PC'
            pc_tree(pc)
            target = root / 'target/release'
            target.mkdir(parents=True)
            (target / 'bake').write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$BAKE_LOG"\n')
            (target / 'bake').chmod(0o755)
            (root / 'bin').mkdir()
            (root / 'bin/cargo').write_text('#!/bin/sh\nexit 0\n')
            (root / 'bin/cargo').chmod(0o755)
            for directory in ('fonts', 'hud', 'locl'):
                stale = root / 'out/common_bf2' / directory / 'old-pc-art.txt'
                stale.parent.mkdir(parents=True, exist_ok=True)
                stale.write_text('stale PC UI')
            log = root / 'calls.log'
            env = dict(os.environ, PATH=str(root / 'bin') + os.pathsep + os.environ['PATH'], CARGO_TARGET_DIR=str(target.parent), BAKE_LOG=str(log), REBAKE_OUT=str(root / 'out'))
            env.pop('PC1', None)
            env.pop('PC2', None)
            command = ['bash', str(root / 'scripts/rebake.sh'), '--pc2', str(pc), 'common', 'load']
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=20)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertFalse(list((root / 'out/common_bf2').rglob('old-pc-art.txt')))
            shared = [line for line in log.read_text().splitlines() if 'common_bf2' in line]
            self.assertGreaterEqual(len(shared), 6)
            self.assertTrue(all(str(pc) not in line for line in shared), shared)
            self.assertTrue(all(str(console) in line for line in shared), shared)
            shutil.rmtree(root / 'extracted')
            log.unlink()
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=20)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('add a PS2 BF2 disc', result.stderr)
            self.assertFalse(log.exists())


if __name__ == '__main__':
    unittest.main()
