"""Correction tables preserve absolute targets and keep geometry private."""
import copy
import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gameplay_corrections as c
import export_public as export


def chunk(tag, payload):
    return tag + struct.pack('<I', len(payload)) + payload + b'\0' * (-len(payload) % 4)


def terrain(value, n=16):
    # Literal RLE runs of 128; one HEXP tile.
    heights = (bytes([127]) + bytes([value]) * 128) * (n * n // 128)
    info = struct.pack('<4fH', 8, 0.5, -100, 100, n) + b'\0' * 10
    return (chunk(b'NAME', b'ground\0') + chunk(b'INFO', info) + chunk(b'HGT8', heights)
            + chunk(b'HEXP', b'\0' * (n * n // 256 * 8)))



def animation_bank(root_z, *, frames=2, root_name='DummyRoot'):
    mina = struct.pack('<QIHH', 0, 123, frames, 1)
    track = struct.pack('<8I4f', c.geometry.crc32_lower(root_name), 0, 0, 0, 0, 0, 0, 0, 0, 0, root_z, 1)
    streams = b'\0\0' + b'\0' * max(frames - 1, 0)  # a flat stream every channel's offset 0 can decode
    header = b'SMNA' + struct.pack('<4IH2x', 0, 8, 1, len(streams), 1)
    body = header + chunk(b'MINA', mina) + chunk(b'TNJA', track) + chunk(b'TADA', streams)
    return chunk(b'NAME', b'rep_walk_kaadu\0') + chunk(b'BIN_', body)

def correction():
    return {'schema': 2, 'reference': 'ps2-v1', 'game': 'bf1', 'platform': 'pc',
            'presentation_hashes': [], 'files': [{'file': 'map/map1.lvl', 'sha256': 'a' * 64,
                'terrain': [{'scope': [], 'name': 'ground', 'ordinal': 0,
                    'grid_size': 64, 'grid_unit': 1, 'height_scale': 0.5,
                    'boundaries': [[[-50, -50], [50, -50], [50, 50], [-50, 50]]],
                    'height_samples': [[x, z, 1] for x in range(20) for z in range(20)]}]}]}


class CorrectionTests(unittest.TestCase):
    def test_only_absolute_height_values_inside_boundary_are_emitted(self):
        loc = {'scope': [], 'name': 'ground', 'ordinal': 0}
        polygon = [[(-8, -8), (8, -8), (8, 8), (-8, 8)]]
        row, report = c.terrain_record(loc, terrain(6), terrain(4), polygon)
        # Ground inside the line reads all four corners of its cell, so the row reaches one cell past it and no further.
        reach = (-16., -8., 0., 8., 16.)
        self.assertEqual(row['height_samples'], [[x, z, 3.0] for z in reach for x in reach])
        self.assertEqual(report['changed_inside'], 25)
        self.assertEqual(report['changed_outside'], 231)
        self.assertTrue(all(c.supports_bounds(polygon, x, z, 8.0) for x, z, _ in row['height_samples']))
        self.assertFalse(c.supports_bounds(polygon, 24.0, 0.0, 8.0))
        self.assertFalse(c.in_bounds(polygon, 16.0, 0.0))

    def test_larger_donor_keeps_canonical_query_size_without_height_payload(self):
        loc = {'scope': [], 'name': 'ground', 'ordinal': 0}
        row, report = c.terrain_record(loc, terrain(4, 16), terrain(4, 32), [])
        self.assertEqual(row['grid_size'], 16)
        self.assertEqual(row['height_samples'], [])
        self.assertEqual(row['boundaries'], [])
        self.assertTrue(report['missing_boundary'])
        self.assertIsNone(c.terrain_record(loc, terrain(4), terrain(4), [])[0])

    def test_root_signature_sees_constant_offset_and_clock_not_nonroot_pose(self):
        a, b = animation_bank(0.000002861), animation_bank(6.5148253)
        self.assertNotEqual(c.root_motion_signature(a, 'bf1'), c.root_motion_signature(b, 'bf1'))
        self.assertNotEqual(c.root_motion_signature(a, 'bf1'), c.root_motion_signature(animation_bank(0.000002861, frames=3), 'bf1'))
        self.assertEqual(c.root_motion_signature(animation_bank(1, root_name='bone_head'), 'bf1'), [])
        self.assertEqual(c.root_motion_signature(animation_bank(2, root_name='bone_head'), 'bf1'), [])

    def test_changed_root_bank_leaves_as_instructions_over_the_donor(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            ref, donor = root / 'ref', root / 'donor'
            (ref / 'side').mkdir(parents=True)
            (donor / 'side').mkdir(parents=True)
            target = chunk(b'zaa_', animation_bank(0.000002861))
            reference = chunk(b'ucfb', target)
            original = chunk(b'ucfb', chunk(b'zaa_', animation_bank(6.5148253)))
            (ref / 'side/gun.lvl').write_bytes(reference)
            (donor / 'side/gun.lvl').write_bytes(original)
            metadata = {'schema': 1, 'source': {'game': 'bf1', 'platform': 'ps2'},
                        'files': [{'name': 'side/gun.lvl', 'sha256': c.digest(reference)}], 'entities': [], 'missions': []}
            other = copy.deepcopy(metadata)
            other['source']['platform'] = 'pc'
            other['files'][0]['sha256'] = c.digest(original)
            report = c.generate(ref, donor, root / 'table.json', root / 'programs.json', reference_census=metadata, donor_census=other)
            public = json.loads((root / 'table.json').read_bytes())
            programs = json.loads((root / 'programs.json').read_bytes())
            resource = public['files'][0]['resources'][0]
            self.assertEqual(resource['kind'], 'zaa_')
            self.assertEqual(report['programs']['bank'], 1)
            self.assertEqual(report['instructions']['root_tracks_written'], 1)
            bank = programs['bank'][resource['program']]
            self.assertEqual(bank['clips'][0]['from'], 0)
            self.assertNotIn('private_file', json.dumps(public))

    def test_model_bounds_keep_nullable_sphere_and_never_export_vertices(self):
        info = b"\0" * 16 + struct.pack("<12fI", *range(12), 5) + b"\0" * 4
        payload = chunk(b"INFO", info) + chunk(b"SPHR", struct.pack("<4f", 1, 2, 3, 4))
        loc = {'scope': [], 'name': 'station', 'ordinal': 0}
        record = c.model_bounds(loc, payload)
        self.assertEqual(record['sphere'], [1., 2., 3., 4.])
        self.assertEqual(record['vis_box'], [[6., 7., 8.], [9., 10., 11.]])
        self.assertNotIn('vertex_box', record)
        self.assertIsNone(c.model_bounds(loc, chunk(b"INFO", info))['sphere'])

    def test_missing_boundary_never_exports_changed_heights(self):
        row, report = c.terrain_record({'scope': [], 'name': 'x', 'ordinal': 0}, terrain(6), terrain(4), [])
        self.assertIsNone(row)
        self.assertTrue(report['missing_boundary'])
        self.assertEqual(report['changed_outside'], 256)

    def test_world_order_uses_original_donor_indices_without_exporting_names(self):
        bad_name = 'c' * 100
        a = {'kind': 'inst', 'name': bad_name, 'ordinal': 0, 'transform': [1]}
        b = {'kind': 'inst', 'name': 'cp1', 'ordinal': 0, 'transform': [2]}
        target = {'scope': [], 'name': 'world', 'ordinal': 0, 'terrain_name': '', 'sky_name': '', 'records': [a, b]}
        existing = {**target, 'records': [b, a]}
        patch = c.world_difference(target, existing)
        self.assertEqual(patch['records'], [])
        self.assertEqual(patch['record_order'], [['donor', 1], ['donor', 0]])
        self.assertNotIn(bad_name, json.dumps(patch))

    def test_compact_decimal_numbers_preserve_float32_bits(self):
        values = [c.geometry.f32(v) for v in (0.1, -75.314, 1e-20, 1e20, 0.0, 1.0 / 3.0)]
        compact = json.loads(c.encoded(values))
        self.assertEqual([struct.pack('<f', v) for v in compact], [struct.pack('<f', v) for v in values])
        self.assertEqual(c.compact_numbers(0.1234567890123), 0.1234567890123)

    def test_config_decode_distinguishes_names_from_numbers_and_refuses_truncation(self):
        payload = struct.pack('<IBIfI', 12, 2, 8, 6.0, 5) + b'sky0\0'
        nodes = c.config_nodes(chunk(b'DATA', payload))
        self.assertEqual(nodes, [{'hash': 12, 'arguments': ['sky0', 6.0], 'children': []}])
        with self.assertRaisesRegex(ValueError, 'short'):
            c.config_nodes(chunk(b'DATA', payload[:-1]))

    def test_unknown_properties_remain_explicit_donor_preserves(self):
        prop = chunk(b'PROP', struct.pack('<I', 9) + b'Words that are free form prose.\0')
        self.assertEqual(c.props(prop), [])
        self.assertEqual(c.preserve_hashes(prop), [9])

    def test_fingerprint_mismatch_stops_before_public_table_is_written(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            ref, donor = root / 'ref', root / 'donor'
            ref.mkdir(); donor.mkdir()
            raw = chunk(b'ucfb', chunk(b'tern', terrain(3)))
            (ref / 'map.lvl').write_bytes(raw)
            (donor / 'map.lvl').write_bytes(raw)
            metadata = {'schema': 1, 'source': {'game': 'bf1', 'platform': 'ps2'},
                        'files': [{'name': 'map.lvl', 'sha256': c.digest(raw)}], 'entities': [], 'missions': []}
            other = copy.deepcopy(metadata)
            other['source']['platform'] = 'pc'
            other['files'][0]['sha256'] = '0' * 64
            with self.assertRaisesRegex(ValueError, 'fingerprint changed'):
                c.generate(ref, donor, root / 'out.json', root / 'private', reference_census=metadata, donor_census=other)
            self.assertFalse((root / 'out.json').exists())


class ExportCorrectionTests(unittest.TestCase):
    path = 'engine/crates/manifest/data/gameplay/corrections-v1/bf1-pc.json'

    def test_approved_in_bounds_heights_can_exceed_old_numeric_block_limit(self):
        self.assertEqual(export.generated_data_problems(self.path, c.encoded(correction())), [])

    def test_out_of_bounds_height_and_foreign_shape_key_are_refused(self):
        data = correction()
        data['files'][0]['terrain'][0]['height_samples'][0][0] = 100
        self.assertTrue(any('outside playable' in p for p in export.generated_data_problems(self.path, c.encoded(data))))
        data = correction()
        data['files'][0]['terrain'][0]['vertices'] = [[0, 0, 0]]
        self.assertTrue(any('schema' in p for p in export.generated_data_problems(self.path, c.encoded(data))))

    def test_a_height_may_reach_one_cell_past_the_line_and_no_further(self):
        # correction() has cells of 1 and a boundary ending at 50.
        data = correction()
        data['files'][0]['terrain'][0]['height_samples'] = [[51, 0, 1]]
        self.assertEqual(export.generated_data_problems(self.path, c.encoded(data)), [])
        data['files'][0]['terrain'][0]['height_samples'] = [[52, 0, 1]]
        self.assertTrue(any('outside playable' in p for p in export.generated_data_problems(self.path, c.encoded(data))))

    def test_a_row_cannot_widen_its_own_reach_with_a_larger_cell(self):
        data = correction()
        data['files'][0]['terrain'][0]['grid_unit'] = 100000
        data['files'][0]['terrain'][0]['height_samples'] = [[5000, 0, 1]]
        self.assertTrue(any('height grid' in p for p in export.generated_data_problems(self.path, c.encoded(data))))

    def test_ordinary_mesh_blocks_and_private_paths_remain_refused(self):
        data = correction()
        data['files'][0]['classes'] = [[x, x, x] for x in range(500)]
        self.assertTrue(any('asset-shaped' in p for p in export.generated_data_problems(self.path, c.encoded(data))))
        self.assertTrue(export.problems_in('engine/private-gameplay/corrections-v1/mesh.json', b'{}'))

    def test_a_program_table_cannot_carry_a_mesh_outright(self):
        path = 'engine/crates/manifest/data/gameplay/corrections-v1/bf1-pc-programs.json'
        table = {'schema': 2, 'reference': 'ps2-v1', 'game': 'bf1', 'platform': 'pc', 'skeleton': [], 'route': [], 'bank': [],
                 'collision': [{'n': 'mesh', 'b': [0] * 6, 'v': [4, 1, 2, 3] * 5000, 'p': []}]}
        self.assertTrue(export.generated_data_problems(path, c.encoded(table)))
        table['collision'][0]['v'] = [4, 1, 2, 3] * 3
        self.assertEqual(export.generated_data_problems(path, c.encoded(table)), [])
        table['collision'][0]['v'] = [9, 1, 2, 3]
        self.assertTrue(export.generated_data_problems(path, c.encoded(table)))
        table['collision'][0]['v'] = []
        table['route'] = [[[1, *range(9000)]]]
        self.assertTrue(any('route words' in p for p in export.generated_data_problems(path, c.encoded(table))))

    def program_problems(self, **parts):
        table = {'schema': 2, 'reference': 'ps2-v1', 'game': 'bf1', 'platform': 'pc', 'collision': [], 'skeleton': [], 'route': [], 'bank': [], **parts}
        return export.generated_data_problems('engine/crates/manifest/data/gameplay/corrections-v1/bf1-pc-programs.json', c.encoded(table))

    def test_moving_one_vertex_counts_as_writing_it(self):
        moves = [n for i in range(10001) for n in (1, 0, 1, 1.5, 2.5, i)]
        meshes = [{'n': 'mesh', 'b': [0] * 6, 'v': moves[at:at + 6 * 2000], 'p': []} for at in range(0, len(moves), 6 * 2000)]
        self.assertTrue(any('vertices written outright' in p for p in self.program_problems(collision=meshes)))

    def test_a_stream_that_is_not_numbers_is_refused_not_a_crash(self):
        self.assertTrue(self.program_problems(collision=[{'n': 'mesh', 'b': [0] * 6, 'v': [[1]], 'p': []}]))

    def test_a_clip_or_bone_cannot_hide_text(self):
        bank = lambda *clips: [{'name': 'a', 'size': 0, 'version': 0, 'spare': 0, 'clips': list(clips)}]
        self.assertEqual(self.program_problems(bank=bank({'from': 0}, {'from': 1, 'mask': [1, 2], 'roots': [[0, 0, 0, 0, 1, [[]] * 7]]})), [])
        self.assertTrue(self.program_problems(bank=bank({'from': 0, 'payload': 'aGVsbG8gd29ybGQ'})))
        self.assertTrue(self.program_problems(bank=bank({'from': 0, 'roots': [['text', 0, 0, 0, 1, [[]] * 7]]})))
        skeleton = lambda *bones: [{'name': 's', 'bones': list(bones)}]
        self.assertEqual(self.program_problems(skeleton=skeleton(0, [1, [0] * 12], ['bone_a', '', [0] * 12])), [])
        self.assertTrue(self.program_problems(skeleton=skeleton(['aGVsbG8gd29ybGQ' * 40, '', [0] * 12])))
        self.assertTrue(self.program_problems(skeleton=skeleton(['bone_a', ['not', 'text'], [0] * 12])))

    def test_root_tracks_and_bones_have_a_cap_too(self):
        roots = [[0, 0, 0, 0, 1, [[]] * 7]] * 101
        bank = [{'name': 'a', 'size': 0, 'version': 0, 'spare': 0, 'clips': [{'from': 0, 'roots': roots}]}]
        self.assertTrue(any('animation tracks' in p for p in self.program_problems(bank=bank)))
        skeleton = [{'name': 's', 'bones': [[0, [0] * 12]] * 601}]
        self.assertTrue(any('skeleton bones' in p for p in self.program_problems(skeleton=skeleton)))

    def test_height_fill_reach_is_the_aim_ray(self):
        data = correction()
        terrain = data['files'][0]['terrain'][0]
        terrain['height_fills'] = [[100, 0, 4, 4, 2.5]]
        self.assertEqual(export.generated_data_problems(self.path, c.encoded(data)), [])
        terrain['height_fills'] = [[5000, 0, 1, 1, 2.5]]
        self.assertTrue(any('height fill' in p for p in export.generated_data_problems(self.path, c.encoded(data))))

    def test_a_region_setting_holds_only_shadow_numbers(self):
        data = correction()
        world = {'scope': [], 'name': 'w', 'ordinal': 0, 'records': [{'region_setting': 'shadow02 d=0.5 c=35,138,17'}]}
        data['files'][0]['world'] = [world]
        self.assertEqual(export.generated_data_problems(self.path, c.encoded(data)), [])
        world['records'][0]['region_setting'] = 'a whole sentence of text that is not a setting'
        self.assertTrue(export.generated_data_problems(self.path, c.encoded(data)))

    def test_generated_tables_pass_without_scrub_changes(self):
        root = Path(__file__).resolve().parents[2]
        for path in sorted((root / 'engine/crates/manifest/data/gameplay/corrections-v1').glob('*.json')):
            with self.subTest(path=path.name):
                raw = path.read_bytes()
                relative = path.relative_to(root).as_posix()
                self.assertEqual(export.generated_data_problems(relative, raw), [])
                self.assertEqual(export.scrub(relative, raw.decode()).encode(), raw)


if __name__ == '__main__':
    unittest.main()
