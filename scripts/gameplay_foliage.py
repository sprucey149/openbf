#!/usr/bin/env python3
"""Generate PS2 foliage lookup numbers, without copying executable bytes.

Usage: python3 scripts/gameplay_foliage.py --bf1 <SWBFRONT.ELF> \
    --bf2 <SWBFRNT2.ELF> --output engine/crates/manifest/data/gameplay/foliage-v1.json

Addresses and checksums are the existing bake::foliage contract. Unknown
table contents are refused. Output is a data artifact, not yet a bake override.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

ADDRESSES = {"bf1": (0x5453C8, 0x518948), "bf2": (0x636AC8, 0x5FFB20)}
CHECKSUMS = {"rng": 0xF1FDBA48, "atlas": 0x000BE48D}


def fnv(data: bytes) -> int:
    value = 2166136261
    for byte in data:
        value = ((value ^ byte) * 16777619) & 0xFFFFFFFF
    return value


def elf_bytes(data: bytes, address: int, size: int) -> bytes:
    """Resolve a virtual range through an ELF32 little-endian LOAD segment."""
    if len(data) < 52 or data[:7] != b"\x7fELF\x01\x01\x01":
        raise ValueError("expected ELF32 little-endian executable")
    if struct.unpack_from("<H", data, 18)[0] != 8:
        raise ValueError("expected MIPS executable")
    phoff = struct.unpack_from("<I", data, 28)[0]
    phsize, phnum = struct.unpack_from("<HH", data, 42)
    if phsize < 32 or phoff + phsize * phnum > len(data):
        raise ValueError("invalid program header table")
    matches = []
    for i in range(phnum):
        kind, offset, vaddr, _, filesz = struct.unpack_from("<IIIII", data, phoff + i * phsize)
        if kind == 1 and vaddr <= address and address + size <= vaddr + filesz:
            begin = offset + address - vaddr
            if begin + size > len(data):
                raise ValueError("LOAD segment points beyond executable")
            matches.append(data[begin:begin + size])
    if len(matches) != 1:
        raise ValueError(f"expected one LOAD segment for {address:#x}; found {len(matches)}")
    return matches[0]


def extract(data: bytes, game: str) -> dict:
    rng_addr, atlas_addr = ADDRESSES[game]
    rng = elf_bytes(data, rng_addr, 1024)
    atlas = elf_bytes(data, atlas_addr, 0x110)
    for name, raw in (("rng", rng), ("atlas", atlas)):
        if fnv(raw) != CHECKSUMS[name]:
            raise ValueError(f"{game} {name} checksum mismatch; unsupported executable")
    values = list(struct.unpack("<256f", rng))
    if not all(math.isfinite(v) for v in values):
        raise ValueError("non-finite placement value")
    atlases = {}
    for name, offset, parts in (("1", 0xD0, 4), ("3", 0, 3), ("4", 0x90, 4), ("default", 0x30, 6)):
        words = struct.unpack_from(f"<{parts * 8}H", atlas, offset)
        atlases[name] = [[[words[p * 8 + c * 2] / 2048, words[p * 8 + c * 2 + 1] / 2048]
                         for c in range(4)] for p in range(parts)]
    return {
        "game": game,
        "source_sha256": hashlib.sha256(data).hexdigest(),
        "placement_virtual_address": rng_addr,
        "placement_fnv1a32": fnv(rng),
        "placement_values": values,
        "atlas_virtual_address": atlas_addr,
        "atlas_fnv1a32": fnv(atlas),
        "atlas_source_uv_divisor": 2048,
        "atlas_uv_by_num_parts": atlases,
    }


def generate(bf1: Path, bf2: Path) -> dict:
    rows = [extract(bf1.read_bytes(), "bf1"), extract(bf2.read_bytes(), "bf2")]
    if rows[0]["placement_values"] != rows[1]["placement_values"]:
        raise ValueError("BF1/BF2 placement tables differ")
    if rows[0]["atlas_uv_by_num_parts"] != rows[1]["atlas_uv_by_num_parts"]:
        raise ValueError("BF1/BF2 atlas tables differ")
    return {"schema": 1, "reference_platform": "ps2", "tables": rows}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bf1", type=Path, required=True)
    parser.add_argument("--bf2", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = generate(args.bf1, args.bf2)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n")
    except (OSError, ValueError, struct.error) as error:
        parser.exit(2, f"gameplay-foliage: {error}\n")


if __name__ == "__main__":
    main()
