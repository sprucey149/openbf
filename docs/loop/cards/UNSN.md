---
id: UNSN
title: A stalled bot no human can see is moved on to a hub past its failing link instead of recycled
milestone: M3
value: 5
phase: done
status: done
deps: COVR
files: engine/crates/core/src/ai/unseen.rs; engine/crates/core/src/ai/unseen/tests.rs; engine/crates/core/src/ai/behave/brain.rs; engine/crates/core/src/ai/state.rs; engine/crates/core/src/ai/why.rs; engine/crates/core/src/ai/stall.rs; engine/crates/core/src/ai.rs; engine/crates/core/src/sim/state.rs; engine/crates/core/src/sim/step.rs; engine/crates/game/src/blackbox.rs; engine/crates/game/src/main/headless.rs; engine/crates/game/examples/round_to_outcome.rs; analysis/bot-design.md
declares: sim
needs:
effort:
branch: botup
resume:
created: 2026-10-28
---
## Why
The last resort of card RCYC (`ai::stall`: 120 s without net progress, or 10 failed windows at one place with no
floor walk out) kills the bot as `Stalled`, and its team pays a ticket for a defect of ours. The user approved
(2026-10-28) moving such a bot on while no human can see it happen, and keeping every move counted as a defect.

## Evidence
Card ARCM's open item: BF1 Bespin 1's arc 150 stall near (-6, 99, -166) has no other way on the graph, so memory
cannot help it; it made 11 of the 19 recycles over six seeds (`runs/HYST/seeds`).

Measured 6 seeds (42-47) x 34 maps, `lead_tmp/botre/runs/UNSN/seeds/` (`base` = the COVR commit, its seed 42
byte-identical to HYST's `final_s42`, so seeds 43-47 are HYST's `final` files; `after` = this commit; the sweep has
no human, so every safe spot passes the sight test):

| | base | after |
|---|---|---|
| finished | 204/204 | 204/204 |
| recycles (`Stalled`) | 19 (BF1 Bespin 1: 11) | 6 (Bespin 1: 0) |
| relocations | - | 11 (Bespin 1 arc 150 area: 9, 7-29 m; BF2 Yavin 1: 2) |
| stuck_max per map: median / mean / max (ticks) | 120 / 158 / 360 | 120 / 158 / 360 |
| time in Recover (share of foot time) | 1.36 % (Bespin 1 7.16 %) | 1.35 % (Bespin 1 6.83 %) |
| unexplained standing (stand20, foot-s) | 8,389 | 8,409 |
| round ticks summed | 2,771,478 | 2,768,638 |

The six recycles left are legs whose far hub is beyond the 60 m reach: BF1 Yavin 1 / BF2 Yavin 1 hub 18 at
(126, -72, -42), legs 18->81 / 18->79 / 18->57 (62-112 m, four of them), BF1 Hoth 1 leg 4->19 (89 m), BF1 Geonosis 1
leg 2->14 (69 m).

## Change
`core::ai::unseen` (D23, OURS): at the end of `think`, a tick whose give-up says `NavTick::stalled` (not the
off-level `recycle`) asks `unseen::spot`: for a bot on foot with a route, the far end of its leg (past the failing
link), then up to two further hubs toward its goal (the table's hop, else the fewest arcs); the first hub within
`REACH` = 60 m (and over 3 m away) with a spot - its centre on its resolved floor, else 8 points at 1.5 m and 3 m
inside its radius - whose floor is the hub's (1 m), on the level, out of the death regions, clear of every body
(`Sim::soldier_clear`) and of every soldier (1.5 m XZ, 2.5 m height). Neither the bot nor the spot may be
`seen_by_human`: a living player with a clear `perception::los` (the bolts' ray) from his eye or from 3 m behind
it (his camera) to the shins, chest or head, within `NEAR_SEEN` = 15 m whichever way he looks, beyond it inside
65 deg of his look. With a spot, `NavTick::relocate` carries it, the bot's walk starts again (no leg, no stall
watch, no lost spell, a fresh mover keeping the post it gave up; ARCM's closed arcs kept) and `Sim::step`'s
write-back moves it in unit order (two on one spot in a tick: the later is recycled) with an `Event::Relocated`.
Without one, the recycle as before.

Counted: WHY code `relocated` (a failed-walk code after `recycled`; the census's `failed walks [..]` column, the
bot's log), the sweep's `relocate <key>: ... from (x,y,z) to (x,y,z) locus Hh D m` list beside the recycle list,
the black box's `Relocate` event row (`v` = where it stood), the headless events CSV's `relocated`.

## Deviation
D23 (`analysis/bot-design.md` 9): the PS2 moves no stuck bot; ours is moved where no human sees it, else recycled.

## Open
- The four Yavin hub-18 recycles and the Hoth / Geonosis ones: legs longer than `REACH`; a spot on the arc itself
  (past what stopped the bot) would cover them, or the arcs need a map fix.
- An off-level bot's give-up (card BNDS, `DeathCause::Lost`) and a shut-in bot's (VMESH) still kill: a move onto the
  level would need the same test plus a floor-reach check.
- The sweep has no human, so the sight test is measured only by the unit tests; a slot-0 human sweep would show
  how often a watched bot keeps the recycle.
