#!/usr/bin/env python3
"""Names/numbers-only geometry census using the shipped LVL schemas.

Used by version_census. No vertices, polygons, height grids or key streams are
returned: SHA-256 fingerprints identify decoded numeric streams. Mesh counts are
AUTHORED polygon-fan counts, before the port's signed-fan repair. Animation root
translation samples use the same float32 operations and root names as bake.
"""

from __future__ import annotations

import hashlib
import math
import struct
import re
from functools import lru_cache
from pathlib import Path
from collections import Counter


class GeometryError(ValueError):
    pass


def chunks(buf, *, clipped_tail=False):
    at = 0
    while at + 8 <= len(buf):
        tag, size = struct.unpack_from("<4sI", buf, at)
        end = at + 8 + size
        if end > len(buf) and not clipped_tail:
            raise GeometryError("truncated chunk")
        yield tag, buf[at + 8 : min(end, len(buf))]
        at = (end + 3) & ~3
    if not clipped_tail and at < len(buf) and any(buf[at:]):
        raise GeometryError("nonzero trailing bytes")


def child(buf, tag, default=None):
    return next((v for t, v in chunks(buf) if t == tag), default)


def name(buf):
    if buf is None or b"\0" not in buf:
        raise GeometryError("missing terminated name")
    value = buf.split(b"\0", 1)[0].decode("ascii")
    if any(ord(c) < 32 for c in value):
        raise GeometryError("invalid name")
    return value.lower()


def floats(buf, count=None):
    if count is None:
        if len(buf) % 4:
            raise GeometryError("unaligned float array")
        count = len(buf) // 4
    out = list(struct.unpack_from("<" + "f" * count, buf))
    if not all(math.isfinite(x) for x in out):
        raise GeometryError("nonfinite number")
    return [0.0 if x == 0 else x for x in out]


def digest_numbers(values, fmt="f"):
    h = hashlib.sha256()
    for value in values:
        h.update(struct.pack("<" + fmt, value))
    return h.hexdigest()


def fingerprint(buf):
    return hashlib.sha256(buf).hexdigest()


def collision(buf):
    info = child(buf, b"INFO")
    if info is None or len(info) != 40:
        raise GeometryError("collision INFO must be 40 bytes")
    vertices, nodes, leaves, indices = struct.unpack_from("<4I", info)
    pos = child(buf, b"POSI")
    if pos is None or len(pos) != vertices * 12:
        raise GeometryError("collision vertex count")
    xyz = floats(pos)
    tree = child(buf, b"TREE")
    if tree is None:
        raise GeometryError("collision TREE absent")
    polys = []
    nn = 0
    for tag, payload in chunks(tree):
        if tag == b"NODE":
            nn += 1
        elif tag == b"LEAF":
            if not payload or len(payload) != 7 + payload[0] * 2:
                raise GeometryError("collision LEAF size")
            poly = list(struct.unpack_from("<" + "H" * payload[0], payload, 7))
            if any(i >= vertices for i in poly):
                raise GeometryError("collision index outside vertices")
            polys.append(poly)
    if (nn, len(polys), sum(map(len, polys))) != (nodes, leaves, indices):
        raise GeometryError("collision tree counts")
    # Preserve topology and vertex order; these are decoded representation
    # differences, not a claim about geometric equivalence under reindexing.
    topology = b"".join(
        struct.pack("<I", len(p)) + struct.pack("<" + "H" * len(p), *p) for p in polys
    )
    points = [tuple(xyz[i * 3 : i * 3 + 3]) for i in range(vertices)]
    return [
        {
            "name": name(child(buf, b"NAME")),
            "node": name(child(buf, b"NODE", b"\0")),
            "vertices": vertices,
            "polygons": leaves,
            "fan_triangles": sum(max(len(p) - 2, 0) for p in polys),
            "bounds": floats(info[16:]),
            "positions_sha256": digest_numbers(xyz),
            "topology_sha256": fingerprint(topology),
            "surface_sha256": surface_digest(points, polys),
            "vertex_set_sha256": digest_numbers(v for p in sorted(set(points)) for v in p),
        }
    ]


