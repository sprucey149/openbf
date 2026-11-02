#!/usr/bin/env python3
"""pack.py -- what you own in iso/, a self-contained game out (card PACK).

    python3 scripts/pack.py [--iso-dir iso] [--out dist/openbf] [--scan-only]

Put your discs and install folders in iso/, under any names: scripts/input_scan.py knows them by their content and
chooses what each game is built from. Each game needs one source; any PC input also needs a console BF2 disc. It says
what to add when something is missing, and names what it refuses. --scan-only stops after that.

Steps, each skipped when its output is already there (so a second run only
recompiles and re-packs):
  0. scan the inputs and choose                 -> <output>.inputs.txt, <output>.inputs.json
  1. build the `bake` tool and the `game` binary (cargo, release)
  2. extract each disc's DATA/_LVL_PS2 tree     -> extracted/bf{1,2}/lvl/  (bake extract-iso)
  3. extract the Lua scripts of the LVLs        -> extracted/bf{1,2}/lua/  (extract_lua.py)
  4. build the missions from the disc's own bytecode -> engine/missions/   (gen_missions.py --ps2-only)
  5. bake every asset                           -> engine/assets_baked/    (rebake.sh)
  6. copy the game, strip the copy, append assets_baked/ + missions/  -> --out
     Below about 3.9 GiB that is one file on every OS. From there on, also on every OS, --out is a small executable
     and the data is openbf.pak in the same folder (Windows cannot run an executable of 4 GiB or more). The game looks
     inside itself first, then for openbf.pak beside it; the two must stay together.

A readable source summary and <output>.sources.json list inputs read this run,
unused optional inputs and stored map-local texture provenance. Cached data is
labelled separately; unsupported inputs are never claimed as consumed.

The result plays from anywhere. It holds the games' own content, so it is for
your machine only: never upload or share it. Needs
cargo, python3, bash and ffmpeg (the bake encodes speech to OGG); `strip` is
used when present. Tested on Linux and Windows (card WIN: Git for Windows' bash
runs the bake; the output is dist/openbf.exe). FAT32 cannot hold a file of 4 GiB
or more: --out needs a folder on another filesystem for a build that large.
"""

import argparse
import glob
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import input_scan
from console_input import pc_game

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ENGINE = os.path.join(ROOT, "engine")
EXTRACTED = os.path.join(ROOT, "extracted")
BAKED = os.path.join(ENGINE, "assets_baked")
MISSIONS = os.path.join(ENGINE, "missions")
TARGET = os.environ.get("CARGO_TARGET_DIR") or os.path.join(ENGINE, "target")
WINDOWS = os.name == "nt"
EXE = ".exe" if WINDOWS else ""

# The LVLs whose scripts the bake and the missions read, per disc (subdir <- LVL).
LUA = {
    "bf1": {"mission": "MISSION.LVL", "common": "COMMON/ENG/COMMON.LVL", "shell": "SHELL/ENG/SHELL.LVL"},
    "bf2": {"mission": "MISSION.LVL", "common": "COMMON.LVL", "shell": "SHELL.LVL",
            "ingame": "INGAME.LVL", "inshell": "INSHELL.LVL"},
}


PAK_NAME = "openbf.pak"
FAT32_LIMIT = 2**32
PACK_MAGIC = b"OPENBFPK"


def carries_pack(path):
    """Does the executable at `path` hold its own data (the archive footer ends the file)?"""
    try:
        with open(path, "rb") as file:
            file.seek(-32, os.SEEK_END)
            return file.read(8) == PACK_MAGIC
    except OSError:
        return False


