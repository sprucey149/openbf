#!/usr/bin/env python3
"""Hash baked gameplay inputs, independently of texture/asset resolution.

    world_fingerprint.py ASSETS MISSIONS MAP [--out fingerprint.json]

This is a conservative compatibility fingerprint, not a security signature.
Hashes cover map/common placements, regions, paths, planning sidecars, collision
triangle order and primitives, ordered class properties (presentation values
replaced by markers), vehicle rigs/collision/root motion, mission declarations
and environment configuration. Unresolved class properties remain included.
Render vertices, textures and sound are excluded. The full hero pose inventory
is included conservatively, so unused animation differences can also change it.
Terrain has separate complete-grid and playable-cell hashes. Playable cells
include ALL their corners, including outside corners influencing an in-bounds
query. Without an authored boundary the complete grid is required.
The report contains only names, hashes and counts; never geometry payloads.
"""
import argparse
import hashlib
from functools import lru_cache
import json
import math
from pathlib import Path
import struct

import gameplay_table
import version_census
from version_terrain_play import in_polygon, parse_ron

SCHEMA = 1
MAP_FIELDS = ("map", "source_game", "classes", "regions", "instances", "paths", "layers", "nav",
              "terrain_cutters", "lighting", "sky", "water", "wind", "precipitation", "lightning")
CLASS_VISUAL = {"attach", "lights", "effects"}


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()).hexdigest()


def read_ron(path):
    if not path.is_file():
        raise ValueError(f"required gameplay input missing: {path}")
    return parse_ron(path.read_text())


def safe_child(folder, relative):
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"unsafe baked relative path: {relative}")
    result = folder / path
    if not result.is_file():
        raise ValueError(f"required gameplay input missing: {result}")
    return result


def read_glb(path):
    """Return the document and a decoded accessor reader, never render bytes."""
    raw = path.read_bytes()
    if len(raw) < 20 or struct.unpack_from("<4sII", raw) != (b"glTF", 2, len(raw)):
        raise ValueError(f"invalid GLB: {path}")
    size, tag = struct.unpack_from("<I4s", raw, 12)
    if tag != b"JSON":
        raise ValueError(f"missing GLB JSON: {path}")
    doc = json.loads(raw[20:20 + size])
    start = 20 + size
    bsize, btag = struct.unpack_from("<I4s", raw, start)
    if btag != b"BIN\x00" or start + 8 + bsize > len(raw):
        raise ValueError(f"missing GLB binary: {path}")
    binary = raw[start + 8:start + 8 + bsize]

    def accessor(index):
        a = doc["accessors"][index]
        if "sparse" in a:
            raise ValueError("sparse gameplay accessor is unsupported")
        view = doc["bufferViews"][a["bufferView"]]
        if view.get("buffer", 0) != 0:
            raise ValueError("external gameplay buffer is unsupported")
        n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[a["type"]]
        fmt = {5121: "B", 5123: "H", 5125: "I", 5126: "f"}[a["componentType"]]
        width = struct.calcsize(fmt) * n
        stride = view.get("byteStride", width)
        at = view.get("byteOffset", 0) + a.get("byteOffset", 0)
        if a["count"] and at + (a["count"] - 1) * stride + width > len(binary):
            raise ValueError("truncated gameplay accessor")
        return [list(struct.unpack_from("<" + fmt * n, binary, at + i * stride)) for i in range(a["count"])]

    return doc, accessor


def glb_geometry(path):
    """Decode positions/indices just as the runtime, never hash render attributes."""
    doc, accessor = read_glb(path)
    positions, indices = [], []
    for mesh in doc.get("meshes", []):
        for p in mesh.get("primitives", []):
            if p.get("mode", 4) != 4:
                raise ValueError("gameplay GLB must contain triangles")
            vertices = accessor(p["attributes"]["POSITION"])
            base = len(positions)
            ix = [v[0] for v in accessor(p["indices"])] if "indices" in p else list(range(len(vertices)))
            if any(i >= len(vertices) for i in ix):
                raise ValueError("out-of-range gameplay index")
            positions.extend(vertices)
            indices.extend(base + i for i in ix)
    return positions, indices



