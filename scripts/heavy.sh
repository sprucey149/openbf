#!/usr/bin/env bash
# Run one heavy job (cargo build/test, bake, sweep, capture) under the machine's job locks.
#
#   scripts/heavy.sh [--gpu] <cmd> [args...]
#
# While the maintainer is playing (a `game` process outside the capped run-*.scope units),
# jobs take ci.lock exclusively: one at a time, as before. Otherwise a job holds ci.lock
# shared plus one of HEAVY_SLOTS slot locks, so up to HEAVY_SLOTS jobs run at once.
# Old-style `flock ci.lock ...` callers still work: they wait for every shared holder.
# A job already running when play starts keeps running (it is niced and CPU-weighted).
#
# Env: HEAVY_SLOTS (3), HEAVY_TIMEOUT seconds (3600), HEAVY_MEM (16G), HEAVY_LOCKS (lock dir).
# HEAVY_WARM=1 (and no CARGO_TARGET_DIR set): build in a persistent per-slot target,
# ~/.cache/openbf-warm/slot<N>, so a new worktree reuses compiled dependencies instead of
# building from scratch. Not for builds whose binary is run from engine/target (the game).
set -euo pipefail

# The whole body is read before it runs, so editing this file never breaks a job waiting in it.
main() {

    LOCKS=${HEAVY_LOCKS:-"$(cd "$(dirname "$0")/.." && pwd)/engine/artifacts/loop"}
SLOTS=${HEAVY_SLOTS:-3}
# A machine-wide override: a number in $LOCKS/slots wins over HEAVY_SLOTS, so the slot
# count can change while agents that pass HEAVY_SLOTS explicitly are already running.
if [ -r "$LOCKS/slots" ]; then s=$(tr -dc 0-9 < "$LOCKS/slots"); [ -n "$s" ] && SLOTS=$s; fi
GPU=0
if [ "${1:-}" = "--gpu" ]; then GPU=1; shift; fi
[ $# -gt 0 ] || { echo "usage: scripts/heavy.sh [--gpu] <cmd> [args...]" >&2; exit 2; }

playing() {
    local p
    for p in $(pgrep -x game || true); do
        grep -q 'run-.*\.scope' "/proc/$p/cgroup" 2>/dev/null || return 0
    done
    return 1
}

exec 9>"$LOCKS/ci.lock"
while :; do
    if playing; then
        flock 9
        break
    fi
    flock -s 9
    got=""
    for i in $(seq 1 "$SLOTS"); do
        exec 8>"$LOCKS/ci.slot$i"
        if flock -n 8; then got=$i; break; fi
        exec 8>&-
    done
    [ -n "$got" ] && break
    flock -u 9
    sleep 5
done

if [ "${HEAVY_WARM:-0}" = 1 ] && [ -z "${CARGO_TARGET_DIR:-}" ]; then
    export CARGO_TARGET_DIR="$HOME/.cache/openbf-warm/slot${got:-1}"
    mkdir -p "$CARGO_TARGET_DIR"
fi

if [ "$GPU" = 1 ]; then
    exec 7>"$LOCKS/gpu.lock"
    flock -s 7
fi

systemd-run --user --scope -q -p MemoryMax="${HEAVY_MEM:-16G}" -p MemorySwapMax=0 -p CPUWeight=30 \
    nice -n 12 timeout "${HEAVY_TIMEOUT:-3600}" "$@"
}
main "$@"
exit
