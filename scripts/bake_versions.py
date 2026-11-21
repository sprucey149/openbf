#!/usr/bin/env python3
"""The landing guard for the bake groups' version numbers (engine/crates/bake/src/versions.rs).

    python3 scripts/bake_versions.py check [--base REV] [--card ID | --declares 'rebake=maps']
    python3 scripts/bake_versions.py bump GROUP... | --card ID [--base REV]
    python3 scripts/bake_versions.py neutral GROUP... | all

Players' packs rebake a group only when its version changes (scripts/bake_cache.py), so a code change that moves a
group's output and keeps its number would leave every player with the old output. This guard stops that at landing.
engine/crates/bake/bake_versions.lock records, per group, the version and a digest of the code that group's output
depends on (its closure). `check` fails when a closure changed and the lock was not brought up to date, saying which
groups. The author then either
  - bumps the group (`bump maps`, or `bump --card ISSUE-N` for every group the card's `rebake=` names, once per branch):
    its output changes and players bake it again, or
  - records it neutral (`neutral maps`): the output is byte-identical, which the bake gate (scripts/gates.py `bake`,
    a scratch bake compared with cmp_bake.sh) proves for every group the card does not declare in `rebake=`. The gate
    runs for any change under gates.py BAKE_CODE, which covers every path in SHARED and EXTRA below.
With --base (land.py passes main) and a card, `check` also refuses a `rebake=` group whose version did not rise.

Closures: every group depends on the bake's main.rs and lib.rs, the mungefs, manifest, audio and pak crates, the
workspace manifests, the part of Cargo.lock the bake builds from and scripts/rebake.sh. The visual groups (tat1, tat2,
maps, common, side, dlc, classes) depend on all of bake/src: their bakers share most modules. sounds, global-sound,
beep and load depend only on the modules their entry modules reach. extract-iso depends on the shared set (mungefs
holds the PS2 extractor) plus bake/src/xbox.rs (the Xbox one). Test-only files (tests/, examples/, tests.rs,
*_tests.rs, a trailing #[cfg(test)] module) and versions.rs itself are left out.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
BAKE_SRC = "engine/crates/bake/src"
VERSIONS = "engine/crates/bake/src/versions.rs"
LOCK = "engine/crates/bake/bake_versions.lock"
SCHEMA = 1

SHARED = ("engine/crates/bake/Cargo.toml", "engine/crates/bake/src/main.rs", "engine/crates/bake/src/lib.rs",
          "engine/crates/mungefs", "engine/crates/manifest", "engine/crates/audio", "engine/crates/pak",
          "engine/Cargo.toml", "scripts/rebake.sh")
VISUAL = ("tat1", "tat2", "maps", "common", "side", "dlc", "classes")
# The modules each small group's subcommands call (bake/src/main.rs), from which its closure is found.
ENTRY = {"sounds": ("sounds", "sound_names"), "global-sound": ("global_sound",), "beep": ("beep",),
         "load": ("loadscreen",), "extract-iso": ()}
# Files a group depends on beyond its module closure. extract-iso: `bake extract-xiso` is bake::xbox::extract, and only
# that one file is taken (not the modules xbox.rs reaches for its other jobs).
# common: rebake.sh's common group runs `gen_missions.py --shell-credits` (which imports extract_lua.py) to write
# common_bf2/shell/credits.json.
EXTRA = {"dlc": ("scripts/bake_dlc.py", "scripts/dlc_inputs.py"), "extract-iso": ("engine/crates/bake/src/xbox.rs",),
         "common": ("scripts/gen_missions.py", "scripts/extract_lua.py")}


def test_only(relative):
    parts = relative.split("/")
    name = parts[-1]
    return any(p in ("tests", "examples", "target") for p in parts[:-1]) or name == "tests.rs" or name.endswith("_tests.rs")


def strip_trailing_tests(text):
    """Drop a `#[cfg(test)] mod name { ... }` that ends the file: editing a test changes no output. Only when the brace
    count closes exactly at the end; anything else is kept whole."""
    match = None
    for match in re.finditer(r"(?m)^#\[cfg\(test\)\]\s*\n\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{", text):
        pass
    if not match:
        return text
    depth, end = 0, None
    for index in range(match.end() - 1, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    if end is None or text[end:].strip():
        return text
    return text[:match.start()]


def files_under(root, path):
    full = root / path
    if full.is_file():
        return [full]
    if not full.is_dir():
        return []
    rows = []
    for folder, dirs, files in os.walk(full):
        dirs.sort()
        for name in sorted(files):
            item = Path(folder, name)
            relative = item.relative_to(full).as_posix()
            if not test_only(relative):
                rows.append(item)
    return rows


def file_digest(root, path):
    data = path.read_bytes().replace(b"\r\n", b"\n")
    if path.suffix == ".rs":
        data = strip_trailing_tests(data.decode("utf-8", "surrogateescape")).encode("utf-8", "surrogateescape")
    return [path.relative_to(root).as_posix(), hashlib.sha256(data).hexdigest()]


def lock_closure(root):
    """The [[package]] entries of engine/Cargo.lock the bake builds from (its dependency graph), not the game's."""
    text = (root / "engine/Cargo.lock").read_text(encoding="utf-8")
    packages = {}
    for block in text.split("[[package]]")[1:]:
        name = re.search(r'^name = "([^"]+)"', block, re.M)[1]
        version = re.search(r'^version = "([^"]+)"', block, re.M)[1]
        deps = re.search(r"^dependencies = \[(.*?)\]", block, re.M | re.S)
        listed = re.findall(r'"([^"]+)"', deps[1]) if deps else []
        packages.setdefault(name, []).append((version, block.strip(), listed))

    def pick(spec):
        name, _, version = spec.partition(" ")
        options = packages.get(name, [])
        return [o for o in options if not version or o[0] == version.split(" ")[0]]

    seen, stack, rows = set(), ["bake"], []
    while stack:
        spec = stack.pop()
        for version, block, listed in pick(spec):
            if (spec.split(" ")[0], version) in seen:
                continue
            seen.add((spec.split(" ")[0], version))
            rows.append(block)
            stack.extend(listed)
    return hashlib.sha256("\n".join(sorted(rows)).encode()).hexdigest()


