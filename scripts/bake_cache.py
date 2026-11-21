"""Which scripts/rebake.sh groups a pack must bake again, and why.

Each group has a key: its version in engine/crates/bake/src/versions.rs (`bake format-versions`), the content of the
inputs it reads, the settings that change its output and, for the groups that encode audio, the encoder and restoration
tools. The bake's own source and binary are not part of it: a code change that changes a group's output raises that
group's version, and scripts/bake_versions.py refuses at landing a code change that did not say which. So a pull that
changes no baked byte bakes nothing, and a fix to the maps bakes only the maps.

A group's key is written as soon as that group finishes, beside the resolved output folder (never inside it: the pack
would ship it), so a run stopped part-way goes on from the group it was in. Keys go before their group runs: a group
cut off half-way has no key and runs again.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

SCHEMA = 1

# rebake.sh's own order: `sounds` reads the baked classes (its sound-names step), so it runs after them.
ORDER = ("tat1", "tat2", "common", "side", "dlc", "classes", "sounds", "load", "maps", "beep")
# global-sound has its own per-game cache (global_sound_cache.py) and runs on every pack.
GROUPS = ORDER + ("global-sound",)
# A group whose output is read by another: the reader's key carries the read group's key. sounds' sound-names step reads
# every classes/*.ron, and classes/bf2_dlc.ron is the dlc group's.
READS = {"sounds": ("classes", "dlc")}
# rebake.sh's `side` clears the DLC heroes and bakes them again: a run of `side` is also a run of `dlc`.
ALSO_BAKES = {"side": ("dlc",)}
# The groups that encode audio (bake::ogg, music_restore, beep, the shell's music and movie): ffmpeg's build and the
# local restoration change their bytes.
AUDIO = ("common", "dlc", "sounds", "global-sound", "beep")

# Which chosen inputs each group reads, from rebake.sh's arguments. Every group reads the games' gameplay data.
_DONORS = ("pc", "xbox")
INPUTS = {
    "tat1": _DONORS,
    "tat2": _DONORS,
    "maps": _DONORS + ("space", "maps"),
    "common": _DONORS + ("menu_font", "sound_trees"),
    "side": _DONORS,
    "dlc": ("dlc", "dlc_scripts"),
    "classes": (),
    "sounds": ("pc", "space", "maps", "lua", "sound_trees"),
    "global-sound": ("sound_trees",),
    "load": (),
    "beep": ("sound_trees",),
}

# What a finished group leaves behind. A key whose group's output is gone is not trusted.
OUTPUTS = {
    "tat1": ("tat1/manifest.ron",),
    "tat2": ("tat2/manifest.ron",),
    "maps": ("bf[12]_*/manifest.ron",),
    "common": ("common_bf1/common.ron", "common_bf2/common.ron"),
    "side": ("side_bf1/soldiers", "side_bf2/soldiers"),
    "classes": ("classes/bf1_all.ron", "classes/bf2_rep.ron"),
    "sounds": ("core_bf1/sounds", "core_bf2/sounds"),
    "load": ("common_bf2/load",),
    "beep": ("audio/*.ogg",),
}


def _sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def format_versions(bake):
    """`bake format-versions`, checked: {"schema", "target", "groups": {name: version}}."""
    try:
        text = subprocess.run([str(bake), "format-versions"], capture_output=True, text=True, check=True,
                              stdin=subprocess.DEVNULL).stdout
        value = json.loads(text)
        missing = [g for g in GROUPS + ("extract-iso",) if not isinstance(value["groups"].get(g), int)]
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError, TypeError, AttributeError) as error:
        raise ValueError(f"{bake} does not report its group versions ({error}); build it again from this checkout") from error
    if missing:
        raise ValueError(f"{bake} reports no version for {', '.join(missing)}; build it again from this checkout")
    return value


class DigestMemo:
    """File digests remembered by (size, mtime_ns, inode), so a warm run reads no multi-GB file twice.

    A file whose size, change time and inode all match is taken as unchanged; `fresh` hashes everything again.
    """

    def __init__(self, path=None, fresh=False):
        self.path, self.fresh, self.dirty = (Path(path) if path else None), fresh, False
        self.rows, self.used = {}, set()
        if self.path and not fresh:
            try:
                rows = json.loads(self.path.read_text(encoding="utf-8"))
                if rows.get("schema") == SCHEMA:
                    self.rows = rows["files"]
            except (OSError, ValueError, KeyError, TypeError, AttributeError):
                self.rows = {}

    def digest(self, path):
        path = os.path.abspath(path)
        stat = os.stat(path)
        stamp = [stat.st_size, stat.st_mtime_ns, stat.st_ino, stat.st_ctime_ns]  # ctime: no tool can set it back
        row = self.rows.get(path)
        self.used.add(path)
        if row and row[0] == stamp and not self.fresh:
            return row[1]
        h = hashlib.sha256()
        with open(path, "rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                h.update(block)
        self.rows[path] = [stamp, h.hexdigest()]
        self.dirty = True
        return h.hexdigest()

    def digest_if_file(self, path):
        return self.digest(path) if path and os.path.isfile(path) else None

    def tree(self, root):
        """One digest over every file under `root` by its path relative to `root`: moving the folder changes nothing,
        changing any byte in it does. None for a missing folder."""
        root = Path(root)
        if root.is_file():
            return self.digest(root)
        if not root.is_dir():
            return None
        rows = []
        for folder, dirs, files in os.walk(root):
            dirs.sort()
            for name in sorted(files):
                path = Path(folder, name)
                if path.is_file():
                    rows.append([path.relative_to(root).as_posix(), self.digest(path)])
        return _sha(rows)

    def save(self):
        """Keeps only the files this run looked at, so the memo does not grow with every old folder."""
        if not self.path or not (self.dirty or set(self.rows) - self.used):
            return
        self.rows = {path: row for path, row in self.rows.items() if path in self.used}
        self.path.parent.mkdir(parents=True, exist_ok=True)
        pending = self.path.with_name(self.path.name + ".new")
        pending.write_text(json.dumps(dict(schema=SCHEMA, files=self.rows)), encoding="utf-8")
        os.replace(pending, self.path)
        self.dirty = False


def ffmpeg_identity(env, memo):
    """The encoder's build: `ffmpeg -version` names it, and its digest catches a rebuild of the same version."""
    ffmpeg = shutil.which("ffmpeg", path=env.get("PATH"))
    if not ffmpeg:
        return None
    try:
        version = subprocess.run([ffmpeg, "-version"], capture_output=True, text=True, errors="replace", env=env,
                                 stdin=subprocess.DEVNULL, timeout=60).stdout
    except (OSError, subprocess.SubprocessError):
        version = ""
    return [memo.digest_if_file(os.path.realpath(ffmpeg)), version]


