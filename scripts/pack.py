#!/usr/bin/env python3
"""pack.py -- what you own in iso/, a self-contained game out (card PACK).

    python3 scripts/pack.py [--iso-dir iso] [--out dist/openbf] [--scan-only]

Put your discs and install folders in iso/, under any names: scripts/input_scan.py knows them by their content and
chooses what each game is built from. Each game needs one source; both PC games can build without a console disc. It says
what to add when something is missing, and names what it refuses. --scan-only stops after that.

Steps, each skipped when its output is already there (so a second run only
recompiles and re-packs):
  0. scan the inputs and choose                 -> <output>.inputs.txt, <output>.inputs.json
  1. build the `bake` tool and the `game` binary (cargo, release)
  2. extract each disc's DATA/_LVL_PS2 tree and executable -> extracted/bf{1,2}/lvl/  (bake extract-iso)
  3. extract the Lua scripts of the LVLs        -> extracted/bf{1,2}/lua/  (extract_lua.py)
  4. build the missions from the disc's own bytecode -> engine/missions/   (gen_missions.py --ps2-only)
     Includes the six Space Assault missions; OPENBF_BAKE_SPACE=0 leaves them out.
  5. bake the asset groups that are not current -> engine/assets_baked/    (rebake.sh <group>)
     Each scripts/rebake.sh group has a key: its version (engine/crates/bake/src/versions.rs), the content of what it
     reads, its settings and, for audio, ffmpeg and the restoration (scripts/bake_cache.py). Only a group whose key
     changed bakes again; --explain says why. A run stopped part-way goes on from the group it was in.
  6. copy the game, strip the copy, append assets_baked/ + missions/  -> --out
     Below about 3.9 GiB that is one file on every OS. From there on, also on every OS, --out is a small executable
     and the data is openbf.pak in the same folder (Windows cannot run an executable of 4 GiB or more). The game looks
     inside itself first, then for openbf.pak beside it; the two must stay together.

--bake-only runs steps 2-5 and stops (only the `bake` tool is built): a development checkout's own engine/missions
and engine/assets_baked, the ones its sweeps and tests read, stay current with the same cache as a pack, instead of
drifting behind the tree (a hand-run rebake.sh records no keys, so nothing said when it went stale).

Optional prepared packs: unzip under visual_packs/, then run the normal command.
Each pack has pack.json; named mod folders may group packs recursively, for example
visual_packs/upscaled_textures/endor/pack.json. Discovery stops at each pack root,
so its payload/preview subfolders are not separate packs. Remove a pack or mod
folder and run again to revert. --visual-packs <dir> selects a lookup folder or a
pack itself. See docs/visual-packs.md for formats and compatibility checks.

The bake restores retail audio locally by default. --retail-audio keeps the
retail audio encode and original music streams.

A readable source summary and <output>.sources.json list inputs read this run,
unused optional inputs and stored map-local texture provenance. Cached data is
labelled separately; unsupported inputs are never claimed as consumed.

The result plays from anywhere. It holds the games' own content, so it is for
your machine only: never upload or share it. Needs
cargo, Python 3.10 or newer, bash and an ffmpeg with the libvorbis encoder (the
bake encodes speech to OGG; both are checked before anything is built); `strip`
is used when present. Tested on Linux and Windows (card WIN: Git for Windows'
bash runs the bake; the output is dist/openbf.exe). macOS: the scripts run under
its older bash and tools, and on a Mac pack.py warns about the setups that make
the bake much slower (Rosetta, an iCloud-synced or external folder, little free
space, no `caffeinate`). A full Mac build has not been timed yet. FAT32
cannot hold a file of 4 GiB or more: --out needs a folder on another filesystem
for a build that large.

Every step line shows the time since the start, `== [mm:ss] ...`, and the run
ends with each step's total, also when it fails or is stopped.
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
import time
from pathlib import Path

MIN_PYTHON = (3, 10)


def python_too_old(version, platform=sys.platform):
    """Why this Python cannot run the build, or None. Checked before the build's own modules are imported, because
    those use Python 3.10 syntax and would fail with a TypeError that names neither the version nor the fix."""
    if tuple(version[:2]) >= MIN_PYTHON:
        return None
    have = ".".join(str(part) for part in version[:3])
    fix = ("The python3 that comes with macOS is too old: `brew install python`, open a new Terminal window and check "
           "that `python3 --version` says 3.10 or newer." if platform == "darwin"
           else "Install Python 3.10 or newer and run pack.py with it.")
    return f"pack: this is Python {have}; the build needs Python {MIN_PYTHON[0]}.{MIN_PYTHON[1]} or newer. {fix}"


if python_too_old(sys.version_info):
    sys.exit(python_too_old(sys.version_info))

import bake_cache
import input_scan
import pack_cache
import visual_packs
from console_input import pc_game

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ENGINE = os.path.join(ROOT, "engine")
EXTRACTED = os.path.join(ROOT, "extracted")
BAKED = os.path.join(ENGINE, "assets_baked")
MISSIONS = os.path.join(ENGINE, "missions")
# cargo runs in engine/ and reads a relative CARGO_TARGET_DIR from there; this script can run from anywhere.
TARGET = os.path.join(ENGINE, os.environ.get("CARGO_TARGET_DIR") or "target")
WINDOWS = os.name == "nt"
EXE = ".exe" if WINDOWS else ""

# The LVLs whose scripts the bake and the missions read, per disc (subdir <- LVL).
LUA = {
    "bf1": {"mission": "MISSION.LVL", "common": "COMMON/ENG/COMMON.LVL", "shell": "SHELL/ENG/SHELL.LVL"},
    "bf2": {"mission": "MISSION.LVL", "common": "COMMON.LVL", "shell": "SHELL.LVL",
            "ingame": "INGAME.LVL", "inshell": "INSHELL.LVL"},
}


# Above each game's tree: bake::foliage reads its placement and grass tables out of it.
GAME_ELF = {"bf1": "SWBFRONT.ELF", "bf2": "SWBFRNT2.ELF"}

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


def clock(seconds):
    """`mm:ss`, the minutes running on past 59 (a slow build reads `[95:12]`)."""
    seconds = int(seconds)
    return f"{seconds // 60:02d}:{seconds % 60:02d}"


class StepClock:
    """How long each step took: the receipt a slow-build report needs."""

    def __init__(self, now=time.monotonic):
        self.now = now
        self.reset()

    def reset(self):
        self.start, self.current, self.rows = None, None, []

    def begin(self, name):
        """Close the step that is running and start `name`; returns the seconds since the first step began."""
        t = self.now()
        if self.start is None:
            self.start = t
        self.close(t)
        self.current = (name, t)
        return t - self.start

    def close(self, t=None):
        if self.current is not None:
            name, began = self.current
            self.rows.append((name, (self.now() if t is None else t) - began))
            self.current = None

    def totals(self):
        """One closing line, or None before any step began."""
        if self.start is None:
            return None
        end = self.now()
        self.close(end)
        steps = ", ".join(f"{name.split(' ', 1)[0]} {clock(seconds)}" for name, seconds in self.rows)
        return f"step times: {steps}; total {clock(end - self.start)}"


CLOCK = StepClock()


def step(msg):
    print(f"\n== [{clock(CLOCK.begin(msg))}] {msg}", flush=True)


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


def disc_executable(game):
    """The executable bake::foliage::elf_for finds for this game's tree, or None."""
    tree = Path(lvl_root(game))
    return next((str(folder / GAME_ELF[game]) for folder in [tree, *tree.parents] if (folder / GAME_ELF[game]).is_file()), None)


