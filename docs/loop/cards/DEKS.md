---
id: DEKS
title: The droideka's shield: 3000 points, -100 /s up and +300 /s down, a press toggles, down while rolling; a 1.25 m sphere that takes a bolt's damage and passes none on; BF2's full shield does not drain
status: archived
issue: 85
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #85](https://github.com/sprucey149/openbf/issues/85).

## Why
`analysis/re-droideka-unit.md` sections 1-2 [O]: the shield is what makes a droideka a droideka - a second health pool with its own collision sphere. Not fielded yet (DEKF), so nothing is declared.

## Evidence
- Class keys of the weapon class `shield`: MaxShield 3000, AddShield -100, AddShieldOff 300, ShieldOffset 0 1.0 0.125, ShieldRadius 1.25 (both discs).
- Rules: spawns full and down; a press edge of slot 2's trigger toggles; forced down at 0, when dead and in states 0xb..0xd; no refill on a press; BF1 drains from full, BF2 only once under max (live: 69 ticks up at 3000.0).
- Damage: a bolt entering the sphere from outside takes damage x ShieldScale off the shield and nothing off the health (even the bolt that empties it); a bolt that starts inside the sphere hits the body and skips the shield; water / crash / kill floor take shield then health. BF2: SetAIDamageThreshold floors it for bot attackers (default 0).
- Sound: ShieldSound loop gain x (1 - (1 - shield / max)^2), ShieldOffSound (BF1 authors `droid_fx7_hum`); the bubble (`ShieldEffect droidekashield`, DEKA's note on FUN_00389338) is DEKD's.

## Acceptance
Tests in `core`: (1) up and unhit, a BF1 shield falls 100.0 /s +- 1 and is down at 30.0 s; down, it rises 300 /s to 3000; (2) a BF2 shield raised at 3000 stays 3000 for 5 s, then drains after one hit; (3) a 55-damage bolt from 10 m takes 55 off the shield and 0 off the health; with 30 shield left it takes the shield to 0 and the health keeps 450; (4) the same bolt fired from 1 m inside the sphere takes 55 health and 0 shield; (5) rolling drops the shield and a press while rolling does nothing; (6) a press edge toggles, a held trigger does not re-toggle. Gates: every gate PASS, nothing declared.

## Notes
- [H] in the RE: which phases an explosion passes - PROVISIONAL both (shield then health), named in the code.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Planner note
Planner 2026-10-21 20:50 (the maintainer via INBOX 17:50 item 3): `first: 9`, after DEKD and DEKF. This card now also carries the shield bubble's draw (from DEKD: `ShieldEffect droidekashield` at ShieldOffset, r 1.25) and the shield's use (from DEKF: a bot raises it in the attack state when down and over half; the player's secondary-trigger press). Add `deps: DEKD,DEKF` if the bubble needs the drawn body.
