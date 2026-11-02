"""Discriminating census format, omission and comparison tests."""
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import version_census as v


def chunk(tag, payload):
    return tag + struct.pack('<I', len(payload)) + payload + bytes(-len(payload) % 4)


class CensusTests(unittest.TestCase):
    def test_multiple_sublevels_and_direct_resource_are_not_dropped(self):
        resource = chunk(b'entc', chunk(b'TYPE', b'unit\0'))
        data = chunk(b'ucfb', chunk(b'lvl_', chunk(struct.pack('<I', 31), resource) + chunk(struct.pack('<I', 42), resource) + resource))
        rows = list(v.walk_resources(data))
        self.assertEqual([x[0] for x in rows], [[31], [42], []])
        self.assertEqual([x[1] for x in rows], [b'entc'] * 3)

    def test_addon_mission_tree_does_not_hide_the_base_install(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for path in ("GameData/Data/_LVL_PC/mission.lvl", "GameData/AddOn/tat3/Data/_lvl_pc/mission.lvl"):
                file = root / path
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b"base" if "AddOn" not in path else b"addon")
            rows = list(v.input_files(root))
            self.assertEqual(rows, [("mission.lvl", b"base")])

    def test_binary_dynamic_planning_is_a_resource_inside_a_sublevel(self):
        data = chunk(b"ucfb", chunk(b"lvl_", chunk(struct.pack("<I", 31), chunk(b"plnp", b"\x01\x02\x03\x04"))))
        rows = list(v.walk_resources(data))
        self.assertEqual([(row[0], row[1], row[2]) for row in rows], [([31], b"plnp", b"\x01\x02\x03\x04")])

    def test_truncated_size_rejected(self):
        with self.assertRaises(ValueError):
            list(v.iter_chunks(b'ucfb' + struct.pack('<I', 100) + b'abc'))

    def test_values_exclude_free_text_but_preserve_numbers_and_names(self):
        self.assertEqual(v.safe_value('localized sentence is forbidden'), 'NM')
        self.assertEqual(v.safe_value('1.25, -2, 3e2'), [1.25, -2, 300])
        self.assertEqual(v.safe_value('all_inf_rifleman'), 'all_inf_rifleman')
        self.assertEqual(v.safe_value(float('nan')), 'NM')

    def test_a_one_word_label_is_text_not_a_name(self):
        label = v.extract_lua.fnv1a("Label")
        self.assertEqual(v.property_value(label, "Stormtrooper"), "NM")
        self.assertEqual(v.property_value(v.extract_lua.fnv1a("GeometryName"), "imp_inf_trooper"), "imp_inf_trooper")

    def test_repeated_properties_and_missing_are_separate(self):
        row = dict(file='side/a.lvl', scope=[10], kind='entc', name='a', ordinal=0, base='soldier', properties=[[1, 2], [1, 3], [9, 'NM']])
        a = dict(source=dict(game='bf1'), entities=[row], missions=[], content=[])
        row2 = {**row, 'properties': [[1, 2], [1, 7], [2, 8], [9, 'NM']]}
        b = {**a, 'entities': [row2]}
        result = v.compare(a, b)
        self.assertEqual(result['properties']['compared'], 2)
        self.assertEqual(result['properties']['changed'], 1)
        self.assertEqual(result['properties']['unmeasured'], 1)
        self.assertEqual(result['properties']['right_only'], 1)
        self.assertEqual(result['properties']['differences'][0]['field'], [1, 1])

    def test_mission_dynamic_unknown_not_equality(self):
        self.assertEqual(v.safe_value(v.gen_missions.Unknown('dynamic')), 'NM')
        with self.assertRaises(ValueError):
            v.mission_facts(b'not a Lua dump')

    def test_typed_property_tokens_and_resource_spaces(self):
        self.assertEqual(v.property_value(v.extract_lua.fnv1a("ControlSpeed"), "stand 1 .7 .2"), ["stand", 1, .7, .2])
        self.assertEqual(v.property_value(v.extract_lua.fnv1a("Label"), "Control Zone"), "NM")
        self.assertEqual(v.resource_name(b"construction create\0"), "construction create")
        self.assertEqual(v.safe_value(""), "")

    def test_mission_branch_and_missing_defaults_are_explicit(self):
        root, child = v.gen_missions.Proto(), v.gen_missions.Proto()
        root.protos, root.kstr = [child], ["ScriptInit"]
        root.code = [v.gen_missions.OPS40.index("CLOSURE"), v.gen_missions.OPS40.index("SETGLOBAL")]
        header = dict(version="4.0", endian=1, sizeof_int=4, sizeof_size_t=4, sizeof_instr=4, sizeof_number=4)
        calls = [v.gen_missions.Call("SetUnitCount", [1., 20.], child), v.gen_missions.Call("AddUnitClass", [1., "unit"], child), v.gen_missions.Call("SetNumBirdTypes", [8.], child)]
        with patch.object(v.extract_lua, "parse_lua_header", return_value=header), patch.object(v.gen_missions, "undump40", return_value=root), patch.object(v.gen_missions, "calls40", return_value=calls):
            facts, _, _, static = v.mission_facts(b"")
            self.assertEqual(facts["units"], {"1": 20})
            self.assertEqual(facts["classes"]["1"][0][1], "NM")
            self.assertEqual(facts["bird_count"], 8)
            self.assertEqual(static[0]["function"], "ScriptInit")
            child.code = [v.gen_missions.OPS40.index("JMP")]
            facts, _, _, static = v.mission_facts(b"")
            self.assertEqual(facts["units"], "NM")
            self.assertEqual(static[0]["control_flow"], "NM")

    def test_dormant_prototype_does_not_change_startup_global(self):
        def ins(op, a=0, b=0, c=0, bx=None):
            return v.gen_missions.OPS50.index(op) | (a << 24) | ((bx << 6) if bx is not None else (b << 15) | (c << 6))
        def proto(constants, code, children=()):
            p = v.gen_missions.Proto()
            p.k, p.code, p.protos = constants, code, list(children)
            return p
        dormant = proto(["TEAM", 2.], [ins("LOADK", bx=1), ins("SETGLOBAL", bx=0)])
        init = proto(["SetUnitCount", "TEAM", 20.], [ins("GETGLOBAL", bx=0), ins("GETGLOBAL", a=1, bx=1), ins("LOADK", a=2, bx=2), ins("CALL", b=3, c=1)])
        root = proto(["TEAM", 1., "Dormant", "ScriptInit"], [ins("LOADK", bx=1), ins("SETGLOBAL", bx=0), ins("CLOSURE", bx=0), ins("SETGLOBAL", bx=2), ins("CLOSURE", bx=1), ins("SETGLOBAL", bx=3)], [dormant, init])
        header = dict(version="5.0", endian=1, sizeof_int=4, sizeof_size_t=4, sizeof_instr=4, sizeof_number=4)
        with patch.object(v.extract_lua, "parse_lua_header", return_value=header), patch.object(v.gen_missions, "undump50", return_value=root):
            facts, _, _, _ = v.mission_facts(b"")
            self.assertEqual(facts["units"], {"1": 20})

    def test_vm_snapshot_isolates_nested_tables_preserving_aliases(self):
        table = v.gen_missions.Table({"team": 1})
        opaque = v.gen_missions.Closure(v.gen_missions.Proto())
        globals_copy, ups_copy = v.gen_missions._snapshot_tables([{"table": table, "closure": opaque}, [table]])
        self.assertIs(globals_copy["table"], ups_copy[0])
        self.assertIs(globals_copy["closure"], opaque)
        ups_copy[0]["team"] = 2
        self.assertEqual(table["team"], 1)

    def test_unknown_subtree_is_not_a_missing_platform_value(self):
        result = v._value_summary([((), {("units",): "NM"}, {("units", "1"): 32})])
        self.assertEqual((result["compared"], result["left_only"], result["right_only"], result["unmeasured"]), (0, 0, 0, 1))

    def test_unknown_disc_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'misleading.iso'
            path.write_bytes(bytes(40 * 2048))
            with self.assertRaisesRegex(ValueError, 'unknown disc format'):
                list(v.disc_entries(path))

    def test_cyclic_iso_and_truncated_extent_rejected(self):
        def record(name, lba, size):
            data = bytearray(34 + len(name))
            data[0] = len(data)
            struct.pack_into('<I', data, 2, lba)
            struct.pack_into('<I', data, 10, size)
            data[25] = 2
            data[32] = len(name)
            data[33:33 + len(name)] = name
            return data
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'disc'
            data = bytearray(40 * 2048)
            data[16*2048:16*2048+7] = b'\x01CD001\x01'
            struct.pack_into('<I', data, 16*2048+158, 20)
            struct.pack_into('<I', data, 16*2048+166, 2048)
            child = record(b'cycle', 20, 2048)
            data[20*2048:20*2048+len(child)] = child
            path.write_bytes(data)
            with self.assertRaisesRegex(ValueError, 'cyclic ISO9660'):
                list(v.disc_entries(path))
            struct.pack_into('<I', data, 16*2048+158, 50)
            path.write_bytes(data)
            with self.assertRaisesRegex(ValueError, 'truncated disc extent'):
                list(v.disc_entries(path))

    def test_unknown_install_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, 'no ucfb LVLs'):
                v.scan(Path(directory))

    def test_iso_directory_extent_and_case_normalization(self):
        self.assertEqual(v.canonical_path('DATA/_LVL_PS2/SIDE/ALL.LVL'), 'side/all.lvl')
        self.assertEqual(v.canonical_path('data/_lvl_pc/SIDE/ALL.lvl'), 'side/all.lvl')


if __name__ == '__main__':
    unittest.main()
