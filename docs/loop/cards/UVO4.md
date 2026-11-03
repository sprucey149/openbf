---
id: UVO4
title: BF2's unit voice in the fight: the port says 0.63-0.64 of the PS2's VOSound lines a minute beside a front-line team-mate (UVO2's census) - friendly hits a fifth of the PS2's, no repair or SC trigger, riders as objects untyped, HeadshotKill, the Wookiee's unbaked wok2_ takes, the black box's hash names
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card UVO2's census beside a front-line team-mate (`lead_tmp/uvo2/census/tab_fight.txt`, 8 seeds x 240 s, CW) reads 0.64 (UTA1) / 0.63 (MYG1) of the PS2's VO lines a minute (5.25 / 6.88 against 8.25 / 11.0): AcquiredTarget passes 0.78 / 0.69 of the PS2's, FriendlyFire 0.18 / 0.19 (0.22 / 0.28 against 1.25 / 1.5), and the PS2's RepairStart (0.25 / 0.5) and SC responses (0-0.5) have no port trigger. Each line property is held by its 40 +- 20 s gate, so the rate is the number of DISTINCT lines asked: more target kinds (turrets, hovers, walkers as objects) raise it.

## The pieces (each small; split if one grows)
1. FriendlyFire: the port's bots hit friends a fifth as often (the Hit events are raised for friendly hits: `core/src/sim/combat.rs` 33) - measure friendly hits a minute against a PS2 count before touching anything.
2. RepairStart / RepairEnd (B2 FUN_002d6828 261557, FUN_002d76a8 262105): an AI engineer's repair goal - the port's bots repair nothing yet; the lines wait for that card.
3. A rider as the object: `announce::vo_body` gives a rider unit type 0 - look up the vehicle class's `VOUnitType` (rep_hover_fightertank 124, ...).
4. HeadshotKill (event 9): the port reports no head hit; [H] in the RE whether it needs one.
5. The Wookiee's `wok2_inf_chatter_*` properties are in the GCW banks but their stream segments are not baked (`game/tests/suite/unit_vo.rs` NO_TAKE_YET).
6. The black box names VO lines by hash (the bank's names table lacks them): add the class lines' sound names to the names the snapshot resolves.
