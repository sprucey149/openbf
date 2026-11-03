"""The calibration's externally capped mode must not escape into heavy.sh scopes."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    'calibrate_match_length', Path(__file__).resolve().parents[1] / 'calibrate_match_length.py')
CAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAL)


class ScopeTests(unittest.TestCase):
    def test_direct_round_inherits_the_callers_scope_and_checks_setup(self):
        output = ('setup map combatants 16/16 tickets 150/150\n'
                  'team 0 wins at tick 12600\n')
        with tempfile.TemporaryDirectory() as out:
            Path(out, 'raw').mkdir()
            with patch.object(CAL.subprocess, 'run') as run:
                run.return_value.returncode = 0
                run.return_value.stdout = output
                run.return_value.stderr = 'round map s1 normal proxy 0 good\n'
                row = CAL.play_once(None, '/owned/round', out, False,
                                    ('map', False, [16, 16], [150, 150]), 0, 100, 100)
                self.assertEqual(run.call_args.args[0], ['/owned/round', '180000', '1', 'map'])
                self.assertEqual(run.call_args.kwargs['env']['SWEEP_THREADS'], '1')
                self.assertEqual((row['finished'], row['minutes']), (1, '7.0000'))

    def test_direct_round_refuses_a_binary_with_different_defaults(self):
        with tempfile.TemporaryDirectory() as out:
            Path(out, 'raw').mkdir()
            with patch.object(CAL.subprocess, 'run') as run:
                run.return_value.returncode = 0
                run.return_value.stdout = 'setup map combatants 16/16 tickets 200/200\n'
                run.return_value.stderr = 'round map s1 normal proxy 0 good\n'
                with self.assertRaisesRegex(RuntimeError, 'want'):
                    CAL.play_once(None, '/owned/round', out, False,
                                  ('map', False, [16, 16], [150, 150]), 0, 100, 100)

    def test_raising_the_limit_retries_a_cached_censored_round(self):
        with tempfile.TemporaryDirectory() as out:
            raw = Path(out, 'raw'); raw.mkdir()
            name = raw / 'map_gcw_t100_r100'
            name.with_suffix('.out').write_text(
                'setup map combatants 16/16 tickets 150/150\nNO OUTCOME in 90000 ticks\n')
            name.with_suffix('.err').write_text('round map s1 normal proxy 0 good\n')
            with patch.object(CAL, 'TICKS', 180000), patch.object(CAL.subprocess, 'run') as run:
                run.return_value.returncode = 0
                run.return_value.stdout = ('setup map combatants 16/16 tickets 150/150\n'
                                           'team 0 wins at tick 100000\n')
                run.return_value.stderr = 'round map s1 normal proxy 0 good\n'
                row = CAL.play_once(None, '/owned/round', out, True,
                                    ('map', False, [16, 16], [150, 150]), 0, 100, 100)
                self.assertEqual(run.call_args.args[0][1], '180000')
                self.assertEqual(row['finished'], 1)

    def test_censored_cells_cannot_rewrite_the_menu_with_a_partial_mean(self):
        with tempfile.TemporaryDirectory() as out:
            import csv
            model = Path(out, 'model.rs'); model.write_text('retained model')
            rows = [dict(team_pct=t, ticket_pct=r, combatants='16/16', tickets='150/150',
                         finished=int(t != 10), minutes='7' if t != 10 else '') for t, r in CAL.CELLS]
            with Path(out, 'rounds.csv').open('w') as f:
                writer = csv.DictWriter(f, fieldnames=list(rows[0])); writer.writeheader(); writer.writerows(rows)
            with patch.object(CAL, 'MODEL_RS', str(model)):
                with self.assertRaisesRegex(ValueError, '--ticks'):
                    CAL.fit(out)
            self.assertEqual(model.read_text(), 'retained model')

    def test_supplied_binary_needs_neither_build_nor_nested_heavy_scope(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp, 'round')
            binary.write_bytes(b'owned prebuilt binary')
            out = Path(tmp, 'measurement')
            with (patch.object(CAL, 'heavy_sh', side_effect=AssertionError('nested scope')),
                  patch.object(CAL, 'build', side_effect=AssertionError('unexpected build')),
                  patch.object(CAL, 'table', return_value=[('map', False, [16, 16], [150, 150])]),
                  patch.object(CAL, 'CELLS', [(100, 100)]),
                  patch.object(CAL, 'play', return_value={'map': 'map', 'era': 'gcw',
                                'team_pct': 100, 'ticket_pct': 100})):
                CAL.measure(str(out), 1, False, str(binary), True)
            self.assertEqual((out / 'round_to_outcome').read_bytes(), binary.read_bytes())
            self.assertEqual((out / 'binary_sha256.txt').read_text().strip(),
                             CAL.hashlib.sha256(binary.read_bytes()).hexdigest())


if __name__ == '__main__':
    unittest.main()
