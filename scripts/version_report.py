#!/usr/bin/env python3
"""Build a compact, reproducible measurement receipt from version censuses, and the tables quoted from it.

Usage: python3 scripts/version_report.py CENSUS.json ... [--heights H.json ...] [--shapes S.json ...]
           [--terrain T.json ...] --out measurements.json
       python3 scripts/version_report.py --update-doc analysis/version-diff.md --receipt measurements.json

The receipt holds counts, magnitudes, property hashes and a few class and mesh names. The local census keeps the rest.
All same-game pairs present are compared.
"""
import argparse
import collections
import hashlib
import itertools
import json
from pathlib import Path
import re

import version_census
import version_geometry


def digest(data):
    return hashlib.sha256(data).hexdigest()


def stats(section):
    """Counts only: names and values stay in the local census."""
    result = {}
    for key, value in section.items():
        if key == "differences":
            continue
        if isinstance(value, list):
            result[key + "_count"] = len(value)
        else:
            result[key] = value
    return result


def file_digest(files):
    """Pin the exact LVL set without listing each file in the receipt."""
    rows = sorted((row["name"], row["sha256"]) for row in files)
    return digest("".join(f"{sha}  {name}\n" for name, sha in rows).encode())


def numeric_groups(comparisons, game):
    """Rank scalar deltas; coalesce identical definitions copied across scopes."""
    groups = {}
    for pair, comparison in comparisons.items():
        if comparison["left"]["game"] != game:
            continue
        for category in ("properties", "mission_values"):
            for row in comparison.get(category, {}).get("differences", []):
                if "absolute_delta" not in row:
                    continue
                # Keep a field's zero-based repeated-property occurrence. This
                # groups duplicate source definitions without losing examples.
                key = json.dumps([category, row["identity"][2:4], row["field"], row["left"], row["right"]])
                group = groups.setdefault(key, {"category": category,
                    "name": row["identity"][3], "field": row["field"],
                    "left": row["left"], "right": row["right"],
                    "absolute_delta": row["absolute_delta"], "pairs": set(), "occurrences": 0})
                group["pairs"].add(pair)
                group["occurrences"] += 1
    top = sorted(groups.values(), key=lambda row: (-row["absolute_delta"], row["name"], str(row["field"])))[:10]
    for row in top:
        row["pairs"] = sorted(row["pairs"])
    return top


def height_rows(heights):
    """The per-terrain sample summary, for terrains that differ or that one edition extends."""
    out = {}
    for doc in heights:
        pair = f"{doc['left']['game']}_{doc['left']['platform']}_vs_{doc['right']['game']}_{doc['right']['platform']}"
        rows = []
        for row in doc["terrains"]:
            if row.get("changed") or row.get("left_only") or row.get("right_only") or row["compared"] == "NM":
                rows.append({key: value for key, value in row.items() if key != "identity"}
                            | {"file": row["identity"][0], "name": row["identity"][2]})
        out[pair] = {"summary": doc["summary"], "terrains": sorted(rows, key=lambda r: r["file"])}
    return out


