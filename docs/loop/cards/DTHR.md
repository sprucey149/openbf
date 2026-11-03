---
id: DTHR
title: Environmental death as the PS2's, built from re-kill-floor.md: landing damage (approach - threshold)^2 x scale instead of a kill at 25 m/s (was SLAND), BF2's death regions tested against the soldier's bounding sphere and on vehicles, and BF1's water-sink plane
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why (rewritten by the planner 2026-10-01; the original card text and SLAND's are kept below)
The maintainer's INBOX 2026-10-01 14:40 item 4 and 17:15: BF2's sphere-against-region death test and BF1's water-sink plane were
still wanted and had no card. The RE this card first asked for is done: `analysis/re-kill-floor.md` ([O] UNAUDITED,
2026-10-01) has both rules, and its section 4 lists what the port should do. KFLR (5b12384) built part of it: death
regions and the kill floor kill past the invincibility cheat; the kill floor itself and "a hard landing under the map
kills" are ours. What is left is three rules a player meets on every map with a ledge, a pit or a sea:
1. **Landing damage** (was card SLAND, merged): the port kills outright at `LETHAL_LANDING_SPEED` = 25 m/s (ours), so
   a 17 m drop kills and a 16 m drop costs nothing. The PS2 deals (approach - T)^2 x S through normal damage (cheat
   honoured), T = 10 + 3 n.y, S = 1, ODF overrides (heroes, jet and dark troopers scale 0). re-kill-floor 2.4 [V];
   worked table there: 8 m 16 HP, 15 m 80 HP, 25 m 181 HP.
2. **BF2's death region is touched by the soldier's bounding sphere** (BF1: the feet point), and BF2 tests layer-8/2
   vehicles too (re-kill-floor 1.3 [V]). The port tests the feet point for soldiers only, so on UTA1's sinkholes (a
   showcase map) a soldier dies about a sphere radius later than on the PS2, and a hover driven off MYG1's platform
   falls forever (note below).
3. **BF1's `water_sink_region`** (a plane y under the feet) kills like a death region (re-kill-floor 4.3). The port
   has no consumer. Which BF1 maps author one is not counted yet.

## Evidence
`analysis/re-kill-floor.md` sections 1.3, 1.4, 2.4, 4, 5. Open in its section 5, needed here: the soldier's
bounding-sphere centre offset and radius (who writes E+0x20: a RAM read of `s+0x20..0x2c` on KAM1 settles it). SLAND's
evidence and method are below. PS2, STHOP's `kam1_edge.csv`: a landing at vy -17.15 after an 8.4 m drop, god-moded, so
no damage number exists yet.

## What to do
1. **PS2 session (this phase, `measure`; PCSX2 README first, SW session, memory cap on new probes):**
   - the drop table WITHOUT god mode: `lead_tmp/sthop/ramp_bf2.py --airtest` holds and releases the player at
     6 / 10 / 16 m; read health E+0x170 before and after (BF2), and one BF1 height through `ps2live.py`;
   - the bounding sphere: `s+0x20..0x2c` for a standing, crouched and prone soldier on BF2 KAM1;
   - a census from the disc: which BF1 maps author `water_sink_region` and at what y; which BF2 maps' death regions a
     vehicle pad can reach.
   Write the numbers into this card and `re-kill-floor.md` section 5, then `phase=build`.
2. **Build** (one commit per rule): landing damage with `LETHAL_LANDING_SPEED` gone; BF2 sphere touch for soldiers and
   the region test for vehicles; BF1 water-sink. Each rule PROVISIONAL only where step 1 left it unmeasured.

## Acceptance
- Step 1: the PS2 drop table (3 heights) within 5 % of the law, or the law corrected; the sphere's centre and radius
  from RAM; the water-sink census (maps, y).
- Build: a core unit test per rule - the drop table's three heights give the PS2's damage within 1 HP; a BF2 soldier
  whose sphere touches a region dies while one a radius + 0.1 m away lives, and a BF1 soldier dies only with his feet
  inside; a hover inside a BF2 death region is destroyed; a BF1 soldier under a water-sink plane dies.
- `round_to_outcome`, 8 seeds: fall / region / floor deaths per map before and after in the report (`sweep_seeds.py`,
  main re-run first); no map's finish count drops.
- Declares at build: sim; sweep; arena (the leak census if it moves, read per class first).

## Notes
- Planner 2026-10-01 22:00: SLAND merged here (batch rule: value 3, same sim files, same RE doc); value 3 -> 4 because
  The maintainer asked for it twice and fall damage is felt on every map. DPIT (DEA1's pit falls, post-alpha) is a different
  question (whether bots walk off) and stays its own card.
- `re-kill-floor.md` is [O] UNAUDITED: read its cited lines yourself for the rules you build (`re_check.py fn / word`).

---
# The original DTHR card text

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 15; bindings BF1 001857a8 / BF2 0018d628; both PROVISIONAL in `core::sim`.

## Acceptance
Both rules ([O]) and a build card.

**Note (HOVX, 2026-09-28):** vehicles too — `hover_drive bf2_myg1 imp_hover_fightertank - 0` drives off a platform edge at 7 s (ground probe −300, NO_GROUND) and falls forever (y −4171 at 22 s); `bf2_uta1 … 0` likewise. Whatever the PS2 does to a vehicle below the world / in a death region, the port does nothing.

**Note (planner 2026-09-28):** see WADE (BF2 FEL1 / YAV1 soldiers in water): where the PS2 stands a soldier in water, and whether depth kills, may land here.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — UTA1's sinkholes: falling to your death (a showcase map). Value 2 → 3. Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

---
# SLAND's card text (merged 2026-10-01)

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

## Acceptance
- The law [V] in both games, with the constants' ELF words and the classes' ODF values.
- The PS2 drop table (3 heights) within 5 % of the law.
- Built: the port's landing deals that damage (the Fall death = health ≤ 0), `LETHAL_LANDING_SPEED` gone.
- The sweep's fall deaths per map reported before and after.
- Declares at build: sim; sweep; arena.

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

- Planner 2026-10-02 14:45: `files:` emptied while the phase is `measure` (step 1 is a PS2 session; the list held `sim/step.rs` and `combat.rs`, which GREN, JARC, PWUP and SQACK need). Restore when you set phase=build: `engine/crates/core/src/sim/state.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/sim/combat.rs,engine/crates/core/src/sim/bounds.rs,engine/crates/game/src/classdb.rs`.

- 2026-10-06 (card WADE, lane 2): the planner's 2026-09-28 question is answered - **depth does kill, by its own timer, not by a region**: 3.0 s with the surface at or over the aim origin (feet + 1.80 m), then 30 damage a second (`analysis/re-water.md` section 9.2 [O]; PS2 timelines to the death in both games). It is card **WDRN** (deps HWAT for the wet grid), not this card. One loose end for this card's PS2 session: at BF1 KAS2 (384, 256) a soldier standing 2.9 m under water went from 120 hp to 0 in ONE update, 9.5 s after being teleported there, with the water timer not expired (`lead_tmp/wade/ps2/out/first_run/bf1_kas2_s292.csv`) - a kill of unknown cause, most likely the battlefield boundary: at BF2 FEL1 (-121.7, 5.1, -118.6), dry ground outside the play area, the frame shows the red "Leaving Battlefield" text (`lead_tmp/wade/ps2/out/bf2_fel1_s105.png`) and he read dead right after it, about 5 s after the drop, with no damage row before.
