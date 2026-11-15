"""Manifest attacks and install/remove through the production native pack writer."""
from pathlib import Path
import hashlib
import contextlib
import io
import json
import os
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import pack
import visual_packs as vp
from visual_pack_formats import KTX_MAGIC, compatible_mesh, compatible_texture, glb, ktx2


def texture(w=2, h=2, color=60, alpha=255):
    levels = max(w, h).bit_length()
    dfd = struct.pack('<IIHH', 92, 0, 2, 88) + bytes([1, 1, 2, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0])
    dfd += b''.join(struct.pack('<HBB4BII', i*8, 7, c, 0, 0, 0, 0, 0, 255) for i, c in enumerate((0, 1, 2, 15)))
    offset = 80 + 24*levels
    header = KTX_MAGIC + struct.pack('<9I4I2Q', 43, 1, w, h, 0, 0, 1, levels, 0, offset, len(dfd), offset+len(dfd), 0, 0, 0)
    data, index = b'', b''
    for i in range(levels):
        pixels = bytes([color, color, color, alpha])*max(1, w >> i)*max(1, h >> i)
        index += struct.pack('<3Q', offset+len(dfd)+len(data), len(pixels), len(pixels))
        data += pixels
    return header+index+dfd+data


def mesh(edit=None):
    positions = struct.pack('<9f', 0, 0, 0, 1, 0, 0, 0, 1, 0)
    normals = struct.pack('<9f', *([0, 0, 1]*3))
    uvs = struct.pack('<6f', 0, 0, 1, 0, 0, 1)
    indices = struct.pack('<3I', 0, 1, 2)
    binary = positions+normals+uvs+indices
    doc = dict(asset={'version': '2.0'}, buffers=[dict(byteLength=len(binary))],
               bufferViews=[dict(buffer=0, byteOffset=o, byteLength=n) for o, n in ((0, 36), (36, 36), (72, 24), (96, 12))],
               accessors=[dict(bufferView=i, count=3, type=t, componentType=c) for i, t, c in ((0, 'VEC3', 5126), (1, 'VEC3', 5126), (2, 'VEC2', 5126), (3, 'SCALAR', 5125))],
               materials=[dict(name='wall', alphaMode='OPAQUE', extras=dict(light='lit', flags=1))],
               meshes=[dict(name='wall', extras=dict(sphere=[0, 0, 0, 2]), primitives=[dict(attributes=dict(POSITION=0, NORMAL=1, TEXCOORD_0=2), indices=3, material=0)])],
               nodes=[dict(mesh=0, name='wall')], scenes=[dict(nodes=[0])], scene=0)
    if edit:
        edit(doc)
    text = json.dumps(doc).encode()
    text += b' '*(-len(text) % 4)
    return struct.pack('<4sII', b'glTF', 2, 28+len(text)+len(binary))+struct.pack('<I4s', len(text), b'JSON')+text+struct.pack('<I4s', len(binary), b'BIN\0')+binary


