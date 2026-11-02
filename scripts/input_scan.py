#!/usr/bin/env python3
"""What is in iso/ and what a build makes of it. Every input is known by its content, never its name.

    python3 scripts/input_scan.py [--iso-dir iso]     print what was found and chosen; exit 2 on a refusal
    python3 scripts/input_scan.py --shell             the same, as shell variables for rebake.sh

Recognized: PS2 discs (ISO9660, system ID PLAYSTATION), Xbox discs (XDVDFS, trimmed or a full dump), PC install
folders, and the Xbox DLC folder. Anything else is named and refused, not guessed at: it is never read as game data.

The choice, per game:
  - gameplay and shared UI: the PS2 data whenever it is present. Numbers and shapes the sim reads never change with
    what else went in. The optional DLC heroes are the one exception, and only once a player turns on the DLC Heroes row
    in Hero Options (scripts/DLC.md).
  - look: every other copy is a texture donor. bake::tier keeps the largest same-art copy of each texture.
  - each game needs one source, and any PC input also needs a console BF2 disc. A folder that holds other, unusable
    things still builds; the summary says what was refused. A game left with no usable source stops the build, and
    the message says what to add.

Each SUPPORTED switch below is the one line that turns on an input a build does not take yet. The recognition, the
choice, pack.py's and rebake.sh's use of it and the test matrix are all in place behind it.
"""
import argparse
import json
import os
from pathlib import Path
import shlex
import struct
import subprocess
import sys

import console_input as content
import dlc_inputs

# An Xbox disc bakes to the PS2's game from what a clone ships: public instruction tables over the disc's own files, each
# file fingerprinted so that a changed one refuses the bake (analysis/source-gates.md has the proof per map). pack.py
# extracts each chosen Xbox disc with XBOX_EXTRACT (`bake extract-xiso <disc> <dir>` writes <dir>/Data/_LVL_XBOX/...), and
# rebake.sh passes those LVLs to the bake's donor argument, comma-joined after any PC LVL.
XBOX_SUPPORTED = True
XBOX_EXTRACT = "extract-xiso"
# scripts/bake_dlc.py reads Kit Fisto and Asajj Ventress out of the DLC folder (OPENBF_DLC_DIR, and OPENBF_DLC_DONOR for
# a second copy's larger textures). Nothing else in the DLC is used, whatever it holds.
DLC_SUPPORTED = True
# A PC BF1 install can be BF1's only source with nothing beyond the public code and the player's own files.
PC_PRIMARY_SUPPORTED = True
# Differently partitioned structures are drawn from exact shared resources
# or lower-detail, own-copy textured stand-ins. All maps are now included;
# their availability does not establish complete render/collision alignment.
WITHHELD_MAPS = {}

GAMES = ("bf1", "bf2")
SHORT = {"bf1": "BF1", "bf2": "BF2"}
TITLE = {"bf1": "Star Wars: Battlefront", "bf2": "Star Wars: Battlefront II"}
TREE_FILE = {"bf1": "COMMON/ENG/COMMON.LVL", "bf2": "INGAME.LVL"}

XBOX_MAGIC = b"MICROSOFT*XBOX*MEDIA"
# Trimmed image, then the game partition of an XGD1, XGD2 and XGD3 full dump.
XBOX_PARTITIONS = (0, 0x18300000, 0xFD90000, 0x2080000)
XBOX_GAME_FILES = {"bf1": "data/_lvl_xbox/common/eng/common.lvl", "bf2": "data/_lvl_xbox/ingame.lvl"}
SECTOR = 2048
WRAPPERS = ((b"CISO", "a CSO compressed disc image"), (b"ZISO", "a ZSO compressed disc image"),
            (b"MComprHD", "a CHD compressed disc image"), (b"PK\x03\x04", "a zip archive"),
            (b"7z\xbc\xaf\x27\x1c", "a 7z archive"), (b"Rar!", "a rar archive"), (b"\x1f\x8b", "a gzip archive"))
REMASTER = "a 64-bit game folder, as Aspyr's Classic Collection is; only the original 32-bit PC releases are supported"
UNKNOWN_FILE = "an unknown format; not read"
LARGE = 64 << 20  # a file this big and unrecognized is somebody's game data, not a stray note
MAX_DEPTH = 12
LEVELS_ABOVE = 3  # GameData/Data/_LVL_PC: how far above its data a game folder's program sits


