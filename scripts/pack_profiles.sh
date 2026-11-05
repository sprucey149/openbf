#!/usr/bin/env bash
# scripts/pack_profiles.sh -- Fast-path multi-profile packaging using a single compiled release binary.
#
# Usage:
#   scripts/pack_profiles.sh [--profiles "ps2-only pc-only xbox-only mixed"] [--out-dir <dir>] [--rebake]
#
# Compiles `game` and `bake` once, then packages all requested profiles via `pack.py --game-bin`.
# Avoids spawning separate worktrees and avoids redundant Cargo compilation passes.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENGINE="$ROOT/engine"
PROFILES=("ps2-only" "xbox-only" "pc-only" "mixed")
INPUT_BASE="${OPENBF_INPUT_BASE:-$ROOT/inputs}"
FONT="$ROOT/extracted/bf2/lvl/DATA/_LVL_PS2/CORE.LVL"
OUT_DIR="${OPENBF_PACK_OUT:-$ROOT/dist/profiles}"
REBAKE_FLAG=""
CARGO_PREFIX=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --profiles)
            IFS=' ' read -r -a PROFILES <<< "$2"
            shift 2
            ;;
        --out-dir)
            OUT_DIR="$2"
            shift 2
            ;;
        --input-base)
            INPUT_BASE="$2"
            shift 2
            ;;
        --rebake)
            REBAKE_FLAG="--rebake"
            shift
            ;;
        -h|--help)
            echo "Usage: scripts/pack_profiles.sh [--profiles \"ps2-only pc-only xbox-only mixed\"] [--input-base <dir>] [--out-dir <dir>] [--rebake]"
            echo "Each profile reads <input-base>/<profile> (xbox-only reads <input-base>/xbox)."
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done


echo "=== Fast-Path Multi-Profile Pack ==="
echo "Root:     $ROOT"
echo "Profiles: ${PROFILES[*]}"
echo "Out Dir:  $OUT_DIR"
echo

mkdir -p "$OUT_DIR"

# Step 1: Single compile of release game and bake tools
echo "--- Step 1: Compiling release binaries (single pass) ---"
START_BUILD=$(date +%s)
"${CARGO_PREFIX[@]}" cargo build --manifest-path "$ENGINE/Cargo.toml" --release -p bake -p game
BUILD_TIME=$(( $(date +%s) - START_BUILD ))
echo "Rust compilation finished in ${BUILD_TIME}s."
echo

GAME_BIN="${CARGO_TARGET_DIR:-$ENGINE/target}/release/game"
if [[ ! -f "$GAME_BIN" ]]; then
    echo "Error: compiled game binary not found at $GAME_BIN" >&2
    exit 1
fi
# Every package must simulate the same game as the binary just built.
EXPECTED_HASH=$("$GAME_BIN" --sim-hash 100)
echo "Reference --sim-hash 100: $EXPECTED_HASH"

# Step 2: Package each profile using the shared binary
TOTAL_START=$(date +%s)
for profile in "${PROFILES[@]}"; do
    echo "--- Step 2: Packaging profile: $profile ---"
    P_START=$(date +%s)
    
    # Map profile name to input folder
    case "$profile" in
        ps2-only)  input_name="ps2-only" ;;
        xbox-only) input_name="xbox" ;;
        pc-only)   input_name="pc-only" ;;
        mixed)     input_name="mixed" ;;
        *)         input_name="$profile" ;;
    esac
    
    input_dir="$INPUT_BASE/$input_name"
    if [[ ! -d "$input_dir" ]]; then
        echo "Error: input dir $input_dir for profile $profile not found" >&2
        exit 1
    fi
    
    profile_out="$OUT_DIR/$profile"
    mkdir -p "$profile_out"
    
    pack_args=(
        python3 "$ROOT/scripts/pack.py"
        --iso-dir "$input_dir"
        --game-bin "$GAME_BIN"
        --out "$profile_out/openbf"
    )
    if [[ -f "$FONT" ]]; then
        pack_args+=(--menu-font-core "$FONT")
    fi
    if [[ -n "$REBAKE_FLAG" ]]; then
        pack_args+=("$REBAKE_FLAG")
    fi
    
    "${pack_args[@]}"
    
    # Smoke test verification
    echo "Verifying simulation hash for $profile..."
    sim_hash=$("$profile_out/openbf" --sim-hash 100)
    echo "  --sim-hash 100: $sim_hash"
    if [[ "$sim_hash" != "$EXPECTED_HASH" ]]; then
        echo "Error: simulation hash mismatch for $profile! Expected $EXPECTED_HASH, got $sim_hash" >&2
        exit 1
    fi
    
    P_TIME=$(( $(date +%s) - P_START ))
    echo "Profile $profile packaged and verified in ${P_TIME}s."
    echo
done

TOTAL_TIME=$(( $(date +%s) - TOTAL_START ))
echo "=== All ${#PROFILES[@]} profiles packaged and verified in ${TOTAL_TIME}s (build: ${BUILD_TIME}s). ==="