def restoration_identity(env, memo):
    """The local audio restoration (scripts/restore_audio.py and the Python that runs it), or None for retail audio."""
    script = env.get("OPENBF_AUDIO_RESTORE_SCRIPT")
    if not script:
        return None
    python = env.get("OPENBF_PYTHON") or sys.executable
    try:
        version = subprocess.run([python, "-I", "-c", "import sys; print(sys.version); print(sys.platform)"],
                                 capture_output=True, text=True, env=env, stdin=subprocess.DEVNULL, timeout=60).stdout
    except (OSError, subprocess.SubprocessError):
        version = ""
    return [memo.digest_if_file(script), version]


def group_keys(versions, inputs, audio):
    """{group: {"key": sha256, "fields": what it was made from}}. `inputs` maps input names (INPUTS) to their content
    identities, `inputs["gameplay"]` included; `audio` is {"ffmpeg": ..., "restoration": ...}."""
    keys = {}
    for group in GROUPS:  # READS names only groups that come earlier
        fields = dict(schema=SCHEMA, group=group, version=versions["groups"][group], target=versions["target"],
                      gameplay=inputs.get("gameplay"))
        for name in INPUTS[group]:
            fields[name] = inputs.get(name)
        if group in AUDIO:
            fields.update(ffmpeg=audio.get("ffmpeg"), restoration=audio.get("restoration"))
        for read in READS.get(group, ()):
            fields["reads_" + read] = keys[read]["key"]
        keys[group] = dict(key=_sha(fields), fields=fields)
    return keys


def keys_folder(baked):
    """Beside the RESOLVED output folder: a worktree's engine/assets_baked may be a link to another checkout's, and its
    keys must describe the folder they sit beside, not the checkout that looked."""
    real = Path(os.path.realpath(baked))
    return real.with_name(real.name + ".groups")


