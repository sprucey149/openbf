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
        obj = dict(status='pass', surface_certificate=exact, render_state_certificates={'model': exact, 'lowd': exact},
                   scene_surface_certificates={'model': exact, 'lowd': exact},
                   opacity_certificates={'model': opacity, 'lowd': opacity}, lod_switch_identical=True,
                   collision_identical=True, class_collision_identical=True, camera_visibility_identical=True)
        ancillary = dict(status='pass', complete=True)
        self.receipt = dict(schema=SCHEMA, complete=True, method=METHOD, tolerance_m=.01, margin_reason=MARGIN_REASON,
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
                      'render_attachment_check': SimpleNamespace(attachment_problems=ancillary_problems)}
        with patch.object(gates, 'sh', side_effect=zero_helper), patch.dict(sys.modules, companions):
            self.g.g_alignment()
        return self.g.rows[-1]['verdict']

    def test_zero_helper_without_output_fails(self):
        self.assertEqual(self.execute(), 'FAIL')

    def test_truncated_json_fails(self):
        self.assertEqual(self.execute(raw='{"complete":'), 'FAIL')

    def test_partial_report_fails(self):
        self.receipt['complete'] = False
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
