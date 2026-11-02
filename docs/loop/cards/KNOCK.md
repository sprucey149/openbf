---
id: KNOCK
title: Explosions throw soldiers as the PS2's do: BF1 pushes every gathered soldier not thrown / tumbling / getting up / mounted by (normalize(d)+(0,0.6,0))·Push·f - thrown at g 13.5, a tumble, a get-up, a 1.93 s lock; a kill by a blast or a direct hit kicks the body 5 m/s, the blast-killed fly and die at the landing, corpses keep a moving body for 4 s; BF2 pushes only the dead unless the class authors PushDeadOnly 0 - today a blast only damages and a dead unit freezes
milestone: M3
value: 5
phase: build
status: done
deps: VDTH
files: engine/crates/core/src/sim/blast.rs,engine/crates/core/src/sim/combat.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/soldier.rs,engine/crates/core/src/soldier/tests.rs,engine/crates/core/src/unit.rs,engine/crates/core/src/weapon.rs,engine/crates/game/src/soldier_vis.rs,engine/crates/game/src/animator.rs
declares: sim; sweep; arena
needs:
effort:
branch: knock
resume: landed 4a41d85a 2026-10-21
created: 2026-09-28
alpha: yes
first: 9
---

## Why (build, rewritten 2026-10-07 after the RE: `analysis/re-knockback.md`)
On the PS2 a grenade beside a BF1 soldier throws him 14 m through the air; he tumbles, gets up and is back in the
fight 4.4 s later. A soldier it kills flies the same way and only dies when he lands. Every kill - by a blast or by a
bolt - kicks the body 5 m/s along the hit, and the body slides and stays in the world for 4 s, where the next blast
throws it again. In BF2 a blast throws only the dead (the Ewok's rock and the wampa's smash are the two exceptions).
In the port a blast only damages, and a dead unit freezes where it stood.

## Evidence
- `analysis/re-knockback.md` (6f52332, 95f4e25): §0 the answers; §1 the push and its apply; §2 thrown / tumble / get-up and
  the lock with every constant; §3 the kick, the late death, the corpse; §4 BF2; §7 which explosions push; §8 the
  build notes. 55 claims through a 12-agent verifier workflow (§9).
- Measured on the PS2 (`engine/artifacts/lead_tmp/knock/out/live_summary.txt`; rows in `out/*.json`): BF1 17 of 17
  living pushed, 5 of 5 corpses, 8 of 8 in the skipped states unpushed, 6 of 6 controls untouched, 0.00 m/s off the
  model at rest; BF2 0 of 22 living pushed, 4 of 4 blast-killed and 5 of 5 corpses pushed; with the PushDeadOnly bit
  cleared in RAM 15 of 15 living thrown.
- The port today: `core/src/sim/blast.rs` line 103 marks the push's place; `ExplosionClass` already holds `push`,
  `push_inner`, `push_outer`; `Stance` has no thrown state; `sim/step.rs` skips a dead unit; `game/src/soldier_vis.rs`
  draws a corpse where the unit died.

## Not in this card
- Droid soldiers' ChunkFrequency roll and the chunk death: card DCHK. Until it lands a BF1 droid is pushed every
  time - a named, temporary deviation (the PS2 skips the push 6 times in 10).
- The mounted x 9 throw (needs a rider's death in a seat: note it on the seat's card when built), BF2's flinch,
  whatever else knocks a living BF2 soldier down (Force powers, vehicles): `re-knockback.md` §9's [H] list.

## History
- 2026-09-28 filed by VDTH's RE as "BF1 knockback of living soldiers" (value 3). 2026-09-29 AUDIT VDTH: BF1 pushes
  corpses too, the death kick is a velocity, the "resist" roll is ChunkFrequency (value 4).
- 2026-09-30 planner: `alpha: yes` (explosions throwing soldiers). Showcase maps: BF1 NAB1, TAT1, YAV2; BF2 UTA1,
  MYG1, HOT1.
- 2026-10-07 RE (manual run, the workflow trial): report `reports/20261007_1731_card_KNOCK.md`. The card's "no BF2
  class sets PushDeadOnly 0" was wrong (two do); the five skipped states are thrown, tumble, get-up and the two
  mounted ones.
- 2026-10-20 (the maintainer): grenades and rockets in the originals have a radius that both damages and PUSHES soldiers inside it, so a grenade can throw you without killing you, depending on health and distance. The port's blasts only damage (RKEX builds the rocket's explosion and uses this push). Build both games' rules as read in the resume.
- 2026-10-21 build (lane 2): b35255c (the push, the stances, the dead's body, the late death, the bolt kick; 8 tests from the
  PS2 rows, 6 mutations each failing a named test) + b98fabb (the thrown_* clips). Gates twice: every row PASS or a declared
  move, fidelity PASS, goldens unchanged. Sweep A/B seeds 1-8 against a 9e381b1 build: finished 256/256 both, recycles
  48 -> 38, floor 15 -> 19, under-map 1 -> 0, attacker-less blast 6 -> 17 (BF2 Geonosis; 40 seeds 21 -> 50, traced: an empty
  pad hover's decay explosion on both builds - note on VPAD2). Report `reports/20261021_1100_card_KNOCK.md`.
