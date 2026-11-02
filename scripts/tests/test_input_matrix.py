"""Every source combination the scan can see, on the real inputs of this machine. Skips without a complete set.

The inputs are found by the scan itself under iso/ and the development folders, so no name here belongs to one machine.
Each combination gets a folder of links to its inputs, and nothing extracted earlier counts.
"""
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


class EveryRealCombination(unittest.TestCase):
    def test_the_switches_decide_exactly_what_is_built_from_what(self):
        inputs = real_inputs()
        if inputs is None:
            self.skipTest('needs PS2, Xbox and PC copies of both games and the DLC folder')
        accepted = 0
        with tempfile.TemporaryDirectory() as temp:
            for index, (bf1, bf2, dlc) in enumerate(itertools.product(subsets(), subsets(), (False, True))):
                with self.subTest(bf1=bf1, bf2=bf2, dlc=dlc):
                    folder = Path(temp) / str(index)
                    folder.mkdir()
                    picks = [(kind, 'bf1') for kind in bf1] + [(kind, 'bf2') for kind in bf2] + ([('dlc', 'bf2')] if dlc else [])
                    for number, pick in enumerate(picks):
                        os.symlink(inputs[pick], folder / f'input{number}')
                    plan = scan.scan([folder], extracted=str(Path(temp) / 'none'), named_root=str(folder))
                    primary1 = 'ps2-disc' in bf1 or (scan.XBOX_SUPPORTED and 'xbox-disc' in bf1) or (scan.PC_PRIMARY_SUPPORTED and 'pc-install' in bf1)
                    primary2 = 'ps2-disc' in bf2 or (scan.XBOX_SUPPORTED and 'xbox-disc' in bf2)
                    self.assertEqual(not plan['refusals'], bool(primary1 and primary2), plan['refusals'])
                    self.assertEqual(plan['dlc']['used'], dlc)
                    if plan['refusals']:
                        self.assertFalse([row for row in plan['found'] if row['status'] == 'used'])
                        continue
                    accepted += 1
                    env = scan.bake_env(plan)
                    for number, game, sources in (('1', 'bf1', bf1), ('2', 'bf2', bf2)):
                        chosen = plan['games'][game]
                        if 'ps2-disc' in sources:
                            self.assertEqual(chosen['gameplay']['kind'], 'ps2-disc')
                            self.assertNotIn(f'OPENBF_BF{number}_TREE', env)
                        self.assertEqual(f'PC{number}' in env, 'pc-install' in sources and chosen['gameplay']['kind'] != 'pc-install')
                        for row in plan['found']:
                            if row['kind'] == 'xbox-disc' and row['game'] == game and not scan.XBOX_SUPPORTED:
                                self.assertEqual(row['status'], 'not supported yet')
        if not scan.XBOX_SUPPORTED and not scan.PC_PRIMARY_SUPPORTED:
            self.assertEqual(accepted, 32)
        if scan.XBOX_SUPPORTED and scan.PC_PRIMARY_SUPPORTED:
            self.assertEqual(accepted, 84)


if __name__ == '__main__':
    unittest.main()
