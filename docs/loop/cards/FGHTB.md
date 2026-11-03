---
id: FGHTB
title: The fight's scout rule by the soldier's CLASS, from audit FGHTA: the PS2 stands a UnitType scout class with a rifle-animation weapon, the port any bot with a scoped rifle - BF1's assassin droid (UnitType trooper, the CIS sniper of 13 Clone Wars missions) stands where the PS2 draws; six subtle mutations pass every fight test (a doubled stop chance among them); on BF1 maps a stander stands anywhere in range
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT FGHTA 2026-10-09: PARTLY (`engine/artifacts/loop/audits.md`; evidence `engine/artifacts/lead_tmp/audit-fghta/README.md`,
`analysis/re-soldier-ai.md` 16, report `reports/20261009_1345_audit_FGHTA.md`). Card FGHTA (b75d36b) holds on every
number it landed with, and both of its PS2 rules hold as read (14 claims to two verifiers: 9 supported, 5 partly on
wording, 0 not). What does not hold:

1. **The key.** The PS2 gives "stand and shoot" to a soldier whose CLASS is `UnitType scout` (class +0x8d4 = 0; BF1
   class +0x1914) when the weapon it chose is of the rifle animation type (`AnimationBank`, not an `OffhandWeapon`)
   and in range. The port gives it to any bot whose weapon in hand passes `ai_distance_table()` (ZoomMax >= 6,
   single-shot, no spread). The landing named this as a deviation and did not count it. Counted on both discs by two
   independent readers: the two keys agree on 35 (class, weapon) pairs (BF1 7, BF2 28) and differ on 3 classes that
   carry a scoped rifle with `UnitType trooper` - BF1 `cis_inf_assassindroid` and `cis_inf_battledroid_sniper`, BF2
   `rep_inf_ep2_jettrooper_sniper`. The port fields the first: it is the CIS sniper (3-5 a team) of all 13 BF1 Clone
   Wars conquest missions (19 PS2 scripts). In PS2 RAM its class reads 5 (trooper), the clone sharpshooter's 0, in
   two Clone Wars states. On the PS2 it takes the trooper's draw (BF1: stand in range for 1 to 3 bots of 6 by the
   skill tier, else ring, zig-zag or flank); in the port every assassin droid is a stander (`style_of`, card FGHT)
   and since FGHTA stands anywhere between 32 and 128 m. The same weapon key decides the Snipe hints
   (`behave.rs:707`, `route.rs:900` / `:950`), which the PS2 also gives by `UnitType scout` (re-soldier-ai 3).
