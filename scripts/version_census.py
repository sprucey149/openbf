#!/usr/bin/env python3
"""Read-only Battlefront census: names and numbers, never assets or bytecode.

Run `python3 scripts/version_census.py scan INSTALL_OR_DISC --out census.json`.
An extracted LVL root, ordinary install folder, ISO9660 or XDVDFS disc works.
`compare LEFT.json RIGHT.json --out differences.json` compares matching scopes.
Requires Python 3 and the adjacent extract_lua.py, gen_missions.py and
version_geometry.py modules; running from a repository checkout also resolves
known animation-name hashes from its source registry.
Unknown formats fail; unmeasured fields are NM, never implicit equality.
"""
import argparse
import collections
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import sys

import extract_lua
import gen_missions
import version_geometry
import version_sky

NAME = re.compile(r"^[A-Za-z0-9_./\\:<>+@*?=;|-]+$")
NUM = re.compile(r"^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$")
CLASS_TAGS = {b"entc", b"wpnc", b"ordc", b"expc"}
CONTENT_TAGS = CLASS_TAGS | {b"modl", b"skel", b"coll", b"gmod", b"tex_", b"wrld", b"tern", b"zaa_", b"zaf_", b"scr_", b"font", b"plan", b"path", b"prim"}
ALL_RESOURCE_TAGS = CONTENT_TAGS | {b"PATH", b"lght", b"Locl", b"mcfg", b"fx__", b"snd_", b"anim", b"bnd_", b"shdw", b"CLTH", b"comb", b"sky_", b"plnp"}
MAX_FILE = 2 << 30


def iter_chunks(buf, start=0, end=None):
    """Strict ucfb siblings. Payload sizes include alignment in shipped LVLs."""
    end = len(buf) if end is None else end
    pos = start
    while pos < end:
        if end - pos < 8:
            if any(buf[pos:end]):
                raise ValueError(f"trailing chunk bytes at {pos}")
            return
        tag, size = struct.unpack_from("<4sI", buf, pos)
        stop = pos + 8 + size
        if stop > end:
            raise ValueError(f"truncated {tag!r} chunk at {pos}")
        yield tag, buf[pos + 8:stop], pos
        pos = stop + (-size % 4)


def walk_resources(buf):
    """Yield resources below ucfb/lvl_ only, retaining every sublevel identity."""
    def walk(data, scope, offset=0):
        for tag, payload, at in iter_chunks(data):
            if tag == b"ucfb":
                yield from walk(payload, scope, offset + at + 8)
            elif tag == b"lvl_":
                for subtag, subpayload, subat in iter_chunks(payload):
                    if subtag in ALL_RESOURCE_TAGS:
                        yield scope, subtag, subpayload, offset + at + 8 + subat
                    else:
                        key = struct.unpack("<I", subtag)[0]
                        yield from walk(subpayload, scope + [key], offset + at + 16 + subat)
            else:
                yield scope, tag, payload, offset + at
    if buf[:4] != b"ucfb":
        raise ValueError("not a ucfb container")
    yield from walk(buf, [])


def cstr(value):
    return value.split(b"\0", 1)[0].decode("ascii", "strict")


def safe_value(value):
    """Only numeric scalars/vectors and token names; never free-form text."""
    if isinstance(value, bool) or value is None:
        return value
    if isinstance(value, (int, float)):
        return value if math.isfinite(value) else "NM"
    if isinstance(value, (list, tuple)):
        return [safe_value(v) for v in value]
    if isinstance(value, dict):
        result = {}
        for key, child in value.items():
            if isinstance(key, tuple):
                key = "/".join(str(safe_value(v)) for v in key)
            if isinstance(key, (int, float)) or NAME.fullmatch(str(key)):
                result[str(key)] = safe_value(child)
        return result
    if not isinstance(value, str):
        return "NM"
    s = value.strip().strip('"')
    if not s:
        return ""
    if NUM.fullmatch(s):
        n = float(s)
        return int(n) if math.isfinite(n) and n.is_integer() else n if math.isfinite(n) else "NM"
    parts = re.split(r"[\s,]+", s)
    if len(parts) > 1 and all(NUM.fullmatch(x) for x in parts):
        return [safe_value(x) for x in parts]
    return s if len(s) <= 256 and NAME.fullmatch(s) else "NM"


