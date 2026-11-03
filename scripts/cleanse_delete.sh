#!/usr/bin/env bash
# cleanse_delete.sh — remove regenerable bloat under engine/artifacts (CLEANSE_PLAN §F2, lead-run).
#
# The harness hook (scripts/claude_guard.sh) denies any recursive delete whose command text names
# engine/artifacts, so the plan routes untracked deletions through THIS reviewed script instead.
# Default is a DRY RUN that prints what would go and how much it frees; pass --yes to delete.
# Safe to re-run any time ("every so often"): it recomputes the candidates from git state.
#
# What it removes (and why it is safe):
#   Tier A  engine/artifacts/lead_tmp/<name>_target  — dead cargo caches, by the SAME test the loop's own sweep uses
#           (scripts/target_cleanup.py: a done/parked/blocked card's target, a *-scratch / *_pxc one, or a name with no
#           card, branch or worktree; a live claim's or in-progress card's never, and never one a cargo build holds or a
#           process runs in — the dry run of `target_cleanup.py` leaves those out). Rebuilt on demand (reflink seed).
#   Tier B  scratch bakes and copies of LANDED branches: every bake tree at any depth under lead_tmp/<name>/ (found by
#           the bake's own output files, bake_trees() below), the older literal names lead_tmp/<name>/{baked,bake,before,
#           bake_before,bake_after,smoke,full_after,baked_after,c2_before,c2_after,goldens_committed,nd,base,spot1..6,…}, lead_tmp/roster_bake,
#           and the superseded s13 scratch bake + pre-s13 walkabout set (s13/baked, s13/walkabout_before) — regenerable from scripts/scratch_bake.sh (recipes are in the
#           branch READMEs). Skipped while a branch/worktree named <name> exists, a card <NAME>.md is not done, or a file
#           in the top three levels of lead_tmp/<name>/ changed in the last 3 hours (a running session's scratch).
#   Tier C  small stale files the audit verified unreferenced: the session-8 copy of NEXT_SESSION_PROMPT.md
#           at the artifacts root, briefs/*.bak_*, loose lead_tmp/*.log, lead_tmp/ordnance, __pycache__ dirs.
# What it NEVER touches: engine/assets_baked, engine/goldens, engine/target, engine/artifacts/{loop,briefs
# (except .bak_*),walkabout,walkabout_defects,p3_walk,lead_tmp/tools,lead_tmp/s* (bar the two s13 dirs named above)}, extracted/, iso/,
# community_mods/ (also when a candidate only resolves there through a link), anything tracked by git (tracked deletions are C2's `git rm` on its branch),
# and any candidate of ANY tier that a running process has as its working directory or runs a binary from (another lane's
# build, sweep or bake: a name says a branch is dead, not that nobody is in its directory — obstfix_target, 2026-10-07).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ART="$ROOT/engine/artifacts"
LT="$ART/lead_tmp"
YES=0
case "${1:-}" in
  --yes) YES=1 ;;
  ""|--dry-run) YES=0 ;;
  -h|--help) sed -n '2,21p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
  --self-test)  # a --dry-run over a fixture repo: it must list exactly the bake trees of the dead branch (CLN4)
    T="$(mktemp -d)"; trap '/bin/rm -rf -- "$T"' EXIT
    mkdir -p "$T/scripts" && cp "${BASH_SOURCE[0]}" "$T/scripts/cleanse_delete.sh" && cp "$ROOT/scripts/target_cleanup.py" "$T/scripts/"
    git -C "$T" init -q && git -C "$T" -c user.name=t -c user.email=t@t commit -q --allow-empty -m t && git -C "$T" branch livebr
    L="$T/engine/artifacts/lead_tmp"; D="$L/deadbr"
    mk() { mkdir -p "$(dirname "$1")" && : > "$1"; }
    mk "$T/engine/assets_baked/common_bf1/common.ron"
    mk "$L/livebr/bake_x/common_bf1/common.ron"                  # a live branch: never
    mk "$D/bake_whatever/common_bf1/common.ron"                  # listed
    mk "$D/land/bake_x/common_bf1/common.ron"                    # listed, one level down
    mk "$D/bake_y/tat2/manifest.ron"; mk "$D/bake_y/inner/common_bf1/common.ron"   # listed once (nested)
    mk "$D/bake-z/classes/bf2_all.ron"; mkdir -p "$D/bake-z/side_bf1/soldiers"     # listed ('-' sorts before '/')
    mk "$D/sheets/a.png"; mk "$D/land/sheets_land/a.png"          # not: sheets
    mkdir -p "$L/deadbr_target" "$L/livebr_target"                 # Tier A: the dead branch's target listed, the live one not
    mk "$D/bake_inuse/common_bf1/common.ron"; mkdir -p "$L/inuse_target/release"   # dead by name, but a process is in each: never
    ( cd "$D/bake_inuse/common_bf1" && exec sleep 120 ) & U1=$!; ( cd "$L/inuse_target/release" && exec sleep 120 ) & U2=$!
    trap 'kill "$U1" "$U2" 2>/dev/null; /bin/rm -rf -- "$T"' EXIT; sleep 0.3
    mk "$D/after/bf1_kam1/vantage.png"                            # not: a picture set
    mk "$T/engine/artifacts/loop/cards/OPENC.md"; printf -- '---\nid: OPENC\nstatus: todo\n---\n' > "$T/engine/artifacts/loop/cards/OPENC.md"
    mk "$L/openc/pics/a.png"; mkdir -p "$L/openc/pics/side_bf1/soldiers"   # not: an open card's evidence, branch or none
    printf -- '---\nid: DONEC\nstatus: done\n---\n' > "$T/engine/artifacts/loop/cards/DONEC.md"; mk "$L/donec/bake/tat1/manifest.ron"   # listed: landed
    mk "$L/donec/pics/a.png"; mkdir -p "$L/donec/pics/side_bf1/soldiers"   # a landed card's pictures stay; only the bake group beside them is listed
    mk "$D/models/bf1_hot1_hoth_cannon/manifest.ron"              # not: a single-model export
    ln -s "$T/engine/assets_baked" "$D/link_to_shipped"           # not: find does not follow it
    ln -s "$T/engine/assets_baked" "$D/bake"                      # a literal name, but it resolves into assets_baked
    ln -s "$T/engine/assets_baked/common_bf1" "$D/land/bake_x/common_bf2"   # an overlay's link: --yes drops the LINK only
    find "$T" -exec touch -h -d '-1 day' {} +                      # the fixture is old, but for one fresh dir:
    mk "$L/fresh/smoke/x.log"                                      # not: a running session's scratch (a literal name, no card)
    # the target sweep must inspect this fixture's branches, even when
    # the caller selects another repository through its environment.
    got="$(env -u OPENBF_REPO -u OPENBF_LOOP_DIR -u OPENBF_LEAD_TMP bash "$T/scripts/cleanse_delete.sh" --dry-run | awk -F'\t' 'NF == 2 { sub(".*/lead_tmp/", "", $2); print $2 }' | sort)"
    want="$(printf '%s\n' deadbr/bake-z deadbr/bake_whatever deadbr/bake_y deadbr/land/bake_x deadbr_target donec/bake donec/pics/side_bf1 | sort)"
    if [ "$got" = "$want" ]; then
      env -u OPENBF_REPO -u OPENBF_LOOP_DIR -u OPENBF_LEAD_TMP bash "$T/scripts/cleanse_delete.sh" --yes >/dev/null
      if [ -f "$T/engine/assets_baked/common_bf1/common.ron" ] && [ ! -e "$D/land/bake_x" ] && [ -f "$D/sheets/a.png" ] \
         && [ -f "$L/livebr/bake_x/common_bf1/common.ron" ] && [ ! -e "$L/deadbr_target" ] && [ -d "$L/livebr_target" ] \
         && [ -f "$D/bake_inuse/common_bf1/common.ron" ] && [ -d "$L/inuse_target/release" ] \
         && [ -f "$L/donec/pics/a.png" ] && [ ! -e "$L/donec/pics/side_bf1" ] && [ -f "$L/fresh/smoke/x.log" ]; then
        echo "self-test: PASS (the bake trees + 1 dead target listed and deleted, nothing else; the two in use, the shipped tree and a picture set beside a bake group untouched)"; exit 0
      fi
      echo "self-test: FAIL (--yes)" >&2; exit 1
    fi
    printf 'self-test: FAIL\n-- got:\n%s\n-- want:\n%s\n' "$got" "$want" >&2; exit 1 ;;
  *) echo "usage: $0 [--dry-run|--yes|--self-test]" >&2; exit 2 ;;