def need(tool, why):
    if shutil.which(tool) is None:
        sys.exit(f"pack: `{tool}` is not on PATH ({why})")


DARWIN_FFMPEG = ("On macOS: `brew install ffmpeg-full` (plain `brew install ffmpeg` has no libvorbis), then put it first "
                 "on PATH: `echo 'export PATH=\"$(brew --prefix ffmpeg-full)/bin:$PATH\"' >> ~/.zprofile` and open a new "
                 "Terminal window.")


def ffmpeg_problem(which=None, run=None, platform=None):
    """Why the ffmpeg on PATH cannot make the bake's OGGs, or None. The bake asks for `-c:a libvorbis`; ffmpeg's own
    `vorbis` encoder makes different bytes (and a different global-sound cache identity), so it is never a fallback."""
    which, run, platform = which or shutil.which, run or subprocess.run, platform or sys.platform
    fix = DARWIN_FFMPEG if platform == "darwin" else "Install an ffmpeg whose `ffmpeg -encoders` lists libvorbis."
    ffmpeg = which("ffmpeg")
    if ffmpeg is None:
        return f"pack: `ffmpeg` is not on PATH (the bake encodes speech and effects to OGG). {fix}"
    try:
        listed = run([ffmpeg, "-hide_banner", "-encoders"], capture_output=True, text=True, errors="replace",
                     stdin=subprocess.DEVNULL, timeout=60)
    except (OSError, subprocess.SubprocessError) as error:
        return f"pack: `{ffmpeg} -encoders` did not run ({error}). {fix}"
    if not re.search(r"^\s*A\S*\s+libvorbis\b", listed.stdout or "", re.MULTILINE):
        return (f"pack: {ffmpeg} has no libvorbis encoder, and the bake encodes speech and effects to OGG with it. "
                f"{fix} `ffmpeg -hide_banner -encoders | grep libvorbis` must print a line.")
    return None


def need_ffmpeg():
    """Checked once, before anything is built: a missing encoder used to surface only at the voice step, deep in the bake."""
    problem = ffmpeg_problem()
    if problem:
        sys.exit(problem)


MAC_FREE_GB = 30  # a first build writes about 20-30 GB: extracted/, assets_baked/, target/ and dist/