def module_refs(root):
    """{module: set of top-level bake modules it names} from crate::/super:: paths in its files."""
    src = root / BAKE_SRC
    modules = {p.stem for p in src.glob("*.rs")} | {p.name for p in src.iterdir() if p.is_dir()}
    modules -= {"main", "lib"}
    refs = {}
    for module in modules:
        text = ""
        for path in module_files(root, module):
            text += path.read_text(encoding="utf-8", errors="replace")
        names = set(re.findall(r"\bcrate::(\w+)", text)) | set(re.findall(r"\b(?:super::)+(\w+)", text))
        for group in re.findall(r"\bcrate::\{([^}]*)\}", text):
            names |= set(re.findall(r"\b(\w+)\b", group))
        refs[module] = (names & modules) - {module}
    return refs


def module_files(root, module):
    src = root / BAKE_SRC
    rows = [src / f"{module}.rs"] if (src / f"{module}.rs").is_file() else []
    if (src / module).is_dir():
        rows += files_under(root, f"{BAKE_SRC}/{module}")
    return [p for p in rows if not test_only(p.relative_to(src).as_posix())]


def includes(paths):
    """Files a closure's sources pull in with include_str!/include_bytes!."""
    extra = []
    for path in paths:
        if path.suffix != ".rs":
            continue
        for name in re.findall(r'include_(?:str|bytes)!\(\s*"([^"]+)"\s*\)', path.read_text(encoding="utf-8", errors="replace")):
            target = (path.parent / name).resolve()
            if target.is_file():
                extra.append(target)
    return extra


