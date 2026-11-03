---
id: HCORN
title: A BF1 speeder bike that starts a leg within its 20 m node radius of the leg's first detour corner skips that corner at once and drives into the barrier it went round (card HJAM's corner reach; AUDIT HJAM 2026-10-16): BF1 Tatooine leg 54 -> 55, barrier267's NE corner (-148, 44) - 63 of 72 such starts die within 2 s against the rock at (-150, 37.8), 1 of 56 before HJAM; BF1 Endor legs 4 -> 14 and 34 -> 38 the same. RE what the PS2 does at such a corner first, then fix and test the driver's radius input
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Build part 1 result (2026-10-21 23:28, lane 1; report `reports/20261021_2328_card_HCORN.md`)
Branch hcorn 350860a: `route::advance_route` re-plans a hover driver's corner walk from `next.from`. Two tests in
`hover_drive/tests.rs`; M5 (radius 0), M6 (radius x 0.5) fail both, M7 (rule off) fails the second (`lead_tmp/hcorn/mut/mut.txt`).
Built binary = prototype HCX=2 hash for hash on 24 of 24 rounds (tat1 / bf1_end1 s25-32, bf2_kas2 s1-8; HCX=0 differs 24 of 24).
Bars (ab.py re-run): tat1 rock deaths 41 -> 4 / 54 -> 6; corner-in-R starts dead in 2 s 0/118, 0/129; bf1_end1 crash a ride
0.255 / 0.255; near-corner 0.01 / 0.00 %. Parked: RTX lost, golden step not run.

