#!/usr/bin/env python3
"""How far apart are two editions' collision meshes? Names and distances only.

    python3 scripts/version_collision.py LEFT_INSTALL_OR_DISC RIGHT_INSTALL_OR_DISC --out shapes.json

For every mesh matched by file, sublevel, name and node whose polygons differ, report the largest distance from a
vertex of either mesh to the nearest vertex of the other. That is an upper bound on how far the surface moved when
vertices merely shift, and an overestimate when a munge retessellates a flat wall. Decoded vertices stay in memory.
"""

import argparse
from bisect import bisect_left
from collections import Counter
import json
import math
from pathlib import Path
import struct

import version_census as census
import version_geometry as geometry

BANDS = (("up_to_1cm", 0.01), ("up_to_10cm", 0.1), ("up_to_1m", 1.0), ("up_to_5m", 5.0))
LARGEST = 15


def nearest_gap(points, others):
    """Largest distance from a point to its nearest neighbour in others, by a sweep along x."""
    if not points or not others:
        return math.inf
    others = sorted(others)
    xs = [p[0] for p in others]
    worst = 0.0
    for x, y, z in points:
        at = bisect_left(xs, x)
        best = math.inf
        for step in (1, -1):
            i = at if step == 1 else at - 1
            while 0 <= i < len(others):
                dx = others[i][0] - x
                if dx * dx >= best:
                    break
                d = dx * dx + (others[i][1] - y) ** 2 + (others[i][2] - z) ** 2
                best = min(best, d)
                i += step
        worst = max(worst, best)
    return math.sqrt(worst)


def rigid_offset(left, right):
    """The one translation taking left's vertices onto right's, or None. A table can carry that; it cannot carry a mesh."""
    if len(left) != len(right) or not left:
        return None
    a, b = sorted(left), sorted(right)
    offset = [b[0][i] - a[0][i] for i in range(3)]
    if all(abs(q[i] - p[i] - offset[i]) <= 1e-3 for p, q in zip(a, b) for i in range(3)):
        return offset
    return None


def meshes(source):
    out = {}
    for file, buf in census.input_files(source):
        if buf[:4] != b"ucfb":
            continue
        counts = Counter()
        for scope, tag, payload, _offset in census.walk_resources(buf):
            if tag != b"coll":
                continue
            row = geometry.collision(payload)[0]
            _name, _node, points = geometry.collision_positions(payload)
            key = (file, tuple(scope), row["name"], row["node"])
            index = counts[key]
            counts[key] += 1
            out[key + (index,)] = (row["surface_sha256"], points)
    return out


def compare(left, right):
    a, b = meshes(left), meshes(right)
    matched = sorted(a.keys() & b.keys())
    changed = [k for k in matched if a[k][0] != b[k][0]]
    bands = Counter()
    by_file = Counter()
    largest = []
    translated = []
    for key in changed:
        gap = max(nearest_gap(a[key][1], b[key][1]), nearest_gap(b[key][1], a[key][1]))
        band = next((label for label, limit in BANDS if gap <= limit), "over_5m")
        bands[band] += 1
        if gap > BANDS[0][1]:
            by_file[key[0]] += 1
            largest.append([key[0], key[2], round(gap, 3)])
            offset = rigid_offset(a[key][1], b[key][1])
            if offset:
                translated.append([key[0], key[2], [round(v, 3) for v in offset]])
    return {
        "schema": 1,
        "matched_meshes": len(matched),
        "different_surfaces": len(changed),
        "nearest_vertex_gap_bands": {label: bands[label] for label in [b[0] for b in BANDS] + ["over_5m"]},
        "whole_mesh_translations": translated,
        "moved_over_1cm_by_file": dict(sorted(by_file.items())),
        "largest": sorted(largest, key=lambda r: (-r[2], r[0], r[1]))[:LARGEST],
        "left_only": len(a.keys() - b.keys()),
        "right_only": len(b.keys() - a.keys()),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("left", type=Path)
    parser.add_argument("right", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = compare(args.left, args.right)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(result, indent=1, sort_keys=True, allow_nan=False) + "\n")
    except (ValueError, OSError, struct.error) as exc:
        parser.exit(2, f"collision: {exc}\n")


if __name__ == "__main__":
    main()