def install_pack(new, new_pak, out, pak):
    """Swap the finished files in: the archive first, so an old game keeps running from what it knows.

    One-file builds drop only an archive the previous --out relied on; another build's stays.
    Returns True when the build is an executable plus the archive.
    """
    # A run killed mid-write leaves new_pak behind and a one-file build never touches it: ask the executable.
    split = os.path.exists(new_pak) and not carries_pack(new)
    relied_on_pak = os.path.exists(pak) and os.path.exists(out) and not carries_pack(out)
    try:
        if split:
            if os.path.exists(pak) and not relied_on_pak:
                print(f"   replacing {pak}: another build in this folder may have used it")
            os.replace(new_pak, pak)
        os.replace(new, out)
        if not split and relied_on_pak:
            os.remove(pak)
    except PermissionError:
        sys.exit(f"pack: {out} is in use (Windows cannot replace a running game): close the game and run pack.py again")
    return split


def pack_summary(out, pak, split):
    """The closing lines about the files just written."""
    gib = lambda path: f"{os.path.getsize(path) / 2**30:.2f} GiB"
    if not split:
        return [f"done: {out} ({gib(out)}). Run it from anywhere; nothing else is needed beside it."]
    lines = [f"done: {out} ({os.path.getsize(out) / 2**20:.0f} MiB) and {pak} ({gib(pak)}). "
             "Run it from anywhere; keep the two files in the same folder.",
             "FAT32 cannot hold a file of 4 GiB or more: keep this build on another filesystem."]
    if os.path.getsize(pak) >= FAT32_LIMIT:
        lines[1] = f"{PAK_NAME} is 4 GiB or more and FAT32 cannot hold such a file: keep this build on another filesystem."
    return lines


def step(msg):
    print(f"\n== {msg}", flush=True)


def run(cmd, cwd=ROOT, env=None):
    print("   $ " + " ".join(cmd), flush=True)
    kw = {"cwd": cwd}
    if env is not None:
        kw["env"] = env
    r = subprocess.run(cmd, **kw)
    if r.returncode:
        sys.exit(f"pack: failed ({r.returncode}): {' '.join(cmd)}")


def ci_path(base, rel):
    direct = os.path.join(base, rel)
    if os.path.exists(direct):
        return direct
    cur = base
    for part in rel.replace("\\", "/").split("/"):
        if not part or part == ".":
            continue
        nxt = os.path.join(cur, part)
        if os.path.exists(nxt):
            cur = nxt
            continue
        found = None
        if os.path.isdir(cur):
            want = part.lower()
            for entry in os.listdir(cur):
                if entry.lower() == want:
                    found = os.path.join(cur, entry)
                    break
        cur = found if found is not None else nxt
    return cur


def classify_lvl_pc(path):
    return pc_game(path)


def find_pc_lvl_dirs(search_root):
    found = {}
    root = os.path.abspath(search_root)
    for cand in (root, ci_path(root, "GameData/Data/_LVL_PC")):
        kind = classify_lvl_pc(cand)
        if kind and kind not in found:
            found[kind] = cand
    if len(found) == 2 or not os.path.isdir(root):
        return found
    for dirpath, dirnames, _ in os.walk(root):
        dirnames.sort()
        for d in dirnames:
            if d.lower() == "_lvl_pc":
                cand = os.path.join(dirpath, d)
                kind = classify_lvl_pc(cand)
                if kind and kind not in found:
                    found[kind] = cand
                    if len(found) == 2:
                        return found
    return found


def resolve_pc_roots(pc_dir, pc1, pc2):
    roots = {}
    if pc_dir:
        found = find_pc_lvl_dirs(pc_dir)
        if not found:
            sys.exit(f"pack: no BF1/BF2 _LVL_PC directories found under {pc_dir}")
        roots.update(found)
    for game, arg in (("bf1", pc1), ("bf2", pc2)):
        if not arg:
            continue
        found = find_pc_lvl_dirs(arg)
        if game in found:
            roots[game] = found[game]
        elif classify_lvl_pc(arg) == game:
            roots[game] = os.path.abspath(arg)
        else:
            sys.exit(f"pack: {arg} is not a {game.upper()} _LVL_PC directory")
    return roots


# Set from the scan for a game whose gameplay data is not a PS2 tree.
GAMEPLAY_TREES = {}