def surface_digest(points, polys):
    """The authored polygons as positions, blind to vertex numbering, polygon order and fan start.

    Equal digests mean the same polygons in space whatever the munge's ordering. Winding stays significant.
    """
    canonical = []
    for poly in polys:
        corners = [points[i] for i in poly]
        start = min(range(len(corners)), key=corners.__getitem__) if corners else 0
        canonical.append(corners[start:] + corners[:start])
    canonical.sort()
    return digest_numbers(v for corners in canonical for corner in corners for v in corner)


def collision_positions(buf):
    """(name, node, positions) for a transient shape comparison; never written to a census."""
    pos = child(buf, b"POSI")
    if pos is None or len(pos) % 12:
        raise GeometryError("collision vertex count")
    xyz = floats(pos)
    return (
        name(child(buf, b"NAME")),
        name(child(buf, b"NODE", b"\0")),
        [tuple(xyz[i : i + 3]) for i in range(0, len(xyz), 3)],
    )


def primitives(buf):
    info = child(buf, b"INFO")
    model = name(info)
    end = info.index(0) + 1
    count = struct.unpack_from("<I", info, end)[0]
    out = []
    pending = None
    mask = 0
    for tag, p in chunks(buf):
        if tag == b"NAME":
            if pending is not None:
                raise GeometryError("unfinished primitive")
            pending = {
                "name": name(p),
                "model": model,
                "parent": "",
                "mask_own": 0,
                "transform": [
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                ],
            }
        elif tag == b"MASK":
            if len(p) != 1 or pending is None:
                raise GeometryError("primitive mask")
            mask = p[0]
            pending["mask_own"] = 1
        elif tag == b"PRNT" and pending is not None:
            pending["parent"] = name(p)
        elif tag == b"XFRM" and pending is not None:
            if len(p) != 48:
                raise GeometryError("primitive transform")
            pending["transform"] = floats(p)
        elif tag == b"DATA":
            if pending is None or len(p) != 16:
                raise GeometryError("primitive DATA")
            pending.update(
                kind=struct.unpack_from("<I", p)[0], dimensions=floats(p[4:]), mask=mask
            )
            out.append(pending)
            pending = None
    if pending is not None or len(out) != count:
        raise GeometryError("primitive count")
    return out


def world(buf):
    world_name = name(child(buf, b"NAME"))
    out = []
    for tag, rec in chunks(buf):
        if tag not in (b"inst", b"regn", b"Hint", b"BARR"):
            continue
        info = child(rec, b"INFO")
        if info is None:
            raise GeometryError("world record INFO absent")
        xfrm = child(info, b"XFRM")
        if xfrm is None or len(xfrm) != 48:
            raise GeometryError("world record transform")
        props = []
        for pt, pv in chunks(rec):
            if pt == b"PROP":
                # Hash rather than retain arbitrary PROP strings: they can be
                # localized prose. Hash changes do not claim numeric changes.
                if len(pv) < 5 or b"\0" not in pv[4:]:
                    raise GeometryError("world property")
                props.append(
                    [
                        struct.unpack_from("<I", pv)[0],
                        fingerprint(pv[4:].split(b"\0", 1)[0]),
                    ]
                )
        row = {
            "world": world_name,
            "kind": tag.decode(),
            "name": name(child(info, b"NAME", b"\0")),
            "class": name(child(info, b"TYPE", b"\0")),
            "transform": floats(xfrm),
            "properties": props,
        }
        size = child(info, b"SIZE")
        if size is not None:
            row["size"] = floats(size, 3)
        if tag == b"BARR":
            flag = child(info, b"FLAG", b"\0" * 4)
            row["flags"] = struct.unpack_from("<I", flag)[0]
        out.append(row)
    return out


