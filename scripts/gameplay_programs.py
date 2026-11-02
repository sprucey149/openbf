#!/usr/bin/env python3
"""Instructions that turn a player's own donor resource into the PS2's gameplay resource.

A program holds names and numbers and is applied to the donor chunk at bake time. Each kind prefers a reuse of what the
donor already holds, in this order: copy runs, translated runs, single-axis moves, midpoints of donor edges, and only then
absolute coordinates. Every program is applied here before it is kept, and the result is compared with the reference, so
a program that does not reproduce the reference cannot be written. The bake applies the same program in
engine/crates/bake/src/gameplay/program.rs and refuses a result whose SHA-256 differs from the table's.

A collision program reproduces what the game reads from `coll`: NAME, NODE, MASK, the bounds, the vertices and the
polygons in order. The tree's split data and each leaf's six unread bytes are written as zero.
A bank program keeps the donor's clips and replaces only root tracks, or a whole clip whose frames or bones differ.
"""
from __future__ import annotations

from collections import Counter
from difflib import SequenceMatcher
import hashlib
import struct

import version_geometry as geometry

ROOTS = geometry.ROOTS


def chunk(tag, payload):
    return tag + struct.pack("<I", len(payload)) + payload + b"\0" * (-len(payload) % 4)


def digest(value):
    return hashlib.sha256(value).hexdigest()


def number(raw):
    """The shortest decimal that reads back as these exact f32 bits."""
    value = struct.unpack("<f", raw)[0]
    if value != value or value in (float("inf"), float("-inf")):
        raise ValueError("nonfinite gameplay number")
    if raw == b"\0\0\0\x80":
        return -0.0
    for digits in range(1, 10):
        short = float(format(value, f".{digits}g"))
        if struct.pack("<f", short) == raw:
            return int(short) if short == int(short) and abs(short) < 1 << 24 else short
    raise ValueError("f32 needs more than nine digits")


def bits(value):
    return struct.pack("<f", value)


def f32_add(a, b):
    """f32 + f32: a written delta is a decimal, so it becomes its f32 first, as the bake reads it."""
    return struct.unpack("<f", struct.pack("<f", a + struct.unpack("<f", struct.pack("<f", b))[0]))[0]


# ---------- collision ----------

def parse_collision(payload):
    children = list(geometry.chunks(payload))
    tags = [t for t, _ in children]
    if tags not in ([b"NAME", b"NODE", b"INFO", b"POSI", b"TREE"], [b"NAME", b"MASK", b"NODE", b"INFO", b"POSI", b"TREE"]):
        raise ValueError(f"unexpected collision layout {tags}")
    parts = dict(children)
    posi = parts[b"POSI"]
    polygons = []
    for tag, leaf in geometry.chunks(parts[b"TREE"]):
        if tag == b"LEAF":
            polygons.append(list(struct.unpack_from("<" + "H" * leaf[0], leaf, 7)))
    return {"name": text(parts[b"NAME"]).encode() + b"\0", "node": text(parts[b"NODE"]).encode() + b"\0", "mask": parts.get(b"MASK"), "bounds": parts[b"INFO"][16:40],
            "vertices": [posi[i:i + 12] for i in range(0, len(posi), 12)], "polygons": polygons}


def collision_chunk(mesh):
    """The canonical collision resource: what the game reads, with unread tree bytes as zero."""
    leaves = b"".join(chunk(b"LEAF", bytes([len(p)]) + bytes(6) + struct.pack("<" + "H" * len(p), *p)) for p in mesh["polygons"])
    info = struct.pack("<4I", len(mesh["vertices"]), 0, len(mesh["polygons"]), sum(map(len, mesh["polygons"]))) + mesh["bounds"]
    body = chunk(b"NAME", mesh["name"])
    if mesh["mask"] is not None:
        body += chunk(b"MASK", mesh["mask"])
    body += chunk(b"NODE", mesh["node"]) + chunk(b"INFO", info) + chunk(b"POSI", b"".join(mesh["vertices"])) + chunk(b"TREE", leaves)
    return chunk(b"coll", body)