def lvl_root(game):
    """The game's data directory: the dev layout's _LVL_PS2, else where extract-iso puts it."""
    return GAMEPLAY_TREES.get(game) or os.path.join(EXTRACTED, game, "lvl", "DATA", "_LVL_PS2")


def need(tool, why):
    if shutil.which(tool) is None:
        sys.exit(f"pack: `{tool}` is not on PATH ({why})")


# What a bake / the missions are made by: when any of it changes, the output is stale and is made again.
BAKE_INPUTS = ("engine/crates/bake", "engine/crates/mungefs", "engine/crates/manifest", "scripts/rebake.sh", "scripts/console_input.py", "scripts/input_scan.py",
               "engine/crates/bake/src/effect_names.txt", "engine/Cargo.toml", "engine/Cargo.lock")
MISSION_INPUTS = ("scripts/gen_missions.py", "scripts/extract_lua.py", "scripts/pack.py", "scripts/console_input.py", "scripts/input_scan.py")
# Kept in extracted/ (local, ignored by git): engine/missions/ is tracked in the dev tree.
BAKE_STAMP = os.path.join(EXTRACTED, "bake_inputs.sha256")
MISSION_STAMP = os.path.join(EXTRACTED, "mission_inputs.sha256")


def inputs_hash(paths, source_paths=None):
    """One sha256 over the source files under `paths` (line endings folded, so a CRLF checkout hashes the same)."""
    h = hashlib.sha256()
    if source_paths:
        h.update(json.dumps(source_paths, sort_keys=True).encode())
    for p in paths:
        full = os.path.join(ROOT, p)
        files = [full] if os.path.isfile(full) else sorted(
            os.path.join(d, f) for d, _, fs in os.walk(full) for f in fs
            if f.endswith((".rs", ".toml")) and os.sep + "tests" not in d[len(full):])
        for f in files:
            h.update(os.path.relpath(f, ROOT).replace(os.sep, "/").encode())
            with open(f, "rb") as source:
                h.update(source.read().replace(b"\r\n", b"\n"))
    return h.hexdigest()


def stale(stamp, paths, source_paths=None):
    """Why the output at `stamp` must be made again, or None when it is current."""
    if not os.path.isfile(stamp):
        return "made before pack.py recorded what made it"
    return None if Path(stamp).read_text(encoding="utf-8").strip() == inputs_hash(paths, source_paths) else "its source changed since it was made"


def find_bash():
    """bash for rebake.sh. On Windows, Git for Windows' own: WSL's System32\\bash.exe runs Linux, which cannot run
    the Windows bake.exe or read C:/ paths."""
    if not WINDOWS:
        need("bash", "rebake.sh")
        return shutil.which("bash")
    roots = []
    git = shutil.which("git")
    if git:  # ...\Git\cmd\git.exe or ...\Git\mingw64\bin\git.exe
        d = os.path.dirname(os.path.realpath(git))
        for _ in range(3):
            roots.append(d)
            d = os.path.dirname(d)
    for env in ("ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"):
        if os.environ.get(env):
            roots.append(os.path.join(os.environ[env], "Git"))
    if os.environ.get("LOCALAPPDATA"):
        roots.append(os.path.join(os.environ["LOCALAPPDATA"], "Programs", "Git"))
    for r in roots:
        b = os.path.join(r, "bin", "bash.exe")
        if os.path.isfile(b):
            return b
    sys.exit("pack: Git for Windows' bash.exe was not found (install Git for Windows, https://git-scm.com)")


