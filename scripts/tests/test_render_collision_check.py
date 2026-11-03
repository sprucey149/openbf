"""Discriminating surface/placement checks; no game content required."""
import json
import copy
import contextlib
import io
import math
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
import sys
sys.path.insert(0, str(SCRIPTS))
import render_collision_check as check
from render_class_inputs import ClassInputs, fnv


def glb(path, vertices, indices, alpha_mode=None):
    packed = b''.join(struct.pack('<3f', *v) for v in vertices)
    packed += b''.join(struct.pack('<I', i) for i in indices)
    doc = dict(asset={'version': '2.0'}, buffers=[{'byteLength': len(packed)}],
               bufferViews=[{'buffer': 0, 'byteLength': len(vertices) * 12},
                            {'buffer': 0, 'byteOffset': len(vertices) * 12, 'byteLength': len(indices) * 4}],
               accessors=[{'bufferView': 0, 'componentType': 5126, 'type': 'VEC3', 'count': len(vertices)},
                          {'bufferView': 1, 'componentType': 5125, 'type': 'SCALAR', 'count': len(indices)}],
               meshes=[{'primitives': [{'attributes': {'POSITION': 0}, 'indices': 1}]}])
    if alpha_mode is not None:
        doc['materials'] = [dict(name='sheet', alphaMode=alpha_mode)]
        doc['meshes'][0]['primitives'][0]['material'] = 0
    if alpha_mode is None:
        doc['materials'] = [dict(name='fixture_gray_lookup', alphaMode='OPAQUE')]
        doc['meshes'][0]['primitives'][0]['material'] = 0
    header = json.dumps(doc).encode()
    header += b' ' * (-len(header) % 4)
    packed += b'\0' * (-len(packed) % 4)
    data = struct.pack('<4sII', b'glTF', 2, 12 + 8 + len(header) + 8 + len(packed))
    data += struct.pack('<I4s', len(header), b'JSON') + header + struct.pack('<I4s', len(packed), b'BIN\0') + packed
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def edit_glb_document(path, edit):
    raw = path.read_bytes()
    size = struct.unpack_from('<I', raw, 12)[0]
    doc = json.loads(raw[20:20 + size])
    edit(doc)
    header = json.dumps(doc).encode()
    header += b' ' * (-len(header) % 4)
    tail = raw[20 + size:]
    path.write_bytes(struct.pack('<4sII', b'glTF', 2, 20 + len(header) + len(tail))
                     + struct.pack('<I4s', len(header), b'JSON') + header + tail)