def entry(path, kind, reason, game=None, status="found"):
    return dict(path=str(path), kind=kind, game=game, status=status, reason=reason)


def refused(path, reason, game=None):
    return entry(path, "unknown", reason, game, "refused")


def xdvdfs_files(path):
    """(partition offset, lowercase file paths) of an Xbox disc, or None when the magic is not there."""
    try:
        with open(path, "rb") as source:
            length = source.seek(0, 2)
            for base in XBOX_PARTITIONS:
                source.seek(base + 32 * SECTOR)
                if source.read(len(XBOX_MAGIC)) == XBOX_MAGIC:
                    break
            else:
                return None
            root_sector, root_size = struct.unpack("<II", source.read(8))
            files, stack, seen_dirs, total = set(), [("", root_sector, root_size)], set(), 0
            while stack:
                prefix, sector, size = stack.pop()
                if not size or sector in seen_dirs:
                    continue
                seen_dirs.add(sector)
                total += size
                offset = base + sector * SECTOR
                if len(seen_dirs) > 4096 or size > 4 << 20 or total > 16 << 20 or offset + size > length:
                    return base, set()
                source.seek(offset)
                table = source.read(size)
                todo, seen = [0], set()
                while todo:
                    at = todo.pop()
                    if at in seen or at + 14 > len(table):
                        continue
                    seen.add(at)
                    left, right, child_sector, child_size, attributes, name_size = struct.unpack_from("<HHIIBB", table, at)
                    if left == 0xFFFF and right == 0xFFFF:  # the padding at the end of a sector, as the bake reads it
                        continue
                    name = table[at + 14:at + 14 + name_size].decode("latin-1").lower()
                    todo += [word * 4 for word in (left, right) if word and word != 0xFFFF]
                    if attributes & 0x10:
                        stack.append((prefix + name + "/", child_sector, child_size))
                    else:
                        if child_size and base + child_sector * SECTOR + child_size > length:
                            return base, set()  # cut short: its data is not all there
                        files.add(prefix + name)
            return base, files
    except (OSError, ValueError, struct.error):
        return None


def pe_is_64bit_program(path):
    """A Windows program, not a library, built for a 64-bit CPU. The original PC releases are 32-bit."""
    try:
        with open(path, "rb") as source:
            head = source.read(4096)
        if head[:2] != b"MZ":
            return False
        at = struct.unpack_from("<I", head, 0x3C)[0]
        if head[at:at + 4] != b"PE\0\0":
            return False
        machine, characteristics = struct.unpack_from("<H", head, at + 4)[0], struct.unpack_from("<H", head, at + 22)[0]
        return machine in (0x8664, 0xAA64) and not characteristics & 0x2000
    except (OSError, struct.error):
        return False


def classify_file(path):
    """One file: a disc entry, a refusal, or None for something that is no kind of game image."""
    path = Path(path)
    try:
        with path.open("rb") as source:
            head = source.read(16)
            source.seek(16 * SECTOR)
            volume = source.read(7)
            source.seek(16 * 2352 + 24)  # a raw 2352-byte sector dump: the volume descriptor after its sector header
            raw = source.read(7)
        size = path.stat().st_size
    except OSError as error:
        return refused(path, f"cannot be read: {error}")
    xbox = xdvdfs_files(path)
    if xbox is not None:
        layout = "trimmed" if xbox[0] == 0 else f"full dump, game partition at {xbox[0]:#x}"
        for game, name in XBOX_GAME_FILES.items():
            if name in xbox[1] and "data/_lvl_xbox/core.lvl" in xbox[1]:
                return entry(path, "xbox-disc", f"Xbox {TITLE[game]} disc ({layout})", game)
        return refused(path, f"an Xbox disc, but not Battlefront or cut short ({layout})")
    if volume == b"\x01CD001\x01":
        system, files = content.iso9660_files(path)
        game = "bf2" if content.REQUIRED <= files else "bf1" if content.REQUIRED_BF1 <= files else None
        if system == b"PLAYSTATION":
            if game:
                return entry(path, "ps2-disc", f"PS2 {TITLE[game]} disc", game)
            return refused(path, "a PS2 disc, but not Battlefront or cut short")
        if game:
            return refused(path, f"a rebuilt or modified {TITLE[game]} disc (no PLAYSTATION system ID); the gameplay "
                                 "reference must be the original disc", game)
        return refused(path, "a disc image of an unknown format")
    if raw == b"\x01CD001\x01":
        return refused(path, "a raw 2352-byte disc dump (BIN); convert it to a 2048-byte .iso")
    for magic, what in WRAPPERS:
        if head.startswith(magic):
            return refused(path, f"{what}; unpack it to a plain disc image")
    if pe_is_64bit_program(path):
        return refused(path, REMASTER)
    if size >= LARGE:
        return refused(path, UNKNOWN_FILE)
    return None