def missing_music():
    """Music bodies a stream index names but the bake has not written (an assets_baked from before card PACK)."""
    out = []
    for idx in glob.glob(os.path.join(BAKED, "*/sounds/streams/streams.ron")) + \
            glob.glob(os.path.join(BAKED, "audio/global_*/global.ron")):
        text = open(idx, encoding="utf-8").read()
        d = os.path.dirname(idx)
        # source: "x.lvl", channels: N, block_size: N, data_offset: N, bytes: N, baked: false
        for m in re.finditer(r'source:\s*"([^"]+)",[^)]*?data_offset:\s*(\d+),\s*bytes:\s*\d+,\s*baked:\s*false', text):
            stem = m.group(1).rsplit(".", 1)[0].lower()
            body = os.path.join(d, "music", f"{stem}_{int(m.group(2)):x}.bin")
            if not os.path.isfile(body):
                out.append(body)
    return out


RON_STRING = r'"(?:\\.|[^"\\])*"'


def ron_array(text, key, required=False):
    """Read one generated RON array, preserving quoted brackets and escapes."""
    match = re.search(r'\b' + re.escape(key) + r'\s*:\s*\[', text)
    if not match:
        if required:
            raise ValueError(f"missing {key} array")
        return ""
    start, depth, quoted, escaped = match.end(), 1, False, False
    for index in range(start, len(text)):
        char = text[index]
        if quoted:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
        elif char == '"':
            quoted = True
        elif char == '[':
            depth += 1
        elif char == ']':
            depth -= 1
            if depth == 0:
                return text[start:index]
    raise ValueError(f"unterminated {key} array")


def map_texture_sources(baked):
    """Stored map-local provenance, not a claim about files read this run."""
    maps = []
    pair = r'\(\s*(' + RON_STRING + r')\s*,\s*(' + RON_STRING + r')\s*,?\s*\)'
    for manifest in sorted(Path(baked).glob('*/manifest.ron')):
        row = {"map": manifest.parent.name}
        try:
            text = manifest.read_text(encoding="utf-8")
            game = re.search(r'\bsource_game\s*:\s*(' + RON_STRING + ')', text)
            game = json.loads(game[1]) if game else None
            if game not in ("bf1", "bf2"):
                raise ValueError("unrecognized source_game")
            raw = ron_array(text, "textures", required=True)
            names = [json.loads(m) for m in re.findall(RON_STRING, raw)]
            if re.sub(RON_STRING, '', raw).strip(' \r\n\t,'):
                raise ValueError("unrecognized textures array")
            def pairs(key):
                raw = ron_array(text, key)
                values = [(json.loads(a), json.loads(b)) for a, b in re.findall(pair, raw)]
                if re.sub(pair, '', raw).strip(' \r\n\t,'):
                    raise ValueError(f"unrecognized {key} array")
                return values

            tiers = pairs("texture_tiers")
            files = {}
            for name, filename in pairs("texture_tier2"):
                path = Path(filename)
                if not filename or path.is_absolute() or '..' in path.parts:
                    raise ValueError("texture_tier2 file must be relative to textures/")
                if name in files and files[name] != filename:
                    raise ValueError(f"conflicting texture_tier2 files for {name!r}")
                files[name] = filename
            textures = manifest.parent / 'textures'
            tier_names = {name for name, _ in tiers}
            present = {name for name, filename in files.items() if (textures / filename).is_file()}
            row.update(source_game=game, base_source=f"{game} PS2",
                       base_textures=sum((textures / f'{n}.ktx2').is_file() for n in set(names)),
                       missing_base_textures=sorted(n for n in set(names) if not (textures / f'{n}.ktx2').is_file()),
                       pc_alternatives=sorted({n for n, source in tiers if source == 'pc' and n in present}),
                       xbox_alternatives=sorted({n for n, source in tiers if source == 'xbox' and n in present}),
                       missing_alternatives=sorted(tier_names.intersection(files) - present),
                       missing_alternative_metadata=sorted(tier_names - files.keys()),
                       unrecognized_alternative_metadata=sorted(files.keys() - tier_names),
                       unrecognized_tiers=sorted({source for _, source in tiers if source not in ('pc', 'xbox')}))
        except (OSError, ValueError) as error:
            row['error'] = str(error)
        maps.append(row)
    return maps