class Packs(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='visual packs ü ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.lookup, self.baked = self.root/'packs', self.root/'assets'
        self.lookup.mkdir(); self.baked.mkdir()
        self.name = 'bf1_rhn2/textures/wall.ktx2'
        self.stock = self.baked/self.name
        self.stock.parent.mkdir(parents=True)
        self.stock.write_bytes(texture())
        self.folder = self.lookup/'prepared pack'
        self.folder.mkdir()
        (self.folder/'wall.ktx2').write_bytes(texture(color=90))
        self.manifest = dict(version=1, replacements=[dict(file='wall.ktx2', targets=[dict(path=self.name, sha256=[vp.digest(self.stock)])])])
        self.write_manifest()

    def write_manifest(self):
        (self.folder/'pack.json').write_text(json.dumps(self.manifest))

    def resolve(self):
        packs, report = vp.discover(self.lookup, explicit=True)
        return vp.resolve(packs, report, self.baked), report

    def test_named_original_only_hashed_once_for_variants(self):
        self.manifest['replacements'][0]['targets'].extend([dict(path=self.name, sha256=['f'*64]), dict(path='bf2_uta1/textures/missing.ktx2', sha256=['e'*64])])
        self.write_manifest()
        with mock.patch.object(vp, 'digest', wraps=vp.digest) as digest:
            entries, report = self.resolve()
        self.assertEqual(len(entries), 1)
        self.assertEqual(sum(c.args[0] == self.stock.resolve() for c in digest.call_args_list), 1)
        self.assertEqual(report['counts'], dict(packs=1, installed=1, missing=1, hash_mismatch=1, originals_hashed=1, ignored=0))
        self.assertEqual(report['targets'][0]['stock_sha256'], vp.digest(self.stock))
        self.assertEqual(report['targets'][0]['replacement_sha256'], vp.digest(self.folder/'wall.ktx2'))

    def test_absent_default_and_empty_directory(self):
        for path in (self.root/'absent', self.root/'empty'):
            if path.name == 'empty': path.mkdir()
            packs, report = vp.discover(path)
            self.assertEqual(vp.resolve(packs, report, self.baked), [])
            self.assertEqual(report['counts']['installed'], 0)
        with self.assertRaisesRegex(vp.PackError, 'folder is missing'):
            vp.discover(self.root/'absent', explicit=True)

    def test_cli_missing_explicit_lookup_fails_before_input_scan(self):
        with mock.patch.object(sys, 'argv', ['pack.py', '--visual-packs', str(self.root/'absent')]), \
             mock.patch.object(pack.input_scan, 'scan') as scan:
            with self.assertRaisesRegex(SystemExit, 'folder is missing'):
                pack.main()
            scan.assert_not_called()

    def test_invalid_hashes_unknown_fields_and_cross_pack_conflicts(self):
        target = self.manifest['replacements'][0]['targets'][0]
        for hashes in ([], ['short'], ['g'*64], 'f'*64):
            target['sha256'] = hashes
            self.write_manifest()
            with self.assertRaises(vp.PackError): vp.discover(self.lookup)
        target['sha256'] = [vp.digest(self.stock)]
        self.manifest['unexpected'] = True
        self.write_manifest()
        with self.assertRaises(vp.PackError): vp.discover(self.lookup)
        del self.manifest['unexpected']
        self.write_manifest()
        other = self.lookup/'other pack'; other.mkdir()
        (other/'pack.json').write_text(json.dumps(self.manifest))
        (other/'wall.ktx2').write_bytes(texture())
        with self.assertRaisesRegex(vp.PackError, 'duplicate matching target'): self.resolve()

    def test_ignored_material_does_not_become_a_pack(self):
        (self.lookup/'preview.zip').write_bytes(b'zip')
        (self.lookup/'tools').mkdir()
        (self.folder/'preview.png').write_bytes(b'preview')
        _, report = self.resolve()
        self.assertEqual(report['counts']['ignored'], 3)
        self.assertEqual(report['counts']['packs'], 1)
        self.assertIn('ignored', '\n'.join(vp.summary(report)))

    def test_nonempty_bad_or_empty_manifest_fails(self):
        for raw in ('', '{}', '{', '{"version":1,"version":1,"replacements":[]}', '{"version":true,"replacements":[]}', '{"version":1,"replacements":[]}'):
            with self.subTest(raw=raw):
                (self.folder/'pack.json').write_text(raw)
                with self.assertRaises(vp.PackError): vp.discover(self.lookup)

    def test_unsafe_paths_and_gameplay_files_fail(self):
        for path in ('../wall.ktx2', '/wall.ktx2', 'C:/wall.ktx2', 'x\\wall.ktx2', 'x//wall.ktx2', 'x/./wall.ktx2', 'x/NUL.ktx2', 'x/wall.ktx2.', 'bf1_rhn2/manifest.ron', 'bf1_rhn2/collision/a.glb', 'side_bf2/soldiers/a.glb', 'bf1_rhn2/terrain_a.glb', 'menu/a.png'):
            with self.subTest(path=path):
                self.manifest['replacements'][0]['targets'][0]['path'] = path
                self.write_manifest()
                with self.assertRaises(vp.PackError): vp.discover(self.lookup)

    def test_escaping_links_and_directory_payloads_fail(self):
        outside = self.root/'external.ktx2'; outside.write_bytes(texture())
        source = self.folder/'wall.ktx2'; source.unlink(); source.symlink_to(outside)
        with self.assertRaisesRegex(vp.PackError, 'escapes'): vp.discover(self.lookup)
        source.unlink(); source.mkdir()
        with self.assertRaisesRegex(vp.PackError, 'regular file'): vp.discover(self.lookup)

    def test_duplicate_matching_targets_fail_even_if_identical(self):
        self.manifest['replacements'] *= 2
        self.write_manifest()
        with self.assertRaisesRegex(vp.PackError, 'duplicate matching target'): self.resolve()

    def test_no_matches_warn_and_invalid_unused_payload_still_fails(self):
        self.manifest['replacements'][0]['targets'][0]['sha256'] = ['f'*64]
        self.write_manifest()
        entries, report = self.resolve()
        self.assertEqual(entries, [])
        self.assertEqual(len(report['warnings']), 1)
        (self.folder/'wall.ktx2').write_bytes(b'broken')
        with self.assertRaisesRegex(vp.PackError, 'KTX2'): self.resolve()

    def test_original_link_cannot_escape_baked_root(self):
        self.stock.unlink(); self.stock.symlink_to(self.folder/'wall.ktx2')
        with self.assertRaisesRegex(vp.PackError, 'escapes'): self.resolve()

    def test_payload_edit_is_seen_without_rebake(self):
        _, before = self.resolve()
        (self.folder/'wall.ktx2').write_bytes(texture(color=123))
        _, after = self.resolve()
        self.assertNotEqual(before['targets'][0]['replacement_sha256'], after['targets'][0]['replacement_sha256'])
        self.assertEqual(self.stock.read_bytes(), texture())


class Formats(unittest.TestCase):
    def test_ktx2_rejects_a_dimension_above_the_renderer_limit(self):
        raw = bytearray(texture())
        struct.pack_into('<I', raw, 20, 8193)
        with self.assertRaisesRegex(ValueError, '8192 pixels'):
            ktx2(raw)

    def test_ktx2_accepts_native_upscale_and_rejects_truncation_ratio_alpha(self):
        compatible_texture(ktx2(texture()), ktx2(texture(4, 4)))
        for data in (texture()[:-1], b'not ktx2'):
            with self.assertRaises(ValueError): ktx2(data)
        for data in (texture(4, 2), texture(alpha=0)):
            with self.assertRaises(ValueError): compatible_texture(ktx2(texture()), ktx2(data))
        compatible_texture(ktx2(texture(alpha=0)), ktx2(texture(color=90, alpha=0)))
        with self.assertRaisesRegex(ValueError, 'alpha'):
            compatible_texture(ktx2(texture(alpha=0)), ktx2(texture(4, 4, alpha=0)))

    def test_ktx2_rejects_wrong_interpretation_and_overlapping_mips(self):
        for off, fmt, value in ((12, '<I', 37), (44, '<I', 1), (80, '<Q', 0), (80+24, '<Q', 220)):
            data = bytearray(texture()); struct.pack_into(fmt, data, off, value)
            with self.assertRaises(ValueError): ktx2(data)

    def test_glb_preserves_material_bindings_and_gameplay_extras(self):
        compatible_mesh(glb(mesh()), glb(mesh()))
        for edit in (lambda d: d['materials'][0].update(name='wrong'), lambda d: d['meshes'][0]['extras'].update(sphere=[0, 0, 0, 3]), lambda d: d['nodes'][0].update(translation=[0, 0, 1])):
            with self.assertRaises(ValueError): compatible_mesh(glb(mesh()), glb(mesh(edit)))

    def test_glb_rejects_external_buffers_skins_images_bad_views_and_indices(self):
        for edit in (lambda d: d['buffers'][0].update(uri='../outside.bin'), lambda d: d.update(skins=[{}]), lambda d: d.update(images=[{'uri': '../outside.png'}]), lambda d: d['bufferViews'][0].update(byteLength=4), lambda d: d['accessors'][0].update(count=400), lambda d: d['meshes'][0]['primitives'][0].update(indices=40), lambda d: d['materials'][0].update(alphaMode='MASK')):
            with self.subTest(edit=edit):
                with self.assertRaises((ValueError, IndexError)): glb(mesh(edit))
        with self.assertRaises(ValueError): glb(mesh()[:-1])


class Bundles(unittest.TestCase):
    setUp = Packs.setUp
    write_manifest = Packs.write_manifest
    resolve = Packs.resolve

    def move_pack(self, name='upscaled_textures/endor'):
        folder = self.lookup/name
        folder.parent.mkdir(parents=True, exist_ok=True)
        self.folder.rename(folder)
        self.folder = folder

    def add_pack(self, name, target):
        folder = self.lookup/name
        folder.mkdir(parents=True)
        stock = self.baked/target
        stock.parent.mkdir(parents=True, exist_ok=True)
        stock.write_bytes(texture())
        manifest = dict(version=1, replacements=[dict(file='wall.ktx2', targets=[dict(path=target, sha256=[vp.digest(stock)])])])
        (folder/'pack.json').write_text(json.dumps(manifest))
        (folder/'wall.ktx2').write_bytes(texture(color=120))
        return folder

    def snapshot(self):
        # Check bytes and metadata: discovery/resolution must not rewrite inputs or bake files.
        return {path.relative_to(self.root).as_posix():
                (path.read_bytes(), path.stat().st_mode, path.stat().st_mtime_ns)
                for path in self.root.rglob('*') if path.is_file() and not path.is_symlink()}

    def test_recursive_bundles_sibling_mod_hashes_and_unchanged_inputs(self):
        self.move_pack()
        shared = self.add_pack('upscaled_textures/shared/foliage', 'common/textures/foliage.ktx2')
        sibling = self.add_pack('other_mod', 'bf2_uta1/textures/sky.ktx2')
        (self.lookup/'upscaled_textures/preview.zip').write_bytes(b'zip')
        (self.lookup/'upscaled_textures/tools/empty').mkdir(parents=True)
        before = self.snapshot()
        with mock.patch.object(vp, 'digest', wraps=vp.digest) as digest:
            entries, report = self.resolve()
        names = ['other_mod', 'upscaled_textures/endor', 'upscaled_textures/shared/foliage']
        self.assertEqual([p['name'] for p in report['packs']], names)
        self.assertEqual([p['pack'] for p in report['targets']], names)
        self.assertEqual(report['counts'], dict(packs=3, installed=3, missing=0, hash_mismatch=0, originals_hashed=3, ignored=2))
        for folder in (self.folder, shared, sibling):
            target = next(t for t in report['targets'] if t['pack'] == folder.relative_to(self.lookup).as_posix())
            self.assertEqual(target['replacement_sha256'], vp.digest(folder/'wall.ktx2'))
            self.assertEqual(target['stock_sha256'], vp.digest(self.baked/target['path']))
            manifest = next(p for p in report['packs'] if p['name'] == target['pack'])
            self.assertEqual(manifest['manifest_sha256'], vp.digest(folder/'pack.json'))
            self.assertEqual(sum(c.args[0] == (self.baked/target['path']).resolve() for c in digest.call_args_list), 1)
        self.assertEqual({r['path'] for r in report['ignored']}, {'upscaled_textures/preview.zip', 'upscaled_textures/tools'})
        self.assertEqual((entries, report), self.resolve())
        self.assertEqual(self.snapshot(), before)

    def test_selected_pack_folder_is_itself_a_pack(self):
        self.move_pack()
        packs, report = vp.discover(self.folder, explicit=True)
        entries = vp.resolve(packs, report, self.baked)
        self.assertEqual([p['name'] for p in report['packs']], ['endor'])
        self.assertEqual(report['targets'][0]['pack'], 'endor')
        self.assertEqual(entries, [['assets_baked/'+self.name, str((self.folder/'wall.ktx2').resolve())]])

    def test_manifest_root_owns_payload_and_preview_descendants(self):
        self.move_pack()
        payload = self.folder/'payload'
        payload.mkdir()
        (self.folder/'wall.ktx2').rename(payload/'wall.ktx2')
        self.manifest['replacements'][0]['file'] = 'payload/wall.ktx2'
        self.write_manifest()
        (payload/'pack.json').write_text('not an implicit pack')
        preview = self.folder/'previews/nested'
        preview.mkdir(parents=True)
        (preview/'pack.json').write_text('{')
        (preview/'wall.ktx2').write_bytes(b'preview')
        entries, report = self.resolve()
        self.assertEqual(len(entries), 1)
        self.assertEqual(report['counts']['packs'], 1)
        self.assertEqual({r['path'] for r in report['ignored']}, {
            'upscaled_textures/endor/payload/pack.json',
            'upscaled_textures/endor/previews/nested/pack.json',
            'upscaled_textures/endor/previews/nested/wall.ktx2'})

    def test_nested_broken_manifest_names_exact_bundle_path_before_scan(self):
        self.move_pack('upscaled_textures/maps/endor')
        for raw in ('{', '{}', '{"version":true,"replacements":[]}',
                    '{"version":1,"replacements":[{"file":"wall.ktx2","targets":[]}]}'):
            with self.subTest(raw=raw):
                (self.folder/'pack.json').write_text(raw)
                with mock.patch.object(sys, 'argv', ['pack.py', '--visual-packs', str(self.lookup)]), \
                     mock.patch.object(pack.input_scan, 'scan') as scan:
                    with self.assertRaisesRegex(SystemExit, 'upscaled_textures/maps/endor/pack.json:.*fix or remove'):
                        pack.main()
                    scan.assert_not_called()

    def test_nested_matching_conflict_names_both_packs_and_hashes_once(self):
        self.move_pack()
        self.add_pack('another_mod/shared', self.name)
        before = self.snapshot()
        with mock.patch.object(vp, 'digest', wraps=vp.digest) as digest:
            with self.assertRaisesRegex(vp.PackError, 'duplicate matching target .*another_mod/shared and upscaled_textures/endor'):
                self.resolve()
        self.assertEqual(sum(c.args[0] == self.stock.resolve() for c in digest.call_args_list), 1)
        self.assertEqual(self.snapshot(), before)
        # A different accepted stock variant is skipped, rather than treated as a conflict.
        self.manifest['replacements'][0]['targets'][0]['sha256'] = ['f'*64]
        self.write_manifest()
        entries, report = self.resolve()
        self.assertEqual(len(entries), 1)
        self.assertEqual(report['counts']['hash_mismatch'], 1)
        self.assertEqual(report['counts']['originals_hashed'], 1)
        self.assertIn('upscaled_textures/endor', report['warnings'][0])

    def test_nested_paths_cannot_escape_the_pack_into_bundle_or_sibling(self):
        self.move_pack()
        outside = self.lookup/'upscaled_textures/outside.ktx2'
        outside.write_bytes(texture())
        for file in ('../outside.ktx2', '/outside.ktx2', 'C:/outside.ktx2', 'payload\\wall.ktx2'):
            with self.subTest(file=file):
                self.manifest['replacements'][0]['file'] = file
                self.write_manifest()
                with self.assertRaisesRegex(vp.PackError, 'upscaled_textures/endor/pack.json:.*unsafe relative path'):
                    vp.discover(self.lookup)
        self.manifest['replacements'][0]['file'] = 'wall.ktx2'
        self.write_manifest()
        (self.folder/'wall.ktx2').unlink()
        (self.folder/'wall.ktx2').symlink_to(outside)
        with self.assertRaisesRegex(vp.PackError, 'upscaled_textures/endor/pack.json:.*escapes'):
            vp.discover(self.lookup)
        (self.folder/'wall.ktx2').unlink()
        (self.folder/'wall.ktx2').write_bytes(texture())
        (self.folder/'pack.json').unlink()
        external_manifest = self.root/'outside.json'
        external_manifest.write_text(json.dumps(self.manifest))
        (self.folder/'pack.json').symlink_to(external_manifest)
        with self.assertRaisesRegex(vp.PackError, 'upscaled_textures/endor/pack.json:.*escapes'):
            vp.discover(self.lookup)

    def test_symlink_directories_are_not_followed_outside_or_in_cycles(self):
        self.move_pack()
        external = self.root/'external'
        external.mkdir()
        (external/'pack.json').write_text('{')
        bundle = self.lookup/'upscaled_textures'
        (bundle/'outside').symlink_to(external, target_is_directory=True)
        (bundle/'cycle').symlink_to(self.lookup, target_is_directory=True)
        (bundle/'alias').symlink_to(self.folder, target_is_directory=True)
        (self.folder/'preview-link').symlink_to(external, target_is_directory=True)
        (self.folder/'self-link').symlink_to(self.folder, target_is_directory=True)
        entries, report = self.resolve()
        self.assertEqual(len(entries), 1)
        self.assertEqual(report['counts']['packs'], 1)
        self.assertEqual({r['path'] for r in report['ignored']}, {
            'upscaled_textures/outside', 'upscaled_textures/cycle', 'upscaled_textures/alias',
            'upscaled_textures/endor/preview-link', 'upscaled_textures/endor/self-link'})


@unittest.skipUnless(os.environ.get('OPENBF_PACK_TOOL'), 'build pak example and set OPENBF_PACK_TOOL for native integration')
class NativeInstall(unittest.TestCase):
    setUp = Packs.setUp
    resolve = Packs.resolve
    write_manifest = Packs.write_manifest
    def test_native_install_repeat_remove_and_failure_preserve_stock(self):
        tool = Path(os.environ['OPENBF_PACK_TOOL'])
        game = self.root/'openbf'
        mapping = self.root/'mapping.json'
        mission = self.root/'missions'; mission.mkdir(); (mission/'one.ron').write_text('unchanged mission')
        collision = self.baked/'bf1_rhn2/collision/wall.glb'; collision.parent.mkdir(); collision.write_bytes(b'unchanged collision')
        def write(entries):
            mapping.write_text(json.dumps(entries))
            return subprocess.run([tool, 'write', game, '--replacements', mapping, f'assets_baked={self.baked}', f'missions={mission}'], capture_output=True)
        def read(name):
            return subprocess.check_output([game, 'read', name], cwd=self.root)
        self.assertEqual(write([]).returncode, 0)
        self.assertEqual(read('assets_baked/'+self.name), texture())
        entries, _ = self.resolve()
        self.assertEqual(write(entries).returncode, 0)
        self.assertEqual(read('assets_baked/'+self.name), texture(color=90))
        first = hashlib.sha256(game.read_bytes()).hexdigest()
        self.assertEqual(write(entries).returncode, 0)
        self.assertEqual(first, hashlib.sha256(game.read_bytes()).hexdigest())
        self.assertEqual(read('missions/one.ron'), b'unchanged mission')
        self.assertEqual(read('assets_baked/bf1_rhn2/collision/wall.glb'), b'unchanged collision')
        self.assertEqual(self.stock.read_bytes(), texture())
        # The complete invalid mapping must fail before opening the output file.
        bad = entries+[['assets_baked/nonexistent.ktx2', entries[0][1]]]
        self.assertNotEqual(write(bad).returncode, 0)
        self.assertEqual(first, hashlib.sha256(game.read_bytes()).hexdigest())
        self.assertNotEqual(write(entries*2).returncode, 0)
        (self.folder/'pack.json').unlink()  # Removing a prepared pack restores the cached stock.
        entries, report = self.resolve()
        self.assertEqual(entries, [])
        self.assertEqual(write(entries).returncode, 0)
        self.assertEqual(read('assets_baked/'+self.name), texture())


@unittest.skipUnless(os.environ.get('OPENBF_GAME_BIN'), 'set OPENBF_GAME_BIN to check pack.py with the native game writer')
class PackScriptInstall(unittest.TestCase):
    setUp = Packs.setUp
    write_manifest = Packs.write_manifest
    move_pack = Bundles.move_pack
    add_pack = Bundles.add_pack

    def test_cached_bake_install_error_remove_and_sources(self):
        self.cached_bake_case()

    def test_cached_bake_nested_bundle_install_error_remove_and_sources(self):
        self.cached_bake_case(bundle=True)

    def cached_bake_case(self, bundle=False):
        # Exercise pack.py's complete orchestration with extraction/baking mocked at the cache boundary.
        # The pack writer, payload resolver, install, cache files and .sources.json stay real.
        self.lookup.rename(self.root/'visual_packs')
        self.lookup = self.root/'visual_packs'
        self.folder = self.lookup/'prepared pack'
        if bundle:
            self.move_pack()
            self.add_pack('upscaled_textures/shared/foliage', 'common/textures/foliage.ktx2')
            sibling = self.add_pack('other_mod', 'bf2_uta1/textures/sky.ktx2')
        (self.baked/'classes').mkdir()
        (self.baked/'classes/bf1_all.ron').write_text('unchanged class data')
        missions = self.root/'missions'; missions.mkdir(); (missions/'one.ron').write_text('unchanged mission')
        lua = self.root/'lua'
        for game, groups in pack.LUA.items():
            for group in groups:
                path = lua/game/group; path.mkdir(parents=True); (path/'.complete').touch()
        plan = dict(games={g: dict(gameplay=dict(kind='xbox-disc', path='local input', tree='cached tree'), visual=[]) for g in ('bf1', 'bf2')},
                    refusals=[], dlc=dict(used=False), found=[])
        out = self.root/'dist/openbf'
        def snapshot():
            return {str(p): (p.read_bytes(), p.stat().st_mode, p.stat().st_mtime_ns)
                    for root in (self.lookup, self.baked, missions) for p in root.rglob('*') if p.is_file()}
        def build(selected=None):
            before = snapshot()
            with contextlib.ExitStack() as stack:
                args = ['pack.py', '--game-bin', os.environ['OPENBF_GAME_BIN'], '--out', str(out)]
                if selected is not None:
                    args += ['--visual-packs', str(selected)]
                stack.enter_context(mock.patch.object(sys, 'argv', args))
                stack.enter_context(mock.patch.dict(os.environ, {}, clear=True))
                stack.enter_context(mock.patch.object(pack, 'GAMEPLAY_TREES', {}))
                for name, value in dict(ROOT=str(self.root), BAKED=str(self.baked), MISSIONS=str(missions)).items():
                    stack.enter_context(mock.patch.object(pack, name, value))
                for name, value in dict(scan=plan, summary_lines=[], write_summary=None, discs_to_extract=[], bake_env={}).items():
                    stack.enter_context(mock.patch.object(pack.input_scan, name, return_value=value))
                for name, value in dict(resolve_pc_roots={}, console_lua_root=str(lua), git_tracked_missions=True,
                                        gameplay_correction_args=[], stale=None, missing_music=[], missing_shadows=[],
                                        check_foliage=[], find_bash='bash').items():
                    stack.enter_context(mock.patch.object(pack, name, return_value=value))
                runner = stack.enter_context(mock.patch.object(pack, 'run', wraps=pack.run))
                stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                pack.main()
                commands = [c.args[0] for c in runner.call_args_list]
                self.assertTrue(all(c[0] == 'strip' or '--write-pack' in c for c in commands), commands)
            self.assertEqual(snapshot(), before)
        build()
        source = Path(str(out)+'.sources.json')
        report = json.loads(source.read_text())['visual_packs']
        self.assertEqual(report['counts']['installed'], 3 if bundle else 1)
        self.assertIn('replacement_sha256', report['targets'][0])
        if bundle:
            self.assertEqual([p['name'] for p in report['packs']],
                             ['other_mod', 'upscaled_textures/endor', 'upscaled_textures/shared/foliage'])
            build()
            self.assertEqual(json.loads(source.read_text())['visual_packs'], report)
            build(selected=self.folder)
            direct = json.loads(source.read_text())['visual_packs']
            self.assertEqual(direct['counts']['installed'], 1)
            self.assertEqual(direct['targets'][0]['pack'], 'endor')
            build()
        before = out.read_bytes(), source.read_bytes()
        self.manifest['replacements'] *= 2; self.write_manifest()
        with self.assertRaisesRegex(SystemExit, 'duplicate matching target'): build()
        self.assertEqual((out.read_bytes(), source.read_bytes()), before)
        if bundle:
            # Removing the named mod removes all its components while the sibling remains.
            (self.lookup/'upscaled_textures').rename(self.root/'removed_mod')
            build()
            remaining = json.loads(source.read_text())['visual_packs']
            self.assertEqual(remaining['counts']['installed'], 1)
            self.assertEqual(remaining['targets'][0]['pack'], 'other_mod')
            sibling.rename(self.root/'removed_sibling')
        else:
            (self.folder/'pack.json').unlink()
        build()
        self.assertEqual(json.loads(source.read_text())['visual_packs']['counts']['installed'], 0)
        self.assertEqual(self.stock.read_bytes(), texture())
        self.assertEqual((self.baked/'classes/bf1_all.ron').read_text(), 'unchanged class data')


if __name__ == '__main__':
    unittest.main()
