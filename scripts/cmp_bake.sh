#!/usr/bin/env bash
# Byte-compare a scratch bake tree (scripts/scratch_bake.sh's OUT) against the
# shipped engine/assets_baked, and say exactly which files differ.
#
#   scripts/cmp_bake.sh <scratch tree>              # vs $ROOT/engine/assets_baked
#   scripts/cmp_bake.sh <scratch tree> <other tree> # vs any other tree
#
# Env:  ROOT  repo or worktree root (default: this script's ..)
#
# Prints one line per file that is not identical, then the counts:
#   DIFF <path>       in both trees, bytes differ
#   ONLY_IN_A <path>  in the scratch tree only
#   ONLY_IN_B <path>  in the reference tree only
#   SAME=<n> DIFF=<n> ONLY_IN_A=<n> ONLY_IN_B=<n>
# NOTHING is filtered out — a scratch bake of a subset of the groups will
# therefore report the rest as ONLY_IN_B, which is expected and not a failure.
# One exception, FOLIAGE_LOST: a map baked in the scratch tree without the
# foliage.bin the reference holds. And two bakes of one bad extraction agree with
# each other, so the scratch tree is also held to the PS2's own counts
# (`bake check-foliage`, using $BAKE or this tree's release bake when built).
#
# Exit: 0 when DIFF=0 and no foliage is lost (files present in only one tree do
#       not fail it — say which groups you baked), 1 when any shared file differs
#       or foliage is lost, 2 on bad args.
#
# The bake is BYTE-DETERMINISTIC since queue item 16 landed (2026-09-08 night,
# `bake-determinism`): two runs of the SAME binary must give DIFF=0 over the
# whole tree — 21,227 files with the global VO bake (item 15) in. ANY DIFF is a
# regression or a designed change; there is no "noise" family any more.
# History, for reading older reports: before item 16, */sounds/names.ron (the
# case of a hash-colliding recovered name, bug M-B3) and audio/*.ogg (24 bytes,
# the Ogg serial ffmpeg drew at random and the page CRCs after it, bug M-B4)
# differed between runs — loop report 20260908_0037_cleanse-mungefs.md.
set -u

case "${1:-}" in
  -h|--help) sed -n '2,31p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
esac

ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
A="${1:-}"
B="${2:-$ROOT/engine/assets_baked}"
[ -n "$A" ] || { echo "cmp_bake: usage: cmp_bake.sh <scratch tree> [reference tree]" >&2; exit 2; }
[ -d "$A" ] || { echo "cmp_bake: not a directory: $A" >&2; exit 2; }
[ -d "$B" ] || { echo "cmp_bake: not a directory: $B" >&2; exit 2; }

same=0; diff=0; onlya=0
while IFS= read -r f; do
  if [ -f "$B/$f" ]; then
    if cmp -s "$A/$f" "$B/$f"; then
      same=$((same + 1))
    else
      diff=$((diff + 1)); echo "DIFF $f"
    fi
  else
    onlya=$((onlya + 1)); echo "ONLY_IN_A $f"
  fi
done < <(cd "$A" && find . -type f | sed 's|^\./||' | sort)

onlyb=0
while IFS= read -r f; do
  [ -f "$A/$f" ] || { onlyb=$((onlyb + 1)); echo "ONLY_IN_B $f"; }
done < <(cd "$B" && find . -type f | sed 's|^\./||' | sort)

lost=0
for f in "$B"/*/foliage.bin; do
  [ -f "$f" ] || continue
  map="$(basename "$(dirname "$f")")"
  if [ -f "$A/$map/manifest.ron" ] && [ ! -f "$A/$map/foliage.bin" ]; then
    lost=$((lost + 1)); echo "FOLIAGE_LOST $map"
  fi
done
BAKE="${BAKE:-$ROOT/engine/target/release/bake}"
if [ -x "$BAKE" ]; then
  "$BAKE" check-foliage "$A" || lost=$((lost + 1))
else
  echo "not checked against the PS2's foliage counts: no bake at $BAKE"
fi

echo "A=$A"
echo "B=$B"
echo "SAME=$same DIFF=$diff ONLY_IN_A=$onlya ONLY_IN_B=$onlyb"
[ "$diff" -eq 0 ] && [ "$lost" -eq 0 ]
