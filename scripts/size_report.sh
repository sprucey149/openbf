#!/usr/bin/env bash
# Source-size report for engine/crates, and the check that keeps engine/CRATES.md
# (the crate/module map) true. Queue CMAP, 2026-09-23. NOT part of ci: a build
# must never fail on size mid-feature — a cleanse or a split runs this.
#
#   scripts/size_report.sh              per-crate line totals (src, and tests +
#                                       examples) and every non-test src file
#                                       over 1,500 lines
#   scripts/size_report.sh --check-map  also: exit 1 unless every
#                                       engine/crates/*/src/**/*.rs has its own
#                                       row in engine/CRATES.md and every path named
#                                       there exists, except local game data;
#                                       rows whose recorded line
#                                       count is stale are listed (not a failure)
#   scripts/size_report.sh --strict     also: exit 1 for any src file over 2,000
#                                       lines that scripts/size_waivers.txt does
#                                       not name (the flags combine)
#
# Env:  ROOT  repo or worktree root (default: this script's ..)
#
# "src" = engine/crates/<crate>/src/**/*.rs; lines are `wc -l`. "Non-test" drops
# src files that are test modules of their own (`tests.rs`, anything under a
# `tests/` dir); an in-file `mod tests` counts, since a session reads it too.
# 1,500 is the cap a new split aims under (engine/artifacts/briefs/SPLIT_RECIPE.md);
# 2,000 is the line --strict holds. A path in CRATES.md is any backticked token
# of letters, digits, `_ . - /` with a `/` in it (so `<name>`, `*` and `::` are
# not paths), resolved against the repo root, engine/ and engine/crates/.
#
# Exit: 0 clean, 1 a check failed, 2 bad args or a missing input file.
set -u

ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
CHECK_MAP=0
STRICT=0
for arg in "$@"; do
  case "$arg" in
    -h|--help) sed -n '2,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    --check-map) CHECK_MAP=1 ;;
    --strict) STRICT=1 ;;
    *) echo "size_report: unknown argument: $arg (try --help)" >&2; exit 2 ;;
  esac
done
[ -d "$ROOT/engine/crates" ] || { echo "size_report: no engine/crates under $ROOT" >&2; exit 2; }

exec python3 - "$ROOT" "$CHECK_MAP" "$STRICT" <<'PY'
import os
import re
import sys

root, check_map, strict = sys.argv[1], sys.argv[2] == "1", sys.argv[3] == "1"
crates_dir = os.path.join(root, "engine", "crates")
map_path = os.path.join(root, "engine", "CRATES.md")
waiver_path = os.path.join(root, "scripts", "size_waivers.txt")
CAP, HARD = 1500, 2000


def wc_l(path):
    with open(path, "rb") as f:
        return f.read().count(b"\n")


def rs_files(top):
    out = []
    for dirpath, dirnames, filenames in os.walk(top):
        dirnames.sort()
        out += [os.path.join(dirpath, n) for n in sorted(filenames) if n.endswith(".rs")]
    return out


def is_test_module(rel):
    parts = rel.split("/")
    return parts[-1] == "tests.rs" or "tests" in parts[2:-1]


crates = sorted(
    d for d in os.listdir(crates_dir) if os.path.isfile(os.path.join(crates_dir, d, "Cargo.toml"))
)
src = {}  # "core/src/ai.rs" -> lines
rows = []
for c in crates:
    s = {os.path.relpath(p, crates_dir): wc_l(p) for p in rs_files(os.path.join(crates_dir, c, "src"))}
    extra = [
        wc_l(p)
        for d in ("tests", "examples", "benches")
        for p in rs_files(os.path.join(crates_dir, c, d))
    ]
    src.update(s)
    rows.append((c, len(s), sum(s.values()), len(extra), sum(extra)))

print(f"{'crate':<10} {'src files':>9} {'src lines':>9}   {'tests+examples files':>20} {'lines':>6}")
for c, nf, nl, ef, el in rows:
    print(f"{c:<10} {nf:>9} {nl:>9}   {ef:>20} {el:>6}")