class KeyStore:
    def __init__(self, baked):
        self.baked = Path(baked)
        self.folder = keys_folder(baked)

    def path(self, group):
        return self.folder / f"{group}.json"

    def read(self, group):
        try:
            value = json.loads(self.path(group).read_text(encoding="utf-8"))
            return value if value.get("schema") == SCHEMA and value.get("group") == group else None
        except (OSError, ValueError, AttributeError):
            return None

    def write(self, group, record):
        self.folder.mkdir(parents=True, exist_ok=True)
        pending = self.path(group).with_name(f"{group}.json.new")
        with open(pending, "w", encoding="utf-8", newline="\n") as target:
            json.dump(dict(schema=SCHEMA, group=group, key=record["key"], fields=record["fields"]), target,
                      sort_keys=True, indent=1)
            target.write("\n")
            target.flush()
            os.fsync(target.fileno())
        os.replace(pending, self.path(group))

    def drop(self, group):
        self.path(group).unlink(missing_ok=True)

    def drop_all(self):
        for group in GROUPS:
            self.drop(group)


def outputs_present(baked, group):
    return all(any(Path(baked).glob(pattern)) for pattern in OUTPUTS.get(group, ()))


FIELD_LABELS = {"version": "version", "target": "platform", "gameplay": "gameplay data", "pc": "PC install",
                "xbox": "Xbox disc", "space": "space maps setting", "maps": "map list", "menu_font": "menu font source",
                "sound_trees": "sound data", "dlc": "DLC folder", "dlc_scripts": "DLC bake scripts",
                "lua": "console Lua scripts", "ffmpeg": "ffmpeg", "restoration": "audio restoration",
                "reads_classes": "the classes it reads", "reads_dlc": "the DLC classes it reads",
                "schema": "cache format", "group": "group"}


def why_stale(stored, record):
    """None when `stored` matches; else the reason, naming what changed ('version 3 -> 4', 'ffmpeg changed')."""
    if stored is None:
        return "no key recorded (first bake here, or the last run stopped during it)"
    if stored.get("key") == record["key"]:
        return None
    old, new = stored.get("fields") or {}, record["fields"]
    if old.get("version") != new.get("version"):
        return f"version {old.get('version')} -> {new.get('version')}"
    changed = [FIELD_LABELS.get(name, name) for name in sorted(set(old) | set(new)) if old.get(name) != new.get(name)]
    return ("changed " + ", ".join(changed)) if changed else "key changed"


def plan(store, keys, forced=None, rebake=False):
    """[(group, action, reason)] in ORDER. action is "bake", "hit" or "with <group>" (made by another group's run)."""
    forced = forced or {}
    rows, baking = [], set()
    for group in ORDER:
        if rebake:
            reason = "--rebake: every group from an empty folder"
        elif group in forced:
            reason = forced[group]
        else:
            reason = why_stale(store.read(group), keys[group])
            if reason is None and not outputs_present(store.baked, group):
                reason = "its output is missing"
        carrier = next((g for g, made in ALSO_BAKES.items() if group in made and g in baking), None)
        if carrier:
            rows.append((group, f"with {carrier}", reason or "baked again by " + carrier))
        elif reason:
            baking.add(group)
            rows.append((group, "bake", reason))
        else:
            rows.append((group, "hit", "key matches"))
    return rows


def run_groups(store, keys, rows, run_group):
    """Bake each planned group in order, through `run_group(group)`, and record its key (and the keys of what its run
    also made) the moment it is done."""
    baked = []
    for group, action, _ in rows:
        if action != "bake":
            continue
        made = (group, *ALSO_BAKES.get(group, ()))
        for name in made:
            store.drop(name)
        run_group(group)
        for name in made:
            store.write(name, keys[name])
        baked.extend(made)
    return baked


def explain_lines(rows, verbose=False):
    lines = []
    for group, action, reason in rows:
        if action == "hit" and not verbose:
            continue
        label = "reused" if action == "hit" else ("baked again: " + reason if action == "bake" else f"baked again with {action[5:]}")
        lines.append(f"   {group:12s} {label}")
    rebuilt = sum(1 for _, action, _ in rows if action != "hit")
    lines.append(f"   bake groups: {rebuilt} to bake, {len(rows) - rebuilt} reused")
    return lines