def model_spheres(doc, folder):
    """Station/armed-building blast spheres are simulation input, even on render models."""
    out = []
    for geometry in doc.get("geometries", []):
        model = geometry["model"]
        path = safe_child(folder, f"models/{model}.glb")
        data, _ = read_glb(path)
        sphere = None
        for mesh in data.get("meshes", []):
            sphere = mesh.get("extras", {}).get("sphere", sphere)
        out.append([geometry["name"], model, sphere])
    return out


@lru_cache(maxsize=256)
def pose_hash(path, modified, size):
    # Cache only immutable staging files. The stat arguments invalidate edits.
    del modified, size
    doc, accessor = read_glb(Path(path))
    nodes = [{k: v for k, v in node.items() if k in ("name", "matrix", "rotation", "translation", "scale", "children")}
             for node in doc.get("nodes", [])]
    skins = []
    for skin in doc.get("skins", []):
        skins.append({k: accessor(v) if k == "inverseBindMatrices" else v for k, v in skin.items()})
    animations = []
    for animation in doc.get("animations", []):
        animations.append({"name": animation.get("name"), "channels": animation.get("channels", []),
                           "samplers": [{k: accessor(v) if k in ("input", "output") else v for k, v in row.items()}
                                        for row in animation.get("samplers", [])]})
    return digest({"nodes": nodes, "skins": skins, "animations": animations})


def saber_inputs(assets):
    """Hash saber collision's skeletons, full poses, combos and motion; omit pixels/vertices."""
    side = assets / "side_bf2"
    hero_path = side / "heroes.ron"
    if not hero_path.is_file():
        return [], ["side_bf2/heroes.ron"]
    heroes = read_ron(hero_path)
    geometries = sorted({row["geometry"] for row in heroes.get("heroes", [])})
    missing, out = [], [["heroes.ron", digest(heroes)]]
    for geometry in geometries:
        body = side / "soldiers" / (geometry + ".glb")
        if not body.is_file():
            missing.append(str(body.relative_to(assets)))
            continue
        stat = body.stat()
        out.append([str(body.relative_to(assets)), pose_hash(str(body.resolve()), stat.st_mtime_ns, stat.st_size)])
        for suffix in ("sets.ron", "motion.ron"):
            path = body.with_suffix("." + suffix)
            if not path.is_file():
                missing.append(str(path.relative_to(assets)))
    # Resolve only file dependencies here; runtime remains the authority on
    # interpreting ordered blade groups and sampling their collision tracks.
    definitions = {}
    for path in sorted((assets / "classes").glob("bf2_*.ron")):
        for row in read_ron(path):
            definitions.setdefault(row["name"].lower(), row)
    def chain(title):
        seen = set()
        while title.lower() in definitions and title.lower() not in seen:
            title = title.lower()
            seen.add(title)
            row = definitions[title]
            yield row
            title = row["base"]
    fnv = version_census.extract_lua.fnv1a
    weapon_keys = {fnv("WeaponName")} | {fnv(f"WeaponName{i}") for i in range(1, 9)}
    for hero in heroes.get("heroes", []):
        ancestors = list(chain(hero["class"]))
        if not ancestors:
            missing.append("hero_class:" + hero["class"])
            continue
        weapons = [value for row in ancestors for key, value in row["props"] if int(key) in weapon_keys]
        has_combo = any(int(key) == fnv("ComboAnimationBank") and value.strip()
                        for weapon in weapons for row in chain(weapon) for key, value in row["props"])
        combo = side / "soldiers" / (hero["geometry"] + ".combos.ron")
        if has_combo and not combo.is_file():
            missing.append(str(combo.relative_to(assets)))
    for model in heroes.get("weapons", []):
        nodes = side / "weapons" / (model + ".nodes.ron")
        if not nodes.is_file():
            missing.append(str(nodes.relative_to(assets)))
    for pattern in ("soldiers/*.sets.ron", "soldiers/*.motion.ron", "soldiers/*.combos.ron", "weapons/*.nodes.ron", "combos/*.ron"):
        for path in sorted(side.glob(pattern)):
            out.append([str(path.relative_to(assets)), digest(read_ron(path))])
    return out, sorted(set(missing))

