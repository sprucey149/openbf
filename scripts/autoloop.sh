#!/usr/bin/env bash
# autoloop.sh — run unattended Claude Code sessions on openbf in a loop (v2, 2026-09-25).
#
# Each iteration: scripts/loopctl.py picks and CLAIMS the next job (resume / land / planner / cleanse / audit / the
# best card) and its effort; this script appends that job's orientation to the standing prompt
# engine/artifacts/briefs/LOOP_PROMPT.md and runs ONE fresh headless session (`claude -p`) on it. The session does the
# job, writes a report + a journal paragraph, releases its claim and exits. State lives ON DISK: the cards
# (engine/artifacts/loop/cards/, BACKLOG.md generated from them), STATUS.md / INBOX.md, gates.toml, PLAN.md, git.
# The maintainer starts it; no session ever does.
#
#   scripts/autoloop.sh                # run until STOP (one lane)
#   LANES=2 scripts/autoloop.sh        # two lanes; lane 1 prefers build/cleanup cards, lane 2 re/capture/measure
#   LANE=2 scripts/autoloop.sh         # add ONE more lane beside a running loop
#   scripts/autoloop.sh --once         # one iteration, then exit
#   scripts/autoloop.sh --probe        # only the limit probe (haiku, 1 turn)
#
# Lanes never collide: loopctl serializes claims under engine/artifacts/loop/locks/ holding the lane's pid (swept once the
# pid is dead, or when it is this lane's own leftover); every ci / gpu / rebake / goldens / PCSX2 run holds flock
# engine/artifacts/loop/ci.lock; cards with overlapping `files:` are never in flight together; main moves only under the
# MAIN claim (scripts/land.py).
#
# Controls (repo root, checked between iterations and by sessions between commits):
#   touch STOP    → finish the current iteration cleanly, then exit
#   touch PAUSE   → wait (re-checked every PROBE_INTERVAL) until it is removed
#   engine/artifacts/loop/DRAINED → written by the planner (or by loopctl after two empty refills): the loop writes STOP and
#                   exits by itself, with the reason in the journal and STATUS.md. scripts/start_loop.sh clears both.
#
# Rate limits: no reset time is exposed, so a limit/overload result puts the lane in WAIT mode: a one-turn haiku probe every
# PROBE_INTERVAL until one succeeds. A session cut off mid-job leaves its card in-progress; loopctl resumes it next.
#
# Tunables (env): MODEL (opus), MAX_TURNS (400), ITER_TIMEOUT (4h), PERM_MODE (auto), PROBE_INTERVAL (20m),
# MAX_ITERS (0 = unbounded), CLAUDE_BIN (claude), EFFORT (empty = the job decides: re/capture/audit xhigh, planner high,
# the rest medium — the 2026-09-24 effort A/B; set a level to force it for every session), LANES (1), LANE_STAGGER (180 s),
# WORKFLOWS (1 — audit sessions and the `re` phase of card sessions get
# engine/artifacts/briefs/WORKFLOW_BLOCK.md appended: the maintainer's explicit opt-in to the Workflow tool for those jobs, 2026-10-07; set
# WORKFLOWS=0 to turn it off), WF_EFFORT (medium — the effort of the workflow's agents, not of the session).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODEL="${MODEL:-opus}"
MAX_TURNS="${MAX_TURNS:-400}"
ITER_TIMEOUT="${ITER_TIMEOUT:-4h}"
PERM_MODE="${PERM_MODE:-auto}"
PROBE_INTERVAL="${PROBE_INTERVAL:-20m}"
MAX_ITERS="${MAX_ITERS:-0}"
CLAUDE_BIN="${CLAUDE_BIN:-claude}"
EFFORT_OVERRIDE="${EFFORT:-}"   # empty = the job decides (loopctl.py); set EFFORT=<level> to force one for every session
LANES="${LANES:-1}"
LANE="${LANE:-}"
LANE_STAGGER="${LANE_STAGGER:-180}"
WORKFLOWS="${WORKFLOWS:-1}"
WF_EFFORT="${WF_EFFORT:-medium}"
WF_BLOCK="$ROOT/engine/artifacts/briefs/WORKFLOW_BLOCK.md"
PROMPT_FILE="$ROOT/engine/artifacts/briefs/LOOP_PROMPT.md"   # v2 (2026-09-25); v1 kept in briefs/v1/
LOOP_DIR="${OPENBF_LOOP_DIR:-$ROOT/engine/artifacts/loop}"   # override only for tests
RUNS_DIR="$LOOP_DIR/runs"
LOCKS_DIR="$LOOP_DIR/locks"
JOURNAL="$LOOP_DIR/JOURNAL.md"
mkdir -p "$RUNS_DIR" "$LOCKS_DIR"
[ -f "$JOURNAL" ] || printf '# openbf autoloop journal (append-only; newest at the bottom)\n\n' > "$JOURNAL"

