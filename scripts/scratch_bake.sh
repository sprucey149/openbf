#!/usr/bin/env bash
# Run scripts/rebake.sh's bake into a SCRATCH tree instead of engine/assets_baked,
# so a change to mungefs/bake can be gated on "the bake is byte-identical".
# Pair it with scripts/cmp_bake.sh, which does the comparing.
#
#   OUT=/path/to/scratch scripts/scratch_bake.sh            # every group
#   OUT=/path/to/scratch scripts/scratch_bake.sh tat1 side  # only those groups
#   BAKE=<binary> CARGO_TARGET_DIR=<dir> OUT=... scripts/scratch_bake.sh
#
# Env:  OUT   scratch output tree (REQUIRED; created if absent)
#       ROOT  repo or worktree root (default: this script's ..)
#       BAKE  bake binary (default: build it, $ROOT/engine/target/release/bake)
#
# A `sounds` run without `classes` links the shipped classes/ into OUT (the
# sound-names step reads them; see below).
#
# It REFUSES with exit 2 rather than write the shipped tree: OUT unset, OUT at
# or under engine/assets_baked, or engine/assets_baked at or under OUT.
#
# The bake commands are rebake.sh's OWN, not a copy: this script rewrites four
# lines of it (ROOT, OUT, BAKE, and clean() to a no-op so a scratch run can
# never delete anything) into a temp file and runs that. Each rewrite must
# match EXACTLY ONE line or the run is refused — so rebake.sh changing shape
# fails loudly here instead of silently baking something else. rebake.sh
# itself is never modified.
set -u

case "${1:-}" in
  -h|--help) sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
esac

ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
REBAKE="$ROOT/scripts/rebake.sh"
SHIPPED="$ROOT/engine/assets_baked"

if [ -z "${OUT:-}" ]; then
  echo "scratch_bake: OUT is unset — refusing (that would bake into $SHIPPED)" >&2
  exit 2
fi
out_abs=$(realpath -m "$OUT")
shipped_abs=$(realpath -m "$SHIPPED")
case "$out_abs" in
  "$shipped_abs"|"$shipped_abs"/*)
    echo "scratch_bake: OUT=$out_abs is at or under $shipped_abs — refusing" >&2
    exit 2 ;;
esac
case "$shipped_abs" in
  "$out_abs"/*)
    echo "scratch_bake: $shipped_abs is under OUT=$out_abs — refusing" >&2
    exit 2 ;;
esac
[ -f "$REBAKE" ] || { echo "scratch_bake: no $REBAKE" >&2; exit 2; }

GEN=$(mktemp -t scratch_bake.XXXXXX.sh) || exit 2
trap 'rm -f "$GEN"' EXIT

# One awk pass, four exact-line rewrites, each counted.
awk -v root="$ROOT" -v out="$out_abs" -v bake="${BAKE:-}" '
  $0 == "ROOT=\"$(cd \"$(dirname \"${BASH_SOURCE[0]}\")/..\" && { pwd -W 2>/dev/null || pwd; })\"" {
    print "ROOT=\"" root "\""; n_root++; next }
  $0 == "OUT=\"${REBAKE_OUT:-$ROOT/engine/assets_baked}\"" {
    print "OUT=\"" out "\""; n_out++; next }
  $0 == "BAKE=\"${CARGO_TARGET_DIR:-$ROOT/engine/target}/release/bake\"" {
    print (bake == "" ? $0 : "BAKE=\"" bake "\"")
    n_bake++; next }
  $0 == "clean() { local t; for t in \"$@\"; do rm -rf \"$t\"; done; }" {
    print "clean() { :; }  # scratch bake: never delete"; n_clean++; next }
  { print }
  END { if (n_root != 1 || n_out != 1 || n_bake != 1 || n_clean != 1) {
          printf("scratch_bake: rebake.sh no longer has the four expected lines (root=%d out=%d bake=%d clean=%d)\n",
                 n_root, n_out, n_bake, n_clean) > "/dev/stderr"
          exit 3 } }
' "$REBAKE" > "$GEN" || exit 2

if [ -n "${BAKE:-}" ]; then
  # A binary was supplied: drop rebake.sh's build so nothing lands in engine/target.
  grep -v '^cargo build -p bake --release$' "$GEN" > "$GEN.tmp" && mv "$GEN.tmp" "$GEN"
fi

# A link left by an earlier sounds-only run goes first: `bake classes` must never write through it into the shipped tree.
[ -L "$out_abs/classes" ] && rm -f "$out_abs/classes"
mkdir -p "$out_abs" "$out_abs/classes" || exit 2   # `bake classes` writes into classes/ without creating it (found by cleanse-bake, 2026-09-08)

# `sounds` without `classes`: rebake.sh's sound-names step reads $OUT/classes, and on the shipped tree a `sounds`
# rebake finds the classes already there. An empty scratch classes/ recovered 114 names on TAT1 against 464 and every
# */sounds/names.ron compared DIFF (card BAKEG). So the scratch run reads the shipped classes through a link, which
# cmp_bake.sh does not follow (they are not this run's output).
has() { local n="$1" g; shift; for g in "$@"; do [ "$g" = "$n" ] && return 0; done; return 1; }
if [ $# -gt 0 ] && has sounds "$@" && ! has classes "$@"; then
  if [ ! -d "$SHIPPED/classes" ]; then
    echo "scratch_bake: sounds without classes and no $SHIPPED/classes — names.ron will be smaller than a full bake's" >&2
  elif rmdir "$out_abs/classes" 2>/dev/null; then
    ln -s "$(realpath "$SHIPPED/classes")" "$out_abs/classes" || exit 2
    echo "scratch_bake: sounds without classes — classes/ is a link to $SHIPPED/classes"
  else
    echo "scratch_bake: sounds without classes — using the classes/ already baked in $out_abs"
  fi
fi
echo "scratch_bake: $REBAKE -> $out_abs (rebake.sh untouched; clean() disabled)"
bash "$GEN" "$@"