def mac_warnings(root, home, *, python_machine, arm64_hardware, translated, rustc_host, free_bytes, ancestors):
    """Warnings, never refusals, for the Mac setups that make the bake much slower. All inputs are measured by the
    caller (mac_facts), so this stays a pure function of them."""
    warnings = []
    if translated or (arm64_hardware and python_machine == "x86_64"):
        warnings.append("Python runs under Rosetta (x86_64 on an Apple Silicon Mac): the bake will be 2-3x slower. "
                        "Use an arm64 Terminal and Homebrew in /opt/homebrew (`uname -m` should say arm64).")
    if arm64_hardware and rustc_host and rustc_host.startswith("x86_64"):
        warnings.append(f"Rust builds for {rustc_host} on an Apple Silicon Mac: the bake tool runs under Rosetta, "
                        "2-3x slower. Install Rust from https://rustup.rs in an arm64 Terminal.")
    real = os.path.realpath(root)
    synced = [os.path.join(home, "Desktop"), os.path.join(home, "Documents"), os.path.join(home, "Library", "Mobile Documents")]
    if any(real == folder or real.startswith(folder + os.sep) for folder in synced):
        warnings.append(f"the checkout is in {real}: with iCloud Drive on, macOS uploads (and may evict) the tens of "
                        "thousands of files the build writes. A folder iCloud does not sync, such as ~/openbf, is much faster.")
    elif real.startswith(os.path.join(os.sep, "Volumes") + os.sep):
        warnings.append(f"the checkout is on an external volume ({real}): the build reads and writes about 20-30 GB; "
                        "a folder on the internal disk is much faster.")
    if free_bytes is not None and free_bytes < MAC_FREE_GB * 10**9:
        warnings.append(f"{free_bytes / 10**9:.0f} GB free; a first build needs about {MAC_FREE_GB} GB.")
    if "caffeinate" not in ancestors:
        warnings.append("the Mac may sleep during the build: run it as `caffeinate -i python3 scripts/pack.py`, "
                        "with the lid open and the charger in.")
    return warnings


def mac_facts():
    """What mac_warnings reads, measured on this Mac; anything that cannot be read counts as unknown."""
    import platform

    def output(*cmd):
        try:
            return subprocess.run(cmd, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=10).stdout.strip()
        except (OSError, subprocess.SubprocessError):
            return ""

    rustc = output("rustc", "-vV")
    host = next((line.split(":", 1)[1].strip() for line in rustc.splitlines() if line.startswith("host:")), None)
    try:
        free = shutil.disk_usage(ROOT).free
    except OSError:
        free = None
    ancestors, pid = [], os.getppid()
    for _ in range(32):  # the parent chain, to see whether caffeinate started this run
        line = output("ps", "-o", "ppid=,comm=", "-p", str(pid)).split(None, 1)
        if len(line) != 2:
            break
        ancestors.append(os.path.basename(line[1].strip()))
        if not line[0].isdigit() or int(line[0]) <= 1:
            break
        pid = int(line[0])
    return dict(python_machine=platform.machine(), arm64_hardware=output("sysctl", "-n", "hw.optional.arm64") == "1",
                translated=output("sysctl", "-n", "sysctl.proc_translated") == "1", rustc_host=host,
                free_bytes=free, ancestors=ancestors)


def warn_mac():
    if sys.platform != "darwin":
        return
    for warning in mac_warnings(ROOT, os.path.expanduser("~"), **mac_facts()):
        print(f"   macOS warning: {warning}", flush=True)


# What the missions are made by: when any of it changes, they are stale and made again. (The bake's groups have their
# own keys: scripts/bake_cache.py.)
MISSION_INPUTS = ("scripts/gen_missions.py", "scripts/extract_lua.py", "scripts/pack.py", "scripts/console_input.py", "scripts/input_scan.py",
                  "scripts/gameplay_missions.py", "engine/crates/manifest/data/gameplay/corrections-v1")
# Kept in extracted/ (local, ignored by git): engine/missions/ is tracked in the dev tree.
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
            if f.endswith((".rs", ".toml", ".json")) and not test_only(os.path.relpath(os.path.join(d, f), full)))
        for f in files:
            h.update(os.path.relpath(f, ROOT).replace(os.sep, "/").encode())
            with open(f, "rb") as source:
                h.update(source.read().replace(b"\r\n", b"\n"))
    return h.hexdigest()


def test_only(relative):
    """Test and example sources change no output: tests/ and examples/ folders, tests.rs and *_tests.rs."""
    parts = relative.replace(os.sep, "/").split("/")
    name = parts[-1]
    return any(part in ("tests", "examples") for part in parts[:-1]) or name == "tests.rs" or name.endswith("_tests.rs")


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


def git_tracked_missions():
    """Tracked missions, or nothing for a checkout with no git around it (a source archive, say)."""
    try:
        return subprocess.run(["git", "-C", ROOT, "ls-files", "--", "engine/missions"], capture_output=True, text=True).stdout.strip()
    except OSError:
        return ""


def missing_shadows():
    """BF1 vehicle shadows are written beside each side's models. None at all means a bake from before them."""
    sides = glob.glob(os.path.join(BAKED, "side_bf1", "vehicles", "*", "vehicles.ron"))
    if sides and not glob.glob(os.path.join(BAKED, "side_bf1", "vehicles", "*", "shadows", "*.ron")):
        return sides
    return []


def restored_music_matches(path, rate, frames, size):
    """Check the FLAC STREAMINFO written by the audio bake."""
    try:
        with path.open("rb") as file:
            header = file.read(43)
            if os.fstat(file.fileno()).st_size != size:
                return False
    except FileNotFoundError:
        return False
    if len(header) != 43 or header[:4] != b"fLaC" or header[4] & 0x7f or header[5:8] != b"\0\0\x22":
        return False
    layout = int.from_bytes(header[18:26], "big")
    return (layout >> 44, (layout >> 41) & 7, (layout >> 36) & 31, layout & ((1 << 36) - 1)) == (
        rate, 1, 15, frames
    )


