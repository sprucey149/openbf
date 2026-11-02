---
id: WHY
title: A reason code on every bot behaviour change and every failed walk - kept per bot for the black box, counted per map by the census
milestone: M3
value: 4
phase: done
status: done
deps: MAIS
files: engine/crates/core/src/ai/why.rs; engine/crates/core/src/ai/behave/brain.rs; engine/crates/core/src/ai/mover.rs; engine/crates/core/src/ai/stall.rs; engine/crates/core/src/ai/state.rs; engine/crates/core/src/sim/state.rs; engine/crates/core/src/sim/step.rs; engine/crates/game/src/blackbox.rs; engine/crates/game/examples/blackbox_read.rs; engine/crates/game/examples/round_to_outcome.rs
declares: sim
needs:
effort:
branch: botup
resume:
created: 2026-10-28
---
## Why
`analysis/bot-design.md` 4.5 asks that every walk have an outcome, "Failed(why)", but the port recorded only the
outcome. A stalled bot's cause was inferred afterwards from geometry (the census verdicts `body` / `post` / `spot` /
`open` / `hull`). The modern-research report (`lead_tmp/botre/modern.md` A6, its recommendation 2) takes L4D's
"reason" on every transition: recorded at the moment of failure, it speeds up every later stuck card (ARCM first).

## Evidence
Sweep `round_to_outcome -- 60000 42`, `STUCK_CENSUS=200`, `lead_tmp/botre/runs/WHY/` (before = MAIS b64df60d,
`runs/MAIS/after_*`): STDOUT is byte-identical (every hash, 34/34 finished, stuck_max median 180, sum 5640): the
log is diagnostic only. The new census line over the 34 maps: 3,667 failed walks, of them `no_gain` 2,181 (the
way open, no nearer: jams and pushes), `round_spent` 394 (254 on BF1 Bespin 1 alone), `driving` 375, `body`
247, `off_graph` 193 (BF2 Endor 57, Polis Massa 50, Utapau 48), `wall` 108, `rim` 69 (Mygeeto 22), `gave_up`
60, `return_failed` 21, `boxed` 11, `ledge` 4, `pinned` 2, `recycled` 2. A headless black box (BF1 Bespin 1,
tick 4000) shows unit 8: `FAIL round_spent arc 150 in recover` every 2 s, `FAIL gave_up`, `recover:goal_changed`.

## Change
- `core::ai::why`: the `Why` code (behaviour reasons, then the failed-walk block), a `WhyLog` ring of the last 8
  rows per bot (`AiState::why`; tick, the behaviour, the code, the failed leg's nav arc, failed or not).
  Diagnostic only, like `AiState::behaviour`: never hashed, never read by a rule; no strings in the sim.
- The codes are the ones the code knows there. A behaviour change: the link of `think`'s chain that ran (off the
  level, grenade, pickup, sniper spot, guard spot, a fight by hit / seen / memory, claim, order, arrived; Travel by
  what ended: target lost, safe, recovered, claim ended; a new post from the commander is `goal_changed`). A
  failed progress window: the mover's books before the window's cure (`why::walk_failed`: a driver, the leg's
  straight source, a rim barrier, the going-round allowance spent, then the steering's pick: ledge, back = boxed,
  slide / fan = wall, round = body, else no gain). Then what came of it: the goal given up (Recover's ladder), the
  floor walk (`pinned`), the recycle; a lost bot's failed return window.
- `NavTick::fails` / `fail_arc` carry them to `think`'s wrapper (`why::log`) and to the write-back:
  `NavStats::fails` per code. The census prints `census <map>: failed walks N [wall .. recycled ..]`.
- Black box schema 9: `why` per unit slot (its log as at its last frame); `blackbox_read --unit <id>` prints it,
  and a troubled bot's line carries its last four.