esac
[ -d "$LT" ] || { echo "cleanse_delete: no $LT" >&2; exit 2; }
# this checkout and, from a worktree, the main checkout its engine/artifacts link points into
ROOTS_REAL=("$(realpath "$ROOT")" "$(realpath "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)/..")")

live_names() {  # every branch name and every worktree basename currently alive
  git -C "$ROOT" branch --format='%(refname:short)'
  git -C "$ROOT" worktree list --porcelain | awk '/^worktree /{n=split($2,a,"/"); print a[n]}'
}
mapfile -t LIVE < <(live_names | sort -u)
is_live() { local n; for n in "${LIVE[@]}"; do [ "$n" = "$1" ] && return 0; done; return 1; }
# a card <NAME>.md that is not done: its work has not landed, so its lead_tmp/<name>/ is evidence, not a copy (cleanse 18:
# lead_tmp/uskn/pics held side_bf1/soldiers beside the 34 pictures card USKN cites, and USKN, a todo card, has no branch)
card_open() {
  local c
  for c in "$ART/loop/cards/private/${1^^}.md" "$ART/loop/cards/archive/${1^^}.md" "$ART/loop/cards/${1^^}.md"; do
    if [ -f "$c" ] && ! grep -q '^status: *done' "$c"; then return 0; fi
  done
  return 1
}
# a file in its top three levels changed in the last 3 hours: a session is working there (cleanse 19: a running audit's
# lead_tmp/audit-flnk/smoke, a literal name with no branch, worktree or card of that name, was listed)
recent() { [ -n "$(find "$1" -maxdepth 3 -newermt '-3 hours' -print -quit 2>/dev/null)" ]; }