def best_translation(literal, donor):
    """A translation that lands donor vertices on the most literal vertices, bit for bit."""
    if not literal or not donor:
        return None, {}
    wanted = set(literal)
    votes = Counter()
    # Pair a sample of literal vertices with the donor: a real translation shows up again and again.
    sample = literal[:: max(1, len(literal) // 24)][:24]
    points = [struct.unpack("<3f", v) for v in donor[:2000]]
    for raw in sample:
        p = struct.unpack("<3f", raw)
        for q in points:
            votes[(p[0] - q[0], p[1] - q[1], p[2] - q[2])] += 1
    best, best_hits = None, {}
    for delta, _ in votes.most_common(6):
        delta = tuple(struct.unpack("<f", bits(d))[0] for d in delta)
        hits = {}
        for index, raw in enumerate(donor):
            x, y, z = struct.unpack("<3f", raw)
            moved = bits(f32_add(x, delta[0])) + bits(f32_add(y, delta[1])) + bits(f32_add(z, delta[2]))
            if moved in wanted:
                hits.setdefault(moved, index)
        if len(hits) > len(best_hits):
            best, best_hits = delta, hits
    return (best, best_hits) if len(best_hits) >= 8 else (None, {})


def midpoints(donor_mesh):
    """The midpoint of each donor polygon edge, keyed by the bits a split vertex would have."""
    out = {}
    if not donor_mesh:
        return out
    vertices = donor_mesh["vertices"]
    for polygon in donor_mesh["polygons"]:
        for a, b in zip(polygon, polygon[1:] + polygon[:1]):
            pa, pb = struct.unpack("<3f", vertices[a]), struct.unpack("<3f", vertices[b])
            mid = b"".join(bits((u + v) / 2) for u, v in zip(pa, pb))
            out.setdefault(mid, (a, b))
    return out


def encode_vertices(target, donor, stats):
    """Flat op stream: 0,start,count | 1,start,count,dx,dy,dz | 2,index,axes,values... | 3,a,b | 4,x,y,z."""
    from_donor = {}
    for index, raw in enumerate(donor["vertices"] if donor else ()):
        from_donor.setdefault(raw, index)
    missing = [raw for raw in target if raw not in from_donor]
    delta, moved = best_translation(missing, donor["vertices"] if donor else [])
    splits = midpoints(donor)
    pairs = {}
    if donor:
        for index, raw in enumerate(donor["vertices"]):
            for axis in range(3):
                kept = raw[:axis * 4] + raw[axis * 4 + 4:]
                pairs.setdefault((axis, kept), index)
    ops, i = [], 0
    while i < len(target):
        raw = target[i]
        if raw in from_donor:
            start, count = from_donor[raw], 1
            while i + count < len(target) and donor["vertices"][start + count:start + count + 1] == [target[i + count]]:
                count += 1
            ops += [0, start, count]
            stats["vertices_copied"] += count
            i += count
        elif raw in moved:
            start, count = moved[raw], 1
            while i + count < len(target) and target[i + count] in moved and moved[target[i + count]] == start + count:
                count += 1
            ops += [1, start, count, number(bits(delta[0])), number(bits(delta[1])), number(bits(delta[2]))]
            stats["vertices_translated"] += count
            i += count
        else:
            kept = next(((axis, pairs[(axis, raw[:axis * 4] + raw[axis * 4 + 4:])]) for axis in range(3)
                         if (axis, raw[:axis * 4] + raw[axis * 4 + 4:]) in pairs), None)
            if kept:
                ops += [2, kept[1], kept[0], number(raw[kept[0] * 4:kept[0] * 4 + 4])]
                stats["vertices_moved"] += 1
            elif raw in splits:
                ops += [3, *splits[raw]]
                stats["vertices_derived"] += 1
            else:
                ops += [4, number(raw[:4]), number(raw[4:8]), number(raw[8:12])]
                stats["vertices_absolute"] += 1
            i += 1
    return ops


def apply_vertices(ops, donor):
    out, at = [], 0
    vertices = donor["vertices"] if donor else []
    while at < len(ops):
        code = ops[at]
        if code == 0:
            start, count = ops[at + 1:at + 3]
            if start + count > len(vertices):
                raise ValueError("vertex copy outside the donor")
            out += vertices[start:start + count]
            at += 3
        elif code == 1:
            start, count, dx, dy, dz = ops[at + 1:at + 6]
            for raw in vertices[start:start + count]:
                x, y, z = struct.unpack("<3f", raw)
                out.append(bits(f32_add(x, dx)) + bits(f32_add(y, dy)) + bits(f32_add(z, dz)))
            at += 6
        elif code == 2:
            index, axis, value = ops[at + 1:at + 4]
            raw = vertices[index]
            out.append(raw[:axis * 4] + bits(value) + raw[axis * 4 + 4:])
            at += 4
        elif code == 3:
            a, b = struct.unpack("<3f", vertices[ops[at + 1]]), struct.unpack("<3f", vertices[ops[at + 2]])
            out.append(b"".join(bits((u + v) / 2) for u, v in zip(a, b)))
            at += 3
        elif code == 4:
            out.append(bits(ops[at + 1]) + bits(ops[at + 2]) + bits(ops[at + 3]))
            at += 4
        else:
            raise ValueError("unknown vertex op")
    return out


def first_index(vertices):
    out = {}
    for index, raw in enumerate(vertices):
        out.setdefault(raw, index)
    return out


def encode_polygons(target, vertices, donor, stats):
    """Flat op stream: 0,start,count copies donor polygons through the vertices' positions; n,i... is one written polygon."""
    index = first_index(vertices)
    donor_polys = {}
    remapped = []
    if donor:
        for number_, polygon in enumerate(donor["polygons"]):
            mapped = [index.get(donor["vertices"][v]) for v in polygon]
            remapped.append(None if None in mapped else mapped)
            if None not in mapped:
                donor_polys.setdefault(tuple(mapped), number_)
    ops, i = [], 0
    while i < len(target):
        start = donor_polys.get(tuple(target[i]))
        if start is None:
            ops += [len(target[i]), *target[i]]
            stats["polygons_written"] += 1
            i += 1
            continue
        count = 1
        while i + count < len(target) and start + count < len(remapped) and remapped[start + count] == target[i + count]:
            count += 1
        ops += [0, start, count]
        stats["polygons_copied"] += count
        i += count
    return ops


def apply_polygons(ops, vertices, donor):
    index = first_index(vertices)
    out, at = [], 0
    while at < len(ops):
        if ops[at] == 0:
            for polygon in donor["polygons"][ops[at + 1]:ops[at + 1] + ops[at + 2]]:
                out.append([index[donor["vertices"][v]] for v in polygon])
            at += 3
        else:
            out.append(list(ops[at + 1:at + 1 + ops[at]]))
            at += 1 + ops[at]
    return out


def text(raw):
    return raw.split(b"\0", 1)[0].decode("ascii")


def collision(reference, donor_payload, stats):
    """(program, canonical chunk). The program is checked against the reference before it is returned."""
    target = parse_collision(reference)
    donor = parse_collision(donor_payload) if donor_payload is not None else None
    program = {"n": text(target["node"]), "b": [number(target["bounds"][i:i + 4]) for i in range(0, 24, 4)],
               "v": encode_vertices(target["vertices"], donor, stats)}
    if target["mask"] is not None:
        program["m"] = list(target["mask"])
    program["p"] = encode_polygons(target["polygons"], target["vertices"], donor, stats)
    vertices = apply_vertices(program["v"], donor)
    polygons = apply_polygons(program["p"], vertices, donor)
    if vertices != target["vertices"] or polygons != target["polygons"]:
        raise ValueError("collision program does not reproduce the reference")
    return program, collision_chunk(target)


# ---------- skeleton ----------

def parse_skeleton(payload):
    parts = dict(geometry.chunks(payload))
    info = parts[b"INFO"]
    end = info.index(b"\0")
    count = struct.unpack_from("<I", info, end + 1)[0]
    names = parts[b"NAME"].split(b"\0")[:count]
    parents = parts[b"PRNT"].split(b"\0")[:count]
    xfrm = parts[b"XFRM"]
    return {"name": info[:end], "bones": [(names[i], parents[i], xfrm[i * 48:i * 48 + 48]) for i in range(count)]}


def skeleton_chunk(skeleton):
    bones = skeleton["bones"]
    return chunk(b"skel", chunk(b"INFO", skeleton["name"] + b"\0" + struct.pack("<I", len(bones)))
                 + chunk(b"NAME", b"".join(b[0] + b"\0" for b in bones))
                 + chunk(b"PRNT", b"".join(b[1] + b"\0" for b in bones))
                 + chunk(b"XFRM", b"".join(b[2] for b in bones)))


def skeleton(reference, donor_payload, stats):
    """A bone is a donor bone's index, or that bone with a new transform, or its name, parent and numbers."""
    target = parse_skeleton(reference)
    donor = parse_skeleton(donor_payload) if donor_payload is not None else {"bones": []}
    whole = {bone: i for i, bone in reversed(list(enumerate(donor["bones"])))}
    named = {bone[:2]: i for i, bone in reversed(list(enumerate(donor["bones"])))}
    bones = []
    for bone in target["bones"]:
        numbers = [number(bone[2][i:i + 4]) for i in range(0, 48, 4)]
        if bone in whole:
            bones.append(whole[bone])
            stats["bones_copied"] += 1
        elif bone[:2] in named:
            bones.append([named[bone[:2]], numbers])
            stats["bones_moved"] += 1
        else:
            bones.append([text(bone[0] + b"\0"), text(bone[1] + b"\0"), numbers])
            stats["bones_written"] += 1
    return {"name": text(target["name"] + b"\0"), "bones": bones}, skeleton_chunk(target)


# ---------- routes: navigation and paths, as words over the donor's ----------

def words(raw):
    return list(struct.unpack("<" + "I" * (len(raw) // 4), raw[:len(raw) // 4 * 4]))


def route(reference, donor_payload, stats):
    """[0,offset,length] copies donor words, [1,word...] writes them, [2,byte...] ends an unaligned payload."""
    target, donor = words(reference), words(donor_payload) if donor_payload is not None else []
    ops = []
    for code, a0, a1, b0, b1 in SequenceMatcher(None, donor, target, autojunk=False).get_opcodes():
        if code == "equal":
            ops.append([0, a0, a1 - a0])
            stats["route_words_copied"] += a1 - a0
        elif b1 > b0:
            ops.append([1, *target[b0:b1]])
            stats["route_words_written"] += b1 - b0
    tail = reference[len(target) * 4:]
    if tail:
        ops.append([2, *tail])
    rebuilt = b""
    for op in ops:
        if op[0] == 0:
            rebuilt += struct.pack("<" + "I" * op[2], *donor[op[1]:op[1] + op[2]])
        elif op[0] == 1:
            rebuilt += struct.pack("<" + "I" * (len(op) - 1), *op[1:])
        else:
            rebuilt += bytes(op[1:])
    if rebuilt != reference:
        raise ValueError("route program does not reproduce the reference")
    return ops


# ---------- animation banks ----------

def parse_bank(payload):
    children = list(geometry.chunks(payload, clipped_tail=True))
    if [t for t, _ in children] != [b"NAME", b"BIN_"]:
        raise ValueError("unexpected animation bank layout")
    name, data = children[0][1], children[1][1]
    _, size, version, tracks, tada_size, count, spare = struct.unpack_from("<4sIIIIHH", data)
    sections = dict(geometry.chunks(data[24:], clipped_tail=True))
    mina, tnja, tada = sections[b"MINA"], sections[b"TNJA"], sections[b"TADA"][:tada_size]
    clips, first, seen = [], 0, Counter()
    for k in range(count):
        mask, crc, frames, bones = struct.unpack_from("<QIHH", mina, k * 16)
        rows = []
        for j in range(bones):
            rec = tnja[(first + j) * 48:(first + j + 1) * 48]
            rows.append({"bone": struct.unpack_from("<I", rec)[0], "offsets": list(struct.unpack_from("<7I", rec, 4)),
                         "base": rec[32:44], "scale": rec[44:48]})
        clips.append({"mask": mask, "crc": crc, "ordinal": seen[crc], "frames": frames, "tracks": rows})
        seen[crc] += 1
        first += bones
    return {"name": name, "size": size, "version": version, "spare": spare, "tada": tada, "clips": clips}


def channels(bank, clip, track):
    """All seven decoded streams; a translation stream the clip's mask leaves unused decodes as None."""
    index = clip["tracks"].index(track)
    out = []
    for slot, offset in enumerate(track["offsets"]):
        if slot >= 4 and not (index < 64 and clip["mask"] >> index & 1):
            out.append(None)
        else:
            out.append(geometry.stream(bank["tada"], offset, clip["frames"]))
    return out


def pack_stream(values):
    if not values:
        return b""
    out = bytearray(struct.pack("<h", values[0]))
    current, at = values[0], 1
    while at < len(values):
        run = 0
        while at + run < len(values) and values[at + run] == current and run < 255:
            run += 1
        if run >= 2:
            out += bytes([0x80, run])
            at += run
            continue
        step = values[at] - current
        if -126 <= step <= 127:
            out += struct.pack("<b", step)
        else:
            out += b"\x81" + struct.pack("<h", values[at])
        current = values[at]
        at += 1
    return bytes(out)


def run_lengths(values):
    """value,count pairs; the frame count is known, so a constant stream is two numbers."""
    if values is None:
        return []
    out = []
    for value in values:
        if out and out[-2] == value:
            out[-1] += 1
        else:
            out += [value, 1]
    return out


def from_runs(runs):
    out = []
    for value, count in zip(runs[::2], runs[1::2]):
        out += [value] * count
    return out


def track_numbers(track, streams):
    return [track["bone"], number(track["base"][:4]), number(track["base"][4:8]), number(track["base"][8:12]),
            number(track["scale"]), [run_lengths(s) for s in streams]]


def bank(reference, donor_payload, stats):
    """(program, rebuilt chunk). Donor clips stay; root tracks and clips of another shape come from the program."""
    target = parse_bank(reference)
    donor = parse_bank(donor_payload) if donor_payload is not None else None
    by_identity = {(c["crc"], c["ordinal"]): i for i, c in enumerate(donor["clips"])} if donor else {}
    clips = []
    for clip in target["clips"]:
        slot = by_identity.get((clip["crc"], clip["ordinal"]))
        other = donor["clips"][slot] if slot is not None else None
        if other is None or other["frames"] != clip["frames"] or [t["bone"] for t in other["tracks"]] != [t["bone"] for t in clip["tracks"]]:
            clips.append({"mask": [clip["mask"] & 0xffffffff, clip["mask"] >> 32], "crc": clip["crc"], "frames": clip["frames"],
                          "tracks": [track_numbers(t, channels(target, clip, t)) for t in clip["tracks"]]})
            stats["clips_written"] += 1
            continue
        patches, mask = [], other["mask"]
        for index, track in enumerate(clip["tracks"]):
            if track["bone"] not in ROOTS:
                continue
            bit = 1 << index if index < 64 else 0
            mask = (mask & ~bit) | (clip["mask"] & bit)
            theirs = other["tracks"][index]
            streams = channels(target, clip, track)
            if (track["base"], track["scale"], streams) != (theirs["base"], theirs["scale"], channels(donor, other, theirs)):
                patches.append([index, *track_numbers(track, streams)[1:]])
                stats["root_tracks_written"] += 1
        row = {"from": slot}
        if mask != other["mask"]:
            row["mask"] = [mask & 0xffffffff, mask >> 32]
        if patches:
            row["roots"] = patches
        clips.append(row)
        stats["clips_copied"] += 1
    program = {"name": text(target["name"]), "size": target["size"], "version": target["version"], "spare": target["spare"], "clips": clips}
    return program, bank_chunk(program, donor)


def bank_chunk(program, donor):
    """The same rebuild as program.rs: donor TADA first, then each written stream in program order."""
    tada = bytearray(donor["tada"] if donor else b"")
    mina, tnja = bytearray(), bytearray()

    def put(streams, frames):
        offsets = []
        for runs in streams:
            values = from_runs(runs)
            if not values:
                offsets.append(0)
                continue
            if len(values) != frames:
                raise ValueError("stream length differs from the clip's frames")
            offsets.append(len(tada))
            tada.extend(pack_stream(values))
        return offsets

    def record(bone, offsets, base, scale):
        tnja.extend(struct.pack("<I", bone) + struct.pack("<7I", *offsets) + base + scale)

    for clip in program["clips"]:
        if "from" in clip:
            source = donor["clips"][clip["from"]]
            mask = source["mask"] if "mask" not in clip else clip["mask"][0] | clip["mask"][1] << 32
            roots = {row[0]: row for row in clip.get("roots", ())}
            for index, track in enumerate(source["tracks"]):
                if index in roots:
                    row = roots[index]
                    record(track["bone"], put(row[5], source["frames"]), bits(row[1]) + bits(row[2]) + bits(row[3]), bits(row[4]))
                else:
                    record(track["bone"], track["offsets"], track["base"], track["scale"])
            mina.extend(struct.pack("<QIHH", mask, source["crc"], source["frames"], len(source["tracks"])))
        else:
            mask = clip["mask"][0] | clip["mask"][1] << 32
            for row in clip["tracks"]:
                record(row[0], put(row[5], clip["frames"]), bits(row[1]) + bits(row[2]) + bits(row[3]), bits(row[4]))
            mina.extend(struct.pack("<QIHH", mask, clip["crc"], clip["frames"], len(clip["tracks"])))
    tracks = len(tnja) // 48
    header = struct.pack("<4sIIIIHH", b"SMNA", program["size"], program["version"], tracks, len(tada), len(program["clips"]), program["spare"])
    data = header + chunk(b"MINA", bytes(mina)) + chunk(b"TNJA", bytes(tnja)) + chunk(b"TADA", bytes(tada))
    return chunk(b"zaa_", chunk(b"NAME", program["name"].encode() + b"\0") + chunk(b"BIN_", data))
