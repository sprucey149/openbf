---
id: CLSE2
title: The rest of the PS2's aim error after CLSE, read and verified (re-soldier-ai 18) - to build: BF1 maps aim with NO error vector; on BF2 the error is the TARGET's own block (grown by its change of velocity, paid from a budget), with the stance factor (crouch 1.5, sprint / jump 2, roll 3), the model-box size term (0.48-1.02) and the weapon class's AIBubble keys (1.25 shotgun, 0.6 sniper rifle), and the direction tumbles
milestone: M3
value: 5
phase: build
status: done
deps: CLSE
files: engine/crates/core/src/ai/gunner_foot.rs,engine/crates/core/src/ai/gunner.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/weapon.rs,engine/crates/core/src/unit.rs,engine/crates/game/src/roster.rs
declares: sweep; arena; sim
needs:
effort:
branch: clse2
resume: landed d57ab7da 2026-10-12
created: 2026-10-09
alpha: yes
---
## Why
Card CLSE (77f4c70) put BF2's scaling of the error VECTOR into the port (x 0.3 against a bot, x (d / 20 + 0.5),
y >= 0, zero for a human's passenger). Its RE (`analysis/re-soldier-ai.md` 17.3, 17.5) found that the error BLOCK
under the vector is not what the port models, and left four things unbuilt. Each changes how often a bot hits the
player or another bot; bots are the maintainer's first concern.

## Evidence - the RE is DONE (2026-10-09, `analysis/re-soldier-ai.md` 18, commits 1232157 + 4c238b1)
Five verifiers, 33 claims: 32 supported, 1 partly (folded in); the second verifier supported all six absence /
twin claims. Scratch: `engine/artifacts/lead_tmp/clse2/` (README there). The rules, each with its section:
- **BF1 has NO error vector** (18.5): `FUN_00233200` -> `FUN_00232a70` aims at the target's posture point (feet +
  1.4 m standing, 0.6 crouched, 0.2 prone) + the weapon's lead. RAM: a BF1 bot's view ray passes a standing target
  at a flat 0.06 m at every range. The port puts BF2's whole radius (0.78-1.4 m) on it. **The referee (18.9):**
  hp per aimer-second at 0-10 / 10-20 / 20-30 / over 30 m, the PS2 27 / 28 / 19 / 13 (three captures pooled),
  the port's main 20.9 / 17.0 / 18.1 / 16.1 (Kashyyyk Docks) and 14.3 / 16.4 / 17.1 / 14.3 (Naboo Plains).
- **The block is the TARGET's own** (18.1, 18.6): knocked in on the running PS2 - with the targets' radius zeroed
  their shooters' miss fell to a flat 0.060 m, the shooters whose own radius was zeroed kept 0.29-0.46 m. err
  decays at blend(0.1, 1.1) a second (the skill of who shoots at the unit) and GROWS by 0.455 x |change of the
  unit's velocity, 3-D| / MaxSpeed between two thinks, paid from a budget (starts 0, + half the decay, at most 5);
  floor blend(1.6, 0.4), ceiling 2.0. A bot's block thinks every 5th update (6 Hz), the human's every frame.
  The port: one err per shooter, grown by the target's displacement, no budget.
- **The stance factor** (18.2): inside the cap, err x 1.5 for a crouched target, x 2 sprinting or jumping, x 3
  rolling (RAM: crouched rows 1.000 with it, 1.500 without, 4,819 rows). The "state 7 / 2" factors never apply
  to a soldier.
- **The size term** (18.3): radius x 2 x 0.25 x hypot(dx, dz) of the model's bind-pose box = the baked GLB's
  POSITION bounds (RAM = GLB to four decimals): 0.48 super battle droid, 0.63-0.70 eight models (the rebel soldier
  0.692), 0.93-1.02 the other 22 (`clse2/out/glb_box_bf2.txt`, `tools/glb_box.py`).
