#!/usr/bin/env python3
"""Where do two editions' terrain heights differ, relative to the ground people play on?

    python3 scripts/version_terrain_play.py REFERENCE_INSTALL OTHER_INSTALL BAKED_DIR --out result.json
        [--game bf1] [--files yav/yav1.lvl ...]

REFERENCE_INSTALL and OTHER_INSTALL are installs/discs version_census accepts. BAKED_DIR is a baked asset folder
holding the reference's <game>_<map>/manifest.ron (hubs, arcs, spawn paths, command posts, cutters). Everything is
read only. The output holds names, counts, magnitudes and bounding boxes; decoded grids stay in memory.

Each differing grid vertex lands in exactly one class, tested in this order:
  cut_both          inside a terrain cutter at both heights: neither version has ground there
  cut_one           inside a cutter at one of the two heights: one version has ground, the other a hole
  unpainted         every render patch touching it has an empty layer mask in the reference: nothing is drawn there
  on_nav            on a hub disc or within ON_NAV_M of a walk arc, spawn node or command post
  near_nav          within NEAR_NAV_M of the same features
  far_painted       drawn, but further than that from every authored ground feature

Each class also counts `under_object`: the vertex lies inside the world bounding box of a placed collision mesh or
primitive, so that object and not the terrain may be the surface there. A bounding box over-covers, so this is an
upper bound on the hidden share and `exposed = samples - under_object` a lower bound on the exposed share.

`in_play_exposed` sums what is left after every exclusion: not cut, drawn, inside the map's boundary path, and with
no placed object over it. `in_play_under_object` is the same with a placed object over it: not proof the ground is
hidden, since the box over-covers. `outside_boundary` sums the vertices beyond that path, whatever their class.

The separate `play_lines` section does not depend on those classes: it evaluates the sim's terrain height query
(core::heightfield: two triangles per cell, the diagonal alternating by row) under both grids along every planning
arc, and at each spawn node and command post.
"""

import argparse
from collections import Counter
import json
import math
from pathlib import Path
import re
import struct
import sys

import version_census as census
import version_geometry as geometry

ON_NAV_M = 8.0
NEAR_NAV_M = 32.0
ARC_STEP_M = 1.0
THRESHOLDS = (("above_1cm", 0.01), ("above_10cm", 0.1), ("above_50cm", 0.5), ("above_1m", 1.0), ("above_5m", 5.0))
CLASSES = ("cut_both", "cut_one", "unpainted", "on_nav", "near_nav", "far_painted")
# AISizeType bits on a planning arc (manifest::ManifestNavArc).
SOLDIER, FLYER = 1, 32
# game::layout::PROP_CAPTURE_REGION: an instance carrying it is a command post.
CAPTURE_REGION = 0xC6C161F9


# ---- a small RON reader: enough for baked manifests, which are plain derive(Serialize) output ----

TOKEN = re.compile(
    r'\s*(?:(//[^\n]*)|("(?:[^"\\]|\\.)*")|([()\[\]{},:])|((?!(?:inf|NaN)\b)[A-Za-z_][A-Za-z0-9_]*)'
    r"|([-+]?(?:[0-9][0-9_]*(?:\.[0-9]*)?(?:[eE][-+]?[0-9]+)?|inf)|NaN))"
)


def parse_ron(text):
    tokens = []
    at = 0
    while at < len(text):
        match = TOKEN.match(text, at)
        if not match:
            if not text[at:].strip():
                break
            raise ValueError(f"RON: unexpected text at {at}")
        at = match.end()
        if match.group(1):
            continue
        if match.group(2):
            tokens.append(("s", match.group(2)[1:-1]))
        elif match.group(3):
            tokens.append(("p", match.group(3)))
        elif match.group(4):
            tokens.append(("i", match.group(4)))
        else:
            tokens.append(("n", float(match.group(5))))
    pos = [0]

    def value():
        kind, v = tokens[pos[0]]
        pos[0] += 1
        if kind in ("s", "n"):
            return v
        if kind == "i":
            if v in ("true", "false"):
                return v == "true"
            if v == "None":
                return None
            if tokens[pos[0]] == ("p", "("):
                pos[0] += 1
                body = sequence(")")
                return body[0] if v == "Some" and isinstance(body, list) and len(body) == 1 else {v: body}
            return v
        return sequence({"[": "]", "(": ")", "{": "}"}[v])

    def sequence(end):
        items, fields = [], {}
        while tokens[pos[0]] != ("p", end):
            if tokens[pos[0]][0] == "i" and tokens[pos[0] + 1] == ("p", ":"):
                key = tokens[pos[0]][1]
                pos[0] += 2
                fields[key] = value()
            else:
                item = value()
                if tokens[pos[0]] == ("p", ":"):
                    pos[0] += 1
                    fields[item] = value()
                else:
                    items.append(item)
            if tokens[pos[0]] == ("p", ","):
                pos[0] += 1
        pos[0] += 1
        return fields if fields else items

    return value()


