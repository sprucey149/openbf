---
id: GRAI
title: The AI's weapon choice and its grenade: bots walk every slot for the target (ammo, target-kind mask, range band, plenty-of-ammo, score), a grenade in 8-24 m is forced in a 3 s window per 30 s per unit or against a group of enemies (BF2: + the difficulty roll = Skill::grenade_chance), thrown on the low-arc ballistic solution
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
`analysis/re-grenades.md` §7 (`[O]`, f0da5ad). On the PS2 a bot's target evaluation walks EVERY weapon slot and picks one
(ammo, target-kind mask, range band, "plenty of ammo", score); a grenade in range is forced in a 3 s window every 30 s
per unit, or at once against a group of enemies, and is thrown on the exact low-arc ballistic solution. The port's bot
fires its one primary; `Skill::grenade_chance` (`core/src/ai/draw.rs` 32, `skill.rs` 231) has no consumer.

## Planner 2026-10-02 17:45 - waits for BOTD
This card changes the soldier brain (`core/src/ai`) that the bot programme is about to redesign (the maintainer, 2026-10-02: "are we just implementing so many bandaid fixes to a fundamentally flawed bot algorithm"; BOTD: no rewrite before he has read the design). Held so no lane builds on the old brain meanwhile; BOTD adopts, rewrites or closes it when it cuts its build cards. Undo: remove BOTD from `deps:`.

## SAIR 2026-10-02 - additions to the weapon choice (`analysis/re-soldier-ai.md` 4.3-4.4, AGENT_H 2)
- "Category 2" of the choice is the ODF key **`OffhandWeapon`** (weapon class +0x1e0 bit 55, hash 0x2ecd946f): when the
  picked weapon is an offhand one and in range the bot's engagement mode is 2 - it STANDS to throw. A `UnitType`
  scout with a rifle-kind weapon also always stands (mode 2) when in range and does not attack when out of range.
- The explosive claim (`FUN_002ab430`): one bot per team at a time may use a grenade- or missile-ordnance weapon on a
  given soldier (10 s claims; as many as seats on a hover / walker).
- The target-kind mask keys are `AITargetPerson / Animal / Droid / Vehicle / Building / BuildingDead / Mine`
  (weapon class +0x124); a clustered soldier is exempt from the mask. The "too weak" rule: a weapon that cannot take
  half the target's max health in 10 s is usable by a foot soldier only with missile / shell / beam / haywire
  ordnance, or grenade / sticky on something slower than 12 m/s - else he does not attack that vehicle at all.
- Against a walker, hover or building the mode is 6 (go behind it) when the class has a weak spot, else flank for
  even controller numbers and stand for odd ones.

