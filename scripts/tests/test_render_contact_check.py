"""Absolute fallback acceptance may never use inactive or undrawn surfaces."""
import copy
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import render_contact_check as c
import test_render_model_dependencies as fixtures
from test_render_collision_check import glb,edit_glb_document
from test_render_terrain_dependencies import ktx

class ContactTests(unittest.TestCase):
    def setUp(self):
        fixtures.ModelDependencyTests.setUp(self)
        self.identity=(0,0)
        for scene in self.scenes:
            path=scene.folder/'models/m.glb'
            scene.instances={self.identity:dict(name='wall',**{'class':'wall'},rotation=[[1,0,0],[0,1,0],[0,0,1]],translation=[0,0,0])}
            scene.resolve=lambda inst,p=path:('g',p)
            scene.resolve_class=lambda inst:dict(name='wall',base='prop',geometry='g',collision=[])
            scene.collision_policy=lambda inst:dict(base='prop',collision=[])
            scene.collision_identity=lambda g:'identical'
            scene.render_states=lambda g,inst=None,p=path:dict(model=(p,),lowd=(p,))
            scene.collisions={'g':(scene.folder,dict(glb='collision/c.glb',prims=[]))}
            glb(scene.folder/'collision/c.glb',[(0,0,0),(6,0,0),(0,0,6)],[0,1,2])
            scene.borrow_lifetime=lambda inst:dict(eligible=True,class_inputs_sha256='a'*64,class_files=1)
        # Reference has an authored gap of five metres. Source exactly fits C.
        glb(self.a.folder/'models/m.glb',[(0,5,0),(6,5,0),(0,5,6)],[0,1,2])
    def contact(self):
        return c.absolute_contact(self.a,self.b,self.identity,[self.b.folder/'models/m.glb'],.01,10000)
    def test_collision_fit_can_pass_even_when_reference_art_is_five_metres_away(self):
        receipt=self.contact();self.assertEqual(receipt['status'],'pass',receipt)
        self.assertEqual(c.contact_problems(receipt,.01),[])
        self.assertLessEqual(receipt['residual_increase_certified_upper_m'],.01)
    def test_source_with_real_gap_cannot_pass_absolute_contact(self):
        glb(self.b.folder/'models/m.glb',[(0,2,0),(6,2,0),(0,2,6)],[0,1,2])
        self.assertEqual(self.contact()['status'],'unmeasured')
    def test_vehicle_mask_body_is_retained_but_not_in_actual_world_contact_union(self):
        prim=dict(name='vehicle',kind='box',mask=2,dims=[3,1,3],rotation=[[1,0,0],[0,1,0],[0,0,1]],translation=[0,10,0])
        self.a.collisions['g'][1]['prims']=[prim]
        surface=c.ContactSurface(self.a,self.identity)
        self.assertFalse(surface.catalog[-1]['runtime_built'])
        self.assertEqual(len(surface.catalog),2)
        glb(self.b.folder/'models/m.glb',[(-3,11,-3),(3,11,-3),(-3,11,3)],[0,1,2])
        self.assertEqual(self.contact()['status'],'unmeasured')
    def test_opaque_missing_binding_cannot_supply_absolute_contact(self):
        edit_glb_document(self.b.folder/'models/m.glb',lambda doc:doc['materials'][0].update(name='noicon'))
        self.b.doc['textures']=[]
        self.assertEqual(self.contact()['status'],'unmeasured')
    def test_partial_alpha_cannot_supply_guaranteed_contact_coverage(self):
        edit_glb_document(self.b.folder/'models/m.glb',lambda doc:doc['materials'][0].update(name='noicon',alphaMode='MASK'))
        (self.b.folder/'textures/noicon.ktx2').write_bytes(ktx(alpha=(0,255,0,255)))
        self.assertEqual(self.contact()['status'],'unmeasured')
    def test_camera_fading_lifetime_or_unproved_query_frame_fail_closed(self):
        self.b.instances[self.identity]['rotation']=[[10,0,-6],[0,1,0],[6,0,10]]
        self.assertEqual(self.contact()['status'],'unmeasured')
        self.b.instances[self.identity]['rotation']=[[1,0,0],[0,1,0],[0,0,1]]
        self.b.borrow_lifetime=lambda inst:dict(eligible=False)
        self.assertEqual(self.contact()['status'],'unmeasured')
    def test_nontrivial_rigid_placement_can_be_proved_with_interval_query_frames(self):
        for scene in self.scenes:
            scene.instances[self.identity]['rotation']=[[0.8,0,-0.6],[0,1,0],[0.6,0,0.8]]
        row=self.contact();self.assertEqual(row['status'],'pass',row)
        self.assertEqual(c.contact_problems(row,.01),[])

    def test_source_contact_policy_allows_a_small_exact_contact_model(self):
        for scene in self.scenes:
            glb(scene.folder/'models/m.glb',[(0,0,0),(2,0,0),(0,0,2)],[0,1,2])
            glb(scene.folder/'collision/c.glb',[(0,0,0),(2,0,0),(0,0,2)],[0,1,2])
        edit_glb_document(self.b.folder/'models/m.glb',lambda doc:doc['meshes'][0].update(extras={'contact_surface':True}))
        self.assertEqual(self.contact()['status'],'pass')

    def test_complete_rotated_cylinder_proxy_can_certify_opaque_donor_art(self):
        from render_contact_proxy import analytic_proxy
        from render_collision_check import placed
        prim=dict(name='pipe',kind='cylinder',mask=17,dims=[1.,6.,0.],
                  rotation=[[.8,0,-.6],[0,1,0],[.6,0,.8]],translation=[0,0,0])
        self.a.collisions['g'][1].update(glb=None,prims=[prim])
        triangles,_error=analytic_proxy('cylinder',(1.,6.,0.),.00144)
        vertices=[placed(p,prim) for triangle in triangles for p in triangle]
        glb(self.b.folder/'models/m.glb',vertices,list(range(len(vertices))))
        edit_glb_document(self.b.folder/'models/m.glb',lambda doc:doc['meshes'][0].update(extras={'contact_surface':True}))
        row=self.contact();self.assertEqual(row['status'],'pass',row)
        self.assertEqual(c.contact_problems(row,.01),[])
        self.assertGreater(row['analytic_proxy_error_m'],0.)
        self.assertLess(row['analytic_proxy_error_m'],.001)

    def test_ordnance_only_cage_cannot_supply_player_contact_coverage(self):
        from render_contact_proxy import proxy_surface
        prim=dict(name='bolt_only',kind='box',mask=16.,dims=[3.,1.,3.],
                  rotation=[[1,0,0],[0,1,0],[0,0,1]],translation=[0.,10.,0.])
        self.a.collisions['g'][1]['prims']=[prim]
        proxy,_error,_catalog,_frames=proxy_surface(self.a,self.identity,.01)
        points=[p for triangle in proxy.triangles for p in triangle]
        glb(self.b.folder/'models/m.glb',points,list(range(len(points))))
        row=self.contact()
        self.assertEqual(row['status'],'unmeasured')

    def test_every_matching_placement_contributes_to_the_worst_budget(self):
        other=(1,0)
        for scene in self.scenes:
            inst=copy.deepcopy(scene.instances[self.identity]);inst['name']='far_instance';inst['translation']=[200000.,0.,0.]
            scene.instances[other]=inst
        row=self.contact()
        self.assertEqual(row['status'],'unmeasured')
        self.assertIn('consume absolute contact budget',row['reason'])

    def test_a_new_model_file_invalidates_a_cached_contact_certificate(self):
        self.assertEqual(self.contact()['status'],'pass')
        glb(self.b.folder/'models/m.glb',[(0,4,0),(6,4,0),(0,4,6)],[0,1,2])
        self.assertEqual(self.contact()['status'],'unmeasured')

    def test_layer_order_and_case_match_actual_runtime(self):
        bodies=[('collisionmesh',0),('a',0),('b',2)]
        self.assertEqual(c.layer_bodies(bodies,[('soldier','a')],'soldier'),[False,True,False])
        self.assertEqual(c.layer_bodies(bodies,[('Soldier','a')],'soldier'),[True,True,False])
        self.assertEqual(c.layer_bodies(bodies,[('soldier','a'),('soldier','clear'),('soldier','collisionmesh')],'soldier'),[True,False,False])
        self.assertEqual(c.layer_bodies(bodies,[('vehicle_only','a')],'ordnance'),[True,False,False])
    def test_receipt_cannot_omit_catalog_or_numeric_guard(self):
        row=self.contact();row['body_catalog']=[];self.assertTrue(c.contact_problems(row,.01))
        row=self.contact();row.pop('numeric_guard_m');self.assertTrue(c.contact_problems(row,.01))
