"""The input scanner over synthetic fixtures: real disc and LVL structure, no game data."""
import contextlib
import io
import itertools
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import input_scan as scan
import pack


def chunk(tag, body):
    return tag + struct.pack('<I', len(body)) + body + bytes(-len(body) % 4)


def ucfb(*children):
    return chunk(b'ucfb', b''.join(children))


def lvl(platform):
    """One model and one texture, in the structure each platform really has."""
    texture = {'ps2': (b'INFO', b'pal_', b'BODY'), 'xbox': (b'INFO', b'BODY'), 'pc': (b'INFO', b'FMT_')}[platform]
    segment = {'ps2': b'_PS2', 'xbox': b'VBUF', 'pc': b'VBUF'}[platform]
    return ucfb(chunk(b'tex_', b''.join(chunk(tag, bytes(4)) for tag in texture)),
                chunk(b'modl', chunk(b'segm', chunk(segment, bytes(4)))))


def write(path, data=b''):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return path


def large(path):
    """A file big enough to count as somebody's game data, taking no space."""
    with write(path).open('wb') as out:
        out.truncate(scan.LARGE)


def tree(root, game, platform):
    """A whole game's LVLs, each under the name its own disc spells it with."""
    write(root / 'CORE.LVL', ucfb())
    write(root / scan.TREE_FILE[game], lvl(platform))
    if game == 'bf2':
        write(root / 'SHELL.LVL', ucfb())
    return root


def iso_record(name, lba, size, directory=False):
    name = name.encode('ascii')
    data = bytearray(33 + len(name) + (len(name) % 2 == 0))
    data[0] = len(data)
    struct.pack_into('<I', data, 2, lba)
    struct.pack_into('<I', data, 10, size)
    data[25], data[32] = 2 if directory else 0, len(name)
    data[33:33 + len(name)] = name
    return data


def iso_image(path, game, system=b'PLAYSTATION'):
    """A real ISO9660 directory: system ID, ELF and data tree of the named game."""
    elf = {'bf1': 'SWBFRONT.ELF', 'bf2': 'SWBFRNT2.ELF'}[game]
    data = bytearray(26 * 2048)
    data[16 * 2048:16 * 2048 + 7] = b'\1CD001\1'
    data[16 * 2048 + 8:16 * 2048 + 40] = system.ljust(32)
    struct.pack_into('<I', data, 16 * 2048 + 158, 20)
    struct.pack_into('<I', data, 16 * 2048 + 166, 2048)

    def put(lba, *records):
        blob = b''.join(records)
        data[lba * 2048:lba * 2048 + len(blob)] = blob

    put(20, iso_record('DATA', 21, 2048, True), iso_record('SYSTEM.CNF;1', 25, 1), iso_record(elf + ';1', 25, 1))
    put(21, iso_record('_LVL_PS2', 22, 2048, True))
    if game == 'bf2':
        put(22, *(iso_record(name + ';1', 25, 1) for name in ('CORE.LVL', 'INGAME.LVL', 'SHELL.LVL')))
    else:
        put(22, iso_record('COMMON', 23, 2048, True), iso_record('CORE.LVL;1', 25, 1))
        put(23, iso_record('ENG', 24, 2048, True))
        put(24, iso_record('COMMON.LVL;1', 25, 1))
    write(path, bytes(data))
    return path