## BOTD (2026-10-03) - adopted (bot-design.md 4.8): the Gunner owns the weapon and grenade keys; after FGHT
The one-thrower claim is the attack token; the tier's grenades by situation are card TCMP's. Acceptance unchanged. In BOTV's deps.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**
- 2026-10-05 planner: value 4 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set GRAI value=4`.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
- 2026-10-06 (GRNP measure, `analysis/re-grenades.md` §13): nothing in the launch changes — the PS2 BF2 launch point
  (feet + 1.8183 up + 0.7433 along the facing, yaw only) and velocity (25 along the aim; a bot gets no ShotElevate) are
  what the port already has, and a running thrower adds no speed. The low-arc solution should aim at where the grenade
  first LANDS, not where it rests: thrown level it rolls 8 m on (first contact 27.45 m, rest 35.49 m, 1.37 s later) and
  from 29 deg up it stops where it lands (51.90 m). GRNP's build changes only the bounces (the terrain ray).
- 2026-10-06 21:16 (GRNP landed, fdff50d): the launch speed and direction are unchanged; the hand is now 1.7983 m over
  the feet (was 1.8183: the PS2 soldier rests 2 cm into the floor) — use `Thrown::BF2_HAND` for the low-arc solution's
  start, on BF1 maps too (`Thrown::BF1_THROWS_AS_BF2`: wind-up 0.51 s, fuse 0.6 s from the rest on every map). The
  bounces after the landing are the PS2's (re-grenades 13.4); a level throw rolls 8 m past its first contact.
- 2026-10-07 planner, GRNP's leftovers (landed fdff50d; `reports/20261006_2116_card_GRNP.md`), neither carded: (a) the snowspeeder's hull stops a grenade 0.81 m further along than the PS2's; (b) on BF1 maps the throw animation plays at its own length against BF2's longer wind-up (0.51 s against 0.3). Look at (b) when the bots' throw is built here; (a) is a hull-shape question for whoever next opens the ordnance layer.

## Build 2026-10-07 (lane 1, branch grai: 3314cd1, f2010af) - acceptance, run
- Built: `core/src/ai/arsenal.rs` (the choice, the score, the grenade test, the group test, the claim, the low-arc aim), the gunner's switch press / lob aim / SECONDARY_FIRE tap, `WeaponClass::ai_target_kind` (the `AITarget*` keys over the `Target*` bits), the stand to throw, the thrower's run from its own grenade, the sweep's `grenades <map>:` line (SWEEP_SHOTS=1).
- Unit tests (7, `cargo test -p core --lib ai::arsenal`: 7 passed): (a) elevation 9.14 deg for 16 m level; the stepped flight's first crossing is 0.83 m SHORT of the target (the solution is the exact parabola's, the 30 Hz step drops faster - the card's 0.5 m is not met and the PS2's shortfall is kept); no solution past 51 m; (b) number 3: only t in [9, 12) of 30 s; (c) the group stamp forces outside the window, 3 of a side within 9 m and not with a friend of the thrower among them; (d) BF2's roll: chance 1 every tick of the window, chance 0 only the tenth of a second where the sum is a multiple of 100 (the PS2's test is <=; "never" in the card was one tenth in ten seconds); (e) the mask; plus the choice's order and a bot in the sim: 4 throws in two windows, first contacts 0.39 / 1.17 / 1.07 / 0.92 m from the target. Mutation-checked (no raise: 2 tests fail; the gate always open: 3 fail).
- Census, 8 seeds x 32 maps against main 69f8c84 built beside it (`lead_tmp/grai/ab/{base,grai}`): grenades thrown by bots > 0 on every map, 83 (bf2_geo1) to 626 (bf1_rhn2) over the 8 seeds, 8,116 in all (10-78 a round); rounds finished 256/256 on both, no map's count drops; team kills 1,098 -> 913, self kills 0 -> 76 (0.9 % of the throws; 61 -> 9 in seed 1's 32 rounds with the stand, the clear arc and the run from the own grenade); recycles 59 -> 82 at spots the sweep already lists (bf2_dag1 hub 2 11 -> 17, bf2_pol1 hub 178 7 -> 14, bf1_end1 hub 10 1 -> 6), units on one leg at the end 24 -> 12.
- Gates (`lead_tmp/grai/gates/SUMMARY.md`): sweep and arena_tat2 MOVED (declared); everything else PASS (sim identical, goldens unchanged).
- PROVISIONAL / not built: the score (read from B2 `FUN_002aa548` / `FUN_002aa630` without a verifier pass; BF1's `FUN_002380a8` not opened - the BF2 formula is used); "plenty" counts clip + reserve; the "too weak for a vehicle" rule, the droideka term and the vehicle modes (the foot bots' targets are soldiers); the grenade's score term class+0x8c is taken as 1.
- AUDIT 2026-10-07: PARTLY (`loop/audits.md`, `lead_tmp/audit-grai/README.md`, report `reports/20261007_1735_audit_GRAI.md`). Every landed number reproduces (rows byte-identical on 8 seeds x 2 builds); the rules hold against both ELFs and four PS2 captures (Polis Massa: a PS2 grenade episode per 347-363 bot-seconds, the port a throw per 406). Not as stated or missing: 37 of the 82 recycles are at spots the parent has none ("at spots the sweep already lists"); the four contact distances are the first commit's (0.25 / 1.20 / 1.08 / 0.93 at 79f1890); 8 of 12 mutations pass every test; "plenty", BF1's score and BF2's reload skip are not the PS2's; the grenades cost 328 team kills + 87 self kills in 256 rounds on main. Follow-up card GRAIA (4).
