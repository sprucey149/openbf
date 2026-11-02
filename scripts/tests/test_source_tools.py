"""Synthetic source-tool regressions; no discs, corpora, bakes or baselines."""
import contextlib
import csv
import io
import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import bot_quality as bot
import callgraph
import extract_lua as lua
import gen_missions as missions
import pack
import scan_tables


def ins(op, a=0, b=0, c=0, bx=None):
    return (missions.OPS50.index(op) | (a << 24) |
            ((bx << 6) if bx is not None else ((b << 15) | (c << 6))))


def proto_calls(calls):
    """Build literal calls using real Lua 5 instruction fields."""
    p = missions.Proto()
    for name, args in calls:
        p.k.append(name)
        p.code.append(ins('GETGLOBAL', bx=len(p.k) - 1))
        for register, value in enumerate(args, 1):
            if isinstance(value, missions.Proto):
                p.protos.append(value)
                p.code.append(ins('CLOSURE', a=register, bx=len(p.protos) - 1))
            else:
                p.k.append(value)
                p.code.append(ins('LOADK', a=register, bx=len(p.k) - 1))
        p.code.append(ins('CALL', b=len(args) + 1, c=1))
    return p


def globals_proto(items):
    p = missions.Proto()
    for name, child in items:
        p.k.append(name)
        p.protos.append(child)
        p.code += [ins('CLOSURE', bx=len(p.protos) - 1),
                   ins('SETGLOBAL', bx=len(p.k) - 1)]
    return p


def original_startup(f, verbs):
    """Reference discovery/walk: traversal order is a contract."""
    top, reg = {}, {}
    for instruction in f.code:
        op = missions.OPS50[instruction & 0x3f]
        a, bx = (instruction >> 24) & 0xff, (instruction >> 6) & 0x3ffff
        if op == 'CLOSURE' and bx < len(f.protos):
            reg[a] = f.protos[bx]
        elif op == 'SETGLOBAL' and a in reg and isinstance(f.k[bx], str):
            top[f.k[bx]] = reg.pop(a)
    base, out = missions._rk_base(f), []

    def run(name, depth):
        p = top.get(name)
        if p is None or depth > 4:
            return
        for call in missions.calls50(p, base=base, recurse=False):
            if call.name in verbs:
                for arg in call.args:
                    if isinstance(arg, str):
                        out.append((arg, call.name == verbs[0]))
            elif call.name in top and not call.args:
                run(call.name, depth + 1)
    run('ScriptPostLoad', 0)
    return out


