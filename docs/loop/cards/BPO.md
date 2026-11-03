---
id: BPO
title: A unit standing in a barrier's margin gets the PS2's push-out, not "ignore the barrier"
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
BPO. [TODO — ADDED 2026-09-25 (lane 1) by STK2's measurement; **[RE] FIRST** (job D, at max), then a build **[SWEEP]**; FREE today; files `core/src/nav.rs`, `core/src/ai/route.rs` (maybe `core/src/ai/brain.rs`) — shared with AQD, never build the two in parallel] **A unit standing in a barrier's margin gets the PS2's push-out, not "ignore the barrier"** — STK2's knots (seed 42, `lead_tmp/stk2-census/`): **bf2_pol1 unit 1** (Defend cp1, stuck 27,480 at (93.8, 13.1, −60.0) from tick ~32,480: HDL's same-level hint `hintnode159` (90.3, −59.9) sits 3.8 m from `pol1_prop_crate_stack_14`; the Defend capture-volume flap re-plans leg 32→35 (arc 29) from there, the straight leg passes through the stack, and barrier 81 is dropped by `NavGraph::leg_ignores` because the unit stands inside it grown by `DETOUR_RADIUS`) and, in HDL's round, **bf1_yav1 units 18/22** (Attack cp0, legs 17→57, within ±1.5 m of (103.6, −58.9, −196.6) / (111, −58.9, −182.8) from tick ~22,500 to 60,000, in the margins of barriers 125/560 and 556/559). The stuck window's recovery (`ai::brain`, "Re-locate and re-route once") re-plans the identical straight leg with the corner walk dropped, so it never ends. analysis/RE_LOG.md:178 has `… the sweep re-baselined with every moved row named; sim_gates + `--sim-hash 100` + both arenas verbatim. Files: `core/src/nav.rs`, `core/src/ai/route.rs`; RE under `lead_tmp/re/bpo/`, `analysis/re-barrier-pushout.md`.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).

`acceptance: needed` (planner 2026-09-30): no measurable acceptance yet (a parity delta, a leak-census count, a sim replay hash or a pixel test). Write one before the card is picked.
- 2026-10-07 planner: the bot design's avoidance has this rule as its own ('one that is in a barrier leaves by the nearest edge first', bot-design.md 4.7 item 1) and OBST (e9ce6db) left that part for a follow-up card (OBST2 in its commit message; not filed as I write). When OBST2 exists, read this card's question there and close this one into it rather than writing an acceptance here.
