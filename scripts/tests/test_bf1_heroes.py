"""BF1 assignments become BF2 playable kits without changing other rosters."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gen_missions as g


class Bf1HeroTests(unittest.TestCase):
    def test_identity_mapping_preserves_team_order_and_snow_variant(self):
        self.assertEqual(g.mission_heroes('bf1', 'con', {'heroes': {
            2: 'all_inf_lukeskywalkersnow', 1: 'imp_inf_darthvader'}}),
            [(1, 'imp_hero_darthvader'), (2, 'all_hero_luke_pilot')])
        self.assertEqual(g.mission_heroes('bf1', 'con', {'heroes': {
            2: 'rep_inf_macewindu', 1: 'cis_inf_countdooku'}}),
            [(1, 'cis_hero_countdooku'), (2, 'rep_hero_macewindu')])
        self.assertEqual(g.mission_heroes('bf1', 'con', {'heroes': {
            1: 'all_inf_lukeskywalker'}}), [(1, 'all_hero_luke_jedi')])
        with self.assertRaisesRegex(ValueError, 'unmapped BF1 hero'):
            g.mission_heroes('bf1', 'con', {'heroes': {1: 'unknown'}})

    def test_hunt_has_no_heroes_and_bf2_keeps_authored_spellings(self):
        facts = {'heroes': {2: 'rep_HERO_aalya', 1: 'other'}}
        self.assertEqual(g.mission_heroes('bf2', 'con', facts),
                         [(1, 'other'), (2, 'rep_HERO_aalya')])
        self.assertEqual(g.mission_heroes('bf1', 'hunt', facts), [])

    def test_owned_bf1_scripts_supply_all_56_assignments(self):
        if not Path(g.PS2_BF1).is_dir():
            self.skipTest('requires owned BF1 PS2 mission bytecode')
        count, snow = 0, []
        with patch.object(g, 'PS2_ONLY', True), patch.object(g, 'GAMEPLAY_REFERENCES', {}):
            for key, game, planet, eras in g.MAPS:
                if game != 'bf1':
                    continue
                for era, (stem, script) in eras.items():
                    original = g.facts_from_calls(g.calls40(g.load_ps2(game, script)))
                    built = g.build(key, game, planet, era, stem, script)
                    self.assertEqual(built.heroes, [(t, g.BF1_HERO_CLASSES[n.lower()])
                                                   for t, n in sorted(original['heroes'].items())])
                    self.assertEqual(len(built.heroes), 2, script)
                    self.assertEqual(built.classes, [original['classes'][1], original['classes'][2]])
                    self.assertEqual(built.class_load_order, [])
                    for team, hero in built.heroes:
                        self.assertEqual(hero.split('_')[0], built.teams[team - 1][0])
                        if hero == 'all_hero_luke_pilot':
                            snow.append(script)
                    count += len(built.heroes)
        self.assertEqual(count, 56)
        self.assertEqual(sorted(snow), ['hot1i', 'rhn1i'])

    def test_owned_bf1_hunt_adaptations_keep_heroes_out(self):
        if not Path(g.PS2_BF1).is_dir():
            self.skipTest('requires owned BF1 PS2 mission bytecode')
        with patch.object(g, 'PS2_ONLY', True), patch.object(g, 'GAMEPLAY_REFERENCES', {}):
            for key, game, planet, eras in g.MAPS:
                if key in g.BF1_HUNTS:
                    era = 'gcw' if key in ('bf1_end1', 'tat1', 'bf1_tat2') else 'cw'
                    self.assertEqual(g.bf1_hunt(key, planet, era).heroes, [], key)


if __name__ == '__main__':
    unittest.main()