class MissionTests(unittest.TestCase):
    def test_hero_classes_are_literal_last_wins_team_assignments(self):
        calls = [missions.Call('SetHeroClass', [1.0, 'all_hero_hansolo_tat']),
                 missions.Call('SetHeroClass', [2.0, 'imp_hero_bobafett']),
                 missions.Call('SetHeroClass', [1.0, 'all_hero_luke_jedi']),
                 missions.Call('SetHeroClass', [missions.Unknown(), 'invalid'])]
        self.assertEqual(missions.facts_from_calls(calls)['heroes'],
                         {1: 'all_hero_luke_jedi', 2: 'imp_hero_bobafett'})

    def test_side_class_load_order_keeps_all_requests_and_case(self):
        calls = [missions.Call('SetHeroClass', [2.0, 'fielded_second']),
                 missions.Call('ReadDataFile', [r'SIDE\REP.lvl', 'rep_HERO_aalya',
                                               'unfielded_vehicle', 'rep_HERO_aalya']),
                 missions.Call('ReadDataFile', [r'sound\rep.lvl;rep', 'not-a-class']),
                 missions.Call('ReadDataFile', [r'side\cis.lvl', 'fielded_second',
                                               'rep_HERO_aalya', 'ordinary_infantry']),
                 missions.Call('ReadDataFile', [r'WORLD\map.lvl', 'not-a-side-class'])]
        facts = missions.facts_from_calls(calls)
        self.assertEqual(facts['class_load_order'],
                         [('REP', 'rep_HERO_aalya'), ('REP', 'unfielded_vehicle'),
                          ('REP', 'rep_HERO_aalya'), ('cis', 'fielded_second'),
                          ('cis', 'rep_HERO_aalya'), ('cis', 'ordinary_infantry')])
        self.assertEqual(facts['heroes'], {2: 'fielded_second'})

    def test_side_class_load_order_reaches_only_bf2_ron(self):
        calls = [missions.Call('ReadDataFile', [r'SIDE\rep.lvl', 'z_last',
                                               'Unfielded', 'z_last', 'a_first'])]
        with mock.patch.object(missions, 'PS2_ONLY', True), \
                mock.patch.object(missions, 'load_ps2', return_value=missions.Proto()), \
                mock.patch.object(missions, 'calls50', return_value=calls), \
                mock.patch.object(missions, 'calls40', return_value=calls):
            bf2 = missions.build('x', 'bf2', 'x', 'cw', 'x', 'x')
            bf1 = missions.build('x', 'bf1', 'x', 'cw', 'x', 'x')
        emitted = missions.ron(bf2)
        self.assertIn('class_load_order: [(side: "rep", request: "z_last"), '
                      '(side: "rep", request: "Unfielded"), '
                      '(side: "rep", request: "z_last"), '
                      '(side: "rep", request: "a_first")]', emitted)
        self.assertNotIn('class_load_order:', missions.ron(bf1))

    def test_startup_walk_matches_original_and_expected_order(self):
        for verbs, wrapper in ((missions.ARC_VERBS, missions.arc_calls50),
                               (missions.BARRIER_VERBS, missions.barrier_calls50)):
            with self.subTest(verbs=verbs):
                positive, negative = verbs
                callback = proto_calls([(positive, ['callback-must-not-run'])])
                entry = proto_calls([(positive, ['first', 7.0, 'second']),
                                     ('helper', []), ('helper', []),
                                     ('helper', [1.0]),  # arguments forbid following
                                     ('OnObjectKillName', [callback, 'name']),
                                     ('chain1', []), (negative, ['last'])])
                items = [('ScriptPostLoad', entry),
                         ('helper', proto_calls([(negative, ['nested'])])),
                         ('unused', callback)]
                for depth in range(1, 6):
                    items.append((f'chain{depth}', proto_calls([
                        (positive, [f'depth{depth}']), (f'chain{depth + 1}', [])])))
                f = globals_proto(items)
                expected = [('first', True), ('second', True), ('nested', False),
                            ('nested', False)] + [(f'depth{i}', True) for i in range(1, 5)] + [('last', False)]
                self.assertEqual(wrapper(f), expected)
                self.assertEqual(wrapper(f), original_startup(f, verbs))

    def test_discovery_preserves_register_pop_and_overwrite(self):
        first, second = missions.Proto(), missions.Proto()
        f = globals_proto([('same', first), ('same', second)])
        f.k += ['alias', 9.0]
        f.code += [ins('SETGLOBAL', bx=2),  # register was consumed
                   ins('CLOSURE', a=1, bx=0), ins('SETGLOBAL', a=1, bx=3),
                   ins('SETGLOBAL', a=1, bx=2)]  # nonstring does not consume it
        self.assertEqual(missions.global_functions50(f), {'same': second, 'alias': first})

    def test_recursive_startup_stops_at_existing_depth(self):
        entry = proto_calls([(missions.ARC_VERBS[0], ['loop']), ('ScriptPostLoad', [])])
        f = globals_proto([('ScriptPostLoad', entry)])
        self.assertEqual(missions.arc_calls50(f), [('loop', True)] * 5)
        self.assertEqual(missions.arc_calls50(f), original_startup(f, missions.ARC_VERBS))

    def test_death_drains_stay_direct_and_branch_free(self):
        handler = proto_calls([('AddReinforcements', [2.0, -3.0])])
        branchy = proto_calls([('AddReinforcements', [2.0, -4.0])])
        branchy.code.append(ins('TEST'))
        extra = proto_calls([('AddReinforcements', [2.0, -5.0]), ('Other', [])])
        indirect = proto_calls([('OnCharacterDeathTeam', [handler, 8.0])])
        entry = proto_calls([('OnCharacterDeathTeam', [handler, 1.0]),
                             ('OnCharacterDeathTeam', [branchy, 1.0]),
                             ('OnCharacterDeathTeam', [extra, 1.0]), ('helper', [])])
        f = globals_proto([('ScriptPostLoad', entry), ('helper', indirect)])
        self.assertEqual(missions.death_drains50(f), [(1, 2, 3)])
        self.assertEqual(missions.death_drains50(globals_proto([])), [])

    def arithmetic(self, op, x, y):
        p = missions.Proto()
        p.k = ['Result', x, y]
        p.code = [ins('GETGLOBAL', bx=0), ins(op, a=1, b=129, c=130),
                  ins('CALL', b=2, c=1)]
        return missions.calls50(p, recurse=False)[0].args[0]

    def test_only_selected_arithmetic_is_evaluated(self):
        self.assertEqual(self.arithmetic('ADD', 1e20, 20.0), 1e20)
        for op, expected in [('ADD', 10.0), ('SUB', 6.0), ('MUL', 16.0),
                             ('DIV', 4.0), ('POW', 64.0)]:
            with self.subTest(op=op):
                self.assertEqual(self.arithmetic(op, 8.0, 2.0), expected)
        self.assertEqual(self.arithmetic('DIV', 8.0, 0.0), 0.0)
        self.assertIsInstance(self.arithmetic('POW', 1e20, 20.0), missions.Unknown)
        self.assertIsInstance(self.arithmetic('ADD', 'unknown', 2.0), missions.Unknown)


