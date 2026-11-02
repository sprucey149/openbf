"""Mission correction targets remain absolute, source-gated and ordered."""
import copy
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gameplay_missions as m
import gen_missions as g
from version_terrain_play import parse_ron


class MissionCorrectionTests(unittest.TestCase):
    def test_absolute_target_replaces_missing_and_donor_only_team_values(self):
        base = {'units': {1: 500, 2: 300, 3: 30}, 'classes': {1: [('wrong', 99)]}, 'sound': ['donor']}
        target = {'units': {'1': 16, '2': 20}, 'classes': {'1': [['soldier', 4, None]]}}
        out = m.overlay(base, target)
        self.assertEqual(out['units'], {1: 16, 2: 20})
        self.assertEqual(out['classes'], {1: [('soldier', 4, None)]})
        self.assertEqual(out['sound'], ['donor'])
        self.assertEqual(base['units'][1], 500)

    def test_unknown_required_field_fails_instead_of_inventing_default(self):
        with self.assertRaisesRegex(ValueError, 'unmeasured'):
            m.overlay({}, {'units': 'NM'}, require_complete=True)
        self.assertEqual(m.overlay({'units': {1: 3}}, {'units': 'NM'})['units'], {1: 3})

    def test_missing_or_unknown_source_fingerprint_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            raw = b'ucfb' + struct.pack('<I', 0)
            source = root / 'mission.lvl'
            source.write_bytes(raw)
            table = root / 'correction.json'
            table.write_text(json.dumps({'schema': 2, 'reference': 'ps2-v1', 'game': 'bf1', 'files': [
                {'file': 'mission.lvl', 'sha256': hashlib.sha256(raw).hexdigest(), 'missions': []}]}))
            self.assertEqual(m.load([table], {'bf1': source}), {'bf1': {}})
            with self.assertRaisesRegex(ValueError, 'require --gameplay-source'):
                m.load([table])
            source.write_bytes(raw + b'other revision')
            with self.assertRaisesRegex(ValueError, 'unknown MISSION.LVL fingerprint'):
                m.load([table], {'bf1': source})

    def test_duplicate_name_is_allowed_only_for_identical_scoped_facts(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'table.json'
            rows = [{'name': 'map1', 'facts': {'units': {'1': 16}}} for _ in range(2)]
            table = {'schema': 2, 'reference': 'ps2-v1', 'game': 'bf1', 'files': [{'file': 'mission.lvl', 'missions': rows}]}
            path.write_text(json.dumps(table))
            self.assertEqual(len(m.load([path], require_source=False)['bf1']), 1)
            table = copy.deepcopy(table)
            table['files'][0]['missions'][1]['facts']['units']['1'] = 32
            path.write_text(json.dumps(table))
            with self.assertRaisesRegex(ValueError, 'ambiguous'):
                m.load([path], require_source=False)

    def test_real_bf1_without_ps2_scripts_keeps_all_gameplay_mission_fields(self):
        root = Path(__file__).resolve().parents[2]
        source = root / 'PC_swbf1_exe'
        if not source.is_dir() or not Path(g.PS2_BF1).is_dir():
            self.skipTest('requires owned BF1 PC and PS2 source data')
        with patch.object(g, 'PS2_ONLY', True), patch.object(g, 'GAMEPLAY_REFERENCES', {}):
            before = {}
            for key, game, planet, eras in g.MAPS:
                if game == 'bf1':
                    for era, (stem, script) in eras.items():
                        before[key, era] = parse_ron(g.ron(g.build(key, game, planet, era, stem, script)))
            refs = m.load([root / 'engine/crates/manifest/data/gameplay/corrections-v1/bf1-pc.json'], {'bf1': source})
            with patch.object(g, 'PS2_BF1', '/missing-ps2-scripts'), patch.object(g, 'GAMEPLAY_REFERENCES', refs):
                checked = 0
                for key, game, planet, eras in g.MAPS:
                    if game != 'bf1':
                        continue
                    for era, (stem, script) in eras.items():
                        after = parse_ron(g.ron(g.build(key, game, planet, era, stem, script)))
                        expected = dict(before[key, era])
                        # PC authored fly-through camera shots differ; these do
                        # not enter the simulation and remain donor presentation.
                        expected.pop('camera_shots', None)
                        after.pop('camera_shots', None)
                        self.assertEqual(after, expected, (key, era))
                        checked += 1
                self.assertEqual(checked, 28)


if __name__ == '__main__':
    unittest.main()