# The project hook (scripts/claude_guard.sh) adds loop-only rules when this is set:
# merges into main must be --ff-only, a worktree never checks out main, and
# PLAN.md / HANDOFF-OPUS.md / analysis/ change only through the Edit/Write tools.
export OPENBF_LOOP=1
# Sessions open their own scopes (`systemd-run --user --scope`): the shim in scripts/loopbin keeps those in openbf.slice,
# the slice scripts/start_loop.sh starts the loop in and scripts/loop_cpu.py keeps off the maintainer's fast cores (2026-10-11).
case ":$PATH:" in *":$ROOT/scripts/loopbin:"*) ;; *) export PATH="$ROOT/scripts/loopbin:$PATH" ;; esac

# --- multi-lane launcher: LANES=N re-executes this script N times with LANE=1..N and waits.
if [ -z "$LANE" ] && [ "$LANES" -gt 1 ] 2>/dev/null; then
  pids=()
  for n in $(seq 1 "$LANES"); do
    OPENBF_LANES="$LANES" LANE="$n" LANES=1 "$0" "$@" >> "$LOOP_DIR/autoloop_lane$n.log" 2>&1 &
    pids+=("$!")
    printf '[autoloop %s] lane %s started (pid %s, log %s)\n' "$(date '+%F %T')" "$n" "$!" "$LOOP_DIR/autoloop_lane$n.log"
    [ "$n" -lt "$LANES" ] && sleep "$LANE_STAGGER"
  done
  wait "${pids[@]}"
  exit 0
fi
LANE="${LANE:-1}"
export OPENBF_LANE="$LANE" OPENBF_LANE_PID="$$" OPENBF_LANES="${OPENBF_LANES:-$LANES}"

log() { printf '[autoloop L%s %s] %s\n' "$LANE" "$(date '+%F %T')" "$*"; }

# Reads a field out of a `--output-format json` result file. Prints "" if absent.
field() { python3 -c '
import json,sys
try:
    d=json.load(open(sys.argv[1]))
except Exception:
    print(""); sys.exit()
v=d.get(sys.argv[2])
print("" if v is None else (json.dumps(v) if not isinstance(v,(str,int,float,bool)) else v))' "$1" "$2"; }

# A result counts as a LIMIT hit when it is an error whose text or status
# looks like a rate/usage limit or an overload. Claude Code documents no
# exact string for this, so the match is deliberately broad; a false positive
# only costs a probe every PROBE_INTERVAL.
is_limit_hit() {
  local f="$1"
  local err status text
  err="$(field "$f" is_error)"; status="$(field "$f" api_error_status)"; text="$(field "$f" result)"
  [ "$err" = "True" ] || [ "$err" = "true" ] || return 1
  case "$status" in 429|529|503) return 0;; esac
  echo "$text" | grep -qiE 'limit|rate|usage|quota|overload|too many|capacity|try again' && return 0
  return 1
}