def missing_music(include_global=True):
    """Referenced retail bodies and restored takes missing from the cached bake."""
    out = []
    baked = Path(BAKED)
    indexes = list(baked.glob("*/sounds/streams/streams.ron"))
    if include_global:
        indexes += list(baked.glob("audio/global_*/global.ron"))
    indexes += list(baked.glob("*/shell/sounds/streams/streams.ron"))
    for idx in indexes:
        text = idx.read_text(encoding="utf-8")
        header = (r'name_hash:\s*(\d+),\s*source:\s*"([^"]+)",[^)]*?'
                  r'data_offset:\s*(\d+),\s*bytes:\s*\d+,\s*baked:\s*false')
        for stream in re.finditer(header, text):
            name, source, offset = stream.groups()
            stem = source.rsplit(".", 1)[0].lower()
            body = idx.parent / "music" / f"{stem}_{int(offset):x}.bin"
            if not body.is_file():
                out.append(str(body))
            segments = ron_array(text[stream.end():], "segments")
            for cue in re.finditer(r'\(([^()]*)\)', segments):
                record = cue[1]
                size = re.search(r'\brestored_bytes:\s*(\d+)', record)
                if not size or not int(size[1]):
                    continue
                ident = int(re.search(r'\bid:\s*(\d+)', record)[1])
                start = int(re.search(r'\boffset:\s*(\d+)', record)[1])
                take = idx.parent / "music_restored" / body.stem / f"{int(name):08x}_{ident:08x}_{start:x}.flac"
                rate = int(re.search(r'\bfrequency:\s*(\d+)', record)[1])
                frames = int(re.search(r'\bframes:\s*(\d+)', record)[1])
                if not restored_music_matches(take, rate, frames, int(size[1])):
                    out.append(str(take))
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


