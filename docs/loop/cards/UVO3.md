---
id: UVO3
title: BF2's unit voice at the post: the PS2 lets an AI speak only at detail level 3 or 4 (B2 FUN_002e6180, +0x560 < 3 returns) and the port has no detail level - the port says 1.53 / 1.06 VOSound lines a minute round a player parked at his post on UTA1 / MYG1 against the PS2's 0.25 (UVO2's bar: at most 1): find what writes +0x560 and gate AI speakers the same way
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card UVO2 landed BF2's VOSound lines with the speaker rule of re-unit-chatter.md 3.3 except step 4, the AI detail level (a named deviation). Its census (`lead_tmp/uvo2/census`: run.sh + tab.py, 8 seeds x 240 s, CW, player parked at his spawn; `tab_post.txt`) reads 1.53 (UTA1) / 1.06 (MYG1) VO lines a minute against the PS2's 0.25 (`lead_tmp/usnd/live_bf2.txt`). At the post the AcquiredTarget speakers stand 30-50 m from the player (gate passes 1.28 / 0.72 a minute against the PS2's 0.25 / 0) and the slaughter lines (subject an AI unit, no tie to the player) add 0.19 / 0.66 a minute with speakers 50-275 m off. The PS2's detail level (AI controller +0x560; 3 and 4 think every 1.0 / 0.25 s) is likely nearness / visibility to the player: it would thin both.

## Evidence
- B2 FUN_002e6180 271980-272003 (`slti 3` at 0x2e6414); FUN_002b6fe8 238927-238946 maps the level to 4 / 3 / 2 / 1 / 0.25 s; a grep of the export for stores to +0x560 finds one (469452, = 0): the writers go through another pointer (re_check writers, an ELF sw scan).
- Port: `lead_tmp/uvo2/census/tab_post.txt`, `tab_fight.txt` (5d61556 = main b74b508's code, plus the scratch follow patch `vo_follow.patch` for the fight rows).

## Acceptance
- RE (re-unit-chatter.md 3.3 step 4): what sets +0x560, as a distance / view rule, [V] cites and the verifier.
- Build: UVO2's census re-run - parked at the post at most 1 VO line a minute on UTA1 and MYG1; beside a front-line team-mate the lines and AcquiredTarget passes a minute not below UVO2's (5.25 / 6.88 lines, 4.88 / 5.69 passes).

## Merged: UVO4 (planner 2026-10-16 17:00, batch rule: same files, the same census, UVO4 value 3; one session pays the census setup once. Order: UVO3's RE and gate first, then these pieces in the same build)
### UVO4 Why
Card UVO2's census beside a front-line team-mate (`lead_tmp/uvo2/census/tab_fight.txt`, 8 seeds x 240 s, CW) reads 0.64 (UTA1) / 0.63 (MYG1) of the PS2's VO lines a minute (5.25 / 6.88 against 8.25 / 11.0): AcquiredTarget passes 0.78 / 0.69 of the PS2's, FriendlyFire 0.18 / 0.19 (0.22 / 0.28 against 1.25 / 1.5), and the PS2's RepairStart (0.25 / 0.5) and SC responses (0-0.5) have no port trigger. Each line property is held by its 40 +- 20 s gate, so the rate is the number of DISTINCT lines asked: more target kinds (turrets, hovers, walkers as objects) raise it.

### UVO4 pieces (each small; split if one grows)
1. FriendlyFire: the port's bots hit friends a fifth as often (the Hit events are raised for friendly hits: `core/src/sim/combat.rs` 33) - measure friendly hits a minute against a PS2 count before touching anything.
2. RepairStart / RepairEnd (B2 FUN_002d6828 261557, FUN_002d76a8 262105): an AI engineer's repair goal - the port's bots repair nothing yet; the lines wait for that card.
3. A rider as the object: `announce::vo_body` gives a rider unit type 0 - look up the vehicle class's `VOUnitType` (rep_hover_fightertank 124, ...).
4. HeadshotKill (event 9): the port reports no head hit; [H] in the RE whether it needs one.
5. The Wookiee's `wok2_inf_chatter_*` properties are in the GCW banks but their stream segments are not baked (`game/tests/suite/unit_vo.rs` NO_TAKE_YET).
6. The black box names VO lines by hash (the bank's names table lacks them): add the class lines' sound names to the names the snapshot resolves.

### UVO4 acceptance (added to the above)
- The fight census (UVO2's run.sh + tab.py with `VO_FOLLOW=20` and `vo_follow.patch`): VO lines a minute within 30 % of the PS2's on UTA1 and MYG1 (at least 5.8 / 7.7), or each remaining gap named with its own number.

## Cloud RE checkpoint (2026-10-24)
- Later research already names the setter: `analysis/re-ai-think-rate.md` 3 and
  `analysis/re-soldier-ai.md` 1.3 cite `FUN_002b7b00` (B2:239522-239661); the
  constructor's level 3 is cited at B2:238841. These are existing research claims,
  not a new primary verification by this worker. Start there rather than repeating
  the original offset-only search. AITHK's section 9 lists its checks and verifier.
- Both BF2 primary inputs, AITHK/SAIR's raw captures and verdicts, and UVO2/USND's
  original census helpers/rows are missing here; `re_check.py fn` and `word` each
  exit 1 on their missing input. Inventory: `lead_tmp/uvo3_20261024_0858/inventory.json`.
  Keep phase `re` until the writer/default/constant checks and independent verifier
  can run. No new [V]/[O] claim, distance approximation, source edit or rate measured.
- AITHK's shared sim detail-level implementation is also blocked. Coordinate reuse
  when available; UVO3 does not require its whole scheduling/performance change.
  All original UVO3 and merged UVO4 acceptance bars remain in force.
