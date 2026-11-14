#!/usr/bin/env python3
"""Generate absolute PS2 gameplay corrections for a fingerprinted donor.

No executable, bytecode or artwork enters the public JSON. Gameplay height rows are
restricted to authored playable boundary polygons and the corners of the grid cells those
polygons reach; gameplay fills cover every other differing sample of the simulation grid,
because nothing in the simulation keeps a soldier or ground vehicle inside the boundary
(issue #168: it bounds only flyers and remote droids). A map with no boundary polygon gets
no gameplay heights. Separate render-only heights cover the donor-only border of a larger
donor drawing grid, without changing the heightfield the simulation reads. Changed collision, skeleton, navigation and root motion leave as instructions
over the player's own donor file, in a sibling programs table (gameplay_programs.py).
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import math
import re
from pathlib import Path
import struct

import gameplay_programs as programs
import gameplay_table as table
import version_census as census
import version_geometry as geometry
from version_terrain_play import Grid, in_polygon

SCHEMA = 2
REGION_SETTING = re.compile(r"shadow[0-9]* ?[a-z0-9 =,.]*")
# The BF1 console common turret's stop cue includes a loader qualifier. PC
# omits this whole property, so retaining only donor hashes loses the cue.
# Keep this measured token pair, without treating arbitrary prose as data.
KNOWN_PROPERTY_TOKENS = {0xc13068b6: {"turret_whir_end defer": ["turret_whir_end", "defer"]}}


def compact_numbers(value):
    """Nine decimal digits preserve a decoded f32 exactly without f64 padding."""
    if isinstance(value, dict):
        return {key: compact_numbers(child) for key, child in value.items()}
    if isinstance(value, (list, tuple)):
        return [compact_numbers(child) for child in value]
    if type(value) is float and math.isfinite(value):
        try:
            if geometry.f32(value) == value:
                shorter = float(format(value, ".9g"))
                if geometry.f32(shorter) == value:
                    return shorter
        except OverflowError:
            pass
    return value


def encoded(value):
    return (json.dumps(compact_numbers(value), separators=(",", ":"), allow_nan=False) + "\n").encode()


def digest(value):
    return hashlib.sha256(value).hexdigest()


def props(payload):
    """Strictly typed ODF/world names and numbers, preserving exact decimal spelling."""
    out = []
    for tag, value in geometry.chunks(payload):
        if tag != b"PROP":
            continue
        if len(value) < 5:
            raise ValueError("truncated PROP")
        key = struct.unpack_from("<I", value)[0]
        if key in table.PRESENTATION:
            continue
        raw = census.cstr(value[4:])
        parsed = KNOWN_PROPERTY_TOKENS.get(key, {}).get(raw, census.property_value(key, raw))
        if parsed == "NM":
            # Unknown/free-text properties cannot be exported. The caller keeps
            # an explicit exclusion so this does not become an invented zero.
            continue
        out.append([key, parsed if isinstance(parsed, list) else raw])
    return out


def config_nodes(payload):
    """Decode config arguments, not the opaque DATA string blob."""
    out = []
    for tag, value in geometry.chunks(payload):
        if tag == b"DATA":
            if len(value) < 9:
                raise ValueError("short config DATA")
            key, count = struct.unpack_from("<IB", value)
            end = 5 + count * 4
            if end + 4 > len(value):
                raise ValueError("short config arguments")
            words = struct.unpack_from("<" + "I" * count, value, 5)
            size = struct.unpack_from("<I", value, end)[0]
            blob = value[end + 4:end + 4 + size]
            if len(blob) != size:
                raise ValueError("short config string blob")
            args = []
            for word in words:
                offset = word - count * 4
                if 0 <= offset < len(blob):
                    raw = census.cstr(blob[offset:])
                    table.name(raw, spaces=True)
                    args.append(raw)
                else:
                    number = struct.unpack("<f", struct.pack("<I", word))[0]
                    if not math.isfinite(number):
                        raise ValueError("nonfinite config argument")
                    args.append(number)
            out.append({"hash": key, "arguments": args, "children": []})
        elif tag == b"SCOP":
            if not out:
                raise ValueError("config scope without DATA")
            out[-1]["children"] = config_nodes(value)
    return out


def boundaries(resources, game):
    polygons = []
    for _loc, tag, payload in resources:
        if tag == b"PATH":
            records = [p for t, p in geometry.chunks(payload) if t == b"path"]
        elif tag == b"path":
            records = [payload]
        else:
            continue
        for record in records:
            raw_name = geometry.child(record, b"NAME")
            if raw_name is not None and geometry.child(record, b"DATA") is None:
                if not geometry.name(raw_name).startswith("boundary"):
                    continue
                points = geometry.child(record, b"PNTS", b"")
                if len(points) % 28:
                    raise ValueError("boundary PNTS size")
                polygon = [(p[0], p[2]) for p in struct.iter_unpack("<7f", points)]
                if len(polygon) >= 3:
                    polygons.append(polygon)
            else:
                for node in config_nodes(record):
                    if node["hash"] != census.extract_lua.fnv1a("Path") or not node["arguments"]:
                        continue
                    if not str(node["arguments"][0]).lower().startswith("boundary"):
                        continue
                    polygon = []
                    for group in node["children"]:
                        if group["hash"] != census.extract_lua.fnv1a("Nodes"):
                            continue
                        for point in group["children"]:
                            for field in point["children"]:
                                if field["hash"] == census.extract_lua.fnv1a("position"):
                                    x, _y, z = field["arguments"]
                                    polygon.append((x, -z if game == "bf2" else z))
                    if len(polygon) >= 3:
                        polygons.append(polygon)
    return polygons


def on_segment(a, b, x, z):
    cross = (x - a[0]) * (b[1] - a[1]) - (z - a[1]) * (b[0] - a[0])
    return abs(cross) <= 1e-7 and min(a[0], b[0]) <= x <= max(a[0], b[0]) and min(a[1], b[1]) <= z <= max(a[1], b[1])


def in_bounds(polygons, x, z):
    return any(in_polygon(p, x, z) or any(on_segment(a, b, x, z) for a, b in zip(p, p[1:] + p[:1])) for p in polygons)


def touches_box(a, b, x, z, half):
    """Does segment a-b reach the square of half-width `half` around (x, z)? Liang-Barsky."""
    low, high = 0.0, 1.0
    for start, delta, centre in ((a[0], b[0] - a[0], x), (a[1], b[1] - a[1], z)):
        near, far = centre - half - start, centre + half - start
        if delta == 0:
            if near > 0 or far < 0:
                return False
            continue
        first, second = sorted((near / delta, far / delta))
        low, high = max(low, first), min(high, second)
        if low > high:
            return False
    return True


def supports_bounds(polygons, x, z, unit):
    """A grid sample the playable area can read: inside it, or a corner of a cell it reaches.

    Ground inside the boundary interpolates all four corners of its cell, so a
    corner just over the line still sets the height of playable ground.
    """
    return in_bounds(polygons, x, z) or any(touches_box(a, b, x, z, unit) for p in polygons for a, b in zip(p, p[1:] + p[:1]))


def water_grid(terrain):
    """The simulation reads WATR height and the row-major WMAP wet bits."""
    water = geometry.child(terrain, b"WATR")
    if water is None:
        raise ValueError("water mask correction needs a WATR chunk")
    info, layer, packed = (geometry.child(water, tag) for tag in (b"INFO", b"LAYR", b"WMAP"))
    if info is None or layer is None or packed is None or len(info) < 24:
        raise ValueError("incomplete WATR for water mask correction")
    grid, cell, half_x, half_z = struct.unpack_from("<I4x3f", info)
    _, separator, layer_info = layer.partition(b"\0")
    if not separator or len(layer_info) < 28 or not 0 < grid <= 128 or grid % 2 or len(packed) != grid * grid // 2:
        raise ValueError("invalid WATR grid for water mask correction")
    height = struct.unpack_from("<f", layer_info, 4)[0]
    if not math.isfinite(height) or not math.isfinite(cell) or not cell > 0 or any(
            not math.isfinite(half) or half * 2 != cell * grid for half in (half_x, half_z)):
        raise ValueError("unaligned WATR extent for water mask correction")
    cells = bytes(bool(nibble) for byte in packed for nibble in (byte >> 4, byte & 15))
    return grid, cell, (half_x, half_z), height, cells


def water_mask_record(reference, donor, polygons):
    grid, cell, half, height, cells = water_grid(reference)
    donor_grid, _, donor_half, donor_height, donor_cells = water_grid(donor)
    if half != donor_half or height != donor_height or grid < donor_grid or grid % donor_grid:
        raise ValueError("water mask correction needs a donor grid with the same extent and height")
    split = grid // donor_grid
    # The donor's cells on the reference grid: the correction only dries them.
    wet = [donor_cells[(i // grid // split) * donor_grid + i % grid // split] for i in range(grid * grid)]
    if any(cells[i] and not wet[i] for i in range(grid * grid)):
        raise ValueError("water mask correction cannot wet a donor cell")
    runs = []
    for z in range(grid):
        x = 0
        while x < grid:
            if cells[z * grid + x] or not wet[z * grid + x]:
                x += 1
                continue
            start = x
            while x < grid and not cells[z * grid + x] and wet[z * grid + x]:
                if not all(in_bounds(polygons, ix * cell - half[0], iz * cell - half[1])
                           for ix in (x, x + 1) for iz in (z, z + 1)):
                    raise ValueError("dry water correction reaches outside the playable bounds")
                x += 1
            runs.append([z, start, x - start])
    if not runs or len(runs) > 128 or sum(run[2] for run in runs) > 256:
        raise ValueError("invalid dry water correction size")
    return {"grid": grid, "dry_runs": runs, "sha256": digest(cells)}


def resources(buf):
    out, counts = [], Counter()
    if buf[:4] != b"ucfb":
        return out
    for scope, tag, payload, _ in census.walk_resources(buf):
        if tag not in census.CLASS_TAGS | {b"wrld", b"prim", b"tern", b"sky_", b"coll", b"PATH", b"path", b"plan", b"plnp", b"skel", b"modl", b"zaa_"}:
            continue
        if tag == b"plnp":
            raw = b"\0"
        elif tag in {b"prim", b"skel"}:
            raw = geometry.child(payload, b"INFO", b"\0")
        else:
            raw = geometry.child(payload, b"NAME", geometry.child(payload, b"TYPE", b"\0"))
        try:
            title = census.resource_name(raw)
        except (ValueError, UnicodeError):
            title = "hash_" + raw.hex() if len(raw) == 4 else ""
        key = (tuple(scope), tag, title.lower())
        ordinal = counts[key]
        counts[key] += 1
        out.append(({"scope": scope, "name": title, "ordinal": ordinal}, tag, payload))
    return out


def identity(resource):
    loc, tag, _ = resource
    return tuple(loc["scope"]), tag, loc["name"].lower(), loc["ordinal"]


def preserve_hashes(payload):
    return sorted({struct.unpack_from("<I", p)[0] for t, p in geometry.chunks(payload)
                   if t == b"PROP" and len(p) >= 5 and struct.unpack_from("<I", p)[0] not in table.PRESENTATION
                   and census.cstr(p[4:]) not in KNOWN_PROPERTY_TOKENS.get(struct.unpack_from("<I", p)[0], {})
                   and census.property_value(struct.unpack_from("<I", p)[0], census.cstr(p[4:])) == "NM"})


def class_record(loc, tag, payload):
    return {**loc, "kind": tag.decode(), "base": census.cstr(geometry.child(payload, b"BASE", b"\0")),
            "properties": props(payload), "preserve_hashes": preserve_hashes(payload)}



def same(a, b):
    """Equal down to the sign of a zero: -0.0 == 0.0 in Python, and the sim's first tick can tell them apart."""
    return encoded(a) == encoded(b)