def tree_platform(directory):
    """(game, platform) of a directory holding a whole game's LVLs at its top, or None."""
    if not content.ci_path(directory, "CORE.LVL").is_file():
        return None
    for game in ("bf2", "bf1"):
        target = content.ci_path(directory, TREE_FILE[game])
        if target.is_file():
            platform = content.geometry_platform(target)
            if platform:
                return game, platform
    return None


def dlc_platform(directory):
    """The format of the two DLC heroes' side LVL under here, or None: known by its classes and geometry, whatever the
    files are called. The side the bake prefers when the folder holds both."""
    platforms = {source.platform for source in dlc_inputs.detect_dlc(directory)}
    return "ps2" if "ps2" in platforms else platforms.pop() if platforms else None


def read_head(path, size):
    if not os.path.isfile(path):  # opening a pipe would wait for a writer that never comes
        return b""
    try:
        with open(path, "rb") as source:
            return source.read(size)
    except OSError:
        return b""


def is_addon_script(path):
    head = read_head(path, 32)
    return head[:4] == b"ucfb" and head[8:12] == b"scr_" and b"addme" in head


def remaster_beside(directory, root):
    """A 64-bit program in this directory or between it and the scan root. A folder linked in from outside the root
    is checked as far up as a folder named on the command line is, or a link would hide the program from the scan."""
    here, root = Path(directory).resolve(), Path(root).resolve()
    outside = LEVELS_ABOVE
    while True:
        try:
            if any(path.is_file() and pe_is_64bit_program(path) for path in sorted(here.iterdir())[:256]):
                return True
        except OSError:
            pass
        if here == root or here == here.parent:
            return False
        if root not in here.parents:
            outside -= 1
            if outside < 0:
                return False
        here = here.parent


def classify_dir(directory, root):
    """A directory that is one input as a whole (nothing under it needs looking at), or None."""
    try:
        return classify_readable_dir(directory, root)
    except OSError as error:
        return refused(directory, f"cannot be read: {error}")


def classify_readable_dir(directory, root):
    tree = tree_platform(directory)
    if tree:
        game, platform = tree
        if platform == "pc":
            if remaster_beside(directory, root):
                return refused(directory, REMASTER, game)
            return entry(directory, "pc-install", f"PC {TITLE[game]} install", game)
        if platform == "ps2":
            return refused(directory, f"an extracted PS2 {TITLE[game]} folder; add the disc image instead", game)
        return refused(directory, f"an extracted Xbox {TITLE[game]} folder; add the disc image instead", game)
    addon = is_addon_script(content.ci_path(directory, "addme.script"))
    if addon or read_head(content.ci_path(directory, "CORE.LVL"), 4) == b"ucfb":
        platform = dlc_platform(directory)
        if platform:
            return dict(entry(directory, "dlc", "BF2 DLC folder holding Kit Fisto and Asajj Ventress", "bf2"), platform=platform)
        if addon:
            return entry(directory, "addon", "an addon folder without the DLC heroes; not used", None, "ignored")
        return refused(directory, "game data in no format this build reads (for example Aspyr's Classic Collection)")
    return None