class SurfaceTests(unittest.TestCase):
    def test_default_gate_requires_the_whole_base_game_catalog(self):
        self.assertEqual(len(set(check.REQUIRED_MAPS)), 40)
        self.assertIn('bf2_spa1', check.REQUIRED_MAPS)
        self.assertIn('bf1_bes2', check.REQUIRED_MAPS)

    def test_face_interior_is_not_vertex_distance(self):
        s = check.Surface([(0, 0, 0), (10, 0, 0), (0, 0, 10)], [0, 1, 2])
        self.assertAlmostEqual(s.distance((2, 3, 2)), 3)
        self.assertAlmostEqual(s.distance((2, 0, 2)), 0)

    def test_nonfinite_geometry_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'nonfinite'):
            check.Surface([(0, 0, float('nan'))], [0, 0, 0])

    def test_degenerate_triangle_and_edge(self):
        tri = [(0, 0, 0), (1, 0, 0), (2, 0, 0)]
        self.assertAlmostEqual(check.triangle_distance2((1, 3, 0), tri), 9)
        self.assertAlmostEqual(check.triangle_distance2((3, 0, 0), tri), 1)

    def test_hole_with_identical_bounds_is_measured(self):
        # A panel has a centre triangle removed; an AABB-only test cannot see it.
        v = [(0, 0, 0), (2, 0, 0), (2, 0, 2), (0, 0, 2), (1, 0, 1)]
        full = check.Surface(v, [0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4])
        gap = check.Surface(v, [0, 1, 4, 2, 3, 4, 3, 0, 4])
        self.assertEqual(full.bounds, gap.bounds)
        p = (1.8, 0, 1)
        self.assertEqual(full.distance(p), 0)
        self.assertGreater(gap.distance(p), .5)

    def test_bvh_matches_brute_force(self):
        v = [(i % 7, (i * 3) % 5, i // 7) for i in range(120)]
        ix = list(range(120))
        s = check.Surface(v, ix)
        for p in ((.3, .7, .5), (9, 4, 9), (-1, 5, 0)):
            expected = min(check.triangle_distance2(p, tri) for tri in s.triangles) ** .5
            self.assertAlmostEqual(s.distance(p), expected)

    def test_primitive_surface_distances_use_full_cylinder_height(self):
        prim = dict(kind='cylinder', dims=[2, 6, 0], rotation=[[1, 0, 0], [0, 1, 0], [0, 0, 1]], translation=[1, 2, 3])
        self.assertEqual(check.primitive_distance((1, 5, 3), prim), 0)
        self.assertEqual(check.primitive_distance((1, 2, 3), prim), 2)
        self.assertAlmostEqual(check.primitive_distance((4, 6, 3), prim), 2 ** .5)

    def test_only_runtime_used_primitive_dimensions_are_validated(self):
        transform = dict(rotation=[[1, 0, 0], [0, 1, 0], [0, 0, 1]], translation=[0, 0, 0])
        for kind, dims in (('sphere', [2, -9, -8]), ('cylinder', [2, 6, -8])):
            self.assertTrue(check.primitive_samples(dict(kind=kind, dims=dims, **transform)))
        with self.assertRaisesRegex(ValueError, 'invalid collision primitive dimensions'):
            check.primitive_samples(dict(kind='box', dims=[-2, 6, 8], **transform))

    def test_nonrigid_transform_is_unmeasured_instead_of_wrong_metres(self):
        with self.assertRaisesRegex(ValueError, 'nonrigid'):
            check.require_rigid(dict(rotation=[[2, 0, 0], [0, 1, 0], [0, 0, 1]], translation=[0, 0, 0]))

    def test_named_lookup_matches_runtime_first_definition_wins(self):
        entries = check.merge_named([dict(name='Geo', value=1), dict(name='GEO', value=2)],
                                    [dict(name='geo', value=3)], 'name')
        self.assertEqual(entries['geo']['value'], 1)

    def test_transform_column_convention(self):
        inst = dict(rotation=[[0, 0, -1], [0, 1, 0], [1, 0, 0]], translation=[10, 20, 30])
        self.assertEqual(check.placed((1, 2, 3), inst), (13, 22, 29))


class SceneTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.reference, self.other = self.root / 'ref', self.root / 'src'
        self.points = [(0, 0, 0), (2, 0, 0), (0, 0, 2)]
        self.inst = dict(name='wall', **{'class': 'class'}, rotation=[[1, 0, 0], [0, 1, 0], [0, 0, 1]], translation=[0, 0, 0])
        self.doc = dict(source_game='bf1', classes=[dict(name='CLASS', geometry='Geo')],
                        geometries=[dict(name='geo', model='MODEL')], instances=[self.inst], models=['model'], textures=[],
                        collision=[dict(geometry='Geo', glb='collision/geo.glb', prims=[])], paths=[], layers=[])
        for root in (self.reference, self.other):
            glb(root / 'map/models/model.glb', self.points, [0, 1, 2])
            glb(root / 'map/collision/geo.glb', self.points, [0, 1, 2])
            (root / 'map/classes.ron').write_text('[]')
        self.raw_classes = [dict(name='CLASS', base='prop', props=[])]
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: self.doc if path.name == 'manifest.ron'
                                   else self.raw_classes if path.name == 'classes.ron' else {})
        self.reader.start()
        self.addCleanup(lambda: self.reader.stop())
        check.surface.cache_clear()

    def run_check(self):
        return check.compare_map(self.reference, self.other, 'map', 'source', .1, 32)

    def test_unchanged_art_passes_even_when_collision_is_simplified(self):
        for root in (self.reference, self.other):
            glb(root / 'map/models/model.glb', [(x, y + 1, z) for x, y, z in self.points], [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'pass')
        self.assertEqual(row['collision_to_reference_max_m'], 1)
        self.assertEqual(row['surface_residual_increase_max_m'], 0)

    def test_shifted_art_fails_with_same_collision(self):
        collision_before = (self.other / 'map/collision/geo.glb').read_bytes()
        glb(self.other / 'map/models/model.glb', [(x, y + 2, z) for x, y, z in self.points], [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['local_object_status'], 'offender')
        self.assertEqual(row['surface_residual_increase_max_m'], 2)
        self.assertEqual((self.other / 'map/collision/geo.glb').read_bytes(), collision_before)

    def test_different_collision_cannot_pass_as_art_alignment(self):
        glb(self.other / 'map/collision/geo.glb', [(x, y + 1, z) for x, y, z in self.points], [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertFalse(row['collision_identical'])
        self.assertIn('collision differs', row['error'])

    def test_same_mesh_cannot_hide_a_different_collision_layer_policy(self):
        changed = copy.deepcopy(self.doc)
        changed['classes'][0]['collision'] = [['soldier', 'none']]
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron' else {})
        self.reader.start()
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertFalse(row['class_collision_identical'])
        self.assertIn('collision-layer', row['error'])

    def test_collision_layer_keys_remain_case_sensitive_like_runtime(self):
        self.doc['classes'][0]['collision'] = [['soldier', 'none']]
        changed = copy.deepcopy(self.doc)
        changed['classes'][0]['collision'] = [['Soldier', 'none']]
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron' else {})
        self.reader.start()
        row = self.run_check()['objects'][0]
        self.assertFalse(row['class_collision_identical'])
        self.assertEqual(row['status'], 'unmeasured')

    def test_render_only_door_parts_are_not_a_physics_policy_mismatch(self):
        self.doc['classes'][0]['door'] = dict(frames=2, triggers=[], prims=[], parts=[])
        changed = copy.deepcopy(self.doc)
        changed['classes'][0]['door']['parts'] = [dict(submesh=4, bone='donor_piece', motion=[])]
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron' else {})
        self.reader.start()
        self.assertTrue(self.run_check()['objects'][0]['class_collision_identical'])

    def test_door_primitive_tracks_remain_physical_policy(self):
        self.doc['classes'][0]['door'] = dict(frames=2, triggers=[], prims=[dict(name='body', motion=[])], parts=[])
        changed = copy.deepcopy(self.doc)
        changed['classes'][0]['door']['prims'][0]['motion'] = [[[0, 0, 0, 1], [0, 1, 0]]]
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron' else {})
        self.reader.start()
        row = self.run_check()['objects'][0]
        self.assertFalse(row['class_collision_identical'])
        self.assertEqual(row['status'], 'unmeasured')

    def test_unused_collision_record_labels_do_not_change_the_surface(self):
        changed = copy.deepcopy(self.doc)
        changed['collision'][0]['node'] = 'source_scene_label'
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron' else {})
        self.reader.start()
        self.assertEqual(self.run_check()['objects'][0]['status'], 'pass')

    def test_dynamic_armed_building_keeps_its_intact_geometry(self):
        self.doc['source_game'] = 'bf2'
        self.doc['classes'][0].update(base='armedbuildingdynamic', build_zone='pad')
        self.doc['classes'].append(dict(name='pad', geometry=None))
        self.assertEqual(self.run_check()['objects'][0]['status'], 'pass')

    def test_added_inner_wall_is_seen_when_placed_bounds_are_unchanged(self):
        reference = self.points + [(0, 0, 0), (0, 2, 0), (0, 0, 0)]
        indices = [0, 1, 2, 3, 4, 5]
        glb(self.reference / 'map/models/model.glb', reference, indices)
        glb(self.other / 'map/models/model.glb', reference + [(.5, .3, .5), (1.3, 1.7, .5), (.5, 1.7, 1.3)],
            indices + [6, 7, 8])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['render_bounds_max_delta_m'], 0)
        self.assertEqual(row['surface_residual_increase_max_m'], 0)
        self.assertGreater(row['render_to_collision_residual_increase_max_m'], .1)
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['local_object_status'], 'offender')

    def test_missing_render_is_unmeasured(self):
        (self.other / 'map/models/model.glb').unlink()
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertIn('missing render model', row['error'])

    def cutout(self, root):
        glb(root / 'map/models/model.glb', self.points, [0, 1, 2], alpha_mode='MASK')
        (root / 'map/textures').mkdir(exist_ok=True)
        from test_render_terrain_dependencies import ktx
        self.doc['textures'] = ['sheet']
        (root / 'map/textures/sheet.ktx2').write_bytes(ktx(alpha=(0,255,0,255)))

    def test_unchanged_cutout_base_control_passes(self):
        for root in (self.reference, self.other):
            self.cutout(root)
        self.assertEqual(self.run_check()['objects'][0]['status'], 'pass')

    def test_reference_cutout_cannot_adopt_its_own_enhanced_texture(self):
        self.cutout(self.reference)
        from test_render_terrain_dependencies import ktx
        (self.reference / 'map/textures/sheet.tier2.ktx2').write_bytes(ktx(alpha=(255,0,255,0)))
        row = check.compare_map(self.reference, self.reference, 'map', 'mixed', .1, 32)['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['opacity_certificates']['model']['status'], 'unmeasured')

    def test_cutout_face_cannot_certify_a_removed_opaque_duplicate(self):
        for root in (self.reference, self.other):
            self.cutout(root)
        def add_opaque(doc):
            doc['materials'].append(dict(name='opaque', alphaMode='OPAQUE'))
            prim = copy.deepcopy(doc['meshes'][0]['primitives'][0])
            prim['material'] = 1
            doc['meshes'][0]['primitives'].append(prim)
        edit_glb_document(self.reference / 'map/models/model.glb', add_opaque)
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['opacity_certificates']['model']['status'], 'unmeasured')

    def test_extra_outside_art_cannot_be_excluded_as_invisible(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([-1, 0, -1], []), ([3, 0, -1], []),
                                                            ([3, 0, 3], []), ([-1, 0, 3], [])])]
        glb(self.other / 'map/models/model.glb', self.points + [(100, 0, 100), (102, 0, 100), (100, 0, 102)],
            [0, 1, 2, 3, 4, 5])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['local_object_status'], 'offender')
        self.assertGreater(row['render_bounds_max_delta_m'], 90)
        self.assertEqual(row['playable_render_bounds_max_delta_m'], 0)

    def test_repeated_layer_names_keep_every_placement(self):
        self.doc['layers'] = [dict(name='common', instances=[self.inst]), dict(name='common', instances=[self.inst])]
        report = self.run_check()
        self.assertEqual(report['placements'], 3)
        self.assertEqual([row['layer_ordinal'] for row in report['objects']], [0, 1, 2])
        self.assertEqual(report['counts'], {'pass': 3})

    def test_submillimetre_boundary_vertex_does_not_create_a_false_tall_envelope(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([0, 0, -1], []), ([2, 0, -1], []),
                                                            ([2, 0, 2], []), ([0, 0, 2], [])])]
        # Keep a far triangle so this is a nonfading building; this test
        # isolates boundary-subset discontinuity rather than camera fading.
        reference = [(0, 5, 0), (1, 0, 0), (1, 0, 1), (6, 0, 0)]
        indices = [0, 1, 2, 1, 3, 2]
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', reference, indices)
        glb(self.reference / 'map/models/model.glb', reference, indices)
        glb(self.other / 'map/models/model.glb', [(-.0001, 5, 0), *reference[1:]], indices)
        row = self.run_check()['objects'][0]
        self.assertGreater(row['playable_render_bounds_max_delta_m'], 4)
        self.assertLess(row['render_bounds_max_delta_m'], .001)
        self.assertFalse(row['render_bounds_regressed'])
        self.assertEqual(row['status'], 'pass')
        self.assertTrue(row['surface_certificate']['complete'])

    def test_shifted_coplanar_detail_can_pass_with_continuous_contact_proof(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([0, 0, 0], []), ([3, 0, 0], []),
                                                            ([3, 0, 3], []), ([0, 0, 3], [])])]
        fixed = [(-10, 0, -10), (10, 0, -10), (0, 0, 10)]
        inside = [(.2, 0, .2), (1, 0, .2), (.2, 0, 1)]
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', fixed, [0, 1, 2])
        glb(self.reference / 'map/models/model.glb', fixed + inside, [0, 1, 2, 3, 4, 5])
        glb(self.other / 'map/models/model.glb', fixed + [(x + 1, y, z) for x, y, z in inside], [0, 1, 2, 3, 4, 5])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['render_bounds_max_delta_m'], 0)
        self.assertGreater(row['playable_render_bounds_max_delta_m'], .9)
        self.assertEqual(row['surface_samples_regressed'], 0)
        self.assertEqual(row['render_to_collision_samples_regressed'], 0)
        self.assertEqual(row['status'], 'pass')
        self.assertTrue(row['surface_certificate']['complete'])

    def test_art_entering_playable_bounds_cannot_hide_behind_outside_collision(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([-1, 0, -1], []), ([3, 0, -1], []),
                                                            ([3, 0, 3], []), ([-1, 0, 3], [])])]
        outside = [(x + 50, y, z) for x, y, z in self.points]
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', outside, [0, 1, 2])
        glb(self.reference / 'map/models/model.glb', outside, [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['playable_samples'], 0)
        self.assertGreater(row['render_to_collision_residual_increase_max_m'], 40)
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['local_object_status'], 'offender')

    def test_improved_contact_without_art_correspondence_remains_unmeasured(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([-1, 0, -1], []), ([3, 0, -1], []),
                                                            ([3, 0, 3], []), ([-1, 0, 3], [])])]
        outside = [(x + 50, y, z) for x, y, z in self.points]
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', outside, [0, 1, 2])
        glb(self.other / 'map/models/model.glb', outside, [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertTrue(row['reference_art_in_play'])
        self.assertFalse(row['source_art_in_play'])
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['local_object_status'], 'unmeasured')

    def test_outside_boundary_is_not_a_visibility_exemption(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([50, 0, 50], []), ([60, 0, 50], []), ([50, 0, 60], [])])]
        glb(self.other / 'map/models/model.glb', [(x, y + 2, z) for x, y, z in self.points], [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['local_object_status'], 'offender')
        self.assertEqual(row['playable_samples'], 0)

    def test_tiny_face_omitted_by_diagnostics_still_fails_continuous_gate(self):
        # More triangles than the diagnostic quota. Delete an interior face while
        # retaining its unused accessor vertices, a former sampled-pass escape.
        points = [(x * 4 + dx, 0, dz) for x in range(400) for dx, dz in ((0, 0), (1, 0), (0, 1))]
        indices = list(range(len(points)))
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', points, indices)
        glb(self.reference / 'map/models/model.glb', points, indices)
        glb(self.other / 'map/models/model.glb', points, indices[:603] + indices[606:])
        row = check.compare_map(self.reference, self.other, 'map', 'source', .1, 4)['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['local_object_status'], 'unmeasured')
        self.assertGreater(row['surface_certificate']['art_forward']['maximum_observed_m'], 1)
        self.assertGreater(row['scene_surface_certificates']['model']['reference_to_source']['maximum_observed_m'], 1)

    def test_no_collision_is_explicit_ps2_baseline_not_an_unchecked_pass(self):
        self.doc['collision'] = []
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'baseline_visual_only')
        self.assertTrue(row['surface_certificate']['complete'])

    def test_no_geometry_is_explicit_ps2_baseline(self):
        self.doc['classes'][0]['geometry'] = None
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'baseline_no_render_geometry')
        self.assertIn('PS2', row['baseline'])

    def test_absent_class_in_both_builds_requires_identical_runtime_inputs(self):
        self.doc['classes'] = []
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'baseline_runtime_skip')
        self.assertTrue(row['resolved_class_absent_both'])
        self.assertTrue(row['raw_class_inputs_identical'])

    def test_unresolved_raw_turret_still_needs_its_dynamic_consumer_proof(self):
        self.doc['classes'] = []
        self.raw_classes[0]['base'] = 'armedbuilding'
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertIn('raw dynamic consumer', row['baseline_error'])

    def test_unresolved_nonpost_does_not_consume_unrelated_regions(self):
        self.doc['classes'] = []
        changed = copy.deepcopy(self.doc)
        changed['regions'] = [dict(name='unrelated_audio_region')]
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron'
            else self.raw_classes if path.name == 'classes.ron' else {})
        self.reader.start()
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'baseline_runtime_skip')
        self.assertTrue(row['layout_inputs_identical'])

    def test_unresolved_post_still_requires_its_referenced_region(self):
        self.doc['classes'] = []
        self.inst['props'] = [[fnv('CaptureRegion'), 'capture']]
        self.doc['regions'] = [dict(name='capture', translation=[0, 0, 0])]
        changed = copy.deepcopy(self.doc)
        changed['regions'][0]['translation'] = [10, 0, 0]
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron'
            else self.raw_classes if path.name == 'classes.ron' else {})
        self.reader.start()
        self.assertEqual(self.run_check()['objects'][0]['status'], 'unmeasured')

    def test_negative_box_is_retained_with_its_world_location(self):
        self.doc['collision'][0]['prims'] = [dict(name='bad_box', kind='box', dims=[-.1, 1, 1], mask=17,
            rotation=[[1, 0, 0], [0, 1, 0], [0, 0, 1]], translation=[10, 2, 3])]
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'baseline_input_identity')
        self.assertFalse(row['absolute_visible_residual_measured'])
        self.assertEqual(row['invalid_collision_primitives'][0]['name'], 'bad_box')
        self.assertEqual(row['invalid_collision_primitives'][0]['world_location'], [10, 2, 3])

    def test_unresolved_foliage_class_is_not_proved_absent(self):
        self.doc['classes'] = []
        self.doc['leaf'] = [dict(name='CLASS')]
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')

    def test_missing_art_in_both_builds_can_be_an_invisible_ps2_body(self):
        for root in (self.reference, self.other):
            (root / 'map/models/model.glb').unlink()
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'baseline_collision_only')
        self.assertTrue(row['collision_identical'])
        self.assertTrue(row['render_absent_both'])

    def partitioned_scene(self):
        # Both objects are large enough never to be hidden by camera fading.
        points = [(0, 0, 0), (6, 0, 0), (6, 0, 6), (0, 0, 6)]
        self.doc['classes'][0]['base'] = 'prop'
        self.doc['classes'].append(dict(name='other_class', base='prop', geometry='other_geo'))
        self.raw_classes.append(dict(name='other_class', base='prop', props=[]))
        self.doc['geometries'].append(dict(name='other_geo', model='other_model'))
        self.doc['models'].append('other_model')
        self.doc['instances'].append(dict(self.inst, name='other', **{'class': 'other_class'}))
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', points, [0, 1, 2, 0, 2, 3])
        glb(self.reference / 'map/models/model.glb', points, [0, 1, 2, 0, 2, 3])
        glb(self.other / 'map/models/model.glb', points, [0, 1, 2])
        glb(self.reference / 'map/models/other_model.glb', points, [0, 1, 2])
        glb(self.other / 'map/models/other_model.glb', points, [0, 1, 2, 0, 2, 3])
        return self.run_check()['objects']

    def test_scene_union_accepts_differently_partitioned_static_art(self):
        rows = self.partitioned_scene()
        self.assertEqual(rows[0]['status'], 'pass')
        self.assertEqual(rows[0]['local_object_status'], 'offender')
        self.assertEqual(rows[0]['scene_surface_certificates']['model']['method'], 'whole_scene_bidirectional_hausdorff')

    def test_authored_neighbor_lifetime_cannot_supply_missing_solid_art(self):
        from render_class_inputs import fnv
        self.assertEqual(self.partitioned_scene()[0]['status'], 'pass')
        self.raw_classes[1]['props'] = [[fnv('MaxHealth'), '100']]
        self.assertEqual(self.run_check()['objects'][0]['status'], 'unmeasured')

    def test_raw_turret_base_cannot_masquerade_as_manifest_static_prop(self):
        self.assertEqual(self.partitioned_scene()[0]['status'], 'pass')
        self.raw_classes[1]['base'] = 'armedbuildingdynamic'
        self.assertEqual(self.run_check()['objects'][0]['status'], 'unmeasured')

    def test_a_camera_faded_neighbor_cannot_supply_missing_solid_art(self):
        # The 6m panel never fades. The 2m prop can be hidden while the camera
        # is inside its grown box. Borrowing its drawing leaves a 2m hole in
        # the source panel in that state, while the reference panel stays solid.
        self.doc['classes'][0]['base'] = 'prop'
        self.doc['classes'].append(dict(name='small_class', base='prop', geometry='small_geo'))
        self.doc['geometries'].append(dict(name='small_geo', model='small_model'))
        self.doc['models'].append('small_model')
        self.doc['instances'].append(dict(self.inst, name='small', **{'class': 'small_class'},
                                          translation=[2, 0, 2]))
        full = [(0, 0, 0), (6, 0, 0), (6, 0, 6), (0, 0, 6)]
        small = [(0, 0, 0), (2, 0, 0), (2, 0, 2), (0, 0, 2)]
        vertices, indices = [], []
        for x0, z0, x1, z1 in ((0, 0, 2, 6), (4, 0, 6, 6),
                                (2, 0, 4, 2), (2, 4, 4, 6)):
            base = len(vertices)
            vertices.extend([(x0, 0, z0), (x1, 0, z0), (x1, 0, z1), (x0, 0, z1)])
            indices.extend(base + i for i in (0, 1, 2, 0, 2, 3))
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', full, [0, 1, 2, 0, 2, 3])
        glb(self.reference / 'map/models/model.glb', full, [0, 1, 2, 0, 2, 3])
        glb(self.other / 'map/models/model.glb', vertices, indices)
        glb(self.reference / 'map/models/small_model.glb', small, [0, 1, 2])
        glb(self.other / 'map/models/small_model.glb', small, [0, 1, 2, 0, 2, 3])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')

    def test_small_shape_motion_cannot_claim_identical_camera_visibility(self):
        glb(self.other / 'map/models/model.glb', [(x, y + .005, z) for x, y, z in self.points], [0, 1, 2])
        row = self.run_check()['objects'][0]
        # A 5mm move can put the chase eye inside only the source's grown
        # blocker box, hiding its whole model despite a tiny local distance.
        self.assertEqual(row['status'], 'unmeasured')
        self.assertFalse(row['camera_visibility_identical'])

    def test_a_far_cutout_neighbor_cannot_cover_a_missing_opaque_panel(self):
        self.test_scene_union_accepts_differently_partitioned_static_art()
        from test_render_terrain_dependencies import ktx
        self.doc['textures']=['sheet']
        self.doc['models'].append('other_lowd')
        self.doc['geometries'][-1]['lods']=[['lowd','other_lowd']]
        points=[(0,0,0),(6,0,0),(6,0,6),(0,0,6)]
        for root in (self.reference,self.other):
            glb(root/'map/models/other_lowd.glb',points,[0,1,2,0,2,3],alpha_mode='MASK')
            (root/'map/textures').mkdir()
            (root/'map/textures/sheet.ktx2').write_bytes(ktx(alpha=(0,255,0,255)))
        rows=self.run_check()['objects']
        self.assertEqual(rows[0]['status'],'unmeasured')

    def test_exact_shared_draws_cancel_even_at_large_world_coordinates(self):
        self.inst['translation']=[262144.,0.,0.]
        row=check.compare_map(self.reference,self.other,'map','source',.01,32)['objects'][0]
        self.assertEqual(row['status'],'pass')
        gpu=row['scene_surface_certificates']['model']['world_gpu_bounds']
        self.assertEqual(gpu['method'],'identical_position_and_pose_inputs')

    def test_nonidentity_world_gpu_rounding_cannot_hide_a_three_centimetre_gap(self):
        def f32(value):return struct.unpack('<f',struct.pack('<f',value))[0]
        translation=f32(262144.)
        reference_x,source_x=f32(.015624),f32(.015626)
        self.assertLess(source_x-reference_x,.00001)
        self.assertEqual(f32(translation+source_x)-f32(translation+reference_x),.03125)
        self.inst['translation']=[translation,0.,0.]
        reference=[(reference_x,0.,0.),(reference_x,6.,0.),(reference_x,0.,6.)]
        source=[(source_x,y,z) for _x,y,z in reference]
        for root in (self.reference,self.other):
            glb(root/'map/collision/geo.glb',reference,[0,1,2])
        glb(self.reference/'map/models/model.glb',reference,[0,1,2])
        glb(self.other/'map/models/model.glb',source,[0,1,2])
        row=check.compare_map(self.reference,self.other,'map','source',.01,32)['objects'][0]
        self.assertEqual(row['status'],'unmeasured')

    def test_world_gpu_budget_includes_the_worst_matching_placement(self):
        points=[(0.,0.,0.),(6.,0.,0.),(0.,0.,6.)]
        glb(self.reference/'map/models/model.glb',points,[0,1,2])
        glb(self.other/'map/models/model.glb',[(x,y+.0001,z) for x,y,z in points],[0,1,2])
        far=copy.deepcopy(self.inst);far.update(name='far wall',translation=[262144.,0.,0.])
        self.doc['layers']=[dict(name='mode',instances=[far])]
        rows=check.compare_map(self.reference,self.other,'map','source',.01,32)['objects']
        self.assertEqual([row['status'] for row in rows],['pass','unmeasured'])
        gpu=rows[0]['scene_surface_certificates']['model']['world_gpu_bounds']
        self.assertEqual(gpu['method'],'eligible_scene_candidate_envelopes')
        # Local proof caches use the whole class/model placement group. Directly
        # inspect it too: scene fallback may use the smaller local candidate set.
        from render_gpu_bounds import group_bounds
        reference,source=check.Scene(self.reference,'map'),check.Scene(self.other,'map')
        a=reference.render_states('geo')['model'];b=source.render_states('geo')['model']
        bound=group_bounds(reference,source,(0,0),a,b)
        self.assertEqual(bound['placement_ids'],[[0,0],[1,0]])
        self.assertGreater(bound['reference_error_m'],.9)

    def test_nonidentity_moving_model_has_no_sampled_world_draw_pass(self):
        points=[(0.,0.,0.),(6.,0.,0.),(0.,0.,6.)]
        for root in (self.reference,self.other):glb(root/'map/models/model.glb',points,[0,1,2])
        glb(self.other/'map/models/model.glb',[(x,y+.0001,z) for x,y,z in points],[0,1,2])
        self.doc['classes'][0]['door']=dict(frames=2,parts=[],prims=[],triggers=[])
        row=check.compare_map(self.reference,self.other,'map','source',.01,32)['objects'][0]
        self.assertEqual(row['status'],'unmeasured')

    def test_far_draw_uses_exact_registered_name_without_case_fallback(self):
        self.doc['models'].append('far')
        self.doc['geometries'][0]['far']='FAR'
        self.doc['lighting']=dict(near_scene_range=[100.,200.,300.])
        points=[(0.,10.,0.),(6.,10.,0.),(0.,10.,6.)]
        glb(self.reference/'map/models/far.glb',points,[0,1,2])
        glb(self.other/'map/models/FAR.glb',points,[0,1,2])
        source_doc=copy.deepcopy(self.doc);source_doc['models'][-1]='FAR'
        self.reader.stop()
        self.reader=patch.object(check,'read_ron',side_effect=lambda path:(source_doc if self.other in path.parents else self.doc)
                                 if path.name=='manifest.ron' else self.raw_classes if path.name=='classes.ron' else {})
        self.reader.start()
        row=check.compare_map(self.reference,self.other,'map','source',.01,32)['objects'][0]
        self.assertEqual(row['status'],'unmeasured')

    def test_disabled_far_geometry_does_not_supply_or_add_drawn_faces(self):
        self.doc['models'].append('far');self.doc['geometries'][0]['far']='far'
        for root,offset in ((self.reference,10.),(self.other,20.)):
            glb(root/'map/models/far.glb',[(0.,offset,0.),(6.,offset,0.),(0.,offset,6.)],[0,1,2])
        row=check.compare_map(self.reference,self.other,'map','source',.01,32)['objects'][0]
        self.assertEqual(row['status'],'pass')
        self.assertEqual(set(row['scene_surface_certificates']),{'model','lowd'})

    def test_far_binding_uses_raw_class_even_when_primary_uses_a_pad_alias(self):
        self.doc['source_game']='bf2'
        self.doc['classes'][0].update(geometry='raw_geo',build_zone='pad',base='prop')
        self.doc['classes'].append(dict(name='pad',geometry='Geo'))
        self.doc['geometries'].append(dict(name='raw_geo',model='model'))
        self.doc['models'].append('far')
        self.doc['lighting']=dict(near_scene_range=[100.,200.,300.])
        for root in (self.reference,self.other):
            glb(root/'map/models/far.glb',[(0.,10.,0.),(6.,10.,0.),(0.,10.,6.)],[0,1,2])
        source_doc=copy.deepcopy(self.doc);source_doc['geometries'][-1]['far']='far'
        self.reader.stop()
        self.reader=patch.object(check,'read_ron',side_effect=lambda path:(source_doc if self.other in path.parents else self.doc)
                                 if path.name=='manifest.ron' else self.raw_classes if path.name=='classes.ron' else {})
        self.reader.start()
        row=check.compare_map(self.reference,self.other,'map','source',.01,32)['objects'][0]
        self.assertEqual(row['status'],'unmeasured')

    def test_continuous_absolute_contact_can_accept_collision_fit_beyond_native_art(self):
        self.doc['classes'][0]['base']='prop'
        self.points=[(0,0,0),(6,0,0),(0,0,6)]
        for root in (self.reference,self.other):
            glb(root/'map/collision/geo.glb',self.points,[0,1,2])
        glb(self.reference/'map/models/model.glb',[(x,5,z) for x,y,z in self.points],[0,1,2])
        glb(self.other/'map/models/model.glb',self.points,[0,1,2])
        row=self.run_check()['objects'][0]
        self.assertEqual(row['status'],'pass',row.get('scene_surface_certificates'))
        self.assertEqual(row['scene_surface_certificates']['model']['method'],'absolute_active_contact_bound')
        self.assertEqual(check.certificate_problems(row['scene_surface_certificates']['model'],.1),[])
        self.assertGreater(row['render_to_reference_max_m'],4)

    def test_source_contact_policy_proves_one_sided_visibility_without_outlier_vertices(self):
        from render_scene_surface import camera_visibility_certificate,camera_problems
        self.doc['classes'][0]['base']='prop'
        self.run_check()  # Write/validate the ordinary fixture first.
        path=self.other/'map/models/model.glb'
        before=check.glb_geometry(path)
        edit_glb_document(path,lambda doc:doc['meshes'][0].update(extras={'contact_surface':True}))
        ref=check.Scene(self.reference,'map',reference_art=True);src=check.Scene(self.other,'map')
        receipt=camera_visibility_certificate(ref,src,(0,0))
        self.assertEqual(receipt['method'],'source_visibility_superset')
        self.assertTrue(receipt['reference']['can_fade'])
        self.assertFalse(receipt['source']['can_fade'])
        self.assertEqual(camera_problems(receipt),[])
        self.assertEqual(check.glb_geometry(path),before)
        self.assertEqual(self.run_check()['objects'][0]['status'],'pass')

    def test_contact_policy_is_strict_bool_and_cannot_hide_source_only(self):
        from render_scene_surface import camera_visibility_certificate
        path=self.reference/'map/models/model.glb'
        edit_glb_document(path,lambda doc:doc['meshes'][0].update(extras={'contact_surface':True}))
        ref=check.Scene(self.reference,'map',reference_art=True);src=check.Scene(self.other,'map')
        self.assertEqual(camera_visibility_certificate(ref,src,(0,0))['status'],'unmeasured')
        edit_glb_document(path,lambda doc:doc['meshes'][0].update(extras={'contact_surface':'true'}))
        self.assertEqual(self.run_check()['objects'][0]['status'],'unmeasured')

    def test_identically_skipped_neighbor_cannot_fill_a_real_missing_panel(self):
        self.test_scene_union_accepts_differently_partitioned_static_art()
        for root in (self.reference, self.other):
            edit_glb_document(root / 'map/models/other_model.glb',
                              lambda doc:doc['materials'][0].update(name='noicon'))
        rows = self.run_check()['objects']
        self.assertEqual(rows[0]['status'], 'unmeasured')
        self.assertEqual(rows[1]['world_draw_dependencies']['model']['status'], 'unmeasured')

    def test_opaque_texture_unregistered_while_file_remains_fails_actual_draw_check(self):
        from test_render_terrain_dependencies import ktx
        for root in (self.reference, self.other):
            edit_glb_document(root / 'map/models/model.glb',
                              lambda doc:doc['materials'][0].update(name='noicon'))
            (root / 'map/textures').mkdir()
            (root / 'map/textures/noicon.ktx2').write_bytes(ktx())
        self.doc['textures'] = ['noicon']
        changed = copy.deepcopy(self.doc)
        changed['textures'] = []
        self.reader.stop()
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: (
            changed if self.other in path.parents else self.doc) if path.name == 'manifest.ron' else {})
        self.reader.start()
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertEqual(row['world_draw_dependencies']['model']['status'], 'unmeasured')

    def test_another_mode_layer_cannot_hide_a_surface_gap(self):
        self.doc['classes'][0]['base'] = 'prop'
        self.doc['layers'] = [dict(name='other_mode', instances=[dict(self.inst)])]
        # The base source is missing a panel. An identical placed object in a
        # different mode's layer must not be used as base-scene evidence.
        points = [(0, 0, 0), (2, 0, 0), (2, 0, 2), (0, 0, 2)]
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', points, [0, 1, 2, 0, 2, 3])
        glb(self.reference / 'map/models/model.glb', points, [0, 1, 2, 0, 2, 3])
        glb(self.other / 'map/models/model.glb', points, [0, 1, 2])
        self.assertEqual(self.run_check()['objects'][0]['status'], 'unmeasured')