def closures(root=ROOT):
    """{group: sha256 of the code its output depends on}."""
    root = Path(root)
    shared = [p for path in SHARED for p in files_under(root, path)]
    shared += includes(shared)
    lock = lock_closure(root)
    versions = (root / VERSIONS).resolve()
    every = [p for p in files_under(root, BAKE_SRC) if p.resolve() != versions]
    refs = module_refs(root)
    out = {}
    for group in VISUAL + tuple(ENTRY):
        if group in VISUAL:
            files = shared + every
        else:
            seen, stack = set(ENTRY[group]), list(ENTRY[group])
            while stack:
                for module in refs.get(stack.pop(), ()):
                    if module not in seen:
                        seen.add(module)
                        stack.append(module)
            own = [p for module in sorted(seen) for p in module_files(root, module)]
            files = shared + own + includes(own)
        files += [p for path in EXTRA.get(group, ()) for p in files_under(root, path)]
        rows = sorted({tuple(file_digest(root, p)) for p in files if p.resolve() != versions})
        out[group] = hashlib.sha256(json.dumps([rows, lock]).encode()).hexdigest()
    return out


def parse_versions(text):
    body = re.search(r"GROUP_VERSIONS:\s*&\[\(&str,\s*u32\)\]\s*=\s*&\[(.*?)\];", text, re.S)
    if not body:
        raise ValueError(f"{VERSIONS} has no GROUP_VERSIONS table")
    rows = re.findall(r'\(\s*"([a-z0-9-]+)"\s*,\s*(\d+)\s*\)', body[1])
    versions = {}
    for name, number in rows:
        if name in versions:
            raise ValueError(f"{VERSIONS} names {name} twice")
        versions[name] = int(number)
    return versions


def read_versions(root=ROOT):
    return parse_versions((Path(root) / VERSIONS).read_text(encoding="utf-8"))


def versions_at(rev, root=ROOT):
    try:
        text = subprocess.run(["git", "-C", str(root), "show", f"{rev}:{VERSIONS}"], capture_output=True, text=True,
                              check=True).stdout
    except subprocess.CalledProcessError:
        return {}  # the base is from before versions.rs: everything on the branch is new
    return parse_versions(text)


def write_versions(root, versions):
    path = Path(root) / VERSIONS
    text = path.read_text(encoding="utf-8")
    for name, number in versions.items():
        text, n = re.subn(r'(\(\s*"' + re.escape(name) + r'"\s*,\s*)\d+(\s*\))', rf"\g<1>{number}\g<2>", text, count=1)
        if n != 1:
            raise ValueError(f"{VERSIONS} has no entry for {name}")
    path.write_text(text, encoding="utf-8")


def rebake_groups(root=ROOT):
    line = re.search(r'^REBAKE_GROUPS="([^"]*)"', (Path(root) / "scripts/rebake.sh").read_text(encoding="utf-8"), re.M)
    return line[1].split()


def read_lock(root=ROOT):
    try:
        value = json.loads((Path(root) / LOCK).read_text(encoding="utf-8"))
        return value["groups"] if value.get("schema") == SCHEMA else {}
    except (OSError, ValueError, KeyError, TypeError):
        return {}


def write_lock(root, groups):
    path = Path(root) / LOCK
    text = json.dumps(dict(schema=SCHEMA, groups={g: groups[g] for g in sorted(groups)}), indent=1, sort_keys=True)
    path.write_text(text + "\n", encoding="utf-8")


def declared_rebake(root, card=None, declares=None):
    """The groups a card's `declares: rebake=` names (maps implies tat1 and tat2, as gate_policy does)."""
    sys.path.insert(0, str(Path(root) / "scripts"))
    from gate_policy import parse_declares
    if card is not None:
        text = (Path(root) / "engine/artifacts/loop/cards/private" / f"{card}.md").read_text(encoding="utf-8")
        line = re.search(r"^declares:\s*(.*)$", text, re.M)
        declares = line[1] if line else ""
    return parse_declares(declares or "")["rebake"]