def source_report(root, baked, missions, images, extracted_used=(), images_used=(), full_bake=False, pc_roots=None, plan=None):
    """Only successful pack steps establish which inputs were consumed."""
    root = Path(root)
    inputs = []
    pc_roots = pc_roots or {}

    def note(path, label, used, reason):
        inputs.append(dict(path=str(Path(path).absolute()), label=label,
                           present=Path(path).exists(), status='used' if used else 'unused', reason=reason))

    selected = set()
    for game, image in images.items():
        if game in pc_roots:
            note(pc_roots[game], f'{game} PC visual input', full_bake,
                 'passed to texture-tier bake this run; console data supplies gameplay and shared UI' if full_bake else
                 'not read this run; cached bake and missions packed')
        if image:
            selected.add(Path(image).resolve())
            used = game in images_used
            note(image, f'{game} requested PS2 image', used,
                 'extracted successfully this run' if used else 'not read or verified this run; cached extraction used')
        tree = root / 'extracted' / game
        used = game in extracted_used
        note(tree, f'{game} extracted PS2 data', used,
             'read for the console Lua cache, mission generation or bake this run' if used else 'not read this run; cached bake and missions packed')
    iso = root / 'iso'
    if plan:
        chosen = {Path(path).resolve() for path in pc_roots.values()} | selected
        for row in plan['found']:
            if Path(row['path']).resolve() not in chosen:
                inputs.append(dict(path=row['path'], label=f"found: {row['kind']}", present=True,
                                   status=row['status'], reason=row['reason']))
    elif iso.is_dir():
        for path in sorted(p for p in iso.iterdir() if p.is_file()):
            if path.resolve() not in selected:
                inputs.append(dict(path=str(path.absolute()), label='other source file', present=True,
                                   status='not recognized', reason='no supported reader selected; file was not read'))
    pc = root / 'PC_swbf2_exe' / 'Star Wars Battlefront II Classic/GameData/data/_lvl_pc'
    recipe = (root / 'scripts/rebake.sh').read_text(encoding='utf-8')
    match = re.search(r'^BF2_MAPS="([^"]*)"', recipe, re.M)
    known_pc = {'side/rep.lvl', 'side/cis.lvl', 'tat/tat2.lvl'}
    if match:
        known_pc.update(token.lower() + '.lvl' for token in match[1].split())
    # With a scan the bake reads only what it chose, and what it chose is listed above.
    read_pc = sorted(str(pc / name) for name in known_pc if full_bake and not pc_roots and not plan and (pc / name).is_file())
    if not any(Path(path).resolve() == pc.resolve() for path in pc_roots.values()):
        note(pc, 'optional BF2 PC texture directory', bool(read_pc),
             'existing libraries passed to the supported PC texture reader' if read_pc else
             'no PC libraries read this run (missing optional input or cached bake)')
    xbox = root / 'extracted' / 'xbox_bf2'
    xbox_read = full_bake and not plan and any(p.name.lower() == 'common.lvl' for p in xbox.glob('*'))
    xbox_cached = any((Path(baked) / 'common_bf2' / 'textures').glob('icon_duke_*.ktx2'))
    note(xbox, 'optional Xbox BF2 extracted data', xbox_read,
         'common.lvl read for the original Xbox button icons only' if xbox_read else
         'not read this run; the bake packed holds Xbox button icons from a chosen disc or an earlier run' if xbox_cached else
         'not read this run (missing optional input or cached bake); the game draws its own Xbox icons')
    note(baked, 'baked game assets', True, 'packed into the game; stored provenance listed separately')
    note(missions, 'mission data', True, 'packed into the game')
    return dict(schema=1, inputs=inputs, pc_libraries_read=read_pc, maps=map_texture_sources(baked),
                input_scan=plan,
                limits='Map-local base and alternative textures only. Shared/side model bakes do not retain per-texture source metadata. Other formats are not claimed as read.')


