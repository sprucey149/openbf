"""Hostile rotations and complete analytic-domain error bounds."""
import math
from pathlib import Path
import random
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import render_query_frame as q
from render_contact_proxy import analytic_proxy

class QueryFrameTests(unittest.TestCase):
    def test_actual_query_library_versions_and_normalization_chain_are_pinned(self):
        import tomllib
        root=Path(__file__).resolve().parents[2]
        lock=tomllib.loads((root/'engine/Cargo.lock').read_text())
        versions={p['name']:p['version'] for p in lock['package'] if p['name'] in ('glam','nalgebra')}
        self.assertEqual(versions,{'glam':'0.30.10','nalgebra':'0.33.3'})
        text=(root/'engine/crates/core/src/collision/shape.rs').read_text()
        body=text.split('pub(super) fn isometry_of(',1)[1].split('/// Axis-aligned box',1)[0]
        self.assertIn('b.x_axis.normalize_or(Vec3::X)',body)
        self.assertIn('b.y_axis.normalize_or(Vec3::Y)',body)
        self.assertIn('b.z_axis.normalize_or(Vec3::Z)',body)
        self.assertIn('Quat::from_mat3',body)
        self.assertIn('.normalize();',body)
        self.assertIn('UnitQuaternion::from_quaternion',body)

    def test_interval_arithmetic_covers_cancellation_and_every_sum_order(self):
        values=[q.iv(1e10),q.iv(1),q.iv(-1e10),q.iv(2)]
        interval=q.sum_all(values)
        self.assertLessEqual(interval[0],0.)
        self.assertGreaterEqual(interval[1],3.)
    def test_crossing_zero_normalization_squares_outward_and_keeps_zero_lower(self):
        interval=q.square((-1e-4,2e-4))
        self.assertEqual(interval[0],0.)
        self.assertGreaterEqual(interval[1],(2e-4)**2)
        normalized=q.normalize([(-1e-4,2e-4),q.iv(1.),q.iv(0.)])
        self.assertLess(normalized[0][0],0.)
        self.assertGreater(normalized[0][1],0.)
        for x in (-1e-4,0.,2e-4):
            expected=x/math.sqrt(1+x*x)
            self.assertLessEqual(normalized[0][0],expected)
            self.assertGreaterEqual(normalized[0][1],expected)

    def test_unsupported_ill_conditioned_query_frames_fail_closed(self):
        with self.assertRaisesRegex(ValueError,'ill-conditioned'):
            q.query_frame({'rotation':[[1e-5,0,0],[0,1,0],[0,0,1]],'translation':[0,0,0]})

    def test_identity_and_rotated_frame_bounds_are_small_but_outward(self):
        for angle in (0.,math.pi/4,math.pi/2,math.pi):
            c,s=math.cos(angle),math.sin(angle)
            frame={'rotation':[[c,0,-s],[0,1,0],[s,0,c]],'translation':[1,2,3]}
            result=q.query_frame(frame)
            self.assertGreater(result['rotation_error_per_unit'],0.)
            self.assertLess(result['rotation_error_per_unit'],.0002)
    def test_reflection_is_not_mislabelled_as_a_rigid_query_frame(self):
        r=q.query_frame({'rotation':[[1,0,0],[0,1,0],[0,0,-1]],'translation':[0,0,0]})
        self.assertGreater(r['rotation_error_per_unit'],1.)
    def test_cylinder_proxy_has_complete_caps_and_proved_radial_error(self):
        triangles,error=analytic_proxy('cylinder',(2.,6.,0.),.001)
        self.assertLessEqual(error,.00101)
        self.assertTrue(any(all(p[1]==3 for p in tri) for tri in triangles))
        self.assertTrue(any(all(p[1]==-3 for p in tri) for tri in triangles))
        self.assertEqual(len(triangles)%4,0)
        for tri in triangles:
            for p in tri:
                self.assertLessEqual(math.hypot(p[0],p[2]),2.+1e-12)
    def test_sphere_proxy_is_closed_and_covers_every_radial_direction(self):
        from collections import Counter
        triangles,error=analytic_proxy('sphere',(1.,0.,0.),.004)
        edges=Counter(tuple(sorted((tri[i],tri[(i+1)%3]))) for tri in triangles for i in range(3))
        self.assertTrue(all(count==2 for count in edges.values()))
        self.assertLessEqual(error,.004)
        self.assertTrue(all(abs(math.sqrt(sum(v*v for v in p))-1)<1e-12 for tri in triangles for p in tri))
