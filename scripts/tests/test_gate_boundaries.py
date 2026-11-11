"""Source-only gate boundary regressions: no builds, real gates, assets or oracle updates."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import gates
import oracle_guard


class GateBoundaryTests(unittest.TestCase):
    def args(self, root, **kw):
        a = types.SimpleNamespace(tree=str(root), target=str(root / 'target'), out=str(root / 'out'),
                                  bake_groups=[], keep_goldens=False, base='custom', fidelity_both=True,
                                  leak_plan='auto', fidelity_allow=None)
        a.__dict__.update(kw)
        Path(a.out).mkdir(exist_ok=True)
        return a

    def test_rounds_build_failure_clears_previous_receipt(self):
        with tempfile.TemporaryDirectory() as td:
            gate = gates.Gates(self.args(Path(td)))
            receipt = Path(gate.p('rounds', 'results.json'))
            receipt.parent.mkdir()
            receipt.write_text('stale receipt')
            with patch.object(gate, 'build_release', return_value=101) as build, \
                 patch.object(gates, 'sh') as run:
                gate.g_rounds()
            build.assert_called_once_with('round_to_outcome', failure_gate='rounds')
            run.assert_not_called()
            self.assertFalse(receipt.exists())

    def test_rounds_build_failure_is_replaced_when_resumed(self):
        class Fake(gates.Gates):
            def g_protect(self):
                if not resumed:
                    self.build_release('ordnance_census')
                    raise InterruptedError
                self.row('protect', 'probe', 0, 'clean', 'clean', 'PASS')

        builds = []

        def sh(cmd, cwd, env, log):
            if cmd.startswith('cargo build'):
                builds.append(cmd)
                return 101 if not resumed and '--example round_to_outcome' in cmd else 0
            Path(log).write_text('round goldens: 14 PASS\n')
            return 0

        with tempfile.TemporaryDirectory() as td:
            a = self.args(Path(td))
            groups = ['rounds', 'protect']
            with patch.object(gates, 'run_args', return_value={'groups': groups}), \
                 patch.object(gates, 'sh', side_effect=sh):
                resumed = False
                with self.assertRaises(InterruptedError):
                    gates.run(a, groups, cls=Fake)
                saved = gates.load_state(a.out)
                self.assertEqual(saved['done'], ['rounds'])
                self.assertEqual([(r['gate'], r['verdict']) for r in saved['rows']], [('rounds', 'FAIL')])
                self.assertIn('--example ordnance_census', builds[1])
                resumed = True
                rows = gates.run(a, groups, resume=True, cls=Fake)
            self.assertEqual([(r['gate'], r['verdict']) for r in rows],
                             [('rounds', 'PASS'), ('protect', 'PASS')])

    def fidelity(self, cfg, values, allow=None):
        with tempfile.TemporaryDirectory() as td:
            a = self.args(Path(td), fidelity_dir='fixture', fidelity_allow=allow)
            g = gates.Gates(a)
            g.exp = {'fidelity': cfg}
            with patch.object(gates, 'fidelity_measure', return_value=values), \
                 patch.object(gates, 'card_fidelity_allow', return_value=set()), \
                 patch.object(gates, 'card_exposes', return_value=set()):
                g.g_fidelity()
            return g.rows

    def test_fidelity_completeness(self):
        cfg = {'enabled': True, 'metrics': {'still': {'dir': 'down', 'agg': 3, 'maps': {'m1': 2, 'm2': 4, 'null': None}}}}
        full = {'still': {'agg': 3, 'maps': {'m1': 2, 'm2': 4}}}
        self.assertEqual(self.fidelity(cfg, full)[0]['verdict'], 'PASS')
        for values, missing in [({}, ['still@agg', 'still@m1', 'still@m2']),
                                ({'still': {'agg': 3, 'maps': {'m1': 2}}}, ['still@m2']),
                                ({'still': {'maps': {'m1': 2, 'm2': 4}}}, ['still@agg'])]:
            row = self.fidelity(cfg, values, 'all')[0]
            self.assertEqual(row['verdict'], 'FAIL')
            for key in missing:
                self.assertIn(key, row['value'])
        for value in (float('nan'), float('inf'), -float('inf')):
            for scope in ('agg', 'map'):
                values = copy.deepcopy(full)
                if scope == 'agg':
                    values['still']['agg'] = value
                else:
                    values['still']['maps']['m1'] = value
                row = self.fidelity(cfg, values, 'all')[0]
                self.assertEqual(row['verdict'], 'FAIL')
                self.assertIn('still@agg' if scope == 'agg' else 'still@m1', row['value'])
        moved = copy.deepcopy(full)
        moved['still']['agg'] = 9
        self.assertEqual(self.fidelity(cfg, moved, 'all')[0]['verdict'], 'MOVED')
        self.assertEqual(self.fidelity(cfg, moved, '')[0]['verdict'], 'FAIL')
        # Partial helper consumers retain their intentional intersection behavior.
        self.assertEqual(gates.fidelity_compare(cfg, {})['checked'], 0)

    def test_confirmation_requires_complete_finite_evidence(self):
        cfg = {'enabled': True, 'confirm_seeds': '9-16', 'seeds': '1-8',
               'metrics': {'still': {'dir': 'down', 'agg': 3, 'maps': {'m1': 2, 'm2': 4}}}}
        first = {'still': {'agg': 3, 'maps': {'m1': 8, 'm2': 4}}}
        complete = {'still': {'agg': 3, 'maps': {'m1': 2, 'm2': 4}}}
        invalid = copy.deepcopy(complete)
        invalid['still']['maps']['m1'] = float('nan')
        for second, expected in [({}, 'FAIL'), (invalid, 'FAIL'), (complete, 'PASS')]:
            with tempfile.TemporaryDirectory() as td:
                g = gates.Gates(self.args(Path(td), fidelity_dir='first', fidelity_confirm_dir='second', fidelity_allow=''))
                g.exp = {'fidelity': cfg}
                with patch.object(gates, 'fidelity_measure', side_effect=[first, second]), \
                     patch.object(gates, 'confirm_base', return_value=None):
                    g.g_fidelity()
                self.assertEqual(g.rows[0]['verdict'], expected)

    def test_required_baseline_confirm_block(self):
        cfg = {'enabled': True, 'confirm_seeds': '9-16', 'seeds': '1-8',
               'metrics': {'still': {'dir': 'down', 'agg': 3, 'maps': {'m1': 2, 'm2': 4}}}}
        complete = {'still': {'agg': 3, 'maps': {'m1': 2, 'm2': 4}}}
        missing_map = {'still': {'agg': 3, 'maps': {'m1': 2}}}
        invalid = copy.deepcopy(complete)
        invalid['still']['agg'] = float('inf')
        cases = [(complete, 0, 'PASS'), ({}, 0, 'FAIL'), (missing_map, 0, 'FAIL'),
                 (invalid, 0, 'FAIL'), (ValueError('unreadable fixture'), 0, 'FAIL'),
                 (complete, 8, 'FAIL'), (complete, OSError('exec fixture'), 'FAIL')]
        for second, rc, expected in cases:
            with self.subTest(second=second, rc=rc), tempfile.TemporaryDirectory() as td:
                g = gates.Gates(self.args(Path(td), fidelity_both=True, fidelity_allow='all'))
                g.exp = {'fidelity': cfg}
                receipt = Path(g.p('fidelity', 'fidelity_confirm_new.json'))
                receipt.parent.mkdir()
                receipt.write_text('stale receipt')
                with patch.object(g, 'build_release', return_value=0), \
                     patch.object(gates, 'run_split', side_effect=[0, rc]), \
                     patch.object(gates, 'fidelity_measure', side_effect=[complete, second]):
                    g.g_fidelity()
                self.assertEqual(g.rows[0]['verdict'], expected)
                if expected == 'PASS':
                    self.assertEqual(json.loads(receipt.read_text()), {'seeds': '9-16', 'values': complete})
                else:
                    self.assertFalse(receipt.exists())
                    self.assertEqual(gates.rows_exit(g.rows), 1)

    def test_fidelity_explicit_unchecked(self):
        for cfg in ({'enabled': False}, {}, {'metrics': {'x': {'dir': 'track', 'agg': 1}}},
                    {'metrics': {'x': {'dir': 'down', 'agg': None}}}):
            row = self.fidelity(cfg, {})[0]
            self.assertEqual(row['verdict'], 'PASS')
            self.assertIn('not checked:', row['value'])

    def test_resume_restoration_and_retention(self):
        ran = []
        class Fake(gates.Gates):
            def g_protect(s):
                ran.append('protect')
                s.row('protect', 'fixture', 0, 'ok', '', 'PASS')
            def write(s):
                pass
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            a = self.args(root)
            with patch.object(gates.subprocess, 'run', return_value=types.SimpleNamespace(stdout='head\n')):
                gates.run(a, ['protect'], cls=Fake)
                saved = gates.load_state(a.out)['args']
                resumed = self.args(root, base='main', fidelity_both=False, leak_plan=None)
                groups = gates.restore_run_args(resumed, saved)
                self.assertEqual((resumed.base, resumed.fidelity_both, resumed.leak_plan), ('custom', True, 'auto'))
                self.assertIsNone(resumed.fidelity_allow)
                # Exercise the CLI's bare-resume branch, with no gate supplier invoked.
                with patch.object(sys, 'argv', ['gates.py', '--out', a.out, '--resume']), \
                     patch.object(gates, 'run', return_value=[]) as run_fixture, \
                     self.assertRaises(SystemExit) as done:
                    gates.main()
                self.assertEqual(done.exception.code, 0)
                cli_args = run_fixture.call_args.args[0]
                self.assertEqual((cli_args.base, cli_args.fidelity_both, cli_args.leak_plan), ('custom', True, 'auto'))
                gates.run(resumed, groups, resume=True, cls=Fake)
                self.assertEqual(ran, ['protect'])
                for key, val in [('base', 'another'), ('fidelity_both', False), ('leak_plan', 'always'), ('fidelity_allow', '')]:
                    changed = copy.copy(resumed)
                    setattr(changed, key, val)
                    self.assertNotEqual(gates.run_args(changed, groups), saved)
                    gates.save_json(str(root / 'out' / gates.STATE), dict(args=saved, rows=[], done=['protect'], status='running'))
                    gates.run(changed, groups, resume=True, cls=Fake)
                self.assertEqual(len(ran), 5)
                for missing in ('base', 'fidelity_both', 'leak_plan'):
                    old = dict(saved)
                    old.pop(missing)
                    gates.save_json(str(root / 'out' / gates.STATE), dict(args=old, rows=[], done=['protect'], status='running'))
                    gates.restore_run_args(resumed, old)
                    gates.run(resumed, groups, resume=True, cls=Fake)
                self.assertEqual(len(ran), 8)

    def test_oracle_comparison(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            def git(*args):
                return subprocess.run(['git', '-C', td, '-c', 'user.name=fixture', '-c', 'user.email=f@f', *args],
                                      check=True, capture_output=True, text=True).stdout.strip()
            git('init', '-q', '-b', 'main')
            (root / 'plain.txt').write_text('one')
            git('add', '.'); git('commit', '-qm', 'initial')
            self.assertEqual(oracle_guard.changed_by_class(td, 'main'), {})
            git('checkout', '-qb', 'fixture')
            (root / 'plain.txt').write_text('two')
            git('add', '.'); git('commit', '-qm', 'unprotected')
            self.assertEqual(oracle_guard.changed_by_class(td, 'main'), {})
            (root / 'scripts').mkdir()
            (root / 'scripts/gates.py').write_text('protected')
            git('add', '.'); git('commit', '-qm', 'protected')
            self.assertEqual(oracle_guard.changed_by_class(td, 'main'), {'guards': ['scripts/gates.py']})
            # A valid unrelated root has no merge base.
            git('checkout', '--orphan', 'unrelated')
            git('commit', '-qm', 'root')
            for base, tree in [('main', td), ('missing-base', td), ('main', str(root / 'absent'))]:
                with self.assertRaises(oracle_guard.ComparisonError):
                    oracle_guard.changed_by_class(tree, base)
                a = self.args(root, tree=tree, base=base)
                g = gates.Gates(a)
                g.g_protect()
                self.assertEqual(g.rows[0]['verdict'], 'FAIL')
                self.assertEqual(gates.rows_exit(g.rows), 1)
                self.assertIn(base, g.rows[0]['value'])
            with patch.object(sys, 'argv', ['oracle_guard.py', 'changed', 'missing-base', td]):
                self.assertEqual(oracle_guard.main(), 1)
            with patch.object(oracle_guard.subprocess, 'run', side_effect=OSError('fixture exec failure')):
                g = gates.Gates(self.args(root, base='main'))
                g.g_protect()
                self.assertEqual(g.rows[0]['verdict'], 'FAIL')
                self.assertIn('fixture exec failure', g.rows[0]['value'])
            git('checkout', '-q', 'fixture')
            for base, expected, approved in [('HEAD', 'PASS', set()), ('main', 'FAIL', set()), ('main', 'MOVED', {'guards'})]:
                a = self.args(root, base=base)
                g = gates.Gates(a)
                with patch.object(oracle_guard, 'card_declares_for', return_value=('FIX', approved)):
                    g.g_protect()
                self.assertEqual(g.rows[0]['verdict'], expected)

    def test_adapter_assertion_detects_wrong_implementation(self):
        # Execute just the actual corrected assertion in a disposable namespace, then mutate its supplier.
        import ast
        source = ast.parse((SCRIPTS / 'gates_selftest.py').read_text())
        node = next(n for n in source.body if isinstance(n, ast.Expr) and isinstance(n.value, ast.Call)
                    and n.value.args and isinstance(n.value.args[0], ast.Constant)
                    and n.value.args[0].value == 'adapter_check: known empty device list -> ADAPTER LOST')
        code = compile(ast.Module(body=[node], type_ignores=[]), '<adapter assertion>', 'exec')
        result = []
        exec(code, {'gates': gates, 'check': lambda name, ok: result.append(ok)})
        self.assertEqual(result, [True])
        with patch.object(gates, 'adapter_check', return_value=(True, 'not checked')):
            exec(code, {'gates': gates, 'check': lambda name, ok: result.append(ok)})
        self.assertEqual(result, [True, False])


if __name__ == '__main__':
    unittest.main()