def geometry_gate(left, right):
    """Which map LVLs carry the same simulation shapes in both editions? Meshes or terrain one edition lacks fail a map.

    Shared LVLs (sides, the space sky, the common banks) are not maps, so what differs in them is reported beside the
    list, since it applies to every map loading them. So are the paths and planning graphs of the maps that pass: a
    table could carry those, so they do not fail a map, but such a map does not yet match.
    """
    a, b = left["geometry"], right["geometry"]
    collision = [version_geometry.indexed_rows(g["collision"], "collision") for g in (a, b)]
    terrain = [version_geometry.indexed_rows(g["terrain"], "terrain") for g in (a, b)]
    routes = [version_geometry.indexed_rows(g.get("routes", []), "routes") for g in (a, b)]
    different = collections.Counter(k[0] for k in collision[0].keys() & collision[1].keys()
                                    if collision[0][k]["surface_sha256"] != collision[1][k]["surface_sha256"])
    missing = collections.Counter(k[0] for k in collision[0].keys() - collision[1].keys())
    fields = ("heights_sha256", "cutters_sha256")
    ground = {k[0] for k in terrain[0].keys() & terrain[1].keys()
              if any(terrain[0][k].get(f) != terrain[1][k].get(f) for f in fields)}
    ground |= {k[0] for k in terrain[0].keys() - terrain[1].keys()}
    other_routes = {k[0] for k in routes[0].keys() & routes[1].keys() if routes[0][k]["sha256"] != routes[1][k]["sha256"]}
    other_routes |= {k[0] for k in routes[0].keys() ^ routes[1].keys()}
    maps = sorted(row["file"] for row in a["terrain"])
    same = [m for m in maps if m not in ground and not different[m] and not missing[m]]

    def shared(counts, sides):
        return {k: v for k, v in sorted(counts.items()) if k not in maps and k.startswith("side/") == sides}

    return {"maps": len(maps),
            "same_shapes": same,
            "same_shapes_other_routes": [m for m in same if m in other_routes],
            "other_ground": sorted(ground),
            "side_meshes_with_other_surfaces": shared(different, True),
            "side_meshes_missing": shared(missing, True),
            "other_shared_meshes_with_other_surfaces": shared(different, False),
            "other_shared_meshes_missing": shared(missing, False)}


def generate(paths, heights=(), shapes=(), terrain=()):
    sources = {}
    receipts = {}
    for path in paths:
        raw = path.read_bytes()
        source = json.loads(raw)
        key = source["source"]["game"] + "_" + source["source"]["platform"]
        if key in sources:
            raise ValueError("duplicate game/platform census")
        sources[key] = source
        receipts[key] = {
            "census_sha256": digest(raw), "source": source["source"],
            "animation_registry_sha256": source.get("animation_registry_sha256"),
            "lvl_files": len(source["files"]), "lvl_files_sha256": file_digest(source["files"]),
            "entities": len(source["entities"]),
            "entity_kinds": dict(collections.Counter(row["kind"] for row in source["entities"])),
            "missions": len(source["missions"]), "content": len(source["content"]),
            "geometry": {key: len(rows) for key, rows in source.get("geometry", {}).items()},
            "animation_names_resolved": sum(row["name"] != "NM" for row in source.get("geometry", {}).get("animations", [])),
            "errors": source["errors"], "unmeasured": source["unmeasured"],
        }
    comparisons = {}
    summaries = {}
    order = {"ps2": 0, "xbox": 1, "pc": 2}
    keys = sorted(sources, key=lambda k: (sources[k]["source"]["game"], order[sources[k]["source"]["platform"]]))
    for left, right in itertools.combinations(keys, 2):
        if sources[left]["source"]["game"] != sources[right]["source"]["game"]:
            continue
        pair = left + "_vs_" + right
        comparison = version_census.compare(sources[left], sources[right])
        comparisons[pair] = comparison
        summary = {key: value for key, value in comparison.items() if not isinstance(value, dict)}
        for category in ("entities", "properties", "missions", "mission_values", "mission_static_values", "content"):
            if category in comparison:
                summary[category] = stats(comparison[category])
        summary["entity_kinds"] = comparison.get("entity_kinds", {})
        summary["content_only_by_kind"] = {
            side: dict(sorted(collections.Counter(r["kind"] for r in comparison["content"][side]).items()))
            for side in ("left_only", "right_only")}
        a_names, b_names = ({(r["kind"], r["name"]) for r in sources[k]["content"]
                             if r["name"] != "NM"} for k in (left, right))
        summary["content_name_sets"] = {
            "compared": len(a_names & b_names),
            "left_only_count": len(a_names - b_names),
            "right_only_count": len(b_names - a_names),
        }
        summary["geometry"] = {key: stats(value) for key, value in comparison.get("geometry", {}).items()}
        if "geometry" in sources[left]:
            summary["geometry_gate"] = geometry_gate(sources[left], sources[right])
        summaries[pair] = summary
    scripts = ("version_census.py", "version_geometry.py", "gen_missions.py", "extract_lua.py", "version_report.py",
               "version_heights.py", "version_collision.py", "version_terrain_play.py")
    return {"schema": 2,
            "tool_sha256": {name: digest(Path(__file__).with_name(name).read_bytes()) for name in scripts},
            "sources": receipts, "comparisons": summaries,
            "height_samples": height_rows(json.loads(p.read_text()) for p in heights),
            "collision_shapes": {p.stem: json.loads(p.read_text()) for p in shapes},
            "terrain_play": {doc["game"]: doc["terrains"] for doc in (json.loads(p.read_text()) for p in terrain)},
            "largest_numeric_differences": {game: numeric_groups(comparisons, game) for game in ("bf1", "bf2")}}