tot = [sum(r[i] for r in rows) for i in range(1, 5)]
print(f"{'total':<10} {tot[0]:>9} {tot[1]:>9}   {tot[2]:>20} {tot[3]:>6}")

waivers, bad_waivers = {}, []
if os.path.isfile(waiver_path):
    with open(waiver_path, encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            line = line.split("#", 1)[0].strip()
            if not line:
                continue
            path, _, reason = line.partition(" ")
            path = path.removeprefix("engine/crates/")
            if reason.strip():
                waivers[path] = reason.strip()
            else:
                bad_waivers.append(f"scripts/size_waivers.txt:{n}: {path} has no reason, so it waives nothing")

big = sorted(((n, p) for p, n in src.items() if n > CAP and not is_test_module(p)), reverse=True)
print(f"\nnon-test src files over {CAP:,} lines: {len(big)}")
for n, p in big:
    note = f"  [waived: {waivers[p]}]" if p in waivers else ""
    print(f"  {n:>5}  {p}{note}")
for w in bad_waivers:
    print(f"  WARNING {w}")
stale = [p for p in sorted(waivers) if p not in src or src[p] <= CAP]
for p in stale:
    state = "no longer exists" if p not in src else f"is {src[p]} lines now"
    print(f"  stale waiver: {p} {state} — delete its line in scripts/size_waivers.txt")

rc = 0
if check_map:
    if not os.path.isfile(map_path):
        print("\n--check-map: engine/CRATES.md is missing")
        sys.exit(2)
    with open(map_path, encoding="utf-8") as f:
        text = f.read()
    tokens = set(re.findall(r"`([^`\n]+)`", text))
    # A file is mapped by its OWN row ('- <path> <lines> ...'), not by a
    # mention anywhere else: that is what the map promises, and the stale-count
    # check below reads only rows.
    map_rows = set(re.findall(r"^- `([^`]+)` \d+ ", text, re.M))
    unmapped = [p for p in sorted(src) if p not in map_rows]
    bases = (root, os.path.join(root, "engine"), crates_dir)
    local_data = {"assets_baked/", "engine/assets_baked/", "engine/missions/", "extracted/", "missions/"}
    missing = []
    for t in sorted(tokens):
        if "/" not in t or not re.fullmatch(r"[\w./-]+", t) or not re.search(r"\w", t):
            continue
        rel = t.removeprefix("./")
        if rel in local_data:
            continue
        want_dir = rel.endswith("/")
        ok = any(
            (os.path.isdir if want_dir else os.path.exists)(os.path.join(b, rel)) for b in bases
        )
        if not ok:
            missing.append(t)
    drift = []
    for m in re.finditer(r"^- `([^`]+)` (\d+) ", text, re.M):
        p, n = m.group(1), int(m.group(2))
        full = os.path.join(crates_dir, p)
        if os.path.isfile(full) and wc_l(full) != n:
            drift.append(f"{p}: {n} in the map, {wc_l(full)} now")
    now = {c: nl for c, _, nl, _, _ in rows}
    for m in re.finditer(r"^\| `(\w+)` \|.*\| ([\d,]+) \|$", text, re.M):
        c, n = m.group(1), int(m.group(2).replace(",", ""))
        if c in now and now[c] != n:
            drift.append(f"crate {c}: {n:,} src lines in the map, {now[c]:,} now")
    print(f"\n--check-map: {len(src)} src files, {len(unmapped)} not in engine/CRATES.md; "
          f"{len(missing)} named paths missing; {len(drift)} stale line counts")
    for p in unmapped:
        print(f"  NOT IN MAP  {p}")
    for t in missing:
        print(f"  NO SUCH PATH  {t}")
    for d in drift:
        print(f"  stale count (info)  {d}")
    if unmapped or missing:
        rc = 1

if strict:
    over = [(n, p) for n, p in big if n > HARD and p not in waivers]
    print(f"\n--strict: {len(over)} src files over {HARD:,} lines without a waiver")
    for n, p in over:
        print(f"  UNWAIVED  {n:>5}  {p}")
    if over:
        rc = 1

sys.exit(rc)
PY
