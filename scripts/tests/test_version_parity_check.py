from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import version_parity_check as parity


class Results(unittest.TestCase):
    def check(self, worlds, rounds):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cases = []
            for label in ('reference', 'donor'):
                assets = root / label
                (assets / 'map').mkdir(parents=True)
                cases.append((label, assets, root / 'missions'))
            with patch.object(parity.world_fingerprint, 'fingerprint', side_effect=worlds), patch.object(parity, 'run_round', side_effect=rounds):
                return parity.compare(cases, ['map'], root / 'binary', root / 'receipt.json')['maps']

    @staticmethod
    def world(component='abc', outside='outside'):
        return {'components': {'terrain_playable': component}, 'terrain_all_sha256': outside, 'complete': True}

    @staticmethod
    def round(finished=True, checkpoint='abc'):
        return {'hash': 'abcd', 'finished': finished, 'checkpoints': [[300, checkpoint]]}

    def test_outside_only_difference_can_pass(self):
        rows = self.check([self.world(), self.world(outside='different')], [self.round(), self.round()])
        self.assertEqual(rows[1]['status'], 'identical')
        self.assertTrue(rows[1]['outside_terrain_differs'])

    def test_equal_endpoint_cannot_hide_divergent_round(self):
        rows = self.check([self.world(), self.world()], [self.round(), self.round(checkpoint='different')])
        self.assertFalse(rows[1]['sim_hash_identical'])
        self.assertNotEqual(rows[1]['status'], 'identical')

    def test_unfinished_equal_hashes_are_not_acceptance(self):
        rows = self.check([self.world(), self.world()], [self.round(False), self.round(False)])
        self.assertTrue(all(r['status'] == 'differs_or_unfinished' for r in rows))

    def test_incomplete_world_cannot_pass_with_equal_hashes(self):
        world = self.world()
        world['complete'] = False
        rows = self.check([world, world], [self.round(), self.round()])
        self.assertTrue(all(r['status'] != 'identical' for r in rows))

    def test_missing_input_is_never_identical(self):
        rows = self.check([self.world(), ValueError('missing collision')], [self.round()])
        self.assertEqual(rows[1]['status'], 'unmeasured')
        self.assertIn('missing collision', rows[1]['error'])


if __name__ == '__main__':
    unittest.main()
