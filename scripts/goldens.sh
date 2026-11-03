#!/usr/bin/env bash
# Regenerate every committed golden PNG, then print what changed.
#
#   scripts/goldens.sh                    # regenerate every group + show the diff summary
#   scripts/goldens.sh --check            # only check (fails if anything differs)
#   scripts/goldens.sh [--check] hud shell   # only these groups (gates.py picks them from engine/goldens/deps.toml)
#
# Groups: tat1 (overview capture + golden_tat1), hud, shell, spawn, gallery (32 maps, 52 units, load screens),
# walkabout (walkabout_stats.ron). A new golden test joins here AND in engine/goldens/deps.toml.
#
# LOOK AT THE IMAGES before committing a regeneration — that is the whole
# point of them. `git diff --stat engine/goldens` lists what moved.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/engine"

CHECK=0
if [ "${1:-}" = "--check" ]; then CHECK=1; shift; fi
GROUPS_WANTED=("$@")
[ ${#GROUPS_WANTED[@]} -eq 0 ] && GROUPS_WANTED=(tat1 hud shell spawn gallery walkabout)

# The goldens are the STRICT-FAITHFUL referee: tier 1 is the PS2 disc's own
# texture, tier 2 the PC-install override item 11 bakes beside it. The tests
# also force it themselves (game/tests/common/mod.rs); this covers the game
# binary's own capture below, which does not go through them.
export OPENBF_TEXTURE_TIER=1

# walkabout_stats (item 7): per-panel mean/σ of a quarter-size walkabout; its reference engine/goldens/walkabout_stats.ron
# moves with any look change and is regenerated with the images (land.py counts it as declared when a golden image is).
GPU_TESTS=()
CAPTURE_TAT1=0
for g in "${GROUPS_WANTED[@]}"; do
  case "$g" in
    tat1)      GPU_TESTS+=(--test golden_tat1); CAPTURE_TAT1=1 ;;
    hud)       GPU_TESTS+=(--test golden_hud) ;;
    shell)     GPU_TESTS+=(--test golden_shell) ;;
    spawn)     GPU_TESTS+=(--test golden_spawn) ;;
    gallery)   GPU_TESTS+=(--test golden_gallery) ;;
    walkabout) GPU_TESTS+=(--test walkabout_stats) ;;
    *) echo "goldens.sh: unknown group '$g' (tat1 hud shell spawn gallery walkabout)" >&2; exit 2 ;;
  esac
done

if [ "$CHECK" = 1 ]; then
  cargo test -p game "${GPU_TESTS[@]}" -- --ignored --test-threads=1
  exit $?
fi

# The map/unit gallery and the single-frame goldens all honour UPDATE_GOLDENS;
# tat1_overview.png comes from the game binary's own capture path.
if [ "$CAPTURE_TAT1" = 1 ]; then
  cargo run -q -p game --release -- --map assets_baked/tat1 --headless \
    --capture-frame goldens/tat1_overview.png >/dev/null
fi
UPDATE_GOLDENS=1 cargo test -q -p game "${GPU_TESTS[@]}" -- --ignored --test-threads=1

echo
echo "== changed goldens =="
git -C "$ROOT" diff --stat -- engine/goldens || true
echo
echo "Review the PNGs (engine/goldens/{maps,units}/) before committing."
