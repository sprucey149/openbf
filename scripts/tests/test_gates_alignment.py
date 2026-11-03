"""The alignment gate must distrust a zero exit without a complete receipt."""
import copy
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gates
from render_collision_check import REQUIRED_MAPS, SCHEMA, METHOD, MARGIN_REASON


class AlignmentReceipt(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.g = object.__new__(gates.Gates)
        self.g.tree = str(self.root)
        self.g.out = str(self.root)
        self.g.env = {}
        self.g.rows = []
        self.g.alignment_reference = str(self.root / 'reference')
        self.g.alignment_cases = [f'xbox={self.root / "source"}']
        exact = dict(status='pass', complete=True, method='identical_indexed_surface', maximum_observed_m=0.)
        opacity = dict(status='pass', reference_sha256=[], source_sha256=[])
        obj = dict(status='pass',layer_ordinal=0,index=0,surface_certificate=exact, render_state_certificates={'model': exact, 'lowd': exact},
                   scene_surface_certificates={'model': exact, 'lowd': exact},
                   opacity_certificates={'model': opacity, 'lowd': opacity}, lod_switch_identical=True,
                   collision_identical=True, class_collision_identical=True, camera_visibility_identical=True)
        camera=dict(can_fade=False,footprint_m=6.,geometry_sha256='a'*64,contact_surface=False)
        obj['camera_visibility_certificate']=dict(status='pass',complete=True,method='source_visibility_superset',
                                                  reference=copy.deepcopy(camera),source=copy.deepcopy(camera))
        def model(tiers):
            records=[dict(tier=t,primitive=0,material='gray',route='lookup_failed_gray',coverage='full',
                          mode='OPAQUE',light='lit',texture=None,min_vertex_alpha=255,max_vertex_alpha=255,
                          shape_sha256='a'*64,scroll=[0.,0.]) for t in tiers]
            return dict(schema=1,complete=True,status='pass',tiers=tiers,records=records,model_registered=True,
                        all_surfaces_covered=True,borrow_opaque=True,indexed_geometry_sha256='b'*64,
                        visibility_signatures=[None],opacity_signature=None)
        draw=dict(schema=1,status='pass',complete=True,reference=[model(['ps2'])],source=[model(['ps2','pc'])])
        obj['world_draw_dependencies']={state:copy.deepcopy(draw) for state in ('model','lowd')}
        ancillary = dict(status='pass', complete=True)
        self.receipt = dict(schema=SCHEMA, complete=True, method=METHOD, tolerance_m=.01, margin_reason=MARGIN_REASON,
                            input_stability=dict(status='pass',complete=True),
                            reference=self.g.alignment_reference, source_paths={'xbox': str(self.root / 'source')},
                            maps=[dict(source='xbox', map=key, placements=1, extra_source_placements=0,
                                       counts={'pass': 1}, objects=[copy.deepcopy(obj)], terrain=copy.deepcopy(ancillary),
                                       attachment=copy.deepcopy(ancillary)) for key in REQUIRED_MAPS])

    def execute(self, receipt=None, raw=None):
        def zero_helper(*_args, **_kwargs):
            if receipt is not None:
                (self.root / 'alignment.json').write_text(json.dumps(receipt))
            elif raw is not None:
                (self.root / 'alignment.json').write_text(raw)
            return 0
        def ancillary_problems(value):
            return [] if (isinstance(value, dict) and value.get('status') in ('pass', 'offender', 'unmeasured')
                          and type(value.get('complete')) is bool) else ['invalid companion receipt']
        # Companion validators have their own targeted tests. These fixtures
        # isolate the gate's trust boundary, including absent/incomplete results.
        companions = {'render_terrain_check': SimpleNamespace(terrain_problems=ancillary_problems),
                      'render_attachment_check': SimpleNamespace(attachment_problems=ancillary_problems),
                      'render_input_stability':SimpleNamespace(problems=lambda value,*_args:
                          [] if value==dict(status='pass',complete=True) else ['missing stable inputs'])}
        with patch.object(gates, 'sh', side_effect=zero_helper), patch.dict(sys.modules, companions):
            self.g.g_alignment()
        return self.g.rows[-1]['verdict']

    def test_default_cli_runs_alignment(self):
        with patch.object(sys,'argv',['gates.py','--out',str(self.root)]), patch.object(gates,'run',return_value=[]) as run:
            with self.assertRaises(SystemExit) as result:
                gates.main()
        self.assertEqual(result.exception.code,0)
        self.assertIn('alignment',run.call_args.args[1])

    def test_landing_filter_keeps_automatically_required_alignment(self):
        import land
        self.assertEqual(land.landing_gate_groups({'map','alignment'}),['map','alignment'])

    def test_required_alignment_without_configured_roots_fails(self):
        self.g.alignment_reference=None
        self.g.g_alignment()
        self.assertEqual(self.g.rows[-1]['verdict'],'FAIL')

    def test_auto_groups_keeps_alignment_for_every_surface_consumer_change(self):
        for path in ('engine/crates/bake/src/lib.rs','engine/crates/game/src/viewer/scene.rs',
                     'engine/crates/core/src/collision.rs','engine/crates/render/src/main.wgsl',
                     'engine/crates/manifest/src/lib.rs','scripts/render_model_dependencies.py',
                     'engine/Cargo.lock','engine/Cargo.toml','scripts/data/terrain_play.json',
                     'scripts/version_terrain_play.py','scripts/world_fingerprint.py'):
            with self.subTest(path=path), patch.object(gates,'changed_files',return_value=[path]):
                self.assertIn('alignment',gates.auto_groups('unused','base')[0])

    def test_alignment_environment_supplies_legacy_landing_inputs(self):
        from types import SimpleNamespace
        import os
        args=SimpleNamespace(tree=str(self.root),target=str(self.root/'target'),out=str(self.root/'env'),
                             bake_groups=[],parity_allow=None,keep_goldens=False,golden_groups='none')
        with patch.dict(os.environ,OPENBF_ALIGNMENT_REFERENCE='/bakes/ps2',
                        OPENBF_ALIGNMENT_CASES='xbox=/bakes/xbox "pc1=/bakes/a pc source"'):
            g=gates.Gates(args)
        self.assertEqual(g.alignment_reference,'/bakes/ps2')
        self.assertEqual(g.alignment_cases,['xbox=/bakes/xbox','pc1=/bakes/a pc source'])

    def test_zero_helper_without_output_fails(self):
        self.assertEqual(self.execute(), 'FAIL')

    def test_missing_or_incomplete_input_stability_cannot_pass(self):
        self.receipt.pop('input_stability')
        self.assertEqual(self.execute(self.receipt),'FAIL')
        self.receipt['input_stability']=dict(status='pass',complete='true')
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def test_truncated_json_fails(self):
        self.assertEqual(self.execute(raw='{"complete":'), 'FAIL')

    def test_partial_report_fails(self):
        self.receipt['complete'] = False
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_camera_certificate_or_forged_superset_fails(self):
        obj=self.receipt['maps'][0]['objects'][0]
        saved=obj.pop('camera_visibility_certificate')
        self.assertEqual(self.execute(self.receipt),'FAIL')
        obj['camera_visibility_certificate']=saved
        saved['source']['footprint_m']=2.
        saved['source']['can_fade']=True
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def contact_fixture(self):
        from test_render_contact_check import ContactTests
        fixture=ContactTests();fixture.setUp();self.addCleanup(fixture.doCleanups)
        cert=fixture.contact();self.assertEqual(cert['status'],'pass')
        obj=self.receipt['maps'][0]['objects'][0]
        obj['scene_surface_certificates']={state:copy.deepcopy(cert) for state in ('model','lowd')}
        return obj

    def test_absolute_contact_receipt_passes_and_rejects_different_query_domains(self):
        obj=self.contact_fixture()
        self.assertEqual(self.execute(self.receipt),'PASS')
        for cert in obj['scene_surface_certificates'].values():cert['query_domains_identical']=False
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def test_edited_body_membership_cannot_hide_behind_an_identity_flag(self):
        from world_fingerprint import digest
        obj=self.contact_fixture()
        for cert in obj['scene_surface_certificates'].values():
            cert['body_catalog'][0]['ordnance']=False
            cert['body_catalog_sha256']=digest(cert['body_catalog'])
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def test_absolute_contact_must_cover_this_placed_object(self):
        obj=self.contact_fixture();obj['index']=1
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def test_world_gpu_certificate_is_required_and_covers_this_placement(self):
        from test_render_gpu_bounds import GpuBoundsTests
        obj=self.receipt['maps'][0]['objects'][0]
        cert=GpuBoundsTests().certificate()
        obj['scene_surface_certificates']={state:copy.deepcopy(cert) for state in ('model','lowd')}
        self.assertEqual(self.execute(self.receipt),'PASS')
        obj['index']=1
        self.assertEqual(self.execute(self.receipt),'FAIL')
        obj['index']=0
        for value in obj['scene_surface_certificates'].values():value.pop('world_gpu_bounds')
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def test_separate_root_native_identity_passes_but_cannot_cover_another_object(self):
        from test_render_identity_baseline import IdentityTests
        from render_identity_baseline import native_anomaly_identity
        a=IdentityTests().fixture(self.root/'native-a',True);b=IdentityTests().fixture(self.root/'native-b')
        baseline=native_anomaly_identity(a,b,(0,0),'invalid collision primitive dimensions: box')
        self.assertIsNotNone(baseline)
        row=self.receipt['maps'][0]
        row['objects'][0]['index']=1
        row['objects'].append(baseline);row['placements']=2
        row['counts']={'pass':1,'baseline_input_identity':1}
        self.assertEqual(self.execute(self.receipt),'PASS')
        baseline['index']=1
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def test_duplicate_object_cannot_replace_a_missing_object(self):
        row=self.receipt['maps'][0]
        row['objects'].append(copy.deepcopy(row['objects'][0]));row['placements']=2;row['counts']={'pass':2}
        self.assertEqual(self.execute(self.receipt),'FAIL')

    def test_missing_world_draw_dependencies_fails(self):
        self.receipt['maps'][0]['objects'][0].pop('world_draw_dependencies')
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_world_draw_tier_fails(self):
        self.receipt['maps'][0]['objects'][0]['world_draw_dependencies']['model']['source'][0]['records'].pop()
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_camera_visibility_correspondence_fails(self):
        self.receipt['maps'][0]['objects'][0].pop('camera_visibility_identical')
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_empty_report_fails(self):
        self.receipt['maps'] = []
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_map_fails(self):
        self.receipt['maps'].pop()
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_duplicate_map_fails(self):
        self.receipt['maps'][-1] = copy.deepcopy(self.receipt['maps'][0])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_extra_placements_fail(self):
        self.receipt['maps'][0]['extra_source_placements'] = 1
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_offender_fails_even_with_zero_exit(self):
        row = self.receipt['maps'][0]
        row.update(counts={'offender': 1}, objects=[{'status': 'offender'}])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_unmeasured_fails_even_with_zero_exit(self):
        row = self.receipt['maps'][0]
        row.update(counts={'unmeasured': 1}, objects=[{'status': 'unmeasured'}])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_inconsistent_counts_fail(self):
        self.receipt['maps'][0]['objects'][0]['status'] = 'offender'
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_visual_only_map_cannot_substitute_for_a_surface_measurement(self):
        row = self.receipt['maps'][0]
        row.update(counts={'no_render_geometry': 1}, objects=[{'status': 'no_render_geometry'}])
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_legacy_sampled_report_cannot_pass(self):
        self.receipt['schema'] = 1
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_sampled_method_cannot_pass_even_with_new_schema(self):
        self.receipt['method'] = 'sampled_bidirectional_render_collision_differential'
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_raised_margin_cannot_pass_build_gate(self):
        self.receipt['tolerance_m'] = .1
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_wrong_source_directory_cannot_pass(self):
        self.receipt['source_paths']['xbox'] += '-stale'
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_continuous_certificate_cannot_pass(self):
        del self.receipt['maps'][0]['objects'][0]['scene_surface_certificates']
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_collision_policy_identity_cannot_pass(self):
        del self.receipt['maps'][0]['objects'][0]['class_collision_identical']
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_lowd_certificate_cannot_pass(self):
        del self.receipt['maps'][0]['objects'][0]['scene_surface_certificates']['lowd']
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_terrain_certificate_cannot_pass(self):
        del self.receipt['maps'][0]['terrain']
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_missing_attachment_certificate_cannot_pass(self):
        del self.receipt['maps'][0]['attachment']
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_unmeasured_attachment_cannot_pass(self):
        self.receipt['maps'][0]['attachment'].update(status='unmeasured', complete=False)
        self.assertEqual(self.execute(self.receipt), 'FAIL')

    def test_complete_clean_report_passes(self):
        self.assertEqual(self.execute(self.receipt), 'PASS')