def scan_root(root):
    """Everything recognized or refused under one folder, plus how many stray files were left out."""
    root = Path(root)
    found, strays, input_dirs = [], 0, []
    whole = classify_dir(root, root) if root.is_dir() else None
    if whole:
        return [whole], 0
    base_depth = len(root.parts)
    seen = {os.path.realpath(root)}  # links are followed, as a disc linked into iso/ is an input; a link loop ends here
    for here, directories, names in os.walk(root, followlinks=True):
        here = Path(here)
        keep = []
        for name in sorted(directories):
            child = here / name
            if os.path.realpath(child) in seen:
                continue
            seen.add(os.path.realpath(child))
            result = classify_dir(child, root)
            if result:
                found.append(result)
                if result["kind"] != "addon":
                    input_dirs.append(child)
            elif len(child.parts) - base_depth < MAX_DEPTH:
                keep.append(name)
        directories[:] = keep
        for name in sorted(names):
            path = here / name
            if not path.is_file():
                continue
            result = classify_file(path)
            if result:
                found.append(result)
            elif here == root:
                found.append(entry(path, "unknown", "not a game image; ignored", status="ignored"))
            else:
                strays += 1
    # What came with an install (its movies, manual, 64-bit launcher) says nothing more about it.
    tops = {root / path.relative_to(root).parts[0] for path in input_dirs}
    kept = []
    for row in found:
        path = Path(row["path"])
        inside = any(top in path.parents for top in tops)
        if inside and row["reason"] in (UNKNOWN_FILE, REMASTER) and path not in input_dirs:
            strays += 1
        else:
            kept.append(row)
    named = {Path(row["path"]) for row in kept}
    for top in sorted(path for path in root.iterdir() if path.is_dir()):
        if top in named or any(top in path.parents for path in named):
            continue
        platform = dlc_platform(top)  # the heroes' side file alone is still them, whatever the folder lacks or is called
        if platform:
            kept.append(dict(entry(top, "dlc", "BF2 DLC folder holding Kit Fisto and Asajj Ventress", "bf2"), platform=platform))
        elif any(files for _, _, files in os.walk(top)):
            kept.append(refused(top, "a folder holding nothing this build recognizes (for example Aspyr's Classic Collection)"))
    return kept, strays


def default_roots(root):
    """iso/, or the main checkout's in a worktree without one; and the development PC install folder."""
    root = Path(root)
    roots = [root / "iso"]
    if not roots[0].is_dir():
        try:
            result = subprocess.run(["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-common-dir"],
                                    capture_output=True, text=True)
            if result.returncode == 0 and isinstance(result.stdout, str) and result.stdout.strip():
                roots = [Path(result.stdout.strip()).parent / "iso"]
        except OSError:
            pass
    return roots + [root / "PC_swbf1_exe", root / "PC_swbf2_exe"]


def extracted_trees(extracted):
    """Earlier extractions of the PS2 discs: `usable` as gameplay data, `verified` as PS2 by its structure."""
    trees = {}
    for game in GAMES:
        tree = Path(extracted) / game / "lvl" / "DATA" / "_LVL_PS2"
        verified = content.is_ps2_bf2_tree(tree) if game == "bf2" else content.is_ps2_lvl(content.ci_path(tree, TREE_FILE[game]))
        trees[game] = dict(path=str(tree), usable=content.ci_path(tree, "CORE.LVL").is_file(), verified=verified)
    return trees


def xbox_tree(extracted, game):
    """Where `bake extract-xiso` puts a chosen Xbox disc's LVLs."""
    return str(Path(extracted) / "xbox" / game / "Data" / "_LVL_XBOX")


def add_message(game, xbox, pc_primary):
    add = f"a PS2 {SHORT[game]} disc image"
    if xbox:
        add += f" or an Xbox {SHORT[game]} disc image"
    if pc_primary and game == "bf1":
        add += " or a PC BF1 install"
    return add


def console_needed_message(xbox, where):
    other = " or an Xbox BF2 disc" if xbox else " (Xbox BF2 input is not supported yet)"
    return f"PC input requires a console Battlefront II disc: add a PS2 BF2 disc{other} to {where}."


