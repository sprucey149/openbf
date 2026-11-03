import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import render_skin_support as support


def body(path, positions, triangles, joint=1, foot_rotation=None, hand_rotation=None, mode='OPAQUE', scale=None, animation_rotation=None):
    owners = [joint] * len(positions) if type(joint) is int else joint
    streams = [(struct.pack('<' + 'f' * len(positions) * 3, *(x for p in positions for x in p)), 5126, 'VEC3', len(positions)),
               (bytes(value for owner in owners for value in (owner, 0, 0, 0)), 5121, 'VEC4', len(positions)),
               (struct.pack('<' + 'f' * len(positions) * 4, *([1., 0., 0., 0.] * len(positions))), 5126, 'VEC4', len(positions)),
               (struct.pack('<' + 'I' * len(triangles), *triangles), 5125, 'SCALAR', len(triangles))]
    if animation_rotation is not None:
        streams.extend([(struct.pack('<2f', 0., 1/30), 5126, 'SCALAR', 2),
                        (struct.pack('<8f', *(animation_rotation * 2)), 5126, 'VEC4', 2)])
    binary = b''
    views, accessors = [], []
    for payload, component, kind, count in streams:
        binary += b'\0' * (-len(binary) % 4)
        views.append(dict(buffer=0, byteOffset=len(binary), byteLength=len(payload)))
        binary += payload
        accessors.append(dict(bufferView=len(views) - 1, componentType=component, type=kind, count=count))
    nodes = [dict(name='DummyRoot', children=[1, 3]), dict(name='bone_l_foot'),
             dict(name='body', mesh=0, skin=0), dict(name='bone_r_hand')]
    if foot_rotation is not None:
        nodes[1]['rotation'] = foot_rotation
    if hand_rotation is not None:
        nodes[3]['rotation'] = hand_rotation
    if scale is not None:
        nodes[1]['scale'] = scale
    doc = dict(asset=dict(version='2.0'), buffers=[dict(byteLength=len(binary))], bufferViews=views,
               accessors=accessors, nodes=nodes, skins=[dict(joints=[0, 1, 3])],
               meshes=[dict(primitives=[dict(attributes=dict(POSITION=0, JOINTS_0=1, WEIGHTS_0=2), indices=3, material=0)])],
               materials=[dict(name='boot', alphaMode=mode)], scenes=[dict(nodes=[0, 2])], scene=0)
    if animation_rotation is not None:
        doc['animations'] = [dict(name='reload', samplers=[dict(input=4, output=5)],
                                  channels=[dict(sampler=0, target=dict(node=1, path='rotation'))])]
    encoded = json.dumps(doc).encode()
    encoded += b' ' * (-len(encoded) % 4)
    binary += b'\0' * (-len(binary) % 4)
    path.write_bytes(struct.pack('<4sII', b'glTF', 2, 28 + len(encoded) + len(binary))
                     + struct.pack('<I4s', len(encoded), b'JSON') + encoded
                     + struct.pack('<I4s', len(binary), b'BIN\0') + binary)
    for suffix in ('.sets.ron', '.anims.ron', '.motion.ron'):
        path.with_suffix(suffix).write_text('()')


class FootSupportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.a, self.b = self.root / 'reference.glb', self.root / 'source.glb'
        self.points = [(0., 0., 0.), (1., 0., 0.), (0., 0., 1.)]
        body(self.a, self.points, [0, 1, 2])

    def test_a_new_vertex_inside_the_triangle_does_not_change_support(self):
        body(self.b, self.points + [(0.3, 0.002, 0.3)], [0, 1, 3, 1, 2, 3, 2, 0, 3])
        result = support.support_pair(self.a, self.b)
        self.assertEqual(result['status'], 'pass')
        self.assertLess(result['support_upper_m'], 0.004001)
        self.assertGreaterEqual(result['support_upper_m'], 0.002)
        self.assertEqual(result['scope'], 'one model-state pair')
        self.assertFalse(result['runtime_complete'])

    def test_matching_rest_positions_do_not_exempt_wrong_joint_ownership(self):
        body(self.b, self.points, [0, 1, 2], joint=0)
        result = support.support_pair(self.a, self.b)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertIn('ownership', result['reason'])

    def test_a_different_foot_ancestor_rotation_invalidates_shared_pose(self):
        body(self.b, self.points, [0, 1, 2], foot_rotation=[0., 0., 1., 0.])
        result = support.support_pair(self.a, self.b)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertFalse(result['pose_inputs_identical'])

    def test_unrelated_hand_rotation_is_not_a_foot_dependency(self):
        body(self.b, self.points, [0, 1, 2], hand_rotation=[0., 0., 1., 0.])
        self.assertEqual(support.support_pair(self.a, self.b)['status'], 'pass')

    def test_no_scale_or_unproven_alpha_can_enter_a_rigid_support_proof(self):
        for args in (dict(scale=[1., 2., 1.]), dict(mode='BLEND')):
            body(self.b, self.points, [0, 1, 2], **args)
            self.assertEqual(support.support_pair(self.a, self.b)['status'], 'unmeasured')

    def test_unused_vertices_do_not_make_visible_feet_lower(self):
        body(self.b, self.points + [(0., -99., 0.)], [0, 1, 2])
        result = support.support_pair(self.a, self.b)
        self.assertEqual(result['status'], 'pass')
        self.assertLess(result['support_upper_m'], 1e-7)
        self.assertEqual(result['groups'][0]['source_vertices'], 3)

    def test_a_vertex_on_a_cross_joint_triangle_still_belongs_to_its_support_set(self):
        points = self.points + [(0.4, 0.1, 0.2), (0., 1., 0.), (0., 1., 1.)]
        for path in (self.a, self.b):
            body(path, points, [0, 1, 2, 3, 4, 5], joint=[1, 1, 1, 1, 0, 0])
        result = support.support_pair(self.a, self.b)
        self.assertEqual(result['status'], 'pass')
        self.assertLess(result['support_upper_m'], 1e-7)
        self.assertEqual(result['groups'][0]['source_vertices'], 4)

    def test_low_companion_must_actually_load_on_the_high_palette(self):
        body(self.b, self.points, [0, 1, 2])
        low_a = self.root / 'reference.low.glb'
        low_b = self.root / 'source.low.glb'
        body(low_a, self.points, [0, 1, 2])
        body(low_b, self.points, [0, 1, 2], foot_rotation=[0., 0., 1., 0.])
        result = support.support_pair(low_a, low_b)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertIn('LOW companion', result['reason'])

    def test_missing_controller_input_is_unmeasured(self):
        body(self.b, self.points, [0, 1, 2])
        self.b.with_suffix('.sets.ron').unlink()
        self.assertEqual(support.support_pair(self.a, self.b)['status'], 'unmeasured')

    def test_cross_joint_faces_need_a_correspondence_not_only_matching_support_vertices(self):
        points = self.points + [(0., 1., 0.), (0., 1., 1.), (1., 1., 0.)]
        body(self.a, points, [0, 1, 2, 0, 3, 4], joint=[1, 1, 1, 0, 0, 0])
        body(self.b, points, [0, 1, 2, 0, 3, 5], joint=[1, 1, 1, 0, 0, 0])
        result = support.support_pair(self.a, self.b)
        self.assertLess(result['vertex_support_upper_m'], 1e-7)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertIn('mixed-joint', result['reason'])

    def test_cross_joint_face_adds_the_other_joint_to_pose_dependencies(self):
        points = self.points + [(0., 1., 0.), (0., 1., 1.)]
        body(self.a, points, [0, 1, 2, 0, 3, 4], joint=[1, 1, 1, 2, 2])
        body(self.b, points, [0, 1, 2, 0, 3, 4], joint=[1, 1, 1, 2, 2], hand_rotation=[0., 0., 1., 0.])
        result = support.support_pair(self.a, self.b)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertFalse(result['pose_inputs_identical'])

    def test_visual_companion_pose_is_a_required_dependency(self):
        body(self.a, self.points, [0, 1, 2], animation_rotation=[0., 0., 0., 1.])
        body(self.b, self.points, [0, 1, 2], animation_rotation=[0., 0., 0., 1.])
        companion = self.root / 'source.render-clips.glb'
        body(companion, self.points, [0, 1, 2], animation_rotation=[0., 0., 1., 0.])
        self._meshless(companion)
        result = support.support_pair(self.a, self.b)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertFalse(result['pose_inputs_identical'])

    def test_visual_companion_can_restore_a_matching_local_pose(self):
        body(self.a, self.points, [0, 1, 2], animation_rotation=[0., 0., 0., 1.])
        body(self.b, self.points, [0, 1, 2], animation_rotation=[0., 0., 1., 0.])
        self.assertEqual(support.support_pair(self.a, self.b)['status'], 'unmeasured')
        companion = self.root / 'source.render-clips.glb'
        body(companion, self.points, [0, 1, 2], animation_rotation=[0., 0., 0., 1.])
        self._meshless(companion)
        self.assertEqual(support.support_pair(self.a, self.b)['status'], 'pass')

    def _meshless(self, path):
        raw = path.read_bytes()
        length = struct.unpack_from('<I', raw, 12)[0]
        doc = json.loads(raw[20:20+length])
        del doc['meshes']
        del doc['materials']
        del doc['nodes'][2]['mesh']
        del doc['nodes'][2]['skin']
        encoded = json.dumps(doc).encode()
        encoded += b' ' * (-len(encoded) % 4)
        binary = raw[20+length:]
        path.write_bytes(struct.pack('<4sII', b'glTF', 2, 20+len(encoded)+len(binary))
                         + struct.pack('<I4s', len(encoded), b'JSON') + encoded + binary)


if __name__ == '__main__':
    unittest.main()
