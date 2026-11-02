import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import world_fingerprint as fp


def glb(path, positions, sphere=None, node=None):
    data = b''.join(struct.pack('<3f', *p) for p in positions)
    indices = list(range(len(positions)))
    binary = data + struct.pack('<' + 'I' * len(indices), *indices)
    doc = {'asset': {'version': '2.0'}, 'buffers': [{'byteLength': len(binary)}],
           'bufferViews': [{'buffer': 0, 'byteOffset': 0, 'byteLength': len(data)},
                           {'buffer': 0, 'byteOffset': len(data), 'byteLength': len(indices) * 4}],
           'accessors': [{'bufferView': 0, 'componentType': 5126, 'count': len(positions), 'type': 'VEC3'},
                         {'bufferView': 1, 'componentType': 5125, 'count': len(indices), 'type': 'SCALAR'}],
           'meshes': [{'primitives': [{'attributes': {'POSITION': 0}, 'indices': 1}]}]}
    if sphere is not None:
        doc["meshes"][0]["extras"] = {"sphere": sphere}
    if node is not None:
        doc["nodes"] = [node]
    text = json.dumps(doc).encode()
    text += b' ' * (-len(text) % 4)
    body = struct.pack('<I4s', len(text), b'JSON') + text + struct.pack('<I4s', len(binary), b'BIN\0') + binary
    path.write_bytes(struct.pack('<4sII', b'glTF', 2, len(body) + 12) + body)


