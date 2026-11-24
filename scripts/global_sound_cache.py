#!/usr/bin/env python3
"""The global-sound recipe and its optional transactional, per-edition cache.

rebake.sh supplies its selected sound roots; both reference and incremental
builds resolve and invoke exactly this recipe. Metadata lives outside assets.
"""
import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

SCHEMA = 1


def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def owned(path):
    """Never replace or write through a linked output or one of its parents."""
    path = Path(os.path.abspath(path))
    if any(p.is_symlink() for p in (path, *path.parents)):
        raise ValueError("output traverses a symlink; select an owned output folder")
    return path


@contextmanager
def lock(path):
    path = owned(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as handle:
        try:
            if os.name == "nt":
                import msvcrt
                handle.seek(0)
                handle.write(b"\0")
                handle.flush()
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise RuntimeError("another build is writing this output; retry after it finishes") from error
        try:
            yield
        finally:
            if os.name == "nt":
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)


def inventory(tree):
    """Complete relative-path/content inventory; links and special files fail closed."""
    tree = Path(tree)
    rows = []
    for folder, dirs, files in os.walk(tree):
        for name in dirs + files:
            path = Path(folder, name)
            if path.is_symlink() or not (path.is_file() or path.is_dir()):
                raise ValueError("cache output contains a link or special file")
        for name in files:
            path = Path(folder, name)
            rows.append([path.relative_to(tree).as_posix(), digest(path)])
    return sorted(rows)


def write_json(path, value):
    with open(path, "w", encoding="utf-8", newline="\n") as target:
        json.dump(value, target, sort_keys=True)
        target.write("\n")
        target.flush()
        os.fsync(target.fileno())


def discard(path):
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink(missing_ok=True)


def replace_tree(staged, destination):
    """Same-filesystem publication, recoverable even if killed between renames."""
    destination = owned(destination)
    previous = destination.with_name(destination.name + ".previous")
    discard(previous)
    if destination.exists():
        destination.rename(previous)
    try:
        Path(staged).rename(destination)
    except BaseException:
        if previous.exists():
            previous.rename(destination)
        raise
    discard(previous)


def ci(base, name):
    direct = Path(base, name)
    if direct.exists():
        return direct
    matches = sorted((p for p in Path(base).iterdir() if p.name.lower() == name.lower()), key=lambda p: os.fsencode(p.name)) if Path(base).is_dir() else []
    return matches[0] if matches else direct


def recipe(game, root):
    sound = ci(root, "SOUND")
    names = ("CW.LVL", "GCW.LVL") if game == "bf1" else ("GLOBAL.LVL",)
    excluded = {"cw.lvl", "gcw.lvl", "test.lvl"} if game == "bf1" else {"global.lvl"}
    sources = [ci(sound, name) for name in names]
    refs = sorted((p for p in sound.iterdir() if p.is_file() and p.suffix.lower() == ".lvl" and p.name.lower() not in excluded), key=lambda p: os.fsencode(p.name)) if sound.is_dir() else []
    if any("," in str(p) for p in sources + refs):
        raise ValueError("global-sound's LVL list cannot represent a comma in an input path")
    return sources, refs


def identity(bake, env):
    """What the global VO bytes are made by: the global-sound group's version (`bake format-versions`), this recipe,
    ffmpeg's build and the local restoration. Not the bake binary or its source: a bake change that moves these bytes
    raises the version (engine/crates/bake/src/versions.rs, guarded by scripts/bake_versions.py)."""
    import bake_cache
    ffmpeg = shutil.which("ffmpeg", path=env.get("PATH"))
    if not ffmpeg:
        raise ValueError("global-sound needs ffmpeg on PATH")
    versions = bake_cache.format_versions(bake)
    result = {"schema": SCHEMA, "version": versions["groups"]["global-sound"], "target": versions["target"],
              "helper": digest(__file__),
              "ffmpeg": [digest(ffmpeg), subprocess.check_output([ffmpeg, "-version"], env=env).decode(errors="replace")]}
    script = env.get("OPENBF_AUDIO_RESTORE_SCRIPT")
    result["restoration"] = None
    if "OPENBF_AUDIO_RESTORE_SCRIPT" in env:
        python = shutil.which(env.get("OPENBF_PYTHON", "python3"), path=env.get("PATH"))
        result["restoration"] = [digest(script), digest(python), subprocess.check_output([python, "-I", "-c", "import sys; print(sys.version); print(sys.platform)"], env=env).decode()]
    return result


def dependency_rows(paths):
    # Rust stores lowercase logical source filenames. Preserve spelling/order
    # conservatively too; physical root paths do not affect output semantics.
    return [[p.name, digest(p) if p.is_file() else None] for p in paths]


def key_for(game, root, tools):
    sources, refs = recipe(game, root)
    value = dict(tools, game=game, sources=dependency_rows(sources), references=dependency_rows(refs))
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest(), value, sources, refs


