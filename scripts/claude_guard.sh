#!/usr/bin/env bash
# claude_guard.sh — Claude Code PreToolUse hook for Bash on openbf.
#
# Enforces PLAN.md §Guardrails and §Working mode at the HARNESS level so an
# unattended session cannot talk itself past them. Reads the hook's JSON on
# stdin, denies by printing a permissionDecision JSON (exit 0), otherwise
# prints nothing. Wired from .claude/settings.json.
#
# Denies force-pushes, blanket staging, writes to shared data, destructive data
# removal, unsafe GPU test commands and temporary Cargo targets.
# Maintainer-run jobs may push their owned branches under AGENTS.md.
set -u
input="$(cat)"
cmd="$(printf '%s' "$input" | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin); print(d.get("tool_input",{}).get("command",""))
except Exception:
    print("")')"
[ -z "$cmd" ] && exit 0

deny() {
  python3 -c 'import json,sys; print(json.dumps({"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":sys.argv[1]}}))' "$1"
  exit 0
}

# --- always ---
printf '%s' "$cmd" | grep -qE '(^|[;&| ])git([[:space:]]+-[Cc][[:space:]]+[^[:space:]]+)*[[:space:]]+push[^|;&]*[[:space:]](--force([^[:alnum:]-]|$)|--force-with-lease|--force-if-includes|-f([^[:alnum:]]|$)|[+])' && deny "openbf: never force-push."

printf '%s' "$cmd" | grep -qE '(^|[;&| ])git([[:space:]]+-[Cc][[:space:]]+[^[:space:]]+)*[[:space:]]+add[[:space:]]+(-A|--all|\.)([[:space:]]|$)' && deny "openbf: never git add -A / git add . — add by path (AGENTS.md)."
printf '%s' "$cmd" | grep -qE '(>|>>|\btee\b|\bcp\b|\bmv\b|\brm\b|\bsed -i\b|\btruncate\b|\bchmod\b|\bdd\b)[^|]*(^|[[:space:]/])(iso|extracted|community_mods|PC_swbf2_exe|PC_swbf1_exe|engine/assets_baked|engine/missions)/' && deny "openbf: shared source data and bake symlinks are read-only."
printf '%s' "$cmd" | grep -qE '\brm\b[^|;&]*-[a-zA-Z]*r[a-zA-Z]*[^|;&]*(assets_baked|engine/goldens|goldens/|engine/artifacts|artifacts/|community_mods)' && deny "openbf: no recursive delete of baked assets, goldens, artifacts or community mods — use a non-destructive route and ask."
printf '%s' "$cmd" | grep -qE 'cargo[[:space:]]+test[[:space:]]+-p[[:space:]]+render[^|;&]*--ignored' && ! printf '%s' "$cmd" | grep -q 'gpu_tests.sh' && deny "openbf: GPU referees run ONLY via scripts/gpu_tests.sh (parallel wgpu devices SIGSEGV)."
printf '%s' "$cmd" | grep -qE 'CARGO_TARGET_DIR=/tmp' && deny "openbf: never a cargo target dir under /tmp (user quota silently breaks the shell) — use engine/artifacts/lead_tmp/<branch>_target."

exit 0
