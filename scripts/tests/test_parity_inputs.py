"""Synthetic source-only regressions; no real RE, render, sweep, or sound inputs."""
import ast, contextlib, csv, importlib.util, io, json, os, pathlib, sys, tempfile, types, unittest
from unittest.mock import patch
ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import parity, sweep_seeds as sweep, re_check

def exit_code(fn):
    try:
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()): fn()
    except SystemExit as e: return e.code
    return 0

class Parity(unittest.TestCase):
    def setUp(self):
        lock_dir = tempfile.TemporaryDirectory()
        self.addCleanup(lock_dir.cleanup)
        self.enterContext(patch.object(parity, 'LOCK', os.path.join(lock_dir.name, 'ci.lock')))

    def test_selected_inventory(self):
        selected = [('a','one','j','p'), ('a','two','j','p'), ('b','one','j','p')]
        full = [dict(map=m, spot=s) for m,s,_,_ in selected]
        for actual in (full, full[:1], full[:2], []):
            with self.subTest(actual=actual), patch.object(parity,'poses',return_value=(selected,['unbaked'])), patch.object(parity,'git_head',return_value='head'), patch.object(parity,'render',return_value=0), patch.object(parity,'score_all',return_value=actual), patch.object(parity,'summary',return_value=({},0)) as summary, patch.object(parity,'worst_sheet'), patch.object(parity,'drop_renders'):
                rc = exit_code(lambda: parity.run_tree('.',None,'.'))
                self.assertEqual(rc == 0, actual == full)
                self.assertEqual(summary.called, actual == full)
                if actual == full: self.assertEqual(summary.call_args.args[1], ['unbaked'])
    def test_gate_and_historical_intersection(self):
        with tempfile.TemporaryDirectory() as d:
            d=pathlib.Path(d); (d/'tree').mkdir()
            def write(p, keys):
                with open(p,'w') as f:
                    w=csv.DictWriter(f,parity.COLS);w.writeheader()
                    for m,s in keys:w.writerow(dict(map=m,spot=s,block=2,dluma=0,dp99=0))
            a=types.SimpleNamespace(out=str(d),allow=None,tree='.',target=None,base=str(d/'base.csv'),threshold=.5)
            for old,new,expected in [([],[],False),([('a','old')],[('b','new')],False),([('a','old'),('a','common')],[('a','common'),('a','added')],True)]:
                write(d/'base.csv',old);write(d/'tree/scores.csv',new)
                with patch.object(parity,'run_tree',return_value=([],{},2,'head')):
                    self.assertEqual(exit_code(lambda:parity.cmd_gate(a)) == 0, expected)
                self.assertEqual(json.load(open(d/'parity.json'))['status'], 'compared' if expected else 'error')
            a.base=None
            with patch.object(parity,'run_tree',return_value=([],{},2,'head')),patch.object(parity,'main_scores',return_value=None):self.assertEqual(exit_code(lambda:parity.cmd_gate(a)),3)

    def test_wrapper_errors_and_prerequisite(self):
        # Execute only the existing wrapper method, with its external command stubbed.
        tree=ast.parse((ROOT/'scripts/gates.py').read_text())
        cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='Gates')
        method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='g_parity')
        ns=dict(sys=sys, json=json, os=os, REPO=str(ROOT))
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d);(p/'parity').mkdir();calls=[]
            obj=types.SimpleNamespace(parity_allow=None,tree='.',target='.',env={},p=lambda *x:str(p.joinpath(*x)),row=lambda *x:calls.append(x))
            for rc,payload,verdict in [(2,dict(status='error',error='no common pairs'),'FAIL'),(1,None,'FAIL'),(3,dict(status='no-baseline',tree=2),'PASS')]:
                dest=p/'parity/parity.json'
                if payload is None:
                    if dest.exists():dest.unlink()
                else:dest.write_text(json.dumps(payload))
                ns['sh']=lambda *args:rc
                exec(compile(ast.Module(body=[method],type_ignores=[]),'g_parity_fixture','exec'),ns)
                ns['g_parity'](obj)
                self.assertEqual(calls[-1][-1],verdict)