def valid(entry, key, dependencies=None):
    try:
        owned(entry)
        owned(entry / "tree")
        meta = json.loads((entry / "result.json").read_text(encoding="utf-8"))
        if meta["schema"] != SCHEMA:
            return False, "old cache schema"
        if meta["key"] != key:
            previous = meta.get("dependencies")
            if isinstance(previous, dict) and dependencies is not None:
                labels = {"sources": "stream inputs", "references": "reference banks",
                          "restoration": "audio restoration", "ffmpeg": "ffmpeg",
                          "version": "global-sound version", "target": "platform", "helper": "recipe"}
                changed = [label for field, label in labels.items()
                           if previous.get(field) != dependencies.get(field)]
                if changed:
                    return False, "changed " + ", ".join(changed)
            return False, "dependencies or tools changed"
        rows = inventory(entry / "tree")
        if not rows or rows != meta["files"] or not (entry / "tree/global.ron").is_file():
            return False, "incomplete or damaged output inventory"
        return True, "dependencies and complete output inventory match"
    except (OSError, ValueError, KeyError, TypeError):
        return False, "missing or unverifiable cache"


def build(game, root, output, bake, env, tools, cache=None, fresh=False, staging=None):
    output = owned(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    key, dependencies, sources, refs = key_for(game, root, tools)
    entry = Path(cache, game) if cache else None
    hit, reason = valid(entry, key, dependencies) if entry and not fresh else (False, "explicit fresh reference" if fresh else "cache disabled")
    if not hit:
        print(f"global-sound {game}: miss ({reason})", flush=True)
    # A killed process may leave its staging directory behind. Keep it on
    # the output filesystem but outside the tree the pack writer collects.
    staging = output.parent.parent if staging is None else owned(staging)
    with tempfile.TemporaryDirectory(prefix="global-sound-", dir=staging) as temporary:
        stage = Path(temporary, "tree")
        if hit:
            expected = json.loads((entry / "result.json").read_text(encoding="utf-8"))["files"]
            # A kill after the rename may leave the backup beside the output.
            # Replace in that case so the archive cannot collect it as an asset.
            previous = output.with_name(output.name + ".previous")
            try:
                materialized = not os.path.lexists(previous) and inventory(output) == expected
            except (OSError, ValueError):
                materialized = False
            if materialized:
                stage = None
            else:
                shutil.copytree(entry / "tree", stage)
                if inventory(stage) != expected:
                    raise ValueError("cache changed during materialization")
        else:
            stage.mkdir()
            subprocess.run([str(bake), "global-sound", ",".join(map(str, sources)), str(stage), ",".join(map(str, refs))], check=True, env=env)
            rows = inventory(stage)
            if not rows or not (stage / "global.ron").is_file():
                raise ValueError("global-sound did not write its complete result")
            if key_for(game, root, identity(bake, env))[0] != key:
                raise ValueError("global-sound inputs changed during the bake; retry")
            if entry:
                entry.parent.mkdir(parents=True, exist_ok=True)
                with tempfile.TemporaryDirectory(prefix="publish-", dir=entry.parent) as publication:
                    candidate = Path(publication, "entry")
                    candidate.mkdir()
                    shutil.copytree(stage, candidate / "tree")
                    if inventory(candidate / "tree") != rows:
                        raise ValueError("cache copy verification failed")
                    for relative, _ in rows:
                        with (candidate / "tree" / relative).open("r+b") as file:
                            os.fsync(file.fileno())
                    write_json(candidate / "result.json", dict(schema=SCHEMA, key=key, dependencies=dependencies, files=rows))
                    replace_tree(candidate, entry)
        if hit and key_for(game, root, identity(bake, env))[0] != key:
            raise ValueError("global-sound inputs changed during reuse; retry")
        if stage is not None:
            replace_tree(stage, output)
    if hit:
        print(f"global-sound {game}: hit ({reason})", flush=True)
    return dict(game=game, action="reused" if hit else "rebuilt", reason=reason, key=key,
                sound_root=str(Path(root).absolute()), sources=dependencies["sources"],
                references=dependencies["references"], dependency_check="content verified this run")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bake", required=True)
    ap.add_argument("--bf1", default="")
    ap.add_argument("--bf2", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    env = dict(os.environ)
    cache = env.get("OPENBF_GLOBAL_CACHE")
    bake_root = owned(args.out)
    output = bake_root / "audio"
    with lock(Path(args.out).with_name(Path(args.out).name + ".global-sound.lock")):
        # Cache shared by separate output trees also needs serialization.
        from contextlib import ExitStack
        with ExitStack() as locks:
            if cache:
                locks.enter_context(lock(Path(cache) / ".lock"))
            tools = identity(args.bake, env)
            games = [(g, r) for g, r in (("bf1", args.bf1), ("bf2", args.bf2)) if r]
            if not args.bf1:
                discard(output / "global_bf1")
            results = [build(game, root, output / ("global_" + game), args.bake, env, tools, cache,
                             env.get("OPENBF_GLOBAL_FRESH") == "1", staging=bake_root.parent)
                       for game, root in games]
            if env.get("OPENBF_GLOBAL_RESULT"):
                write_json(env["OPENBF_GLOBAL_RESULT"], results)


if __name__ == "__main__":
    main()
