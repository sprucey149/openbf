---
id: DEKF
title: Field the droideka: the roster accepts BASE walkerdroid (2-4 per BF1 Clone Wars battle, 1-2 in BF2), a bot rolls everywhere and deploys only to fight, raises its shield over half, and the player can pick it and fold with the crouch button
status: archived
issue: 85
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #85](https://github.com/sprucey149/openbf/issues/85).

## Why
UAUD1 / UAUD2: `cis_inf_droideka` is in AddUnitClass on all 13 BF1 and 16 BF2 Clone Wars missions and alive in every CW save state; `ClassRoster::build` rejects it (BASE chain ends at walkerdroid). With DEKU / DEKS / DEKD landed this card puts it into the battle.

## Evidence (`analysis/re-droideka-unit.md` section 5 [O])
- Form order: roll by default and through every capture / hold / follow order; deploy on fight enter and every fight think; roll on fight exit; the stuck walk (2 m / 6 s / 6 s) unrolls, walks to a random point within 10 m, done 2 m out. RAM: 218 of 218 ball rows order 2, 189 of 191 walk rows order 1.
- Shield: only in the attack state, down and over half -> raise (that think's fire request is skipped); never lowered.
- Fight: sight starts one only within 10 m (3-D); stand and shoot only; never a flyer; no dash-away; arrival radius 4 m, hub margin 4 m, hold radius 2 m; walk speed to the AI = MaxSpeed, no strafe.
- Never boards a vehicle or a turret, drops no items; captures posts; BF2 unlock at 12 points is card UNLK.
- Player: fold = the crouch button (BF2's; BF1's pad button is [H]), shield = the secondary trigger's press, the class's own camera sections.

## Acceptance
(1) A roster census line over every CW mission RON: droidekas fielded = the disc's AddUnitClass count on 13 of 13 BF1 and 15 of 15 baked BF2 missions (was 0). (2) BOT_SAMPLES / a headless round on BF1 NAB1 and BF2 UTA1 CW: share of droideka ticks in ball form with no enemy within 10 m >= 95 %, share deployed while it holds a fight target >= 90 % (PS2 stored states: 218 of 218 and 189 of 191). (3) A test: a deployed bot with the shield down at 1600 raises it within one think, at 1400 does not. (4) The sweep finishes on all maps; sim / sweep / arena moves declared.

## Notes
- deps SPL11 so the form order sits on the new mover seam (bot design); if SPL11 is still held when DEKU / DEKS / DEKD are in, the planner may drop the dep - the hook into today's brain is small (an order field written by the fight state's enter / think / exit).
- The bot design (analysis/bot-design.md) has no droideka section; this card's rules are the PS2's, deviations to be named there if the maintainer asks.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Planner note
Planner 2026-10-21 20:50 (the maintainer via INBOX 17:50 item 3): dep DEKS dropped, `first: 8`. The bot's shield raise (down and over half -> raise) and the player's shield press move to DEKS; this card fields the droideka without a shield.
