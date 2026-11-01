"""Reference export keeps context/order and refuses unsafe or non-PS2 input."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gameplay_table as table


def census():
    return {"schema": 1, "source": {"game": "bf1", "platform": "ps2"},
            "entities": [{"file": "side/all.lvl", "scope": [12], "ordinal": 0,
                          "kind": "entc", "name": "soldier", "base": "unit",
                          "properties": [[34, 1], [34, 2], [56, "NM"]]}],
            "missions": [{"file": "mission.lvl", "scope": [78], "name": "tat1a",
                          "facts": {"units": {"1": 16}}}]}


class TableTests(unittest.TestCase):
    def test_repeated_properties_and_local_context_survive(self):
        self.assertEqual(table.scalar("all\\allwook;all_1st_wookie"),
                         "all\\allwook;all_1st_wookie")
        source = census()
        other = copy.deepcopy(source["entities"][0])
        other["scope"] = [13]
        other["properties"][0][1] = 99
        source["entities"].append(other)
        game, files, omitted, dropped = table.convert(source)
        self.assertEqual(game, "bf1")
        self.assertEqual(omitted, 2)
        self.assertEqual(dropped, 0)
        rows = files["side/all.lvl"]["classes"]
        self.assertEqual(rows[0]["properties"], [[34, 1], [34, 2], [56, "NM"]])
        self.assertEqual(rows[1]["scope"], [13])
        self.assertEqual(rows[1]["properties"][0][1], 99)

    def test_presentation_and_display_text_never_reach_the_table(self):
        source = census()
        label, sound, health = (table.extract_lua.fnv1a(n) for n in ("Label", "VOSound", "MaxHealth"))
        source["entities"][0]["properties"] = [[label, "Stormtrooper"], [sound, "imp_inf_chatter"], [health, 300]]
        _, files, omitted, dropped = table.convert(source)
        self.assertEqual(files["side/all.lvl"]["classes"][0]["properties"], [[health, 300]])
        self.assertEqual((omitted, dropped), (0, 2))

    def test_a_class_copied_across_lvls_is_one_record_with_two_definitions(self):
        source = census()
        copy_in_map = copy.deepcopy(source["entities"][0])
        copy_in_map["file"], copy_in_map["scope"] = "tat/tat1.lvl", [99]
        changed = copy.deepcopy(copy_in_map)
        changed["ordinal"], changed["properties"] = 1, [[34, 7]]
        source["entities"] += [copy_in_map, changed]
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            (root / "census.json").write_text(json.dumps(source))
            table.generate([root / "census.json"], root / "out")
            document = json.loads((root / "out" / "bf1.json").read_text())
        self.assertEqual(len(document["class_pool"]), 2)
        self.assertEqual(document["files"]["side/all.lvl"]["classes"], [[[12], 0, 0]])
        self.assertEqual(document["files"]["tat/tat1.lvl"]["classes"], [[[99], 0, 0], [[99], 1, 1]])

    def test_committed_table_holds_only_names_and_numbers(self):
        root = Path(__file__).resolve().parents[2] / "engine/crates/manifest/data/gameplay/ps2-v1"
        manifest = json.loads((root / "manifest.json").read_text())
        self.assertEqual({s["table"] for s in manifest["sources"]}, {"bf1.json", "bf2.json"})
        self.assertEqual(sorted(p.name for p in root.iterdir()), ["bf1.json", "bf2.json", "manifest.json"])
        for source in manifest["sources"]:
            document = json.loads((root / source["table"]).read_text())
            self.assertEqual(len(document["class_pool"]), source["distinct_class_records"])
            definitions = sum(len(f["classes"]) for f in document["files"].values())
            self.assertEqual(definitions, source["class_definitions"])
            for record in document["class_pool"]:
                for key, value in record["properties"]:
                    self.assertNotIn(key, table.PRESENTATION)
                    table.scalar(value)  # raises on anything but a name, a number or a flat vector

    def test_rejects_non_reference_input_and_path_escape(self):
        source = census()
        source["source"]["platform"] = "pc"
        with self.assertRaisesRegex(ValueError, "PS2"):
            table.convert(source)
        for path in ("../escape.lvl", "/tmp/escape.lvl", "..\\escape.lvl"):
            source = census()
            source["entities"][0]["file"] = path
            with self.assertRaisesRegex(ValueError, "unsafe"):
                table.convert(source)

    def test_rejects_nonfinite_data(self):
        source = census()
        source["entities"][0]["properties"] = [[1, float("nan")]]
        with self.assertRaisesRegex(ValueError, "nonfinite"):
            table.convert(source)

    def test_copies_only_known_sections_and_is_reproducible(self):
        source = census()
        source["script_bytecode"] = "not part of exported data"
        source["entities"][0]["localized_text"] = "not part of exported data"
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            input_file = root / "census.json"
            input_file.write_text(json.dumps(source))
            first = table.generate([input_file], root / "one")
            second = table.generate([input_file], root / "two")
            self.assertEqual(first, second)
            for path in (root / "one").rglob("*.json"):
                data = path.read_bytes()
                self.assertNotIn(b"not part", data)
                self.assertEqual(data, (root / "two" / path.relative_to(root / "one")).read_bytes())

    def test_rejects_payloads_inside_exported_fields(self):
        mutations = [
            ("entities", "ordinal", "localized prose copied verbatim"),
            ("entities", "ordinal", True),
            ("entities", "scope", {"script_bytecode": [1, 2, 3]}),
            ("entities", "scope", [0x100000000]),
            ("entities", "scope", [-1]),
            ("entities", "kind", "modl"),
            ("entities", "properties", [[1, {"mesh": [[1, 2, 3]]}]]),
            ("entities", "properties", [[1, [[1, 2, 3]]]]),
            ("entities", "properties", [[1, "localized sentence is forbidden"]]),
            ("missions", "unresolved_arguments", {"caption": "localized prose"}),
            ("missions", "facts", {"script_bytecode": [1, 2, 3]}),
            ("missions", "facts", {"units": {"1": [1, 2, 3]}}),
            ("missions", "facts", {"classes": {"1": [["soldier", 1, {"mesh": [1]}]]}}),
            ("missions", "facts", {"ai_capture": {"post/1": 1}}),
        ]
        for category, field, value in mutations:
            with self.subTest(category=category, field=field, value=value):
                source = census()
                source[category][0][field] = value
                with self.assertRaises(ValueError):
                    table.convert(source)

    def test_preserves_typed_names_empty_values_and_flat_vectors(self):
        source = census()
        source["entities"][0]["name"] = "construction create"
        source["entities"][0]["properties"] = [[1, ""], [2, ["body", 1, -0.5]]]
        row = table.convert(source)[1]["side/all.lvl"]["classes"][0]
        self.assertEqual(row["name"], "construction create")
        self.assertEqual(row["properties"], [[1, ""], [2, ["body", 1, -0.5]]])

    def test_known_mission_shapes_preserve_unknowns(self):
        source = census()
        facts = {
            "units": {"1": 16}, "tickets": {"1": "NM"}, "ai_aggr": {"1": 0.5},
            "names": {"1": "all"}, "heroes": {"1": "hero"},
            "classes": {"1": [["soldier", 4, None], ["pilot", "NM", 2]]},
            "spawn": [10, 0.25], "bird_types": {"0": [1.0, "bird"]},
            "bird_count": 1, "attacking": 1, "fly": None, "ai_view": "NM",
            "death_regions": ["Death Region"], "posts": [], "killed": [], "layers": [],
            "cps": [["cp1", 1]], "ai_damage": [["cp1", 0.25]],
            "class_load_order": [["all", "soldier"]], "ai_capture": {"cp1/1": False},
        }
        source["missions"][0]["facts"] = facts
        row = table.convert(source)[1]["mission.lvl"]["missions"][0]
        self.assertEqual(row["facts"], facts)
        self.assertEqual(row["unresolved_arguments"], "NM")

    def test_invalid_input_never_creates_output(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            source = census()
            source["missions"][0]["ordinal"] = "bad prose"
            path = root / "invalid.json"
            path.write_text(json.dumps(source))
            with self.assertRaises(ValueError):
                table.generate([path], root / "out")
            self.assertFalse((root / "out").exists())

    def test_does_not_silently_leave_stale_output(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            input_file = root / "census.json"
            input_file.write_text(json.dumps(census()))
            output = root / "out"
            output.mkdir()
            (output / "foreign.json").write_text("{}")
            with self.assertRaisesRegex(ValueError, "stale or foreign"):
                table.generate([input_file], output)
            self.assertEqual(list(output.iterdir()), [output / "foreign.json"])


if __name__ == "__main__":
    unittest.main()
