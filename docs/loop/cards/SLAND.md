---
id: SLAND
title: Soldier landing damage is the PS2's law — (approach speed − CollisionThreshold)² × CollisionScale per class, from the collision response — not a kill at a PROVISIONAL 25 m/s (BF1 default threshold 10 + 3·(up·n): a flat landing hurts from 13 m/s, a ~4.7 m drop)
milestone: M3
value: 3
phase: re
status: done
deps:
files: engine/crates/core/src/sim/state.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/sim/combat.rs,engine/crates/game/src/classdb.rs
declares:
needs:
effort:
branch:
resume: MERGED into DTHR (planner 2026-10-01 22:00): same RE doc (re-kill-floor.md), same sim files, value 3 - one PS2 session and one build pay the gates once
created: 2026-09-30
---

## Why
Found by STHOP's verifier (`analysis/re-soldier-ground.md` §4). The port kills a soldier outright when he lands at
`LETHAL_LANDING_SPEED` = 25 m/s (PROVISIONAL, `core/src/sim/state.rs`). So a 17 m drop kills and a 16 m drop costs
nothing.

The PS2's collision response deals damage on every hard contact:
- damage = (approach − threshold)² × scale;
- threshold = T0 + T1·(up·n) + T2·(fwd·n), the class's `CollisionThreshold`;
- scale likewise from `CollisionScale`;
- a vehicle-contact multiplier.

That decides how many of KAM1's / DEA1's / BES1's drops kill, and what a player's 6 m jump costs.

## Evidence
- **BF1 `FUN_00202f38`** 114478–114540: class+0x1850/54/58 = the threshold (fnv 0x2cdb588a), +0x185c/60/64 = the
  scale (0x1a87bacf). [V, read by STHOP]
  - Defaults in `FUN_0020d238`: threshold (10, 3, 0) (121824–121839); scale (1, 0, 0) per `re-stance-roll.md` §3.5.
  - A flat landing therefore starts to hurt at 13 m/s, a ~4.7 m drop at g = 18.
- **BF2 `FUN_0023cfd0`** 155769–155866: class+0x7dc / +0x7e8, skipped on the recorded ground object (155798).
  [verifier; not re-read]
- **PS2, STHOP's `kam1_edge.csv`:** the player landed at vy −17.15 after an 8.4 m drop, god-moded (health pinned),
  so no damage number.

## What to find
- BF2's defaults and the per-class ODF values on both discs (`classdb`).
- What the damage is applied to (health, with which multiplier).
- Whether a moving ground object changes it.
- A PCSX2 drop table without god mode: `lead_tmp/sthop/ramp_bf2.py --airtest` holds and releases the player; add
  heights 6 / 10 / 16 m and read the health E+0x170 before and after.

## Notes
- **Steep faces.** For a contact with 0 ≤ n.y ≤ 0.6 the PS2 pushes the soldier out along the TRUE normal (it lifts
  him) and removes only horizontal velocity (BF1 114813–114830; BF2 156134–156396). The port's wall pass pushes
  horizontally only. Not carded; fold it into a wall-pass card if a leak or a stall points at it.
- **BF1 half rate.** BF1 AI soldiers that are unimportant (< 10 m/s, > 45 m from the camera, off screen) skip an
  update and run the next with 2·dt (FUN_0020aad0 119745–119772). Not ported; noted here only.

## Note from STHOP's build (2026-09-30, lane 1) - an acceptance line carried here
- **BF1 BES1's falls on the PS2 (`ps2live.py`) beside the port's** (from STFAL, then STHOP) was not measured; it is a
  landing-damage question and belongs to this card's PS2 session. The port after STHOP, 8 seeds: 454 fall deaths of
  1891 deaths (24 %; before: 494, 27 %) - `lead_tmp/sthop/sweep8_final/seeds.md`.
- BF1's ground rules went in code-only (x0.2 air control, the 0.7 / 1.0 s fall timer, stick x |N.y|, the
  0.4 * Acceleraton slope pull; `core::soldier::GroundRules::Bf1`). The same PS2 session can check them: sample
  s+0xbfd bit 1, s+0x960 (N) and the stance timer s+0xbec on a BES1 walkway edge (`audit-vwlk/wsample1.py` shape).
- With BF2's full air control a faller presses on slanted walls every tick; the leak census's push/tunnel rose
  781 -> 923 / 1441 -> 1641 (DAG1 (112.7, 40.3): 14 m/s down a 73 deg face). The wall pass's steep-face push (above)
  is the lead.

- Fable 2026-10-01: `analysis/re-kill-floor.md` re-read the landing law in both games (threshold (10, 3, 0), scale (1, 0, 0); heroes, jet and dark troopers scale 0; BF2's Fall-state drag caps the speed near 38.7 m/s) and worked the drop table: 8 m 16 HP, 15 m 80 HP, 25 m 181 HP. KFLR 5b12384 added a kill for a landing at >= 13 m/s UNDER THE MAP (ours); the 25 m/s lethal landing itself is unchanged and still this card's.