# A scratch bake tree, at ANY depth under a landed branch's lead_tmp/<name>/ (CLN4): a directory holding one of the
# bake's own output files — common_bf{1,2}/common.ron, <map>/manifest.ron for a map group (tat1, tat2, bf1_kam1 …),
# classes/bf{1,2}_all.ron or side_bf{1,2}/soldiers/. A walkabout picture set (<map>/vantage.png, cp*.png), a sheet
# directory or a render never holds any of them, and a single-model export (lead_tmp/hoth/bf1_hot1_hoth_cannon/
# manifest.ron) fails the map-name pattern. Copied PICTURE sets are left alone deliberately: they are the evidence a
# report's sheets were cut from, and a name or layout cannot tell a disposable copy from a kept render.
# find never follows a symlink (-P), and only the topmost tree is listed (a bake inside a bake is the same delete).
# A tree whose top holds pictures (lead_tmp/uskn/pics: 34 cited PNGs beside a scratch side_bf1/) lists only its bake groups,
# so the pictures stay (cleanse 19).
bake_trees() {
  find "$1" -mindepth 3 \( -path '*/common_bf[12]/common.ron' -o -path '*/classes/bf[12]_all.ron' \
      -o -path '*/side_bf[12]/soldiers' -o -regextype posix-extended -regex '.*/(tat[12]|bf[12]_[a-z]{3}[0-9])/manifest\.ron' \) \
      2>/dev/null | while IFS= read -r m; do
        g="$(dirname "$m")"; r="$(dirname "$g")"
        if compgen -G "$r/*.png" >/dev/null; then echo "$g"; else echo "$r"; fi
      done | sort -u |
    awk '{ for (i = 1; i <= n; i++) if (index($0, top[i] "/") == 1) next; top[++n] = $0; print }'
}

CANDS=()
add() { [ -e "$1" ] && CANDS+=("$1"); return 0; }

