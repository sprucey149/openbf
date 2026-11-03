---
id: HVYA
title: Audit HVYW follow-up: a flyer is attacked as the PS2's test says (a weapon that is not too weak at any speed, both games; BF2's state / 16 m/s only for a weak one), the vehicle choice's wiring from live state gets tests (9 of 13 audit mutations passed every test), and the PS2's explosive claim on MISSILES is read for a vehicle target
status: archived
issue: 97
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #97](https://github.com/sprucey149/openbf/issues/97).

## Why
AUDIT HVYW (2026-10-15, `loop/audits.md`, PARTLY). Card HVYW's numbers all reproduce and its reading of the PS2's
suitability test is right, but (1) `arsenal::against_vehicle`'s first line refuses EVERY weapon at a flyer of 16 m/s or more
in BOTH games, built on `analysis/re-soldier-ai.md` 4.4's "a foot soldier never attacks a flyer in the air" - which two blind
verifiers and the auditor refute; and (2) the rule's wiring from live state (the `VehAsk` / `Ask` that `think`,
`vehicle_bonus` and `choose_target` build) has no test: 9 of the audit's 13 compiling mutations passed all 661 core tests.
Player-visible: no bot flies, so the flyer gate acts only on bots answering the maintainer's own fighter - on the PS2 a rocket trooper
whose launcher is not too weak for that flyer shoots at it at any speed; in the port none does above 16 m/s.

## Evidence
- The PS2 (both verifiers + the auditor, `lead_tmp/audit-hvyw/wf/results.json` V1 / V2 claim H12): the matrices' soldier x
  flyer cell chooses a weapon on both branches - B2 `FUN_002ac8f8` 231822 (target category 5 = `DAT_00689368`, the flyer
  type, `FUN_002a6138`) -> `FUN_002ac2b8` 231489-231495 -> `FUN_002abee0` / `FUN_002ac270` -> the chooser `FUN_002ab590`;
  B1 `FUN_002397d8` 152675 (category 5 = `DAT_005849f0`) -> `FUN_002392a0` -> `FUN_00239260` 152331 -> `FUN_002387b0`.
  "In the air" is asked ONLY in B2 `FUN_002aaba0`'s too-weak path (230690-230712): a flyer in state 0, 1 or 3 (+0x60c) or
  slower than 16 m/s (speed^2 < 256.0). B1's function has no flyer test (re-soldier-ai 22.2, receipt 0b484166): a too-weak
  weapon is never used on a BF1 flyer, one that is not too weak at any speed.
- The port: `against_vehicle` (arsenal.rs, main 70b9c47 line ~168) `if v.kind == Flyer && v.speed >= FLYER_SPEED { return
  None; }` before any other test; the doc comment cites "4.4". `by_ordnance ... && (bf2 || v.kind != Flyer)` is right.
- Surviving mutations (`lead_tmp/audit-hvyw/tools/mk_mut.py` <ID>, run by `tools/mut_run.sh`; logs `mut/<ID>.log`):
  MA1 `think`'s VehAsk takes the hull's CURRENT health; MA3 `think`'s VehAsk halves the hull's speed; MA4b shell and beam lose
  the exemption; MA6 `vehicle_bonus` +100 for any usable weapon, not only a heavy one; MA7 `choose` keeps the heavy flag at a
  vehicle; MA10 `choose_target` adds the bonus to an UNSEEN contact; MA14 every hull asked as health type Vehicle (a building
  turret's Building lost); MA17 "heavy weapon in the hands" tests slot 0; MA20 the +100 halved at its point of use.
  Caught: MA5, MA8, MA9 and the landing's MS1. Every HVYW scene spawns the hull at rest and whole (`manned()`), so cur =
  max and speed = 0 at every choice.
- The 22.1 step-5 building flag is identified (both verifiers): +0x26e bit 0 = the class's MaxHealth is finite (B2
  `FUN_00267bf0` 185257-185258; default FLT_MAX at 186491, set only by the `MaxHealth` key 0x19971f1b); BF1's +0xe0 the same
  (`FUN_00221200` B1:135459). So the PS2 lets a weak weapon with missile / shell / beam ordnance be used on a DESTRUCTIBLE
  building; the port's "every turret is taken (PROVISIONAL)" is that rule for every turret with a MaxHealth.
- OPEN (read before building anything on it): B2 `FUN_002ab430` (called from the chooser 231038-231051 when its commit flag
  is set) throttles GRENADE (`DAT_00699d10`) and MISSILE (`DAT_00699d50`) ordnance per TARGET and team - `FUN_002e0160` /
  `FUN_002e0338(target, team, 4)` / `FUN_002e03e0`; the capacity is the target class's virtual `vtable+0xa4` (kind 4), not
  read for a vehicle. The port's claim (`ArmState::claim`) covers thrown weapons only, so every launcher of a team may fire at
  one tank at once. re-soldier-ai 4.3 says "on a given soldier"; the code tests no target type.

## Acceptance
1. A test, both games: a launcher that is NOT too weak for a flyer is chosen for it at 40 m/s (BF2 and BF1); a weapon that IS
   too weak is chosen in BF2 only under 16 m/s (the port has no flyer states: say so), never in BF1. Mutation-check: restore
   the 16 m/s gate for every weapon and the test fails.
2. Scene tests through `think` / `choose_target` that kill each surviving mutation above (MA1, MA3, MA4b, MA6, MA7, MA10,
   MA14, MA17, MA20): run `lead_tmp/audit-hvyw/tools/mk_mut.py` + `mut_run.sh` (rebase the anchors on the new main first)
   and paste the table; a mutation left alive gets one sentence why it is equivalent on shipped data.
3. The arsenal.rs comments (`against_vehicle`'s "(4.4)", the module header) cite the corrected rule:
   `analysis/re-soldier-ai.md` 24.2 / 24.3 (committed 988b229 by the audit; 4.4 and 22.1 step 5 carry the correction).
4. Step 3 (OPEN above): `FUN_002e0338`'s capacity for a hover / walker target read (one live RAM read or the vtable slot of
   each class); if the PS2 lets one rocket trooper per team fire at a tank at a time, put the claim on missile weapons too
   (or name the deviation in the module header) and give the heavy census (`HEAVY_CENSUS`) before / after.
5. `gates.py --auto` shows no undeclared move (a sim hash move is expected only if (1) or (4) changes a bot's choice in a
   headless round - (1) should not: no bot flies).

## Notes
- Not a defect, measured by the audit (K2): card HOLD's 10-24 s hub dwell (`behave::hold_at_hub`, any contact within 80 m)
  is also taken for a rider the bot may not attack, so a rifleman stands out the dwell in `fight` beside an AT-ST it does not
  shoot (BF1 Yavin Arena s2 unit 22, `lead_tmp/audit-hvyw/bb/bf1_yav2_s2.json`): 10 s+ no-shot stands at riders 8 -> 57 per
  256 rounds, about 3 s a round. Fold into this card only if cheap (skip the dwell when no weapon of the bot's is for the only
  contacts in reach); else leave.
- BF1's `rep_weap_inf_rocket_launcher` (base cannon, bullet ordnance, TargetPerson 1) is not "heavy" by HVYW's rule and fires
  759 of 842 shots at lone soldiers on main 70b9c47 (bf1_geo1, bf1_kam1): PS2-faithful by its own keys, against the letter
  of the maintainer's 2026-10-12 ask. A question for the planner / the maintainer, not this card's build.