- **The weapon class's keys** (18.4): `AIBubbleSizeMultiplier` (the vector's factor: 1.25 `com_weap_inf_shotgun`,
  0.6 the sniper rifles and three building turrets, 0.85 bulldog, 0.4 `tur_weap_laser_auto`, 0.1
  `com_weap_inf_defenseturret`), `AIBubbleScaleDistDivider` (k = d / D + 0.5: 20; 100 or 50 on four turret classes),
  `AIBubbleScaleClamp` (k's cap: 10; 3.0 on those four). Children inherit by BASE (`clse2/out/bubble_bf2.txt`).
- **The direction** (18.1): the shooter's own unit vector = a quaternion applied to (0, 1, 0); every think the
  quaternion is turned 2 x dt radians about an axis that walks (axis = normalise(axis + U(-1, 1)^3)). The port
  draws a new uniform direction every think.
- **A fixture for a unit test**: `clse2/out/radius_rows_rest.csv`, 80 distinct RAM rows at rest (69 standing, 11
  crouched) that the formula reproduces to float precision (size, stance, err, vis, cap -> radius).
- Expect little from the BF2 terms inside 10 m (the vector there is already 0.2 m); the BF1 rule is the large one.

## Result (2026-10-12 build, LANDED main 8ba75a7 + d57ab7d; report `reports/20261012_1442_card_CLSE2.md`)
- LANDED, BF2's five rules: the block per unit (`unit::AimBlock`), the stance factor, the size term, the weapon
  class's three keys, the tumbling direction. NOT landed: BF1's no-error rule - built, measured, refused by the
  fidelity gate (contact20_pct all maps 11.15 -> 9.66, all of it the 15 BF1 maps: 10.87 -> 8.12) and filed as card
  CLSE2B (commit 8ba75a7 has it; d57ab7d puts BF1 back to main's model, byte for byte on the 15 BF1-rule maps).
- Acceptance: tests + 24 mutations each failing one (`lead_tmp/clse2/mut/summary2.txt`), the 80 RAM rows within 1e-4;
  256 / 256 rounds; kills a minute 33.0 -> 32.4 all maps (BF2-rule maps 32.5 -> 31.4, BF1's unchanged 33.4) - 1.8 %
  UNDER today's main, over the card's 30.6-31.7; BF2 rifle falls per shot inside 10 m: Kamino 0.34 / 0.33 -> 0.39 /
  0.32, Polis Massa 0.22 / 0.30 -> 0.23 / 0.28 (the PS2 0.57-0.88: not moved, as the RE expected); BF1's bins: not
  met, the rule is CLSE2B's.
- The audit's four notes: (1) the three asserts are in `bf2_scales_the_error_vector_by_target_and_range` (mutations
  M14-M16); (2) beyond 30 m: Kamino 22.1 -> 24.5 hp per aimer-second (the PS2 10.1), Polis Massa 21.5 -> 20.4 (7.0);
  deaths with an enemy inside 10 m on the two maps 38.8 -> 41.1 % (Kamino 18.0 -> 19.0, Polis Massa 57.6 -> 61.6);
  (4) the two flyer terms are in `error_vector`'s PROVISIONAL list.

- 2026-10-12 audit CLSE2 (lane 1; `lead_tmp/audit-clse2/README.md`, report `reports/20261012_2045_audit_CLSE2.md`,
  `analysis/re-soldier-ai.md` 21, docs 35709e1): **PARTLY**. Held: the five rules are the PS2's - the port's own
  `AimBlock::think` gives the PS2's err and budget on 20,979 of 21,272 live thinks (the human's 2,089 of 2,090), the
  radius with its speed term on 11,585 of 11,709 moving thinks; every number of the landing reproduces; all five
  rules off = the parent's rows on 256 of 256 rounds; the 24 session mutations each fail a test on main. Not held:
  (1) "the rest of it" is not the rest - a jump / roll / sprint start adds error through the budget on the PS2
  (card JINK); (2) a target in a vehicle or turret takes the vehicle's block on the PS2, the seated soldier's here
  (card VAIM); (3) "inside the seed noise" leaves out the far bin (+5 %, t +2.8 at the landing, t +4.7 on main);
  (4) 3 of the audit's 20 mutations pass every test and the 80-row RAM test kept only rows that already fitted
  (both on card JINK's step 3).
