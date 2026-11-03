---
id: KNOCK2
title: Thrown bodies move as on the PS2 (AUDIT KNOCK PARTLY): BF2's tumble / get-up / corpse integrator (FUN_00240bf8: -2.5 / -10 x the velocity along the body rows, no floor test; the corpse brakes in the air and slides on slopes), horizontal speed^2 in the landing / tumble / get-up tests (both games), the corpse draw follows the sim body and a respawn keeps a flying body's item drop, and tests for the 19 live survivors of the audit's mutations (no foot death drops its item, corpses through walls, firing while thrown)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why (AUDIT KNOCK, 2026-10-21: PARTLY)
The landing's BF1 rules hold against the PS2: the push and its skip set, the apply, the thrown flight, the lock,
the kick, the late death and the corpse (two RE verifiers on 17 claims, none refuted; every sweep number reproduced
to the digit; the all-off switch build equals the parent on 256 of 256 rounds). What does not hold: in BF2 bodies
slide by the wrong law; three speed tests use 3-D speed where the PS2 uses horizontal speed; the corpse is still
drawn on its own 6 s clock (`re-knockback.md` §8 asked for the draw to follow the sim body); a respawn deletes a body
that is still flying, and its item drop with it. And 19 of the audit's 43 mutations pass every test. Two of them:
"no soldier killed on foot ever drops his item" (K28) and "corpses slide through walls" (K31).

## Evidence
1. **BF2 tumble (10) / get-up (0xb)**: B2 FUN_00240bf8 (B2:160735-160812) hands the air integrator FUN_00248560 an
   acceleration of -2.5 x (v . body row) for the tumble and -10 x (v . body row) for the get-up, plus the stick x
   ControlSpeed row 7. There is no floor test, and gravity applies off the floor. The port (`soldier.rs` knock_slide)
   gives BF2 BF1's linear brake, Acceleration x 0.6 x 0.2 / 0.4 x slope_control. Both verifiers gave V14 PARTLY with
   port_matches no, and the auditor read the lines.
   **BF2 corpse (0xf)**: FUN_002480a0 (B2:162931-163191) applies the min(t,1)^2 x 100 clamp on AND off the floor, and
   on the floor it adds the slope's gravity shift (V1; V2 confirms the air part). The port brakes only on the
   ground and has no slide.
   How often this is reached: every BF2 blast-killed body. `lead_tmp/audit-knock/sw/obsl0`, 136 BF2 rounds: 3808
   pushes, 3233 tumbles, 3596 get-ups.
2. **Horizontal speed^2**: B1 0x207aa8 / 0x207e40 / 0x207f14 = `vmul.xz` + `vaddz.x` (x^2 + z^2). These are the
   thrown landing, the tumble end and the get-up end (V2; the auditor decoded the words). BF2's sites are 0x243488 /
   0x244278 / 0x2448e8 / 0x244a8c (V2, not re-decoded). The port's `soldier.rs` landing test (`sp2` in the Thrown
   branch) and knock_slide use the 3-D `vel.length_squared()`. Effect: a body tumbling off a ledge never gets under
   4 m/s in the port.
3. **Corpse draw**: `game/src/soldier_vis.rs` uses CORPSE_SECS 6.0 counted from the death and ignores `Unit::body`.
   931 bodies in 256 rounds were still moving in the sim after the draw had hidden them (C7 in
   `lead_tmp/audit-knock/wf/results.json`).
4. **Respawn cuts a body**: `sim/spawn.rs` respawn_slot overwrites the unit. 14,408 of 52,969 bodies ended at a
   respawn (mostly exactly 4.00 s after the kill). 133 of them were still Thrown / Tumble / GetUp, and their item drop
   (`drop_due`) is lost. The PS2's rule is [H]: does a respawn wait for the soldier object, which is released 4 s into
   the dead state? Read it before choosing a fix.
5. **Off-map bodies**: the kill floor and the death regions test only living units (`sim/step.rs`). 77 bodies fell
   more than 50 m under a map (bf1_bes1 to y 0, bf2_dea1 / bes2 / myg1 / rhn1 to y -300) and dropped their items there.
   The PS2's rule here is not read.
6. **Smaller points from the verifiers' extras** (`wf/results.json` V1 / V2):
   - BF2 throws at scale > 5 strictly, where the port uses >= 5, and picks its airborne arm by state, not by the
     grounded bit (V13).
   - The PS2 picks the flying / tumble clip ONCE, at the state change, from the per-update displacement of the body
     matrix. The port re-picks it every frame from the velocity (V15).
   - A soldier killed in the get-up lock (ids 0x10 / 0x11) rests in the thrown rest pose on the PS2; the port plays
     stand_death_forward.
   - Special::Dead always plays stand_death_forward, where B1 case 3 picks forward / backward / left / right by the
     hit (this predates KNOCK).
   - BF2's thrown state uses ControlSpeed row 7; the port uses row 4. This is latent: the port's BF2 throws no living
     soldier.
   - FUN_002098b8 scales the acceleration by 0.01 on a floor with n.y < 0.75, which affects a grounded throw.
   - A corpse is re-damaged on the PS2 and plays the class sound (+0x68) again. The port skips it, a named decision
     of the build.
7. **Test gaps**: the audit's mutation table is `lead_tmp/audit-knock/tools/muts.py`, results in `mut/results.txt`,
   judged by J1 / J2 in `wf/results.json`. 21 of 43 survived, 2 of them equivalent (K04 on today's bake, K32). The 19
   live survivors:
   - K01: a rider is pushed.
   - K05: the sim fires while knocked or locked (only `Soldier::no_fire` is tested).
   - K07: the tumble caps swapped between the games.
   - K08: the brake's 0.75 floor test.
   - K11: no stick in a throw.
   - K12 / K13: the airborne get-up.
   - K14: the tumble's 0.5 s minimum.
   - K15 / K16 / K17: BF2's apply (no soldier-level BF2 knock test exists).
   - K19: BF1's throw test on the impulse.
   - K28: the death block's item drop removed. It moves every round of the sweep (`sw/mK28`).
   - K31: corpses skip the wall pass.
   - K33: BF2's death at the get-up.
   - K35: a throw's wall pass.
   - K37: the tumble capsule.
   - K38: the tumble turn rate.
   - G3: the rest pose.

## Acceptance
1. **BF2 body motion** (core test from the PS2 rows `lead_tmp/knock/out/bf2_hot1b_knock.json` and
   `bf2_hot1flag_knock.json`): a blast-killed body's tumble and get-up speeds, update by update, follow the
   -2.5 / -10 proportional law within 0.05 m/s. A BF2 corpse on a floor of normal y 0.9 slides.
2. **Horizontal speed**: in both games, a tumbling body falling off a ledge (horizontal under 4 m/s, vy -10) ends its
   tumble at 0.5 s.
3. **The corpse draw follows `Unit::body`** (a game test). For a respawn while the body is in flight, read the PS2 rule
   first. At the least, the item drop is not lost (a test).
4. **Every live survivor of item 7 fails a named test.** Re-run `lead_tmp/audit-knock/tools/chain_mut.py <ids>` and
   paste its table.
5. **Sweep** 8 seeds x 32 maps against main: finished rounds, recycles, deaths by cause, and the body census of
   `lead_tmp/audit-knock/tools/akn.py` (observer: `tools/obs_patch.py`) per game.