2. **Tests.** The landing's 14 mutations all fail a test (re-run on main: 14 of 14) - they are gross (a constant to
   0, a rule removed). Of 11 subtle ones 6 pass all 126 `ai::` tests: **the stop chance applied at twice the
   constant** (C4a: the test's measured band 0.04-0.105 a think undercounts by about 0.7, so it takes 1 in 7 for
   1 in 14; only the constant's own arithmetic is pinned), a scoped rifle standing beyond MaxRange (S2: the "out of
   range it circles" of the doc comment is asserted nowhere), the rule keyed on ZoomMax alone (S4), the crouch draw
   tied to the stand draw (B5: two thirds of the standers crouch; the test counts crouchers among ALL bots),
   `FIGHT_BEYOND` 10 -> 6 (F1: the test brackets it at +5 / +15), `RING_SNAP` 10 -> 20 (R1).
3. **BF1's stander.** BF1's draw has ONE range flag (`FUN_00238b38`, B1:152006-152117; verifier C8): a drawn "stand"
   stands wherever the target is in range and rings only out of range. BF2's becomes a ring outside the MIDDLE of the
   band. The port uses BF2's band on every map, and FGHTA's `a_stander_circles_outside_the_middle_of_its_weapons_band`
   now pins it for both games. On the BF1 maps of the sweep 39 % of unscoped fight seconds (86,000 of 222,000; 8
   seeds, the landing; `re/obs_land.txt`) are in range and outside the middle band; about half of those bots are
   standers by the draw (54 % stand inside the band): they ring there where BF1's stand.

Measured for whoever weighs them (main 296fa0c, 32 seeds x 32 maps, per-seed paired; `lead_tmp/audit-fghta/re/main_32seeds.txt`):
- The landing's contact gain is the stop chance's alone: +0.91 points of an enemy within 20 m (10.65 -> 11.56 %, t
  +10.9). **The scoped rule on top takes 0.47 back** (11.56 -> 11.09 %, t -4.5, down on 28 of 32 maps; BF1 Geonosis,
  where both teams field snipers, 16.0 -> 13.2 %, under its 14.1 % with neither rule). Per sniper life it changes
  nothing resolved (kills 0.626 -> 0.625, deaths 0.927 -> 0.919); the lives last 7 % longer.
- **Team kills rise with the lower stop chance**: 120.1 -> 139.8 a seed (t +4.8), 133.0 with both rules (t +4.0);
  1.65 -> 1.82 % of kills. BF2 Coruscant 2.6 -> 3.2 % and Mygeeto 2.7 -> 3.0 % cross the bot design's 3 % bar,
  Mustafar goes 3.4 -> 4.2 % (note on card GRAIA).
- The stop rate is the PS2's level-3 think (0.208 a second). By the PS2's own captures 81 % (Kamino) and 100 %
  (Polis Massa) of its circle rows are at levels 1-2: 0.07-0.10 a second. On main 1 in 28.8 a think is not told
  apart from 1 in 14.4 (contact +0.19, t +0.8): no change asked; `FEEL_NOTES.md` has the choice.

## What to build (each step its own commit; one switch per rule in ONE scratch binary first, null = main hash for hash)
1. The rule's key: a soldier class flag `scout` (the class db's `UnitType`, any case; `game/src/roster.rs` reads it
   already as `unit_type`) and a weapon class flag for the rifle animation type (`AnimationBank` after its first `_`
   = `rifle`, not offhand; `core/src/weapon.rs`). `stands_at`'s scoped clause and `style_of`'s "always a stander"
   ask both; a non-scout with a scoped rifle draws its style like any bot. Decide with the switch binary whether the
   Snipe-hint sites take the same key (the PS2's does; count the hints taken per map before and after).
2. Tests until each of the six mutations fails one: `python3 engine/artifacts/lead_tmp/audit-fghta/tools/mk_mut.py <ID>`
   prints the run command (IDs S2 S4 C4a B5 F1 R1; the mutation worktree is at 296fa0c - re-make it on main). The
   stop rate measured per walking THINK over enough walks to reject a factor of 1.5 either way (count every chance
   stop, not only those over 4 m short of the point); a scout's rifle at MaxRange + 5 of a [8, 20, 30] weapon rings;
   crouchers among STANDERS within 0.33 +- 0.1; `FIGHT_BEYOND` at +9 and +11; a hub 15 m from the drawn point is not
   taken.
3. BF1 maps: a stander stands wherever its target is in range (the sim's BF1 rule flag, as `arsenal.rs` has one);
   the band test keeps BF2's rule under BF2 rules. PROVISIONAL until measured: 8 seeds x 32 maps with the switch,
   contact and the scene, then 24 more seeds if contact falls.

## Acceptance (run and paste into the report)
- A census test in `game/tests/roster.rs` over the baked class dbs of both games: the rule's (class, weapon) pairs
  are exactly the 7 + 28 of `lead_tmp/audit-fghta/re/census_bf{1,2}.txt`; `cis_inf_assassindroid` with
  `cis_weap_inf_sniperrifle` is not among them.
- A unit test: a non-scout class with a scoped rifle [32, 64, 128] and a target at 40 m follows its draw (a circler
  walks; a stander stands only by the band / BF1's range rule); a scout class stands on 590+ of 600 ticks as today.
- The six mutations: 6 of 6 fail a test (today 0 of 6); the landing's 14 still fail (`tools/session_mut.py`).
- 8 seeds x 32 maps against main: 256 of 256 rounds finish; contact20 on bf1_geo1 and bf1_kam1 (the sweep's two CIS
  maps) not under main's by `audit-fght/tools/c20.py`'s paired t (a fall with t under -2 fails); team kills of
  kills not over main's 1.82 % by the same test.
- The maintainer's Kamino scene (`lead_tmp/audit-fghta/tools/scene_run.sh <binary> <out>`), row `engage`: a teammate within
  3 m <= 33.5 % and still <= 51.4 % (the bars card FGHTA met).

## Cloud checkpoint 2026-10-24
Blocked before implementation: baked class databases/maps and missions plus the audit census/mutation/contact/scene instruments are absent. Current-main reproduction and every asset-dependent acceptance bar remain unrun; no source changes or landing. See `../reports/20261024_0747_card_FGHTB.md` and its JSON path checks. Restore prerequisites before starting the switch binary; historical FGHTA measurements are not a current-main baseline.