def classes(rows):
    return [{k: v for k, v in row.items() if k not in CLASS_VISUAL} for row in rows]


def world(doc, folder):
    out = {key: doc.get(key) for key in MAP_FIELDS if key in doc}
    out["classes"] = classes(doc.get("classes", []))
    out["nav_sidecars"] = []
    def sidecars(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key in ("routes", "records") and isinstance(child, str):
                    p = safe_child(folder, child)
                    out["nav_sidecars"].append([key, hashlib.sha256(p.read_bytes()).hexdigest()])
                else:
                    sidecars(child)
        elif isinstance(value, list):
            for child in value:
                sidecars(child)
    sidecars(doc.get("nav"))
    sidecars(doc.get("layers"))
    return out


def collisions(doc, folder):
    out = []
    for row in doc.get("collision", []):
        entry = {k: v for k, v in row.items() if k not in ("glb", "node")}
        # mask_own and node are reader provenance, not collision behavior.
        entry["prims"] = [{k: v for k, v in p.items() if k != "mask_own"} for p in row.get("prims", [])]
        entry["mesh"] = glb_geometry(safe_child(folder, row["glb"])) if row.get("glb") else None
        out.append(entry)
    # Runtime resolves collision resources by geometry name. Preserve primitive
    # and triangle order within each resource, including duplicate-key order.
    return sorted(out, key=lambda row: row["geometry"].lower())


def intersects(a, b, c, d):
    def cross(p, q, r):
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    def on(p, q, r):
        return cross(p, q, r) == 0 and min(p[0], q[0]) <= r[0] <= max(p[0], q[0]) and min(p[1], q[1]) <= r[1] <= max(p[1], q[1])
    return ((cross(a, b, c) * cross(a, b, d) < 0 and cross(c, d, a) * cross(c, d, b) < 0)
            or on(a, b, c) or on(a, b, d) or on(c, d, a) or on(c, d, b))


def cell_in_play(polygons, x0, z0, x1, z1):
    if not polygons:
        return True
    corners = [(x0, z0), (x1, z0), (x1, z1), (x0, z1)]
    for polygon in polygons:
        if any(in_polygon(polygon, x, z) for x, z in corners):
            return True
        if any(x0 <= x <= x1 and z0 <= z <= z1 for x, z in polygon):
            return True
        if any(intersects(a, b, c, d) for a, b in zip(corners, corners[1:] + corners[:1])
               for c, d in zip(polygon, polygon[1:] + polygon[:1])):
            return True
    return False


def terrain(doc, folder):
    if not doc.get("terrain_glb"):
        return None, None, {"vertices": 0, "playable_vertices": 0, "boundary_paths": 0}
    positions, _ = glb_geometry(safe_child(folder, doc["terrain_glb"]))
    n = next((i for i in range(1, len(positions)) if positions[i][0] == positions[0][0]), 0)
    if n < 2 or n * n > len(positions):
        raise ValueError("terrain is not a square leading grid")
    grid = positions[:n * n]
    requested = doc.get("terrain_query_size")
    if requested is not None and (type(requested) not in (int, float) or not math.isfinite(requested)
                                  or int(requested) != requested):
        raise ValueError("invalid terrain query size")
    query_n = n if requested is None else int(requested)
    if query_n < 2 or query_n > n or (n - query_n) % 2:
        raise ValueError("terrain query grid is not a centered subwindow")
    start = (n - query_n) // 2
    query = [grid[z * n + x] for z in range(start, start + query_n) for x in range(start, start + query_n)]
    unit = query[1][0] - query[0][0]
    if requested is not None and (query[0][0] != -(query_n / 2) * unit
                                  or query[0][2] != -(query_n / 2 - 1) * unit):
        raise ValueError("terrain query grid does not use the canonical world frame")
    polygons = [[(p[0], p[2]) for p, _ in row["nodes"]] for row in doc.get("paths", [])
                if row["name"].lower().startswith("boundary") and len(row["nodes"]) >= 3]
    selected = set()
    for z in range(query_n - 1):
        for x in range(query_n - 1):
            a, b = query[z * query_n + x], query[(z + 1) * query_n + x + 1]
            if cell_in_play(polygons, a[0], a[2], b[0], b[2]):
                selected.update((z * query_n + x, z * query_n + x + 1, (z + 1) * query_n + x, (z + 1) * query_n + x + 1))
    # World positions, rather than source indices, support different donor extents.
    playable = sorted((query[i] for i in selected), key=lambda p: (p[2], p[0]))
    return grid, playable, {"vertices": len(grid), "playable_vertices": len(playable), "boundary_paths": len(polygons),
                            "query_size": query_n, "query_unit": unit,
                            "query_offset_x": -query[0][0], "query_offset_z": -query[0][2]}


def class_defs(path):
    doc = read_ron(path)
    def property_record(key, value):
        # Even visual setters can create/reorder a stateful blade edge. Keep
        # their presence and exact position while ignoring the visual value.
        if int(key) in gameplay_table.PRESENTATION:
            return [key, {"presentation": True}]
        parsed = version_census.property_value(int(key), value)
        return [key, value if parsed == "NM" else parsed]
    def filter_record(row):
        if isinstance(row, dict):
            return {k: ([property_record(h, v) for h, v in value]
                        if k == "props" else filter_record(value)) for k, value in row.items() if k != "source"}
        if isinstance(row, list):
            return [filter_record(v) for v in row]
        return row
    return filter_record(doc)


def fingerprint(assets, missions, key):
    assets, missions = Path(assets), Path(missions)
    folder = assets / key
    doc = read_ron(folder / "manifest.ron")
    game = doc.get("source_game", "bf1")
    common_dir = assets / ("common_" + game)
    common = read_ron(common_dir / "common.ron")
    definitions = sorted((assets / "classes").glob("*.ron"))
    if not definitions:
        raise ValueError("no baked class definitions")
    mission_paths = sorted(missions.glob(key + "_*.ron"))
    if not mission_paths:
        raise ValueError(f"no missions for {key}")
    sections = {
        "map": world(doc, folder),
        "common_classes": classes(common.get("classes", [])),
        "map_collision": collisions(doc, folder),
        "common_collision": collisions(common, common_dir),
        "map_blast_spheres": model_spheres(doc, folder),
        "common_blast_spheres": model_spheres(common, common_dir),
        "class_definitions": [[p.name, class_defs(p)] for p in definitions],
        "map_class_definitions": class_defs(folder / "classes.ron"),
        "missions": [[p.name, {k: v for k, v in read_ron(p).items() if k not in ("source", "voice", "music")}]
                     for p in mission_paths],
    }
    grid, playable, counts = terrain(doc, folder)
    sections["terrain_playable"] = playable
    sections["terrain_query_frame"] = {k: v for k, v in counts.items() if k.startswith("query_")}
    vehicles = []
    for p in sorted((assets / ("side_" + game) / "vehicles").glob("*/vehicles.ron")):
        vehicle = read_ron(p)
        vehicles.append([p.parent.name, {k: v for k, v in vehicle.items() if k not in ("source", "models", "textures", "geometries", "collision")},
                         collisions(vehicle, p.parent)])
    sections["vehicles"] = vehicles
    sections["saber_inputs"], missing = saber_inputs(assets)
    hashes = {k: digest(v) for k, v in sections.items()}
    return {"schema": SCHEMA, "algorithm": "sha256", "map": key, "source_game": game,
            "sha256": digest(hashes), "components": hashes, "complete": not missing, "missing_inputs": missing, "terrain_all_sha256": digest(grid),
            "counts": {**counts, "class_files": len(definitions), "missions": len(mission_paths), "vehicle_files": len(vehicles)},
            "coverage": "conservative_baked_gameplay_inputs_v1"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("assets", type=Path)
    parser.add_argument("missions", type=Path)
    parser.add_argument("map")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    try:
        result = fingerprint(args.assets, args.missions, args.map)
        text = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.out:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(text)
        else:
            print(text, end="")
    except (ValueError, OSError, KeyError, IndexError, struct.error) as exc:
        parser.exit(2, f"world fingerprint: {exc}\n")


if __name__ == "__main__":
    main()
