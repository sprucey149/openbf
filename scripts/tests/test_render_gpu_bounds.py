"""Reject missing/forged world draw budgets; never sample placement tails."""
import copy
import math
from pathlib import Path
import sys
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import render_gpu_bounds as gpu
from render_collision_check import certificate_problems
from world_fingerprint import digest


class GpuBoundsTests(unittest.TestCase):
    def envelope(self):
        value=dict(local_coordinate_extent_m=6.,translation_extent_m=10.,matrix_row_sum=1.)
        value['error_m']=math.nextafter(32*2.**-23*17.,math.inf)
        return value

    def receipt(self):
        row=dict(identity=[0,0],reference=self.envelope(),source=self.envelope(),stretch_upper=1.000001)
        return dict(schema=1,status='pass',complete=True,method='all_static_placement_roundoff_envelopes',
                    reference_error_m=row['reference']['error_m'],source_error_m=row['source']['error_m'],
                    stretch_upper=row['stretch_upper'],placement_ids=[[0,0]],placements=[row],inputs_sha256=digest([row]))

    def certificate(self):
        exact=dict(status='pass',complete=True,method='identical_indexed_surface',maximum_observed_m=0.)
        return dict(status='pass',complete=True,method='placed_object_bidirectional_hausdorff',stretch_upper=1.000001,
                    reference_to_source=exact,source_to_reference=copy.deepcopy(exact),world_gpu_bounds=self.receipt())

    def test_complete_world_bounds_and_child_budget(self):
        value=self.certificate()
        self.assertEqual(certificate_problems(value,.01),[])
        child=dict(status='pass',complete=True,method='continuous_directed_hausdorff',maximum_observed_m=.0099,
                   certified_upper_m=.0099,cells=1)
        value['reference_to_source']=child
        self.assertTrue(certificate_problems(value,.01))

    def test_missing_bound_and_understated_stretch_fail(self):
        value=self.certificate();value.pop('world_gpu_bounds')
        self.assertTrue(certificate_problems(value,.01))
        value=self.certificate();value['stretch_upper']=1.
        self.assertTrue(certificate_problems(value,.01))

    def test_nonfinite_or_bool_and_underreported_maxima_fail(self):
        for key in ('reference_error_m','source_error_m','stretch_upper'):
            for value in (float('nan'),float('inf'),True):
                receipt=self.receipt();receipt[key]=value
                self.assertTrue(gpu.bounds_problems(receipt,.01))
        for key in ('reference_error_m','stretch_upper'):
            receipt=self.receipt();receipt[key]=1. if key=='stretch_upper' else 0.
            self.assertTrue(gpu.bounds_problems(receipt,.01))

    def test_placement_identity_must_be_nonempty_unique_and_typed(self):
        for ids in ([],[[0,0],[0,0]],[[True,0]],[[0,-1]],[[0,'0']]):
            receipt=self.receipt();receipt['placement_ids']=ids
            self.assertTrue(gpu.bounds_problems(receipt,.01))

    def test_rehashed_underreported_envelope_still_fails(self):
        receipt=self.receipt();receipt['placements'][0]['source']['error_m']=0.
        receipt['inputs_sha256']=digest(receipt['placements'])
        self.assertTrue(gpu.bounds_problems(receipt,.01))

    def test_overflowing_envelope_fails_closed(self):
        data=self.envelope();data.update(matrix_row_sum=2.**120,local_coordinate_extent_m=32.,error_m=2.**127)
        self.assertTrue(gpu.envelope_problems(data))

    def test_scene_bound_checks_every_candidate_and_sum(self):
        value=dict(schema=1,status='pass',complete=True,method='eligible_scene_candidate_envelopes',
                   reference_own=self.envelope(),source_own=self.envelope(),
                   reference_targets=[self.envelope()],source_targets=[self.envelope(),self.envelope()])
        value['source_targets'][1]['translation_extent_m']=100.
        value['source_targets'][1]['error_m']=math.nextafter(32*2.**-23*107.,math.inf)
        value['forward_error_m']=value['reference_own']['error_m']+value['source_targets'][1]['error_m']
        value['reverse_error_m']=value['source_own']['error_m']+value['reference_targets'][0]['error_m']
        self.assertEqual(gpu.scene_bounds_problems(value),[])
        value['forward_error_m']=2*value['reference_own']['error_m']
        self.assertTrue(gpu.scene_bounds_problems(value))


if __name__=='__main__':unittest.main()
