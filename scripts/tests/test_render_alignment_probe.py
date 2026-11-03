"""Practical contact samples discriminate shifts and separate query domains."""
import sys
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from render_alignment_probe import LayerContact, measure_layer
from render_collision_check import Surface


def plane(y):
    return Surface([(-2,y,-2),(2,y,-2),(2,y,2),(-2,y,2)], [0,1,2,0,2,3])


class FlatContact:
    def points(self, count):
        return [(0,0,0),(-1,0,-1),(1,0,1)]
    def distance(self, point):
        return abs(point[1])


class ProbeTests(unittest.TestCase):
    def test_uses_ps2_residual_instead_of_zero(self):
        result=measure_layer(FlatContact(),plane(1),plane(1.005),plane(1.005),32)
        self.assertEqual(result['status'],'sample_clear')
        self.assertAlmostEqual(result['worst']['extra_gap_m'],.005)

    def test_metres_of_missing_wall_fail(self):
        result=measure_layer(FlatContact(),plane(1),plane(3),plane(3),32)
        self.assertEqual(result['status'],'offender')
        self.assertAlmostEqual(result['worst']['extra_gap_m'],2)
        self.assertGreater(result['samples'],0)

    def test_missing_guaranteed_art_stays_unknown(self):
        self.assertEqual(measure_layer(FlatContact(),plane(0),None,plane(0),32)['status'],'unknown')

    def test_layers_do_not_borrow_each_others_bodies(self):
        class Scene:
            collisions={'geo':(Path(),{'prims':[
                dict(name='foot',mask=1.0,kind='sphere',dims=[1,0,0],translation=[0,0,0]),
                dict(name='bolt',mask=16.0,kind='sphere',dims=[2,0,0],translation=[4,0,0])]})}
            def resolve_class(self, instance):
                return dict(base='prop',collision=[('soldier','foot'),('ordnance','bolt')])
            def resolve(self, instance):
                return 'geo',None
        a=LayerContact(Scene(),{},'soldier')
        b=LayerContact(Scene(),{},'ordnance')
        self.assertEqual([p['name'] for p in a.primitives],['foot'])
        self.assertEqual([p['name'] for p in b.primitives],['bolt'])


if __name__=='__main__':unittest.main()