probe() {
  local f="$RUNS_DIR/probe_$(date +%Y%m%d_%H%M%S)_lane$LANE.json"
  timeout 180 "$CLAUDE_BIN" -p "Reply with exactly the word OK and nothing else." \
    --model haiku --max-turns 1 --output-format json < /dev/null > "$f" 2>&1   # stdin closed: launched from inside a Claude session the CLI otherwise waits 3 s and prints a warning INTO the JSON (2026-09-08 late night)
  if [ "$(field "$f" is_error)" = "False" ] || [ "$(field "$f" is_error)" = "false" ]; then
    log "probe OK ($(field "$f" result))"; return 0
  fi
  log "probe failed: $(field "$f" result | head -c 200)"; return 1
}

wait_for_limit_reset() {
  log "LIMIT WAIT: probing every $PROBE_INTERVAL until a probe succeeds"
  while true; do
    sleep "$PROBE_INTERVAL"
    [ -f STOP ] && { log "STOP seen while waiting; exiting"; exit 0; }
    probe && return 0
  done
}

# Drop item claims whose lane is gone (a session that died without its wrap-up)
# and any claim carrying THIS lane's pid (our previous iteration's leftover).
sweep_stale_locks() {
  python3 "$ROOT/scripts/loopctl.py" sweep-claims --pid "$$"
}

if [ "${1:-}" = "--probe" ]; then probe; exit $?; fi