def make_plan(found, trees=None, strays=0, roots=(), explicit_errors=(), xbox_supported=None, dlc_supported=None,
              pc_primary=None, extracted=None, require_all=True):
    """Choose from what the scan found. `require_all` False skips the one-source-per-game refusals (a partial rebake)."""
    xbox_on = XBOX_SUPPORTED if xbox_supported is None else xbox_supported
    dlc_on = DLC_SUPPORTED if dlc_supported is None else dlc_supported
    pc_on = PC_PRIMARY_SUPPORTED if pc_primary is None else pc_primary
    trees = trees or {}
    found = [dict(row) for row in found]
    where = ", ".join(str(root) for root in roots[:1]) or "iso/"
    refusals = list(explicit_errors)
    games = {}

    def of_kind(kind, game):
        return [row for row in found if row["kind"] == kind and row["game"] == game and row["status"] == "found"]

    any_pc = any(row["kind"] == "pc-install" for row in found)
    for game in GAMES:
        ps2, xbox, pc = of_kind("ps2-disc", game), of_kind("xbox-disc", game), of_kind("pc-install", game)
        tree = trees.get(game) or {}
        short = SHORT[game]
        gameplay, visual = None, []
        if ps2 or tree.get("usable"):
            gameplay = dict(kind="ps2-disc" if ps2 else "ps2-extracted", path=ps2[0]["path"] if ps2 else tree["path"],
                            extracted=tree["path"] if tree.get("usable") else None,
                            verified=bool(ps2) or bool(tree.get("verified")))
        elif xbox and xbox_on:
            gameplay = dict(kind="xbox-disc", path=xbox[0]["path"], verified=True)
            if extracted:
                gameplay["tree"] = xbox_tree(extracted, game)
        elif pc and pc_on and game == "bf1":
            gameplay = dict(kind="pc-install", path=pc[0]["path"], verified=True)
        elif xbox:
            refusals.append(f"{short}: found the Xbox disc {xbox[0]['path']}, but Xbox input is not supported yet. "
                            f"Add {add_message(game, xbox_on, pc_on)} to {where}.")
        elif pc and game == "bf1":
            refusals.append(f"{short}: found the PC install {pc[0]['path']}, but PC BF1 assets currently also need the "
                            "BF1 PS2 gameplay reference: the PC mission, class and collision data cannot replace it. "
                            f"Add {add_message(game, xbox_on, pc_on)} to {where}.")
        elif not (pc and game == "bf2") and require_all:
            refusals.append(f"{short}: no source found. Add {add_message(game, xbox_on, pc_on)} to {where}.")

        if pc and game == "bf1" and gameplay and gameplay["kind"] == "ps2-extracted" and not gameplay["verified"]:
            refusals.append(f"{short}: PC BF1 assets currently also need the BF1 PS2 gameplay reference, and "
                            f"{gameplay['path']} does not hold it. Add {add_message(game, xbox_on, pc_on)} to {where}.")
        for row in pc:
            if gameplay and gameplay["kind"] == "pc-install" and row["path"] == gameplay["path"]:
                row.update(status="used", reason=row["reason"] + "; supplies assets with fingerprinted PS2 gameplay corrections")
            elif not any(item["kind"] == "pc-install" for item in visual):
                row.update(status="used", reason=row["reason"] + "; texture donor")
                visual.append(dict(kind="pc-install", path=row["path"]))
            else:
                row.update(status="unused", reason=row["reason"] + f"; another PC copy is in use ({pc[0]['path']})")
        for row in xbox:
            if not xbox_on:
                row.update(status="not supported yet", reason=row["reason"] + "; recognized, not read")
            elif gameplay and gameplay["kind"] == "xbox-disc" and row["path"] == gameplay["path"]:
                row.update(status="used", reason=row["reason"] + "; supplies gameplay (no PS2 copy)")
            elif not any(item["kind"] == "xbox-disc" for item in visual) and not (gameplay and gameplay["kind"] == "xbox-disc"):
                row.update(status="used", reason=row["reason"] + "; texture donor")
                donor = dict(kind="xbox-disc", path=row["path"])
                if extracted:
                    donor["tree"] = xbox_tree(extracted, game)
                visual.append(donor)
            else:
                row.update(status="unused", reason=row["reason"] + f"; another Xbox copy is in use ({xbox[0]['path']})")
        for index, row in enumerate(ps2):
            if index == 0:
                row.update(status="used", reason=row["reason"] + "; gameplay reference and base textures")
            else:
                row.update(status="unused", reason=row["reason"] + f"; another PS2 copy is in use ({ps2[0]['path']})")
        games[game] = dict(title=TITLE[game], gameplay=gameplay, visual=visual)

    if any_pc:
        console = games["bf2"]["gameplay"]
        if not console or console["kind"] == "pc-install" or not console["verified"]:
            refusals.append(console_needed_message(xbox_on, where))

    dlc = dict(present=False, path=None, donor=None, used=False)
    # PS2 geometry first, as everywhere; a PC copy beside it can still lend the heroes its larger textures.
    copies = sorted(of_kind("dlc", "bf2"), key=lambda row: row.get("platform") != "ps2")
    for index, row in enumerate(copies):
        if not index:
            dlc.update(present=True, path=row["path"], used=dlc_on)
            if dlc_on:
                row.update(status="used", reason=row["reason"] + "; only those two heroes are taken")
            else:
                row.update(status="not supported yet", reason=row["reason"] + "; recognized, not read")
        elif dlc_on and not dlc["donor"] and copies[0].get("platform") == "ps2" and row.get("platform") == "pc":
            dlc["donor"] = row["path"]
            row.update(status="used", reason=row["reason"] + "; the two heroes' larger textures")
        else:
            row.update(status="unused", reason=row["reason"] + f"; another copy is in use ({dlc['path']})")

    if refusals:
        for row in found:
            if row["status"] == "used":
                row["status"] = "would be used"
        cannot = [row for row in found if row["status"] == "refused"]
        if cannot:
            refusals.append("Also found, and not usable: " + "; ".join(f"{row['path']} ({row['reason']})" for row in cannot))
    return dict(schema=1, roots=[str(root) for root in roots], games=games, dlc=dlc, found=found, strays=strays,
                refusals=refusals, xbox_supported=xbox_on, dlc_supported=dlc_on,
                rule="PS2 is the gameplay reference whenever present; other copies change only the look, and the bake "
                     "keeps the largest same-art copy of each texture.")