def xdvdfs_table(entries):
    """Entries as a right-leaning tree: (name, sector, size, is directory)."""
    table = bytearray()
    for index, (name, sector, size, directory) in enumerate(entries):
        raw = name.encode('ascii')
        record = struct.pack('<HHIIBB', 0, 0, sector, size, 0x10 if directory else 0x20, len(raw)) + raw
        record += b'\xff' * (-len(record) % 4)
        if index + 1 < len(entries):
            struct.pack_into('<H', (record := bytearray(record)), 2, (len(table) + len(record)) // 4)
        table += record
    return bytes(table)


def xbox_image(path, game, partition=0):
    """A real XDVDFS volume, trimmed or at a full dump's partition offset (a sparse file). As on a real
    .xiso, an ISO9660 volume descriptor with an empty system ID sits ahead of it."""
    root = xdvdfs_table([('Data', 301, 2048, True), ('default.xbe', 400, 4, False)])
    data = xdvdfs_table([('_LVL_XBOX', 302, 2048, True)])
    if game == 'bf2':
        level = xdvdfs_table([('core.lvl', 400, 4, False), ('ingame.lvl', 400, 4, False)])
        parts = [(302, level)]
    else:
        level = xdvdfs_table([('core.lvl', 400, 4, False), ('Common', 303, 2048, True)])
        parts = [(302, level), (303, xdvdfs_table([('ENG', 304, 2048, True)])),
                 (304, xdvdfs_table([('common.lvl', 400, 4, False)]))]
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('wb') as out:
        def put(offset, blob):
            out.seek(partition + offset)
            out.write(blob)
        put(16 * 2048, b'\1CD001\1' + bytes(2041))
        put(32 * 2048, b'MICROSOFT*XBOX*MEDIA' + struct.pack('<II', 300, len(root)))
        put(300 * 2048, root)
        put(301 * 2048, data)
        for sector, blob in parts:
            put(sector * 2048, blob)
        put(400 * 2048, b'data')
        out.truncate(partition + 401 * 2048)
    return path


def windows_program(path, machine=0x14c):
    """A PE header: enough to tell the 32-bit original releases from a 64-bit one."""
    data = bytearray(512)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 0x80)
    data[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<H', data, 0x84, machine)
    return write(path, bytes(data))


def pc_install(root, game):
    tree(root / 'GameData/Data/_LVL_PC', game, 'pc')
    windows_program(root / 'GameData/game.exe')
    large(root / 'Movies/huge.bik')
    return root


def entity(name):
    return chunk(b'entc', chunk(b'BASE', b'soldier\0') + chunk(b'TYPE', name.encode() + b'\0'))


def addon_script():
    return ucfb(chunk(b'scr_', chunk(b'NAME', b'addme\0') + chunk(b'INFO', b'\0')))


def dlc_folder(root, heroes=('dlc_hero_fisto', 'dlc_hero_ventress'), geometry=b'VBUF'):
    """The layout that really ships: an addon script over a PC-format _LVL_PC with its own core.lvl."""
    write(root / 'addme.script', addon_script())
    data = root / 'data/_LVL_PC'
    write(data / 'core.lvl', ucfb())
    # Named so that nothing about the name helps.
    model = chunk(b'modl', chunk(b'segm', chunk(geometry, bytes(4))))
    level = chunk(b'lvl_', bytes(8) + b''.join(entity(hero) for hero in heroes) + model)
    write(data / 'SIDE/x.lvl', ucfb(chunk(b'tex_', chunk(b'INFO', bytes(4))), level))
    return root


def plan_of(root, **options):
    return scan.scan([root], **options)


def row_for(plan, path):
    return next(row for row in plan['found'] if row['path'] == str(path))


class Recognition(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name) / 'iso'
        self.root.mkdir()

    def kind(self, path):
        row = scan.classify_file(path)
        return row and (row['kind'], row['game'], row['status'])

    def test_ps2_discs_are_known_by_their_structure_not_their_name(self):
        self.assertEqual(self.kind(iso_image(self.root / 'renamed.dat', 'bf1')), ('ps2-disc', 'bf1', 'found'))
        self.assertEqual(self.kind(iso_image(self.root / 'SWBF1.iso', 'bf2')), ('ps2-disc', 'bf2', 'found'))

    def test_xbox_discs_trimmed_and_every_full_dump_partition(self):
        for game, partition in itertools.product(scan.GAMES, scan.XBOX_PARTITIONS):
            with self.subTest(game=game, partition=hex(partition)):
                path = xbox_image(self.root / f'{game}{partition:x}.bin', game, partition)
                self.assertEqual(self.kind(path), ('xbox-disc', game, 'found'))
                if partition:
                    self.assertIn('full dump', scan.classify_file(path)['reason'])

    def test_the_partition_offsets_are_the_real_ones(self):
        # The test above takes its offsets from the scanner, so it cannot notice one going missing.
        self.assertEqual(scan.XBOX_PARTITIONS, (0, 0x18300000, 0xFD90000, 0x2080000))

    def test_an_xbox_node_with_only_its_left_child_absent_as_0xffff_is_an_entry(self):
        """Only both words at 0xFFFF are padding, as mungefs::xdvdfs reads it: the two readers must list the same."""
        table = bytearray(xdvdfs_table([('default.xbe', 400, 4, False), ('ingame.lvl', 400, 4, False)]))
        struct.pack_into('<H', table, 0, 0xFFFF)
        path = self.root / 'odd.iso'
        with path.open('wb') as out:
            out.seek(32 * 2048)
            out.write(b'MICROSOFT*XBOX*MEDIA' + struct.pack('<II', 300, len(table)))
            out.seek(300 * 2048)
            out.write(table)
            out.truncate(401 * 2048)
        self.assertEqual(scan.xdvdfs_files(str(path)), (0, {'default.xbe', 'ingame.lvl'}))

    def test_hostile_xbox_directories_end(self):
        good = xbox_image(self.root / 'good.iso', 'bf2').read_bytes()
        for label, offset, word in (('huge root', 32 * 2048 + 24, 0xffffffff), ('outside', 32 * 2048 + 20, 0xfffffff0)):
            with self.subTest(label):
                data = bytearray(good)
                struct.pack_into('<I', data, offset, word)
                path = write(self.root / 'bad.iso', bytes(data))
                self.assertEqual(scan.classify_file(path)['status'], 'refused')
        # A tree entry pointing at itself, and a directory holding itself, both end.
        data = bytearray(good)
        second = struct.unpack_from('<H', data, 300 * 2048 + 2)[0]
        struct.pack_into('<H', data, 300 * 2048 + second * 4 + 2, second)
        struct.pack_into('<I', data, 300 * 2048 + 4, 300)
        struct.pack_into('<I', data, 300 * 2048 + 8, 2048)
        self.assertEqual(scan.classify_file(write(self.root / 'loop.iso', bytes(data)))['status'], 'refused')
        # Cut anywhere, an image is still only refused or left alone.
        for size in (32 * 2048 + 4, 300 * 2048 + 2):
            row = scan.classify_file(write(self.root / 'short.iso', good[:size]))
            self.assertTrue(row is None or row['status'] == 'refused', row)

    def test_other_discs_and_wrappers_are_refused_by_what_they_are(self):
        cases = {
            'rebuilt': (iso_image(self.root / 'SWBF2.iso', 'bf2', system=b''), 'rebuilt or modified'),
            'data disc': (self.other_iso(), 'unknown format'),
            'other ps2 game': (self.other_ps2(), 'not Battlefront'),
            'cso': (write(self.root / 'SWBF2.cso', b'CISO' + bytes(64)), 'CSO'),
            'chd': (write(self.root / 'a.chd', b'MComprHD' + bytes(64)), 'CHD'),
            'zip': (write(self.root / 'a.iso', b'PK\x03\x04' + bytes(64)), 'zip'),
            'raw': (write(self.root / 'a.bin', bytes(16 * 2352 + 24) + b'\1CD001\1' + bytes(64)), 'BIN'),
        }
        for label, (path, words) in cases.items():
            with self.subTest(label):
                row = scan.classify_file(path)
                self.assertEqual(row['status'], 'refused')
                self.assertIn(words, row['reason'])

    def other_iso(self):
        data = bytearray(iso_image(self.root / 'data.iso', 'bf2', system=b'LINUX').read_bytes())
        data[16 * 2048 + 158:16 * 2048 + 162] = struct.pack('<I', 24)  # an empty root directory
        return write(self.root / 'data.iso', bytes(data))

    def other_ps2(self):
        data = bytearray(iso_image(self.root / 'ps2.iso', 'bf2').read_bytes())
        data[16 * 2048 + 158:16 * 2048 + 162] = struct.pack('<I', 24)
        return write(self.root / 'ps2.iso', bytes(data))

    def test_an_xbox_tree_has_pc_geometry_and_still_is_not_a_pc_install(self):
        self.assertEqual(scan.content.pc_game(tree(self.root / 'x', 'bf2', 'xbox')), None)
        self.assertEqual(scan.content.pc_game(tree(self.root / 'y', 'bf1', 'xbox')), None)
        self.assertEqual(scan.content.pc_game(tree(self.root / 'z', 'bf2', 'pc')), 'bf2')
        plan = plan_of(self.root)
        self.assertEqual(row_for(plan, self.root / 'x')['status'], 'refused')
        self.assertIn('extracted Xbox', row_for(plan, self.root / 'x')['reason'])
        self.assertEqual(row_for(plan, self.root / 'z')['kind'], 'pc-install')

    def test_an_extracted_ps2_tree_in_iso_is_refused_not_mistaken(self):
        plan = plan_of(tree(self.root / 'a', 'bf2', 'ps2').parent)
        self.assertIn('extracted PS2', row_for(plan, self.root / 'a')['reason'])

    def test_dlc_is_its_hero_classes_and_other_addons_are_not_dlc(self):
        dlc = dlc_folder(self.root / 'whatever' / 'one')
        one_hero = dlc_folder(self.root / 'whatever' / 'two', heroes=('dlc_hero_fisto',))
        update = self.root / 'whatever' / 'update'
        write(update / 'addme.script', addon_script())
        write(update / 'data/_LVL_PC/core.lvl', ucfb())
        write(update / 'data/_LVL_PC/KAS/kas2.lvl', lvl('pc'))
        plan = plan_of(self.root)
        self.assertEqual(row_for(plan, dlc)['kind'], 'dlc')
        self.assertEqual(row_for(plan, one_hero)['kind'], 'addon')
        self.assertEqual(row_for(plan, update)['kind'], 'addon')
        self.assertEqual(plan['dlc']['path'], str(dlc))
        self.assertTrue(plan['dlc']['used'])
        self.assertEqual(row_for(plan, dlc)['status'], 'would be used')  # no discs in this folder
        self.assertEqual(scan.content.pc_game(dlc / 'data/_LVL_PC'), None)

    def test_a_dlc_folder_with_no_geometry_of_a_known_format_is_not_dlc(self):
        addon = dlc_folder(self.root / 'a', geometry=b'XXXX')
        self.assertEqual(row_for(plan_of(self.root), addon)['kind'], 'addon')

    def test_ps2_dlc_is_the_heroes_and_a_pc_copy_lends_its_textures(self):
        pc = dlc_folder(self.root / 'a-pc')
        ps2 = dlc_folder(self.root / 'b-ps2', geometry=b'_PS2')
        plan = plan_of(self.root)
        self.assertEqual((plan['dlc']['path'], plan['dlc']['donor']), (str(ps2), str(pc)))
        self.assertEqual({row_for(plan, path)['status'] for path in (pc, ps2)}, {'would be used'})  # no discs here
        env = scan.bake_env(plan)
        self.assertEqual((env['OPENBF_DLC_DIR'], env['OPENBF_DLC_DONOR']), (str(ps2), str(pc)))

    def test_a_second_pc_dlc_is_only_a_spare(self):
        first, second = dlc_folder(self.root / 'a'), dlc_folder(self.root / 'b')
        plan = plan_of(self.root)
        self.assertEqual((plan['dlc']['path'], plan['dlc']['donor']), (str(first), None))
        self.assertEqual(row_for(plan, second)['status'], 'unused')
        self.assertNotIn('OPENBF_DLC_DONOR', scan.bake_env(plan))

    def test_the_switch_off_reads_no_dlc(self):
        plan = plan_of(dlc_folder(self.root / 'a').parent, dlc_supported=False)
        self.assertFalse(plan['dlc']['used'])
        self.assertEqual(row_for(plan, self.root / 'a')['status'], 'not supported yet')
        self.assertNotIn('OPENBF_DLC_DIR', scan.bake_env(plan))

    def test_dlc_without_its_script_is_still_dlc(self):
        data = dlc_folder(self.root / 'a') / 'data/_LVL_PC'
        (self.root / 'a/addme.script').unlink()
        self.assertEqual(row_for(plan_of(self.root), data)['kind'], 'dlc')

    def test_a_folder_holding_only_the_heroes_side_file_is_still_dlc(self):
        data = dlc_folder(self.root / 'a') / 'data/_LVL_PC'
        (self.root / 'a/addme.script').unlink()
        (data / 'core.lvl').unlink()
        plan = plan_of(self.root)
        self.assertEqual(row_for(plan, self.root / 'a')['kind'], 'dlc')
        self.assertTrue(plan['dlc']['used'])

    def test_aspyr_style_64_bit_folder_is_refused_even_with_pc_shaped_data(self):
        aspyr = pc_install(self.root / 'Classic', 'bf2')
        windows_program(aspyr / 'launcher.exe', machine=0x8664)
        plan = plan_of(self.root)
        row = row_for(plan, aspyr / 'GameData/Data/_LVL_PC')
        self.assertEqual(row['status'], 'refused')
        self.assertIn("Aspyr's Classic Collection", row['reason'])
        self.assertEqual(plan['games']['bf2']['visual'], [])

    def test_a_32_bit_install_with_its_extras_is_one_clean_input(self):
        install = pc_install(self.root / 'Install', 'bf2')
        plan = plan_of(self.root)
        self.assertEqual([row['path'] for row in plan['found']], [str(install / 'GameData/Data/_LVL_PC')])
        self.assertGreaterEqual(plan['strays'], 1)

    def test_a_folder_of_unknown_data_is_refused_by_name(self):
        write(self.root / 'Collection/content.pak', b'not a known format')
        write(self.root / 'Collection/core.lvl', ucfb())
        plan = plan_of(self.root)
        self.assertEqual(row_for(plan, self.root / 'Collection')['status'], 'refused')
        large(self.root / 'Other/content.pak')
        plan = plan_of(self.root)
        self.assertIn('unknown format', row_for(plan, self.root / 'Other/content.pak')['reason'])

    def test_a_folder_with_nothing_recognized_is_named(self):
        write(self.root / 'Strange/thing.dat', b'x')
        self.assertIn('nothing this build recognizes', row_for(plan_of(self.root), self.root / 'Strange')['reason'])

    def test_small_loose_files_are_named_and_harmless(self):
        note = write(self.root / 'checksums.sha256', b'abc')
        plan = plan_of(self.root)
        self.assertEqual(row_for(plan, note)['status'], 'ignored')

    @unittest.skipIf(os.name == 'nt' or (hasattr(os, 'geteuid') and os.geteuid() == 0), 'needs enforced permissions')
    def test_an_unreadable_folder_is_named_not_fatal(self):
        locked = self.root / 'locked'
        locked.mkdir()
        locked.chmod(0)
        self.addCleanup(locked.chmod, 0o755)
        self.assertEqual(row_for(plan_of(self.root), locked)['status'], 'refused')

    @unittest.skipIf(sys.platform in ('win32', 'darwin'), 'file names there are always text')
    def test_a_file_name_that_is_not_text_still_makes_a_summary(self):
        name = os.fsdecode(b'bad-\xff.bin')
        large(self.root / name)
        plan = plan_of(self.root)
        lines = '\n'.join(scan.summary_lines(plan))
        lines.encode('utf-8')
        self.assertIn('bad-', lines)
        out = self.root.parent / 'dist/game'
        scan.write_summary(plan, out)
        self.assertEqual(json.loads(Path(str(out) + '.inputs.json').read_text())['schema'], 1)

    def test_links_are_followed_and_a_loop_ends(self):
        disc = iso_image(self.root.parent / 'store/disc.bin', 'bf2')
        (self.root / 'link.iso').symlink_to(disc)
        (self.root / 'loop').mkdir()
        (self.root / 'loop/back').symlink_to(self.root, target_is_directory=True)
        plan = plan_of(self.root)
        self.assertEqual(plan['games']['bf2']['gameplay']['path'], str(self.root / 'link.iso'))


class Matrix(unittest.TestCase):
    """Every combination, for both states of the Xbox switch. The oracle here is the written policy, not the code."""
    BF1 = ('none', 'ps2', 'xbox', 'pc')
    BF2 = ('none', 'ps2', 'xbox', 'pc', 'pc+ps2', 'pc+xbox', 'ps2+xbox')

    def build(self, root, bf1, bf2, dlc, unknown):
        for game, want in (('bf1', bf1), ('bf2', bf2)):
            for source in want.split('+'):
                if source == 'ps2':
                    iso_image(root / f'{game}-a', game)
                elif source == 'xbox':
                    xbox_image(root / f'{game}-b', game)
                elif source == 'pc':
                    pc_install(root / f'{game}-c', game)
        if dlc:
            dlc_folder(root / 'dlc')
        if unknown:
            large(root / 'mystery.bin')

    def test_every_combination(self):
        count = 0
        for bf1, bf2, dlc, unknown, xbox_on in itertools.product(self.BF1, self.BF2, (False, True), (False, True), (False, True)):
            with self.subTest(bf1=bf1, bf2=bf2, dlc=dlc, unknown=unknown, xbox=xbox_on), tempfile.TemporaryDirectory() as temp:
                count += 1
                root = Path(temp) / 'iso'
                root.mkdir()
                self.build(root, bf1, bf2, dlc, unknown)
                plan = scan.scan([root], extracted=str(Path(temp) / 'extracted'), xbox_supported=xbox_on, pc_primary=False)
                sources1, sources2 = set(bf1.split('+')), set(bf2.split('+'))
                console1 = 'ps2' in sources1 or ('xbox' in sources1 and xbox_on)
                console2 = 'ps2' in sources2 or ('xbox' in sources2 and xbox_on)
                accepted = console1 and console2
                self.assertEqual(not plan['refusals'], accepted, plan['refusals'])
                if not accepted:
                    text = '\n'.join(plan['refusals'])
                    if not console1:
                        self.assertIn('BF1', text)
                        self.assertIn('Add a PS2 BF1 disc image', text)
                    if not console2:
                        self.assertRegex(text, 'BF2|Battlefront II')
                        self.assertRegex(text, '(?i)add a PS2 BF2 disc')
                    if unknown:
                        self.assertIn('mystery.bin', text)
                    if ('xbox' in sources1 and not console1) or ('xbox' in sources2 and not console2):
                        self.assertIn('not supported yet', text)
                    self.assertFalse([row for row in plan['found'] if row['status'] == 'used'])
                    continue
                for game, sources in (('bf1', sources1), ('bf2', sources2)):
                    chosen = plan['games'][game]
                    self.assertEqual(chosen['gameplay']['kind'], 'ps2-disc' if 'ps2' in sources else 'xbox-disc')
                    donors = {item['kind'] for item in chosen['visual']}
                    expect = set()
                    if 'pc' in sources:
                        expect.add('pc-install')
                    if xbox_on and 'xbox' in sources and 'ps2' in sources:
                        expect.add('xbox-disc')
                    self.assertEqual(donors, expect)
                    env = scan.bake_env(plan)
                    number = game[-1]
                    self.assertEqual(f'PC{number}' in env, 'pc' in sources)
                    self.assertEqual(f'XB{number}' in env, expect >= {'xbox-disc'})
                    self.assertEqual(f'OPENBF_BF{number}_TREE' in env, 'ps2' not in sources)
                    if 'xbox' in sources and not xbox_on:
                        self.assertEqual(row_for(plan, root / f'{game}-b')['status'], 'not supported yet')
                self.assertEqual(plan['dlc']['present'], dlc)
                self.assertEqual(plan['dlc']['used'], dlc)
                self.assertEqual('OPENBF_DLC_DIR' in scan.bake_env(plan), dlc)
                if dlc:
                    self.assertIn(row_for(plan, root / 'dlc')['status'], ('used', 'would be used'))
                if unknown:
                    self.assertEqual(row_for(plan, root / 'mystery.bin')['status'], 'refused')
        self.assertEqual(count, 4 * 7 * 2 * 2 * 2)

    def test_every_combination_once_pc_bf1_can_stand_alone(self):
        """What the switch would accept, so turning it on is not the first time this runs."""
        for bf1, bf2, xbox_on in itertools.product(self.BF1, self.BF2, (False, True)):
            with self.subTest(bf1=bf1, bf2=bf2, xbox=xbox_on), tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / 'iso'
                root.mkdir()
                self.build(root, bf1, bf2, False, False)
                plan = scan.scan([root], extracted=str(Path(temp) / 'extracted'), xbox_supported=xbox_on, pc_primary=True)
                sources1, sources2 = set(bf1.split('+')), set(bf2.split('+'))
                console2 = 'ps2' in sources2 or ('xbox' in sources2 and xbox_on)
                primary1 = 'ps2' in sources1 or ('xbox' in sources1 and xbox_on) or 'pc' in sources1
                self.assertEqual(not plan['refusals'], console2 and primary1, plan['refusals'])
                if plan['refusals']:
                    continue
                kind = plan['games']['bf1']['gameplay']['kind']
                self.assertEqual(kind, 'ps2-disc' if 'ps2' in sources1 else 'xbox-disc' if 'xbox' in sources1 and xbox_on else 'pc-install')
                self.assertEqual(plan['games']['bf2']['gameplay']['kind'], 'ps2-disc' if 'ps2' in sources2 else 'xbox-disc')
                env = scan.bake_env(plan)
                self.assertEqual('OPENBF_BF1_TREE' in env, kind != 'ps2-disc')
                # A PC install is the data or a donor, never both.
                self.assertEqual('PC1' in env, 'pc' in sources1 and kind != 'pc-install')


class Messages(unittest.TestCase):
    """The exact words a person acts on."""
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name) / 'iso'
        self.root.mkdir()

    def refusals(self, **options):
        # These read the refusal texts of each switch position, whichever way the switches are set today.
        options.setdefault('xbox_supported', False)
        options.setdefault('pc_primary', False)
        return scan.scan([self.root], **options)['refusals']

    def test_nothing_at_all(self):
        self.assertEqual(self.refusals(), [f'BF1: no source found. Add a PS2 BF1 disc image to {self.root}.',
                                           f'BF2: no source found. Add a PS2 BF2 disc image to {self.root}.'])

    def test_nothing_at_all_with_xbox_on(self):
        self.assertEqual(self.refusals(xbox_supported=True)[1],
                         f'BF2: no source found. Add a PS2 BF2 disc image or an Xbox BF2 disc image to {self.root}.')

    def test_one_game(self):
        iso_image(self.root / 'a', 'bf1')
        self.assertEqual(self.refusals(), [f'BF2: no source found. Add a PS2 BF2 disc image to {self.root}.'])

    def test_pc_bf2_only(self):
        iso_image(self.root / 'a', 'bf1')
        pc_install(self.root / 'pc', 'bf2')
        self.assertEqual(self.refusals(), [
            f'PC input requires a console Battlefront II disc: add a PS2 BF2 disc (Xbox BF2 input is not supported yet) to {self.root}.'])
        self.assertEqual(self.refusals(xbox_supported=True), [
            f'PC input requires a console Battlefront II disc: add a PS2 BF2 disc or an Xbox BF2 disc to {self.root}.'])

    def test_xbox_until_it_is_switched_on(self):
        disc = xbox_image(self.root / 'x', 'bf2')
        iso_image(self.root / 'a', 'bf1')
        self.assertEqual(self.refusals(), [
            f'BF2: found the Xbox disc {disc}, but Xbox input is not supported yet. Add a PS2 BF2 disc image to {self.root}.'])
        self.assertEqual(self.refusals(xbox_supported=True), [])

    def test_pc_bf1_alone_keeps_the_ps2_reference_rule(self):
        pc = pc_install(self.root / 'pc', 'bf1') / 'GameData/Data/_LVL_PC'
        iso_image(self.root / 'b', 'bf2')
        self.assertEqual(self.refusals(), [
            f'BF1: found the PC install {pc}, but PC BF1 assets currently also need the BF1 PS2 gameplay reference: the '
            f'PC mission, class and collision data cannot replace it. Add a PS2 BF1 disc image to {self.root}.'])
        self.assertEqual(self.refusals(pc_primary=True), [])

    def test_no_correction_table_names_a_private_file(self):
        folder = Path(scan.__file__).parent.parent / 'engine/crates/manifest/data/gameplay/corrections-v1'
        for path in folder.glob('*.json'):
            self.assertNotIn('private_file', path.read_text(), path.name)

    def test_sources_keep_the_maps_with_recovered_structures(self):
        xbox_image(self.root / 'x1', 'bf1')
        xbox_image(self.root / 'x2', 'bf2')
        plan = scan.scan([self.root], extracted=str(self.root / 'none'), xbox_supported=True)
        env = scan.bake_env(plan)
        self.assertEqual(plan['refusals'], [])
        self.assertNotIn('OPENBF_BF1_WITHHELD', env)
        self.assertNotIn('OPENBF_BF2_WITHHELD', env)
        self.assertEqual(scan.withheld_maps('bf2', {'kind': 'xbox-disc'}), ())
        self.assertFalse(any('maps left out' in line and 'RHN/RHN1' in line for line in scan.summary_lines(plan)))
        self.assertEqual(scan.withheld_maps('bf1', {'kind': 'pc-install'}), ())

    def test_the_ps2_disc_keeps_every_map(self):
        iso_image(self.root / 'a', 'bf1')
        iso_image(self.root / 'b', 'bf2')
        xbox_image(self.root / 'x', 'bf1')
        plan = scan.scan([self.root], extracted=str(self.root / 'none'), xbox_supported=True)
        self.assertFalse([name for name in scan.bake_env(plan) if name.endswith('_WITHHELD')])
        self.assertFalse([line for line in scan.summary_lines(plan) if 'maps left out' in line])

    def test_pc_primary_never_stands_in_for_the_console_bf2(self):
        iso_image(self.root / 'a', 'bf1')
        pc_install(self.root / 'pc', 'bf2')
        self.assertIn('requires a console Battlefront II disc', self.refusals(pc_primary=True)[0])

    def test_a_refused_format_is_named_when_a_game_is_left_without_a_source(self):
        aspyr = pc_install(self.root / 'Classic', 'bf2')
        windows_program(aspyr / 'launcher.exe', machine=0x8664)
        iso_image(self.root / 'a', 'bf1')
        text = '\n'.join(self.refusals())
        self.assertIn('BF2: no source found. Add a PS2 BF2 disc image', text)
        self.assertIn("as Aspyr's Classic Collection is", text)

    def test_a_refused_format_beside_a_complete_set_does_not_stop_the_build(self):
        large(self.root / 'mystery.bin')
        iso_image(self.root / 'a', 'bf1')
        iso_image(self.root / 'b', 'bf2')
        plan = scan.scan([self.root])
        self.assertEqual(plan['refusals'], [])
        self.assertIn(f'  refused: {self.root}/mystery.bin - an unknown format; not read', scan.summary_lines(plan))

    def test_named_inputs_are_checked_and_win(self):
        first = iso_image(self.root / 'a', 'bf2')
        named = iso_image(self.root.parent / 'mine', 'bf2')
        iso_image(self.root / 'b', 'bf1')
        plan = scan.scan([self.root], {'bf2': str(named)})
        self.assertEqual(plan['games']['bf2']['gameplay']['path'], str(named))
        self.assertEqual(row_for(plan, first)['status'], 'unused')
        wrong = scan.scan([self.root], {'bf2': str(self.root / 'b')})['refusals']
        self.assertEqual(wrong, [f"--iso2 {self.root / 'b'} is not a BF2 disc: PS2 Star Wars: Battlefront disc."])
        gone = scan.scan([self.root], {'bf1': str(self.root / 'gone')})['refusals']
        self.assertEqual(gone, [f"--iso1 {self.root / 'gone'}: no such file; add a PS2 BF1 disc at that path, or drop the option."])

    def test_a_named_64_bit_pc_folder_is_refused(self):
        iso_image(self.root / 'a', 'bf1')
        iso_image(self.root / 'b', 'bf2')
        aspyr = pc_install(self.root.parent / 'Classic', 'bf2')
        windows_program(aspyr / 'launcher.exe', machine=0x8664)
        refusals = scan.scan([self.root], pc_roots={'bf2': str(aspyr / 'GameData/Data/_LVL_PC')})['refusals']
        self.assertIn("Aspyr's Classic Collection", refusals[0])

    def test_extracted_ps2_data_counts_only_if_it_is_ps2_for_a_pc_build(self):
        extracted = self.root.parent / 'extracted'
        tree(extracted / 'bf1/lvl/DATA/_LVL_PS2', 'bf1', 'ps2')
        tree(extracted / 'bf2/lvl/DATA/_LVL_PS2', 'bf2', 'ps2')
        self.assertEqual(scan.scan([self.root], extracted=str(extracted))['refusals'], [])
        pc_install(self.root / 'pc', 'bf2')
        self.assertEqual(scan.scan([self.root], extracted=str(extracted))['refusals'], [])
        (extracted / 'bf2/lvl/DATA/_LVL_PS2/INGAME.LVL').write_bytes(lvl('pc'))
        self.assertIn('requires a console', scan.scan([self.root], extracted=str(extracted))['refusals'][0])

    def test_pc_bf1_refuses_an_extraction_that_is_not_ps2(self):
        extracted = self.root.parent / 'extracted'
        tree(extracted / 'bf1/lvl/DATA/_LVL_PS2', 'bf1', 'pc')
        iso_image(self.root / 'b', 'bf2')
        pc_install(self.root / 'pc', 'bf1')
        self.assertIn('BF1 PS2 gameplay reference', '\n'.join(scan.scan([self.root], extracted=str(extracted))['refusals']))

    def test_two_copies_choose_one_and_say_so(self):
        first, second = iso_image(self.root / 'a', 'bf1'), iso_image(self.root / 'b', 'bf1')
        iso_image(self.root / 'c', 'bf2')
        plan = scan.scan([self.root])
        self.assertEqual(row_for(plan, first)['status'], 'used')
        self.assertEqual(row_for(plan, second)['status'], 'unused')