class ContinuousTests(unittest.TestCase):
    def test_scene_borrowing_certificate_requires_lifetime_evidence(self):
        exact = dict(status='pass', complete=True, method='identical_indexed_surface', maximum_observed_m=0.)
        proof = dict(status='pass', complete=True, method='whole_scene_bidirectional_hausdorff',
                     reference_to_source=exact, source_to_reference=exact)
        self.assertTrue(check.certificate_problems(proof, .01))

    def test_boolean_or_negative_distance_cannot_be_a_hausdorff_bound(self):
        exact = dict(status='pass', complete=True, method='identical_indexed_surface', maximum_observed_m=False)
        self.assertTrue(check.certificate_problems(exact, .01))
        proof = dict(status='pass', complete=True, method='continuous_directed_hausdorff',
                     certified_upper_m=-1., cells=1, certified_patches=1)
        self.assertTrue(check.certificate_problems(proof, .01))
        proof['certified_upper_m'] = False
        self.assertTrue(check.certificate_problems(proof, .01))
    def test_outward_bound_does_not_trust_an_underreported_bvh_distance(self):
        target = check.Surface([(0, 1, 0), (2, 1, 0), (0, 1, 2)], [0, 1, 2])
        domain = check.Surface([(0, 0, 0), (2, 0, 0), (0, 0, 2)], [0, 1, 2])
        # A wrong scalar from an ill-conditioned distance calculation cannot
        # become a proof: the selected target face is still one metre away.
        with patch.object(target, 'distance', return_value=0.), patch.object(target, 'closest_triangle',
                side_effect=lambda p: (0., p, target.triangles[0])):
            result = check.art_certificate(domain, target, .1, 10000)
        self.assertNotEqual(result['status'], 'pass')

    def test_outward_distance_handles_thin_large_offset_triangle(self):
        triangle = ((1e12, 0, 0), (1e12 + 1, 0, 0), (1e12, 1e-12, 0))
        upper = check.triangle_distance_upper((1e12 + .25, .25e-12, .001), triangle)
        self.assertTrue(math.isfinite(upper))
        self.assertGreaterEqual(upper, .001)
        self.assertLess(upper, .001000001)

    def test_late_original_face_witness_precedes_difficult_prefix_subdivision(self):
        patches = [check.triangle_patch(((x, 0, 0), (x + 1, 0, 0), (x, 0, 1)), ('test', i))
                   for i, x in enumerate((0, 10))]
        def value(p):
            delta = max(0., (p[0] - 1) / 9)
            return delta, delta, {}
        result = check.certify_surface(patches, value, 2., .01, 1)
        self.assertEqual(result['status'], 'offender')
        self.assertEqual(result['cells'], 0)
        self.assertEqual(result['prepass_patches'], 2)

    def test_convex_face_certificate_covers_interior_without_dense_samples(self):
        target = check.Surface([(0, 0, 0), (100, 0, 0), (0, 0, 100)], [0, 1, 2])
        result = check.art_certificate(target, target, .1, 1)
        self.assertEqual(result['status'], 'pass')
        self.assertLessEqual(result['certified_upper_m'], .1)

    def test_exact_triangle_union_proves_opposite_quad_triangulations(self):
        points = [(0, 0, 0), (100, 0, 0), (100, 0, 100), (0, 0, 100)]
        a = check.Surface(points, [0, 1, 2, 0, 2, 3])
        b = check.Surface(points, [0, 1, 3, 1, 2, 3])
        result = check.art_certificate(a, b, .01, 2)
        self.assertEqual(result['status'], 'pass')
        self.assertLess(result['certified_upper_m'], 1e-6)

    def test_exact_union_does_not_discard_a_small_interior_hole(self):
        points = [(0, 0, 0), (2, 0, 0), (2, 0, 2), (0, 0, 2), (1, 0, .999), (1, 0, 1.001), (1.001, 0, 1)]
        # Several target faces cover a square except a small true triangle.
        target = check.Surface(points, [0, 1, 4, 1, 6, 4, 1, 2, 6, 2, 5, 6, 2, 3, 5, 3, 4, 5, 3, 0, 4])
        self.assertIsNone(target.triangle_union_bound(((0, 0, 0), (2, 0, 0), (2, 0, 2)), .01))

    def test_exact_union_does_not_cover_a_nonplanar_gap(self):
        points = [(0, .02, 0), (2, .02, 0), (2, .02, 2), (0, .02, 2)]
        target = check.Surface(points, [0, 1, 3, 1, 2, 3])
        self.assertIsNone(target.triangle_union_bound(((0, 0, 0), (2, 0, 0), (2, 0, 2)), .01))

    def test_budget_exhaustion_cannot_pass(self):
        patches = [check.triangle_patch(((0, 0, 0), (1, 0, 0), (0, 0, 1)), ('test', i)) for i in range(2)]
        result = check.certify_surface(patches, lambda p: (0., 0., {}), 2., .1, 1)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertFalse(result['complete'])

    def test_conservative_reverse_bound_does_not_invent_a_physical_offender(self):
        patches = [check.triangle_patch(((0, 0, 0), (1, 0, 0), (0, 0, 1)), ('test', 0))]
        result = check.certify_surface(patches, lambda p: (2., 0., {}), 1., .1, 10)
        self.assertEqual(result['status'], 'unmeasured')
        self.assertEqual(result['maximum_observed_m'], 0.)

    def test_primitive_domains_cover_sphere_poles_and_cylinder_caps(self):
        transform = dict(rotation=[[1, 0, 0], [0, 1, 0], [0, 0, 1]], translation=[0, 0, 0])
        for kind, dims in (('sphere', [2, 0, 0]), ('cylinder', [2, 6, 0]), ('box', [2, 3, 4])):
            patches = list(check.primitive_patches(dict(kind=kind, dims=dims, **transform), 0))
            self.assertTrue(patches)
            for patch in patches:
                self.assertGreaterEqual(patch[3], 0)
                if patch[0] == 'parameter':
                    point, _radius, limits = patch[1]
                    u0, u1, v0, v1 = limits
                    for u in (u0, (u0 + u1) / 2, u1):
                        for v in (v0, (v0 + v1) / 2, v1):
                            self.assertLessEqual(math.dist(point(u, v), patch[2]), patch[3] + 1e-12)


