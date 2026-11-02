"""Attachment checks must see late vertices, skin bindings and missing inputs."""
import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import render_attachment_check as check


def glb(path, last_y=0, joint=0, animated_x=0, animation_name='idle', alpha=False):
    positions = [(0, 0, 0), (1, 0, 0), (0, last_y, 1)]
    binary = b''.join(struct.pack('<3f', *p) for p in positions)
    binary += bytes([joint, 0, 0, 0] * 3)
    binary += struct.pack('<3I', 0, 1, 2)
    binary += struct.pack('<3f', animated_x, 0, 0)
    doc = {'asset': {'version': '2.0'}, 'buffers': [{'byteLength': len(binary)}],
           'bufferViews': [{'buffer': 0, 'byteOffset': at, 'byteLength': size}
                           for at, size in ((0, 36), (36, 12), (48, 12), (60, 12))],
           'accessors': [{'bufferView': view, 'componentType': component, 'type': kind, 'count': count}
                         for view, component, kind, count in ((0, 5126, 'VEC3', 3), (1, 5121, 'VEC4', 3),
                                                               (2, 5125, 'SCALAR', 3), (3, 5126, 'VEC3', 1))],
           'meshes': [{'primitives': [{'attributes': {'POSITION': 0, 'JOINTS_0': 1}, 'indices': 2}]}],
           'animations': [{'name': animation_name, 'channels': [], 'samplers': [{'input': 2, 'output': 3}]}]}
    if alpha:
        doc['materials'] = [{'name': 'alpha', 'alphaMode': 'MASK'}]
        doc['meshes'][0]['primitives'][0]['material'] = 0
    header = json.dumps(doc).encode()
    header += b' ' * (-len(header) % 4)
    binary += b'\0' * (-len(binary) % 4)
    path.write_bytes(struct.pack('<4sII', b'glTF', 2, 28 + len(header) + len(binary))
                     + struct.pack('<I4s', len(header), b'JSON') + header
                     + struct.pack('<I4s', len(binary), b'BIN\0') + binary)


def receipt():
    return {'method': 'exhaustive_attachment_dependency_identity_v1', 'complete': True,
            'status': 'pass', 'categories': {category: {'items': 0, 'counts': {}, 'status': 'pass'}
                                             for category in check.REQUIRED_CATEGORIES},
            'objects': [], 'errors': []}


class AttachmentTests(unittest.TestCase):
    def test_direct_hologram_load_cannot_escape_class_geometry_inventory(self):
        with tempfile.TemporaryDirectory() as folder:
            roots = [Path(folder) / label for label in ('ref', 'src')]
            for root in roots:
                path = root / 'common_bf1/models/com_icon_alliance.glb'
                path.parent.mkdir(parents=True)
                glb(path)
            a, b = [check.hologram_dependencies(root, 'map', 'bf1') for root in roots]
            self.assertEqual(a, b)
            self.assertTrue(all(row['missing'] for row in a if row['name'] != 'alliance'))
            glb(roots[1] / 'common_bf1/models/com_icon_alliance.glb', last_y=2)
            changed = check.hologram_dependencies(roots[1], 'map', 'bf1')
            self.assertNotEqual(a[0]['sha256'], changed[0]['sha256'])

    def test_hologram_turret_prefers_map_local_model(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            common = root / 'common_bf1/models/com_icon_turret.glb'
            local = root / 'map/models/com_icon_turret.glb'
            common.parent.mkdir(parents=True)
            local.parent.mkdir(parents=True)
            glb(common)
            glb(local, last_y=2)
            rows = check.hologram_dependencies(root, 'map', 'bf1')
            self.assertEqual(rows[-1]['source'], 'map')
            before = rows[-1]['sha256']
            common.unlink()
            self.assertEqual(check.hologram_dependencies(root, 'map', 'bf1')[-1]['sha256'], before)

    def test_identity_includes_all_vertices_joints_and_animation_keys(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder) / 'base.glb'
            other = Path(folder) / 'other.glb'
            glb(base)
            for change in ({}, {'last_y': 9}, {'joint': 1}, {'animated_x': 3}, {'animation_name': 'walk'}):
                glb(other, **change)
                self.assertEqual(check.shape(base) == check.shape(other), not bool(change), change)

    def test_alpha_shape_identity_includes_texture_and_invalidates_edits(self):
        with tempfile.TemporaryDirectory() as folder:
            roots = [Path(folder) / label for label in ('a', 'b')]
            for root in roots:
                (root / 'models').mkdir(parents=True)
                (root / 'textures').mkdir()
                glb(root / 'models' / 'model.glb', alpha=True)
                (root / 'textures/alpha.ktx2').write_bytes(b'alpha-a')
            a, b = [root / 'models/model.glb' for root in roots]
            self.assertEqual(check.shape(a), check.shape(b))
            (roots[1] / 'textures/alpha.ktx2').write_bytes(b'alpha-b')
            self.assertNotEqual(check.shape(a), check.shape(b))

    def test_water_plane_measures_last_vertex(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            glb(root / 'water.glb', last_y=4)
            _signature, worst = check._water(root, {'water': {'glb': 'water.glb', 'height': 0}})
            self.assertEqual(worst, 4)

    def test_missing_dynamic_directories_are_not_an_empty_pass(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'map').mkdir()
            (root / 'common_bf1').mkdir()
            (root / 'map/manifest.ron').write_text('(source_game:"bf1",classes:[],instances:[])')
            (root / 'common_bf1/common.ron').write_text('(classes:[])')
            report = check.attachment_map(root, root, 'map')
            self.assertFalse(report['complete'])
            self.assertEqual(report['status'], 'unmeasured')
            self.assertIn('soldier_feet: required side inventory missing', report['errors'])

    def test_validator_requires_every_category_and_consistent_counts(self):
        value = receipt()
        self.assertEqual(check.attachment_problems(value), [])
        value['categories'].pop('water')
        self.assertIn('attachment categories incomplete or unknown', check.attachment_problems(value))
        value = receipt()
        value['categories']['water']['items'] = 1
        self.assertIn('water: incomplete or inconsistent inventory', check.attachment_problems(value))

    def test_shape_difference_is_structurally_valid_but_not_a_proof(self):
        value = receipt()
        value['objects'] = [check._row('vehicles_hulls', 'hull', 'a', 'b')]
        value['complete'], value['status'] = False, 'unmeasured'
        value['categories']['vehicles_hulls'] = {'items': 1, 'counts': {'unmeasured': 1}, 'status': 'unmeasured'}
        self.assertEqual(check.attachment_problems(value), [])
        wrong = copy.deepcopy(value)
        wrong['objects'][0]['residual_excess_bound_m'] = 0
        self.assertIn('unmeasured attachment claims a bound', check.attachment_problems(wrong))


if __name__ == '__main__':
    unittest.main()
