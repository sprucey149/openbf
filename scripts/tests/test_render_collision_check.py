"""Discriminating surface/placement checks; no game content required."""
import json
import copy
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
import sys
sys.path.insert(0, str(SCRIPTS))
import render_collision_check as check


def glb(path, vertices, indices):
    packed = b''.join(struct.pack('<3f', *v) for v in vertices)
    packed += b''.join(struct.pack('<I', i) for i in indices)
    doc = dict(asset={'version': '2.0'}, buffers=[{'byteLength': len(packed)}],
               bufferViews=[{'buffer': 0, 'byteLength': len(vertices) * 12},
                            {'buffer': 0, 'byteOffset': len(vertices) * 12, 'byteLength': len(indices) * 4}],
               accessors=[{'bufferView': 0, 'componentType': 5126, 'type': 'VEC3', 'count': len(vertices)},
                          {'bufferView': 1, 'componentType': 5125, 'type': 'SCALAR', 'count': len(indices)}],
               meshes=[{'primitives': [{'attributes': {'POSITION': 0}, 'indices': 1}]}])
    header = json.dumps(doc).encode()
    header += b' ' * (-len(header) % 4)
    packed += b'\0' * (-len(packed) % 4)
    data = struct.pack('<4sII', b'glTF', 2, 12 + 8 + len(header) + 8 + len(packed))
    data += struct.pack('<I4s', len(header), b'JSON') + header + struct.pack('<I4s', len(packed), b'BIN\0') + packed
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


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
                        geometries=[dict(name='geo', model='MODEL')], instances=[self.inst],
                        collision=[dict(geometry='Geo', glb='collision/geo.glb', prims=[])], paths=[], layers=[])
        for root in (self.reference, self.other):
            glb(root / 'map/models/model.glb', self.points, [0, 1, 2])
            glb(root / 'map/collision/geo.glb', self.points, [0, 1, 2])
        self.reader = patch.object(check, 'read_ron', side_effect=lambda path: self.doc if path.name == 'manifest.ron' else {})
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
        self.assertEqual(row['status'], 'offender')
        self.assertEqual(row['surface_residual_increase_max_m'], 2)
        self.assertEqual((self.other / 'map/collision/geo.glb').read_bytes(), collision_before)

    def test_different_collision_cannot_pass_as_art_alignment(self):
        glb(self.other / 'map/collision/geo.glb', [(x, y + 1, z) for x, y, z in self.points], [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertFalse(row['collision_identical'])
        self.assertIn('collision differs', row['error'])

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
        self.assertEqual(row['status'], 'offender')

    def test_missing_render_is_unmeasured(self):
        (self.other / 'map/models/model.glb').unlink()
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'unmeasured')
        self.assertIn('missing render model', row['error'])

    def test_extra_outside_art_does_not_change_playable_bounds(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([-1, 0, -1], []), ([3, 0, -1], []),
                                                            ([3, 0, 3], []), ([-1, 0, 3], [])])]
        glb(self.other / 'map/models/model.glb', self.points + [(100, 0, 100), (102, 0, 100), (100, 0, 102)],
            [0, 1, 2, 3, 4, 5])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'pass')
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
        reference = [(0, 5, 0), (1, 0, 0), (1, 0, 1)]
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', reference, [0, 1, 2])
        glb(self.reference / 'map/models/model.glb', reference, [0, 1, 2])
        glb(self.other / 'map/models/model.glb', [(-.0001, 5, 0), (1, 0, 0), (1, 0, 1)], [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertGreater(row['playable_render_bounds_max_delta_m'], 4)
        self.assertLess(row['render_bounds_max_delta_m'], .001)
        self.assertFalse(row['render_bounds_regressed'])
        self.assertEqual(row['status'], 'unmeasured')
        self.assertIn('ambiguous', row['error'])

    def test_fixed_outside_envelope_cannot_certify_shifted_inside_vertex_bounds(self):
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
        self.assertEqual(row['status'], 'unmeasured')

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
        self.assertEqual(row['status'], 'offender')

    def test_art_leaving_playable_bounds_cannot_hide_behind_outside_collision(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([-1, 0, -1], []), ([3, 0, -1], []),
                                                            ([3, 0, 3], []), ([-1, 0, 3], [])])]
        outside = [(x + 50, y, z) for x, y, z in self.points]
        for root in (self.reference, self.other):
            glb(root / 'map/collision/geo.glb', outside, [0, 1, 2])
        glb(self.other / 'map/models/model.glb', outside, [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertTrue(row['reference_art_in_play'])
        self.assertFalse(row['source_art_in_play'])
        self.assertEqual(row['status'], 'offender')

    def test_outside_boundary_cannot_fail_in_bounds_gate(self):
        self.doc['paths'] = [dict(name='boundary', nodes=[([50, 0, 50], []), ([60, 0, 50], []), ([50, 0, 60], [])])]
        glb(self.other / 'map/models/model.glb', [(x, y + 2, z) for x, y, z in self.points], [0, 1, 2])
        row = self.run_check()['objects'][0]
        self.assertEqual(row['status'], 'outside_playable_samples')
        self.assertEqual(row['playable_samples'], 0)


if __name__ == '__main__':
    unittest.main()