# Tier A — dead cargo target dirs: target_cleanup.py protects active branches, cards and processes
while IFS=$'\t' read -r d _why; do add "$d"; done < <(python3 "$ROOT/scripts/target_cleanup.py")
# Tier B — scratch bakes of landed branches
for d in "$LT"/*/; do
  name="$(basename "$d")"
  case "$name" in tools|s[0-9]*|*_target) continue ;; esac
  is_live "$name" && { echo "keep (live): $d"; continue; }
  card_open "$name" && { echo "keep (open card): $d"; continue; }
  recent "${d%/}" && { echo "keep (recent): $d"; continue; }
  # the old literal names stay as a fallback, so nothing collected before CLN4 stops being collected
  for sub in baked baked_before bake_before bake_after smoke full_after baked_after ogg_ctl1 ogg_ctl2 before c2_before c2_after goldens_committed nd base spot1 spot2 spot3 spot4 spot5 spot6 bake bake_nopc; do add "$d$sub"; done
  while IFS= read -r t; do add "$t"; done < <(bake_trees "${d%/}")
done
add "$LT/roster_bake"
# superseded session evidence (lead 2026-09-08, session 15): the s13 scratch bake and the pre-s13 walkabout set
add "$LT/s13/baked"
add "$LT/s13/walkabout_before"
add "$LT/s15/bake_cleanse-bake"   # the lander's fresh scratch bake of cleanse-bake (its cmp result is in s15/logs)
# session 16 (lead, 2026-09-08 night): the landers' per-branch scratch bakes and the lead's rebake-check pair — every cmp
# result is in s16/logs, and engine/assets_baked IS the same bake (two fresh bakes of main DIFF=0)
for b in terrain-ptch global-sound bake-determinism bake-determinism_2 two-tier-ktx2 main_1 main_2; do add "$LT/s16/bake_$b"; done
for b in test-scratch-dirs-2 bake-ogg two-tier-soldier far-scene-cull main_pre; do add "$LT/s17/bake_$b"; done   # s17: the lander's four family checks + the lead's pre-rebake scratch bake (cmp results in s17/logs)
add "$LT/bake-ogg/bake"; add "$LT/bake-ogg/bake2"   # item OGG's two 1.6 GB scratch bakes (landed a1ff75f)
for b in map-vo-streams water-fx-config main_pre; do add "$LT/s18/bake_$b"; done   # s18: the lander's two family checks + the lead's pre-rebake scratch bake (cmp results in s18/logs)
add "$LT/map-vo-streams/bake"; add "$LT/water-fx-config/bake"   # the loop sessions' full scratch bakes of VO and W1 (landed 2026-09-09 evening)
for d in before after_cull_off after_cull_on; do add "$LT/bf2-far-fade/$d"; done   # item 6's raw renders (~1 GB); its sheets stay
add "$LT/s19/bake_ron-twins"   # s19: the lander's DIFF=0 check of item 13 (cmp result in s19/logs)
for d in before after; do add "$LT/water-tint-fog/$d"; done; for d in before after; do add "$LT/walk-spawn-always-post/$d"; done   # raw renders of W2 / WS (sheets stay)
# Tier C — small stale
add "$ART/NEXT_SESSION_PROMPT.md"
for f in "$ART"/briefs/*.bak_*; do add "$f"; done
for f in "$LT"/*.log "$LT"/*.results; do add "$f"; done
add "$LT/ordnance"
while IFS= read -r d; do add "$d"; done < <(find "$LT/tools" -type d -name __pycache__ 2>/dev/null)

# what every running process has as its working directory or runs a binary from (read once; another user's is unreadable)
mapfile -t IN_USE < <(for p in /proc/[0-9]*; do readlink "$p/cwd" "$p/exe" 2>/dev/null || true; done | sort -u)
in_use() {  # $1 = a candidate's real path: is a process inside it?
  local u; for u in "${IN_USE[@]}"; do case "$u/" in "$1"/*) return 0 ;; esac; done; return 1
}

# never delete anything git tracks
SAFE=()
for c in "${CANDS[@]}"; do
  rel="${c#$ROOT/}"
  if git -C "$ROOT" ls-files --error-unmatch "$rel" >/dev/null 2>&1 || [ -n "$(git -C "$ROOT" ls-files "$rel" | head -1)" ]; then
    echo "SKIP (tracked): $rel"; continue
  fi
  case "$rel" in engine/assets_baked*|engine/goldens*|engine/target*|extracted/*|iso/*|community_mods/*|engine/artifacts/loop*|engine/artifacts/walkabout*|engine/artifacts/p3_walk*)
    echo "SKIP (protected): $rel"; continue ;; esac
  real="$(realpath -m -- "$c")"; prot=0
  for r in "${ROOTS_REAL[@]}"; do
    case "$real" in "$r"/engine/assets_baked*|"$r"/engine/goldens*|"$r"/engine/target*|"$r"/extracted*|"$r"/iso*|"$r"/community_mods*) prot=1 ;; esac
  done
  [ "$prot" = 0 ] || { echo "SKIP (resolves into a protected tree): $rel -> $real"; continue; }
  in_use "$real" && { echo "SKIP (in use by a running process): $rel"; continue; }
  SAFE+=("$c")
done

[ "${#SAFE[@]}" -gt 0 ] || { echo "nothing to delete"; exit 0; }
echo "== candidates =="
du -sh "${SAFE[@]}" 2>/dev/null | sort -h
total=$(du -sc "${SAFE[@]}" 2>/dev/null | tail -1 | awk '{print $1}')
echo "== total: $(( total / 1024 / 1024 )) GB in ${#SAFE[@]} paths =="
if [ "$YES" = 1 ]; then
  for c in "${SAFE[@]}"; do /bin/rm -rf -- "$c"; done
  echo "deleted ${#SAFE[@]} paths"
else
  echo "(dry run — pass --yes to delete)"
fi
