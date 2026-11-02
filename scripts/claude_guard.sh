#!/usr/bin/env bash
# claude_guard.sh — Claude Code PreToolUse hook for Bash on openbf.
#
# Enforces PLAN.md §Guardrails and §Working mode at the HARNESS level so an
# unattended session cannot talk itself past them. Reads the hook's JSON on
# stdin, denies by printing a permissionDecision JSON (exit 0), otherwise
# prints nothing. Wired from .claude/settings.json.
#
# ALWAYS denied (every session, interactive or loop):
#   - git push (any form)                        — git is LOCAL ONLY (PLAN §Guardrails)
#   - writes/deletes under iso/ or extracted/     — read-only disc data
#   - rm -r on assets_baked / engine/goldens / engine/artifacts / community_mods
#   - git add -A / git add . / git add --all      — never blanket-add (HANDOFF §Rules)
#   - a bare `cargo test -p render -- --ignored`  — GPU referees run via scripts/gpu_tests.sh
#   - cargo target dirs under /tmp                — the /tmp user quota breaks the shell
# DENIED ONLY IN LOOP SESSIONS (OPENBF_LOOP=1, set by scripts/autoloop.sh). Since 2026-09-14 the loop IS
# the lead (the maintainer): its sessions land branches, rebake and regenerate goldens themselves
# (engine/artifacts/briefs/v1/OPUS_LOOP_PROMPT.md §3), so only these stay:
#   - a git merge without --ff-only               — main only ever fast-forwards
#   - git checkout main / git switch main          — main stays checked out at the repo root
#   - a shell WRITE to PLAN.md, HANDOFF-OPUS.md or analysis/ (a redirect targeting them, cp/mv/rm/sed -i/tee
#     naming them, a python/perl script that names them and opens for writing) — reads are fine, and the
#     Edit/Write tools are how a session changes them
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
printf '%s' "$cmd" | grep -qE '(^|[;&| ])git[[:space:]]+push([[:space:]]|$)' && deny "openbf: git is LOCAL ONLY — never push (PLAN §Guardrails)."
printf '%s' "$cmd" | grep -qE '(^|[;&| ])git[[:space:]]+add[[:space:]]+(-A|--all|\.)([[:space:]]|$)' && deny "openbf: never git add -A / git add . — add by path (HANDOFF §Rules)."
printf '%s' "$cmd" | grep -qE '(>|>>|\btee\b|\bcp\b|\bmv\b|\brm\b|\bsed -i\b|\btruncate\b|\bchmod\b|\bdd\b)[^|]*(^|[[:space:]/])(iso|extracted)/' && deny "openbf: iso/ and extracted/ are READ-ONLY disc data."
printf '%s' "$cmd" | grep -qE '\brm\b[^|;&]*-[a-zA-Z]*r[a-zA-Z]*[^|;&]*(assets_baked|engine/goldens|goldens/|engine/artifacts|artifacts/|community_mods)' && deny "openbf: no recursive delete of baked assets, goldens, artifacts or community mods — use a non-destructive route and ask."
printf '%s' "$cmd" | grep -qE 'cargo[[:space:]]+test[[:space:]]+-p[[:space:]]+render[^|;&]*--ignored' && ! printf '%s' "$cmd" | grep -q 'gpu_tests.sh' && deny "openbf: GPU referees run ONLY via scripts/gpu_tests.sh (parallel wgpu devices SIGSEGV)."
printf '%s' "$cmd" | grep -qE 'CARGO_TARGET_DIR=/tmp' && deny "openbf: never a cargo target dir under /tmp (user quota silently breaks the shell) — use engine/artifacts/lead_tmp/<branch>_target."

# --- loop sessions only ---
# A WRITE to one of the lead's files: a redirect whose target is the file, or
# a file-modifying command whose argument list names it. A mere MENTION of the
# name (`2>&1`, `NR>850`, prose inside a heredoc) is not a write — the
# shakedown run was blocked on three read-only greps and one JOURNAL.md edit.
LEADFILES='(PLAN\.md|HANDOFF-OPUS\.md|analysis/[^[:space:]]*)'
writes_leadfile() {
  printf '%s' "$cmd" | grep -qE "(^|[^0-9&])>{1,2}[[:space:]]*[^[:space:];&|]*${LEADFILES}" && return 0
  printf '%s' "$cmd" | grep -qE "(^|[;&|])[[:space:]]*(sudo[[:space:]]+)?(tee|sed[[:space:]]+-i[^[:space:]]*|cp|mv|rm|truncate)[[:space:]][^|;&]*${LEADFILES}" && return 0
  # python/perl one-liners and heredocs: deny only when the script both names
  # the file and opens something for writing.
  # … and names it as a PATH LITERAL of its own (a quoted string that IS the
  # path), not as a word inside prose — the shakedown's JOURNAL.md edit only
  # mentioned PLAN.md in a sentence.
  if printf '%s' "$cmd" | grep -qE '(^|[;&|])[[:space:]]*(python3?|perl)[[:space:]]'; then
    printf '%s' "$cmd" | grep -qE "['\"]([^'\"[:space:]]*/)?(PLAN\.md|HANDOFF-OPUS\.md|analysis/[^'\"[:space:]]+)['\"]" \
      && printf '%s' "$cmd" | grep -qE "open\([^)]*['\"](w|a)['\"]|write_text\(|\.write\(" && return 0
  fi
  return 1
}
if [ "${OPENBF_LOOP:-0}" = "1" ]; then
  # `git merge` (also behind -C <dir>) must carry --ff-only; `--abort` is harmless. `git merge-base` is not a merge.
  if printf '%s' "$cmd" | grep -qE '(^|[;&| ])git([[:space:]]+-[Cc][[:space:]]+[^[:space:]]+)*[[:space:]]+merge([[:space:]]|$)' \
     && ! printf '%s' "$cmd" | grep -qE -- '--ff-only|--abort'; then
    deny "openbf loop: main only ever fast-forwards — rebase the branch onto main in its worktree, then git -C <repo> merge --ff-only <branch>."
  fi
  printf '%s' "$cmd" | grep -qE '(^|[;&| ])git[[:space:]]+(checkout|switch)[[:space:]]+main([[:space:]]|$)' && deny "openbf loop: never check out main inside a worktree — main stays checked out at the repo root; land with git -C <repo> merge --ff-only <branch>."
  writes_leadfile && deny "openbf loop: change PLAN.md, HANDOFF-OPUS.md and analysis/ with the Edit/Write tools, never a shell write (a redirect or sed must not be able to clobber them)."
fi
exit 0