def explicit_inputs(discs, pc_roots):
    """Inputs named on a command line are checked the same way, and a wrong one is an error, not a skip."""
    found, errors = [], []
    for game, path in discs.items():
        if not path:
            continue
        path = os.path.abspath(path)
        option = f"--iso{game[-1]}"
        if not os.path.isfile(path):
            errors.append(f"{option} {path}: no such file; add a PS2 {SHORT[game]} disc at that path, or drop the option.")
            continue
        row = classify_file(path)
        if row and row["kind"] in ("ps2-disc", "xbox-disc") and row["game"] == game:
            found.append(row)
        else:
            what = row["reason"] if row else "not a disc image"
            errors.append(f"{option} {path} is not a {SHORT[game]} disc: {what}.")
    for game, path in pc_roots.items():
        path = os.path.abspath(path)
        here = Path(path).resolve()
        if any(remaster_beside(folder, folder) for folder in [here, *list(here.parents)[:LEVELS_ABOVE]]):
            errors.append(f"PC input {path} is {REMASTER}.")
        else:
            found.append(entry(path, "pc-install", f"PC {TITLE[game]} install (named on the command line)", game))
    return found, errors


def scan(roots, discs=None, pc_roots=None, extracted=None, named_root=None, **options):
    """Scan the roots and choose. Named inputs come first, so they win over anything else found. Paths come back
    absolute: the bake runs from another directory. `named_root` is a folder the user asked for, so it must exist."""
    named, errors = explicit_inputs(discs or {}, pc_roots or {})
    if named_root and not os.path.isdir(named_root):
        errors.append(f"--iso-dir {named_root}: no such folder.")
    roots = [os.path.abspath(root) for root in roots]
    found, strays, seen = list(named), 0, {os.path.realpath(row["path"]) for row in named}
    for root in roots:
        if not Path(root).is_dir():
            continue
        rows, more = scan_root(root)
        strays += more
        for row in rows:
            real = os.path.realpath(row["path"])
            if real not in seen:
                seen.add(real)
                found.append(row)
    trees = extracted_trees(extracted) if extracted else None
    return make_plan(found, trees, strays, roots, errors, extracted=extracted, **options)


def withheld_maps(game, gameplay):
    platform = {"xbox-disc": "xbox", "pc-install": "pc"}.get((gameplay or {}).get("kind"))
    return WITHHELD_MAPS.get((platform, game), ())


def bake_env(plan):
    """What the bake is told. Empty for anything not chosen, so the bake's default is the PS2-only one."""
    env = {}
    for game in GAMES:
        number = game[-1]
        chosen = plan["games"][game]
        gameplay = chosen["gameplay"] or {}
        if gameplay.get("kind") == "xbox-disc" and gameplay.get("tree"):
            env[f"OPENBF_BF{number}_TREE"] = gameplay["tree"]
        elif gameplay.get("kind") == "pc-install":
            env[f"OPENBF_BF{number}_TREE"] = gameplay["path"]
        if withheld_maps(game, gameplay):
            env[f"OPENBF_BF{number}_WITHHELD"] = " ".join(withheld_maps(game, gameplay))
        for donor in chosen["visual"]:
            if donor["kind"] == "pc-install":
                env[f"PC{number}"] = donor["path"]
            elif donor.get("tree"):
                env[f"XB{number}"] = donor["tree"]
        if game == "bf2" and gameplay.get("kind") == "ps2-disc":
            env["BF2_CONSOLE_ISO"] = gameplay["path"]
    if plan["dlc"]["used"]:
        env["OPENBF_DLC_DIR"] = plan["dlc"]["path"]
        if plan["dlc"]["donor"]:
            env["OPENBF_DLC_DONOR"] = plan["dlc"]["donor"]
    return {name: value.replace("\\", "/") for name, value in env.items()}