def rle(buf, expected):
    out = bytearray()
    at = 0
    while at < len(buf):
        c = buf[at]
        at += 1
        n = (c & 127) + 1
        used = 1 if c & 128 else n
        if at + used > len(buf) or len(out) + n > expected:
            raise GeometryError("height RLE bounds")
        out.extend(buf[at : at + 1] * n if c & 128 else buf[at : at + n])
        at += used
    if len(out) != expected:
        raise GeometryError("height RLE count")
    return out


def terrain(buf, *, retain_samples=False):
    title = name(child(buf, b"NAME"))
    info = child(buf, b"INFO")
    hgt = child(buf, b"HGT8")
    if info is None and hgt is None:
        return [{"name": title, "heightfield": 0}]
    if info is None or hgt is None or len(info) < 28:
        raise GeometryError("unsupported terrain height layout")
    unit, scale, floor, ceiling = floats(info[:16])
    n = struct.unpack_from("<H", info, 16)[0]
    if n == 0 or n % 16 or n > 1024:
        raise GeometryError("terrain grid size")
    h8 = rle(hgt, n * n)
    hexp = child(buf, b"HEXP")
    if hexp is None or len(hexp) != n * n // 256 * 8:
        raise GeometryError("terrain HEXP size")
    raw = []
    for z in range(n):
        for x in range(n):
            patch = (x >> 4) + (z >> 4) * (n // 16)
            shift = hexp[patch * 8]
            if shift > 24:
                raise GeometryError("terrain shift")
            offset = struct.unpack_from("<h", hexp, patch * 8 + 2)[0]
            raw.append(
                (h8[(x & 15) + ((z & 15) << 4) + (patch << 8)] << shift) + offset
            )
    ys = [f32(v * scale) for v in raw]
    # CUTR contains actual plane sets; expose count and digest, never planes.
    cuts = child(buf, b"CUTS", b"")
    cut_records = [p for t, p in chunks(cuts) if t == b"CUTR"]
    cutter_values = []
    for rec in cut_records:
        ci, planes = child(rec, b"INFO"), child(rec, b"PLNS")
        if ci is None or len(ci) < 20 or planes is None:
            raise GeometryError("terrain cutter metadata")
        count = struct.unpack_from("<I", ci)[0]
        if len(planes) != count * 16:
            raise GeometryError("terrain cutter plane count")
        cutter_values.append(float(count))
        cutter_values.extend(floats(ci[4:20]))
        cutter_values.extend(floats(planes))
    row = {
        "name": title,
        "heightfield": 1,
        "grid_size": n,
        "grid_unit": unit,
        "height_scale": scale,
        "samples": n * n,
        "min_height": min(ys),
        "max_height": max(ys),
        "heights_sha256": digest_numbers(ys),
        "cutters": len(cut_records),
        "cutters_sha256": digest_numbers(cutter_values),
    }
    if retain_samples:
        row["_samples"] = ys
    return [row]


def f32(v):
    return struct.unpack("<f", struct.pack("<f", v))[0]


def crc32_lower(s):
    crc = 0xFFFFFFFF
    for b in s.lower().encode("ascii"):
        crc ^= b << 24
        for _ in range(8):
            crc = ((crc << 1) ^ (0x04C11DB7 if crc & 0x80000000 else 0)) & 0xFFFFFFFF
    return (~crc) & 0xFFFFFFFF


def stream(buf, off, count):
    if count == 0:
        return []
    if off + 2 > len(buf):
        raise GeometryError("animation stream start")
    value = struct.unpack_from("<h", buf, off)[0]
    at = off + 2
    out = [value]
    while len(out) < count:
        if at >= len(buf):
            raise GeometryError("animation stream truncated")
        op = buf[at]
        at += 1
        if op == 128:
            if at >= len(buf) or buf[at] == 0:
                raise GeometryError("animation hold")
            out.extend([value] * min(buf[at], count - len(out)))
            at += 1
        elif op == 129:
            if at + 2 > len(buf):
                raise GeometryError("animation absolute")
            value = struct.unpack_from("<h", buf, at)[0]
            at += 2
            out.append(value)
        else:
            delta = op if op < 128 else op - 256
            value = ((value + delta + 32768) % 65536) - 32768
            out.append(value)
    return out


ROOTS = {crc32_lower(n): n for n in ("bone_root", "DummyRoot", "root")}


@lru_cache(maxsize=1)
def known_animation_names():
    """Resolve CRCs only against literal names already present in source.

    No brute-force name generation. CRC matches are candidates, not recovery of
    discarded strings. Ambiguous CRCs stay NM. The source-only lookup is safe
    to run in an install without any proprietary corpus.
    """
    root = Path(__file__).resolve().parents[1]
    paths = []
    for rel in (
        "engine/crates/bake/src",
        "engine/crates/manifest/src",
        "engine/crates/bake/tests/data",
    ):
        base = root / rel
        paths.extend(base.rglob("*.rs"))
        paths.extend(base.rglob("*.json"))
    paths += list((root / "analysis").glob("re-anim*.md"))
    paths += list((root / "analysis").glob("re-soldier-anim*.md"))
    candidates = {}
    for path in paths:
        for token in re.findall(r'["`\']([A-Za-z0-9_]+)["`\']', path.read_text()):
            token = token.lower()
            candidates.setdefault(crc32_lower(token), set()).add(token)
    return {
        crc: next(iter(names)) for crc, names in candidates.items() if len(names) == 1
    }


def animations(buf, game):
    bank = name(child(buf, b"NAME"))
    data = child(buf, b"BIN_")
    if data is None or len(data) < 24 or data[:4] != b"SMNA":
        raise GeometryError("unsupported animation bank header")
    version, tracks, tada_size, count = struct.unpack_from("<IIIH", data, 8)
    if version != 8:
        raise GeometryError("unsupported animation version")
    sections = dict(chunks(data[24:], clipped_tail=True))
    mina, tnja, tada = (sections.get(t, b"") for t in (b"MINA", b"TNJA", b"TADA"))
    if len(mina) < count * 16 or len(tnja) < tracks * 48 or len(tada) < tada_size:
        raise GeometryError("animation section size")
    tada = tada[:tada_size]
    out = []
    first = 0
    for k in range(count):
        mask, crc, frames, bones = struct.unpack_from("<QIHH", mina, k * 16)
        if first + bones > tracks:
            raise GeometryError("animation track count")
        roots = []
        for j in range(bones):
            rec = tnja[(first + j) * 48 : (first + j + 1) * 48]
            bone = struct.unpack_from("<I", rec)[0]
            if bone not in ROOTS:
                continue
            offsets = struct.unpack_from("<3I", rec, 20)
            base = floats(rec[32:44])
            scale = floats(rec[44:48])[0]
            if j < 64 and mask & (1 << j):
                chans = [stream(tada, o, frames) for o in offsets]
                positions = [
                    [f32(base[a] + f32(scale * chans[a][f])) for a in range(3)]
                    for f in range(frames)
                ]
            else:
                positions = [base] * frames
            delta = (
                [f32(positions[-1][a] - positions[0][a]) for a in range(3)]
                if positions
                else [0.0, 0.0, 0.0]
            )
            roots.append(
                {
                    "name": ROOTS[bone],
                    "crc": bone,
                    "delta": delta,
                    "samples": frames,
                    "translations_sha256": digest_numbers(
                        v for p in positions for v in p
                    ),
                }
            )
        out.append(
            {
                "bank": bank,
                "name_crc": crc,
                "name": known_animation_names().get(crc, "NM"),
                "name_resolution": (
                    "source_crc_match" if crc in known_animation_names() else "NM"
                ),
                "frames": frames,
                "bones": bones,
                "duration_seconds": (frames - (game == "bf2")) / 30 if frames else 0,
                "roots": roots,
            }
        )
        first += bones
    if first != tracks:
        raise GeometryError("animation summed tracks")
    return out


def route(tag):
    """A whole planning or path resource, by fingerprint: the sim steers and spawns on them, a differing byte counts."""

    def parse(buf):
        return [{"name": tag.decode(), "bytes": len(buf), "sha256": fingerprint(buf)}]

    return parse


def animation_registry_digest():
    """The resolved names a census used: they come from this checkout's source, not from the input."""
    names = sorted(known_animation_names().values())
    return hashlib.sha256("\n".join(names).encode()).hexdigest()


PARSERS = {
    b"plan": ("routes", route(b"plan")),
    b"path": ("routes", route(b"path")),
    b"PATH": ("routes", route(b"PATH")),
    b"coll": ("collision", collision),
    b"prim": ("primitives", primitives),
    b"wrld": ("world", world),
    b"tern": ("terrain", terrain),
}


def parse_resource(tag, payload, game):
    """Return (category, rows), or None for non-geometry resources.

    Caller must retain file, sublevel scope, occurrence and parser errors. Only
    successful categories may enter a comparison denominator.
    """
    if tag == b"zaa_":
        return "animations", animations(payload, game)
    if tag in PARSERS:
        category, parser = PARSERS[tag]
        return category, parser(payload)
    return None


IDENTITY_FIELDS = {
    "collision": ("name", "node"),
    "primitives": ("model", "name"),
    "world": ("world", "kind", "name"),
    "routes": ("name",),
    "terrain": ("name",),
    "animations": ("bank", "name_crc"),
}
COMPARE_FIELDS = {
    "collision": (
        "vertices",
        "polygons",
        "fan_triangles",
        "bounds",
        "positions_sha256",
        "topology_sha256",
        "surface_sha256",
        "vertex_set_sha256",
    ),
    "primitives": ("parent", "transform", "kind", "dimensions", "mask"),
    "world": ("class", "transform", "properties", "size", "flags"),
    "routes": ("sha256",),
    "terrain": (
        "heightfield",
        "grid_size",
        "grid_unit",
        "height_scale",
        "samples",
        "heights_sha256",
        "cutters",
        "cutters_sha256",
    ),
    "animations": ("frames", "bones", "roots"),
}


def indexed_rows(rows, category):
    """Keep each file/sublevel occurrence; never collapse local overrides."""
    counts = Counter()
    out = {}
    for row in rows:
        key = (row["file"], tuple(row["scope"])) + tuple(
            row.get(f) for f in IDENTITY_FIELDS[category]
        )
        # Unnamed world instances have no stable ordinal across platforms.
        # Match the exact class/transform multiset; moves become unmatched,
        # never spurious comparisons against whichever row follows an insertion.
        if category == "world":
            key += (
                (row["class"], tuple(row["transform"]))
                if not row.get("name")
                else ("", ())
            )
        index = counts[key]
        counts[key] += 1
        out[key + (index,)] = row
    return out


def compare_geometry(left, right):
    """Compare category -> rows maps produced by parse_resource.

    Returned differences are names, scalars, bounds and hashes only. Ordinal
    matching of duplicate named records is order-sensitive. Unnamed world
    records match exact class/transforms; moved objects remain unmatched.
    """
    result = {}
    for category, fields in COMPARE_FIELDS.items():
        a = indexed_rows(left.get(category, []), category)
        b = indexed_rows(right.get(category, []), category)
        keys = sorted(a.keys() & b.keys())
        differences = []
        counts = Counter()
        for key in keys:
            changed = [f for f in fields if a[key].get(f) != b[key].get(f)]
            counts.update(changed)
            if changed:
                differences.append(
                    {
                        "identity": key,
                        "fields": changed,
                        "left": {f: a[key].get(f) for f in changed},
                        "right": {f: b[key].get(f) for f in changed},
                    }
                )
        result[category] = {
            "compared": len(keys),
            "changed": len(differences),
            "field_changes": dict(counts),
            "left_only": [k for k in sorted(a.keys() - b.keys())],
            "right_only": [k for k in sorted(b.keys() - a.keys())],
            "differences": differences,
        }
        if category == "collision":
            result[category]["surface_changes"] = collision_changes(a, b, keys)
        if category == "world":
            result[category]["by_kind"] = {
                kind: {
                    "compared": sum(a[k]["kind"] == kind for k in keys),
                    "changed": sum(
                        a[k]["kind"] == kind
                        and any(a[k].get(f) != b[k].get(f) for f in fields)
                        for k in keys
                    ),
                    "left_only": sum(a[k]["kind"] == kind for k in a.keys() - b.keys()),
                    "right_only": sum(
                        b[k]["kind"] == kind for k in b.keys() - a.keys()
                    ),
                }
                for kind in ("inst", "regn", "Hint", "BARR")
            }
        if category == "animations":

            def motion(row):
                return [r for r in row["roots"] if r["name"] == "DummyRoot"]

            root_keys = [k for k in keys if motion(a[k]) and motion(b[k])]
            result[category]["root_motion_compared"] = len(root_keys)
            result[category]["root_motion_changed"] = sum(
                motion(a[k]) != motion(b[k]) for k in root_keys
            )
            result[category]["root_motion_NM"] = len(keys) - len(root_keys)
    return result


def collision_changes(a, b, keys):
    """Split changed meshes by what actually differs, and name the largest bounds moves."""
    out = {"order_only": 0, "same_vertices_other_polygons": 0, "different_vertices": 0}
    moves = []
    for key in keys:
        x, y = a[key], b[key]
        if x.get("positions_sha256") == y.get("positions_sha256") and (
            x.get("topology_sha256") == y.get("topology_sha256")
        ):
            continue
        if x.get("surface_sha256") == y.get("surface_sha256"):
            out["order_only"] += 1
        elif x.get("vertex_set_sha256") == y.get("vertex_set_sha256"):
            out["same_vertices_other_polygons"] += 1
        else:
            out["different_vertices"] += 1
        delta = max(abs(u - v) for u, v in zip(x["bounds"], y["bounds"]))
        if delta:
            moves.append([key[0], x["name"], delta])
    out["bounds_changed"] = len(moves)
    out["largest_bounds_moves"] = sorted(moves, key=lambda r: (-r[2], r[0], r[1]))[:10]
    return out


def compare_height_samples(left, right):
    """Compare transient decoded grids on exactly matching world coordinates.

    Inputs are terrain rows with retain_samples=True. Raw grids never enter the
    returned census; differing grid spacings are NM (no interpolation guess).
    """
    a = indexed_rows(left, "terrain")
    b = indexed_rows(right, "terrain")
    out = []
    for key in sorted(a.keys() & b.keys()):
        x, y = a[key], b[key]
        row = {"identity": key}
        if not x["heightfield"] or not y["heightfield"]:
            row.update(
                compared=0,
                changed=0,
                left_only=x.get("samples", 0),
                right_only=y.get("samples", 0),
            )
        elif x["grid_unit"] != y["grid_unit"]:
            row.update(compared="NM", changed="NM", reason="different_grid_spacing")
        else:
            n, m = x["grid_size"], y["grid_size"]
            offset = (m - n) // 2
            differences = []
            for z in range(n):
                for i in range(n):
                    j, k = i + offset, z + offset
                    if 0 <= j < m and 0 <= k < m:
                        differences.append(
                            abs(x["_samples"][z * n + i] - y["_samples"][k * m + j])
                        )
            row.update(
                compared=len(differences),
                changed=sum(v != 0 for v in differences),
                max_abs_metres=max(differences, default=0),
                mean_abs_metres=(
                    sum(differences) / len(differences) if differences else 0
                ),
                rms_metres=(
                    math.sqrt(sum(v * v for v in differences) / len(differences))
                    if differences
                    else 0
                ),
                above_1mm=sum(v > 0.001 for v in differences),
                above_1cm=sum(v > 0.01 for v in differences),
                above_10cm=sum(v > 0.1 for v in differences),
                above_1m=sum(v > 1 for v in differences),
                left_grid_size=n,
                right_grid_size=m,
                left_only=n * n - len(differences),
                right_only=m * m - len(differences),
            )
        out.append(row)
    return out