class LuaHeaderTests(unittest.TestCase):
    @staticmethod
    def header(version, width):
        fields = [1, 4, 4, 4, 32, 9, 6] if version == 0x40 else [1, 4, 4, 4, 6, 8, 9, 9]
        return b'\x1bLua' + bytes([version] + fields + [width]) + struct.pack('<f' if width == 4 else '<d', 123.5)

    def test_all_truncated_prefixes_and_complete_widths(self):
        for version in (0x40, 0x50):
            for width in (4, 8):
                body = self.header(version, width)
                for length in range(4, len(body)):
                    with self.subTest(version=version, width=width, length=length):
                        self.assertEqual(lua.parse_lua_header(body[:length])['error'], 'truncated header')
                self.assertEqual(lua.parse_lua_header(body), dict(
                    version='4.0' if version == 0x40 else '5.0', endian=1,
                    sizeof_int=4, sizeof_size_t=4, sizeof_instr=4,
                    sizeof_number=width, test_number=123.5))
        self.assertIsNone(lua.parse_lua_header(b'print("hello")'))

    def test_cli_continues_after_malformed_script_and_keeps_bytecode_extension(self):
        def chunk(tag, body):
            return tag + struct.pack('<I', len(body)) + body + b'\0' * (-len(body) % 4)
        def script(name, body):
            return chunk(b'scr_', chunk(b'NAME', name + b'\0') + chunk(b'BODY', body))
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / 'fixture.lvl'
            source.write_bytes(chunk(b'ucfb', script(b'bad', self.header(0x40, 4)[:14]) +
                                     script(b'good', self.header(0x50, 8))))
            result = subprocess.run([sys.executable, lua.__file__, str(source), '--out', str(root / 'out')],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('malformed Lua 4.0 bytecode: truncated header', result.stdout)
            self.assertIn('Lua 5.0 bytecode', result.stdout)
            self.assertEqual((root / 'out/bad.luac').read_bytes(), self.header(0x40, 4)[:14])
            self.assertTrue((root / 'out/good.luac').is_file())


class BotTests(unittest.TestCase):
    @staticmethod
    def sample(t, **fields):
        return dict(t=t, p=(t * 2, 0.0, 0.0), team=fields.get('team', 0),
                    seated=fields.get('seated', False), fight=fields.get('fight', False),
                    post=fields.get('post', 0), steer=fields.get('steer', 'walk'),
                    slide=fields.get('slide', 0), beh=fields.get('beh', 'travel'))

    def test_exact_observation_uses_all_categorical_fields(self):
        fields = dict(team=1, seated=True, fight=True, post=1, steer='slide', slide=-1, beh='fight')
        life = [self.sample(0), self.sample(.25, **fields), self.sample(.5, **fields)]
        samples = dict(bot.resample(life, 0, 1)[0])
        self.assertEqual(samples[1], life[1])
        self.assertEqual(samples[2], life[2])
        self.assertEqual(bot.measure([life])['seated_pct'], 200 / 3)

    def test_between_observations_keeps_preceding_state(self):
        life = [self.sample(0), self.sample(.25 + 1e-6, seated=True, post=3), self.sample(.5)]
        samples = dict(bot.resample(life, 0, 1)[0])
        self.assertFalse(samples[1]['seated'])
        self.assertEqual(samples[1]['post'], 0)
        self.assertEqual(samples[1]['p'], (.5, 0.0, 0.0))

    def test_unused_hold_has_no_effect_on_measurements(self):
        life = [self.sample(i / 4) for i in range(121)]
        with_hold = [dict(sample, hold=i % 2) for i, sample in enumerate(life)]
        a, b = bot.measure([life]), bot.measure([with_hold])
        self.assertEqual(a.keys(), b.keys())
        for key in a:
            with self.subTest(key=key):
                self.assertTrue(a[key] == b[key] or math.isnan(a[key]) and math.isnan(b[key]))

    def test_port_csv_still_accepts_exported_hold_column(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'samples.csv'
            row = dict(human='0', unit='1', gen='0', t='0', x='1', y='2', z='3', team='0',
                       veh='-1', target='-1', cp='0', steer='walk', slide='0', hold='1', beh='travel')
            with path.open('w', newline='') as out:
                writer = csv.DictWriter(out, fieldnames=list(row))
                writer.writeheader()
                writer.writerow(row)
            sample = bot.load_port(path)[0][0]
            self.assertNotIn('hold', sample)
            self.assertEqual(sample['p'], (1.0, 2.0, 3.0))


class PackTests(unittest.TestCase):
    def test_sources_distinguish_reads_cache_optional_and_unknown(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'scripts').mkdir()
            (root / 'iso').mkdir()
            images = {g: root / 'iso' / (g + '.iso') for g in ('bf1', 'bf2')}
            for image in images.values():
                image.touch()
            unknown = root / 'iso/unknown.bin'
            unknown.write_bytes(b'unsupported')
            baked, missions = root / 'baked', root / 'missions'
            baked.mkdir()
            missions.mkdir()
            plan = dict(games={}, found=[dict(path=str(unknown), kind='unknown', status='refused', reason='not a disc')])
            report = pack.source_report(root, baked, missions, images, plan, ['bf1'], ['bf1'])
            rows = {r['label']: r for r in report['inputs']}
            self.assertEqual(rows['bf1 requested PS2 image']['status'], 'used')
            self.assertEqual(rows['bf2 requested PS2 image']['status'], 'cached')
            self.assertEqual(rows['bf2 extracted PS2 data']['status'], 'cached')
            self.assertEqual(rows['found: unknown']['status'], 'refused')
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                path = pack.write_source_report(root / 'game', report)
            self.assertEqual(__import__('json').loads(Path(path).read_text()), report)
            self.assertFalse(Path(path + '.new').exists())
            self.assertIn('not a disc', output.getvalue())

    def test_pack_records_successful_steps_and_writes_beside_output(self):
        for rebake in (False, True):
            with self.subTest(rebake=rebake), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                engine = root / 'engine'
                baked, missions = engine / 'assets_baked', engine / 'missions'
                (baked / 'classes').mkdir(parents=True)
                (baked / 'classes/bf1_all.ron').touch()
                missions.mkdir()
                (missions / 'one.ron').touch()
                (root / 'scripts').mkdir()
                (root / 'scripts/rebake.sh').write_text('BF2_MAPS="MUS/MUS1"\n')
                for game, lvls in pack.LUA.items():
                    lvl = root / 'extracted' / game / 'lvl/DATA/_LVL_PS2'
                    lvl.mkdir(parents=True)
                    (lvl / 'CORE.LVL').touch()
                    for sub, library in lvls.items():
                        (lvl / library).parent.mkdir(parents=True, exist_ok=True)
                        (lvl / library).touch()
                        lua = root / 'extracted' / game / 'lua' / sub
                        lua.mkdir(parents=True)
                        (lua / 'one.lua').touch()
                target = engine / 'target'
                (target / 'release').mkdir(parents=True)
                (target / 'release' / ('game' + pack.EXE)).write_bytes(b'binary')
                out = root / 'dist/game'
                def run(command, **kwargs):
                    if '--write-pack' in command:
                        Path(command[2]).write_bytes(b'packed')
                values = dict(ROOT=str(root), ENGINE=str(engine), EXTRACTED=str(root / 'extracted'),
                              BAKED=str(baked), MISSIONS=str(missions), TARGET=str(target),
                              BAKE_STAMP=str(root / 'extracted/bake.sha'),
                              MISSION_STAMP=str(root / 'extracted/mission.sha'))
                args = ['pack.py', '--out', str(out)] + (['--rebake'] if rebake else [])
                with mock.patch.multiple(pack, **values), mock.patch.object(sys, 'argv', args), \
                        mock.patch.object(pack, 'need'), mock.patch.object(pack, 'run', side_effect=run), \
                        mock.patch.object(pack, 'find_bash', return_value='bash'), \
                        mock.patch.object(pack, 'stale', return_value=None), \
                        mock.patch.object(pack, 'inputs_hash', return_value='hash'), \
                        mock.patch.object(pack, 'missing_music', return_value=[]), \
                        mock.patch.object(pack.shutil, 'which', return_value=None), \
                        mock.patch.object(pack.subprocess, 'run', return_value=mock.Mock(stdout='tracked', returncode=0)), \
                        contextlib.redirect_stdout(io.StringIO()):
                    pack.main()
                report = __import__('json').loads(Path(str(out) + '.sources.json').read_text())
                extracted = [r for r in report['inputs'] if r['label'].endswith('extracted PS2 data')]
                # Read every run: their bytes key the console Lua cache.
                self.assertEqual({r['status'] for r in extracted}, {'used'})
                self.assertTrue(out.is_file())

    def test_map_sources_preserve_base_and_alternative_file_spelling(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            texture_dir = root / 'map/textures'
            texture_dir.mkdir(parents=True)
            (root / 'map/manifest.ron').write_text(
                '(source_game: "bf2", textures: ["Foo"], '
                'texture_tiers: [("foo", "pc")], '
                'texture_tier2: [("foo", "Foo.tier2.ktx2")])')
            (texture_dir / 'Foo.ktx2').touch()
            (texture_dir / 'Foo.tier2.ktx2').touch()
            row, = pack.map_texture_sources(root)
            self.assertEqual(row['base_textures'], 1)
            self.assertEqual(row['missing_base_textures'], [])
            self.assertEqual(row['pc_alternatives'], ['foo'])
            self.assertEqual(row['missing_alternatives'], [])
            self.assertEqual(row['missing_alternative_metadata'], [])

    def test_map_sources_require_explicit_alternative_file_metadata(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            texture_dir = root / 'map/textures'
            texture_dir.mkdir(parents=True)
            manifest = root / 'map/manifest.ron'
            # Even an existing conventionally named file is not evidence
            # without the producer's filename metadata.
            (texture_dir / 'foo.tier2.ktx2').touch()
            header = '(source_game: "bf2", textures: [], texture_tiers: [("foo", "pc")], '
            manifest.write_text(header + ')')
            row, = pack.map_texture_sources(root)
            self.assertEqual(row['pc_alternatives'], [])
            self.assertEqual(row['missing_alternative_metadata'], ['foo'])
            self.assertEqual(row['missing_alternatives'], [])
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                pack.write_source_report(root / 'game', dict(inputs=[], maps=[row], limits='fixture'))
            self.assertIn("missing alternative filename metadata: ['foo']", output.getvalue())
            (texture_dir / 'Other.ktx2').touch()
            manifest.write_text(header + 'texture_tier2: [("other", "Other.ktx2")])')
            row, = pack.map_texture_sources(root)
            self.assertEqual(row['unrecognized_alternative_metadata'], ['other'])
            self.assertEqual(row['missing_alternative_metadata'], ['foo'])
            self.assertEqual(row['pc_alternatives'], [])
            manifest.write_text(header + 'texture_tier2: [("foo", "Absent.ktx2")])')
            row, = pack.map_texture_sources(root)
            self.assertEqual(row['missing_alternatives'], ['foo'])
            self.assertEqual(row['missing_alternative_metadata'], [])
            for mapping in ['[("foo", 3)]', '[("foo", "one"), ("foo", "two")]',
                            '[("foo", "")]', '[("foo", "../outside.ktx2")]',
                            '[("foo", "/outside.ktx2")]', '[("foo", "unfinished")']:
                with self.subTest(mapping=mapping):
                    manifest.write_text(header + 'texture_tier2: ' + mapping + ')')
                    self.assertIn('error', pack.map_texture_sources(root)[0])

    def test_map_sources_count_existing_files_without_inventing_tiers(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            texture_dir = root / 'map/textures'
            texture_dir.mkdir(parents=True)
            manifest = root / 'map/manifest.ron'
            manifest.write_text('(source_game: "bf2", textures: ["lava", "missing", "bracket]"], '
                                'texture_tiers: [("lava", "pc"), ("missing", "pc"), ("odd", "unknown"), '
                                '("bracket]", "xbox")], '
                                'texture_tier2: [("lava", "lava.tier2.ktx2"), ("missing", "missing.tier2.ktx2"), '
                                '("odd", "odd.tier2.ktx2"), ("bracket]", "bracket].tier2.ktx2")])')
            (texture_dir / 'lava.ktx2').touch()
            (texture_dir / 'bracket].ktx2').touch()
            (texture_dir / 'lava.tier2.ktx2').touch()
            (texture_dir / 'odd.tier2.ktx2').touch()
            (texture_dir / 'bracket].tier2.ktx2').touch()
            row, = pack.map_texture_sources(root)
            self.assertEqual(row['base_textures'], 2)
            self.assertEqual(row['missing_base_textures'], ['missing'])
            self.assertEqual(row['pc_alternatives'], ['lava'])
            self.assertEqual(row['xbox_alternatives'], ['bracket]'])
            self.assertEqual(row['missing_alternatives'], ['missing'])
            self.assertEqual(row['unrecognized_tiers'], ['unknown'])
            manifest.write_text('(source_game: "unrecognized", textures: [])')
            self.assertIn('error', pack.map_texture_sources(root)[0])
            manifest.write_text('(source_game: "bf1", textures: [broken])')
            self.assertIn('error', pack.map_texture_sources(root)[0])
            manifest.write_text('(source_game: "bf1", textures: ["unfinished"')
            self.assertIn('error', pack.map_texture_sources(root)[0])

    def test_shipping_inputs_invalidate_hash_and_fold_line_endings(self):
        added = ('engine/crates/bake/src/effect_names.txt', 'engine/Cargo.toml', 'engine/Cargo.lock')
        with tempfile.TemporaryDirectory() as temp, mock.patch.object(pack, 'ROOT', temp):
            root = Path(temp)
            contents = dict.fromkeys(added, b'line one\nline two\n')
            contents.update({'engine/crates/bake/src/main.rs': b'fn main() {}\n',
                             'engine/crates/manifest/src/lib.rs': b'// manifest\n',
                             'engine/crates/mungefs/src/lib.rs': b'// munge\n',
                             'scripts/rebake.sh': b'# rebake\n'})
            for name, data in contents.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            original = pack.inputs_hash(pack.BAKE_INPUTS)
            self.assertEqual(original, pack.inputs_hash(pack.BAKE_INPUTS))
            for name in added:
                with self.subTest(input=name):
                    path = root / name
                    path.write_bytes(contents[name] + b'changed\n')
                    self.assertNotEqual(original, pack.inputs_hash(pack.BAKE_INPUTS))
                    path.write_bytes(contents[name].replace(b'\n', b'\r\n'))
                    self.assertEqual(original, pack.inputs_hash(pack.BAKE_INPUTS))
                    path.write_bytes(contents[name])
            # Recreate files in reverse order; discovery must not depend on creation order.
            for name in contents:
                (root / name).unlink()
            for name, data in reversed(list(contents.items())):
                (root / name).write_bytes(data)
            self.assertEqual(original, pack.inputs_hash(pack.BAKE_INPUTS))


class AnalysisIndexTests(unittest.TestCase):
    def test_check_prints_exact_written_index_without_editing_it(self):
        script=Path(__file__).resolve().parents[1] / 'analysis_index.py'
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'analysis').mkdir()
            (root/'analysis/re-fixture.md').write_text('# Fixture\n\nSummary.\n')
            command=[sys.executable,str(script),'--root',str(root)]
            subprocess.run(command,check=True,capture_output=True)
            path=root/'analysis/INDEX.md';before=path.read_bytes()
            result=subprocess.run(command+['--check'],check=True,capture_output=True)
            self.assertEqual(result.stdout,before)
            self.assertEqual(path.read_bytes(),before)


    def test_other_notes_never_show_private_text(self):
        script=Path(__file__).resolve().parents[1] / 'analysis_index.py'
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'analysis/plan').mkdir(parents=True)
            (root/'analysis/re-fixture.md').write_text('# Fixture\n\nSummary.\n')
            (root/'analysis/public-note.md').write_text('# Public title\n\nBody.\n')
            (root/'analysis/plan/mixed.md').write_text('\n# Shown heading\n')
            (root/'analysis/all-private.md').write_text('\n')
            subprocess.run([sys.executable,str(script),'--root',str(root)],check=True,capture_output=True)
            index=(root/'analysis/INDEX.md').read_text()
            self.assertIn('[public-note.md](public-note.md)',index)
            self.assertIn('Public title',index)
            self.assertIn('[plan/mixed.md](plan/mixed.md)',index)
            self.assertIn('Shown heading',index)
            self.assertNotIn('Hidden heading',index)
            self.assertNotIn('Secret ledger',index)
            self.assertNotIn('all-private.md',index)
            self.assertEqual(index.count('re-fixture.md'),2)  # its one row: link text and target, nothing under Other notes


class ScannerProjectionTests(unittest.TestCase):
    def test_callgraph_synthetic_markers(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'fixture.c'
            path.write_text('/* ==== 00001000 Start ==== */\nFUN_00002000();\n'
                            'use(FUN_00003000);\nuse(0x3000);\n'
                            '/* ==== 00002000 FUN_00002000 ==== */\nreturn;\n')
            funcs, calls, takes, literals = callgraph.parse_decomp(path, {}, {})
            self.assertEqual(funcs, {'00001000': 3, '00002000': 1})
            self.assertEqual(dict(calls), {'00001000': {'00002000'}})
            self.assertEqual(dict(takes), {'00001000': {'00003000'}})
            self.assertEqual(dict(literals), {})
            self.assertEqual(callgraph.bfs({'00001000'}, calls, takes),
                             {'00001000', '00002000', '00003000'})

    def test_scanner_preserves_table_order_and_duplicate_filter(self):
        names = b'Alpha\0Beta\0Gamma\0Delta\0'
        pointers = [0x3000, 0x3006, 0x300b, 0x3011]
        data = b''.join(struct.pack('<II', ptr, fun) for ptr, fun in
                        zip(pointers, [0x1000, 0x1004, 0x1000, 0x100c]))
        sections = {'.text': dict(addr=0x1000, size=16),
                    '.data': dict(addr=0x2000, size=len(data)),
                    '.rodata': dict(addr=0x3000, size=len(names))}
        out, err = io.StringIO(), io.StringIO()
        with mock.patch('builtins.open', mock.mock_open(read_data=b'')), \
                mock.patch.object(scan_tables, 'sections', return_value=sections), \
                mock.patch.object(scan_tables, 'load', side_effect=[data, names]), \
                contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            scan_tables.main('synthetic')
        self.assertEqual(out.getvalue(), '00001000 Alpha\n00001004 Beta\n0000100c Delta\n')
        self.assertEqual(err.getvalue(), '# tables: 4 entries, 3 unique functions\n')


if __name__ == '__main__':
    unittest.main()