# ---- the tables analysis/version-diff.md quotes; the doc's blocks are generated so they cannot drift ----

PAIRS = ("bf1_ps2_vs_bf1_xbox", "bf2_ps2_vs_bf2_xbox", "bf2_ps2_vs_bf2_pc", "bf2_xbox_vs_bf2_pc")
PROPERTY_NAMES = ("MaxDistance", "LockOnRange", "OmniRadius", "DamageStartPercent", "RoundsPerClip", "NumParticles",
                  "MaxHealth", "Velocity", "Damage")


def ratio(row):
    return f"{row['changed']:,} / {row['compared']:,}"


def corpus_table(m):
    rows = ["| Corpus | UCFB LVLs | Class records | Mission records | Collision meshes | Skeletal clips | Candidate clip names |",
            "|---|---:|---:|---:|---:|---:|---:|"]
    for key in ("bf1_ps2", "bf1_xbox", "bf2_ps2", "bf2_xbox", "bf2_pc"):
        s = m["sources"][key]
        errors = sum(e.get("error") == "not_ucfb" for e in s["errors"])
        rows.append(f"| {key.replace('_', ' ').upper()} | {s['lvl_files'] - errors} | {s['entities']} | {s['missions']} | "
                    f"{s['geometry']['collision']} | {s['geometry']['animations']} | {s['animation_names_resolved']} |")
    rows.append("| BF1 PC | NM | NM | NM | NM | NM | NM |")
    return rows