def problems(root=ROOT, base=None, declared=None):
    """Everything that stops a landing, as sentences; an empty list passes."""
    root = Path(root)
    found = []
    versions = read_versions(root)
    want = set(rebake_groups(root)) | {"extract-iso"}
    if set(versions) != want:
        found.append(f"{VERSIONS} must name exactly {', '.join(sorted(want))} "
                     f"(missing {sorted(want - set(versions))}, extra {sorted(set(versions) - want)})")
        return found
    lock = read_lock(root)
    current = closures(root)
    drifted = sorted(g for g in want if (lock.get(g) or {}).get("closure") != current[g])
    unrecorded = sorted(g for g in want if g not in drifted and (lock.get(g) or {}).get("version") != versions[g])
    if drifted:
        found.append(f"the code behind {', '.join(drifted)} changed: if their output changes, "
                     f"`python3 scripts/bake_versions.py bump {' '.join(drifted)}` (or `bump --card <ID>`); if every "
                     f"byte stays the same, `python3 scripts/bake_versions.py neutral {' '.join(drifted)}` and let the "
                     f"bake gate prove it")
    if unrecorded:
        found.append(f"{VERSIONS} and {LOCK} disagree on {', '.join(unrecorded)}: "
                     f"`python3 scripts/bake_versions.py neutral {' '.join(unrecorded)}` records the new numbers")
    if base is not None and declared:
        before = versions_at(base, root)
        stale = sorted(g for g in declared if g in versions and g in before and versions[g] <= before[g])
        if stale:
            found.append(f"the card's rebake= changes {', '.join(stale)} but their versions did not rise from {base}: "
                         f"players would keep the old output. `python3 scripts/bake_versions.py bump {' '.join(stale)}`")
    return found


def refresh(root, groups, bump=False, base=None):
    root = Path(root)
    versions = read_versions(root)
    unknown = sorted(set(groups) - set(versions))
    if unknown:
        raise ValueError(f"unknown group(s) {', '.join(unknown)} (groups: {', '.join(versions)})")
    before = versions_at(base, root) if base else {}
    if bump:
        raised = {g: versions[g] + 1 for g in groups if not (g in before and versions[g] > before[g])}
        if raised:
            write_versions(root, raised)
            versions.update(raised)
    lock = read_lock(root)
    current = closures(root)
    for g in groups:
        lock[g] = dict(version=versions[g], closure=current[g])
    write_lock(root, lock)
    return {g: versions[g] for g in groups}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="command", required=True)
    check = sub.add_parser("check", help="fail when bake code changed without a bump or a neutral record")
    bump = sub.add_parser("bump", help="raise groups' versions (once per branch against --base) and record them")
    neutral = sub.add_parser("neutral", help="record groups' changed code with unchanged versions")
    for parser in (check, bump):
        parser.add_argument("--base", default=None, help="the revision the branch lands on (main)")
        source = parser.add_mutually_exclusive_group()
        source.add_argument("--card", default=None, help="read `declares: rebake=` from this card")
        source.add_argument("--declares", default=None, help="a declares string, e.g. 'rebake=maps,side'")
    bump.add_argument("groups", nargs="*")
    neutral.add_argument("groups", nargs="+")
    ap.add_argument("--root", default=str(ROOT), help=argparse.SUPPRESS)
    a = ap.parse_args(argv)
    root = Path(a.root)
    try:
        if a.command == "check":
            declared = declared_rebake(root, a.card, a.declares) if (a.card or a.declares) else None
            found = problems(root, a.base or ("main" if declared else None), declared)
            for line in found:
                print(f"bake_versions: {line}", file=sys.stderr)
            if not found:
                print("bake_versions: every bake group's version matches its code")
            return 1 if found else 0
        if a.command == "bump":
            groups = list(a.groups)
            if a.card or a.declares:
                groups += [g for g in declared_rebake(root, a.card, a.declares) if g not in groups]
            if not groups:
                ap.error("bump needs groups or a card whose declares name rebake=")
            done = refresh(root, groups, bump=True, base=a.base or ("main" if (a.card or a.declares) else None))
        else:
            groups = sorted(read_versions(root)) if a.groups == ["all"] else a.groups
            done = refresh(root, groups)
            print("bake_versions: recorded as neutral; the bake gate must show no difference in "
                  + ", ".join(groups))
        print("bake_versions: " + ", ".join(f"{g} {v}" for g, v in done.items()))
        return 0
    except (OSError, ValueError) as error:
        print(f"bake_versions: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