class Pack(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.top = Path(directory.name)
        self.iso = self.top / 'iso'
        self.iso.mkdir()
        self.out = self.top / 'dist/game'

    def scan_only(self, *extra):
        args = ['pack.py', '--iso-dir', str(self.iso), '--out', str(self.out), '--scan-only', *extra]
        output = io.StringIO()
        with mock.patch.object(sys, 'argv', args), mock.patch.object(pack, 'EXTRACTED', str(self.top / 'extracted')), \
                mock.patch.object(pack, 'run') as run, contextlib.redirect_stdout(output):
            try:
                pack.main()
                code = None
            except SystemExit as stop:
                code = stop.code
            run.assert_not_called()
        return code, output.getvalue()

    def test_a_refusal_builds_nothing_and_leaves_a_summary(self):
        large(self.iso / 'mystery.bin')
        code, printed = self.scan_only()
        self.assertIn('pack: BF1: no source found', code)
        self.assertIn('mystery.bin', code)
        self.assertIn('BF1 gameplay: NONE', printed)
        written = self.out.with_name('game.inputs.txt').read_text()
        self.assertIn('REFUSED: BF1: no source found', written)
        self.assertIn('mystery.bin', written)
        self.assertEqual(json.loads(self.out.with_name('game.inputs.json').read_text())['schema'], 1)

    def test_scan_only_prints_and_writes_what_was_chosen(self):
        iso_image(self.iso / 'a', 'bf1')
        iso_image(self.iso / 'b', 'bf2')
        xbox_image(self.iso / 'c', 'bf2')
        dlc_folder(self.iso / 'd')
        pc_install(self.iso / 'e', 'bf2')
        with mock.patch.object(scan, 'XBOX_SUPPORTED', False):
            code, printed = self.scan_only()
        self.assertIsNone(code)
        for line in (f"BF1 gameplay: PS2 disc {self.iso / 'a'}", f"BF2 gameplay: PS2 disc {self.iso / 'b'}",
                     f"BF2 texture donors: PC install {self.iso / 'e/GameData/Data/_LVL_PC'}",
                     f"DLC: {self.iso / 'd'} (Kit Fisto and Asajj Ventress used, off until the DLC Heroes row in Hero Options is turned on)",
                     f"not supported yet: {self.iso / 'c'} - Xbox Star Wars: Battlefront II disc (trimmed); recognized, not read"):
            self.assertIn(line, printed)
        self.assertEqual(self.out.with_name('game.inputs.txt').read_text().strip(), printed[:printed.index('Input summary')].strip())

    def full_run(self, xbox_on, xbox_only=False, before=None):
        """pack.main end to end with the tools stubbed: what it runs and what it tells rebake.sh."""
        root = self.top
        if before:
            before(root)
        engine = root / 'engine'
        baked, missions = engine / 'assets_baked', engine / 'missions'
        missions.mkdir(parents=True)
        (missions / 'one.ron').touch()
        (root / 'scripts').mkdir()
        (root / 'scripts/rebake.sh').write_text('BF2_MAPS="MUS/MUS1"\n')
        ps2, xbox = self.iso / 'p', self.iso / 'x'
        iso_image(self.iso / 'one', 'bf1')
        if not xbox_only:
            iso_image(ps2, 'bf2')
        xbox_image(xbox, 'bf2')
        target = engine / 'target'
        write(target / 'release' / ('game' + pack.EXE), b'binary')
        calls, envs = [], []

        def run(command, **kwargs):
            calls.append(command)
            if kwargs.get('env'):
                envs.append(kwargs['env'])
            if command[1:2] == ['extract-iso']:
                game = 'bf1' if command[2].endswith('one') else 'bf2'
                tree(Path(command[3]) / 'DATA/_LVL_PS2', game, 'ps2')
                for library in pack.LUA[game].values():
                    write(pack.ci_path(Path(command[3]) / 'DATA/_LVL_PS2', library), b'x')
                write(Path(command[3]) / pack.GAME_ELF[game], b'elf')
            if command[1:2] == [scan.XBOX_EXTRACT]:
                tree(Path(command[3]) / 'Data/_LVL_XBOX', 'bf2', 'xbox')
                for library in pack.LUA['bf2'].values():
                    write(pack.ci_path(Path(command[3]) / 'Data/_LVL_XBOX', library), b'x')
            if len(command) > 2 and command[1] == '--write-pack':
                Path(command[2]).write_bytes(b'packed')
            if 'rebake.sh' in ' '.join(command):
                write(baked / 'classes/bf1_all.ron')

        values = dict(ROOT=str(root), ENGINE=str(engine), EXTRACTED=str(root / 'extracted'), BAKED=str(baked),
                      MISSIONS=str(missions), TARGET=str(target), BAKE_STAMP=str(root / 'extracted/bake.sha'),
                      MISSION_STAMP=str(root / 'extracted/mission.sha'))
        args = ['pack.py', '--iso-dir', str(self.iso), '--out', str(self.out)]
        with mock.patch.multiple(pack, **values), mock.patch.object(sys, 'argv', args), mock.patch.object(pack, 'need'), \
                mock.patch.object(pack, 'run', side_effect=run), mock.patch.object(pack, 'find_bash', return_value='bash'), \
                mock.patch.object(pack, 'stale', return_value=None), mock.patch.object(pack, 'inputs_hash', return_value='hash'), \
                mock.patch.object(pack, 'missing_music', return_value=[]), mock.patch.object(pack.shutil, 'which', return_value=None), \
                mock.patch.object(pack.subprocess, 'run', side_effect=self.tool), \
                mock.patch.object(scan, 'XBOX_SUPPORTED', xbox_on), mock.patch.object(pack.os, 'environ', dict(os.environ)), \
                contextlib.redirect_stdout(io.StringIO()):
            pack.main()
        return calls, envs[-1], ps2, xbox

    def tool(self, command, **kwargs):
        """git and the foliage check, as pack.py runs them directly."""
        self.foliage_checks = getattr(self, 'foliage_checks', [])
        if 'check-foliage' in command:
            self.foliage_checks.append(command)
        return mock.Mock(stdout=getattr(self, 'tool_output', 'tracked'), stderr=getattr(self, 'tool_error', ''),
                         returncode=getattr(self, 'tool_code', 0))

    def old_extraction(self, root, elf=False):
        """A tree extracted before the executable came along, or with it."""
        for game in pack.LUA:
            data = root / 'extracted' / game / 'lvl/DATA/_LVL_PS2'
            tree(data, game, 'ps2')
            for library in pack.LUA[game].values():
                write(pack.ci_path(data, library), b'x')
            if elf:
                write(data.parent.parent / pack.GAME_ELF[game], b'elf')

    def test_an_extraction_without_the_executable_is_finished_from_the_disc(self):
        calls, _, _, _ = self.full_run(False, before=self.old_extraction)
        self.assertEqual(sorted(Path(call[3]).parent.name for call in calls if call[1:2] == ['extract-iso']), ['bf1', 'bf2'])
        for game in pack.LUA:
            self.assertTrue((self.top / 'extracted' / game / 'lvl' / pack.GAME_ELF[game]).is_file())

    def test_a_whole_extraction_is_left_alone(self):
        calls, _, _, _ = self.full_run(False, before=lambda root: self.old_extraction(root, elf=True))
        self.assertFalse([call for call in calls if call[1:2] == ['extract-iso']])

    def test_every_build_checks_its_foliage_and_lost_foliage_stops_it(self):
        self.full_run(False)
        self.assertEqual(len(self.foliage_checks), 1)
        self.assertNotIn('--warn', self.foliage_checks[0])
        self.tool_code, self.tool_error = 1, '14 map(s) lost their terrain foliage.'
        with self.assertRaises(SystemExit) as stop:
            self.top = self.top / 'again'
            self.top.mkdir()
            self.iso = self.top / 'iso'
            self.iso.mkdir()
            self.full_run(False)
        self.assertIn('lost their terrain foliage', str(stop.exception))

    def test_a_tree_that_is_not_the_ps2s_turns_lost_foliage_into_a_summary_warning(self):
        self.tool_output = 'foliage: 0 instances\nWARNING: foliage lost on bf2_mus1: foliage_grass 0 of 1\n'
        self.full_run(True, xbox_only=True)
        self.assertIn('--warn', self.foliage_checks[0])
        report = json.loads(Path(str(self.out) + '.sources.json').read_text())
        self.assertEqual(report['warnings'], ['foliage lost on bf2_mus1: foliage_grass 0 of 1'])

    def test_a_game_on_a_ps2_tree_beside_one_that_is_not_still_owes_its_foliage(self):
        plan = dict(games=dict(bf1=dict(gameplay=dict(kind='ps2-disc')), bf2=dict(gameplay=dict(kind='xbox-disc'))))
        with mock.patch.object(pack.subprocess, 'run', side_effect=self.tool), contextlib.redirect_stdout(io.StringIO()):
            pack.check_foliage('bake', plan)
        self.assertEqual(self.foliage_checks[0], ['bake', 'check-foliage', pack.BAKED, '--warn', 'bf2'])

    def test_xbox_stays_unread_until_the_switch(self):
        calls, env, ps2, xbox = self.full_run(False)
        self.assertFalse([call for call in calls if scan.XBOX_EXTRACT in call])
        self.assertEqual(env['BF2_CONSOLE_ISO'], str(ps2))
        self.assertNotIn('XB2', env)
        self.assertEqual(env['OPENBF_INPUTS_CHOSEN'], '1')
        report = json.loads(Path(str(self.out) + '.sources.json').read_text())
        row = next(row for row in report['inputs'] if row['path'] == str(xbox))
        self.assertEqual(row['status'], 'not supported yet')
        self.assertEqual(report['input_scan']['games']['bf2']['gameplay']['kind'], 'ps2-disc')

    def test_a_stale_environment_cannot_add_to_the_scans_choice(self):
        stale = {name: '/stale' for name in ('XB1', 'XB2', 'OPENBF_BF1_TREE', 'OPENBF_BF2_TREE', 'OPENBF_DLC_DIR', 'OPENBF_DLC_DONOR')}
        with mock.patch.dict(os.environ, stale):
            _, env, ps2, _ = self.full_run(False)
        self.assertFalse(set(stale) & set(env), env)
        self.assertEqual(env['BF2_CONSOLE_ISO'], str(ps2))

    def test_the_switch_extracts_and_passes_the_donor(self):
        calls, env, _, xbox = self.full_run(True)
        extract = [call for call in calls if scan.XBOX_EXTRACT in call]
        self.assertEqual(len(extract), 1)
        self.assertEqual(extract[0][2], str(xbox))
        self.assertEqual(env['XB2'], str(self.top / 'extracted/xbox/bf2/Data/_LVL_XBOX'))
        self.assertNotIn('OPENBF_BF2_TREE', env)

    def test_the_switch_with_no_ps2_makes_xbox_the_data(self):
        calls, env, _, xbox = self.full_run(True, xbox_only=True)
        self.assertEqual(env['OPENBF_BF2_TREE'], str(self.top / 'extracted/xbox/bf2/Data/_LVL_XBOX'))
        self.assertNotIn('XB2', env)
        self.assertNotIn('BF2_CONSOLE_ISO', env)
        self.assertEqual([call[2] for call in calls if scan.XBOX_EXTRACT in call], [str(xbox)])
        self.assertFalse([call for call in calls if 'extract-iso' in call and 'bf2' in call[3]])


class RebakeCase(unittest.TestCase):
    """The real rebake.sh with a stub bake and cargo: which arguments each bake gets."""
    ROOT_NAME = ''

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = root = Path(directory.name) / self.ROOT_NAME
        (root / 'scripts').mkdir(parents=True)
        for name in ('rebake.sh', 'console_input.py', 'input_scan.py', 'dlc_inputs.py', 'bake_dlc.py'):
            shutil.copyfile(ROOT / 'scripts' / name, root / 'scripts' / name)
        (root / 'engine').mkdir()
        target = root / 'target/release'
        target.mkdir(parents=True)
        (target / 'bake').write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$BAKE_LOG"\n')
        (target / 'bake').chmod(0o755)
        (root / 'bin').mkdir()
        (root / 'bin/cargo').write_text('#!/bin/sh\nexit 0\n')
        (root / 'bin/cargo').chmod(0o755)
        tree(root / 'extracted/bf1/lvl/DATA/_LVL_PS2', 'bf1', 'ps2')
        tree(root / 'extracted/bf2/lvl/DATA/_LVL_PS2', 'bf2', 'ps2')
        self.log = root / 'calls.log'
        self.env = dict(os.environ, PATH=str(root / 'bin') + os.pathsep + os.environ['PATH'],
                        CARGO_TARGET_DIR=str(target.parent), BAKE_LOG=str(self.log), REBAKE_OUT=str(root / 'out'),
                        OPENBF_BF1_MAPS='BES/BES1', OPENBF_BF2_MAPS='MUS/MUS1')
        for name in ('PC1', 'PC2', 'XB1', 'XB2', 'BF2_CONSOLE_ISO', 'OPENBF_INPUTS_CHOSEN', 'OPENBF_DLC_DIR', 'OPENBF_DLC_DONOR'):
            self.env.pop(name, None)

    def rebake(self, *groups, **env):
        if self.log.exists():
            self.log.unlink()
        result = subprocess.run(['bash', str(self.root / 'scripts/rebake.sh'), *groups], env={**self.env, **env},
                                capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result, self.log.read_text().splitlines() if self.log.exists() else []

    def donors(self, tree_root):
        write(tree_root / 'bes/bes1.lvl')
        write(tree_root / 'mus/mus1.lvl')
        for side in ('all', 'imp', 'rep', 'cis'):
            write(tree_root / f'side/{side}.lvl')


class RebakeWithheld(RebakeCase):
    def test_a_left_out_map_is_not_baked_and_an_earlier_copy_goes(self):
        stale = self.root / 'out/bf1_bes2/manifest.ron'
        write(stale)
        kept = self.root / 'out/bf1_bes1/manifest.ron'
        write(kept)
        _, calls = self.rebake('maps', OPENBF_BF1_MAPS='BES/BES1 BES/BES2', OPENBF_BF1_WITHHELD='BES/BES2')
        self.assertFalse(stale.exists())
        self.assertTrue(any(' ' + str(self.root / 'out/bf1_bes1') in call for call in calls), calls)
        self.assertFalse([call for call in calls if 'bes2' in call.lower()])

    def test_a_link_to_other_assets_keeps_its_maps(self):
        other = self.root / 'other'
        stale = other / 'bf1_bes2/manifest.ron'
        write(stale)
        self.env['REBAKE_OUT'] = str(self.root / 'link')
        (self.root / 'link').symlink_to(other)
        self.rebake('maps', OPENBF_BF1_MAPS='BES/BES1', OPENBF_BF1_WITHHELD='BES/BES2')
        self.assertTrue(stale.exists())


class RebakeDlc(RebakeCase):
    STALE = ('side_bf2/dlc_heroes.ron', 'classes/bf2_dlc.ron', 'side_bf2/dlc_sounds/sounds.ron', 'side_bf2/effects/dlc/effects.ron')

    def stale(self):
        for name in self.STALE:
            write(self.root / 'out' / name, b'an earlier bake')

    def heroes(self, calls):
        return [line for line in calls if line.startswith('dlc-heroes ')]

    def test_the_scans_folder_is_baked_into_the_real_output_with_the_game_reference(self):
        dlc = dlc_folder(self.root / 'iso/renamed')
        _, calls = self.rebake('dlc')
        self.assertEqual(len(self.heroes(calls)), 1, calls)
        line = self.heroes(calls)[0].split()
        self.assertEqual(line[2:], [str(self.root / 'extracted/bf2/lvl/DATA/_LVL_PS2/INGAME.LVL'), str((self.root / 'out').resolve())])
        self.assertTrue(dlc.is_dir())

    def test_under_pack_the_environment_is_the_choice(self):
        dlc_folder(self.root / 'iso/not-chosen')
        chosen = dlc_folder(self.root / 'elsewhere', geometry=b'_PS2')
        _, calls = self.rebake('dlc', OPENBF_INPUTS_CHOSEN='1', OPENBF_DLC_DIR=str(chosen))
        self.assertEqual(len(self.heroes(calls)), 1)
        _, calls = self.rebake('dlc', OPENBF_INPUTS_CHOSEN='1')
        self.assertEqual(self.heroes(calls), [])

    def test_a_pc_copy_arrives_as_a_second_source(self):
        ps2, pc = dlc_folder(self.root / 'a', geometry=b'_PS2'), dlc_folder(self.root / 'b')
        _, calls = self.rebake('dlc', OPENBF_INPUTS_CHOSEN='1', OPENBF_DLC_DIR=str(ps2), OPENBF_DLC_DONOR=str(pc))
        self.assertEqual(len(self.heroes(calls)[0].split()), 5, calls)  # the side, reference, output and donor LVL

    def test_side_runs_it_after_clearing_what_it_stands_on(self):
        dlc_folder(self.root / 'iso/a')
        _, calls = self.rebake('side')
        self.assertTrue(self.heroes(calls))
        self.assertLess(max(index for index, line in enumerate(calls) if line.startswith(('soldier ', 'vehicle '))),
                        calls.index(self.heroes(calls)[0]))

    def test_no_folder_takes_an_earlier_bakes_enabling_files_away(self):
        for group in ('dlc', 'side'):
            self.stale()
            _, calls = self.rebake(group)
            self.assertEqual(self.heroes(calls), [])
            for name in self.STALE:
                self.assertFalse((self.root / 'out' / name).exists(), (group, name))

    def test_other_groups_leave_the_heroes_alone(self):
        dlc_folder(self.root / 'iso/a')
        self.stale()
        _, calls = self.rebake('beep')
        self.assertEqual(self.heroes(calls), [])
        self.assertTrue((self.root / 'out' / self.STALE[0]).exists())

    def test_a_linked_output_is_refused(self):
        dlc_folder(self.root / 'iso/a')
        (self.root / 'shared').mkdir()
        (self.root / 'linked').symlink_to(self.root / 'shared')
        result = subprocess.run(['bash', str(self.root / 'scripts/rebake.sh'), 'dlc'], capture_output=True, text=True,
                                env={**self.env, 'REBAKE_OUT': str(self.root / 'linked')}, timeout=60)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn('REBAKE_OUT', result.stderr)

    def test_a_linked_output_loses_nothing_when_refused(self):
        """The link's target is someone else's bake: refusing it must not first delete their heroes, DLC folder or not."""
        (self.root / 'linked').symlink_to(self.root / 'shared', target_is_directory=True)
        for name in self.STALE:
            write(self.root / 'shared' / name, b'a shared bake')
        for folder in (False, True):
            if folder:
                dlc_folder(self.root / 'iso/a')
            result = subprocess.run(['bash', str(self.root / 'scripts/rebake.sh'), 'dlc'], capture_output=True, text=True,
                                    env={**self.env, 'REBAKE_OUT': str(self.root / 'linked')}, timeout=60)
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            for name in self.STALE:
                self.assertTrue((self.root / 'shared' / name).exists(), f'{name}, folder={folder}')

    def test_a_folder_is_refused_over_a_link_before_any_group_runs(self):
        dlc_folder(self.root / 'iso/a')
        (self.root / 'linked').symlink_to(self.root / 'shared', target_is_directory=True)
        write(self.root / 'shared/side_bf1/soldiers/kept.glb', b'a shared bake')
        result = subprocess.run(['bash', str(self.root / 'scripts/rebake.sh')], capture_output=True, text=True,
                                env={**self.env, 'REBAKE_OUT': str(self.root / 'linked')}, timeout=60)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertFalse(self.log.exists(), 'no bake ran')
        self.assertTrue((self.root / 'shared/side_bf1/soldiers/kept.glb').exists())

    def test_side_over_a_link_without_a_folder_still_clears_the_heroes_it_deletes(self):
        (self.root / 'linked').symlink_to(self.root / 'shared', target_is_directory=True)
        write(self.root / 'shared' / self.STALE[0], b'an earlier bake')
        self.rebake('side', REBAKE_OUT=str(self.root / 'linked'))
        self.assertFalse((self.root / 'shared' / self.STALE[0]).exists())

    def test_a_link_above_the_output_is_not_the_outputs_own(self):
        dlc_folder(self.root / 'iso/a')
        (self.root / 'real/below').mkdir(parents=True)
        (self.root / 'above').symlink_to(self.root / 'real')
        _, calls = self.rebake('dlc', REBAKE_OUT=str(self.root / 'above/below'))
        self.assertEqual(self.heroes(calls)[0].split()[3], str((self.root / 'real/below').resolve()))


class Rebake(RebakeCase):
    def test_no_donors_means_the_ps2_alone(self):
        _, calls = self.rebake('maps', 'side')
        maps = [line for line in calls if line.startswith('map ')]
        self.assertEqual(len(maps), 2)
        self.assertTrue(all(len(line.split()) == 3 for line in maps), maps)
        self.assertFalse([line for line in calls if 'xbox' in line])

    def test_xbox_trees_reach_every_map_and_side_bake(self):
        xbox1, xbox2 = self.root / 'extracted/xbox/bf1/Data/_LVL_XBOX', self.root / 'extracted/xbox/bf2/Data/_LVL_XBOX'
        self.donors(xbox1)
        self.donors(xbox2)
        _, calls = self.rebake('maps', 'side', 'tat1', 'tat2', XB1=str(xbox1), XB2=str(xbox2))
        self.assertTrue([line for line in calls if line.startswith('map ') and line.endswith(f'{xbox1}/bes/bes1.lvl')])
        self.assertTrue([line for line in calls if line.startswith('map ') and line.endswith(f'{xbox2}/mus/mus1.lvl')])
        for kind in ('soldier', 'weapon', 'vehicle'):
            for tree_root in (xbox1, xbox2):
                for side in ('all', 'imp', 'rep', 'cis'):
                    self.assertTrue([line for line in calls if line.startswith(kind + ' ') and line.endswith(f'{tree_root}/side/{side}.lvl')],
                                    (kind, tree_root, side))

    def test_pc_and_xbox_join_into_one_argument_pc_first(self):
        xbox2 = self.root / 'extracted/xbox/bf2/Data/_LVL_XBOX'
        self.donors(xbox2)
        pc = tree(self.root / 'pc/GameData/Data/_LVL_PC', 'bf2', 'pc')
        self.donors(pc)
        _, calls = self.rebake('maps', 'side', PC2=str(pc), XB2=str(xbox2))
        self.assertTrue([line for line in calls if line.startswith('map ') and line.endswith(f'{pc}/mus/mus1.lvl,{xbox2}/mus/mus1.lvl')])
        for side in ('all', 'imp', 'rep', 'cis'):
            self.assertTrue([line for line in calls if line.startswith('soldier ') and line.endswith(f'{pc}/side/{side}.lvl,{xbox2}/side/{side}.lvl')], side)

    def test_run_by_hand_it_scans_iso_by_content(self):
        pc = pc_install(self.root / 'iso/anything', 'bf2') / 'GameData/Data/_LVL_PC'
        self.donors(pc)
        disc = iso_image(self.root / 'iso/not-the-usual-name', 'bf2')
        result, calls = self.rebake('maps', 'common')
        self.assertIn('Inputs (recognized by content)', result.stderr)
        self.assertTrue([line for line in calls if line.startswith('map ') and line.endswith(f'{pc}/mus/mus1.lvl')])
        shell = next(line for line in calls if line.startswith('shell '))
        self.assertIn(str(disc), shell)

    def test_under_pack_it_does_not_scan(self):
        pc_install(self.root / 'iso/anything', 'bf2')
        result, calls = self.rebake('maps', OPENBF_INPUTS_CHOSEN='1')
        self.assertNotIn('Inputs (recognized by content)', result.stderr)
        self.assertTrue(all(len(line.split()) == 3 for line in calls if line.startswith('map ')))

    def test_xbox_icons_come_from_the_chosen_disc_or_a_hand_run_tree(self):
        icons = lambda calls: [line.split()[1] for line in calls if line.startswith('xbox-icons ')]
        write(self.root / 'extracted/bf2/lvl/DATA/_LVL_PS2/MOVIES/SHELL.MVS')
        chosen = self.root / 'extracted/xbox/bf2/Data/_LVL_XBOX'
        write(chosen / 'common.lvl')
        _, calls = self.rebake('common', XB2=str(chosen))
        self.assertEqual(icons(calls), [str(chosen / 'common.lvl')])
        _, calls = self.rebake('common')
        self.assertEqual(icons(calls), [])
        # A leftover extraction is read by hand, never under a scan that did not choose it.
        write(self.root / 'extracted/xbox_bf2/common.lvl')
        _, calls = self.rebake('common')
        self.assertEqual(icons(calls), [str(self.root / 'extracted/xbox_bf2/common.lvl')])
        _, calls = self.rebake('common', OPENBF_INPUTS_CHOSEN='1')
        self.assertEqual(icons(calls), [])

    def test_a_named_pc_folder_stops_the_scan_from_adding_another(self):
        first = tree(self.root / 'named/GameData/Data/_LVL_PC', 'bf2', 'pc')
        self.donors(first)
        other = pc_install(self.root / 'iso/other', 'bf2') / 'GameData/Data/_LVL_PC'
        self.donors(other)
        _, calls = self.rebake('maps', PC2=str(first))
        self.assertTrue([line for line in calls if line.endswith(f'{first}/mus/mus1.lvl')])
        self.assertFalse([line for line in calls if str(other) in line])


class ReviewFindings(unittest.TestCase):
    """Defects an independent review reproduced on the real tools; each test failed before its fix."""
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.top = Path(directory.name)
        self.iso = self.top / 'iso'
        self.iso.mkdir()

    def scan_in_process(self, root):
        code = f"import sys; sys.path.insert(0, {str(ROOT / 'scripts')!r}); import input_scan; print(input_scan.scan_root({str(root)!r})[0])"
        return subprocess.run([sys.executable, '-c', code], capture_output=True, text=True, timeout=30).stdout

    def test_a_pipe_in_a_folder_does_not_hang_the_scan(self):
        dlc_folder(self.iso / 'dlc', heroes=('x',))
        os.mkfifo(self.iso / 'dlc/data/_LVL_PC/pipe')
        self.assertIn('without the DLC heroes', self.scan_in_process(self.iso))
        other = self.top / 'other'
        (other / 'addon').mkdir(parents=True)
        os.mkfifo(other / 'addon/addme.script')
        os.mkfifo(other / 'addon/CORE.LVL')
        self.scan_in_process(other)

    def test_a_64_bit_folder_linked_in_from_outside_is_still_refused(self):
        outside = self.top / 'outside/Collection'
        tree(outside / 'GameData/Data/_LVL_PC', 'bf2', 'pc')
        windows_program(outside / 'Launcher.exe', 0x8664)
        (self.iso / 'whole').symlink_to(outside)
        (self.iso / 'data').symlink_to(outside / 'GameData/Data/_LVL_PC')
        plan = plan_of(self.iso)
        self.assertFalse([row for row in plan['found'] if row['kind'] == 'pc-install'], plan['found'])

    def test_a_32_bit_install_linked_in_is_still_accepted(self):
        outside = self.top / 'outside/Install'
        tree(outside / 'GameData/Data/_LVL_PC', 'bf2', 'pc')
        windows_program(outside / 'Game.exe')
        (self.iso / 'link').symlink_to(outside)
        self.assertEqual([row['kind'] for row in plan_of(self.iso)['found']], ['pc-install'])

    def test_of_two_copies_the_first_by_name_wins_whatever_the_folder_order(self):
        for name in ('b', 'c', 'a'):
            pc_install(self.iso / name, 'bf2')
        plan = plan_of(self.iso)
        self.assertEqual(plan['games']['bf2']['visual'][0]['path'], str(self.iso / 'a/GameData/Data/_LVL_PC'))

    def test_a_cut_short_xbox_disc_is_refused(self):
        disc = xbox_image(self.iso / 'x.iso', 'bf2')
        self.assertEqual(scan.classify_file(disc)['status'], 'found')
        os.truncate(disc, 305 * 2048)
        row = scan.classify_file(disc)
        self.assertEqual(row['status'], 'refused')
        self.assertIn('cut short', row['reason'])

    def test_a_relative_folder_gives_paths_the_bake_can_read_after_it_changes_directory(self):
        iso_image(self.iso / 'a.iso', 'bf1')
        disc = iso_image(self.iso / 'b.iso', 'bf2')
        pc = pc_install(self.iso / 'pc', 'bf2') / 'GameData/Data/_LVL_PC'
        named = iso_image(self.top / 'named.iso', 'bf1')
        here = os.getcwd()
        os.chdir(self.top)
        try:
            plan = scan.scan(['iso'], {'bf1': 'named.iso'})
        finally:
            os.chdir(here)
        env = scan.bake_env(plan)
        real = lambda path: os.path.realpath(path)
        self.assertTrue(os.path.isabs(env['PC2']) and os.path.isabs(env['BF2_CONSOLE_ISO']), env)
        self.assertEqual(real(env['BF2_CONSOLE_ISO']), real(disc))
        self.assertEqual(real(env['PC2']), real(pc))
        self.assertEqual(real(plan['games']['bf1']['gameplay']['path']), real(named))
        self.assertTrue(os.path.isabs(plan['games']['bf1']['gameplay']['path']))

    def test_a_named_folder_that_is_not_there_is_an_error_not_an_empty_scan(self):
        extracted = self.top / 'extracted'
        tree(extracted / 'bf1/lvl/DATA/_LVL_PS2', 'bf1', 'ps2')
        tree(extracted / 'bf2/lvl/DATA/_LVL_PS2', 'bf2', 'ps2')
        typo = self.top / 'isso'
        plan = scan.scan([typo], extracted=extracted, named_root=typo)
        self.assertTrue([line for line in plan['refusals'] if str(typo) in line and 'no such folder' in line], plan['refusals'])
        self.assertFalse(scan.scan([self.iso], extracted=extracted, named_root=self.iso)['refusals'])

    def run_scan_tool(self, *arguments, **env):
        tool = self.top / 'checkout/scripts'
        tool.mkdir(parents=True, exist_ok=True)
        for name in ('input_scan.py', 'console_input.py', 'dlc_inputs.py'):
            shutil.copyfile(ROOT / 'scripts' / name, tool / name)
        return subprocess.run([sys.executable, str(tool / 'input_scan.py'), *arguments], capture_output=True, timeout=30,
                              env={**os.environ, **env})

    def test_shell_output_carries_a_path_that_is_not_text(self):
        iso = self.top / 'checkout/iso'
        name = os.fsdecode(os.fsencode(str(iso)) + b'/pc\xff')
        pc_install(Path(name), 'bf2')
        result = self.run_scan_tool('--root', str(self.top / 'checkout'), '--shell')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(b'/pc\xff/', result.stdout)

    def test_a_console_that_cannot_show_a_path_does_not_stop_the_scan(self):
        iso_image(self.iso / '\u5730\u56f3.iso', 'bf1')
        iso_image(self.iso / 'b.iso', 'bf2')
        result = self.run_scan_tool('--iso-dir', str(self.iso), PYTHONIOENCODING='cp1252')
        self.assertEqual(result.returncode, 0, result.stderr)


class ReviewFindingsRebake(RebakeCase):
    """Which install and disc the real rebake.sh reads when the scan chose none."""
    def default_install(self, machine=0x14c):
        install = self.root / 'PC_swbf2_exe/Star Wars Battlefront II Classic'
        lvl = tree(install / 'GameData/data/_lvl_pc', 'bf2', 'pc')
        self.donors(lvl)
        windows_program(install / 'Launcher.exe', machine)
        return lvl

    def test_an_install_the_scan_refused_is_not_read(self):
        lvl = self.default_install(0x8664)
        result, calls = self.rebake('maps')
        self.assertIn('refused', result.stderr)
        self.assertFalse([line for line in calls if str(lvl) in line], calls)

    def test_under_pack_only_the_chosen_inputs_are_read(self):
        lvl = self.default_install()
        _, calls = self.rebake('maps', 'side', OPENBF_INPUTS_CHOSEN='1')
        self.assertFalse([line for line in calls if str(lvl) in line], calls)

    def test_the_default_install_is_still_found_by_the_scan(self):
        lvl = self.default_install()
        _, calls = self.rebake('maps')
        self.assertTrue([line for line in calls if line.endswith(f'{lvl}/mus/mus1.lvl')], calls)

    def test_pc_bf1_primary_reaches_the_real_bake_recipe(self):
        install = self.root / 'PC_swbf1_exe/Classic'
        lvl = tree(install / 'GameData/Data/_LVL_PC', 'bf1', 'pc')
        windows_program(install / 'Battlefront.exe')
        self.donors(lvl)
        shutil.rmtree(self.root / 'extracted/bf1')
        # The scan's gate is off (test_pc_primary_stays_off_...): this is the recipe once it is on.
        _, calls = self.rebake('maps', OPENBF_INPUTS_CHOSEN='1', OPENBF_BF1_TREE=str(lvl))
        bf1 = [line for line in calls if line.startswith('map ') and 'BES1' in line.upper()]
        self.assertEqual(len(bf1), 1)
        self.assertIn(str(lvl), bf1[0])
        self.assertNotIn('_LVL_PS2/BES', bf1[0])

    def test_first_person_weapons_take_their_sides_donors(self):
        lvl = self.default_install()
        xbox1 = self.root / 'extracted/xbox/bf1/Data/_LVL_XBOX'
        self.donors(xbox1)
        _, calls = self.rebake('side', XB1=str(xbox1))
        first = [line for line in calls if line.startswith('weapon ') and '_1st_weap_inf_' in line and '/SIDE/' in line]
        self.assertEqual(len(first), 8, calls)
        for game, root in (('bf1', xbox1), ('bf2', lvl)):
            for side in ('all', 'imp', 'rep', 'cis'):
                self.assertTrue([line for line in first if f' {side}_1st_weap' in line and f'/{game}/' in line
                                 and line.endswith(f'{root}/side/{side}.lvl')], (game, side, first))

    def test_every_donor_variable_is_one_the_script_sets(self):
        text = (self.root / 'scripts/rebake.sh').read_text()
        self.assertNotIn('PCSIDE_', text)

    def test_no_shell_movie_stops_with_a_message(self):
        result = subprocess.run(['bash', str(self.root / 'scripts/rebake.sh'), 'common'], env=self.env,
                                capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn('SHELL.MVS', result.stderr)
        self.assertFalse(self.log.exists() and [line for line in self.log.read_text().splitlines() if line.startswith('shell ')])


class RebakeOnAnyMachine(RebakeCase):
    ROOT_NAME = 'a checkout (with spaces)'

    def run_group(self, *groups, **env):
        return self.rebake(*groups, **env)

    def test_a_folder_with_spaces_keeps_each_sound_folder_one_argument(self):
        for name in ('tat1', 'bf2_mus1'):
            (self.root / 'out' / name / 'sounds').mkdir(parents=True)
        bake = self.root / 'target/release/bake'
        bake.write_text('#!/bin/sh\nif [ "$1" = sound-names ]; then for a; do printf "%s\\n" "$a"; done > "$BAKE_LOG.args"; fi\n')
        self.run_group('sounds')
        out = self.root / 'out'
        self.assertEqual(Path(str(self.log) + '.args').read_text().splitlines()[3:], [str(out / 'bf2_mus1'), str(out / 'tat1')])

    def store_stand_in(self):
        """A `python3` that runs nothing, as the Store's alias on a fresh Windows."""
        stub = self.root / 'bin/python3'
        stub.write_text('#!/bin/sh\necho "Python was not found; run without arguments to install from the Microsoft Store" >&2\nexit 9009\n')
        stub.chmod(0o755)

    def test_the_python_pack_passes_is_the_one_it_runs(self):
        self.store_stand_in()
        pc = pc_install(self.root / 'mine pc', 'bf1')
        _, calls = self.run_group('tat1', OPENBF_INPUTS_CHOSEN='1', OPENBF_PYTHON=sys.executable, PC1=str(pc))
        self.assertTrue([line for line in calls if line.startswith('map ')], calls)

    def test_by_hand_a_python_that_runs_nothing_is_passed_over(self):
        self.store_stand_in()
        real = self.root / 'bin/python'
        real.write_text(f'#!/bin/sh\nexec "{sys.executable}" "$@"\n')
        real.chmod(0o755)
        result, _ = self.run_group('tat1')
        self.assertIn('Gameplay data: BF1=PS2 BF2=PS2', result.stdout)

    def test_a_tree_with_lowercase_names_reaches_the_first_person_bake(self):
        lower = self.root / 'lowercase'
        write(lower / 'core.lvl')
        write(lower / 'fpm/all/alltroop.lvl')
        write(lower / 'common/eng/common.lvl')
        result, calls = self.run_group('side', OPENBF_BF1_TREE=str(lower))
        fpm = [line for line in calls if line.startswith('fpm ')]
        self.assertIn(str(lower / 'fpm/all/alltroop.lvl'), fpm[0], fpm)
        self.assertIn(f'Gameplay data: BF1={lower} BF2=PS2', result.stdout)

    def test_a_python_2_is_not_taken_for_python_3(self):
        self.store_stand_in()
        old = self.root / 'bin/python'
        old.write_text('#!/bin/sh\ncase "$2" in *version_info*) exit 1 ;; esac\nexit 0\n')
        old.chmod(0o755)
        env = {**self.env, 'PATH': str(self.root / 'bin') + os.pathsep + '/bin_none'}
        bash = shutil.which('bash')
        result = subprocess.run([bash, str(self.root / 'scripts/rebake.sh'), 'tat1'], env=env, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn('no Python 3 found', result.stderr)

    def test_every_lvl_of_a_lowercase_tree_reaches_the_bake(self):
        """The bake opens the path it is given, and on Linux FPM/ is not fpm/: one literal path stops the whole build."""
        bf1, bf2 = self.root / 'lower1', self.root / 'lower2'
        for root in (bf1, bf2):
            for name in ('core.lvl', 'ingame.lvl', 'shell.lvl', 'common/eng/common.lvl', 'fpm/all/alltroop.lvl',
                         *(f'side/{side}.lvl' for side in ('all', 'imp', 'rep', 'cis', 'gun', 'des', 'ewk', 'gam', 'geo', 'tur', 'wok'))):
                write(root / name)
        bake = self.root / 'target/release/bake'
        bake.write_text("""#!/bin/sh
for arg in "$@"; do
  old_ifs=$IFS; IFS=,
  for one in $arg; do
    case "$one" in
      *.lvl|*.LVL) [ -e "$one" ] || printf '%s\\n' "$one" >> "$BAKE_LOG.missing" ;;
    esac
  done
  IFS=$old_ifs
done
for one in "$BF1_ANIMS" "$BF2_ANIMS"; do
  [ -z "$one" ] || [ -e "$one" ] || printf '%s\\n' "$one" >> "$BAKE_LOG.missing"
done
printf '%s\\n' "$*" >> "$BAKE_LOG"
""")
        _, calls = self.run_group('side', OPENBF_BF1_TREE=str(bf1), OPENBF_BF2_TREE=str(bf2))
        self.assertTrue([line for line in calls if line.startswith('soldier ')], calls)
        self.assertTrue([line for line in calls if line.startswith('fpm ')], calls)
        missing = Path(str(self.log) + '.missing')
        self.assertEqual(sorted(set(missing.read_text().splitlines())) if missing.exists() else [], [])

    def test_the_summary_names_the_donors_it_read(self):
        xbox = self.root / 'xbox'
        xbox.mkdir()
        result, _ = self.run_group('tat1', OPENBF_INPUTS_CHOSEN='1', XB2=str(xbox))
        self.assertIn(f'Xbox BF2={xbox}', result.stdout)
        self.assertNotIn('PS2-only', result.stdout)

    def test_nothing_in_it_needs_a_newer_bash_or_gnu_tools(self):
        """macOS ships bash 3.2 and BSD tools. Bash before 4.4 calls an empty array unbound under `set -u`."""
        text = (ROOT / 'scripts/rebake.sh').read_text()
        unguarded = [line for line in text.splitlines()
                     if re.search(r'"\$\{\w+\[@\]\}"', re.sub(r'\$\{(\w+)\[@\]\+"\$\{\1\[@\]\}"\}', '', line))]
        self.assertEqual(unguarded, [])
        for needs_more in ('mapfile', 'readarray', 'declare -A', 'xargs -r', 'readlink -f', 'sed -i', 'sha256sum', 'paste -sd,)'):
            self.assertNotIn(needs_more, text)
        self.assertIsNone(re.search(r'\$\{\w+(,,|\^\^)\}', text))


class ReviewFindingsReport(unittest.TestCase):
    def test_the_report_never_claims_an_install_the_plan_did_not_choose(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / 'scripts/rebake.sh', b'')
            pc = root / 'PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc'
            write(pc / 'side/rep.lvl')
            plan = dict(found=[], games={})
            report = pack.source_report(root, root / 'baked', root / 'missions', {'bf1': None, 'bf2': None}, plan,
                                        rebaked='all', pc_roots={})
            self.assertFalse([row for row in report['inputs'] if row['path'] == str(pc)])

    def test_a_donor_read_from_elsewhere_is_not_called_unused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / 'scripts/rebake.sh', b'')
            write(root / 'PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc/side/rep.lvl')
            write(root / 'extracted/xbox_bf2/common.lvl')
            pc, xbox = root / 'mine/bf2', root / 'mine/xbox.iso'
            write(pc / 'side/rep.lvl')
            write(xbox)
            plan = dict(games={}, found=[
                dict(path=str(pc), kind='pc-install', game='bf2', status='used', reason='texture donor'),
                dict(path=str(xbox), kind='xbox-disc', game='bf2', status='used', reason='texture donor')])
            report = pack.source_report(root, root / 'baked', root / 'missions', {'bf1': None, 'bf2': None}, plan,
                                        rebaked='all', pc_roots={'bf2': str(pc)})
            rows = report['inputs']
            self.assertEqual([row['status'] for row in rows if row['path'] == str(pc)], ['used'])
            self.assertEqual([row['status'] for row in rows if row['path'] == str(xbox)], ['used'])
            self.assertFalse([row for row in rows if 'optional' in row['label']])

    def report_for(self, root, rebaked, gameplay=None):
        pc = root / 'pc'
        pc.mkdir(exist_ok=True)
        games = {'bf1': dict(gameplay=dict(kind='ps2-disc', path=str(root / 'a.iso'))), 'bf2': dict(gameplay=gameplay or dict(kind='ps2-disc', path=str(root / 'b.iso')))}
        images = {'bf1': root / 'a.iso', 'bf2': None if gameplay else root / 'b.iso'}
        return pack.source_report(root, root / 'baked', root / 'missions', images, dict(games=games, found=[]),
                                  ('bf1', 'bf2'), (), rebaked, pc_roots={'bf1': str(pc)})

    def test_a_donor_behind_a_cached_bake_is_in_the_build_and_says_so(self):
        with tempfile.TemporaryDirectory() as directory:
            rows = {row['label']: row for row in self.report_for(Path(directory), None)['inputs']}
            self.assertEqual(rows['bf1 PC visual input']['status'], 'cached')
            self.assertIn('made with it', rows['bf1 PC visual input']['reason'])

    def test_a_partial_rebake_says_what_it_read(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sides = {row['label']: row for row in self.report_for(root, ['side'])['inputs']}['bf1 PC visual input']
            self.assertEqual(sides['status'], 'used')
            self.assertIn('earlier run', sides['reason'])
            sounds = {row['label']: row for row in self.report_for(root, ['sounds', 'global-sound'])['inputs']}['bf1 PC visual input']
            self.assertEqual(sounds['status'], 'cached')

    def test_gameplay_data_that_is_not_the_ps2s_is_not_called_ps2(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'baked/bf2_mus1/manifest.ron'
            write(manifest, b'(source_game: "bf2", textures: [], texture_tiers: [], texture_tier2: [])')
            xbox = root / 'xbox/Data/_LVL_XBOX'
            report = self.report_for(root, 'all', dict(kind='xbox-disc', path=str(root / 'x.iso'), tree=str(xbox)))
            labels = {row['label']: row['path'] for row in report['inputs']}
            self.assertEqual(labels['bf2 Xbox gameplay data'], str(xbox))
            self.assertNotIn('bf2 extracted PS2 data', labels)
            self.assertIn('bf1 extracted PS2 data', labels)
            self.assertEqual(report['maps'][0]['base_source'], 'bf2 Xbox')

    def test_gameplay_data_behind_a_cached_bake_says_cached(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            xbox = dict(kind='xbox-disc', path=str(root / 'x.iso'), tree=str(root / 'xbox'))
            games = {'bf2': dict(gameplay=xbox)}
            for used, status in (((), 'cached'), (('bf2',), 'used')):
                report = pack.source_report(root, root / 'baked', root / 'missions', {'bf2': None}, dict(games=games, found=[]), used)
                row = next(row for row in report['inputs'] if row['label'] == 'bf2 Xbox gameplay data')
                self.assertEqual(row['status'], status, row)

    def test_a_checkout_without_git_still_finds_its_missions_generated(self):
        with mock.patch.object(pack.subprocess, 'run', side_effect=FileNotFoundError('git')):
            self.assertEqual(pack.git_tracked_missions(), '')


if __name__ == '__main__':
    unittest.main()

class CorrectionMissionSelection(unittest.TestCase):
    def test_pc_primary_pins_both_correction_table_and_source(self):
        plan = {'games': {'bf1': {'gameplay': {'kind': 'pc-install', 'path': '/chosen/base'}},
                          'bf2': {'gameplay': {'kind': 'ps2-disc', 'path': '/chosen/disc'}}}}
        args = pack.gameplay_correction_args(plan)
        self.assertEqual(args[-2:], ['--gameplay-source', 'bf1=/chosen/base'])
        self.assertTrue(args[1].endswith('/corrections-v1/bf1-pc.json'))
        self.assertEqual(args.count('--gameplay-reference'), 1)
