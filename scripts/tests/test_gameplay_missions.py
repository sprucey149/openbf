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
import gameplay_corrections as corrections
from version_terrain_play import parse_ron


class MissionCorrectionTests(unittest.TestCase):
    def test_turret_stop_cue_keeps_its_defer_qualifier(self):
        prop = struct.pack('<I', 0xc13068b6) + b'turret_whir_end defer\0'
        payload = b'PROP' + struct.pack('<I', len(prop)) + prop + bytes(-len(prop) % 4)
        self.assertEqual(corrections.props(payload), [[0xc13068b6, ['turret_whir_end', 'defer']]])
        self.assertEqual(corrections.preserve_hashes(payload), [])
        root = Path(__file__).resolve().parents[2]
        table = json.loads((root / 'engine/crates/manifest/data/gameplay/corrections-v1/bf1-pc.json').read_bytes())
        row = next(c for f in table['files'] for c in f.get('classes', []) if c['name'] == 'com_weap_gunturret')
        self.assertIn([0xc13068b6, ['turret_whir_end', 'defer']], row['properties'])

    def test_donor_only_matches_all_required_console_gameplay_records(self):
        root = Path(__file__).resolve().parents[2]
        games = ('bf1', 'bf2')
        source_profiles = {
            'pc': {game: root / f'PC_swbf{game[-1]}_exe' for game in games},
            'xbox': {game: root / 'extracted' / f'xbox_{game}' for game in games},
        }
        if not Path(g.PS2_BF1).is_dir() or not Path(g.PS2_BF2).is_dir():
            self.skipTest('requires the owned console mission control')
        available = {platform: sources for platform, sources in source_profiles.items()
                     if all((path / 'mission.lvl').is_file() or platform == 'pc' and path.is_dir()
                            for path in sources.values())}
        if not available:
            self.skipTest('requires owned PC or Xbox data')
        table_dir = root / 'engine/crates/manifest/data/gameplay/corrections-v1'
        entries = [(key, game, planet, era, stem, script)
                   for key, game, planet, eras in g.MAPS
                   for era, (stem, script) in eras.items()
                   if not g.optional_script(game, script)]
        self.assertEqual(len(entries), 74)
        with patch.object(g, 'PS2_ONLY', True), patch.object(g, 'GAMEPLAY_REFERENCES', {}):
            expected = [parse_ron(g.ron(g.build(*entry))) for entry in entries]
        for platform, sources in available.items():
            refs = m.load([table_dir / f'{game}-{platform}.json' for game in games], sources)
            for entry, before in zip(entries, expected, strict=True):
                with self.subTest(platform=platform, map=entry[0], script=entry[-1]), \
                        patch.object(g, 'PS2_ONLY', True), \
                        patch.object(g, 'TABLE_ONLY_GAMES', games), \
                        patch.object(g, 'GAMEPLAY_REFERENCES', refs):
                    after = parse_ron(g.ron(g.build(*entry)))
                    # These sections use the selected source's presentation.
                    for field in ('source', 'voice', 'music', 'camera_shots'):
                        before.pop(field, None)
                        after.pop(field, None)
                    self.assertEqual(after, before)

    def test_campaign_from_a_pc_or_xbox_copy_is_the_console_campaign(self):
        """Rise of the Empire comes from whichever BF2 copy the player owns, and plays the console
        campaign's rules: with no console scripts at all (and BF2 table-only, as a PC primary is)
        every campaign mission and library a PC or Xbox copy builds is written exactly as the
        console copy's. This covers each difference the campaign programs normalize (the Mace
        timer, the wrapped Objective 6 handler, icons, victory delays, AI difficulty, shield
        recharge, death regions, class limits) and anything a later table would miss."""
        import campaign_ir
        root = Path(__file__).resolve().parents[2]
        if not Path(g.PS2_BF2).is_dir():
            self.skipTest('requires the owned console mission control')
        sources = {'pc': root / 'PC_swbf2_exe', 'xbox': root / 'extracted' / 'xbox_bf2'}
        available = {platform: path for platform, path in sources.items()
                     if (path / 'mission.lvl').is_file() or platform == 'pc' and path.is_dir()}
        if not available:
            self.skipTest('requires owned PC or Xbox BF2 data')
        table_dir = root / 'engine/crates/manifest/data/gameplay/corrections-v1'
        self.assertEqual(len(g.CAMPAIGN_MISSIONS), 18)

        def written(missions):
            libs = g.campaign_libs([mission.campaign['script'] for mission in missions])
            return ([g.ron(mission) for mission in missions],
                    {name: campaign_ir.ron_lib(script) for name, script in libs.items()})

        with patch.object(g, 'PS2_ONLY', True), patch.object(g, 'GAMEPLAY_REFERENCES', {}):
            expected_missions, expected_libs = written([g.build_campaign(*e) for e in g.CAMPAIGN_MISSIONS])
        for platform, source in available.items():
            refs = m.load([table_dir / f'bf2-{platform}.json'], {'bf2': source})
            with patch.object(g, 'PS2_ONLY', True), \
                    patch.object(g, 'PS2_BF2', '/missing-console-scripts'), \
                    patch.object(g, 'TABLE_ONLY_GAMES', ('bf2',)), \
                    patch.object(g, 'GAMEPLAY_REFERENCES', refs):
                for entry in g.CAMPAIGN_MISSIONS:
                    self.assertIsNotNone(g.load_campaign_script('bf2', entry[-1]))
                missions, libs = written([g.build_campaign(*e) for e in g.CAMPAIGN_MISSIONS])
            for entry, after, before in zip(g.CAMPAIGN_MISSIONS, missions, expected_missions, strict=True):
                with self.subTest(platform=platform, script=entry[-1]):
                    self.assertEqual(after, before)
            self.assertEqual(sorted(libs), sorted(expected_libs))
            for name in expected_libs:
                with self.subTest(platform=platform, library=name):
                    self.assertEqual(libs[name], expected_libs[name])

    def test_campaign_program_rebuilds_only_its_own_result_from_its_own_copy(self):
        import campaign_programs as cp
        fn = lambda name, body: {'name': name, 'params': 0, 'vararg': False, 'locals': 2, 'body': body}
        call = lambda f, *a: ('Do', ('Call', ('Global', f), list(a)))
        donor = {'main': 0, 'fns': [fn('main', [call('CreateTimer', ('Str', 't')), call('KillObject', ('Str', 'cp1'))]),
                                    fn('handler', [call('ShowMessageText', ('Str', 'done'))])]}
        want = {'main': 0, 'fns': [fn('main', [call('KillObject', ('Str', 'cp1')), call('KillObject', ('Str', 'cp2'))]),
                                   fn('handler', [call('ShowMessageText', ('Str', 'done'))])]}
        program = cp.make(want, donor)
        out = cp.apply(program, donor, donor_sha=cp.digest(donor), result_sha=cp.digest(want))
        self.assertEqual(out, want)
        self.assertNotIn('CreateTimer', json.dumps(program))
        with self.assertRaisesRegex(ValueError, 'not the one'):
            cp.apply(program, want, donor_sha=cp.digest(donor), result_sha=cp.digest(want))
        with self.assertRaisesRegex(ValueError, 'does not reproduce'):
            cp.apply(program, donor, donor_sha=cp.digest(donor), result_sha=cp.digest(donor))

    def test_campaign_program_tables_are_for_the_fingerprinted_copies(self):
        root = Path(__file__).resolve().parents[2]
        table_dir = root / 'engine/crates/manifest/data/gameplay/corrections-v1'
        import campaign_programs as cp
        for platform in ('pc', 'xbox'):
            table = cp.load(table_dir / f'bf2-{platform}-campaign.json')
            reference = json.loads((table_dir / f'bf2-{platform}.json').read_bytes())
            lvl = next(row['sha256'] for row in reference['files'] if row['file'] == 'mission.lvl')
            self.assertEqual(table['mission_lvl'], lvl)
            for entry in g.CAMPAIGN_MISSIONS:
                self.assertIn(entry[-1], table['scripts'])

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
