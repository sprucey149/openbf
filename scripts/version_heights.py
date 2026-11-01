#!/usr/bin/env python3
"""Compare decoded terrain samples from two Battlefront installs or discs.

python3 scripts/version_heights.py compare INSTALL_OR_DISC INSTALL_OR_DISC --out heights.json

Only names, hashes and numerical summaries are written. Decoded height grids
stay in memory. Samples compare at the same world coordinates; no interpolation
is used. Masked, cut-out and unplayable cells remain in these authored grids.
"""

import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import struct

import extract_lua
import version_census as census
import version_geometry as geometry


def read_source(path):
    rows = []
    files = []
    platforms = Counter()
    games = set()
    skipped = []
    for file, buf in census.input_files(path):
        if buf[:4] != b"ucfb":
            skipped.append(file)
            continue
        has_terrain = False
        for scope, tag, payload, offset in census.walk_resources(buf):
            if tag == b"tex_":
                fields = dict(geometry.chunks(payload))
                platform = (
                    "pc"
                    if b"FMT_" in fields
                    else (
                        "xbox"
                        if b"BODY" in fields and len(fields.get(b"INFO", b"")) == 20
                        else "ps2"
                    )
                )
                platforms[platform] += 1
            elif tag == b"scr_" and file == "mission.lvl":
                header = extract_lua.parse_lua_header(
                    geometry.child(payload, b"BODY", b"")
                )
                if (
                    header
                    and not header.get("error")
                    and header["version"] in ("4.0", "5.0")
                ):
                    games.add("bf1" if header["version"] == "4.0" else "bf2")
            elif tag == b"tern":
                has_terrain = True
                for row in geometry.terrain(payload, retain_samples=True):
                    rows.append({"file": file, "scope": scope, **row})
        if has_terrain:
            files.append({"name": file, "sha256": hashlib.sha256(buf).hexdigest()})
    if len(games) != 1 or len(platforms) != 1 or not rows:
        raise ValueError("unknown/mixed source or no terrain records")
    return {
        "game": next(iter(games)),
        "platform": next(iter(platforms)),
        "terrain_files": files,
        "skipped_non_ucfb": skipped,
    }, rows


def compare(left_source, right_source):
    a_source, a = read_source(left_source)
    b_source, b = read_source(right_source)
    if a_source["game"] != b_source["game"]:
        raise ValueError("compare editions of the same game")
    rows = geometry.compare_height_samples(a, b)
    ai = geometry.indexed_rows(a, "terrain")
    bi = geometry.indexed_rows(b, "terrain")
    measured = [row for row in rows if row["compared"] != "NM"]
    count = sum(row["compared"] for row in measured)
    summary = {
        "compared": count,
        "changed": sum(row["changed"] for row in measured),
        "max_abs_metres": max(
            (row.get("max_abs_metres", 0) for row in measured), default=0
        ),
        "rms_metres": (
            math.sqrt(
                sum(row.get("rms_metres", 0) ** 2 * row["compared"] for row in measured)
                / count
            )
            if count
            else "NM"
        ),
        "left_only_samples_on_shared_terrains": sum(
            row["left_only"] for row in measured
        ),
        "right_only_samples_on_shared_terrains": sum(
            row["right_only"] for row in measured
        ),
        "unmeasured_terrain_pairs": sum(row["compared"] == "NM" for row in rows),
    }
    return {
        "schema": 1,
        "left": a_source,
        "right": b_source,
        "summary": summary,
        "terrains": rows,
        "left_only_terrains": [
            {"identity": k, "samples": ai[k].get("samples", 0)}
            for k in sorted(ai.keys() - bi.keys())
        ],
        "right_only_terrains": [
            {"identity": k, "samples": bi[k].get("samples", 0)}
            for k in sorted(bi.keys() - ai.keys())
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    command = sub.add_parser("compare")
    command.add_argument("left", type=Path)
    command.add_argument("right", type=Path)
    command.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = compare(args.left, args.right)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(
            json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + "\n"
        )
    except (ValueError, OSError, struct.error, OverflowError) as exc:
        parser.exit(2, f"heights: {exc}\n")


if __name__ == "__main__":
    main()