def write_source_report(out, report):
    path = str(out) + '.sources.json'
    temporary = path + '.new'
    try:
        with open(temporary, 'w', encoding='utf-8') as target:
            json.dump(report, target, indent=2)
            target.write('\n')
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.remove(temporary)
    print('\nGame sources (this run):')
    for row in report['inputs']:
        print(f"  {row['status']}: {row['label']} — {row['path']!r}; {row['reason']}")
    print('Stored map-local texture provenance:')
    for row in report['maps']:
        if 'error' in row:
            print(f"  {row['map']}: not recognized — {row['error']}")
        else:
            print(f"  {row['map']}: {row['base_textures']} {row['base_source']} base, "
                  f"{len(row['pc_alternatives'])} PC and {len(row['xbox_alternatives'])} Xbox alternatives; "
                  f"{len(row['missing_base_textures'])} missing base / {len(row['missing_alternatives'])} missing alternatives")
            if row['missing_alternative_metadata']:
                print(f"    missing alternative filename metadata: {row['missing_alternative_metadata']!r}")
            if row['unrecognized_alternative_metadata']:
                print(f"    alternative files without source metadata: {row['unrecognized_alternative_metadata']!r}")
            if row['unrecognized_tiers']:
                print(f"    unrecognized provenance tags: {row['unrecognized_tiers']!r}")
    print(report['limits'])
    print(f'Source report: {path}')
    return path



def console_lua_root():
    """A content-keyed console-only cache cannot inherit old PC mission scripts."""
    digest = hashlib.sha256(inputs_hash(MISSION_INPUTS).encode())
    for game, libraries in LUA.items():
        for group, library in libraries.items():
            source = ci_path(lvl_root(game), library)
            digest.update((game + "/" + group).encode())
            with open(source, "rb") as data:
                for block in iter(lambda: data.read(1 << 20), b""):
                    digest.update(block)
    return os.path.join(EXTRACTED, "console_lua", digest.hexdigest())