def counts_table(m, game):
    pairs = (f"{game}_ps2_vs_{game}_xbox", f"{game}_ps2_vs_{game}_pc", f"{game}_xbox_vs_{game}_pc")
    comparisons = m["comparisons"]

    def geom(pair, category, field=None):
        x = comparisons[pair]["geometry"][category]
        return ratio(x) if field is None else f"{x['field_changes'].get(field, 0):,} / {x['compared']:,}"

    def heights(pair):
        h = m["height_samples"][pair]["summary"]
        return f"{h['changed']:,} / {h['compared']:,}"

    def names(pair):
        n = comparisons[pair]["content_name_sets"]
        only = n["left_only_count"] + n["right_only_count"]
        return f"{only:,} / {n['compared'] + only:,}"

    def surfaces(pair, *fields):
        c = comparisons[pair]["geometry"]["collision"]
        return f"{sum(c['surface_changes'][f] for f in fields):,} / {c['compared']:,}"

    def root(pair):
        a = comparisons[pair]["geometry"]["animations"]
        return f"{a['root_motion_changed']:,} / {a['root_motion_compared']:,}"

    table = [
        ("Class records (all four kinds)", lambda p: ratio(comparisons[p]["entities"])),
        ("Entity ODF records (`entc`)", lambda p: ratio(comparisons[p]["entity_kinds"]["entc"])),
        ("Weapon ODF records (`wpnc`)", lambda p: ratio(comparisons[p]["entity_kinds"]["wpnc"])),
        ("Ordnance ODF records (`ordc`)", lambda p: ratio(comparisons[p]["entity_kinds"]["ordc"])),
        ("Explosion ODF records (`expc`)", lambda p: ratio(comparisons[p]["entity_kinds"]["expc"])),
        ("Ordered property occurrence values", lambda p: ratio(comparisons[p]["properties"])),
        ("Direct mission declaration values", lambda p: ratio(comparisons[p]["mission_values"])),
        ("Effective mission execution", lambda p: "NM"),
        ("Collision mesh records", lambda p: geom(p, "collision")),
        ("Collision meshes, same polygons in another order", lambda p: surfaces(p, "order_only")),
        ("Collision meshes, same vertices, other polygons", lambda p: surfaces(p, "same_vertices_other_polygons")),
        ("Collision meshes, different vertices", lambda p: surfaces(p, "different_vertices")),
        ("Collision bounds", lambda p: geom(p, "collision", "bounds")),
        ("Collision primitives", lambda p: geom(p, "primitives")),
        ("Path and planning resources", lambda p: geom(p, "routes")),
        ("Terrain records (heights/cutters/grid)", lambda p: geom(p, "terrain")),
        ("Terrain height fingerprints", lambda p: geom(p, "terrain", "heights_sha256")),
        ("Terrain cutter fingerprints", lambda p: geom(p, "terrain", "cutters_sha256")),
        ("Terrain height samples at shared coordinates", heights),
        ("Placed objects (`inst`)", lambda p: ratio(comparisons[p]["geometry"]["world"]["by_kind"]["inst"])),
        ("All world records", lambda p: geom(p, "world")),
        ("Skeletal clip directory/root records", lambda p: geom(p, "animations")),
        ("Clip frame counts", lambda p: geom(p, "animations", "frames")),
        ("Gameplay root-motion trajectories", root),
        ("Corpus-only content names / union names", names),
    ]
    rows = ["| Category | PS2–Xbox | PS2–PC | Xbox–PC |", "|---|---:|---:|---:|"]
    for title, cell in table:
        rows.append(f"| {title} | " + " | ".join(cell(p) if p in comparisons else "NM" for p in pairs) + " |")
    return rows


def unmatched_table(m):
    table = [
        ("Classes L/R", lambda v: f"{v['entities']['left_only_count']}/{v['entities']['right_only_count']}"),
        ("Properties L/R; NM", lambda v: f"{v['properties']['left_only']}/{v['properties']['right_only']}; {v['properties']['unmeasured']}"),
        ("Mission records L/R", lambda v: f"{v['missions']['left_only_count']}/{v['missions']['right_only_count']}"),
        ("Mission declaration leaves L/R; NM subtrees", lambda v: f"{v['mission_values']['left_only']}/{v['mission_values']['right_only']}; {v['mission_values']['unmeasured']}"),
        ("Collision meshes L/R", lambda v: f"{v['geometry']['collision']['left_only_count']}/{v['geometry']['collision']['right_only_count']}"),
        ("Primitives L/R", lambda v: f"{v['geometry']['primitives']['left_only_count']}/{v['geometry']['primitives']['right_only_count']}"),
        ("Terrains L/R", lambda v: f"{v['geometry']['terrain']['left_only_count']}/{v['geometry']['terrain']['right_only_count']}"),
        ("Placed objects L/R", lambda v: f"{v['geometry']['world']['by_kind']['inst']['left_only']}/{v['geometry']['world']['by_kind']['inst']['right_only']}"),
        ("Animation records L/R; root-motion NM", lambda v: f"{v['geometry']['animations']['left_only_count']}/{v['geometry']['animations']['right_only_count']}; {v['geometry']['animations']['root_motion_NM']}"),
        ("Scoped content occurrences L/R", lambda v: f"{v['content']['left_only_count']}/{v['content']['right_only_count']}"),
        ("Unique content names L/R", lambda v: f"{v['content_name_sets']['left_only_count']}/{v['content_name_sets']['right_only_count']}"),
    ]
    rows = ["| Population | BF1 PS2–Xbox | BF2 PS2–Xbox | BF2 PS2–PC | BF2 Xbox–PC |", "|---|---:|---:|---:|---:|"]
    for title, cell in table:
        rows.append(f"| {title} | " + " | ".join(cell(m["comparisons"][p]) for p in PAIRS) + " |")
    return rows