def discs_to_extract(plan):
    """(game, disc, tree to extract it into) for each Xbox disc the plan reads."""
    picks = []
    for game in GAMES:
        chosen = plan["games"][game]
        for item in [chosen["gameplay"] or {}] + chosen["visual"]:
            if item.get("kind") == "xbox-disc":
                picks.append((game, item["path"], item.get("tree")))
    return picks


def summary_lines(plan):
    lines = ["Inputs (recognized by content):"]
    label = {"ps2-disc": "PS2 disc", "ps2-extracted": "PS2 data extracted earlier", "xbox-disc": "Xbox disc", "pc-install": "PC install"}
    for game in GAMES:
        chosen = plan["games"][game]
        gameplay = chosen["gameplay"]
        if gameplay:
            lines.append(f"  {SHORT[game]} gameplay: {label[gameplay['kind']]} {gameplay['path']}")
        else:
            lines.append(f"  {SHORT[game]} gameplay: NONE")
        donors = ", ".join(f"{label[item['kind']]} {item['path']}" for item in chosen["visual"]) or "none"
        lines.append(f"  {SHORT[game]} texture donors: {donors}")
        if withheld_maps(game, gameplay):
            lines.append(f"  {SHORT[game]} maps left out, because this source has no model for some of their solid buildings "
                         f"(add a PS2 {SHORT[game]} disc to play them): {', '.join(withheld_maps(game, gameplay))}")
    dlc = plan["dlc"]
    if dlc["present"]:
        lines.append(f"  DLC: {dlc['path']} ({'Kit Fisto and Asajj Ventress used, off until the DLC Heroes row in Hero Options is turned on' if dlc['used'] else 'recognized; not supported yet'})")
    else:
        lines.append("  DLC: absent (optional)")
    lines.append("Everything found:")
    for row in plan["found"]:
        lines.append(f"  {row['status']}: {row['path']} - {row['reason']}")
    if not plan["found"]:
        lines.append("  nothing")
    if plan["strays"]:
        lines.append(f"  ignored: {plan['strays']} other small files")
    lines.append(plan["rule"])
    return [line.encode("utf-8", "backslashreplace").decode("utf-8") for line in lines]  # a file name need not be text


def write_summary(plan, out):
    """The summary beside the game file: <out>.inputs.txt to read, <out>.inputs.json to process."""
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    text = "\n".join(summary_lines(plan) + [f"REFUSED: {line}" for line in plan["refusals"]]) + "\n"
    Path(str(out) + ".inputs.txt").write_text(text, encoding="utf-8", errors="backslashreplace")
    with open(str(out) + ".inputs.json", "w", encoding="utf-8") as target:
        json.dump(plan, target, indent=2)
        target.write("\n")


def tolerant_stdout():
    """A console that cannot show a file's name must not stop the build (a Windows code page, say)."""
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(errors="backslashreplace")


def main():
    tolerant_stdout()
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    parser.add_argument("--iso-dir", default=None, help="the folder of inputs (default: iso/)")
    parser.add_argument("--shell", action="store_true", help="print the choice as SCAN_* shell variables for rebake.sh")
    args = parser.parse_args()
    roots = [args.iso_dir] if args.iso_dir else default_roots(args.root)
    extracted = os.path.join(args.root, "extracted")
    plan = scan(roots, extracted=extracted, named_root=args.iso_dir, require_all=not args.shell)
    if args.shell:
        # rebake.sh keeps its own refusals: a partial rebake needs only what its groups read.
        print("\n".join(summary_lines(plan)), file=sys.stderr)
        for name, value in bake_env(plan).items():
            if name in ("XB1", "XB2") and not os.path.isdir(value):
                continue
            # As bytes: a path need not be text, and the shell reading this needs its real name.
            sys.stdout.buffer.write(os.fsencode(f"SCAN_{name}={shlex.quote(value)}\n"))
        return 0
    print("\n".join(summary_lines(plan)))
    if plan["refusals"]:
        print("\n".join(plan["refusals"]), file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