class CliScopeTests(unittest.TestCase):
    def run_cli(self, extra, mutate=False):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            report = root / 'report.json'
            for name in ('ref','src'):
                (root/name).mkdir();(root/name/'manifest.ron').write_text('control')
            clean = dict(source='source', map='map', counts={'pass': 1}, placements=1, extra_source_placements=0,
                         terrain=dict(status='pass', complete=True), attachment=dict(status='pass', complete=True))
            argv = ['render_collision_check.py', '--reference', str(root / 'ref'), '--case', f'source={root / "src"}', '--out', str(report), *extra]
            # Isolate the CLI coverage boundary from the surface algorithm,
            # whose real-content/geometry checks have separate regressions.
            def compare(*_args,**_kwargs):
                if mutate:(root/'src/manifest.ron').write_text('changed')
                return clean
            with patch.object(sys, 'argv', argv), patch.object(check, 'compare_map', side_effect=compare), contextlib.redirect_stdout(io.StringIO()):
                result = check.main()
            return result, json.loads(report.read_text())

    def test_clean_full_catalog_is_eligible(self):
        result, report = self.run_cli([])
        self.assertEqual(result, 0)
        self.assertTrue(report['acceptance_eligible'])

    def test_clean_scoped_map_cannot_return_acceptance_success(self):
        result, report = self.run_cli(['--map', 'bf1_rhn1'])
        self.assertEqual(result, 1)
        self.assertFalse(report['acceptance_eligible'])

    def test_explicit_map_selection_is_diagnostic_even_when_it_lists_the_catalog(self):
        result,report=self.run_cli([arg for key in check.REQUIRED_MAPS for arg in ('--map',key)])
        self.assertEqual(result,1)
        self.assertFalse(report['acceptance_eligible'])

    def test_rebake_during_measurement_cannot_return_acceptance_success(self):
        result,report=self.run_cli([],mutate=True)
        self.assertEqual(result,1)
        self.assertEqual(report['input_stability']['status'],'unmeasured')

    def test_old_margin_diagnostic_cannot_return_acceptance_success(self):
        result, report = self.run_cli(['--tolerance', '.1'])
        self.assertEqual(result, 1)
        self.assertFalse(report['acceptance_eligible'])