## RE result (2026-10-21, lane 2) -> build part 1 (`analysis/re-hover-corners.md`)
HJAM's corner drop IS the PS2's (BF1 Endor: 389 of 395 corner exits at 2-20 m; all 67 hand-overs of a corner already
inside 20 m left after one tick) and it kills the PS2's own bikes (5 of its 15 Endor crash kills). What the port gets
wrong is WHERE the corner walk is planned: the PS2 plans a leg from the waypoint just reached (B1 `FUN_002509b0`, B2
`FUN_002c0220`; live 200 / 200 Endor legs and 54 / 54 tat1 legs), the port from where the bike stands - which is what
gives tat1's leg 54 -> 55 its deadly corner (-148, 44). Also at the hover's own radius (BF1 bike 2.44, corners 2.94 m
out) - split off to card HCRAD (it needs a bake change and halves Endor's crashes below the PS2's).
Prototype (`lead_tmp/hcorn/hcorn_x_prototype.patch`, switch HCX=2 = this card's rule; HCX=0 = main hash for hash):
tat1 rock deaths 41 -> 4 and 54 -> 6 (seeds 1-24 / 25-48); bf1_end1 crashes a ride 0.297 / 0.278 -> 0.255 / 0.255
(PS2 0.28); near-corner driving 0.22 -> 0.01 %; the other 12 hover maps within one block's noise (`ab_bars*.txt`).

## Build (part 1)
1. `route::advance_route`: when the next leg is planned on arrival at a hub (`next.via_arc` set, `next.from` = the
   waypoint reached), a HOVER driver's corner walk is searched again from `next.from` (at the unit's y), not from the
   unit, and `ref_dist` re-taken (the prototype's bit 2, `hcorn_x_prototype.patch`). Soldiers and walkers unchanged.
2. Tests: (a) a hover driver whose leg starts with its first corner inside ForwardSpeed / 2 moves past it in the same
   think and fails with the radius `drive` hands `advance_route` zeroed or halved (AUDIT HJAM's M5 / M6); (b) a leg
   planned on arrival gets the corners seen from the waypoint (a fixture where the unit -> waypoint line crosses a
   barrier and the waypoint -> waypoint line does not). Mutation-check both.
3. Bars (24 seeds; the instrument = the patch's HJE / HJD / HJC lines on a scratch copy, `lead_tmp/hcorn/tools/ab.py`):
   tat1 rider deaths within 3 m of barrier267 <= 10 (main 41 / 54); bf1_end1 crash per ride in 0.22-0.34; near-corner
   share <= 1 %; the 14 hover maps (`lead_tmp/hcorn/chain2.sh`'s list) crash per ride within noise of main.

## Why
AUDIT HJAM (2026-10-16, `engine/artifacts/lead_tmp/audit-hjam/EVIDENCE.md`). Card HJAM gave a hover's driver the PS2's
node radius at detour corners (ForwardSpeed x 0.5: 20 m for BF1's bike; both ELFs' `FUN_002a6318` / `FUN_00234db0` hover
branch, re-checked). It removed the corner circling (near-corner driving 14.8 % -> 1.4 % of hover driving time). But a
leg that STARTS with its first corner already inside 20 m (the hub before it is also reached at 20 m) skips the corner on
its first tick and the bike aims straight across the barrier the corner walks round. Traced, BF1 Tatooine (tat1) seed 2,
unit 0 on bike 12: hub 56 reached 18.9 m out at 37 m/s heading west, 11 m south of its leg; the next leg 54 -> 55 has one
corner (-148, 44), barrier267's NE corner (barrier at (-155.3, 39.1), half 4.2 x 4.8 m), 6.5 m away -> skipped at once;
the rider dies 5 ticks later at (-150.0, 37.8), the barrier's east face (`bb/tr_x0.err`, `bb/tat1/tat1_s2.json`).

tat1 is a showcase map (PLAN.md 130): about 2.5 bikes a round explode at that one rock in play.

## Evidence (all under engine/artifacts/lead_tmp/audit-hjam/)
- `ent/` (tat1 seeds 1-24, the landing's switch binary: HJAMX=3 = the parent byte for byte): leg 54 -> 55 starts carrying
  the corner, died within 2 s: parent 1/56, HJAM 63/72; the same leg without the corner 0/65 and 0/63. Starts and start
  speed equal (121 vs 135, median 35 m/s) - the rule makes the leg deadly, it does not send more bikes into it.
- `dth/`: 57 of tat1's 201 rider-with-vehicle deaths at (-148.0, 14.8, 37.5) over 24 seeds (2 there before); tat1 bike
  crashes a ride 0.028 -> 0.091 (rides 963 -> 981).
- `ent2/legs.py` (14 hover maps x 12 seeds): BF1 Endor leg 4 -> 14 corner starts 2/100 -> 26/88, leg 34 -> 38 5/98 -> 28/143;
  BF1 Yavin 17 -> 20 was already deadly before (49/135 -> 72/161). BF1 Endor's crash a ride 0.23 -> 0.30 overall
  (the PS2 0.28 = 15 of 54 rides, re-hover-ai 11), so the AVERAGE is the PS2's; the question is WHERE they crash.
- Mutations M5 / M6 (`mut/`): `hover_drive::drive` handing `advance_route` 0.0 or `arrive * 0.5` passes all 1,587 tests
  - the corner rule's (and VAI's hub reach's) input in its only caller is untested.

## RE first (the faithful answer is not known)
By the RE the PS2's follower also takes a non-last node within R (`FUN_002f1140` d^2 < R^2), so the PS2 may skip this
corner too. Open: (a) the corner's offset for a HOVER on the PS2 (the A* over barrier corners at r + 0.5 m - which r for a
hover?) against the port's (-148, 44) 2.4 m off the barrier's corner; (b) barrier267 `flags: 31` against the PS2 hover's
barrier mask (does a hover plan round it at all?); (c) whether the PS2's obstacle probe (1.5 x speed ray, re-hover-ai 2)
turns the bike off this rock - the port's probe did not; (d) the PS2's own BF1 Endor crash SPOTS (re-hover-ai 11's capture,
15 crashes in 5 min) against the port's legs 4 -> 14 and 34 -> 38.

## Acceptance
- tat1 leg 54 -> 55 corner starts dying within 2 s <= 5 % over 24 seeds (`ent2/legs.py` on the new binary) - or, if the
  RE shows the PS2 crashes there too, a PS2 capture of it and this card closed as faithful.
- BF1 Endor near-corner driving share stays <= 1 % (HJC census) and crash a ride inside the PS2's 0.28 +- 0.06.
- A test that drives `hover_drive::drive` past a corner inside the node radius and fails under M5 / M6 (the radius the
  driver hands in zeroed / halved).