# Text a player reads, even when one word passes the name check ("Stormtrooper").
DISPLAY_TEXT_PROPERTIES = {extract_lua.fnv1a("Label")}
# These ODF handlers consume whitespace-separated token sequences, not prose.
# Unknown keys keep the conservative scalar/name parser.
TOKEN_PROPERTIES = {extract_lua.fnv1a(key) for key in (
    "VOSound", "ControlSpeed", "BoostSound", "AnimationTrigger", "ComboAnimationBank",
    "AmbientSound", "AttachToHardpoint", "HitLocation", "DamageLength", "SoundProperty",
    "OffhandFirePointName", "HoloImageGeometry", "PowerupSound", "BuildingCollisionPrim",
    "VehicleCollisionPrim", "TurretYawSound", "TurretPitchSound", "TurretActivateSound",
    "Emitter", "AttachedFirePoint", "ChargeSound", "CapturedSound", "DischargeSound",
    "LostSound", "DisputeSound", "TurretDeactivateSound", "CollisionOtherSound",
    "OrdnanceSound", "FireSound", "SkeletonName", "CustomAnimationBank",
)}


def property_value(key, value):
    if key in DISPLAY_TEXT_PROPERTIES:
        return "NM"
    result = safe_value(value)
    if result != "NM" or key not in TOKEN_PROPERTIES:
        return result
    tokens = [safe_value(v) for v in re.split(r"[\s,]+", value.strip())]
    return tokens if tokens and "NM" not in tokens else "NM"


def resource_name(value):
    """NAME/TYPE are typed identifiers and may contain spaces (UI textures)."""
    name = cstr(value)
    if not name or len(name) > 256 or any(ord(c) < 32 or ord(c) > 126 for c in name):
        raise ValueError("invalid resource name")
    return name


def canonical_path(path):
    parts = path.replace("\\", "/").lower().split("/")
    for marker in ("_lvl_ps2", "_lvl_xbox", "_lvl_pc"):
        if marker in parts:
            parts = parts[parts.index(marker) + 1:]
            break
    return "/".join(parts)


def _read_at(handle, offset, size):
    if size < 0 or size > MAX_FILE:
        raise ValueError("unreasonable file size")
    handle.seek(offset)
    data = handle.read(size)
    if len(data) != size:
        raise ValueError("truncated disc extent")
    return data