class Supplemental(unittest.TestCase):
    def test_source_boundaries(self):
        with tempfile.TemporaryDirectory() as d, patch.object(re_check,'lines',return_value=['synthetic']), patch.object(re_check,'enclosing',return_value=(0,'f','0')):
            p=pathlib.Path(d);(p/'empty').touch();(p/'dir').mkdir();(p/'hit').write_text('needle')
            self.assertEqual(re_check.search(['needle'],[str(p/'empty'),str(p/'dir')],quiet=True)['total_hits'],0)
            self.assertEqual(re_check.search(['needle'],[str(p/'hit')],quiet=True)['sources'][0]['hits'],1)
            for src in [str(p/'missing'),'']:
                with self.assertRaises(OSError):re_check.search(['needle'],[src],quiet=True)
            with patch('builtins.open',side_effect=PermissionError('mock unreadable')):
                with self.assertRaises(OSError):re_check.search(['needle'],[str(p/'hit')],quiet=True)
            with patch.object(re_check,'MAX_SRC_FILES',0):
                with self.assertRaisesRegex(OSError,'budget 0'):re_check.search(['needle'],[d],quiet=True)
            def broken(*args,**kwargs):kwargs['onerror'](PermissionError('traversal'));return []
            with patch.object(re_check.os,'walk',side_effect=broken):
                with self.assertRaisesRegex(OSError,'traversal'):re_check.search(['needle'],[d],quiet=True)
            with patch.object(re_check,'write_receipt') as write:
                with self.assertRaises(OSError):re_check.absent('b1',['needle','--src',str(p/'missing')])
                write.assert_not_called()

class Sweep(unittest.TestCase):
    def test_execution_receipt(self):
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d);exe=p/'target/release/examples/round_to_outcome';exe.parent.mkdir(parents=True);out=p/'out';out.mkdir()
            def binary(rc=0, rows=True):
                exe.write_text('#!/bin/sh\n' + ('echo "a no outcome tickets 1/1 stuck_max 2 hash 00"\necho "sweep: finished 0/1  stuck_max median 2"\necho "a done (1/1, 0 s)" >&2\n' if rows else '')+f'exit {rc}\n');exe.chmod(0o755)
            args=['sweep','--out',str(out),'--target',str(p/'target'),'--seeds','1','--reuse','--maps','a']
            def run(extra=()):
                with patch.object(sys,'argv',args+list(extra)),patch.object(sweep,'LOCK',str(p/'lock')):return exit_code(sweep.main)
            binary(7);self.assertEqual(run(),1);self.assertEqual(run(),1);self.assertFalse((out/'sweep_s1.complete.json').exists())
            binary();self.assertEqual(run(),0);receipt=out/'sweep_s1.complete.json';before=receipt.read_bytes()
            with patch.object(sweep.subprocess,'call',side_effect=AssertionError('should reuse')):self.assertEqual(run(),0)
            self.assertEqual(receipt.read_bytes(),before)
            (out/'sweep_s1.txt').write_text('truncated');self.assertEqual(run(),0)
            self.assertIn('no outcome',(out/'sweep_s1.txt').read_text())
            self.assertEqual(run(['--ticks','2']),0);self.assertNotEqual(receipt.read_bytes(),before)
            receipt.unlink();self.assertEqual(run(),0)
            self.assertEqual(run(['--threads','2']),0)
            self.assertEqual(run(['--maps','b']),1);self.assertFalse(receipt.exists())
            self.assertEqual(run(),0)
            binary(7);self.assertEqual(run(),1);self.assertFalse(receipt.exists())
            binary();self.assertEqual(run(),0)
            self.assertEqual(run(['--botq']),1);self.assertFalse(receipt.exists())
            self.assertEqual(run(['--botq','--no-botq-report']),1);self.assertFalse(receipt.exists())
            binary(rows=False);self.assertEqual(run(),1);self.assertFalse(receipt.exists())
    def test_parse_stdout_skips_round_detail_lines(self):
        # round_to_outcome prints setup / opening / timing detail per map; they are not map rows,
        # indented (current) or in column 0 (builds before the fix).
        text = ('setup bf1_nab1 combatants 20/20 tickets 150/150\nopening bf1_nab1 alive 20/20\n'
                '    setup bf1_nab1 combatants 20/20 tickets 150/150\n    opening bf1_nab1 alive 20/20\n'
                'bf1_nab1   team 1 wins at tick 25000 stuck_max 3 hash 00\n    mission: x\n'
                'timing bf1_nab1 median_us 900 p99_us 2000 max_us 5000\n'
                '    timing bf1_nab1 median_us 900 p99_us 2000 max_us 5000\n'
                'bf2_pol1   NO OUTCOME in 9000 ticks  tickets 3/4\nsweep: finished 1/2  stuck_max median 3\n')
        with tempfile.TemporaryDirectory() as d:
            f = pathlib.Path(d) / 'sweep_s1.txt'; f.write_text(text)
            rows = sweep.parse_stdout(str(f))
        self.assertEqual(sorted(rows), ['bf1_nab1', 'bf2_pol1'])
        self.assertTrue(rows['bf1_nab1']['finished']); self.assertEqual(rows['bf1_nab1']['tick'], 25000)
        self.assertFalse(rows['bf2_pol1']['finished'])
    def test_flyer_diagnostics_do_not_become_map_results(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d)
            (p / 'sweep_s1.txt').write_text(
                'a team 0 wins at tick 123 stuck_max 2 hash 00\n'
                'flyers a: boarded 49 took-off 49 airborne 7332 s\n'
                'b NO OUTCOME in 9000 ticks stuck_max 3 hash 01\n'
                'flyers b: boarded 0 took-off 0 airborne 0 s\n'
                'sweep: finished 1/2  stuck_max median 2\n')
            (p / 'sweep_s1.err').write_text('a done (1/2, 0 s)\nb done (2/2, 0 s)\n')
            self.assertEqual(set(sweep.parse_stdout(str(p / 'sweep_s1.txt'))), {'a', 'b'})
            self.assertEqual(len(sweep.completed_artifacts(d, 1, [], False)), 2)
            # A missing genuine result still fails the selected-map inventory.
            (p / 'sweep_s1.txt').write_text('flyers a: boarded 49\nsweep: finished 0/2  stuck_max median 2\n')
            with self.assertRaises(ValueError):
                sweep.completed_artifacts(d, 1, [], False)

    def test_default_inventory_and_botq(self):
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d);(p/'sweep_s1.txt').write_text('a no outcome stuck_max 2\nsweep: finished 0/1  stuck_max median 2\n');(p/'sweep_s1.err').write_text('a done (1/1, 0 s)\n')
            self.assertEqual(len(sweep.completed_artifacts(d,1,[],False)),2)
            with self.assertRaises(OSError):sweep.completed_artifacts(d,1,[],True)
            (p/'botq').mkdir()
            for ext in ['.csv','.ev.csv']:(p/f'botq/a_s1{ext}').write_text('header\n')
            self.assertEqual(len(sweep.completed_artifacts(d,1,[],True)),4)
            (p/'sweep_s1.err').write_text('a done (1/2, 0 s)\n')
            with self.assertRaises(ValueError):sweep.completed_artifacts(d,1,[],False)