class Fingerprint(unittest.TestCase):
    def test_glb_decodes_only_positions_and_indices(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'mesh.glb'
            points = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]
            glb(path, points)
            self.assertEqual(fp.glb_geometry(path), (points, [0, 1, 2]))
            before = fp.digest(fp.glb_geometry(path))
            points[1][0] = 2.
            glb(path, points)
            self.assertNotEqual(fp.digest(fp.glb_geometry(path)), before)

    def test_playable_cells_include_outside_interpolation_corner(self):
        square = [[(.25, .25), (.75, .25), (.75, .75), (.25, .75)]]
        self.assertTrue(fp.cell_in_play(square, 0., 0., 1., 1.))
        self.assertFalse(fp.cell_in_play(square, 2., 2., 3., 3.))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 't.glb'
            # 3x3 terrain; first cell contains boundary, none of its corners do.
            points = [[float(x), 0., float(z)] for z in range(3) for x in range(3)]
            glb(path, points)
            doc = {'terrain_glb': path.name, 'paths': [{'name': 'boundary', 'nodes': [([x, 0., z], []) for x, z in square[0]]}]}
            grid, play, counts = fp.terrain(doc, Path(tmp))
            self.assertEqual(len(play), 4)
            self.assertEqual(counts['boundary_paths'], 1)
            points[0][1] = 4.
            glb(path, points)
            self.assertNotEqual(fp.digest(play), fp.digest(fp.terrain(doc, Path(tmp))[1]))
            points[0][1] = 0.
            points[-1][1] = 7.
            glb(path, points)
            after = fp.terrain(doc, Path(tmp))
            self.assertEqual(fp.digest(play), fp.digest(after[1]))
            self.assertNotEqual(fp.digest(grid), fp.digest(after[0]))

    def test_canonical_query_window_keeps_render_extent_out_of_playable_hash(self):
        def points(n):
            return [[float(x - n // 2), float((x - n // 2) ** 2 + z - n // 2 + 1),
                     float(z - n // 2 + 1)] for z in range(n) for x in range(n)]
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            glb(folder / 'ps2.glb', points(4))
            glb(folder / 'pc.glb', points(8))
            a = fp.terrain({'terrain_glb': 'ps2.glb'}, folder)
            b = fp.terrain({'terrain_glb': 'pc.glb', 'terrain_query_size': 4}, folder)
            self.assertEqual(a[1], b[1])
            self.assertEqual({k: v for k, v in a[2].items() if k.startswith('query_')},
                             {k: v for k, v in b[2].items() if k.startswith('query_')})
            self.assertNotEqual(fp.digest(a[0]), fp.digest(b[0]))
            for size in (1, 3, 9, 4.5, float("inf"), float("nan")):
                with self.assertRaises(ValueError):
                    fp.terrain({'terrain_glb': 'pc.glb', 'terrain_query_size': size}, folder)

    def test_nav_sidecars_are_required_and_hashed(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            doc = {'nav': {'plan': {'routes': 'route.bin', 'plnp': {'records': 'patch.bin'}}}}
            with self.assertRaisesRegex(ValueError, 'required gameplay input missing'):
                fp.world(doc, folder)
            (folder / 'route.bin').write_bytes(b'route-a')
            (folder / 'patch.bin').write_bytes(b'patch-a')
            before = fp.digest(fp.world(doc, folder))
            (folder / 'route.bin').write_bytes(b'route-b')
            self.assertNotEqual(before, fp.digest(fp.world(doc, folder)))

    def test_presentation_values_are_excluded_but_setter_presence_remains(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'classes.ron'
            def write(health, sound):
                path.write_text(f'[(name: "a", kind: "entc", base: "soldier", props: [(2822513700, "{health}"), ({next(iter(fp.gameplay_table.PRESENTATION))}, "{sound}")])]')
            write(10, 'sound_a')
            before = fp.digest(fp.class_defs(path))
            write(10, 'sound_b')
            self.assertEqual(before, fp.digest(fp.class_defs(path)))
            write(20, 'sound_b')
            self.assertNotEqual(before, fp.digest(fp.class_defs(path)))
            write(10, 'sound_a')
            path.write_text('[(name: "a", kind: "entc", base: "soldier", props: [(2822513700, "10")])]')
            self.assertNotEqual(before, fp.digest(fp.class_defs(path)))

    def test_station_blast_sphere_and_saber_poses_change_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / 'models').mkdir()
            path = folder / 'models/station.glb'
            points = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]
            doc = {'geometries': [{'name': 'station', 'model': 'station'}]}
            glb(path, points, sphere=[0., 0., 0., 1.], node={'name': 'bone_root', 'translation': [0., 0., 0.]})
            before = fp.digest(fp.model_spheres(doc, folder))
            pose = fp.pose_hash(str(path), 1, path.stat().st_size)
            glb(path, points, sphere=[0., 0., 0., 20.], node={'name': 'bone_root', 'translation': [0., 1., 0.]})
            self.assertNotEqual(before, fp.digest(fp.model_spheres(doc, folder)))
            self.assertNotEqual(pose, fp.pose_hash(str(path), 2, path.stat().st_size))
            # Art-only vertex resolution has no effect on the collision pose hash.
            after = fp.pose_hash(str(path), 2, path.stat().st_size)
            glb(path, points + points, sphere=[0., 0., 0., 20.], node={'name': 'bone_root', 'translation': [0., 1., 0.]})
            self.assertEqual(after, fp.pose_hash(str(path), 3, path.stat().st_size))

    def test_missing_saber_inputs_are_explicit(self):
        with tempfile.TemporaryDirectory() as tmp:
            data, missing = fp.saber_inputs(Path(tmp))
            self.assertEqual(data, [])
            self.assertEqual(missing, ['side_bf2/heroes.ron'])

    def test_missing_saber_combo_and_weapon_nodes_are_required(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            side = folder / 'side_bf2'
            (side / 'soldiers').mkdir(parents=True)
            (folder / 'classes').mkdir()
            (side / 'heroes.ron').write_text('(heroes: [(class: "luke", geometry: "luke", side: "all")], weapons: ["saber"])')
            fnv = fp.version_census.extract_lua.fnv1a
            (folder / 'classes/bf2_all.ron').write_text(f'[(name: "luke", base: "soldier", props: [({fnv("WeaponName")}, "saber")]), (name: "saber", base: "melee", props: [({fnv("ComboAnimationBank")}, "human saber combo")])]')
            body = side / 'soldiers/luke.glb'
            glb(body, [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]])
            body.with_suffix('.sets.ron').write_text('[]')
            body.with_suffix('.motion.ron').write_text('[]')
            _, missing = fp.saber_inputs(folder)
            self.assertIn('side_bf2/soldiers/luke.combos.ron', missing)
            self.assertIn('side_bf2/weapons/saber.nodes.ron', missing)

    def test_collision_resource_order_is_lookup_only_but_body_order_matters(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            a = {'geometry': 'a', 'prims': [{'name': 'first'}, {'name': 'second'}]}
            z = {'geometry': 'z', 'prims': []}
            before = fp.digest(fp.collisions({'collision': [z, a]}, folder))
            self.assertEqual(before, fp.digest(fp.collisions({'collision': [a, z]}, folder)))
            changed = {'geometry': 'a', 'prims': list(reversed(a['prims']))}
            self.assertNotEqual(before, fp.digest(fp.collisions({'collision': [changed, z]}, folder)))

    def test_crossing_edges_are_not_missed(self):
        strip = [[(-1., .4), (2., .4), (2., .6), (-1., .6)]]
        self.assertTrue(fp.cell_in_play(strip, 0., 0., 1., 1.))
        self.assertFalse(fp.intersects((0., 0.), (1., 0.), (2., 0.), (3., 0.)))


if __name__ == '__main__':
    unittest.main()
