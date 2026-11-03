---
id: TEAMS
title: Special teams survey: every special roster on both discs (Wampas, Ewoks, Gungans, Geonosians, Naboo guards, Wookiees, Jawas/Tuskens, BF2's hunt rosters, BF1's historical rosters) — classes, weapons, items, sounds, movement, which LVLs and missions load them; verify the BF1 _h vs IA diff
status: archived
issue: 22
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #22](https://github.com/sprucey149/openbf/issues/22).

## Why
The maintainer (INBOX 2026-09-28, DECISION 2): special teams and modes become options, played the conquest way. Eventually the
player picks who fights on a map — clones, droids, rebels, Empire, and the special teams (Wampas, Ewoks, Gungans,
Geonosians, Naboo guards, Wookiees, Jawas/Tuskens, the BF2 hunt rosters, BF1's historical rosters) — with their own
weapons, items, sounds and movement, under conquest rules with no hunt score. For now: survey.

## Evidence (the maintainer's string-level diff of the 15 BF1 `lua/mission/*_h.luac` vs their IA twins — [H] until checked)
- Every `_h` adds `SetHistorical` and per-team difficulty. Content differences:
  - NAB1: Gungans (`gun_inf_defender`, `SIDE\gun`), fambaa shield, `NAB\nab1_h.lvl` vs battle droids (Republic removed)
  - NAB2: Naboo Royal Guard (`SIDE\gar.lvl`: `gar_inf_basic/soldier/pilot/vanguard`), `nab2_h.lvl` vs battle droids
  - KAS1: Wookiees vs CIS (Republic removed), plus mounted turrets, a carrier and hero bonuses
  - YAV2: its own world `yav2_h.lvl`, TIE fighters, a walker; TAT1: its own world `tat1h.lvl`
  - TAT2: a Mon Calamari backdrop instead of the Star Destroyer; KAS2: own objectives and a fly-height cap (KAS2_H.LVL)
  - END1/GEO1/HOT1: roster trims and a HERO bonus; BES1/BES2/KAM1/RHN1/RHN2/YAV1: difficulty only
- Special side files on the BF1 disc: `GUN EWK WOK GEO DES GAR` (planner listed SIDE/). BF2's hunts are still to survey.

## Acceptance
- `analysis/survey-special-teams.md`: every special roster of both discs — classes, weapons, items, sounds, movement,
  which LVL/mission loads them — and the BF1 `_h` vs IA diff verified; BF2 hunt missions surveyed.
- Feeds TEAMD (the design). Not built ahead of M1/M2 (the maintainer), except FAMB.

## Notes
- Created 2026-09-28 by the planner. M4 by the planner's judgement: roster content after M1-M3, beside heroes.
- UAUD1 (2026-09-29): the census tools (`engine/artifacts/lead_tmp/unit-audit/`, README) read any side LVL's sublevels
  and any mission's rosters; `census_all.md` already covers the 16 `_h` scripts (e.g. nab1c_h: Gungans incl.
  `gun_inf_defender`; nab2c_h: `gar_inf_*`; kas1c_h: Wookiees vs CIS). The IA locals are listed in analysis/unit-audit-bf1.md §2.
- UAUD2 (2026-09-29): the BF2 census tools are `lead_tmp/unit-audit/census2.py` (any ground script, `--missions all`
  covers the 87 con / ctf / 1flag / c / hunt / eli scripts), `ramclass2.py` / `ramcensus2.py` (BF2 class objects and
  live soldiers in a save state). The 87 ground scripts load 101 classes in all (`census2_all.md`).

- KNOCK's RE (2026-10-07, `analysis/re-knockback.md` §4, §7): **the Ewok's rock and the wampa's smash are the only
  two BF2 blasts that push LIVING soldiers** - `ewk_weap_inf_rock_exp` (SIDE/EWK.LVL: Damage 25, Push 4, both radii
  0.2 - 0.3) and `hoth_weap_inf_smash_exp` (SIDE/SNW.LVL: Damage 800, Push 8, PushRadius 3.5 - 4.5, HurtOwner 0)
  author PushDeadOnly 0. The port bakes neither side today (`expc.py`'s class tables miss them too). When these teams
  are built: bake EWK and SNW, and the pushed soldier follows BF2's apply (thrown at v·n > 3, tumble over 8 m/s, else
  a flinch; card KNOCK reads PushDeadOnly per class).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
