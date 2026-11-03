"""Native anomalies require immutable identity, not a tolerated bad value."""
import sys
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_render_collision_check import glb
from render_identity_baseline import native_anomaly_identity, identity_problems
from world_fingerprint import digest

class IdentityTests(unittest.TestCase):
    def fixture(self, root, reference=False):
        path = root / 'map/models/model.glb'
        glb(path, [(0,0,0),(1,0,0),(0,0,1)], [0,1,2])
        obj = SimpleNamespace(assets=root, folder=root/'map', reference_art=reference,
             doc=dict(source_game='bf1',models=['model'],textures=[]), classes={}, geometries={}, models={'m':path},
             instances={(0,0):dict(name='a',**{'class':'class'},translation=[0.,0.,0.])},texture_dirs=[root/'map/textures'],
             collisions={'g':(root,dict(prims=[dict(kind='box',dims=[-1,2,3])]))})
        obj.resolve=lambda inst: ('g',path)
        obj.render_states=lambda g,inst=None: {'model':(path,)}
        obj.collision_identity=lambda g: digest(obj.collisions[g][1])
        obj.collision_policy=lambda inst: dict(base='prop',collision=[],door=None,armed_nodes=None)
        obj.raw_classes=[];obj.unrelated_classes=[]
        obj.resolve_class=lambda inst:dict(name='class')
        obj.class_inputs=lambda:SimpleNamespace(sha256=digest([obj.raw_classes,obj.unrelated_classes]),files=1,
                                               chain=lambda name:(obj.raw_classes,'prop'))
        return obj
    def test_signed_native_body_retained_by_exact_input_identity(self):
        with tempfile.TemporaryDirectory() as name:
            scene=self.fixture(Path(name))
            row=native_anomaly_identity(SimpleNamespace(**dict(vars(scene),reference_art=True)),scene,(0,0),'invalid collision primitive dimensions: box')
            self.assertEqual(row['status'],'baseline_input_identity')
            self.assertEqual(identity_problems(row),[])
            self.assertFalse(row['absolute_visible_residual_measured'])
    def test_identical_detached_inputs_prove_native_baseline_across_roots(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name); a=self.fixture(root/'a',True); b=self.fixture(root/'b')
            row=native_anomaly_identity(a,b,(0,0),'invalid collision primitive dimensions: box')
            self.assertIsNotNone(row)
            self.assertFalse(row['reference_identical_input_root'])
            self.assertEqual(identity_problems(row),[])

    def test_changed_geometry_collision_policy_or_control_inputs_refuse(self):
        from test_render_collision_check import edit_glb_document
        mutations=[lambda b:b.instances[(0,0)]['translation'].__setitem__(0,1.),
                   lambda b:b.doc.update(lighting={'near_scene_range':[1.,2.,3.]}),
                   lambda b:b.collisions['g'][1]['prims'][0]['dims'].__setitem__(0,-2.),
                   lambda b:setattr(b,'collision_policy',lambda inst:dict(collision=[['soldier','none']])),
                   lambda b:b.raw_classes.append(dict(name='changed',props=[])),
                   lambda b:edit_glb_document(b.models['m'],lambda doc:doc['meshes'][0].update(extras={'contact_surface':True})),
                   lambda b:glb(b.models['m'],[(0,0,0),(1,0,0),(0,.001,1)],[0,1,2])]
        for mutation in mutations:
            with self.subTest(mutation=mutation),tempfile.TemporaryDirectory() as name:
                root=Path(name);a=self.fixture(root/'a',True);b=self.fixture(root/'b');mutation(b)
                self.assertIsNone(native_anomaly_identity(a,b,(0,0),'invalid collision primitive dimensions: box'))

    def test_signed_zero_control_change_is_not_erased_by_float_equality(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);a=self.fixture(root/'a',True);b=self.fixture(root/'b')
            b.instances[(0,0)]['translation'][0]=-0.
            self.assertEqual(a.instances,b.instances)
            self.assertIsNone(native_anomaly_identity(a,b,(0,0),'invalid collision primitive dimensions: box'))

    def test_unrelated_raw_class_files_are_recorded_without_changing_selected_chain(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);a=self.fixture(root/'a',True);b=self.fixture(root/'b')
            b.unrelated_classes.append(dict(name='unused_side_class'))
            row=native_anomaly_identity(a,b,(0,0),'invalid collision primitive dimensions: box')
            self.assertIsNotNone(row)
            self.assertNotEqual(row['raw_class_inventories']['reference']['sha256'],row['raw_class_inventories']['source']['sha256'])
            self.assertEqual(identity_problems(row),[])

    def test_receipt_requires_matching_physics_models_and_draw_states(self):
        import copy
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);a=self.fixture(root/'a',True);b=self.fixture(root/'b')
            row=native_anomaly_identity(a,b,(0,0),'invalid collision primitive dimensions: box')
            altered=copy.deepcopy(row);altered['input_identity_evidence']['source']['placements_sha256']='d'*64
            self.assertTrue(identity_problems(altered))
            altered=copy.deepcopy(row);altered['model_files']['model'][0]['source_sha256']='d'*64
            self.assertTrue(identity_problems(altered))
            altered=copy.deepcopy(row);altered['world_draw_dependencies'].clear()
            self.assertTrue(identity_problems(altered))
            altered=copy.deepcopy(row);altered['residual_increase_certified_upper_m']=False
            self.assertTrue(identity_problems(altered))
            altered=copy.deepcopy(row);altered['absolute_visible_residual_measured']=True
            self.assertTrue(identity_problems(altered))
            altered=copy.deepcopy(row);altered['index']=1
            self.assertTrue(identity_problems(altered))

    def test_changed_effective_alpha_refuses_separate_root_identity(self):
        from test_render_collision_check import edit_glb_document
        from test_render_terrain_dependencies import ktx
        with tempfile.TemporaryDirectory() as name:
            root=Path(name);a=self.fixture(root/'a',True);b=self.fixture(root/'b')
            for obj in (a,b):
                obj.doc['textures']=['cut'];(obj.folder/'textures').mkdir()
                (obj.folder/'textures/cut.ktx2').write_bytes(ktx())
                edit_glb_document(obj.models['m'],lambda doc:doc['materials'][0].update(name='cut',alphaMode='MASK'))
            (b.folder/'textures/cut.ktx2').write_bytes(ktx(alpha=(0,0,0,0),small=0))
            self.assertIsNone(native_anomaly_identity(a,b,(0,0),'invalid collision primitive dimensions: box'))
    def test_enhanced_override_prevents_missing_texture_identity_proof(self):
        with tempfile.TemporaryDirectory() as name:
            scene=self.fixture(Path(name))
            from test_render_collision_check import edit_glb_document
            edit_glb_document(scene.models['m'], lambda d:d.update(materials=[dict(name='skin',alphaMode='BLEND')]))
            (Path(name)/'map/textures').mkdir()
            scene.doc['textures']=['skin']
            (Path(name)/'map/textures/skin.tier2.ktx2').write_bytes(b'present')
            self.assertIsNone(native_anomaly_identity(SimpleNamespace(**dict(vars(scene),reference_art=True)),scene,(0,0),'nonopaque material has no measured texture: skin'))
    def test_lowrez_enhanced_fallback_cannot_bypass_native_identity(self):
        from test_render_collision_check import edit_glb_document
        from test_render_terrain_dependencies import ktx
        with tempfile.TemporaryDirectory() as name:
            scene=self.fixture(Path(name));scene.doc['textures']=['cut']
            textures=Path(name)/'map/textures';textures.mkdir()
            (textures/'cut.ktx2').write_bytes(ktx())
            (textures/'cut.tier2.ktx2').write_bytes(ktx(alpha=(0,0,0,0),small=0))
            edit_glb_document(scene.models['m'], lambda d:d.update(materials=[dict(name='cut_lowrez',alphaMode='MASK')]))
            edit_glb_document(scene.models['m'], lambda d:d['meshes'][0]['primitives'][0].update(material=0))
            ref=SimpleNamespace(**dict(vars(scene),reference_art=True))
            row=native_anomaly_identity(ref,scene,(0,0),'invalid collision primitive dimensions: box')
            self.assertIsNone(row)

    def test_unknown_error_and_forged_receipt_are_not_accepted(self):
        with tempfile.TemporaryDirectory() as name:
            scene=self.fixture(Path(name))
            self.assertIsNone(native_anomaly_identity(SimpleNamespace(**dict(vars(scene),reference_art=True)),scene,(0,0),'bad mesh'))
            row=native_anomaly_identity(SimpleNamespace(**dict(vars(scene),reference_art=True)),scene,(0,0),'invalid collision primitive dimensions: box')
            row['reference_identical_input_root']='true'
            self.assertTrue(identity_problems(row))
