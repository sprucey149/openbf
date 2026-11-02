---
id: FGHTA
title: The fight, from its audit: a sniper circles inside 36 m where the PS2's scout stands, the circle's pause chance is twice the PS2's per think (1 in 6 for 1 in 12), and 8 of the fight's 11 rules have no test that fails when they are broken - fix the two, pin the constants; the port's fighters still stand twice as long as the PS2's (49-58 % against 24-36 %)
milestone: M3
value: 5
phase: build
status: done
deps: FGHT
files: engine/crates/core/src/ai.rs,engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai/mover.rs
declares: sweep; arena; sim
needs:
effort: medium
branch: fghta
resume: landed b75d36bc 2026-10-09
created: 2026-10-07
alpha: yes
---

## Why
From AUDIT FGHT (2026-10-07, PARTLY; `engine/artifacts/loop/audits.md`). Card FGHT (69f8c84) holds on every number it
landed with, and it does what the maintainer asked on 2026-10-05: with a human standing at his Kamino spot, the bots that
engage him have a teammate within 3 m 63 -> 30 % of the time and stand still 75 -> 44 % (main today 34 / 51 %; four
more scenes the same way). Three things in it are not what its own comments say, and the PS2 side of "how much do
fighters stand" was never set beside the port's:

1. **A sniper does not stand.** `fight_position`'s doc says "A scoped weapon stands"; `style()` makes it a stander,
   but the stander's band rule (`behave.rs:806`) then sends it round the ring whenever its target is nearer than
   (MinRange + OptimalRange) / 2 = 36 m (BF2; BF1 42-48 m) - a scoped rifle [8, 64, 128] with a target at 12, 20 or
   30 m is in `FightState::STAND` on 0 of 600 ticks and walks 45-65 m in 20 s. The PS2: a class of UnitType scout
   with a rifle-kind weapon IN RANGE (MinRange < d < MaxRange) gets mode 2 before the draw is called, so no "2 becomes
   3 outside the band" (`FUN_002abc38`, B2:231230-231234; the class's UnitType is read by its vtable slot +0x2c =
   0x2565b8, `lw v0,0x8d4(a0)`, scout = 0); out of range it does not attack. The PS2 keys on the CLASS, the port on
   the weapon in hand.
2. **`CIRCLE_STOP` is 1 in 6 a think and says "(1 in 6; attack mode 3)".** The PS2's is 1 in 12: `FUN_002a3ff0`
   multiplies its bound by `DAT_006980a0` = 30 / 15 = 2 (B2:225611, 234460; re-soldier-ai 1.1 and 4.5 say so). By
   rate: the port thinks every 0.33 s (0.5 stops a second); the PS2's fighter every 0.4 s at the level of a bot 7-28 m
   in front of the player (0.21 a second), 0.8-1.6 s farther off (0.05-0.10), 0.1 s inside 7 m (0.83).
3. **Eight of eleven mutations pass all 114 `ai::` tests** (`lead_tmp/audit-fght/tools/mutate.py`): the stander's
   band rule, `STAND_SHARE`, `STAND_CROUCH` (the test seam `tests::STYLE` answers before `style()`'s draw in all 14
   tests that set it), `CIRCLE_PAUSE`, `CIRCLE_STOP`, `FIGHT_BEYOND` (the scene changes, no assertion sees it),
   `RING_BACK`, `RING_SNAP` (no fight test has a nav graph). Finding 2 sat exactly where no test looks.

## Evidence
- `engine/artifacts/lead_tmp/audit-fght/README.md` (tools, tables), `verify/VERDICT.md` (an independent verifier:
  23 of 25 claims supported, 2 corrected), `observer_main_6f6160b.patch` (the `AUDIT_FGHT` switch on main and the
  scratch test `audit_a_scoped_rifle_by_distance`).
- The PS2's on-foot attack rows, still by the port's measure (under 0.5 m in 1 s; `tools/ps2_still.py` over SAIR's
  `bf2_kam1_v2` / `bf2_pol1_v2`): 24.2 % and 23.9 % (the whole combat agent 35.5 / 24.2 %); by mode on Kamino circle
  48 % of rows (15 % still), stand 23 % (63 % still), cover 21 %, flank 5 %, zig-zag 2 %. The port's fighters
  (`fght/tools/mates.py`, `beh == fight`): Kamino 64 -> 49 %, Polis Massa 72 -> 56 %.
- Levers on main, 8 seeds x 32 maps and the maintainer's Kamino scene (a teammate within 3 m / still of the bots engaging him;
  main 33.5 / 51.4 %, contact 10.65 %): pause chance 1/12 a think 30.7 / 47.9 %, contact 11.01 % (paired t +1.4);
  no chance pause 29.1 / 45.1 %, 13.12 % (255 of 256 rounds); every bot circles 34.5 / 35.2 %, 12.16 %; every bot
  stands 51.8 / 69.3 %, 9.55 % (the contact gain of card FGHT is the circlers' alone).

## Notes
- The switch build needs no knob for item 2: `AUDIT_FGHT=16` is 1/12 a think in `observer_main_6f6160b.patch`.
- The scenes' human is pushed by the bots (Polis Massa 8 m, Death Star 12 m; one Death Star round he falls and
  respawns): use Kamino, Naboo Plains or tat2 for a bar.

## Planner 2026-10-07 08:55 - value 4 -> 5, and BOTV waits on it
Items 1 and 2 change what every sniper and every circling bot does, so BOTV's table would be taken on a fight the
audit already calls wrong in two places; one medium session. At 4 the picker did not reach it in 44 picks. Undo:
`loopctl.py set FGHTA value=4` and take FGHTA out of BOTV's `deps`.

## Notes from card FGHT2 (2026-10-09; `analysis/re-soldier-ai.md` 15.5, `lead_tmp/fght2/out/`)
- A fight spell (consecutive `beh == fight` samples on foot, main 853ecf0, 8 seeds): Kamino median 4.3 s, Polis
  Massa 5.5 s, Death Star 4.7 s (p90 9.7 / 15.2 / 12.0 s) against the PS2's 2.2 / 3.4 s (p90 10.7 / 9.9; 4.11) -
  the port's fights last about twice as long (`out/spells_port.txt`).
- Where the fighters are: distance fighter -> target in a fight, median 32 m on Kamino (the PS2's attack 30 m), 18 m
  on Polis Massa (13 m); 42 % of Kamino's fight time is at 30-45 m (the PS2's 19 %, spread evenly from 0 to 70 m).
- The close-range damage gap is card CLSE's (inside 10 m: 65-90 hp per aimer-second on the PS2, 21-33 on the port).

## Landed 2026-10-09 11:46 (b75d36b; report `reports/20261009_1147_card_FGHTA.md`)
- Mutations: 14 of 14 fail a test (`lead_tmp/fghta/mutate.txt`). Scoped rifle [8, 64, 128]: STAND on 590+ of 600
  ticks at 12 / 20 / 30 / 45 m. Kamino scene, engage row: 33.3 / 47.3 % (parent ff68507) -> 32.9 / 45.3 %.
  8 seeds x 32 maps: 256 / 256 both, contact20 10.69 -> 11.16 %, paired t +1.6.
- `CIRCLE_STOP_RATE` 0.208 a second = 1 in 14.4 port thinks. `sim` was declared and did not move.
