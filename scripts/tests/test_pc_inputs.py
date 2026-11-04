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
            args = ['pack.py', '--pc2', str(pc), '--iso2', str(root / 'missing.iso'), '--iso-dir', str(root / 'iso'), '--out', str(root / 'dist/game')]
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
            with self.assertRaisesRegex(ValueError, 'BF1 PS2 gameplay input'):
                inputs.require_ps2_bf1_reference(True, directory)

    def test_the_bf1_visual_receipt_names_only_its_role_beside_pc_bf2(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pc1, pc2 = root / 'PC BF1', root / 'PC BF2'
            pc1.mkdir()
            pc2.mkdir()
            plan = {
                'games': {'bf1': {'gameplay': {'kind': 'ps2-disc'}},
                          'bf2': {'gameplay': {'kind': 'pc-install', 'path': str(pc2)}}},
                'found': [],
            }
            report = pack.source_report(root, root / 'baked', root / 'missions', {'bf1': None, 'bf2': None},
                                        plan, ('bf1', 'bf2'), rebaked='all', pc_roots={'bf1': pc1})
            rows = {row['label']: row for row in report['inputs']}
            donor = rows['bf1 PC visual input']
            self.assertEqual((donor['status'], donor['path'], donor['reason']),
                             ('used', str(pc1), 'passed to the texture-tier bake this run'))
            self.assertEqual(rows['bf2 PC gameplay data']['path'], str(pc2))

    def test_rebake_uses_the_selected_bf2_shell_sound_on_a_warm_tree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'scripts').mkdir()
            script = root / 'scripts/rebake.sh'
            shutil.copyfile(ROOT / 'scripts/rebake.sh', script)
            (root / 'engine').mkdir()
            console = root / 'extracted/bf2/lvl/DATA/_LVL_PS2'
            warm_sound = root / 'extracted/bf2/sound/DATA/_LVL_PS2/SOUND/SHELL.LVL'
            warm_sound.parent.mkdir(parents=True)
            warm_sound.touch()
            pc = root / 'PC BF2'
            xbox = root / 'Xbox BF2'
            for primary in (console, pc, xbox):
                for file in ('MOVIES/SHELL.MVS', 'SOUND/SHELL.LVL', 'SHELL.LVL', 'CORE.LVL'):
                    path = primary / file
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.touch()

            target = root / 'engine/target'
            bin_dir = root / 'bin'
            for path, text in (
                (bin_dir / 'cargo', '#!/bin/sh\n[ "$*" = "build -p bake --release" ]\n'),
                (target / 'release/bake',
                 '#!/bin/sh\nif [ "$1" = shell ]; then printf "%s\\000" "$@" >> "$BAKE_LOG"; fi\n'),
            ):
                path.parent.mkdir(parents=True)
                path.write_text(text)
                path.chmod(0o755)
            log = root / 'shell-argv'
            env = dict(PATH=str(bin_dir) + os.pathsep + os.environ['PATH'],
                       CARGO_TARGET_DIR=str(target), BAKE_LOG=str(log), REBAKE_OUT=str(root / 'out'),
                       OPENBF_INPUTS_CHOSEN='1')
            for label, selected, shell_sound in (
                ('PS2 primary', None, warm_sound),
                ('PC primary', pc, pc / 'SOUND/SHELL.LVL'),
                ('Xbox primary', xbox, xbox / 'SOUND/SHELL.LVL'),
            ):
                with self.subTest(label=label):
                    log.unlink(missing_ok=True)
                    source = dict(env, OPENBF_BF2_TREE=str(selected)) if selected else env
                    done = subprocess.run(['bash', str(script), 'common'], env=source,
                                          capture_output=True, text=True, timeout=20)
                    self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
                    primary = selected or console
                    self.assertEqual(log.read_bytes().decode().split('\0')[:-1],
                                     ['shell', str(primary / 'SHELL.LVL'), str(primary / 'MOVIES/SHELL.MVS'),
                                      str(shell_sound), str(primary / 'CORE.LVL'), str(root / 'out/common_bf2')])

    def test_rebake_keeps_console_ui_primary_with_a_selected_pc_art_donor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'scripts').mkdir()
            for name in ('rebake.sh', 'console_input.py', 'input_scan.py', 'dlc_inputs.py'):
                shutil.copyfile(ROOT / 'scripts' / name, root / 'scripts' / name)
            (root / 'engine').mkdir()
            console = root / 'extracted/bf2/lvl/DATA/_LVL_PS2'
            console_tree(console)
            (console / 'MOVIES').mkdir()
            (console / 'MOVIES/SHELL.MVS').touch()
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
            inputs = {'PC', 'PC1', 'PC2', 'XB1', 'XB2', 'BF2_CONSOLE_ISO'}
            env = {key: value for key, value in os.environ.items()
                   if key not in inputs and not key.startswith('OPENBF_')}
            env.update(PATH=str(root / 'bin') + os.pathsep + os.environ['PATH'],
                       CARGO_TARGET_DIR=str(target.parent), BAKE_LOG=str(log), REBAKE_OUT=str(root / 'out'))
            command = ['bash', str(root / 'scripts/rebake.sh'), '--pc2', str(pc), 'common', 'load']
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=20)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertFalse(list((root / 'out/common_bf2').rglob('old-pc-art.txt')))
            shared = [line for line in log.read_text().splitlines() if 'common_bf2' in line]
            self.assertGreaterEqual(len(shared), 6)
            self.assertTrue(all(str(console) in line for line in shared), shared)
            donor_calls = [line for line in shared if str(pc) in line]
            self.assertEqual([line.split()[0] for line in donor_calls], ['fonts', 'ui-art'])
            self.assertTrue(all(line.endswith(str(pc / 'CORE.LVL')) for line in donor_calls), donor_calls)

            log.unlink()
            console_only = dict(env, OPENBF_INPUTS_CHOSEN='1', OPENBF_BF2_TREE=str(console))
            result = subprocess.run(['bash', str(root / 'scripts/rebake.sh'), 'common', 'load'],
                                    env=console_only, capture_output=True, text=True, timeout=20)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            shared = [line for line in log.read_text().splitlines() if 'common_bf2' in line]
            self.assertGreaterEqual(len(shared), 6)
            self.assertTrue(all(str(console) in line and str(pc) not in line for line in shared), shared)

            shutil.rmtree(root / 'extracted')
            log.unlink()
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=20)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('add a PS2 BF2 disc', result.stderr)
            self.assertFalse(log.exists())


if __name__ == '__main__':
    unittest.main()
