---
id: VDROP
title: Hover bots drive by the PS2's law (re-hover-ai.md) and ride where their graph ends short of the post: BF2 END1's bikes stop dying (PS2 0 crashes in 5 min; port 40 unowned losses a round), BF1 END1 matches the PS2's own 3 crashes a minute, and both Hoths get hover boards
milestone: M2
value: 5
phase: build
status: done
deps: WAI,HAIS
files: engine/crates/core/src/ai/ride.rs,engine/crates/core/src/ai/route.rs,engine/crates/core/src/vehicle/hover.rs
declares: sim; sweep; leak
needs:
effort:
branch: vdrop
resume: landed 99895d81 2026-10-03
created: 2026-10-02
alpha: yes
---
## Why
Card WAI (2026-10-02) lets a WALKER's driver route to the reachable hub nearest its post when its type's graph does not
reach the post's hub (`ai::route::post_hub`, `nearest`). The same rule for hovers was built and switched off: with it on,
BF2 END1's bots crashed 42 speeder bikes a round with a rider aboard against 7 on main (8 seeds, `veh` deaths 60 -> 340;
`lead_tmp/wai/build/sw8` against `sw8c`). The crash is the hover driver's own defect and is on main already: BF1 END1
loses 217-230 riders over 8 seeds (27-29 a round; `vehicles destroyed (killed / unowned): imp_hover_speederbike 1/33`).
Without the rule no hover is boarded on BF1 HOT1 or BF2 HOT1 (no post's hub is on a vehicle type's graph there).

## Evidence
- `lead_tmp/wai/build/sw8/seeds.md` (rule on for hovers) and `sw8c/seeds.md` (walkers only) against main's
  `lead_tmp/bowl/sw2_v1t/seeds.md`; `lead_tmp/wai/build/cmp.py <a> <b>` prints the per-map table.
- `round_to_outcome` (ROUND_VERBOSE=1) prints `vehicles destroyed (killed / unowned)` per class.
- PS2 side not measured: how often a PS2 bot crashes a speeder bike on Endor (`ps2-ai-battle-sampler` memory; the hover's
  AI FUN_00220528 calls the same steering FUN_002a54d8 with the hover yaw of re-walker-ai.md section 4, which is not the
  port's `steer_to`).

## HAIS note (2026-10-03, `analysis/re-hover-ai.md`, docs commit b073094, verifier 49 / 0 / 0)
- The hover AI `FUN_00220528` (BF1 `FUN_001eedc0`) is transcribed and replayed tick by tick against PS2 RAM: BF2
  Endor 22617 of 22621 ticks equal on throttle, strafe and turn, Kashyyyk 15815 of 15832, Geonosis 6491 of 6492; BF1
  Endor 24089 of 24222. Tools and data: `lead_tmp/hais/` (`hai_check.py`, `hai_check1.py`, `hai_loss.py`,
  `hai_rows.py`, `live_*.csv`, `*_end.p2s`).
- What the port's `steer_to` lacks against it: the (throttle, strafe) pair — a BF2 bike over 6.1° off its goal holds
  strafe ±1 and a throttle of 0.107 / tan(error), which is why the PS2's bikes ride at a median 12.9 m/s; the
  squared turn stick; the probe (a ray 1.5 × speed ahead; a hit moves the goal a quarter of that sideways and halves
  the throttle); the three-point unstick; arrival at ForwardSpeed / 2 (14 m for a BF2 bike).
- Two of BF1's 15 PS2 crash kills happened inside the unstick manoeuvre (3 s of full throttle with half turn).
- Found on the way, not traced: an EMPTY bike at rest loses a tenth of its health a second until it is gone (both
  games: 60 hp a second of 600, 8 of 80) — the PS2's 2 / 3 empty losses are that, and the port's "unowned" count
  should be read against it.

## Planner 2026-10-03 03:35 - value 4 -> 5
M2 had no value-5 card. This is M2's largest measured gap (BF2 END1: 40 bikes lost a round in the port, the PS2's bots
crash none in 5 minutes), HAIS's RE is hours old with 347 PS2 rows ready for the unit test (`lead_tmp/hais/ps2_rows.csv`),
and BF2 HOT1 (a showcase map) gets its hover boards from it. It is the hover driver (`ai/ride.rs`), not the foot
soldier: not held for BOTD. Undo: `loopctl.py set VDROP value=4`.
- The empty-bike decay (line "Found on the way" above; INBOX 2026-10-03 02:45): the PS2 loses an EMPTY bike at rest at a
  tenth of its health a second, both games, the writer not traced. Step 1's census must count the port's "unowned"
  losses in two rows - empty at rest, and everything else - and set only the second against the PS2's crash numbers.
  If the port has no such decay and the first row differs from the PS2's 3 empty bikes in 5 min (BF2 END1), write the
  finding here and file ONE small RE card (the writer of the health field at rest: `re_check.py writers`); do not
  build a decay from the observation alone.