# ---- the terrain ----


def read_terrains(source, files):
    """{file: (terrain row with _samples, render patch masks or None)} for tern resources."""
    out = {}
    for file, buf in census.input_files(source, set(files)):
        if buf[:4] != b"ucfb":
            continue
        for _scope, tag, payload, _offset in census.walk_resources(buf):
            if tag != b"tern":
                continue
            row = geometry.terrain(payload, retain_samples=True)[0]
            if row["heightfield"]:
                out[file] = (row, patch_masks(payload))
    return out


def patch_masks(tern):
    """PCHS -> PTCH -> INFO's layer mask per 8-cell render patch, row-major (mungefs::terrain)."""
    pchs = geometry.child(tern, b"PCHS")
    if pchs is None:
        return None
    masks = []
    for tag, patch in geometry.chunks(pchs):
        if tag == b"PTCH":
            info = geometry.child(patch, b"INFO", b"")
            masks.append(struct.unpack_from("<H", info)[0] if len(info) >= 14 else 0)
    return masks


class Grid:
    """World frame of the sim's heightfield: x = ix*u - n/2*u, z = iz*u - (n/2-1)*u (mungefs::terrain)."""

    def __init__(self, row):
        self.n = row["grid_size"]
        self.unit = row["grid_unit"]
        self.heights = row["_samples"]
        self.offset_x = self.n // 2 * self.unit
        self.offset_z = (self.n // 2 - 1) * self.unit

    def world(self, ix, iz):
        return ix * self.unit - self.offset_x, iz * self.unit - self.offset_z

    def covers(self, x, z):
        top = (self.n - 1) * self.unit
        return 0 <= x + self.offset_x <= top and 0 <= z + self.offset_z <= top

    def height(self, x, z):
        """core::heightfield::height_at."""
        n, unit = self.n, self.unit
        top = (n - 1) * unit
        fx = min(max(x + self.offset_x, 0.0), top) / unit
        fz = min(max(z + self.offset_z, 0.0), top) / unit
        ix, iz = int(fx), int(fz)
        xf, zf = fx - ix, fz - iz

        def h(a, b):
            return self.heights[min(iz + b, n - 1) * n + min(ix + a, n - 1)]

        if iz & 1 == 0:
            if xf > zf:
                return h(0, 0) + (h(1, 0) - h(0, 0)) * xf + (h(1, 1) - h(1, 0)) * zf
            return h(0, 0) + (h(0, 1) - h(0, 0)) * zf + (h(1, 1) - h(0, 1)) * xf
        if xf + zf < 1.0:
            return h(0, 0) + (h(1, 0) - h(0, 0)) * xf + (h(0, 1) - h(0, 0)) * zf
        return h(1, 1) + (h(0, 1) - h(1, 1)) * (1.0 - xf) + (h(1, 0) - h(1, 1)) * (1.0 - zf)


def is_cut(cutters, x, y, z):
    """core::collision::is_cut_by: inside when every plane is strictly negative."""
    for sphere, planes in cutters:
        r, cx, cy, cz = sphere
        if r > 0 and (x - cx) ** 2 + (y - cy) ** 2 + (z - cz) ** 2 > r * r:
            continue
        if all(p[0] * x + p[1] * y + p[2] * z + p[3] < 0 for p in planes):
            return True
    return False


def painted(masks, n, ix, iz):
    """Does any render patch touching this vertex carry a layer?"""
    side = n // 8
    for pz in {iz // 8, max(iz - 1, 0) // 8}:
        for px in {ix // 8, max(ix - 1, 0) // 8}:
            if masks[min(pz, side - 1) * side + min(px, side - 1)]:
                return True
    return False


# ---- what the map authors as ground ----


def glb_bounds(path):
    """POSITION accessor min/max over a GLB's meshes, from its JSON chunk."""
    data = path.read_bytes()
    length = struct.unpack_from("<I", data, 12)[0]
    doc = json.loads(data[20:20 + length])
    lows, highs = [], []
    for mesh in doc.get("meshes", []):
        for primitive in mesh.get("primitives", []):
            accessor = doc["accessors"][primitive["attributes"]["POSITION"]]
            if "min" in accessor and "max" in accessor:
                lows.append(accessor["min"])
                highs.append(accessor["max"])
    if not lows:
        return None
    return [min(v[i] for v in lows) for i in range(3)], [max(v[i] for v in highs) for i in range(3)]


class Cover:
    """World bounding boxes of placed collision, bucketed by x/z. Sky domes and terrain-sized shells are left out."""

    CELL = 32.0
    LARGEST = 400.0

    def __init__(self, manifest, folder):
        geometry_of = {c["name"].lower(): (c.get("geometry") or c["name"]).lower() for c in manifest.get("classes", [])}
        local = {}
        for entry in manifest.get("collision", []):
            boxes = local.setdefault(entry["geometry"].lower(), [])
            if entry.get("glb") and (folder / entry["glb"]).exists():
                bounds = glb_bounds(folder / entry["glb"])
                if bounds:
                    boxes.append(bounds)
            for prim in entry.get("prims", []):
                r = max(prim["dims"]) if prim["kind"] != "box" else math.sqrt(sum(v * v for v in prim["dims"]))
                c = prim["translation"]
                boxes.append(([c[i] - r for i in range(3)], [c[i] + r for i in range(3)]))
        self.buckets = {}
        self.boxes = 0
        for world in [manifest] + list(manifest.get("layers") or []):
            for inst in world.get("instances", []):
                name = inst["class"].lower()
                for low, high in local.get(geometry_of.get(name, name), ()):
                    self.place(inst, low, high)

    def place(self, inst, low, high):
        corners = [[(low, high)[(k >> i) & 1][i] for i in range(3)] for k in range(8)]
        rows, t = inst["rotation"], inst["translation"]
        world = [[sum(rows[r][c] * p[c] for c in range(3)) + t[r] for r in range(3)] for p in corners]
        box = tuple(min(p[i] for p in world) for i in range(3)) + tuple(max(p[i] for p in world) for i in range(3))
        if box[3] - box[0] > self.LARGEST or box[5] - box[2] > self.LARGEST:
            return
        self.boxes += 1
        for i in range(int(box[0] // self.CELL), int(box[3] // self.CELL) + 1):
            for j in range(int(box[2] // self.CELL), int(box[5] // self.CELL) + 1):
                self.buckets.setdefault((i, j), []).append(box)

    def covers(self, x, z, *heights):
        """Does a placed box span this x/z and reach within a stride of either terrain height?"""
        for box in self.buckets.get((int(x // self.CELL), int(z // self.CELL)), ()):
            if box[0] <= x <= box[3] and box[2] <= z <= box[5]:
                if any(box[1] - 2.0 <= y <= box[4] + 2.0 for y in heights):
                    return True
        return False


def features(manifest):
    """All of base + every mode layer: a superset across modes, so 'far' is conservative."""
    worlds = [manifest] + list(manifest.get("layers") or [])
    hubs, arcs, nodes, posts = [], [], [], []
    for world in worlds:
        nav = world.get("nav") or {}
        plan = nav.get("plan") or {}
        base = len(hubs)
        for hub in plan.get("hubs", []):
            hubs.append((hub["pos"][0], hub["pos"][2], hub["radius"], hub["pos"][1]))
        for arc in plan.get("arcs", []):
            arcs.append((int(arc["a"]) + base, int(arc["b"]) + base, int(arc["type_mask"])))
        for inst in world.get("instances", []):
            if any(int(key) == CAPTURE_REGION for key, _ in inst.get("props", [])):
                posts.append((inst["name"], inst["translation"]))
    for path in manifest.get("paths", []):
        if "boundary" in path["name"].lower():
            continue
        for pos, _rotation in path["nodes"]:
            nodes.append((path["name"], pos))
    boundary = [[(p[0], p[2]) for p, _ in path["nodes"]] for path in manifest.get("paths", [])
                if path["name"].lower().startswith("boundary") and len(path["nodes"]) >= 3]
    return hubs, arcs, nodes, posts, boundary


def in_polygon(polygon, x, z):
    inside = False
    for (x1, z1), (x2, z2) in zip(polygon, polygon[1:] + polygon[:1]):
        if (z1 > z) != (z2 > z) and x < (x2 - x1) * (z - z1) / (z2 - z1) + x1:
            inside = not inside
    return inside


class Index:
    """A coarse 2-D buckets of ground features: returns the distance to the nearest within REACH."""

    CELL = 32.0

    def __init__(self, hubs, arcs, nodes, posts, reach):
        self.buckets = {}
        self.reach = reach
        for x, z, radius, _y in hubs:
            self.add(x, z, radius)
        for _name, pos in list(nodes) + list(posts):
            self.add(pos[0], pos[2], 0.0)
        for a, b, mask in arcs:
            if mask and not mask & ~FLYER:
                continue  # flyer-only arcs are not ground
            if a >= len(hubs) or b >= len(hubs):
                continue
            (ax, az), (bx, bz) = hubs[a][:2], hubs[b][:2]
            length = math.hypot(bx - ax, bz - az)
            steps = max(int(length / 4.0), 1)
            for i in range(steps + 1):
                self.add(ax + (bx - ax) * i / steps, az + (bz - az) * i / steps, 0.0)

    def add(self, x, z, radius):
        self.buckets.setdefault((int(x // self.CELL), int(z // self.CELL)), []).append((x, z, radius))

    def distance(self, x, z):
        best = math.inf
        span = int((self.reach + 64.0) // self.CELL) + 1
        cx, cz = int(x // self.CELL), int(z // self.CELL)
        for i in range(cx - span, cx + span + 1):
            for j in range(cz - span, cz + span + 1):
                for px, pz, radius in self.buckets.get((i, j), ()):
                    best = min(best, math.hypot(x - px, z - pz) - radius)
        return max(best, 0.0)


def magnitudes(deltas):
    out = {"samples": len(deltas), "max_abs_metres": max(deltas, default=0.0)}
    for label, limit in THRESHOLDS:
        out[label] = sum(d > limit for d in deltas)
    return out


def analyse(file, ref, other, manifest, folder):
    (a_row, a_masks), (b_row, b_masks) = ref, other
    grid, grid_b = Grid(a_row), Grid(b_row)
    if grid.n != grid_b.n or grid.unit != grid_b.unit:
        raise ValueError(f"{file}: grids differ; only same-size grids are classified")
    cutters = [(c.get("sphere", [0, 0, 0, 0]), c["planes"]) for c in manifest.get("terrain_cutters", [])]
    hubs, arcs, nodes, posts, boundary = features(manifest)
    index = Index(hubs, arcs, nodes, posts, NEAR_NAV_M)
    cover = Cover(manifest, folder)
    hidden = Counter()
    in_play, in_play_covered, outside = [], [], []
    classes = {name: [] for name in CLASSES}
    cells = {name: [] for name in CLASSES}
    in_boundary = Counter()
    for iz in range(grid.n):
        for ix in range(grid.n):
            ya, yb = grid.heights[iz * grid.n + ix], grid_b.heights[iz * grid.n + ix]
            if ya == yb:
                continue
            x, z = grid.world(ix, iz)
            cut_a, cut_b = is_cut(cutters, x, ya, z), is_cut(cutters, x, yb, z)
            if cut_a and cut_b:
                name = "cut_both"
            elif cut_a or cut_b:
                name = "cut_one"
            elif a_masks is not None and not painted(a_masks, grid.n, ix, iz):
                name = "unpainted"
            else:
                d = index.distance(x, z)
                name = "on_nav" if d <= ON_NAV_M else "near_nav" if d <= NEAR_NAV_M else "far_painted"
            classes[name].append(abs(ya - yb))
            over = cover.covers(x, z, ya, yb)
            hidden[name] += over
            beyond = bool(boundary) and not any(in_polygon(p, x, z) for p in boundary)
            if beyond:
                outside.append(abs(ya - yb))
            elif name not in ("cut_both", "unpainted"):
                (in_play_covered if over else in_play).append(abs(ya - yb))
            cells[name].append((x, z))
            if boundary:
                in_boundary[name + ("_inside" if any(in_polygon(p, x, z) for p in boundary) else "_outside")] += 1
    report = {
        "file": file,
        "map": manifest.get("map"),
        "placed_collision_boxes": cover.boxes,
        "grid_size": grid.n,
        "grid_unit": grid.unit,
        "total_changed": sum(map(len, classes.values())),
        "classes": {},
        "boundary_paths": len(boundary),
        "in_play_exposed": magnitudes(in_play),
        "in_play_under_object": magnitudes(in_play_covered),
        "outside_boundary": magnitudes(outside),
        "by_boundary": dict(sorted(in_boundary.items())),
        "painted_patches": None if a_masks is None else {
            "reference": sum(1 for m in a_masks if m),
            "total": len(a_masks),
        },
    }
    for name in CLASSES:
        summary = magnitudes(classes[name])
        summary["under_object"] = hidden[name]
        if cells[name]:
            xs, zs = zip(*cells[name])
            summary["bbox_xz"] = [round(min(xs), 1), round(min(zs), 1), round(max(xs), 1), round(max(zs), 1)]
        report["classes"][name] = summary
    report["play_lines"] = play_lines(grid, grid_b, cutters, hubs, arcs, nodes, posts, cover)
    report["instrument"] = instrument(grid, nodes, posts, hubs, a_masks, boundary)
    return report


def play_lines(a, b, cutters, hubs, arcs, nodes, posts, cover):
    def delta(x, z):
        if not (a.covers(x, z) and b.covers(x, z)):
            return None
        ya, yb = a.height(x, z), b.height(x, z)
        if is_cut(cutters, x, ya, z) and is_cut(cutters, x, yb, z):
            return None
        return abs(ya - yb)

    def exposed(x, z):
        return not cover.covers(x, z, a.height(x, z), b.height(x, z))

    def group(selected):
        worst = worst_open = 0.0
        touched = Counter()
        open_touched = Counter()
        examined = 0
        names = []
        for number, (ia, ib, _mask) in selected:
            ax, az = hubs[ia][:2]
            bx, bz = hubs[ib][:2]
            steps = max(int(math.hypot(bx - ax, bz - az) / ARC_STEP_M), 1)
            at = [(ax + (bx - ax) * i / steps, az + (bz - az) * i / steps) for i in range(steps + 1)]
            samples = [(delta(x, z), x, z) for x, z in at]
            samples = [row for row in samples if row[0] is not None]
            if not samples:
                continue
            examined += 1
            top = max(row[0] for row in samples)
            worst = max(worst, top)
            top_open = max((row[0] for row in samples if row[0] and exposed(row[1], row[2])), default=0.0)
            worst_open = max(worst_open, top_open)
            for label, limit in THRESHOLDS:
                touched[label] += top > limit
                open_touched[label] += top_open > limit
            if top_open > 0.1:
                names.append([number, round(top_open, 2)])
        return {"arcs": examined, "max_abs_metres": worst, **{k: touched[k] for k, _ in THRESHOLDS},
                "exposed": {"max_abs_metres": worst_open, **{k: open_touched[k] for k, _ in THRESHOLDS}},
                "exposed_arcs_over_10cm": sorted(names, key=lambda r: -r[1])[:12]}

    valid = [(i, arc) for i, arc in enumerate(arcs) if arc[0] < len(hubs) and arc[1] < len(hubs)]
    soldier = [r for r in valid if r[1][2] & SOLDIER or not r[1][2]]
    vehicle = [r for r in valid if not r[1][2] & SOLDIER and r[1][2] & ~FLYER]

    def points(rows):
        moved = []
        for name, pos in rows:
            d = delta(pos[0], pos[2])
            if d:
                # [name, |change| m, the authored point's height over the reference terrain]: a point metres above
                # the terrain stands on something else, and the change does not reach it.
                moved.append([name, round(d, 3), round(pos[1] - a.height(pos[0], pos[2]), 2)])
        on_ground = [r for r in moved if abs(r[2]) < 2.0]
        return {"total": len(rows), "moved_count": len(moved), "moved_on_terrain": len(on_ground),
                "largest": sorted(moved, key=lambda r: -r[1])[:12]}

    spawn = [row for row in nodes if "spawn" in row[0].lower()]
    return {"soldier_arcs": group(soldier), "vehicle_only_arcs": group(vehicle), "spawn_nodes": points(spawn),
            "other_path_nodes": points([row for row in nodes if "spawn" not in row[0].lower()]),
            "command_posts": points(posts)}


def instrument(grid, nodes, posts, hubs, masks, boundary):
    """Does this frame put authored points on the terrain and on painted patches? It must, or nothing above holds."""
    gaps = sorted(abs(pos[1] - grid.height(pos[0], pos[2])) for _, pos in list(nodes) + list(posts)
                  if grid.covers(pos[0], pos[2]))
    flipped = sorted(abs(pos[1] - grid.height(pos[0], -pos[2])) for _, pos in list(nodes) + list(posts)
                     if grid.covers(pos[0], -pos[2]))
    on_paint = covered = 0
    for x, z, _radius, _y in hubs:
        if grid.covers(x, z):
            covered += 1
            ix = int((x + grid.offset_x) / grid.unit)
            iz = int((z + grid.offset_z) / grid.unit)
            on_paint += masks is None or painted(masks, grid.n, ix, iz)
    median = lambda v: round(v[len(v) // 2], 3) if v else None
    return {"median_node_gap_m": median(gaps), "median_node_gap_with_z_flipped_m": median(flipped),
            "hubs_on_grid": covered, "hubs_on_painted_patches": on_paint, "hubs_total": len(hubs),
            "hubs_inside_boundary": None if not boundary else
            sum(any(in_polygon(p, x, z) for p in boundary) for x, z, _r, _y in hubs)}


def compare(reference, other, baked, game, files, map_dirs=None):
    left = read_terrains(reference, files)
    right = read_terrains(other, files)
    out = []
    for file in sorted(left.keys() & right.keys()):
        (a_row, _), (b_row, _) = left[file], right[file]
        if a_row["_samples"] == b_row["_samples"]:
            continue  # including the same heights under other cutters, which this tool does not classify
        stem = Path(file).stem
        names = [(map_dirs or {}).get(stem), f"{game}_{stem}", stem]
        folder = next((baked / n for n in names if n and (baked / n / "manifest.ron").exists()), None)
        if folder is None:
            raise ValueError(f"no baked manifest for {file}")
        manifest = parse_ron((folder / "manifest.ron").read_text())
        if manifest.get("source_game") != game:
            raise ValueError(f"{folder} is not a {game} bake")
        report = analyse(file, left[file], right[file], manifest, folder)
        report["baked_map"] = folder.name
        out.append(report)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("reference", type=Path)
    parser.add_argument("other", type=Path)
    parser.add_argument("baked", type=Path)
    parser.add_argument("--game", required=True, choices=("bf1", "bf2"))
    parser.add_argument("--files", nargs="+", required=True, help="canonical LVL paths such as yav/yav1.lvl")
    parser.add_argument("--map-dir", action="append", default=[], metavar="STEM=FOLDER",
                        help="baked folder for a map whose name is not <game>_<stem>")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        dirs = dict(item.split("=", 1) for item in args.map_dir)
        rows = compare(args.reference, args.other, args.baked, args.game, args.files, dirs)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps({"schema": 1, "game": args.game, "terrains": rows}, indent=1, sort_keys=True,
                                       allow_nan=False) + "\n")
    except (ValueError, OSError, struct.error, KeyError) as exc:
        parser.exit(2, f"terrain play: {exc}\n")


if __name__ == "__main__":
    sys.exit(main())