def map_texture_sources(baked, base_labels=None):
    """Stored map-local provenance, not a claim about files read this run. `base_labels` names the format of each game's
    gameplay data, where it is not the PS2's."""
    base_labels = base_labels or {}
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
                if not filename or path.is_absolute() or bool(path.root) or '..' in path.parts:
                    raise ValueError("texture_tier2 file must be relative to textures/")
                if name in files and files[name] != filename:
                    raise ValueError(f"conflicting texture_tier2 files for {name!r}")
                files[name] = filename
            textures = manifest.parent / 'textures'
            tier_names = {name for name, _ in tiers}
            present = {name for name, filename in files.items() if (textures / filename).is_file()}
            row.update(source_game=game, base_source=f"{game} {base_labels.get(game, 'PS2')}",
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


def source_report(root, baked, missions, images, plan, extracted_used=(), images_used=(), rebaked=None, pc_roots=None, warnings=(), images_verified=()):
    """Only successful pack steps establish which inputs were consumed. `rebaked` is what the bake did this run: None
    for nothing, "all", or a list of groups. "cached" means an input of the bake packed, not read this run: the group
    keys hold its content, so a bake made without it would have been made again."""
    root = Path(root)
    inputs = []
    pc_roots = pc_roots or {}
    pc_read = rebaked == "all" or "side" in (rebaked or ())

    def note(path, label, status, reason):
        inputs.append(dict(path=str(Path(path).absolute()), label=label,
                           present=Path(path).exists(), status=status, reason=reason))

    selected = set()
    gameplay_labels = {"xbox-disc": "Xbox", "pc-install": "PC"}
    base_labels = {}
    for game, image in images.items():
        if game in pc_roots:
            note(pc_roots[game], f'{game} PC visual input', 'used' if pc_read else 'cached',
                 'passed to the texture-tier bake this run' if rebaked == "all" else
                 'passed to the soldier, weapon and vehicle bake this run; the rest of the bake is from an earlier run' if pc_read else
                 'not read this run; the cached bake packed was made with it')
        if image:
            selected.add(Path(image).resolve())
            used = game in images_used
            note(image, f'{game} requested PS2 image', 'used' if used or game in images_verified else 'cached',
                 'extracted successfully this run' if used else
                 'identity checked this run (its digest is remembered by size, change time and inode; --rebake reads it again); verified extraction reused' if game in images_verified else
                 'not read or verified this run; cached extraction used')
        gameplay = (plan.get('games', {}).get(game) or {}).get('gameplay') or {}
        platform = gameplay_labels.get(gameplay.get('kind'))
        if platform:
            base_labels[game] = platform
            note(gameplay.get('tree') or gameplay['path'], f'{game} {platform} gameplay data', 'used' if game in extracted_used else 'cached',
                 'read for the console Lua cache, mission generation or bake this run' if game in extracted_used else
                 'not read this run; the cached bake packed was made with it')
            continue
        used = game in extracted_used
        note(root / 'extracted' / game, f'{game} extracted PS2 data', 'used' if used else 'cached',
             'read for the console Lua cache, mission generation or bake this run' if used else 'not read this run; cached bake and missions packed')
    chosen = {Path(path).resolve() for path in pc_roots.values()} | selected
    for row in plan['found']:
        if Path(row['path']).resolve() not in chosen:
            inputs.append(dict(path=row['path'], label=f"found: {row['kind']}", present=True,
                               status=row['status'], reason=row['reason']))
    note(baked, 'baked game assets', 'used', 'packed into the game; stored provenance listed separately')
    note(missions, 'mission data', 'used', 'packed into the game')
    return dict(schema=1, inputs=inputs, maps=map_texture_sources(baked, base_labels),
                input_scan=plan, warnings=list(warnings),
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
    for line in report.get('warnings', ()):
        print(f'WARNING: {line}')
    print(f'Source report: {path}')
    return path



def check_foliage(bake, plan):
    """Stop a build that lost its foliage, cached bake or not. A game with no PS2 gameplay data does not owe the PS2's counts,
    so what it lost becomes a warning in the summary; the other game is still held to them. Returns the warnings, for the
    source report."""
    command = [bake, "check-foliage", BAKED]
    for game, chosen in sorted(plan["games"].items()):
        if (chosen["gameplay"] or {}).get("kind") not in ("ps2-disc", "ps2-extracted"):
            command += ["--warn", game]
    print("   $ " + " ".join(command), flush=True)
    result = subprocess.run(command, capture_output=True, text=True)
    print(result.stdout, end="")
    if result.returncode:
        sys.exit(f"pack: {result.stderr.strip()}")
    return [line[len("WARNING: "):] for line in result.stdout.splitlines() if line.startswith("WARNING: ")]


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



def gameplay_correction_args(plan):
    """Mission corrections use the same chosen source and exact fingerprint as the bake."""
    args = []
    for game, chosen in plan["games"].items():
        source = chosen["gameplay"] or {}
        kind = source.get("kind")
        platform = {"pc-install": "pc", "xbox-disc": "xbox"}.get(kind)
        if platform:
            table = os.path.join(ROOT, "engine", "crates", "manifest", "data", "gameplay", "corrections-v1", f"{game}-{platform}.json")
            args += ["--gameplay-reference", table, "--gameplay-source", f"{game}={source.get('tree') or source['path']}"]
    return args

def menu_font_input(value):
    """Only an explicitly supplied local font source participates in the cache."""
    if not value:
        return None
    path = Path(value).expanduser().resolve()
    if not path.is_file():
        raise ValueError("--menu-font-core needs a local PS2 BF2 CORE.LVL file")
    return dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def group_versions(bake):
    """`bake format-versions`: what each bake group's key starts from."""
    return bake_cache.format_versions(bake)


def pc_lvl_root(game, path):
    """The _LVL_PC folder the bake reads under a chosen PC install (rebake.sh's find_pc_lvl), else the path itself."""
    return find_pc_lvl_dirs(path).get(game, path) if os.path.isdir(path) else path


def bake_group_inputs(plan, bake_env, images, bake_inputs, lua_root, memo):
    """The content of every input the bake groups read, by the names bake_cache.INPUTS gives them. Content, not
    paths: moving iso/ or an install changes nothing, a changed byte in what a group reads bakes it again."""
    def tree(path):
        return memo.tree(path) if path else None

    gameplay = {"pc_only": bake_env.get("OPENBF_PC_ONLY") == "1"}
    sound_trees = {}
    for g in sorted(images):
        gameplay[g] = bake_inputs.get(g + "_image") if images[g] else tree(lvl_root(g))
        legacy = os.path.join(EXTRACTED, g, "sound", "DATA", "_LVL_PS2")
        # rebake.sh reads an old extraction's separate sound tree only for a game with no image and no other tree.
        sound_trees[g] = tree(legacy) if not images[g] and g not in GAMEPLAY_TREES and os.path.isdir(legacy) else None
    number = {"bf1": "1", "bf2": "2"}
    return dict(
        gameplay=gameplay,
        pc={g: tree(pc_lvl_root(g, bake_env[f"PC{n}"])) for g, n in number.items() if bake_env.get(f"PC{n}")},
        xbox={g: tree(bake_env[f"XB{n}"]) for g, n in number.items() if bake_env.get(f"XB{n}")},
        space=bake_inputs.get("space"),
        maps={name: bake_env.get(name) for name in ("OPENBF_BF1_MAPS", "OPENBF_BF2_MAPS",
                                                    "OPENBF_BF1_WITHHELD", "OPENBF_BF2_WITHHELD")},
        menu_font=(bake_inputs.get("menu_font_core") or {}).get("sha256"),
        sound_trees=sound_trees,
        dlc=[tree(bake_env.get("OPENBF_DLC_DIR")), tree(bake_env.get("OPENBF_DLC_DONOR"))],
        dlc_scripts=[memo.digest_if_file(os.path.join(ROOT, "scripts", name)) for name in ("bake_dlc.py", "dlc_inputs.py")],
        lua=tree(lua_root),
    )


def bake_plan(store, keys, rebake):
    """[(group, action, reason)]: what the bake does this run (bake_cache.plan)."""
    return bake_cache.plan(store, keys, forced_groups(), rebake=rebake)


def forced_groups():
    """Groups whose key matches but whose output is known to be incomplete: retail music bodies or restored takes
    missing (sounds, and common for the shell's), or a side baked before BF1 vehicle shadows."""
    forced = {}
    missing = missing_music(include_global=False)
    if missing:
        reason = f"{len(missing)} music files missing or incomplete"
        forced["sounds"] = reason
        if any("shell" in Path(path).parts for path in missing):
            forced["common"] = reason
    if missing_shadows():
        forced["side"] = "no BF1 vehicle shadows in this bake"
    return forced


def clear_baked(path):
    """Empty the bake folder for a bake from nothing. Never through a link: that folder is another checkout's."""
    if os.path.islink(path):
        sys.exit(f"pack: {path} is a link to another checkout's assets; pass --baked with a folder of your own")
    if os.path.isdir(path):
        shutil.rmtree(path)


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
    ap.add_argument("--menu-font-core", default=os.environ.get("OPENBF_MENU_FONT_CORE"),
                    help="optional local PS2 BF2 CORE.LVL for the approved BF2 Light menu glyphs only; native PC/Xbox fallback uses different glyphs")
    ap.add_argument("--game-bin", default=None, help="use an already compiled game binary (skips step 1 cargo build)")
    ap.add_argument("--out", default=os.path.join(ROOT, "dist", "openbf" + EXE), help="the game file to write")
    ap.add_argument("--rebake", action="store_true", help="bake every asset again, from an empty assets_baked/")
    ap.add_argument("--bake-only", action="store_true", help="bring engine/missions and engine/assets_baked up to date "
                    "(steps 2-5, with the bake cache) and stop: no game binary, no pack")
    ap.add_argument("--explain", action="store_true", help="say for every bake group whether it was reused or why it bakes again")
    ap.add_argument("--baked", default=None, help="the folder the bake writes (default: engine/assets_baked); one per input profile")
    ap.add_argument("--visual-packs", default=None, help="prepared pack folder or recursive lookup of packs/bundles (default: repo-root visual_packs/; discovery stops at pack.json)")
    ap.add_argument("--retail-audio", action="store_true", help="keep the original audio presentation")
    a = ap.parse_args()
    if a.baked:
        global BAKED
        BAKED = os.path.abspath(a.baked)
    try:
        packs, visual_report = visual_packs.discover(a.visual_packs or Path(ROOT) / "visual_packs", explicit=a.visual_packs is not None)
    except visual_packs.PackError as error:
        sys.exit(f"pack: {error}")
    out = os.path.abspath(a.out)
    if out.startswith(os.path.join(ENGINE, "")) or os.path.isdir(out):
        sys.exit(f"pack: --out {out} must be a file outside engine/")
    named_pc = resolve_pc_roots(a.pc_dir, a.pc1 or os.environ.get("PC1"), a.pc2 or os.environ.get("PC2"))
    roots = [a.iso_dir] if a.iso_dir else input_scan.default_roots(ROOT)
    plan = input_scan.scan(roots, {"bf1": a.iso1, "bf2": a.iso2}, named_pc, EXTRACTED, a.iso_dir)
    with pack_cache.outputs_locked((out, os.path.join(os.path.dirname(out), ".openbf-pack"))):
        print("\n".join(input_scan.summary_lines(plan)))
        input_scan.write_summary(plan, out)
        print(f"Input summary: {out}.inputs.txt")
        if plan["refusals"]:
            message = "pack: " + "\npack: ".join(plan["refusals"])
            sys.exit(message.encode("utf-8", "backslashreplace").decode("utf-8"))
        if a.scan_only:
            return
        CLOCK.reset()
        try:
            with pack_cache.outputs_locked((EXTRACTED, BAKED, MISSIONS)):
                build(a, out, plan, packs, visual_report)
        finally:
            totals = CLOCK.totals()
            if totals:
                print(f"\n{totals}", flush=True)


def build(a, out, plan, packs, visual_report):
    try:
        menu_font = menu_font_input(a.menu_font_core)
    except ValueError as error:
        sys.exit(f"pack: {error}")
    menu_font_core = menu_font["path"] if menu_font else None
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
    if menu_font_core:
        bake_inputs["menu_font_core"] = menu_font
    bake_inputs["gameplay_trees"] = dict(GAMEPLAY_TREES)
    bake_inputs["audio_restoration"] = not a.retail_audio
    if plan["dlc"]["used"]:
        bake_inputs["dlc"] = plan["dlc"]["path"]
        if plan["dlc"]["donor"]:
            bake_inputs["dlc_donor"] = plan["dlc"]["donor"]
    images_used, extracted_used = set(), set()
    rebaked = None

    # Both are needed by every run (the global-sound step always runs), so check before the first cargo build.
    need_ffmpeg()
    warn_mac()

    step("1/6 build bake + game (release)")
    if a.bake_only:
        # A development checkout's own bake (the one its sweeps and tests read): keyed and current like a pack's.
        need("cargo", "the standalone bake tool")
        run(["cargo", "build", "--release", "-p", "bake"], cwd=ENGINE)
        bake, game = os.path.join(TARGET, "release", "bake" + EXE), None
    elif a.game_bin:
        game = os.path.abspath(a.game_bin)
        if not os.path.isfile(game):
            sys.exit(f"pack: --game-bin `{game}` not found")
        bake = os.path.join(os.path.dirname(game), "bake" + EXE)
        if not os.path.isfile(bake):
            bake = os.path.join(TARGET, "release", "bake" + EXE)
        if not os.path.isfile(bake):
            need("cargo", "the standalone bake tool")
            run(["cargo", "build", "--release", "-p", "bake"], cwd=ENGINE)
        print(f"   using existing game binary: {game}")
    else:
        need("cargo", "the Rust toolchain, https://rustup.rs")
        run(["cargo", "build", "--release", "-p", "game"], cwd=ENGINE)
        run(["cargo", "build", "--release", "-p", "bake"], cwd=ENGINE)
        bake, game = os.path.join(TARGET, "release", "bake" + EXE), os.path.join(TARGET, "release", "game" + EXE)
    try:
        versions = group_versions(bake)
    except ValueError as error:
        sys.exit(f"pack: {error}")
    memo = bake_cache.DigestMemo(os.path.join(EXTRACTED, "digests.json"), fresh=a.rebake)

    step("2/6 the discs")
    for g, image in images.items():
        if g in pc_roots:
            print(f"   {g}: larger textures from the PC install ({pc_roots[g]})")
        if g in GAMEPLAY_TREES:
            continue
        if image:
            if pack_cache.extract_image(bake, "extract-iso", image, os.path.join(EXTRACTED, g, "lvl"), run,
                                        version=versions["groups"]["extract-iso"], fresh=a.rebake, memo=memo):
                images_used.add(g)
            # A selected image's verified extraction takes precedence over an
            # unattested legacy sound/ tree, even when that tree still exists.
            bake_inputs[g + "_image"] = json.loads(Path(EXTRACTED, g, "lvl.image.json").read_text())["identity"]
        elif not disc_executable(g):
            print(f"   {g}: no disc executable: foliage uses the numbers shipped with the source")
    for g, image, tree in input_scan.discs_to_extract(plan):
        destination = Path(tree).parent.parent
        pack_cache.extract_image(bake, input_scan.XBOX_EXTRACT, image, destination, run,
                                 version=versions["groups"]["extract-iso"], fresh=a.rebake, memo=memo)
        receipt = destination.with_name(destination.name + ".image.json")
        bake_inputs["xbox_" + g] = json.loads(receipt.read_text(encoding="utf-8"))["identity"]
    bake_inputs["gameplay_trees"] = dict(GAMEPLAY_TREES)

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
    tracked = git_tracked_missions()
    have = glob.glob(os.path.join(MISSIONS, "*.ron"))
    correction_args = gameplay_correction_args(plan)
    # Keep the full Lua cache for sound names. For each PC primary the mission
    # tables must supply the required facts even when that cache is populated.
    table_only = [game for game, chosen in plan["games"].items() if chosen["gameplay"]["kind"] == "pc-install"]
    table_only_args = [arg for game in table_only for arg in ("--table-only-game", game)]
    mission_env = dict(os.environ)
    for name in ("OPENBF_GAMEPLAY_CORRECTIONS", "OPENBF_GAMEPLAY_SOURCE_BF1", "OPENBF_GAMEPLAY_SOURCE_BF2"):
        mission_env.pop(name, None)
    space = os.environ.get("OPENBF_BAKE_SPACE", "1") != "0"
    bake_inputs["space"] = space
    mission_inputs = {"console_lua": lua_root, "corrections": correction_args, "table_only": table_only, "space": space}
    why = stale(MISSION_STAMP, MISSION_INPUTS, mission_inputs) if have and not tracked else None
    if have and (tracked or (not a.rebake and not pc_roots and why is None)):
        print(f"   already there ({MISSIONS})")
    else:
        if why:
            print(f"   making the missions again: {why}")
        Path(MISSION_STAMP).unlink(missing_ok=True)
        os.makedirs(os.path.dirname(MISSIONS), exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="missions-", dir=os.path.dirname(MISSIONS)) as staging:
            command = [sys.executable, "-I", os.path.join(ROOT, "scripts", "gen_missions.py"),
                       "--ps2-only", "--lua-root", lua_root, "--out", staging, "--hunt"]
            run(command + (["--space"] if space else []) + correction_args + table_only_args, env=mission_env)
            pack_cache.replace_tree(staging, MISSIONS)
        extracted_used.update(("bf1", "bf2"))
        os.makedirs(EXTRACTED, exist_ok=True)
        with open(MISSION_STAMP, "w") as fh:
            fh.write(inputs_hash(MISSION_INPUTS, mission_inputs) + "\n")

    step("5/6 the bake")
    bash = find_bash()
    bake_env = dict(os.environ)
    for name in ("PC1", "PC2", "XB1", "XB2", "BF2_CONSOLE_ISO", "OPENBF_BF1_TREE", "OPENBF_BF2_TREE", "OPENBF_DLC_DIR", "OPENBF_DLC_DONOR", "OPENBF_PC_ONLY", "OPENBF_MENU_FONT_CORE", "OPENBF_AUDIO_RESTORE_SCRIPT"):
        bake_env.pop(name, None)  # the scan's choice is the whole story
    bake_env.update(input_scan.bake_env(plan))
    for g, image in images.items():
        bake_env[f"OPENBF_IMAGE_BF{g[-1]}"] = "1" if image else "0"
    bake_env["OPENBF_BAKE_BIN"] = bake.replace("\\", "/")
    bake_env["REBAKE_OUT"] = BAKED.replace("\\", "/")
    bake_env["OPENBF_GLOBAL_CACHE"] = os.path.join(EXTRACTED, "global-sound-cache")
    bake_env["OPENBF_GLOBAL_FRESH"] = "1" if a.rebake else "0"
    global_result = os.path.join(EXTRACTED, "global-sound-result.json")
    bake_env["OPENBF_GLOBAL_RESULT"] = global_result
    bake_env["OPENBF_SKIP_GLOBAL_SOUND"] = "1"
    if menu_font_core:
        bake_env["OPENBF_MENU_FONT_CORE"] = menu_font_core.replace("\\", "/")
    bake_env["OPENBF_INPUTS_CHOSEN"] = "1"  # rebake.sh need not scan again
    bake_env["OPENBF_PYTHON"] = sys.executable.replace("\\", "/")  # `python3` is not there on every Windows install
    if not a.retail_audio:
        bake_env["OPENBF_AUDIO_RESTORE_SCRIPT"] = str(Path(ROOT, "scripts", "restore_audio.py")).replace("\\", "/")
    bake_env["OPENBF_CONSOLE_LUA_ROOT"] = lua_root.replace("\\", "/")
    rebake_sh = [bash, os.path.join(ROOT, "scripts", "rebake.sh")]
    Path(EXTRACTED, "bake_inputs.sha256").unlink(missing_ok=True)  # the single whole-bake stamp this replaced
    store = bake_cache.KeyStore(BAKED)
    keys = bake_cache.group_keys(versions, bake_group_inputs(plan, bake_env, images, bake_inputs, lua_root, memo),
                                 dict(ffmpeg=bake_cache.ffmpeg_identity(bake_env, memo),
                                      restoration=bake_cache.restoration_identity(bake_env, memo)))
    memo.save()
    rows = bake_plan(store, keys, a.rebake)
    to_bake = [group for group, action, _ in rows if action == "bake"]
    print("\n".join(bake_cache.explain_lines(rows, verbose=a.explain)), flush=True)
    if to_bake:
        if not a.retail_audio:
            print("   restoring audio locally during the bake")
        if a.rebake or all(action != "hit" for _, action, _ in rows):
            # Nothing in the folder is current: bake from empty, as the reference bake does.
            store.drop_all()
            clear_baked(BAKED)
        if os.path.islink(BAKED):
            print(f"   warning: {BAKED} is a link; these groups are baked into {os.path.realpath(BAKED)} with this "
                  "checkout's code (pass --baked for a folder of your own)", flush=True)
        os.makedirs(BAKED, exist_ok=True)
        done = bake_cache.run_groups(store, keys, rows, lambda group: run([*rebake_sh, group], env=bake_env))
        rebaked = "all" if set(done) >= set(bake_cache.ORDER) else done
        extracted_used.update(("bf1", "bf2"))
    # Always plan both units, including a warm pack whose groups all match.
    # The helper verifies and materializes the complete cached result before packing.
    bake_env["OPENBF_SKIP_GLOBAL_SOUND"] = "0"
    Path(global_result).unlink(missing_ok=True)
    run([*rebake_sh, "global-sound"], env=bake_env)
    global_actions = json.loads(Path(global_result).read_text(encoding="utf-8"))
    if a.explain:
        for row in global_actions:
            print(f"   global-sound {row['game']}: {row['action']} ({row.get('reason', '')})")
    extracted_used.update(row["game"] for row in global_actions if row["action"] == "rebuilt")
    left = missing_music()
    if left:
        sys.exit(f"pack: {len(left)} music files missing or incomplete after the bake, e.g. {left[0]}")
    left = missing_shadows()
    if left:
        sys.exit(f"pack: the BF1 sides were baked without vehicle shadows, e.g. {left[0]}")

    warnings = check_foliage(bake, plan)
    if a.bake_only:
        print(f"   bake only: {BAKED} and {MISSIONS} are current; no pack written")
        return

    try:
        visual_replacements = visual_packs.resolve(packs, visual_report, BAKED)
    except visual_packs.PackError as error:
        sys.exit(f"pack: {error}")
    print("\n".join(visual_packs.summary(visual_report)), flush=True)

    step("6/6 pack")
    os.makedirs(os.path.dirname(out), exist_ok=True)
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
        # A small JSON response file avoids command-line limits on Windows. Never stage a bake copy.
        with tempfile.TemporaryDirectory(prefix="openbf-visual-") as mapping_dir:
            flags = []
            if visual_replacements:
                mapping = Path(mapping_dir) / "replacements.json"
                mapping.write_text(json.dumps(visual_replacements), encoding="utf-8")
                flags = ["--replacements", str(mapping)]
            run([tmp, "--write-pack", new, "--pak", new_pak, *flags, f"assets_baked={BAKED}", f"missions={MISSIONS}"])
        split = install_pack(new, new_pak, out, pak)
        old_report = out + ".sources.json"  # keep the old receipt if validation/writing fails
        if os.path.exists(old_report):
            os.remove(old_report)
    finally:
        os.remove(tmp)
        for leftover in (new, new_pak):
            if os.path.exists(leftover):
                os.remove(leftover)
    print()
    print("\n".join(pack_summary(out, pak, split)))
    print("It contains the games' own content: keep it on your machine, never upload or share it.")
    report = source_report(ROOT, BAKED, MISSIONS, images, plan, extracted_used, images_used, rebaked,
                           pc_roots=pc_roots, warnings=warnings, images_verified={g for g, image in images.items() if image})
    report["visual_packs"] = visual_report
    report["global_sound"] = global_actions
    report["bake_groups"] = [dict(group=group, action=action, reason=reason) for group, action, reason in rows]
    report["audio_restoration"] = not a.retail_audio
    if menu_font_core:
        report["inputs"].append(dict(path=menu_font_core, label="optional PS2 BF2 menu font source",
            present=True, status="used" if rebaked == "all" or "common" in (rebaked or ()) else "cached",
            reason="menu glyphs only; native HUD art and metrics retain their selected source",
            sha256=bake_inputs["menu_font_core"]["sha256"]))
    write_source_report(out, report)


if __name__ == "__main__":
    main()