def largest_table(m, game):
    known = {version_census.extract_lua.fnv1a(n): n for n in PROPERTY_NAMES}
    rows = ["| Name | Property | Left → right | Absolute delta | Pairs |", "|---|---|---:|---:|---|"]
    for r in m["largest_numeric_differences"][game]:
        f = r["field"]
        if isinstance(f[0], int):
            field = known.get(f[0], f"0x{f[0]:08x}") + (f" #{f[1]}" if f[1] else "")
        else:
            field = "/".join(map(str, f))
        pairs = ", ".join("P–X" if p.endswith("xbox") else "P–C" if "_ps2_vs_" in p else "X–C" for p in r["pairs"])
        rows.append(f"| `{r['name']}` | {field} | {r['left']:g} → {r['right']:g} | {r['absolute_delta']:g} | {pairs} |")
    return rows


def gate_table(m):
    rows = ["| Pair | Maps | Same ground and collision surfaces, nothing missing | Of those, with other paths or planning | "
            "Side LVL meshes: other surfaces, missing | Other shared LVL meshes: other surfaces, missing |",
            "|---|---:|---|---|---:|---:|"]
    for pair in ("bf1_ps2_vs_bf1_xbox", "bf2_ps2_vs_bf2_xbox", "bf2_ps2_vs_bf2_pc"):
        gate = m["comparisons"][pair]["geometry_gate"]
        names = ", ".join(f"`{Path(n).stem}`" for n in gate["same_shapes"])
        routes = ", ".join(f"`{Path(n).stem}`" for n in gate["same_shapes_other_routes"]) or "none"
        rows.append(f"| {pair.replace('_vs_', ' – ').replace('_', ' ').upper()} | {gate['maps']} | {len(gate['same_shapes'])}: {names} | {routes} | "
                    f"{sum(gate['side_meshes_with_other_surfaces'].values())}, {sum(gate['side_meshes_missing'].values())} | "
                    f"{sum(gate['other_shared_meshes_with_other_surfaces'].values())}, {sum(gate['other_shared_meshes_missing'].values())} |")
    return rows