class LifetimeTests(unittest.TestCase):
    def inputs(self, rows):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        folder = root / 'map'
        folder.mkdir()
        (folder / 'classes.ron').write_text('[]')
        return ClassInputs(root, folder, 'bf1', reader=lambda _path: rows)

    def test_complete_static_class_chain_can_supply_coverage(self):
        values = self.inputs([dict(name='wall', base='wall_base', props=[]), dict(name='wall_base', base='prop', props=[])])
        self.assertTrue(values.static_lifetime(['wall'])['eligible'])

    def test_runtime_turret_alias_cannot_be_borrowed_as_a_static_prop(self):
        values = self.inputs([dict(name='wall', base='armedbuilding', props=[])])
        self.assertFalse(values.static_lifetime(['wall'])['eligible'])

    def test_authored_health_needs_a_lifetime_proof(self):
        values = self.inputs([dict(name='crate', base='prop', props=[[fnv('MaxHealth'), '100']])])
        self.assertFalse(values.static_lifetime(['crate'])['eligible'])

    def test_destroyed_geometry_dependency_cannot_be_assumed_static(self):
        values = self.inputs([dict(name='crate', base='prop', props=[[fnv('DestroyedGeometryName'), 'dead_crate']])])
        self.assertFalse(values.static_lifetime(['crate'])['eligible'])

    def test_ambiguous_class_stack_cannot_supply_a_static_proof(self):
        values = self.inputs([dict(name='wall', base='prop', props=[]), dict(name='wall', base='armedbuilding', props=[])])
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            values.static_lifetime(['wall'])

if __name__ == '__main__':
    unittest.main()
