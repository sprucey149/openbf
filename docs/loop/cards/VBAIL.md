---
id: VBAIL
title: Two ride rules, one session and one gates run (VBAIL + HNEAR): (1) bot pilots leave a hover or walker under 20 % health when their draw says so (PS2: 26 of 101 controllers; FUN_002a95a8 tells every AI occupant to get out); (2) a hover rides to the hub nearest its post when its graph ends short (ai::HOVER_NEAREST, built by VDROP and off) - BF2 POL1's riders get out over no floor, tat1's bikes crash
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
On the PS2 an AI driver whose vehicle falls under 20 % health may abandon it: the walker AI FUN_00262248 (B2:181508-181515)
and the hover AI FUN_00220528 (B2:135604-135610) both test health < 0.2 x max and (ai+0x5a0 & 3) == 0, then call
FUN_002a95a8(E), which sends every AI controller on the vehicle a get-out order (type 0x14), and return without steering.
ai+0x5a0 is an integer 0..100 drawn once when the controller is built (FUN_002b6c20, B2:238826): 26 of 101 values pass.
The port's bots ride every vehicle down.

## Evidence
analysis/re-walker-ai.md section 8 ([O]; verifier A3 / A4 SUPPORTED). Live BF2 HOT1: an AT-ST whose driver drew 83 kept
driving from 17.7 % health to its death (lead_tmp/wai/live_hot1.csv). The bail itself was read, never observed.

## Acceptance
- Core test: a bot pilot with a passing draw, its vehicle at 19 % health, asks to leave on its next think and so does a
  bot gunner; with a failing draw both stay; at 21 % nobody leaves.
- The draw comes from the sim's own entropy (SimRng / GameLcg), one per boarding; over 8 sweep seeds the share of AI
  drivers that left a vehicle under 20 % is within 15-37 % (26 / 101 = 25.7 %), printed by round_to_outcome.
- 8 seeds: no map's finish count drops.
- HAIS note (2026-10-03, `analysis/re-hover-ai.md` §2 H2, §10): the hover's bail-out is live in RAM. On BF2 Kashyyyk
  315 ticks had a hover under 20 % health with (ai+0x5a0 & 3) == 0; on 313 of them the AI wrote no stick (the four
  sticks kept the last values: the hover coasts on its last input while the order to get out is pending). BF1's
  hover AI FUN_001eedc0 has no health test at all. Data: `lead_tmp/hais/live_kas2.csv` (columns health, ai5a0).

## Planner 2026-10-03 03:35 - not merged with SQVEH
Both are M2 value 3 and edit `ai/ride.rs` (the batch rule's case), and SQVEH's EVERYONE OUT is the same "AI seats
leave". Not merged: SQVEH already carries four orders, a follower rule and a `resume:` chain, and this card's check
(the draw's share over 8 seeds) is its own. Whoever builds one of the two should use the same get-out path for both.

## Part 2 (was card HNEAR, merged by the planner 2026-10-03 07:15)
The batch rule: HNEAR and VBAIL are both M2, value 3, both edit `ai/ride.rs`, both declare `sim; sweep`, and both end
in the same place - a bot getting out of a hover, which on BF2 POL1 can be over no floor. Two commits, one gates run,
a sweep table per part. Do part 1 (the bail-out) first: it is the smaller one and its exit path is what part 2 fixes.

Why (HNEAR's text). Card VDROP built "a hover rides to the hub nearest its post when its graph ends short" and left
it off (`ai::HOVER_NEAREST = false`). With it on (8 seeds, `lead_tmp/vdrop/sw_near` against `sw_law3`): rides 3753 ->
5366 (BF2 POL1's combat speeder 0 -> 304, BF1 NAB2 188 -> 348), crashes a ride unchanged (0.06 -> 0.05), rounds
finished 255 -> 256 of 256, recycled 230 -> 190. Against it: tat1's `veh` deaths 16 -> 61 (5.6 a round; its bikes
crash 28 times in 450 rides against 8 in 297) and BF2 POL1 gets 22 floor deaths in 8 rounds (none before): riders
leaving the speeder at a drop hub with no floor under the exit point.

Evidence: `lead_tmp/vdrop/sw_near/seeds.md`, `sw_law3/seeds.md`; `python3 lead_tmp/vdrop/hov.py <dir> [map]`;
`lead_tmp/wai/build/cmp.py <a>/seeds.md <b>/seeds.md`. Neither Hoth has a hover class: nothing to gain there.

Acceptance, part 2:
1. BF2 POL1: find where the 22 fall (`HOVER_LOSS_LOG=1` and `SWEEP_DEATHS=1` with the const on) and fix the exit
   point or the drop hub's test; floor deaths 0 in 8 seeds. The same exit point serves part 1's bail-out: with both
   parts on, no bot that left a hover dies of the floor within 3 s on any map (8 seeds, printed by round_to_outcome).
2. tat1: `veh` deaths rise by at most 5 a round over 8 seeds against the const off.
3. Then `HOVER_NEAREST = true`: no map's finish count drops, no map's `veh` deaths rise by more than 5 a round.
If part 2's bars are not met in the session, land part 1 alone and leave the const off with `resume:` naming the bar.
Reverse the merge: `loopctl.py set HNEAR status=todo resume=` and restore this card from
`lead_tmp/planner_0715/VBAIL.md`.

## Cloud source checkpoint — 2026-10-24

Part 1 is implemented pending local validation, independently controlled by
`ai::VEHICLE_BAIL`. It draws once per successful AI hover/walker boarding using
SimRng; a passing pilot under 20% orders every AI seat out through the existing
exit path. Humans are excluded. The card's per-boarding lifetime differs from
the analysis's PS2 controller-construction lifetime and is labelled PROVISIONAL;
no new RE claim is made. The blended port uses this BF2 rule for both games.

Five synthetic fixtures pass, including both kinds, 19/20/21%, failing draws,
reversed seat/unit order, human controls, think timing, RNG/ride/hash lifecycle
and autonomous boarding state retention. Eight synthetic seeds give 210/808
passing draws (25.99%); this does not satisfy the actual-driver map-sweep bar.
Part 2 remains unchanged/off (`HOVER_NEAREST=false`); no floor-safety claim.

restore evidence/bakes/GPU, paired parent/null/Part1 8-seed all-map sweeps,
controlled 90-tick hover-exit/floor-death observer, then Part 2 POL1/tat1 fix and
its separate sweep bars plus full unchanged gates. The historical split backup
is absent; no earned Part 1 landing or historical split restoration is claimed.
Report: reports/20261024_1601_card_VBAIL.md; logs: lead_tmp/vbail_cloud/.
