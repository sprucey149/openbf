#!/usr/bin/env bash
# The deterministic sim referees, in one place (PLAN §B0). Every branch
# that touches core/game has to show these unchanged (or changed for a stated,
# designed reason) before it lands.
#
# What it measures — fixed-seed; walk/aim also require a capture adapter:
#   1. walk_demo   407 ticks of a recorded input log on TAT1 -> sim hash
#   2. aim_demo    120 ticks of the same rig                 -> sim hash
#      The two logs are pad input only, tracked in
#      engine/crates/game/tests/data/replays/ so a fresh worktree has them.
#   3. --aim-replay 72 scripted ticks (targets, friend, range, wall)
#   4. --sim-hash 600   600 ticks of the built-in fixture    -> sim hash
# With BASE set it also byte-compares the per-tick `motion.csv` each replay
# writes. The scripted aim_targets replay also records each tick's sim hash.
# The hash is the whole sim state and its FORMAT changes when state is
# added; motion.csv is the trajectory only, so "hash moved, motion.csv did
# not" in walk/aim is the signature of a format change with no behaviour change.
#
# Expected values as of 2026-09-08 evening (main fc077bb and later — the s15
# baselines, engine/artifacts/lead_tmp/s15/baselines/SUMMARY.txt):
#   walk_demo     bb860216c775e788
#   aim_demo      b489b1b3aa14084f
#   --sim-hash 600  fb19c1d2c811788b
#   motion.csv    baselines/motion_main/{walk,aim}/motion.csv  (408 / 121 lines)
# History (each a hash-FORMAT change with motion.csv byte-identical): before
# phase-c commit 1 (658c6af) 43d2dfd6ce28c862 / 242fcc93623533a9 /
# 20fd5f8a77fa37f1; after it (the s14 values) 8d9d661ebcdd5245 /
# c12fdeb33d06ce48 / 537c1c6235c1f47e; Phase C commits 2-5 (fc077bb) grew
# the AiState/Unit hash again -> the values above. BASE must be an ABSOLUTE path.
#
# Usage:  scripts/sim_gates.sh            # this repo, engine/target/release
#         ROOT=<worktree> TARGET=<cargo target dir> OUT=<dir> scripts/sim_gates.sh
#         BASE=<dir with walk, aim, aim_targets motion.csv files> scripts/sim_gates.sh
# Env:  ROOT   repo or worktree root      (default: this script's ..)
#       TARGET cargo target dir           (default: $ROOT/engine/target)
#       OUT    where to write the outputs (default: $TARGET/sim_gates)
#       BASE   motion.csv tree to compare (default: unset — no compare)
# Exit: 0 all four ran and (with BASE) every motion.csv matched; 1 otherwise.
#       TRACE=1 also checks byte-identical 600-tick fixture traces (SHASH).
# It builds nothing: `cargo build --release -p game` first.
set -u

case "${1:-}" in
  -h|--help)
    sed -n '2,35p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
esac

ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
TARGET="${TARGET:-$ROOT/engine/target}"
OUT="${OUT:-$TARGET/sim_gates}"
BASE="${BASE:-}"
GAME="$TARGET/release/game"

[ -x "$GAME" ] || { echo "sim_gates: no game binary at $GAME (cargo build --release -p game)" >&2; exit 1; }
mkdir -p "$OUT/motion/aim_targets" "$OUT/motion/walk" "$OUT/motion/aim" || exit 1
cd "$ROOT/engine" || exit 1
rc=0

replay() {  # <name> <input log>
  local name="$1" log="$2" e
  "$GAME" --map assets_baked/tat1 --headless --spawn -140,230 \
          --walk-replay "crates/game/tests/data/replays/$log" \
          --capture-series "100000,$OUT/motion/$name" > "$OUT/${name}_raw.txt" 2>&1
  e=$?
  if [ $e -ne 0 ]; then
    echo "$name: FAILED (exit $e) — see $OUT/${name}_raw.txt" >&2
    rc=1
    return
  fi
  # A run that exits 0 but prints no hash fails too (the old pipe through tee hid it).
  grep -E '^sim hash after' "$OUT/${name}_raw.txt" > "$OUT/$name.txt" || { echo "$name: no sim hash line" >&2; rc=1; }
  cat "$OUT/$name.txt"
}

# AIMG runs even when the map/GPU referees below are unavailable. The
# replay rejects an incorrect per-tick magnet before reporting its state hash.
if "$GAME" --aim-replay "$OUT/motion/aim_targets" > "$OUT/aim_targets_raw.txt" 2>&1; then
  grep -E '^sim hash after' "$OUT/aim_targets_raw.txt" > "$OUT/aim_targets.txt" || rc=1
  cat "$OUT/aim_targets.txt"
else
  echo "aim_targets: FAILED — see $OUT/aim_targets_raw.txt" >&2
  rc=1
fi

replay walk walk_demo.txt
replay aim  aim_demo.txt

if "$GAME" --sim-hash 600 > "$OUT/sim_hash_raw.txt" 2>&1; then
  echo "sim hash after 600 ticks: $(tail -1 "$OUT/sim_hash_raw.txt")" | tee "$OUT/sim_hash.txt"
else
  echo "--sim-hash 600: FAILED — see $OUT/sim_hash_raw.txt" >&2
  rc=1
fi

if [ -n "$BASE" ]; then
  for m in walk aim aim_targets; do
    if [ ! -f "$BASE/$m/motion.csv" ]; then
      echo "$m/motion.csv: NO BASE at $BASE/$m/motion.csv" >&2
      rc=1
    elif cmp "$BASE/$m/motion.csv" "$OUT/motion/$m/motion.csv"; then
      echo "$m/motion.csv identical to base"
    else
      echo "$m/motion.csv DIFFERS from base" >&2
      rc=1
    fi
  done
fi

# Opt-in: keep the ordinary, tracing-off referee workload unchanged.
if [ "${TRACE:-0}" = 1 ]; then
  if "$GAME" --sim-trace "$OUT/trace_a.txt" 600 &&
     "$GAME" --sim-trace "$OUT/trace_b.txt" 600 &&
     cmp "$OUT/trace_a.txt" "$OUT/trace_b.txt"; then
    echo "600-tick trace byte-identical"
  else
    echo "600-tick trace FAILED" >&2
    rc=1
  fi
fi

[ $rc -eq 0 ] && echo "sim gates: all green ($OUT)"
exit $rc