def exact(raw):
    """Every f32 as stored. geometry.floats folds -0.0 into 0.0, which is right for a census and wrong for a correction."""
    return list(struct.unpack("<" + "f" * (len(raw) // 4), raw))


def primitive_records(payload):
    rows = geometry.primitives(payload)
    index = -1
    model = census.cstr(geometry.child(payload, b"INFO", b"\0"))
    for tag, raw in geometry.chunks(payload):
        if tag == b"NAME":
            index += 1
            rows[index]["name"] = census.cstr(raw)
            rows[index]["model"] = model
        elif tag == b"PRNT" and index >= 0:
            rows[index]["parent"] = census.cstr(raw)
        elif tag == b"XFRM" and index >= 0:
            rows[index]["transform"] = exact(raw)
        elif tag == b"DATA" and index >= 0:
            rows[index]["dimensions"] = exact(raw[4:])
    return rows

def world_record(loc, payload):
    rows = geometry.world(payload)
    raw_rows = [p for t, p in geometry.chunks(payload) if t in {b"inst", b"regn", b"Hint", b"BARR"}]
    counts = Counter()
    for row, raw in zip(rows, raw_rows):
        info = geometry.child(raw, b"INFO", b"")
        row["transform"] = exact(geometry.child(info, b"XFRM"))
        if geometry.child(info, b"SIZE") is not None:
            row["size"] = exact(geometry.child(info, b"SIZE"))[:3]
        row["name"] = census.cstr(geometry.child(info, b"NAME", b"\0"))
        row["class"] = census.cstr(geometry.child(info, b"TYPE", b"\0"))
        key = (row["kind"], row["name"])
        row["ordinal"] = counts[key]
        counts[key] += 1
        row.pop("world", None)
        row["properties"] = props(raw)
        row["preserve_hashes"] = preserve_hashes(raw)
    return {**loc, "terrain_name": census.cstr(geometry.child(payload, b"TNAM", b"\0")),
            "sky_name": census.cstr(geometry.child(payload, b"SNAM", b"\0")), "records": rows}


def height_fills(rows, unit):
    """Rectangles of equal height over (x, z, y) rows, [x, z, columns, rows, y]: flat ground costs one rectangle."""
    by_row = {}
    for x, z, y in sorted(rows, key=lambda r: (r[1], r[0])):
        spans = by_row.setdefault(z, [])
        if spans and spans[-1][2] == y and spans[-1][0] + spans[-1][1] * unit == x:
            spans[-1][1] += 1
        else:
            spans.append([x, 1, y])
    done, open_spans = [], {}
    for z in sorted(by_row):
        following = {}
        for x, count, y in by_row[z]:
            key = (x, count, y)
            rectangle = open_spans.pop(key, None)
            if rectangle and rectangle[1] + rectangle[3] * unit == z:
                rectangle[3] += 1
            else:
                if rectangle:
                    done.append(rectangle)
                rectangle = [x, z, count, 1, y]
            following[key] = rectangle
        done += open_spans.values()
        open_spans = following
    done += open_spans.values()
    return sorted(done, key=lambda r: (r[1], r[0]))


def world_difference(target, existing):
    if existing is None:
        existing = {**target, "records": []}
    def key(row):
        return row["kind"], row["name"], row["ordinal"]
    before = {key(row): row for row in existing["records"]}
    donor_indexes = {key(row): index for index, row in enumerate(existing["records"])}
    records, order = [], []
    for row in target["records"]:
        if key(row) in before and same(row, before[key(row)]):
            order.append(["donor", donor_indexes[key(row)]])
        else:
            order.append(["target", len(records)])
            patch = dict(row)
            title = patch["name"]
            if (len(title) > 200 or not re.fullmatch(r"[A-Za-z0-9_./\\:;<>+@*?=|() -]*", title)
                    or len(title.split()) > 4 or re.search(r"[^_./\\:;<>+@*?=|() -]{32,}", title)):
                patch["name"] = ""
                if key(row) in donor_indexes:
                    patch["preserve_name_from"] = donor_indexes[key(row)]
                elif REGION_SETTING.fullmatch(title):
                    # The map's regions carry shadow settings in their names, and a setting holds only numbers.
                    patch["region_setting"] = title
                else:
                    raise ValueError("unsupported new world name cannot pass public scrub")
            records.append(patch)
    if not records and order == [["donor", i] for i in range(len(existing["records"]))] and all(target[k] == existing[k] for k in ("terrain_name", "sky_name")):
        return None
    return {**{k: v for k, v in target.items() if k != "records"}, "patch_records": True,
            "records": records, "record_order": order}



def cutters(payload):
    out = []
    for tag, record in geometry.chunks(geometry.child(payload, b"CUTS", b"")):
        if tag != b"CUTR":
            continue
        info = geometry.child(record, b"INFO")
        if info is None or len(info) < 20:
            raise ValueError("cutter INFO")
        count = struct.unpack_from("<I", info)[0]
        planes = geometry.child(record, b"PLNS", b"")
        if len(planes) != count * 16:
            raise ValueError("cutter plane count")
        out.append({"name": f"cutter_{len(out)}", "sphere": exact(info[4:20]), "planes": [list(p) for p in struct.iter_unpack("<4f", planes)]})
    return out




def root_motion_signature(payload, game):
    """Physics-relevant clip identities, frame clocks and decoded root curves.

    Bank packing, clip-name registry resolution and non-root pose data do not
    trigger a replacement. A changed constant root offset still does: subtracting
    it later can change f32 deltas, as the Kaadu bank demonstrates.
    """
    counts, rows = Counter(), []
    for clip in geometry.animations(payload, game):
        crc = clip["name_crc"]
        ordinal = counts[crc]
        counts[crc] += 1
        if clip["roots"]:
            rows.append((crc, ordinal, clip["frames"], clip["roots"]))
    return sorted(rows, key=lambda row: row[:2])

def model_bounds(loc, payload):
    info = geometry.child(payload, b"INFO")
    if info is None or len(info) < 68:
        raise ValueError("short model INFO")
    base = 16 if len(info) >= 72 else 12
    sphere = geometry.child(payload, b"SPHR")
    return {**loc, "sphere": exact(sphere[:16]) if sphere is not None and len(sphere) >= 16 else None,
            "vis_box": [exact(info[base + 24:base + 36]), exact(info[base + 36:base + 48])]}

def render_height_fills(reference, donor, gameplay_samples=()):
    """All donor grid heights that differ from the PS2 query surface, as numerical rectangles.

    The render grid can extend past the PS2 grid. Its border follows the PS2
    height query's clamped edge, rather than inventing playable donor hills.
    These rows never change the separately retained simulation grid.
    """
    a = geometry.terrain(reference, retain_samples=True)[0]
    b = geometry.terrain(donor, retain_samples=True)[0]
    if not a["heightfield"] or not b["heightfield"]:
        return [], {"samples": 0, "different": 0, "worst_m": 0.0, "worst_at": None}
    ga, gb = Grid(a), Grid(b)
    if ga.unit != gb.unit or a["height_scale"] != b["height_scale"]:
        raise ValueError("render terrain needs a reviewed grid/quantization conversion")
    already_corrected = {(x, z) for x, z, _ in gameplay_samples}
    rows, worst, at, different = [], 0.0, None, 0
    for iz in range(gb.n):
        for ix in range(gb.n):
            x, z = gb.world(ix, iz)
            # Shared grid units make these exact grid coordinates, including
            # the clamped donor-only border.
            ax = min(max(int((x + ga.offset_x) / ga.unit), 0), ga.n - 1)
            az = min(max(int((z + ga.offset_z) / ga.unit), 0), ga.n - 1)
            y = ga.heights[az * ga.n + ax]
            other = gb.heights[iz * gb.n + ix]
            if y != other:
                different += 1
                if (x, z) not in already_corrected:
                    rows.append([x, z, y])
                if abs(y - other) > worst:
                    worst, at = abs(y - other), [x, other, z]
    return height_fills(rows, gb.unit), {"samples": gb.n * gb.n, "different": different, "render_only_different": len(rows),
                                       "worst_m": worst, "worst_at": at}


def terrain_record(loc, reference, donor, polygons):
    a = geometry.terrain(reference, retain_samples=True)[0]
    b = geometry.terrain(donor, retain_samples=True)[0]
    if not a["heightfield"] or not b["heightfield"]:
        return None, {"changed_inside": 0, "changed_outside": 0, "missing_boundary": False}
    ga, gb = Grid(a), Grid(b)
    samples, fills, inside, outside = [], [], 0, 0
    for iz in range(ga.n):
        for ix in range(ga.n):
            x, z = ga.world(ix, iz)
            y = ga.heights[iz * ga.n + ix]
            # Only exact shared grid samples are legal corrections. A different
            # grid resolution needs a separately reviewed grid conversion.
            bx, bz = (x + gb.offset_x) / gb.unit, (z + gb.offset_z) / gb.unit
            if bx != int(bx) or bz != int(bz) or not (0 <= bx < gb.n and 0 <= bz < gb.n):
                continue
            other = gb.heights[int(bz) * gb.n + int(bx)]
            if y == other:
                continue
            if supports_bounds(polygons, x, z, ga.unit):
                samples.append([x, z, y])
                inside += 1
            else:
                outside += 1
                # The simulation reads the whole grid: a soldier or ground
                # vehicle may walk on past the boundary, and every query off
                # the grid clamps onto its edge (issue #168).
                if polygons:
                    fills.append([x, z, y])
    render_fills, render_summary = render_height_fills(reference, donor, samples + fills)
    ac, bc = cutters(reference), cutters(donor)
    summary = {"changed_inside": inside, "changed_outside": outside, "changed_filled": len(fills), "missing_boundary": not bool(polygons), "render": render_summary}
    if not samples and not fills and not render_fills and ac == bc and ga.n == gb.n:
        return None, summary
    out = {**loc, "grid_size": ga.n, "grid_unit": ga.unit, "height_scale": a["height_scale"],
           "height_samples": samples, "boundaries": polygons}
    if fills:
        out["height_fills"] = height_fills(fills, ga.unit)
    if render_fills:
        out["render_height_fills"] = render_fills
        out["render_grid_size"] = gb.n
    if ac != bc:
        out["cutters"] = ac
    return out, summary


def mission_scripts(raw):
    scripts = {}
    for _scope, tag, payload, _offset in census.walk_resources(raw):
        if tag == b"scr_":
            fields = {t: p for t, p, _ in census.iter_chunks(payload)}
            if b"NAME" in fields and b"BODY" in fields:
                scripts.setdefault(census.cstr(fields[b"NAME"]).lower(), fields[b"BODY"])
    return scripts


def script_facts(scripts, name, game):
    """What the mission generator reads from one script, as table JSON (the same loader, the same call fold)."""
    import gen_missions as missions

    if game == "bf1":
        facts = missions.facts_from_calls(missions.calls40(missions.undump40(scripts[name])))
    else:
        mission = missions.undump50(scripts[name])
        mode = missions.mission_mode(name)
        if mode in ("ass", "1flag"):
            shared_name = name.rsplit("_", 1)[0] + "_cmn"
            common_body = scripts.get(shared_name)
            common = missions.undump50(common_body) if common_body is not None else None
            calls = missions.mission_calls50(mission, common)
            facts = missions.startup_mission_facts50(calls, mission, common)
        else:
            facts = missions.facts_from_calls(missions.calls50(mission))
    capture = facts.get("ai_capture") or {}
    facts["ai_capture"] = {f"{key[0]}/{key[1]}": value for key, value in capture.items()}
    return json.loads(json.dumps(facts))


def mission_differences(rows, reference_lvl, donor_lvl, game):
    """Settle donor differences and required declarations for console-free imports."""
    import gen_missions
    from gameplay_missions import REQUIRED, unknown

    supported = {script for _, source, _, eras in gen_missions.MAPS if source == game
                 for _, script in eras.values()}
    reference, donor = mission_scripts(reference_lvl), mission_scripts(donor_lvl)
    added = 0
    for row in rows:
        name = row["name"].lower()
        if name not in reference or not isinstance(row.get("facts"), dict):
            continue
        target = script_facts(reference, name, game)
        theirs = script_facts(donor, name, game) if name in donor else {}
        for field, value in target.items():
            # Equal donor declarations still need an absolute target when the
            # reference census marked a branched/helper declaration unmeasured.
            # Use exactly the console generator's existing call fold, including
            # space's common setup; never turn an unresolved value into a default.
            required = (name in supported and field in REQUIRED and not unknown(value)
                        and all(str(team) in value for team in (1, 2)))
            if (required or value != theirs.get(field)) and row["facts"].get(field) != value:
                row["facts"][field] = value
                added += 1
    return added


class Programs:
    """Each distinct instruction list once: the same mesh over the same donor recurs across a game's LVLs."""

    def __init__(self):
        self.lists = {"collision": [], "skeleton": [], "route": [], "bank": []}
        self.seen = {}
        self.stats = Counter()

    def add(self, kind, program):
        key = kind, json.dumps(program, sort_keys=True)
        if key not in self.seen:
            self.seen[key] = len(self.lists[kind])
            self.lists[kind].append(program)
        return self.seen[key]

    def resource(self, kind, payload, donor):
        """Table row fields for one reference resource over its donor, or a refusal if nothing reproduces it."""
        whole = kind.encode() + struct.pack("<I", len(payload)) + payload + b"\0" * (-len(payload) % 4)
        if kind == "coll":
            program, built = programs.collision(payload, donor, self.stats)
            return {"program": self.add("collision", program), "sha256": digest(built)}
        if kind == "skel":
            program, built = programs.skeleton(payload, donor, self.stats)
            if built != whole:
                raise ValueError("skeleton program does not reproduce the reference")
            return {"program": self.add("skeleton", program), "sha256": digest(built)}
        if kind == "zaa_":
            program, built = programs.bank(payload, donor, self.stats)
            body = built[8:8 + struct.unpack_from("<I", built, 4)[0]]
            if root_motion_signature(body, self.game) != root_motion_signature(payload, self.game):
                raise ValueError("bank program does not reproduce the reference's root motion")
            return {"program": self.add("bank", program), "sha256": digest(built)}
        return {"program": self.add("route", programs.route(payload, donor, self.stats)), "sha256": digest(whole)}


def generate(reference_root, donor_root, out, program_out, *, reference_census=None, donor_census=None):
    """Fingerprint exact source files; emit only differences, always as target values."""
    reference = reference_census or census.scan(Path(reference_root), geometry=False)
    donor = donor_census or census.scan(Path(donor_root), geometry=False)
    game = reference["source"]["game"]
    platform = donor["source"]["platform"]
    if reference["source"]["platform"] != "ps2" or donor["source"]["game"] != game or platform not in {"pc", "xbox"}:
        raise ValueError("need a PS2 reference and a same-game PC/Xbox donor")
    ref_files = {row["name"]: row for row in reference["files"]}
    donor_files = {row["name"]: row for row in donor["files"]}
    out, program_out = Path(out), Path(program_out)
    if out.is_symlink() or program_out.is_symlink():
        raise ValueError("output may not be a symlink")
    book = Programs()
    book.game = game
    files, report = [], {"game": game, "platform": platform, "maps": {}}
    # Keep only file paths in memory; read each matching LVL pair together.
    donor_paths = {}
    if Path(donor_root).is_dir():
        for p in Path(donor_root).rglob("*"):
            if p.is_file() and p.suffix.lower() == ".lvl" and "addon" not in [part.lower() for part in p.relative_to(donor_root).parts]:
                donor_paths[census.canonical_path(p.relative_to(donor_root).as_posix())] = p
    for file, raw_reference in census.input_files(Path(reference_root), set(donor_files)):
        row = {"file": file, "sha256": donor_files[file]["sha256"]}
        if file in donor_paths:
            raw_donor = donor_paths[file].read_bytes()
        else:
            raw_donor = next(raw for name, raw in census.input_files(Path(donor_root), {file}) if name == file)
        if digest(raw_donor) != row["sha256"] or digest(raw_reference) != ref_files[file]["sha256"]:
            raise ValueError(f"census fingerprint changed for {file}")
        ar, br = resources(raw_reference), resources(raw_donor)
        bi = {identity(r): r for r in br}
        ai = {identity(r): r for r in ar}
        polygon = boundaries(ar, game)
        for loc, tag, payload in ar:
            key = (tuple(loc["scope"]), tag, loc["name"].lower(), loc["ordinal"])
            other = bi.get(key)
            if tag in census.CLASS_TAGS:
                target = class_record(loc, tag, payload)
                existing = class_record(other[0], tag, other[2]) if other else None
                if target != existing:
                    row.setdefault("classes", []).append(target)
            elif tag == b"wrld":
                target = world_record(loc, payload)
                existing = world_record(other[0], other[2]) if other else None
                patch = world_difference(target, existing)
                if patch:
                    row.setdefault("world", []).append(patch)
            elif tag == b"prim":
                target = {**loc, "records": primitive_records(payload)}
                existing = {**other[0], "records": primitive_records(other[2])} if other else None
                if existing is None or not same(target, existing):
                    row.setdefault("primitives", []).append(target)
            elif tag == b"modl" and other:
                target = model_bounds(loc, payload)
                if not same(target, model_bounds(other[0], other[2])):
                    row.setdefault("models", []).append(target)
            elif tag == b"sky_":
                target = {**loc, "nodes": config_nodes(payload)}
                existing = {**other[0], "nodes": config_nodes(other[2])} if other else None
                if target != existing:
                    row.setdefault("sky", []).append(target)
            elif tag == b"tern" and other:
                # WATR feeds swimming, drowning and the tactical-position
                # table even on a terrain stub with no heightfield.
                if geometry.child(payload, b"WATR") is None and geometry.child(other[2], b"WATR") is not None:
                    row.setdefault("remove_water", []).append(loc)
                target, summary = terrain_record(loc, payload, other[2], polygon)
                report["maps"][file] = summary
                if target:
                    row.setdefault("terrain", []).append(target)
                # Kashyyyk Islands: an entirely wet coarser donor grid. Naboo
                # Theed: PC and Xbox wet two plaza cells the PS2 keeps dry
                # (issue #168).
                if game == "bf1" and file in ("kas/kas1.lvl", "nab/nab2.lvl"):
                    if "water_mask" in row:
                        raise ValueError(f"multiple {file} water corrections")
                    row["water_mask"] = water_mask_record(payload, other[2], polygon)
            elif tag in {b"PATH", b"path", b"plan", b"plnp", b"skel", b"zaa_"}:
                if tag == b"zaa_":
                    reference_motion = root_motion_signature(payload, game)
                    donor_motion = root_motion_signature(other[2], game) if other else []
                    if reference_motion == donor_motion:
                        continue
                elif other and payload == other[2]:
                    continue
                kind = tag.decode()
                row.setdefault("resources", []).append({**loc, "kind": kind, **book.resource(kind, payload, other[2] if other else None)})
            elif tag == b"coll":
                # Ignore NODE in resource pairing: a renamed attachment does
                # not mean that the mesh under the same file/scope/name vanished.
                if other and payload == other[2]:
                    continue
                row.setdefault("collision", []).append({**loc, **book.resource("coll", payload, other[2] if other else None)})
        for loc, tag, _ in br:
            if (tuple(loc["scope"]), tag, loc["name"].lower(), loc["ordinal"]) not in ai:
                category = "remove_classes" if tag in census.CLASS_TAGS else {b"coll": "remove_collision", b"prim": "remove_primitives", b"PATH": "remove_resources", b"path": "remove_resources", b"plan": "remove_resources", b"plnp": "remove_resources", b"skel": "remove_resources"}.get(tag)
                if category:
                    row.setdefault(category, []).append({**loc, "kind": tag.decode()})
        files.append(row)
    indexed = {row["file"] for row in files}
    files += [{"file": file, "sha256": meta["sha256"]} for file, meta in donor_files.items() if file not in indexed]
    _, target_files, _, _ = table.convert(reference)
    # Mission facts stay explicit and typed. Unknown branches remain NM.
    for row in files:
        missions = target_files.get(row["file"], {}).get("missions")
        if missions:
            row["missions"] = missions
    mission_row = next((row for row in files if row["file"] == "mission.lvl"), None)
    if mission_row and mission_row.get("missions"):
        reference_lvl = next(raw for name, raw in census.input_files(Path(reference_root), {"mission.lvl"}) if name == "mission.lvl")
        donor_lvl = next(raw for name, raw in census.input_files(Path(donor_root), {"mission.lvl"}) if name == "mission.lvl")
        report["mission_facts_added"] = mission_differences(mission_row["missions"], reference_lvl, donor_lvl, game)
    document = {"schema": SCHEMA, "reference": "ps2-v1", "game": game, "platform": platform,
                "presentation_hashes": sorted(table.PRESENTATION), "files": sorted(files, key=lambda r: r["file"])}
    data = encoded(document)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(data)
    program_data = encoded({"schema": SCHEMA, "reference": "ps2-v1", "game": game, "platform": platform, **book.lists})
    program_out.write_bytes(program_data)
    report.update(table_bytes=len(data), table_sha256=digest(data), program_bytes=len(program_data),
                  program_sha256=digest(program_data), programs={kind: len(rows) for kind, rows in book.lists.items()},
                  program_bytes_by_kind={kind: len(encoded(rows)) for kind, rows in book.lists.items()},
                  instructions=dict(sorted(book.stats.items())))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("donor", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--programs-out", type=Path, required=True)
    parser.add_argument("--reference-census", type=Path)
    parser.add_argument("--donor-census", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    try:
        report = generate(args.reference, args.donor, args.out, args.programs_out,
                          reference_census=json.loads(args.reference_census.read_bytes()) if args.reference_census else None,
                          donor_census=json.loads(args.donor_census.read_bytes()) if args.donor_census else None)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_bytes(encoded(report))
        print(json.dumps(report, sort_keys=True))
    except (ValueError, OSError, KeyError, struct.error) as exc:
        parser.exit(2, f"gameplay corrections: {exc}\n")


if __name__ == "__main__":
    main()
