#!/usr/bin/env python3
"""Measure crop envelopes for PS2 pieces absent from a donor's whole LVL inventory.

Only model names and six INFO bounds numbers leave this tool; no render vertices,
indices, materials or textures. The bake clips the player's own placed artwork.
"""
import argparse
import json
from pathlib import Path
import struct

import version_census as census
import version_geometry as geometry


def resources(path):
    return list(census.walk_resources(path.read_bytes()))


def donor_names(root):
    names = set()
    for path in sorted(root.rglob("*")):
        if path.suffix.lower() != ".lvl":
            continue
        for _, tag, payload, _ in resources(path):
            if tag in (b"modl", b"gmod"):
                names.add(geometry.name(geometry.child(payload, b"NAME")))
    return names


def measure(reference, maps, donors):
    inventories = [donor_names(root) for root in donors]
    output = {}
    for relative in maps:
        path = next(p for p in reference.rglob("*") if p.relative_to(reference).as_posix().lower() == relative.lower())
        content = resources(path)
        placed = set()
        classes = {}
        for _, tag, payload, _ in content:
            if tag == b"wrld":
                placed.update(r["class"] for r in geometry.world(payload) if r["kind"] == "inst")
            elif tag == b"entc":
                name = census.cstr(geometry.child(payload, b"TYPE")).lower()
                for key, value in geometry.chunks(payload):
                    if key == b"PROP" and struct.unpack_from("<I", value)[0] == 0x47C86B4A:
                        classes[name] = census.cstr(value[4:]).lower()
        wanted = {classes[n] for n in placed if n in classes and any(classes[n] not in names for names in inventories)}
        for _, tag, payload, _ in content:
            if tag != b"modl":
                continue
            name = geometry.name(geometry.child(payload, b"NAME"))
            if name not in wanted:
                continue
            info = geometry.child(payload, b"INFO")
            offset = 16 if len(info) >= 72 else 12
            values = struct.unpack_from("<6f", info, offset)
            row = {"min": values[:3], "max": values[3:]}
            if name in output and output[name] != row:
                raise ValueError(f"map-dependent render envelope: {name}")
            output[name] = row
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("--donor", type=Path, action="append", required=True)
    parser.add_argument("--map", action="append", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    rows = measure(args.reference, args.map, args.donor)
    args.out.write_text(json.dumps(rows, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n")
    print(f"{len(rows)} measured render crop envelopes -> {args.out}")


if __name__ == "__main__":
    main()