def main():
    input_scan.tolerant_stdout()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--iso-dir", default=None, help="the folder of discs and install folders to scan (default: iso/)")
    ap.add_argument("--scan-only", action="store_true", help="print and write what the scan found and chose, then stop")
    ap.add_argument("--iso1", default=None, help="use this Star Wars: Battlefront disc image, whatever else the scan finds")
    ap.add_argument("--iso2", default=None, help="use this Star Wars: Battlefront II disc image, whatever else the scan finds")
    ap.add_argument("--pc-dir", default=None, help="directory containing installed PC Battlefront I and/or II")
    ap.add_argument("--pc1", default=None, help="Star Wars: Battlefront (PC) _LVL_PC or install directory")
    ap.add_argument("--pc2", default=None, help="Star Wars: Battlefront II (PC) _lvl_pc or install directory")
    ap.add_argument("--out", default=os.path.join(ROOT, "dist", "openbf" + EXE), help="the game file to write")
    ap.add_argument("--rebake", action="store_true", help="bake every asset again even when assets_baked/ exists")
    a = ap.parse_args()
    out = os.path.abspath(a.out)
    if out.startswith(os.path.join(ENGINE, "")) or os.path.isdir(out):
        sys.exit(f"pack: --out {out} must be a file outside engine/")
    named_pc = resolve_pc_roots(a.pc_dir, a.pc1 or os.environ.get("PC1"), a.pc2 or os.environ.get("PC2"))
    roots = [a.iso_dir] if a.iso_dir else input_scan.default_roots(ROOT)
    plan = input_scan.scan(roots, {"bf1": a.iso1, "bf2": a.iso2}, named_pc, EXTRACTED, a.iso_dir)
    print("\n".join(input_scan.summary_lines(plan)))
    input_scan.write_summary(plan, out)
    print(f"Input summary: {out}.inputs.txt")
    if plan["refusals"]:
        message = "pack: " + "\npack: ".join(plan["refusals"])
        sys.exit(message.encode("utf-8", "backslashreplace").decode("utf-8"))
    if a.scan_only:
        return
    pc_roots = {g: item["path"] for g, chosen in plan["games"].items()
                for item in chosen["visual"] if item["kind"] == "pc-install"}
    images = {g: chosen["gameplay"]["path"] if chosen["gameplay"]["kind"] == "ps2-disc" else None
              for g, chosen in plan["games"].items()}
    GAMEPLAY_TREES.clear()
    for g, chosen in plan["games"].items():
        if chosen["gameplay"].get("tree"):
            GAMEPLAY_TREES[g] = chosen["gameplay"]["tree"]
        elif chosen["gameplay"]["kind"] == "pc-install":
            GAMEPLAY_TREES[g] = chosen["gameplay"]["path"]
    bake_inputs = dict(pc_roots)
    for g, image, _ in input_scan.discs_to_extract(plan):
        bake_inputs["xbox_" + g] = image
    if plan["dlc"]["used"]:
        bake_inputs["dlc"] = plan["dlc"]["path"]
    images_used, extracted_used = set(), set()
    full_bake = False
    need("cargo", "the Rust toolchain, https://rustup.rs")

    step("1/6 build bake + game (release)")
    run(["cargo", "build", "--release", "-p", "bake", "-p", "game"], cwd=ENGINE)
    bake, game = os.path.join(TARGET, "release", "bake" + EXE), os.path.join(TARGET, "release", "game" + EXE)

    step("2/6 the discs")
    for g, image in images.items():
        if g in pc_roots:
            print(f"   {g}: using PC assets ({pc_roots[g]})")
            # Console extraction remains the source of gameplay and the shared UI.
        if g in GAMEPLAY_TREES:
            continue
        if os.path.isfile(ci_path(lvl_root(g), "CORE.LVL")):
            print(f"   {g}: already extracted ({lvl_root(g)})")
            continue
        run([bake, "extract-iso", image, os.path.join(EXTRACTED, g, "lvl")])
        images_used.add(g)
    for g, image, tree in input_scan.discs_to_extract(plan):
        # The Xbox twin of extract-iso: a file already there at its size is left alone, so a second run costs little.
        run([bake, input_scan.XBOX_EXTRACT, image, os.path.dirname(os.path.dirname(tree))])

    step("3/6 the Lua scripts")
    lua_root = console_lua_root()
    extracted_used.update(LUA)  # those source LVLs were read to key the console cache
    for g, lvls in LUA.items():
        for sub, lvl in lvls.items():
            d = os.path.join(lua_root, g, sub)
            if os.path.isfile(os.path.join(d, ".complete")):
                continue
            os.makedirs(os.path.dirname(d), exist_ok=True)
            with tempfile.TemporaryDirectory(prefix="console-lua-", dir=os.path.dirname(d)) as scratch:
                run([sys.executable, "-I", os.path.join(ROOT, "scripts", "extract_lua.py"),
                     ci_path(lvl_root(g), lvl), "--out", scratch])
                Path(scratch, ".complete").touch()
                os.replace(scratch, d)
            extracted_used.add(g)

    step("4/6 the missions")
    # A checkout that tracks the missions in git (the dev tree) keeps its own; only a generated set goes stale.
    tracked = subprocess.run(["git", "-C", ROOT, "ls-files", "--", "engine/missions"], capture_output=True,
                             text=True).stdout.strip()
    have = glob.glob(os.path.join(MISSIONS, "*.ron"))
    why = stale(MISSION_STAMP, MISSION_INPUTS, {"console_lua": lua_root}) if have and not tracked else None
    if have and (tracked or (not pc_roots and not a.rebake and why is None)):
        print(f"   already there ({MISSIONS})")
    else:
        if why:
            print(f"   making the missions again: {why}")
        run([sys.executable, "-I", os.path.join(ROOT, "scripts", "gen_missions.py"), "--ps2-only", "--lua-root", lua_root, "--out", MISSIONS])
        extracted_used.update(("bf1", "bf2"))
        os.makedirs(EXTRACTED, exist_ok=True)
        with open(MISSION_STAMP, "w") as fh:
            fh.write(inputs_hash(MISSION_INPUTS, {"console_lua": lua_root}) + "\n")

    step("5/6 the bake")
    bash = find_bash()
    bake_env = dict(os.environ)
    for name in ("PC1", "PC2", "XB1", "XB2", "BF2_CONSOLE_ISO", "OPENBF_BF1_TREE", "OPENBF_BF2_TREE", "OPENBF_DLC_DIR"):
        bake_env.pop(name, None)  # the scan's choice is the whole story
    bake_env.update(input_scan.bake_env(plan))
    bake_env["OPENBF_INPUTS_CHOSEN"] = "1"  # rebake.sh need not scan again
    bake_env["OPENBF_CONSOLE_LUA_ROOT"] = lua_root.replace("\\", "/")
    baked = os.path.isfile(os.path.join(BAKED, "classes", "bf1_all.ron"))
    why = stale(BAKE_STAMP, BAKE_INPUTS, bake_inputs) if baked else None
    if a.rebake or not baked or why:
        if why and not a.rebake:
            print(f"   baking again: the bake in {BAKED} was {why}")
        need("ffmpeg", "the bake encodes speech and effects to OGG")
        run([bash, os.path.join(ROOT, "scripts", "rebake.sh")], **({"env": bake_env} if bake_env else {}))
        full_bake = True
        extracted_used.update(("bf1", "bf2"))
        os.makedirs(EXTRACTED, exist_ok=True)
        with open(BAKE_STAMP, "w") as fh:
            fh.write(inputs_hash(BAKE_INPUTS, bake_inputs) + "\n")
    else:
        print(f"   already baked ({BAKED}); --rebake bakes again")
        n = len(missing_music())
        if n:
            print(f"   {n} music streams not copied out yet: rebaking the sound groups")
            need("ffmpeg", "the bake encodes speech and effects to OGG")
            run([bash, os.path.join(ROOT, "scripts", "rebake.sh"), "sounds", "global-sound"],
                **({"env": bake_env} if bake_env else {}))
            extracted_used.update(("bf1", "bf2"))
    left = missing_music()
    if left:
        sys.exit(f"pack: {len(left)} music streams still missing after the bake, e.g. {left[0]}")

    step("6/6 pack")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    old_report = out + ".sources.json"  # never left describing an earlier pack
    if os.path.exists(old_report):
        os.remove(old_report)
    tmp = os.path.join(os.path.dirname(out), "pack-tmp" + EXE)  # Windows runs only a name ending in .exe
    shutil.copyfile(game, tmp)
    os.chmod(tmp, 0o755)
    if WINDOWS:
        pass  # MSVC keeps the symbols in a .pdb beside the build, not in the .exe
    elif shutil.which("strip"):
        run(["strip", tmp])
    else:
        print("   (no `strip` on PATH: the game keeps its symbols, ~100 MB more)")
    # Written beside --out and swapped in: a game still running from --out keeps its old files.
    new = out + ".new"
    pak = os.path.join(os.path.dirname(out), PAK_NAME)
    new_pak = pak + ".new"
    try:
        run([tmp, "--write-pack", new, "--pak", new_pak, f"assets_baked={BAKED}", f"missions={MISSIONS}"])
        split = install_pack(new, new_pak, out, pak)
    finally:
        os.remove(tmp)
        for leftover in (new, new_pak):
            if os.path.exists(leftover):
                os.remove(leftover)
    print()
    print("\n".join(pack_summary(out, pak, split)))
    print("It contains the games' own content: keep it on your machine, never upload or share it.")
    report = source_report(ROOT, BAKED, MISSIONS, images,
                           extracted_used, images_used, full_bake, pc_roots=pc_roots, plan=plan)
    write_source_report(out, report)


if __name__ == "__main__":
    main()
