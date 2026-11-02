import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import subprocess
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import render_terrain_dependencies as d


def ktx(alpha=(255,255,255,255), small=255, rgb=64):
    levels=[bytes(v for a in alpha for v in (rgb,rgb,rgb,a)),bytes([rgb,rgb,rgb,small])]
    count=2;dfd=80+count*24;payload=dfd+4
    header=d.KTX2+struct.pack('<9I',43,1,2,2,0,0,1,count,0)+struct.pack('<4I2Q',dfd,4,payload,0,0,0)
    index=b'';data=b''
    for level in levels:
        index+=struct.pack('<3Q',payload+len(data),len(level),len(level));data+=level
    return header+index+b'\0'*4+data


def glb(material='ground',shift=0.0,y=0.0,alpha=None,grid=False):
    positions=[(0.0,y,0.0),(1.0,0.0,0.0),(0.0,0.0,1.0)]
    if grid:positions.append((1.0,0.0,1.0))
    uv=[(p[0]+shift,p[2]) for p in positions]
    indices=[0,2,3,0,3,1] if grid else [0,1,2]
    values=[(struct.pack('<'+'f'*len(positions)*3,*(v for p in positions for v in p)),5126,'VEC3',len(positions),False),
            (struct.pack('<'+'f'*len(uv)*2,*(v for p in uv for v in p)),5126,'VEC2',len(uv),False),
            (struct.pack('<'+'I'*len(indices),*indices),5125,'SCALAR',len(indices),False)]
    if alpha is not None:values.append((bytes([255,255,255,alpha])*len(positions),5121,'VEC4',len(positions),True))
    binary=b'';views=[];accessors=[]
    for payload,component,kind,count,normalized in values:
        while len(binary)%4:binary+=b'\0'
        views.append({'buffer':0,'byteOffset':len(binary),'byteLength':len(payload)});binary+=payload
        a={'bufferView':len(views)-1,'componentType':component,'count':count,'type':kind}
        if normalized:a['normalized']=True
        accessors.append(a)
    attrs={'POSITION':0,'TEXCOORD_0':1}
    if alpha is not None:attrs['COLOR_0']=3
    doc={'asset':{'version':'2.0'},'buffers':[{'byteLength':len(binary)}],'bufferViews':views,'accessors':accessors,
         'materials':[{'name':material,'alphaMode':'OPAQUE'}],
         'meshes':[{'primitives':[{'attributes':attrs,'indices':2,'material':0,'mode':4}]}],
         'nodes':[{'mesh':0}],'scenes':[{'nodes':[0]}],'scene':0}
    encoded=json.dumps(doc,separators=(',',':')).encode()
    while len(encoded)%4:encoded+=b' '
    while len(binary)%4:binary+=b'\0'
    return struct.pack('<4sII',b'glTF',2,12+8+len(encoded)+8+len(binary))+struct.pack('<I4s',len(encoded),b'JSON')+encoded+struct.pack('<I4s',len(binary),b'BIN\0')+binary


class DrawDependencyTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.ref=self.root/'ref';self.src=self.root/'src'
        self.manifest={'source_game':'bf1','terrain_glb':'base.glb','lowres_glb':'far.glb','textures':['ground','far_tex'],
                       'sky':{'terrain_enable':1.0,'low_res_terrain':'far_tex','low_res_max_distance':2000.0},
                       'lighting':{'near_scene_range':[0.0,100.0,150.0],'far_scene_range':1000.0}}
        for root in (self.ref,self.src):
            folder=root/'map';(folder/'textures').mkdir(parents=True)
            (folder/'base.glb').write_bytes(glb())
            (folder/'far.glb').write_bytes(glb('this_material_is_not_the_far_binding'))
            for name in ('ground','far_tex'):(folder/'textures'/f'{name}.ktx2').write_bytes(ktx())

    def check(self,reference=None,source=None):
        receipt=d.terrain_dependencies(self.ref,self.src,'map',reference or self.manifest,source or self.manifest)
        self.assertEqual(d.dependency_problems(receipt),[])
        return receipt

    def test_enable_and_opt_in_range_policy_are_measured(self):
        source=copy.deepcopy(self.manifest);source['sky']['terrain_enable']=0
        self.assertEqual(self.check(source=source)['status'],'offender')
        source=copy.deepcopy(self.manifest);source['lighting']['near_scene_range'][2]=151
        self.assertEqual(self.check(source=source)['status'],'offender')
        reference=copy.deepcopy(self.manifest);source=copy.deepcopy(self.manifest)
        reference['source_game']=source['source_game']='bf2';source['sky']['terrain_enable']=0
        self.assertEqual(self.check(reference,source)['status'],'pass')

    def test_extra_far_art_needs_a_direct_canonical_geometry_proof(self):
        reference=copy.deepcopy(self.manifest);reference['lowres_glb']=None
        (self.ref/'map/base.glb').write_bytes(glb(grid=True))
        (self.src/'map/base.glb').write_bytes(glb(grid=True))
        (self.src/'map/far.glb').write_bytes(glb('ignored',grid=True))
        receipt=self.check(reference,self.manifest)
        self.assertEqual(receipt['status'],'pass')
        self.assertTrue(any(r['kind']=='far_geometry' for r in receipt['records']))
        (self.src/'map/far.glb').write_bytes(glb('ignored',grid=True,y=0.02))
        self.assertEqual(self.check(reference,self.manifest)['status'],'offender')

    def test_a_file_outside_loaded_name_registry_does_not_bind(self):
        source=copy.deepcopy(self.manifest);source['textures']=['far_tex']
        self.assertTrue((self.src/'map/textures/ground.ktx2').is_file())
        self.assertEqual(self.check(source=source)['status'],'offender')

    def test_common_fallback_and_lowrez_alias_match_runtime(self):
        manifest=copy.deepcopy(self.manifest);manifest['textures']=['far_tex']
        for root in (self.ref,self.src):
            (root/'map/base.glb').write_bytes(glb('GROUND_LOWREZ'))
            common=root/'common_bf1';(common/'textures').mkdir(parents=True)
            (common/'common.ron').write_text('(models: [], textures: ["Ground"], collision: [])')
            (common/'textures/Ground.ktx2').write_bytes(ktx())
        receipt=self.check(manifest,manifest)
        self.assertEqual(receipt['status'],'pass')
        base=[r for r in receipt['records'] if r['kind']=='base_texture']
        self.assertTrue(all(r['source']['owner']=='common' for r in base))

    def test_map_names_shadow_common_and_tier2_is_selected_by_file(self):
        source=copy.deepcopy(self.manifest);source['texture_tier2']=[]
        (self.src/'map/textures/far_tex.tier2.ktx2').write_bytes(ktx((0,0,0,0),small=0))
        receipt=self.check(source=source)
        self.assertEqual(receipt['status'],'offender')
        far={r['tier']:r for r in receipt['records'] if r['kind']=='far_alpha'}
        self.assertEqual(far['ps2']['status'],'pass');self.assertEqual(far['pc']['status'],'offender')

    def test_far_sheet_opaque_glb_still_discards_transparent_texture(self):
        (self.src/'map/textures/far_tex.ktx2').write_bytes(ktx((0,0,0,0),small=0))
        self.assertEqual(self.check()['status'],'offender')

    def test_every_loaded_mip_and_vertex_alpha_must_cover(self):
        (self.src/'map/textures/far_tex.ktx2').write_bytes(ktx(small=0))
        self.assertNotEqual(self.check()['status'],'pass')
        (self.src/'map/textures/far_tex.ktx2').write_bytes(ktx())
        (self.src/'map/far.glb').write_bytes(glb('ignored',alpha=0))
        self.assertEqual(self.check()['status'],'offender')

    def test_alpha_32_is_sufficient_and_rgb_may_differ(self):
        (self.src/'map/textures/far_tex.ktx2').write_bytes(ktx((32,32,32,32),small=32,rgb=240))
        (self.src/'map/far.glb').write_bytes(glb('ignored',y=0.1))
        self.assertEqual(self.check()['status'],'pass')

    def test_native_nonuniform_alpha_identity_survives_rgb_changes(self):
        (self.ref/'map/textures/far_tex.ktx2').write_bytes(ktx((0,255,0,255),small=128,rgb=24))
        (self.src/'map/textures/far_tex.ktx2').write_bytes(ktx((0,255,0,255),small=128,rgb=240))
        self.assertEqual(self.check()['status'],'pass')

    def test_nonuniform_alpha_with_changed_uv_or_geometry_is_unmeasured(self):
        for root in (self.ref,self.src):(root/'map/textures/far_tex.ktx2').write_bytes(ktx((0,255,0,255),small=128))
        (self.src/'map/far.glb').write_bytes(glb('ignored',shift=0.25))
        self.assertEqual(self.check()['status'],'unmeasured')
        (self.src/'map/far.glb').write_bytes(glb('ignored',y=0.01))
        self.assertEqual(self.check()['status'],'unmeasured')

    def test_missing_texture_and_malformed_metadata_fail_closed(self):
        path=self.src/'map/textures/ground.ktx2';path.unlink()
        self.assertEqual(self.check()['status'],'unmeasured')
        raw=bytearray(ktx());struct.pack_into('<I',raw,48,0xffffffff);path.write_bytes(raw)
        self.assertEqual(self.check()['status'],'unmeasured')

    def test_cannot_drop_texture_proofs_or_direct_far_proof(self):
        receipt=self.check();self.assertEqual(receipt['status'],'pass')
        for kind in ('base_texture','far_texture','far_alpha'):
            broken=copy.deepcopy(receipt);broken['records']=[r for r in broken['records'] if r['kind']!=kind]
            self.assertTrue(d.dependency_problems(broken),kind)
        reference=copy.deepcopy(self.manifest);reference['lowres_glb']=None
        (self.ref/'map/base.glb').write_bytes(glb(grid=True))
        (self.src/'map/base.glb').write_bytes(glb(grid=True))
        (self.src/'map/far.glb').write_bytes(glb('ignored',grid=True))
        extra=self.check(reference,self.manifest);self.assertEqual(extra['status'],'pass')
        extra['records']=[r for r in extra['records'] if r['kind']!='far_geometry']
        self.assertTrue(d.dependency_problems(extra))

    def test_hostile_ktx_headers_are_bounded_under_watchdog(self):
        root=Path(self.temp.name)/'hostile';root.mkdir();paths=[]
        for index,offset in enumerate((20,24,40,44,48,52,56,60,64,72,80,88,96,104,112,120)):
            raw=bytearray(ktx());struct.pack_into('<I',raw,offset,0xffffffff)
            path=root/f'{index}.ktx2';path.write_bytes(raw);paths.append(str(path))
        code="""import sys
sys.path.insert(0,sys.argv[1])
from render_terrain_dependencies import _ktx_descriptor
for name in sys.argv[2:]:
 try:_ktx_descriptor(name)
 except (ValueError,OSError):continue
 raise SystemExit('hostile header was accepted: '+name)
"""
        result=subprocess.run([sys.executable,'-c',code,str(Path(d.__file__).parent),*paths],capture_output=True,text=True,timeout=3)
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)

    def test_cannot_omit_a_tier_or_flag_variant_from_passing_receipt(self):
        receipt=self.check();self.assertEqual(receipt['status'],'pass')
        broken=copy.deepcopy(receipt);broken['tiers']=['pc']
        self.assertTrue(d.dependency_problems(broken))
        broken=copy.deepcopy(receipt);broken['records']=broken['records'][1:]
        self.assertTrue(d.dependency_problems(broken))


if __name__=='__main__':unittest.main()
