#!/usr/bin/env python3
"""Bake only the optional DLC heroes from content-detected local inputs.

scripts/rebake.sh runs this after its side group, on the folders scripts/input_scan.py
chose. Missing DLC is a no-op. This helper does not invalidate caches or remove a
previous bake: rebake.sh and pack.py own the output and its staleness.
"""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

from dlc_inputs import detect_dlc, detect_hero_sounds


def bake_dlc(inputs, ingame, out, bake_binary):
    """Bake one preferred side source; return its DlcSource, or None if absent.

    PS2 hero geometry/gameplay wins when available; a PC alternative supplies
    higher resolution textures. Only selected side LVLs are materialized in a
    temporary directory outside ``out``. Map and script files
    are never extracted. Errors in an identified source propagate to the caller.
    ``out`` must be an owned directory, with no symlink in its ancestry.
    """
    if isinstance(inputs, (str, os.PathLike)):
        inputs = [inputs]
    roots = [Path(path).absolute() for path in inputs]
    candidates = [source for root in roots for source in detect_dlc(root)]
    if not candidates:
        return None
    chosen = min(candidates, key=lambda source: (
        source.platform != "ps2", str(source.path), source.member or "", source.offset))
    pc = min((source for source in candidates if source.platform == "pc"),
             key=lambda source: (str(source.path), source.member or "", source.offset),
             default=None) if chosen.platform == "ps2" else None
    out = Path(os.path.abspath(out))
    if any(path.is_symlink() for path in (out, *out.parents)):
        raise ValueError("DLC output must be an owned directory, without symlink ancestors")
    if out == Path(out.anchor) or out.is_file():
        raise ValueError("DLC output must be an owned directory")
    for root in roots:
        if root.is_dir() and (out == root.resolve() or root.resolve() in out.parents):
            raise ValueError("DLC output must be outside the input directories")
    if out == chosen.path.resolve():
        raise ValueError("DLC output cannot replace its source")
    ingame = Path(ingame).absolute()
    if not ingame.is_file():
        raise ValueError("DLC heroes need the PS2 BF2 INGAME.LVL reference")
    binary = str(Path(bake_binary).absolute())
    with tempfile.TemporaryDirectory(prefix="openbf-dlc-") as temporary:
        lvl = Path(temporary) / "heroes.lvl"
        lvl.write_bytes(chosen.read_bytes())
        command = [binary, "dlc-heroes", str(lvl), str(ingame), str(out)]
        if pc is not None:
            donor = Path(temporary) / "pc-heroes.lvl"
            donor.write_bytes(pc.read_bytes())
            command.append(str(donor))
        subprocess.run(command, check=True)
    bake_sounds(roots, ingame, out, binary)
    return chosen


def bake_sounds(roots, ingame, out, binary):
    """The heroes' own sound effects, when a source folder holds them. Optional: a hero without them still plays, so a
    bank this cannot read is reported and left out rather than stopping the build."""
    banks = (detect_hero_sounds(root) for root in roots)
    bank = min(filter(None, banks), key=lambda path: path.stat().st_size, default=None)
    core = next((path for path in ingame.parent.iterdir() if path.name.lower() == "core.lvl"), None)
    if bank is None or core is None:
        print("DLC heroes: no sound bank of their own found; they use the game's sounds")
        return
    try:
        subprocess.run([binary, "dlc-sounds", str(bank), str(core), str(out)], check=True)
    except subprocess.CalledProcessError as error:
        print(f"DLC heroes: their sound bank was not baked ({error}); they use the game's sounds")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", action="append", required=True,
                        help="folder, side LVL or ISO; repeat to compare alternatives")
    parser.add_argument("--ingame", required=True, help="PS2 BF2 INGAME.LVL")
    parser.add_argument("--out", required=True, help="owned assets output directory")
    parser.add_argument("--bake", required=True, help="path to the compiled bake executable")
    args = parser.parse_args()
    try:
        source = bake_dlc(args.source, args.ingame, args.out, args.bake)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"DLC bake: {error}\n")
    if source is None:
        print("DLC heroes absent; ordinary game assets unchanged")
    else:
        member = f"::{source.member}" if source.member else ""
        print(f"DLC heroes baked from {source.path}{member} ({source.platform})")


if __name__ == "__main__":
    main()
