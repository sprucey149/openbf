#!/usr/bin/env python3
"""List unused legacy Cargo caches for cleanse_delete.sh, one path<TAB>reason per row.

Only inspect engine/artifacts/lead_tmp. Branches, worktrees, open cards, Cargo locks
and processes protect their targets. Deletion remains in cleanse_delete.sh.
"""
import glob
import os
from pathlib import Path
import re
import subprocess

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
LEAD_TMP = os.path.join(ROOT, "engine/artifacts/lead_tmp")


def cards():
    records = {}
    for path in Path(ROOT, "engine/artifacts/loop/cards").rglob("*.md"):
        text = path.read_text().removeprefix("<!-- " "private -->\n")
        match = re.match(r"---\n(.*?)\n---", text, re.S)
        if not match:
            continue
        record = dict(line.split(":", 1) for line in match[1].splitlines() if ":" in line)
        record = {key.strip(): value.strip() for key, value in record.items()}
        if record.get("id"):
            records[record["id"]] = record
    return records


def git_names(root=ROOT):
    """Every local branch name and every worktree basename."""
    b = subprocess.run(["git", "-C", root, "branch", "--format=%(refname:short)"], capture_output=True, text=True).stdout.split()
    w = subprocess.run(["git", "-C", root, "worktree", "list", "--porcelain"], capture_output=True, text=True).stdout
    return set(b) | {os.path.basename(l[9:]) for l in w.splitlines() if l.startswith("worktree ")}

def target_verdicts(lt, cs, names):
    """[(path, dead?, why)] for every lt/<name>_target. Pure: cards and Git names are passed in.
    Kept, always: live branch/worktree names and open card targets. Dead: a *-scratch or
    *_pxc scratch target (`audit-<card>-*-scratch` too, once no matching worktree is live); a `<card>-render` target once its
    card is done/parked/blocked or missing; a card's own target once the card is done/parked/blocked, or while it is todo with no branch or
    worktree of that name (nothing to resume: `start` seeds a new one); a name with no card, branch or worktree."""
    protect = {k.lower() for k in names}
    by_name = {}
    for c in cs.values():
        ns = {c["id"].lower(), (c.get("branch") or "").lower()} - {""}
        for n in ns:
            by_name[n] = c
        if c.get("status") not in ("done", "archived"):
            protect |= ns
    names = {n.lower() for n in names}
    out = []
    for d in sorted(glob.glob(os.path.join(lt, "*_target"))):
        if os.path.islink(d) or not os.path.isdir(d):
            continue
        name = os.path.basename(d)[:-len("_target")].lower()
        stem = re.split(r"[-_]", name)[0]
        c = by_name.get(name)
        if name in protect or stem in protect:
            out.append((d, False, "live branch/worktree or open card"))
        elif name.endswith("-scratch") or name.endswith("_pxc"):
            out.append((d, True, "scratch target"))
        elif name.endswith("-render"):  # a capture's render target, `<card>-render` (PXWA/PXWB left 47 GB): its card's
            rc = next((x for x in cs.values() if x["id"].lower() == stem), None)
            if not rc or rc.get("status") in ("done", "parked", "blocked"):
                out.append((d, True, f"render target, card {stem.upper()} is {rc.get('status') if rc else 'missing'}"))
            else:
                out.append((d, False, f"render target, card {rc['id']} is {rc.get('status')}"))
        elif c and c.get("status") in ("done", "parked", "blocked"):
            out.append((d, True, f"card {c['id']} is {c['status']}"))
        elif c and c.get("status") == "todo" and name not in names:
            out.append((d, True, f"card {c['id']} is todo with no branch or worktree"))
        elif c:
            out.append((d, False, f"card {c['id']} is {c.get('status')}"))
        elif name in names:
            out.append((d, False, "live branch/worktree"))
        else:
            out.append((d, True, "no card, branch or worktree"))
    # A cargo target nested in a card's own scratch dir, lt/<card>/.../<dir>/CACHEDIR.TAG (card WEARY: HJAM's measuring
    # target lead_tmp/hjam/mtgt held 152 GB after the card was done): dead with its card, as the card's own target is.
    for d in nested_targets(lt):
        name = os.path.relpath(d, lt).split(os.sep)[0].lower()
        c = by_name.get(name)
        if c and c.get("status") in ("done", "parked", "blocked") and name not in protect and re.split(r"[-_]", name)[0] not in protect:
            out.append((d, True, f"nested target, card {c['id']} is {c['status']}"))
    return out

def nested_targets(lt, name=None):
    """Every cargo target dir (it holds a CACHEDIR.TAG) one to three levels inside lt/<name>/ (every <name> that is not a
    *_target when None); a target is not searched inside and a symlink is not followed."""
    out = []

    def walk(d, depth):
        try:
            subs = [e.path for e in os.scandir(d) if e.is_dir(follow_symlinks=False)]
        except OSError:
            return
        for sd in sorted(subs):
            if os.path.isfile(os.path.join(sd, "CACHEDIR.TAG")):
                out.append(sd)
            elif depth < 3:
                walk(sd, depth + 1)

    tops = [name] if name else [e.name for e in os.scandir(lt) if e.is_dir(follow_symlinks=False)] if os.path.isdir(lt) else []
    for n in sorted(tops):
        top = os.path.join(lt, n)
        if not n.endswith("_target") and os.path.isdir(top) and not os.path.islink(top):
            walk(top, 1)
    return out

def target_busy(path):
    """A cargo build holding the target's lock, or a process running from / inside it."""
    import fcntl
    for prof in ("release", "debug"):
        lk = os.path.join(path, prof, ".cargo-lock")
        if os.path.exists(lk):
            try:
                with open(lk) as f:
                    fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return True
            except OSError:
                pass
    return bool(procs_in(path))

def procs_in(path):
    """Pids of the processes whose cwd or executable is inside path, or with an argument naming a path inside it (a
    `cargo --target-dir <it>` or a sweep's `--tree <it>` started from elsewhere). Never this process."""
    reals = {p.rstrip("/") + "/" for p in (os.path.realpath(path), os.path.abspath(path))}
    inside = lambda s: any((s.rstrip("/") + "/").startswith(r) for r in reals)
    out = []
    for p in glob.glob("/proc/[0-9]*"):
        pid = int(os.path.basename(p))
        if pid == os.getpid():
            continue
        try:
            if any(inside(os.readlink(os.path.join(p, link))) for link in ("cwd", "exe") if os.path.lexists(os.path.join(p, link))):
                out.append(pid); continue
        except OSError:
            pass
        try:
            args = open(os.path.join(p, "cmdline"), "rb").read().decode(errors="replace").split("\0")
        except OSError:
            continue
        if any(inside(w) for a in args for w in re.split(r"[=\s]", a) if w.startswith("/")):
            out.append(pid)
    return out


def main():
    names = git_names()
    for path, dead, why in target_verdicts(LEAD_TMP, cards(), names):
        if dead and not target_busy(path):
            print(f"{path}\t{why}")


if __name__ == "__main__":
    main()
