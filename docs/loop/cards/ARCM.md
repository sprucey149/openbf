---
id: ARCM
title: Failed-link memory (a bot's own for 60 s, its team's for the round) and a back-and-forth detector that fails the progress window early
milestone: M3
value: 5
phase: done
status: done
deps: WHY
files: engine/crates/core/src/ai/arcm.rs; engine/crates/core/src/ai/arcm/tests.rs; engine/crates/core/src/ai/mover.rs; engine/crates/core/src/ai/route.rs; engine/crates/core/src/nav.rs; engine/crates/core/src/ai/state.rs; engine/crates/core/src/sim/state.rs; engine/crates/core/src/sim/step.rs; engine/crates/core/src/ai/hover_drive.rs; engine/crates/core/src/ai/walker_drive.rs; engine/crates/game/src/unit_probe.rs; engine/crates/game/examples/round_to_outcome.rs; analysis/bot-design.md
declares: sim
needs:
effort:
branch: botup
resume:
created: 2026-10-28
---
## Why
Recover's first rung (`analysis/bot-design.md` 6.3) re-plans from the hub the bot stands in, and nothing stopped it
drawing the arc that had just failed: the authored table offers a second hop at only 0-2 % of entries. Nothing was
shared across lives or the team. And the progress window (0.3 m in 2 s) passes a bot shoved to and fro that nets a
little now and then. `lead_tmp/botre/modern.md` A1 (Pogamut's failed-link report, Halo 2's failure memory) and A2
(Pogamut's distance detector), its recommendations 1 and 3.

## Evidence
Card WHY's census (`runs/WHY`): 3,667 failed walks on seed 42, `round_spent` 394 (254 of them BF1 Bespin 1, arc 150).
Measured 6 seeds (42-47) x 34 maps, `lead_tmp/botre/runs/ARCM/seeds/` (`base` = WHY's behaviour, its seed 42
byte-identical to `runs/WHY`; `final` = this commit; the other prefixes are the variants below):

| | base | final |
|---|---|---|
| finished | 204/204 | 204/204 |
| stuck_max per map: median / mean / max (ticks) | 180 / 169 / 600 | 120 / 161 / 360 |
| time in Recover (share of foot time) | 1.52 % | 1.47 % (without BF1 Bespin 1: 1.26 -> 1.11 %) |
| recycles | 25 | 19 |
| unexplained standing (stand20, foot-s) | 10,124 | 9,164 |
| round ticks summed | 2,776,253 | 2,762,621 |
| arcs closed / team-shunned / legs drawn round / kept (no other way) | - | 1,904 / 90 / 2,423 / 2,566 |
| back-and-forth fires | - | 186 |

Seed 42 alone (`runs/ARCM/before_*`, `after_*`): 34/34; stuck_max median 180 -> 180, sum 5640 -> 5820; Recover 1.30
-> 1.61 % and recycles 2 -> 4, both from BF1 Bespin 1 (Recover 6.3 -> 10.2 %: its arc 150 has no other way, 541
legs kept it; over the 6 seeds Bespin 1 runs 7.9 -> 9.0 %, noise of 5.1-13.6 % per seed). Failed walks 3,667 ->
4,006 (`round_spent` 394 -> 761, 588 on Bespin 1; `oscillation` 33).

Variants measured on the way (same 6 seeds): memory alone 1.44 % / 19 recycles; the detector alone 1.40 % / 24;
the first full build 1.52 % / **55 recycles** - 32 on BF1 Rhen Var 1, bots whose detour round a shunned arc went
back to the table's way at the next hub and round in loops (fixed: the hub-by-hub detour below, then 1.45 % / 33);
a 0.75 m turn 1.45 % / 33 against 1.0 m 1.34 % / 23; that build failed the OBST guarantee (`a long wall is rounded
on one side`: the detector fired along the wall and flipped the side), so the detector waits while a bot goes
round a body under its allowance (`final`).

## Change
`core::ai::arcm` (D20, OURS):
- `ArcmState::closed` (per bot, 3 slots, hashed): the progress window (`mover::stuck_window`) closes the leg's arc
  to the bot for `ARC_CLOSED` = 60 s when it fails with an earlier failure since the walk last got anywhere
  (`MoverState::recover > 0`) or the detector fired; on foot only.
- `Sim::arc_fails` (per team, per arc, for the round, hashed once written): each closing counts in the write-back,
  not twice running by the same bot; at `ARC_TEAM_FAILS` = 3 the team shuns the arc.
- `route::plan_leg`: for a bot with a closed or shunned arc, the whole choice (`choose_hop`, today's table / BFS /
  any-other-arc order) runs with those arcs treated as unwalkable, the BFS round them
  (`NavGraph::route_bfs_avoiding`); a hop rejected only for being shunned gets no random "any other arc"; with
  nothing found the choice runs again without them (never stranded; `shun_kept`). A leg drawn round one is
  `RouteSource::Shun` (black box `src` `r`), and from there the bot walks the fewest-arcs way without them hub by hub
  to the same goal.
- `Osc` (the back-and-forth detector, hashed with the rest): on an arc leg of a bot nothing excuses from the walk
  and not going round a body under its allowance, three turns of over `OSC_AMP` = 1 m between closing on and opening
  from the window's waypoint inside `OSC_SPAN` = 5 s, with no new ground (1 m nearer than ever), fail the window at
  once (`Why::Oscillation`): the window's cure, the Recover count, and the arc closed.
- Census: `failed walks ... ; arcs closed N shunned N legs round N kept N`.

## Deviation
D20 (`analysis/bot-design.md` 9): the PS2 retries (BF2) or stalls on (BF1) a failed walk's arc and its stuck test
is a clock; we remember the link per bot and per team and detect back-and-forth.

## Open
BF1 Bespin 1's arc 150 stall (units near (-6, 99, -166), `round_spent` every 2 s, then `gave_up`) has no other way
on the graph: memory cannot help it; it is a spot for card UNSN or a map annotation (modern.md A4).
