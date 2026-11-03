"""Actual draw presence and alpha coverage, including hostile registry changes."""
import copy
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import render_model_dependencies as d
from test_render_terrain_dependencies import ktx
from test_render_collision_check import glb,edit_glb_document

class ModelDependencyTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.scenes=[]
        for name,reference in (('ref',True),('src',False)):
            root=self.root/name;folder=root/'map';(folder/'models').mkdir(parents=True);(folder/'textures').mkdir()
            scene=SimpleNamespace(assets=root,folder=folder,reference_art=reference,
                                  doc={'source_game':'bf1','models':['m'],'textures':['noicon']})
            glb(folder/'models/m.glb',[(0,0,0),(6,0,0),(0,0,6)],[0,1,2],alpha_mode='OPAQUE')
            edit_glb_document(folder/'models/m.glb',lambda doc:doc['materials'][0].update(name='noicon'))
            (folder/'textures/noicon.ktx2').write_bytes(ktx())
            self.scenes.append(scene)
        self.a,self.b=self.scenes
    def check(self):
        r=d.model_dependencies(self.a,self.b,[self.a.folder/'models/m.glb'],[self.b.folder/'models/m.glb'])
        self.assertEqual(d.dependency_problems(r),[]);return copy.deepcopy(r)
    def mode(self,value):
        for scene in self.scenes:
            edit_glb_document(scene.folder/'models/m.glb',lambda doc:doc['materials'][0].update(alphaMode=value))
    def test_opaque_loaded_file_removed_from_registry_cannot_pass(self):
        self.b.doc['textures']=[]  # File still exists. Actual renderer skips noicon.
        r=self.check();self.assertEqual(r['status'],'unmeasured')
        self.assertEqual(r['source'][0]['records'][0]['route'],'dangling_skip')
        self.assertFalse(r['source'][0]['borrow_opaque'])
    def test_unregistered_model_file_cannot_supply_a_surface(self):
        self.b.doc['models']=[]
        with self.assertRaisesRegex(ValueError,'not the model loaded'):
            self.check()
    def test_rgb_only_texture_change_preserves_nonuniform_alpha_cutout(self):
        self.mode('MASK')
        for scene,rgb in zip(self.scenes,(8,240)):
            (scene.folder/'textures/noicon.ktx2').write_bytes(ktx(alpha=(0,255,0,255),rgb=rgb))
        r=self.check();self.assertEqual(r['status'],'pass');self.assertFalse(r['source'][0]['borrow_opaque'])
    def test_changed_alpha_or_last_mip_prevents_a_coverage_pass(self):
        self.mode('MASK')
        (self.b.folder/'textures/noicon.ktx2').write_bytes(ktx(small=0))
        self.assertEqual(self.check()['status'],'unmeasured')
    def test_fully_covered_mask_ignores_texture_rgb_and_size(self):
        self.mode('MASK')
        (self.b.folder/'textures/noicon.ktx2').write_bytes(ktx(rgb=254))
        self.assertEqual(self.check()['status'],'pass')
        self.assertTrue(self.check()['source'][0]['all_surfaces_covered'])
    def test_reference_never_uses_enhanced_alpha_as_baseline(self):
        self.mode('MASK')
        (self.a.folder/'textures/noicon.tier2.ktx2').write_bytes(ktx(alpha=(0,255,0,255)))
        r=d.model_dependencies(self.a,SimpleNamespace(**dict(vars(self.a),reference_art=False)),
                               [self.a.folder/'models/m.glb'],[self.a.folder/'models/m.glb'])
        self.assertEqual(r['status'],'unmeasured')
    def test_named_unloaded_material_is_drawn_gray_unless_census_skips(self):
        self.b.doc['textures']=[]
        edit_glb_document(self.b.folder/'models/m.glb',lambda doc:doc['materials'][0].update(name='real_missing'))
        r=self.check();self.assertEqual(r['status'],'pass')
        self.assertEqual(r['source'][0]['records'][0]['route'],'lookup_failed_gray')
        self.assertTrue(r['source'][0]['borrow_opaque'])
    def test_lowrez_lookup_uses_registered_full_sheet(self):
        edit_glb_document(self.b.folder/'models/m.glb',lambda doc:doc['materials'][0].update(name='noicon_lowrez'))
        self.assertEqual(self.check()['status'],'pass')
    def test_same_skip_is_baseline_identity_but_never_borrowed(self):
        for scene in self.scenes:scene.doc['textures']=[]
        r=self.check();self.assertEqual(r['status'],'pass')
        self.assertFalse(r['source'][0]['all_surfaces_covered']);self.assertFalse(r['source'][0]['borrow_opaque'])
    def test_receipt_cannot_drop_tier_or_forge_coverage(self):
        r=self.check();r['source'][0]['records'].pop()
        self.assertTrue(d.dependency_problems(r))
        r=self.check();r['source'][0]['records'][0]['coverage']='absent'
        self.assertTrue(d.dependency_problems(r))
    def add_common_bad_uv(self):
        from test_render_terrain_dependencies import glb as textured_glb
        import struct,json
        common=self.b.assets/'common_bf1';(common/'models').mkdir(parents=True)
        (common/'common.ron').write_text('(models: ["unrelated_uv_model"], textures: [], collision: [])')
        raw=bytearray(textured_glb());size=struct.unpack_from('<I',raw,12)[0]
        doc=json.loads(raw[20:20+size]);part=doc['meshes'][0]['primitives'][0]
        accessor=doc['accessors'][part['attributes']['TEXCOORD_0']]
        start=doc['bufferViews'][accessor['bufferView']].get('byteOffset',0)
        struct.pack_into('<f',raw,20+size+8+start,float('nan'))
        path=common/'models/unrelated_uv_model.glb';path.write_bytes(raw)
        return path

    def test_loadable_unrelated_nan_uv_does_not_invent_a_common_abort(self):
        self.add_common_bad_uv()
        row=self.check();self.assertEqual(row['status'],'pass')
        notes=row['source'][0]['model_load_notes']['ps2']
        self.assertEqual(len(notes),1)
        self.assertEqual(notes[0]['nonfinite_uv_rows'],1)
        self.assertEqual(notes[0]['world_instance_ids'],[])

    def test_actually_selected_nan_uv_model_still_fails_its_own_draw_receipt(self):
        path=self.add_common_bad_uv()
        with self.assertRaisesRegex(ValueError,'texture coordinates'):
            d.model_descriptor(path,self.b)

    def test_invalid_common_render_grid_cannot_supply_phantom_common_textures(self):
        path=self.add_common_bad_uv()
        edit_glb_document(path,lambda doc:doc['meshes'][0].update(extras={'render_grid_offset':-1}))
        with self.assertRaisesRegex(ValueError,'uncertified'):
            self.check()

    def test_actual_common_abort_cannot_supply_an_opaque_noicon_binding(self):
        common=self.b.assets/'common_bf1';(common/'models').mkdir(parents=True);(common/'textures').mkdir()
        (common/'common.ron').write_text('(models: ["grid_model"], textures: ["noicon"], collision: [])')
        glb(common/'models/grid_model.glb',[(0,0,0),(1,0,0),(0,0,1)],[0,1,2])
        edit_glb_document(common/'models/grid_model.glb',lambda doc:doc['meshes'][0].update(extras={'render_grid_offset':-1,'render_grid_size':2}))
        (common/'textures/noicon.ktx2').write_bytes(ktx())
        self.b.doc['textures']=[]
        with self.assertRaisesRegex(ValueError,'uncertified'):
            self.check()

    def test_unused_common_contact_metadata_must_still_be_a_strict_bool(self):
        for value in ('true',None):
            with self.subTest(value=value):
                path=self.add_common_bad_uv() if not (self.b.assets/'common_bf1').exists() else self.b.assets/'common_bf1/models/unrelated_uv_model.glb'
                edit_glb_document(path,lambda doc:doc['meshes'][0].update(extras={'contact_surface':value}))
                for name in ('draw_inventory_cache','model_dependency_cache'):
                    if hasattr(self.b,name):delattr(self.b,name)
                with self.assertRaisesRegex(ValueError,'uncertified'):
                    self.check()

    def test_census_route_matches_actual_renderer_list(self):
        import re
        text=(Path(__file__).resolve().parents[2]/'engine/crates/game/src/viewer/scene.rs').read_text()
        block=text.split('const DANGLING_ON_DISC: &[&str] = &[',1)[1].split('];',1)[0]
        self.assertEqual(tuple(re.findall(r'"([^"]+)"',block)),d.DANGLING)