def disc_entries(path):
    """Yield (disc path, byte offset, length), bounded directory traversal."""
    with path.open("rb") as f:
        for base in (0, 0x18300000, 0xFD90000, 0x2080000):
            f.seek(base + 0x10000)
            if f.read(20) != b"MICROSOFT*XBOX*MEDIA":
                continue
            sector, size = struct.unpack("<II", f.read(8))
            todo, seen = [("", sector, size)], set()
            while todo:
                prefix, sector, size = todo.pop()
                if sector in seen:
                    raise ValueError("cyclic XDVDFS directory")
                seen.add(sector)
                if len(seen) > 4096 or size > 4 << 20:
                    raise ValueError("XDVDFS directory limit")
                data = _read_at(f, base + sector * 2048, size)
                nodes, visited = [0], set()
                while nodes:
                    off = nodes.pop()
                    if off in visited:
                        raise ValueError("cyclic XDVDFS node")
                    visited.add(off)
                    if off + 14 > len(data):
                        raise ValueError("short XDVDFS node")
                    left, right, start, length, attr, n = struct.unpack_from("<HHIIBB", data, off)
                    if left == right == 0xFFFF:
                        continue
                    if off + 14 + n > len(data):
                        raise ValueError("short XDVDFS name")
                    name = data[off + 14:off + 14 + n].decode("ascii")
                    child = prefix + name
                    if attr & 0x10:
                        if length:
                            todo.append((child + "/", start, length))
                    else:
                        yield child, base + start * 2048, length
                    nodes.extend(v * 4 for v in (left, right) if v)
            return
        pvd = _read_at(f, 16 * 2048, 2048)
        if pvd[0:7] != b"\x01CD001\x01":
            raise ValueError("unknown disc format: expected ISO9660 or XDVDFS")
        todo = [("", struct.unpack_from("<I", pvd, 158)[0], struct.unpack_from("<I", pvd, 166)[0])]
        seen = set()
        while todo:
            prefix, sector, size = todo.pop()
            if sector in seen:
                raise ValueError("cyclic ISO9660 directory")
            seen.add(sector)
            if len(seen) > 4096 or size > 4 << 20:
                raise ValueError("ISO9660 directory limit")
            data, off = _read_at(f, sector * 2048, size), 0
            while off < len(data):
                n = data[off]
                if n == 0:
                    off = (off // 2048 + 1) * 2048
                    continue
                rec = data[off:off + n]
                if n < 34 or len(rec) != n or 33 + rec[32] > n:
                    raise ValueError("short ISO9660 record")
                off += n
                raw = rec[33:33 + rec[32]]
                if raw in (b"\0", b"\1"):
                    continue
                name = raw.decode("ascii").split(";")[0]
                start, length = struct.unpack_from("<I", rec, 2)[0], struct.unpack_from("<I", rec, 10)[0]
                if rec[25] & 2:
                    todo.append((prefix + name + "/", start, length))
                else:
                    yield prefix + name, start * 2048, length


def input_files(path, only=None):
    if path.is_dir():
        candidates = sorted(p for p in path.rglob("*") if p.is_file() and p.suffix.lower() == ".lvl")
        # An install may contain addons. Census the base LVL tree only; each
        # addon must be scanned separately so it cannot masquerade as retail.
        roots = {p.parent for p in candidates if p.name.lower() == "mission.lvl"
                 and p.parent.name.lower() in ("_lvl_pc", "_lvl_ps2", "_lvl_xbox")
                 and "addon" not in [part.lower() for part in p.relative_to(path).parts]}
        if len(roots) > 1:
            raise ValueError("multiple base LVL trees: choose one install/disc")
        root = next(iter(roots), path)
        for p in candidates:
            if p.is_relative_to(root) and "addon" not in [x.lower() for x in p.relative_to(root).parts]:
                name = canonical_path(p.relative_to(root).as_posix())
                if only is None or name in only:
                    yield name, p.read_bytes()
    elif path.is_file():
        entries = list(disc_entries(path))
        with path.open("rb") as f:
            for name, offset, size in sorted(entries):
                if name.lower().endswith(".lvl") and "addon" not in name.lower().split("/"):
                    name = canonical_path(name)
                    if only is None or name in only:
                        yield name, _read_at(f, offset, size)
    else:
        raise ValueError(f"input does not exist: {path}")


def mission_facts(body):
    header = extract_lua.parse_lua_header(body)
    if not header or header.get("error") or header["endian"] != 1 or any(header[k] != 4 for k in ("sizeof_int", "sizeof_size_t", "sizeof_instr", "sizeof_number")):
        raise ValueError("unsupported Lua header")
    if header["version"] == "4.0":
        proto = gen_missions.undump40(body)
        calls = gen_missions.calls40(proto)
    elif header["version"] == "5.0":
        proto = gen_missions.undump50(body)
        calls = gen_missions.calls50(proto, isolate_prototypes=True)
    else:
        raise ValueError("unsupported Lua version")
    # Explicit gameplay whitelist, excluding music, VO and localization.
    sources = {
        "units": {"SetUnitCount", "SetupTeams"}, "tickets": {"SetReinforcementCount", "SetupTeams"},
        "names": {"SetTeamName", "SetupTeams"}, "classes": {"AddUnitClass", "SetupTeams"},
        "death_regions": {"AddDeathRegion"}, "attacking": {"SetAttackingTeam"},
        "spawn": {"SetSpawnDelay"}, "cps": {"SetProperty", "SetObjectTeam"},
        "posts": {"CommandPost:New"}, "killed": {"KillObject"}, "layers": {"ReadDataFile"},
        "numbots": {"ScriptCB_SetNumBots"}, "ai_view": {"SetAIViewMultiplier"},
        "ai_aggr": {"SetTeamAggressiveness"}, "ai_snipe": {"AISnipeSuitabilityDist"},
        "ai_capture": {"AICanCaptureCP"}, "ai_damage": {"SetAIDamageThreshold"},
        "fly": {"SetMaxPlayerFlyHeight"}, "bird_count": {"SetNumBirdTypes"},
        "bird_types": {"SetBirdType"}, "heroes": {"SetHeroClass"}, "class_load_order": {"ReadDataFile"},
    }
    def unknown(value):
        if isinstance(value, gen_missions.Unknown):
            return True
        if isinstance(value, dict):
            return any(unknown(v) for v in value.values())
        if isinstance(value, (list, tuple)):
            return any(unknown(v) for v in value)
        return False
    proto_paths = {}
    def visit(p, path):
        proto_paths[id(p)] = path
        for i, child in enumerate(p.protos):
            visit(child, path + [i])
    visit(proto, [])
    if header["version"] == "5.0":
        named = {id(p): name for name, p in gen_missions.global_functions50(proto).items()}
        def branches(p):
            return any(gen_missions.OPS50[i & 0x3f] in {"JMP", "EQ", "LT", "LE", "TEST", "FORLOOP", "TFORLOOP", "TFORPREP"} for i in p.code)
    else:
        named, closure = {}, None
        for instruction in proto.code:
            op, u, _, _, a = gen_missions._dec40(instruction)
            if op == "CLOSURE":
                closure = proto.protos[a] if a < len(proto.protos) else None
            elif op == "SETGLOBAL" and closure is not None:
                named[id(closure)] = proto.kstr[u]
                closure = None
        def branches(p):
            return any(gen_missions._dec40(i)[0].startswith(("JMP", "FOR", "LFOR")) for i in p.code)
    verbs = set().union(*sources.values())
    declarations = []
    linear, ambiguous = [], set()
    for call in calls:
        if call.name not in verbs:
            continue
        p = call.prototype
        has_branches = branches(p)
        function = named.get(id(p), "anonymous")
        declarations.append({"prototype": proto_paths[id(p)], "function": function,
                             "verb": call.name, "arguments": safe_value(call.args),
                             "control_flow": "NM" if has_branches else "linear"})
        if function in ("ScriptInit", "ScriptPostLoad"):
            if has_branches or branches(proto) or function != "ScriptInit":
                ambiguous.add(call.name)
            else:
                linear.append(call)
    facts = gen_missions.facts_from_calls(linear)
    # These helper defaults are not bytes on the disc: preserve omission instead.
    facts["classes"] = {}
    for call in linear:
        args = call.args
        if call.name == "AddUnitClass" and len(args) >= 2 and isinstance(args[0], (int, float)) and isinstance(args[1], str):
            facts["classes"].setdefault(int(args[0]), []).append([args[1], args[2] if len(args) > 2 else "NM", args[3] if len(args) > 3 else None])
        elif call.name == "SetupTeams" and args and isinstance(args[0], dict):
            for side in args[0].values():
                if not isinstance(side, dict) or not isinstance(side.get("team"), (int, float)):
                    continue
                for value in side.values():
                    if isinstance(value, dict) and isinstance(value.get(1.0), str):
                        facts["classes"].setdefault(int(side["team"]), []).append([value[1.0], value.get(2.0, "NM"), value.get(3.0)])
        elif call.name == "SetNumBirdTypes" and args:
            facts["bird_count"] = args[0]  # Raw declaration, no engine clamp.
    measured = {}
    for key, wanted in sources.items():
        relevant = [c for c in linear if c.name in wanted]
        if wanted & ambiguous:
            measured[key] = "NM"
        elif relevant:
            measured[key] = "NM" if any(unknown(c.args) for c in relevant) else safe_value(facts[key])
    return measured, sum(unknown(v) for c in calls for v in c.args), header["version"], declarations



def scan(path, geometry=True):
    out = {"schema": 1, "source": {}, "files": [], "entities": [], "missions": [], "skies": [], "content": [], "errors": [], "unmeasured": ["mission_dynamic_execution", "geometry", "animation_root_motion"]}
    texture_formats, lua_versions = collections.Counter(), set()
    geometry_files = set()
    for file, buf in input_files(path):
        out["files"].append({"name": file, "bytes": len(buf), "sha256": hashlib.sha256(buf).hexdigest()})
        if buf[:4] != b"ucfb":
            out["errors"].append({"file": file, "error": "not_ucfb"})
            continue
        ordinals = collections.Counter()
        for scope, tag, payload, offset in walk_resources(buf):
            if tag == b"sky_":
                row = version_sky.parse(payload)
                key = (tuple(scope), tag, row["name"])
                ordinal = ordinals[key]
                ordinals[key] += 1
                out["skies"].append({"file": file, "scope": scope, "ordinal": ordinal, **row})
                continue
            if tag in {b"coll", b"prim", b"wrld", b"tern", b"zaa_", b"plan", b"plnp", b"path", b"PATH"}:
                geometry_files.add(file)
            if tag not in CONTENT_TAGS:
                continue
            children = list(iter_chunks(payload))
            fields = {t: p for t, p, _ in children}
            raw_name = fields.get(b"TYPE", fields.get(b"NAME", b""))
            try:
                name = resource_name(raw_name)
            except (UnicodeError, ValueError):
                name = "hash_" + raw_name.hex() if len(raw_name) == 4 else "NM"
            identity = (tuple(scope), tag, name)
            ordinal = ordinals[identity]
            ordinals[identity] += 1
            loc = {"file": file, "scope": scope, "kind": tag.decode("ascii"), "name": name, "ordinal": ordinal}
            out["content"].append(loc)
            if tag == b"tex_":
                texture_formats["pc" if b"FMT_" in fields else "xbox" if b"BODY" in fields and len(fields.get(b"INFO", b"")) == 20 else "ps2"] += 1
            if tag in CLASS_TAGS:
                props = []
                for t, p, _ in children:
                    if t == b"PROP":
                        if len(p) < 5:
                            raise ValueError("short entity PROP")
                        key = struct.unpack_from("<I", p)[0]
                        try:
                            value = property_value(key, cstr(p[4:]))
                        except UnicodeError:
                            value = "NM"
                        props.append([key, value])
                out["entities"].append({**loc, "base": safe_value(cstr(fields.get(b"BASE", b""))), "properties": props})
            if tag == b"scr_" and file == "mission.lvl":
                # Identify the game even when a supported Lua generation has
                # widths the argument decoder cannot yet measure (e.g. f64).
                header = extract_lua.parse_lua_header(fields.get(b"BODY", b""))
                if header and header.get("version") in ("4.0", "5.0"):
                    lua_versions.add(header["version"])
                try:
                    facts, unknown, version, declarations = mission_facts(fields[b"BODY"])
                    lua_versions.add(version)
                    out["missions"].append({**loc, "facts": facts, "static_calls": declarations, "unresolved_arguments": unknown})
                except (ValueError, IndexError, KeyError, struct.error) as exc:
                    out["missions"].append({**loc, "facts": "NM", "error": type(exc).__name__})
    if not out["files"]:
        raise ValueError("no ucfb LVLs found")
    if len(lua_versions) != 1 or len(texture_formats) != 1:
        raise ValueError(f"unknown/mixed source: Lua={sorted(lua_versions)}, textures={dict(texture_formats)}")
    out["source"] = {"game": "bf1" if lua_versions == {"4.0"} else "bf2", "platform": next(iter(texture_formats)), "texture_format_counts": dict(texture_formats)}
    if geometry:
        out["geometry"] = {"collision": [], "primitives": [], "world": [], "terrain": [], "animations": [], "routes": []}
        out["animation_registry_sha256"] = version_geometry.animation_registry_digest()
        for file, buf in input_files(path, geometry_files):
            if buf[:4] != b"ucfb":
                continue
            ordinals = collections.Counter()
            for scope, tag, payload, offset in walk_resources(buf):
                if tag not in {b"coll", b"prim", b"wrld", b"tern", b"zaa_", b"plan", b"plnp", b"path", b"PATH"}:
                    continue
                try:
                    parsed = version_geometry.parse_resource(tag, payload, out["source"]["game"])
                    if parsed:
                        category, rows = parsed
                        for row in rows:
                            key = (tuple(scope), tag, row.get("name", row.get("bank", "")), row.get("name_crc"))
                            ordinal = ordinals[key]
                            ordinals[key] += 1
                            out["geometry"][category].append({"file": file, "scope": scope, "ordinal": ordinal, **row})
                except (ValueError, struct.error, UnicodeError, IndexError, OverflowError) as exc:
                    out["errors"].append({"file": file, "scope": scope, "tag": tag.decode("ascii"), "offset": offset, "error": str(exc)})
        out["unmeasured"] = ["mission_dynamic_execution", "world_animation_chunks", "animation_full_pose", "sanm_blends", "animation_unresolved_names"]
    return out


def identity(row):
    return (row["file"], tuple(row["scope"]), row["kind"], row["name"], row["ordinal"])


def _contains_nm(value):
    if value == "NM":
        return True
    if isinstance(value, dict):
        return any(_contains_nm(v) for v in value.values())
    if isinstance(value, list):
        return any(_contains_nm(v) for v in value)
    return False


def _props(row):
    counts = collections.Counter()
    result = {}
    for key, value in row["properties"]:
        result[(key, counts[key])] = value
        counts[key] += 1
    return result


def _leaves(value, path=()):
    if isinstance(value, dict):
        for key, child in value.items():
            yield from _leaves(child, path + (str(key),))
    elif isinstance(value, list):
        for key, child in enumerate(value):
            yield from _leaves(child, path + (str(key),))
    elif value is not None:
        yield path, value


def _static_values(row):
    counts = collections.Counter()
    out = {}
    for call in row.get("static_calls", []):
        function = call["function"]
        context = function if function != "anonymous" else "prototype_" + "_".join(map(str, call["prototype"]))
        identity = (context, call["verb"])
        ordinal = counts[identity]
        counts[identity] += 1
        for field, value in _leaves(call["arguments"]):
            out[identity + (str(ordinal),) + field] = value
    return out


def _value_summary(pairs):
    result = {"compared": 0, "changed": 0, "unmeasured": 0, "left_only": 0, "right_only": 0, "differences": []}
    for context, a, b in pairs:
        # Unknown subtrees are coverage gaps, never missing platform values.
        unknown_roots = {key for values in (a, b) for key, value in values.items() if value == "NM"}
        minimal = {key for key in unknown_roots if not any(key != other and key[:len(other)] == other for other in unknown_roots)}
        result["unmeasured"] += len(minimal)
        def known(values):
            return {key: value for key, value in values.items() if not any(key[:len(root)] == root for root in minimal)}
        a, b = known(a), known(b)
        result["left_only"] += len(a.keys() - b.keys())
        result["right_only"] += len(b.keys() - a.keys())
        for key in sorted(a.keys() & b.keys()):
            av, bv = a[key], b[key]
            if _contains_nm(av) or _contains_nm(bv):
                result["unmeasured"] += 1
                continue
            result["compared"] += 1
            if av != bv:
                result["changed"] += 1
                row = {"identity": list(context), "field": list(key), "left": av, "right": bv}
                if isinstance(av, (int, float)) and isinstance(bv, (int, float)):
                    row["absolute_delta"] = abs(av - bv)
                result["differences"].append(row)
    return result


def compare(left, right):
    if left["source"]["game"] != right["source"]["game"]:
        raise ValueError("compare editions of the same game")
    result = {"schema": 1, "left": left["source"], "right": right["source"]}
    for category, values in (("entities", ("base", "properties")), ("missions", ("facts", "static_calls")), ("content", ())):
        a, b = ({identity(r): r for r in doc[category]} for doc in (left, right))
        common = sorted(a.keys() & b.keys())
        diffs = []
        for key in common:
            if any(a[key].get(v) != b[key].get(v) for v in values):
                diffs.append({"identity": list(key), "left": {v: a[key].get(v) for v in values}, "right": {v: b[key].get(v) for v in values}})
        result[category] = {"compared": len(common), "changed": len(diffs), "left_only": [a[k] for k in sorted(a.keys() - b.keys())], "right_only": [b[k] for k in sorted(b.keys() - a.keys())], "differences": diffs}
        if category == "entities":
            result["properties"] = _value_summary((k, _props(a[k]), _props(b[k])) for k in common)
            result["property_order_changed"] = sum([p[0] for p in a[k]["properties"]] != [p[0] for p in b[k]["properties"]] for k in common)
            result["entity_kinds"] = {kind: {"compared": sum(k[2] == kind for k in common), "changed": sum(d["identity"][2] == kind for d in diffs)} for kind in sorted({k[2] for k in common})}
        if category == "missions":
            result["mission_static_values"] = _value_summary((k, _static_values(a[k]), _static_values(b[k])) for k in common)
            result["mission_values"] = _value_summary((k, dict(_leaves(a[k]["facts"])), dict(_leaves(b[k]["facts"]))) for k in common if a[k]["facts"] != "NM" and b[k]["facts"] != "NM")
    for category in ("entities", "missions", "content"):
        for side in ("left_only", "right_only"):
            result[category][side + "_count"] = len(result[category][side])
    if "skies" in left and "skies" in right:
        indexes = [{(r["file"], tuple(r["scope"]), r["name"], r["ordinal"]): r for r in d["skies"]}
                   for d in (left, right)]
        a, b = indexes
        common = sorted(a.keys() & b.keys())
        differences = [{"identity": key, "left": {f: a[key][f] for f in ("sky_info", "low_res_terrain")},
                        "right": {f: b[key][f] for f in ("sky_info", "low_res_terrain")}}
                       for key in common if any(a[key][f] != b[key][f] for f in ("sky_info", "low_res_terrain"))]
        result["skies"] = {"compared": len(common), "changed": len(differences),
                           "left_only_count": len(a.keys() - b.keys()), "right_only_count": len(b.keys() - a.keys()),
                           "differences": differences}
    if "geometry" in left and "geometry" in right:
        result["geometry"] = version_geometry.compare_geometry(left["geometry"], right["geometry"])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    scan_parser = sub.add_parser("scan")
    scan_parser.add_argument("input", type=Path)
    scan_parser.add_argument("--out", required=True, type=Path)
    scan_parser.add_argument("--no-geometry", action="store_true", help="ODF/mission/content census only")
    cmp_parser = sub.add_parser("compare")
    cmp_parser.add_argument("left", type=Path)
    cmp_parser.add_argument("right", type=Path)
    cmp_parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = scan(args.input, not args.no_geometry) if args.command == "scan" else compare(json.loads(args.left.read_text()), json.loads(args.right.read_text()))
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + "\n")
    except (ValueError, OSError, struct.error) as exc:
        parser.exit(2, f"census: {exc}\n")


if __name__ == "__main__":
    main()
