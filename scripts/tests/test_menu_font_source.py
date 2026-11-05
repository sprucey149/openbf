"""Optional local PS2 menu fonts must not become a general asset donor."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import pack


class MenuFontInput(unittest.TestCase):
    def test_optional_source_fingerprint_invalidates_the_pack_cache(self):
        self.assertIsNone(pack.menu_font_input(None))
        with tempfile.TemporaryDirectory() as folder:
            core = Path(folder) / 'PS2 font source' / 'CORE.LVL'
            core.parent.mkdir()
            core.write_bytes(b'first fixture')
            first = pack.menu_font_input(str(core))
            before = pack.inputs_hash((), {'menu_font_core': first})
            core.write_bytes(b'second fixture')
            second = pack.menu_font_input(str(core))
            self.assertEqual(first['path'], str(core.resolve()))
            self.assertNotEqual(first['sha256'], second['sha256'])
            self.assertNotEqual(before, pack.inputs_hash((), {'menu_font_core': second}))
            with self.assertRaisesRegex(ValueError, 'local PS2 BF2 CORE.LVL'):
                pack.menu_font_input(str(core.parent))


class RebakeMenuFonts(unittest.TestCase):
    def commands(self, override, donor, via_environment=False):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            bf1, bf2, xbox = [base / name for name in ('bf1', 'selected bf2', 'xbox donor')]
            for path in (bf1, bf2, xbox):
                path.mkdir()
                (path / 'CORE.LVL').write_text('fixture')
            (bf2 / 'MOVIES').mkdir()
            (bf2 / 'MOVIES/SHELL.MVS').write_text('fixture')
            optional = base / 'optional PS2 font' / 'CORE.LVL'
            optional.parent.mkdir()
            optional.write_text('fixture')
            log = base / 'commands.jsonl'
            bake = base / 'record-bake'
            bake.write_text('#!' + sys.executable + '\nimport os,sys,json\nwith open(os.environ["FONT_COMMAND_LOG"], "a") as log: log.write(json.dumps(sys.argv[1:])+"\\n")\n')
            bake.chmod(0o755)
            env = dict(os.environ, OPENBF_INPUTS_CHOSEN='1', OPENBF_BF1_TREE=str(bf1),
                       OPENBF_BF2_TREE=str(bf2), PC1='', PC2='', XB1='', XB2=str(xbox) if donor else '',
                       OPENBF_MENU_FONT_CORE=str(optional) if override and via_environment else '',
                       OPENBF_DLC_DIR='', OPENBF_DLC_DONOR='', BAKE=str(bake),
                       OUT=str(base / 'owned output'), FONT_COMMAND_LOG=str(log))
            args = ['bash', str(ROOT / 'scripts/scratch_bake.sh'), 'common']
            if override and not via_environment:
                args += ['--menu-font-core', str(optional)]
            result = subprocess.run(args, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            return [json.loads(line) for line in log.read_text().splitlines()], str(optional), str(bf2 / 'CORE.LVL')

    def test_override_is_menu_only_and_handles_spaces_with_or_without_donors(self):
        for donor in (False, True):
            for via_environment in (False, True):
                with self.subTest(donor=donor, environment=via_environment):
                    commands, optional, selected = self.commands(True, donor, via_environment)
                    selected_fonts = [c for c in commands if c[0] == 'fonts' and c[3] == '']
                    self.assertEqual(len(selected_fonts), 1)
                    self.assertEqual(selected_fonts[0][1], selected)
                    self.assertEqual(selected_fonts[0][-2:], ['--menu-font-core', optional])
                    self.assertTrue(all(optional not in c for c in commands if c is not selected_fonts[0]))

    def test_no_override_uses_only_the_existing_selected_source(self):
        commands, optional, _ = self.commands(False, False)
        self.assertFalse(any(optional in c or '--menu-font-core' in c for c in commands))
        self.assertFalse(any(c[0] == 'fonts' and c[3] == '' for c in commands))
        self.assertEqual(sum(c[0] == 'common' for c in commands), 2)


if __name__ == '__main__':
    unittest.main()
