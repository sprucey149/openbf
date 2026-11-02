import math
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import tempfile

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import render_terrain_check as t
import render_terrain_dependencies as dependencies
from test_render_terrain_dependencies import glb, ktx


class TerrainProofTests(unittest.TestCase):
    def test_polygon_difference_does_not_turn_overlaps_into_missing_ground(self):
        square=[[0,0,0],[2,0,0],[2,0,2],[0,0,2]]
        faces=t.triangles([square])
        self.assertEqual(t.difference([square],faces+faces),[])
        half=[[0,0,0],[1,0,0],[1,0,2],[0,0,2]]
        self.assertAlmostEqual(sum(t.area(p) for p in t.difference([square],[half])),2.0)

    def test_coplanar_cutter_boundary_survives_once(self):
        poly=[[0,0,0],[1,0,0],[0,0,1]]
        cutter={'planes':[[0,1,0,0],[1,0,0,-0.5]]}
        result=t.subtract(poly,[cutter])
        self.assertEqual(len(result),1)
        self.assertEqual(t.area(result[0]),0.5)

    def test_continuous_partition_finds_an_interior_hill_missed_by_vertices(self):
        positions=[[x,2.0 if (x,z)==(1,1) else 0.0,z] for z in range(3) for x in range(3)]
        grid=t.Grid(positions)
        tri=[[0,0,0],[0,0,2],[2,0,2]]
        self.assertTrue(all(p[1]==grid.point(int(p[0]),int(p[2]))[1] for p in tri))
        worst=0.0
        for key,poly in t.partitions(tri,grid):
            target=t.plane(grid.triangle(key))
            worst=max(worst,max(abs(t.height(p,target)-p[1]) for p in poly))
        self.assertEqual(worst,2.0)

    def test_split_at_each_alternating_diagonal(self):
        grid=t.Grid([[x,0.0,z] for z in range(3) for x in range(3)])
        for z in range(2):
            for half in (0,1):
                key=(0,z,half)
                result=list(t.partitions(grid.triangle(key),grid))
                self.assertEqual(result,[(key,grid.triangle(key))])

    def test_a_small_area_cannot_hide_a_hole_wider_than_the_margin(self):
        r=0.012
        hole=[[-r,0,-r],[r,0,-r],[r,0,r],[-r,0,r]]
        self.assertLess(t.area(hole),8*8*1e-5)
        def rectangle(x0,z0,x1,z1):return [[x0,0,z0],[x1,0,z0],[x1,0,z1],[x0,0,z1]]
        candidates=t.triangles([rectangle(-1,-1,-r,1),rectangle(r,-1,1,1),
                                rectangle(-r,-1,r,-r),rectangle(-r,r,r,1)])
        proof=t.coverage_certificate([hole],candidates)
        self.assertFalse(proof['complete'])

    def test_long_micron_seam_is_certified_by_convex_distance_not_area(self):
        seam=[[0,0,-0.00002],[8,0,-0.00002],[8,0,0],[0,0,0]]
        candidates=[[[0,0,0],[8,0,0],[8,0,8]]]
        proof=t.coverage_certificate([seam],candidates)
        self.assertTrue(proof['complete'])
        self.assertLessEqual(proof['upper_bound_m'],0.00002)

    def test_vertical_reference_facet_has_a_continuous_3d_bound(self):
        grid=t.Grid([[x,0.0,z] for z in range(2) for x in range(2)])
        tri=[[0,0.005,0],[0,0.005,1],[0,0.004,0.5]]
        self.assertIsNone(t.plane(tri))
        self.assertTrue(t.nonempty_3d(tri))
        proof=t.vertical_bound(tri,grid,[])
        self.assertTrue(proof['complete'])
        self.assertGreater(proof['fragments'],0)
        self.assertAlmostEqual(proof['upper_bound_m'],0.005)

    def test_uniform_grid_rejects_nan_and_missing_vertices(self):
        with self.assertRaises(ValueError):t.Grid([[0,0,0],[1,0,0],[0,0,1]])
        with self.assertRaises(ValueError):t.Grid([[0,0,0],[1,0,0],[0,math.nan,1],[1,0,1]])

    def test_an_improvement_has_zero_excess_and_a_valid_receipt(self):
        manifest={'terrain_glb':'base.glb','terrain_cutters':[]}
        measured={'triangles':1,'vertical_triangles':0,'vertical_complete':True,'worst_m':0.001,
                  'height_worst_m':0.001,'worst_at':[0,0.001,0],'missing_area_m2':0.0,'overcut_area_m2':0.0,
                  'coverage_complete':True,'coverage_upper_bound_m':0.0}
        reference=({'worst_m':0.01,'missing_area_m2':0.0,'overcut_area_m2':0.0},{(0,0,0):0.01})
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            for source in ('reference','donor'):
                folder=root/source/'map';folder.mkdir(parents=True)
                (folder/'manifest.ron').write_text('()')
                (folder/'base.glb').write_bytes(source.encode())
            with patch.object(t,'read_ron',return_value=manifest), \
                 patch.object(t,'reference_data',return_value=(manifest,SimpleNamespace(n=2,unit=1),{'terrain_base':reference})), \
                 patch.object(t,'measure',return_value=(measured,{(0,0,0):0.001})), \
                 patch.object(t,'terrain_dependencies',return_value=dependencies.terrain_dependencies(root,root,'absent',{},{}),create=True):
                receipt=t.terrain_map(root/'reference',root/'donor','map')
        self.assertEqual(receipt['surfaces'][0]['excess_m'],0.0)
        self.assertEqual(receipt['status'],'pass')
        self.assertEqual(t.terrain_problems(receipt),[])

    def test_truthy_string_cannot_exempt_a_large_donor_gap(self):
        row={'name':'terrain_base','status':'pass','complete':True,'triangles':1,
             'worst_m':100.0,'baseline_m':100.0,'excess_m':0.0,'missing_area_m2':0.0,
             'overcut_area_m2':0.0,'reference_bytes_identical':'false',
             'coverage_complete':True,'coverage_upper_bound_m':0.0}
        receipt={'schema':1,'status':'pass','complete':True,'reference':{'kind':'heightfield'},
                 'margin_m':0.01,'worst_excess_m':0.0,'surfaces':[row]}
        self.assertTrue(t.terrain_problems(receipt))

    def test_receipt_structure_does_not_turn_unknown_status_into_pass(self):
        self.assertTrue(t.terrain_problems({'schema':1,'complete':True,'status':'not_checked',
                                           'reference':{},'margin_m':0.01,'surfaces':[]}))


class TerrainDrawIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.ref=self.root/'ref';self.src=self.root/'src'
        self.manifest={'source_game':'bf1','terrain_glb':'base.glb','lowres_glb':'far.glb',
                       'terrain_cutters':[],'textures':['ground','far_tex'],
                       'sky':{'terrain_enable':1.0,'low_res_terrain':'far_tex','low_res_max_distance':2000.0},
                       'lighting':{'near_scene_range':[0.0,100.0,150.0],'far_scene_range':1000.0}}
        for root in (self.ref,self.src):
            folder=root/'map';(folder/'textures').mkdir(parents=True)
            (folder/'manifest.ron').write_text('()')
            (folder/'base.glb').write_bytes(glb(grid=True))
            (folder/'far.glb').write_bytes(glb('ignored_far_binding',grid=True))
            for name in ('ground','far_tex'):(folder/'textures'/f'{name}.ktx2').write_bytes(ktx())

    def check(self,source=None):
        source=source or self.manifest
        with patch.object(t,'read_ron',side_effect=lambda path: source if path.parent==self.src/'map' else self.manifest):
            return t.terrain_map(self.ref,self.src,'map')

    def test_identical_geometry_cannot_hide_disabled_terrain(self):
        source=copy.deepcopy(self.manifest);source['sky']['terrain_enable']=0
        result=self.check(source)
        self.assertEqual(result['status'],'offender')
        self.assertEqual(result['dependencies']['status'],'offender')

    def test_existing_texture_file_must_be_in_the_loaded_registry(self):
        source=copy.deepcopy(self.manifest);source['textures']=['far_tex']
        self.assertTrue((self.src/'map/textures/ground.ktx2').is_file())
        self.assertEqual(self.check(source)['status'],'offender')

    def test_opaque_far_geometry_can_disappear_through_texture_alpha(self):
        (self.src/'map/textures/far_tex.ktx2').write_bytes(ktx((0,0,0,0),small=0))
        self.assertEqual(self.check()['status'],'offender')

    def test_passing_terrain_receipt_requires_render_dependencies(self):
        receipt=self.check()
        self.assertEqual(receipt['status'],'pass')
        self.assertEqual(t.terrain_problems(receipt),[])
        receipt.pop('dependencies',None)
        self.assertTrue(t.terrain_problems(receipt))

if __name__=='__main__':unittest.main()