class Sound(unittest.TestCase):
    def test_prerequisites_and_decoder_results(self):
        # Load definitions without the script's unconditional entry point; no decoder is invoked.
        fake_np=types.ModuleType('numpy')
        ns={'__file__':str(ROOT/'scripts/sound_bake_check.py')}
        with patch.dict(sys.modules,{'numpy':fake_np}):exec(compile((ROOT/'scripts/sound_bake_check.py').read_text().rsplit('\nmain()',1)[0],ns['__file__'],'exec'),ns)
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d)
            for key in ['bf1_a','bf1_b']:
                (p/key/'sounds/bank').mkdir(parents=True);(p/key/'sounds/sounds.ron').write_text('source: "x"');(p/key/'sounds/bank/0001.wav').touch()
            with patch.object(sys,'argv',['sound',d]):
                ns['disc_files']=lambda _:{}
                self.assertEqual(exit_code(ns['main']),2)
                # Substitute only the pool's job execution to exercise complete/partial
                # source bookkeeping and existing result handling without decoding.
                disc=p/'source.lvl';disc.write_bytes(b'fixture')
                ns['disc_files']=lambda _: {'x':str(disc)}
                ns['samples']=lambda _: {}
                class Pool:
                    def __init__(self,*args):pass
                    def __enter__(self):return self
                    def __exit__(self,*args):pass
                    def map(self, fn, jobs):return [(w,None,False,0.0) for _,w,_ in jobs]
                ns['ThreadPoolExecutor']=Pool
                self.assertEqual(exit_code(ns['main']),0)
                (p/'bf1_b/sounds/sounds.ron').write_text('source: "missing"')
                self.assertEqual(exit_code(ns['main']),2)
                (p/'bf1_b/sounds/sounds.ron').write_text('source: "x"')
                Pool.map=lambda self,fn,jobs:[(w,'not on the disc',True,0.0) for _,w,_ in jobs]
                self.assertEqual(exit_code(ns['main']),1)
                Pool.map=lambda self,fn,jobs:[(w,'after saturated frame',False,0.0) for _,w,_ in jobs]
                self.assertEqual(exit_code(ns['main']),0)
                for wav in p.glob('*/sounds/bank/*.wav'):wav.unlink()
                self.assertEqual(exit_code(ns['main']),2)

if __name__=='__main__':unittest.main()