iter=0
while true; do
  if [ -f STOP ]; then log "STOP file present; exiting loop"; exit 0; fi
  while [ -f PAUSE ]; do log "PAUSE file present; sleeping $PROBE_INTERVAL"; sleep "$PROBE_INTERVAL"; done
  iter=$((iter+1))
  if [ "$MAX_ITERS" != "0" ] && [ "$iter" -gt "$MAX_ITERS" ]; then log "MAX_ITERS reached"; exit 0; fi
  sweep_stale_locks

  # The JOB is chosen and claimed by scripts/loopctl.py BEFORE the session starts (2026-09-25): it decides the
  # effort (re / capture / audit -> xhigh, planner -> high, the rest -> medium; EFFORT overrides everything if set by hand)
  # and its orientation is appended to the prompt, so the session starts on its task instead of reading the project.
  job_json="$(python3 "$ROOT/scripts/loopctl.py" next --lane "$LANE" --lanes "$OPENBF_LANES" --pid "$$")"
  job="$(printf '%s' "$job_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("job",""))' 2>/dev/null)"
  if [ -z "$job" ]; then
    # loopctl.py crashed or printed no JSON: a broken tool must stop the loop loudly, never spin (2026-09-25 pilot).
    log "loopctl.py next FAILED (output: ${job_json:-empty}) — stopping this lane; run it by hand to see the error"
    printf -- '- LANE %s STOPPED %s: scripts/loopctl.py next failed (output: %s)\n' "$LANE" "$(date '+%F %T')" "${job_json:-empty}" >> "$JOURNAL"
    exit 1
  fi
  if [ "$job" = "stop" ]; then
    # The loop is drained (nothing eligible left, or only work that needs the maintainer): stop ALL lanes instead of burning tokens.
    reason="$(printf '%s' "$job_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("reason",""))')"
    touch "$ROOT/STOP"
    log "LOOP STOPPED ITSELF: $reason"
    printf -- '- LOOP STOPPED ITSELF %s (lane %s): %s — wrote STOP; remove %s/DRAINED and STOP to run again\n' "$(date '+%F %T')" "$LANE" "$reason" "$LOOP_DIR" >> "$JOURNAL"
    exit 0
  fi
  if [ "$job" = "none" ]; then
    [ -f STOP ] && { log "STOP file present; exiting loop"; exit 0; }
    iter=$((iter-1))   # an idle wait is not a session: it must not use up MAX_ITERS
    log "no job to take ($job_json); sleeping $PROBE_INTERVAL"; sleep "$PROBE_INTERVAL"; continue
  fi
  cur_effort="$(printf '%s' "$job_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("effort","medium"))')"
  [ -n "${EFFORT_OVERRIDE:-}" ] && cur_effort="$EFFORT_OVERRIDE"
  effort_args=(--effort "$cur_effort")
  export OPENBF_EFFORT="$cur_effort" OPENBF_JOB_JSON="$job_json"

  stamp="$(date +%Y%m%d_%H%M%S)"
  out="$RUNS_DIR/run_${stamp}_lane$LANE.json"
  promptf="$RUNS_DIR/prompt_${stamp}_lane$LANE.md"
  { cat "$PROMPT_FILE"; printf '\n\n---\n\n'; python3 "$ROOT/scripts/loopctl.py" orient; } > "$promptf"
  # Workflows (trial, 2026-10-07): an audit, or the research (`re`) phase of a card, may use the Workflow tool — the block is
  # The maintainer's explicit opt-in the tool requires, and it is appended ONLY for those jobs.
  wf=0
  cur_phase="$(printf '%s' "$job_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("phase") or "")' 2>/dev/null)"
  if [ "$WORKFLOWS" != "0" ] && [ -f "$WF_BLOCK" ] && { [ "$job" = "audit" ] || { [ "$job" = "card" ] && [ "$cur_phase" = "re" ]; }; }; then
    wf=1
    { printf '\n\n---\n\n'; sed "s/@WF_EFFORT@/$WF_EFFORT/g" "$WF_BLOCK"; } >> "$promptf"
  fi
  log "iteration $iter: job=$job_json model=$MODEL effort=$cur_effort wf=$wf max_turns=$MAX_TURNS timeout=$ITER_TIMEOUT → $out"
  printf '\n## autoloop lane %s iteration %s started %s (model %s, effort %s, wf=%s) job %s\n' "$LANE" "$iter" "$(date '+%F %T')" "$MODEL" "$cur_effort" "$wf" "$job_json" >> "$JOURNAL"

  # The prompt (standing prompt + this session's orientation) goes in on stdin; the JSON result comes out on stdout.
  started="$(date +%s)"
  timeout "$ITER_TIMEOUT" "$CLAUDE_BIN" -p \
    --model "$MODEL" --max-turns "$MAX_TURNS" --permission-mode "$PERM_MODE" "${effort_args[@]}" \
    --output-format json < "$promptf" > "$out" 2> "${out%.json}.stderr"
  rc=$?

  err="$(field "$out" is_error)"; reason="$(field "$out" terminal_reason)"
  turns="$(field "$out" num_turns)"; cost="$(field "$out" total_cost_usd)"
  denials="$(field "$out" permission_denials)"
  log "iteration $iter done: rc=$rc is_error=$err terminal=$reason turns=$turns est_cost=\$$cost"
  printf -- '- ended %s (lane %s): rc=%s is_error=%s terminal=%s turns=%s est_cost=$%s result=%s\n' \
    "$(date '+%F %T')" "$LANE" "$rc" "$err" "$reason" "$turns" "$cost" "$out" >> "$JOURNAL"
  # A session that died (claim still held by this lane, or its card in-progress with no report since it started) gets a
  # `- DIED …` journal line naming its job, card, phase and claim, and the claim is freed (LDOC).
  pm="$(python3 "$ROOT/scripts/loopctl.py" postmortem --since "$started" --pid "$$" --lane "$LANE" 2>&1)" && log "postmortem: $pm"
  if [ -n "$denials" ] && [ "$denials" != "[]" ]; then
    log "permission denials in this run (the session was blocked on a tool): $denials"
    printf -- '  - PERMISSION DENIALS: %s\n' "$denials" >> "$JOURNAL"
  fi

  if is_limit_hit "$out"; then
    printf -- '  - LIMIT HIT; waiting for reset\n' >> "$JOURNAL"
    wait_for_limit_reset
  elif [ "$rc" -ne 0 ] && [ "$rc" -ne 124 ]; then
    # Any other failure: back off one probe interval so a broken state does
    # not spin, but keep going — the next session reads the journal.
    log "non-zero exit ($rc); backing off $PROBE_INTERVAL"; sleep "$PROBE_INTERVAL"
  fi

  [ "${1:-}" = "--once" ] && { log "--once: exiting after one iteration"; exit 0; }
done
