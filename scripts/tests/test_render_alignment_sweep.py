"""The scene sweep composes neighbours and terrain before ranking, keeps PS2's residual as the baseline, and gates
small real map selections against a ceiling when bakes are supplied."""
import json
import os
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from render_alignment_sweep import Collisions, GATE_MAPS, Providers, Terrain, gate_problems, local_witnesses
from render_collision_check import Surface


def plane(y, half=2.0, x=0.0):
    return Surface([(x-half, y, -half), (x+half, y, -half), (x+half, y, half), (x-half, y, half)], [0, 1, 2, 0, 2, 3])


IDENTITY = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]


def placed(x):
    return dict(rotation=IDENTITY, translation=[x, 0.0, 0.0])


class NoTerrain(Terrain):
    def __init__(self, surface=None):
        self.surface, self.cutters = surface, False


def providers(items, terrain=None):
    p = object.__new__(Providers)
    p.terrain, p.unknown = NoTerrain(terrain), []
    p.items = []
    for n, (x, art) in enumerate(items):
        box = [x + art.bounds[0], art.bounds[1], art.bounds[2], x + art.bounds[3], art.bounds[4], art.bounds[5]]
        p.items.append(dict(identity=(0, n), inst=placed(x), potential=art, covered=art, box=box, geometry=f'g{n}'))
    return p


class FlatContact:
    def points(self, count):
        return [(0, 0, 0), (-1, 0, -1), (1, 0, 1)]

    def distance(self, point):
        return abs(point[1])