- The maintainer's question (STATUS q17): should bots on BF1 bikes drive better than the PS2's (15 fatal crashes in 54 rides)?
  Default: match the PS2, as this card is written. An answer in INBOX changes the BF1 END1 target only.

## Build 2026-10-03 (lane 1; branch vdrop, commit 88394b1; `analysis/re-hover-ai.md` §12b)
Checks run (8 seeds 1-8, `lead_tmp/vdrop/hov.py <sweep dir>`; sweeps `lead_tmp/vdrop/sw_base` = main, `sw_law1` = the
law alone, `sw_law3` = as landed, `sw_near` = HOVER_NEAREST on):
- core test `ai::hover_drive::tests::the_sticks_are_the_ps2s_on_347_ticks`: `VDROP rows: 347 of 347 within 2e-3 on four
  sticks and the unstick state (worst 2.35e-5); 15 branches, 162 rows with a probe hit`.
- **BF2 END1** crashes with a rider (no killer, 10 m/s or more): 135 in 8 rounds (16.9 a round) -> **2 (0.2 a round)**;
  riders lost with no killer 231 -> 3; `veh` deaths 237 -> 23. Bar: at most 1 a round. MET.
- **BF1 END1** crashes with a rider per ride: 0.53 (187 / 351) -> **0.19 (74 / 384)**; PS2 0.28, bar 0.14-0.28. MET.
- Step 1's split: `round_to_outcome`'s new `hovers lost` line (rides; rider killed / unowned / crash; empty killed /
  at rest / moving). The port ALREADY has the empty hover's decay (`Sim::decay_pad_vehicle`): the "empty at rest"
  column (BF2 END1 49 in 8 rounds before) is that, so no RE card for it from this side.
- **Found: Endor's log trap.** 160 of the 171 unowned rider losses left after the law (sw_law1) were at two spots, the
  two `end_weap_treesmash` instances: their bone-parented log cylinders lay on the ground on the bikes' path. The
  PS2's bikes pass 0.3-0.8 m from both origins at 28-40 m/s. A `trap` class now gets no body (`game/src/assets.rs`);
  declared `leak` (both Endors: sites -5, new 0). Card TRAP for the real thing.
- **Step 3 NOT landed** (`ai::HOVER_NEAREST = false`): neither Hoth has a hover class (tauntauns, AT-STs, AT-ATs,
  snowspeeders), so "both Hoths get hover boards" cannot happen. With it on: rides 3753 -> 5366, crashes a ride the
  same, but tat1's `veh` deaths rise 5.6 a round (bar 5) and BF2 POL1 gets 22 floor deaths in 8 rounds (riders getting
  out over no floor). Card HNEAR.
- Law only, all maps: crashes 549 / 3561 rides -> 225 / 3753; `veh` deaths 919 -> 516; rounds finished 256 -> 255 of
  256 (BF2 GEO1 seed 7: a 3 v 1 endgame, the three in one command walker; 256 with HOVER_NEAREST on - seed noise).
