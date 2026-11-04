"""Every source combination the scan can see, on the real inputs of this machine. Skips without a complete set.

The inputs are found by the scan itself under iso/ and the development folders, so no name here belongs to one machine.
Each combination gets a folder of links to its inputs, and nothing extracted earlier counts.
This exercises input selection and bake arguments. Packaging and gameplay have their own checks.
"""
from collections import Counter
import itertools
import os
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import input_scan as scan

SOURCES = ('ps2-disc', 'xbox-disc', 'pc-install')


def real_inputs():
    """{(kind, game): path}, or None unless every source of both games is here."""
    found = {}
    for root in scan.default_roots(ROOT):
        if Path(root).is_dir():
            for row in scan.scan_root(root)[0]:
                if row['status'] != 'refused' and row['kind'] in (*SOURCES, 'dlc'):
                    found.setdefault((row['kind'], row['game']), row['path'])
    want = {(kind, game) for kind in SOURCES for game in scan.GAMES} | {('dlc', 'bf2')}
    return found if want <= set(found) else None


def subsets():
    for size in range(len(SOURCES) + 1):
        yield from itertools.combinations(SOURCES, size)


def preferred_primary(sources, allow_pc):
    eligible = ('ps2-disc',) + (('xbox-disc',) if scan.XBOX_SUPPORTED else ())
    if allow_pc:
        eligible += ('pc-install',)
    return next((kind for kind in eligible if kind in sources), None)


def shell_path(path):
    return str(path).replace('\\', '/') if path is not None else None


class EveryRealCombination(unittest.TestCase):
    def test_the_switches_decide_exactly_what_is_built_from_what(self):
        inputs = real_inputs()
        if inputs is None:
            self.skipTest('needs PS2, Xbox and PC copies of both games and the DLC folder')
        accepted = Counter()
        with tempfile.TemporaryDirectory() as temp:
            extracted = Path(temp) / 'none'
            for index, (bf1, bf2, dlc) in enumerate(itertools.product(subsets(), subsets(), (False, True))):
                with self.subTest(bf1=bf1, bf2=bf2, dlc=dlc):
                    folder = Path(temp) / str(index)
                    folder.mkdir()
                    picks = [(kind, 'bf1') for kind in bf1] + [(kind, 'bf2') for kind in bf2] + ([('dlc', 'bf2')] if dlc else [])
                    for number, pick in enumerate(picks):
                        source = Path(inputs[pick])
                        os.symlink(source, folder / f'input{number}', target_is_directory=source.is_dir())
                    plan = scan.scan([folder], extracted=str(extracted), named_root=str(folder))
                    paired_pc = scan.PC_PRIMARY_SUPPORTED and 'pc-install' in bf1 and 'pc-install' in bf2
                    primary1 = preferred_primary(bf1, scan.PC_PRIMARY_SUPPORTED)
                    primary2 = preferred_primary(bf2, paired_pc)
                    self.assertEqual(not plan['refusals'], bool(primary1 and primary2), plan['refusals'])
                    self.assertEqual(plan['dlc']['used'], dlc and scan.DLC_SUPPORTED)
                    if plan['refusals']:
                        self.assertFalse([row for row in plan['found'] if row['status'] == 'used'])
                        continue
                    both_pc = primary1 == primary2 == 'pc-install'
                    if primary2 == 'pc-install':
                        accepted['both-pc' if both_pc else 'pc-bf2'] += 1
                    else:
                        accepted['console-bf2'] += 1
                    self.assertEqual((plan['source_plan'] or {}).get('kind'), 'pc-only' if both_pc else None)
                    env = scan.bake_env(plan)
                    self.assertEqual(env.get('OPENBF_PC_ONLY'), '1' if both_pc else None)
                    dlc_path = plan['dlc']['path'] if dlc and scan.DLC_SUPPORTED else None
                    if dlc_path is not None:
                        self.assertEqual(Path(dlc_path).resolve(), Path(inputs['dlc', 'bf2']).resolve())
                    self.assertEqual(env.get('OPENBF_DLC_DIR'), shell_path(dlc_path))
                    bf2_ps2 = plan['games']['bf2']['gameplay']['path'] if primary2 == 'ps2-disc' else None
                    self.assertEqual(env.get('BF2_CONSOLE_ISO'), shell_path(bf2_ps2))
                    xbox_extracts = []
                    for game, sources, primary in (('bf1', bf1, primary1), ('bf2', bf2, primary2)):
                        number = game[-1]
                        chosen = plan['games'][game]
                        self.assertEqual(chosen['gameplay']['kind'], primary)
                        self.assertEqual(Path(chosen['gameplay']['path']).resolve(), Path(inputs[primary, game]).resolve())
                        donors = [kind for kind in ('pc-install', 'xbox-disc') if kind in sources and kind != primary
                                  and (kind != 'xbox-disc' or scan.XBOX_SUPPORTED)]
                        self.assertEqual([row['kind'] for row in chosen['visual']], donors)
                        for row in chosen['visual']:
                            self.assertEqual(Path(row['path']).resolve(), Path(inputs[row['kind'], game]).resolve())
                        xbox_tree = str(extracted / 'xbox' / game / 'Data' / '_LVL_XBOX')
                        primary_tree = {'xbox-disc': xbox_tree, 'pc-install': chosen['gameplay']['path']}.get(primary)
                        pc_donor = next((row['path'] for row in chosen['visual'] if row['kind'] == 'pc-install'), None)
                        self.assertEqual(env.get(f'OPENBF_BF{number}_TREE'), shell_path(primary_tree))
                        self.assertEqual(env.get(f'PC{number}'), shell_path(pc_donor))
                        self.assertEqual(env.get(f'XB{number}'), shell_path(xbox_tree) if 'xbox-disc' in donors else None)
                        if primary == 'xbox-disc' or 'xbox-disc' in donors:
                            xbox_extracts.append((game, Path(inputs['xbox-disc', game]).resolve(), xbox_tree))
                        for row in plan['found']:
                            if row['kind'] == 'xbox-disc' and row['game'] == game and not scan.XBOX_SUPPORTED:
                                self.assertEqual(row['status'], 'not supported yet')
                    self.assertEqual([(game, Path(disc).resolve(), tree) for game, disc, tree in scan.discs_to_extract(plan)],
                                     xbox_extracts)
        if not scan.XBOX_SUPPORTED and not scan.PC_PRIMARY_SUPPORTED:
            self.assertEqual(accepted, {'console-bf2': 32})
        if scan.XBOX_SUPPORTED and scan.PC_PRIMARY_SUPPORTED:
            self.assertEqual(accepted, {'console-bf2': 84, 'both-pc': 2, 'pc-bf2': 6})


if __name__ == '__main__':
    unittest.main()