class SweepTests(unittest.TestCase):
    def test_neighbour_covers_a_local_gap(self):
        # The source moved this object's art 2 m up, but a neighbouring slab draws at the collision height in both.
        ps2 = providers([(0.0, plane(0.0)), (1.0, plane(0.0, half=3.0))])
        source = providers([(0.0, plane(2.0)), (1.0, plane(0.0, half=3.0))])
        self.assertAlmostEqual(source.nearest((0.0, 0.0, 0.0), 0, 'covered')[0], 0.0)
        self.assertAlmostEqual(ps2.nearest((0.0, 0.0, 0.0), 0, 'covered')[0], 0.0)

    def test_uncovered_gap_survives_composition(self):
        ps2 = providers([(0.0, plane(0.0)), (20.0, plane(0.0))])
        source = providers([(0.0, plane(2.0)), (20.0, plane(0.0))])
        gap, by = source.nearest((0.0, 0.0, 0.0), 0, 'covered')
        self.assertAlmostEqual(gap - ps2.nearest((0.0, 0.0, 0.0), 0, 'covered')[0], 2.0)
        self.assertEqual(by, 'g0')

    def test_terrain_draw_counts_as_cover(self):
        source = providers([(0.0, plane(2.0))], terrain=plane(0.0, half=50.0))
        self.assertEqual(source.nearest((0.0, 0.0, 0.0), 0, 'covered'), (0.0, 'terrain'))

    def test_other_mode_layers_never_cover(self):
        p = providers([(0.0, plane(2.0)), (0.0, plane(0.0))])
        p.items[1]['identity'] = (2, 0)
        self.assertAlmostEqual(p.nearest((0.0, 0.0, 0.0), 1, 'covered')[0], 2.0)
        self.assertAlmostEqual(p.nearest((0.0, 0.0, 0.0), 2, 'covered')[0], 0.0)

    def test_ps2_residual_is_the_baseline(self):
        forward, reverse = local_witnesses(FlatContact(), plane(1.0), plane(1.005), plane(1.005), 8)
        self.assertAlmostEqual(max(w['extra_m'] for w in forward), .005)
        self.assertTrue(all(w['opaque'] for w in reverse))

    def test_reverse_uses_the_whole_scene_collision(self):
        c = object.__new__(Collisions)
        c.terrain = NoTerrain(plane(-5.0, half=50.0))
        c.items = [dict(identity=(0, 0), inst=placed(0.0), contact=FlatContact(), box=[-2, 0, -2, 2, 0, 2], geometry='g')]
        self.assertAlmostEqual(c.nearest((0.0, 3.0, 0.0), 0)[0], 3.0)
        self.assertEqual(c.nearest((0.0, -4.0, 0.0), 0), (1.0, 'terrain'))

    def test_gate_reports_only_unlisted_offenders_above_the_ceiling(self):
        report = dict(maps=[dict(source='xbox', map='bf2_pol1', objects=dict(witnesses=[
            dict(geometry='a', scene_extra_m=.6, in_playable_boundary=True, below_all_ps2_terrain=False, world=[0, 0, 0]),
            dict(geometry='b', scene_extra_m=.6, in_playable_boundary=False, below_all_ps2_terrain=False, world=[0, 0, 0]),
            dict(geometry='c', scene_extra_m=.2, in_playable_boundary=True, below_all_ps2_terrain=False, world=[0, 0, 0]),
            dict(geometry='k', scene_extra_m=.9, in_playable_boundary=True, below_all_ps2_terrain=False, world=[0, 0, 0]),
        ], unknown=[]), terrain=dict(worst_excess_m=0.0))])
        problems = gate_problems(report, .25, {('xbox', 'bf2_pol1', 'k'): (.9, 'listed')})
        self.assertEqual(len(problems), 1)
        self.assertIn(' a ', problems[0])

    def test_gate_fails_a_listed_offender_that_grew(self):
        def report(extra):
            return dict(maps=[dict(source='xbox', map='bf2_pol1', objects=dict(witnesses=[
                dict(geometry='k', scene_extra_m=extra, in_playable_boundary=True, below_all_ps2_terrain=False,
                     world=[0, 0, 0])], unknown=[]), terrain=dict(worst_excess_m=0.0))])
        known = {('xbox', 'bf2_pol1', 'k'): (.9, 'listed')}
        self.assertEqual(gate_problems(report(.94), .25, known), [])
        problems = gate_problems(report(3.0), .25, known)
        self.assertEqual(len(problems), 1)
        self.assertIn('past its listed 0.900 m', problems[0])

    def test_near_repair_limit_cannot_hide_below_the_general_ceiling(self):
        witness = dict(geometry='fel1_bldg_bladder', state='model', scene_extra_m=.20,
                       in_playable_boundary=True, below_all_ps2_terrain=False, world=[0, 0, 0])
        report = dict(maps=[dict(source='xbox', map='bf2_fel1',
                                objects=dict(witnesses=[witness], unknown=[]),
                                terrain=dict(worst_excess_m=0.0))])
        # A broad known exception must not mask a repaired near model either.
        known = {('xbox', 'bf2_fel1', 'fel1_bldg_bladder'): (1., 'distant model')}
        self.assertEqual(len(gate_problems(report, .25, known)), 1)
        witness['scene_extra_m'] = .14
        self.assertEqual(gate_problems(report, .25, known), [])
        witness.update(state='lowd', scene_extra_m=.20)
        self.assertEqual(gate_problems(report, .25, known), [])

    def test_new_maps_allow_only_five_mm_growth_of_an_outstanding_row(self):
        witness = dict(geometry='remaining', state='model', scene_extra_m=.504,
                       in_playable_boundary=True, below_all_ps2_terrain=False, world=[0, 0, 0])
        report = dict(maps=[dict(source='xbox', map='bf1_rhn2',
                                objects=dict(witnesses=[witness], unknown=[]),
                                terrain=dict(worst_excess_m=0.0))])
        known = {('xbox', 'bf1_rhn2', 'remaining'): (.5, 'outstanding')}
        self.assertEqual(gate_problems(report, .25, known), [])
        witness['scene_extra_m'] = .506
        self.assertEqual(len(gate_problems(report, .25, known)), 1)

    def test_gate_fails_an_unlisted_unmeasured_placement(self):
        report = dict(maps=[dict(source='pc1', map='bf1_bes2', objects=dict(witnesses=[], unknown=[
            dict(identity=[0, 1], name='listed', reason='source placement differs from PS2'),
            dict(identity=[0, 2], name='new', reason='source placement differs from PS2')],
            provider_unknown=dict(reference=[], source=[dict(identity=[0, 3], name='drawn',
                                                              reason='missing configured draw model')])),
            terrain=dict(worst_excess_m=0.0))])
        problems = gate_problems(report, .25, {}, {('pc1', 'bf1_bes2', 'listed'): 'why'})
        self.assertEqual(len(problems), 2)
        self.assertIn('placement new', problems[0])
        self.assertIn('source drawing drawn', problems[1])


@unittest.skipUnless(os.environ.get('OPENBF_ALIGNMENT_REFERENCE') and os.environ.get('OPENBF_ALIGNMENT_CASES'),
                     'needs OPENBF_ALIGNMENT_REFERENCE and OPENBF_ALIGNMENT_CASES bakes (only the gate maps)')
class RealMapGate(unittest.TestCase):
    """The gate maps only; bake just those map groups for each source (see the module docstring)."""
    def test_gate_maps_have_no_new_scene_offender(self):
        script = Path(__file__).resolve().parents[1] / 'render_alignment_sweep.py'
        cases = shlex.split(os.environ['OPENBF_ALIGNMENT_CASES'])
        with tempfile.TemporaryDirectory() as scratch:
            out = Path(scratch) / 'sweep.json'
            command = [sys.executable, str(script), '--reference', os.environ['OPENBF_ALIGNMENT_REFERENCE'],
                       '--gate', '--no-vehicles', '--out', str(out)]
            for case in cases:
                command += ['--case', case]
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            report = json.loads(out.read_text())
            self.assertEqual(sorted({row['map'] for row in report['maps']}), sorted(GATE_MAPS))


if __name__ == '__main__':
    unittest.main()