def terrain_table(m):
    rows = ["| Game, map | Changed | Beyond boundary (largest) | Undrawn | Exposed in play: samples, >1 m, largest | "
            "In play under a placed object: samples, largest | Soldier arcs with exposed change >10 cm / >1 m (largest) / all | Spawn nodes, posts moved on terrain |",
            "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for game, terrains in sorted(m["terrain_play"].items()):
        rows.append(f"| {game.upper()} all, each terrain once | {sum(t['total_changed'] for t in terrains):,} | "
                    f"{sum(t['outside_boundary']['samples'] for t in terrains):,} | "
                    f"{sum(t['classes']['unpainted']['samples'] for t in terrains):,} | "
                    f"{sum(t['in_play_exposed']['samples'] for t in terrains):,}, "
                    f"{sum(t['in_play_exposed']['above_1m'] for t in terrains):,} | "
                    f"{sum(t['in_play_under_object']['samples'] for t in terrains):,} | | |")
        for t in terrains:
            out, play, under = t["outside_boundary"], t["in_play_exposed"], t["in_play_under_object"]
            lines = t["play_lines"]
            arcs = lines["soldier_arcs"]
            exposed = arcs["exposed"]
            rows.append(
                f"| {game.upper()} {t['file']} | {t['total_changed']:,} | {out['samples']:,} ({out['max_abs_metres']:.2f} m) | "
                f"{t['classes']['unpainted']['samples']:,} | {play['samples']:,}, {play['above_1m']:,}, {play['max_abs_metres']:.2f} m | "
                f"{under['samples']:,}, {under['max_abs_metres']:.2f} m | "
                f"{exposed['above_10cm']} / {exposed['above_1m']} ({exposed['max_abs_metres']:.2f} m) / {arcs['arcs']} | "
                f"{lines['spawn_nodes']['moved_on_terrain']}, {lines['command_posts']['moved_on_terrain']} |")
    return rows


def collision_table(m):
    rows = ["| Pair | Matched | Different surfaces | ≤1 cm | ≤10 cm | ≤1 m | ≤5 m | >5 m | Whole-mesh shifts | Largest |",
            "|---|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    for pair in PAIRS:
        c = m["collision_shapes"][pair]
        bands = c["nearest_vertex_gap_bands"]
        top = c["largest"][0] if c["largest"] else None
        rows.append(f"| {pair.replace('_vs_', ' – ').replace('_', ' ').upper()} | {c['matched_meshes']:,} | {c['different_surfaces']:,} | "
                    f"{bands['up_to_1cm']} | {bands['up_to_10cm']} | {bands['up_to_1m']} | {bands['up_to_5m']} | {bands['over_5m']} | "
                    f"{len(c['whole_mesh_translations'])} | " + (f"`{top[1]}` {top[2]:.1f} m" if top else "none") + " |")
    return rows


def doc_blocks(m):
    return {"corpus": corpus_table(m), "bf1": counts_table(m, "bf1"), "bf2": counts_table(m, "bf2"),
            "unmatched": unmatched_table(m), "largest-bf1": largest_table(m, "bf1"), "largest-bf2": largest_table(m, "bf2"),
            "terrain": terrain_table(m), "collision": collision_table(m), "gate": gate_table(m)}


def render_doc(text, m):
    """Replace each `<!-- generated:NAME -->` ... `<!-- /generated -->` block. An unknown or missing name refuses."""
    blocks = doc_blocks(m)
    seen = set()

    def fill(match):
        seen.add(match.group(1))
        return f"<!-- generated:{match.group(1)} -->\n" + "\n".join(blocks[match.group(1)]) + "\n<!-- /generated -->"

    try:
        text = re.sub(r"<!-- generated:([a-z0-9-]+) -->.*?<!-- /generated -->", fill, text, flags=re.S)
    except KeyError as exc:
        raise ValueError(f"unknown generated block {exc}") from exc
    if seen != blocks.keys():
        raise ValueError(f"doc is missing generated blocks: {sorted(blocks.keys() - seen)}")
    return text


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("census", type=Path, nargs="*")
    parser.add_argument("--heights", type=Path, nargs="*", default=[], help="version_heights.py outputs")
    parser.add_argument("--shapes", type=Path, nargs="*", default=[], help="version_collision.py outputs, named <pair>.json")
    parser.add_argument("--terrain", type=Path, nargs="*", default=[], help="version_terrain_play.py outputs")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--update-doc", type=Path, help="rewrite this doc's generated blocks from --receipt")
    parser.add_argument("--receipt", type=Path)
    args = parser.parse_args()
    try:
        if args.update_doc:
            args.update_doc.write_text(render_doc(args.update_doc.read_text(), json.loads(args.receipt.read_text())))
            return
        if not args.census or not args.out:
            parser.error("give censuses and --out, or --update-doc and --receipt")
        result = generate(args.census, args.heights, args.shapes, args.terrain)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(result, indent=1, sort_keys=True, allow_nan=False) + "\n")
    except (ValueError, OSError, KeyError) as exc:
        parser.exit(2, f"version report: {exc}\n")


if __name__ == "__main__":
    main()
