---
id: UCNT
title: Per-map unit defaults: units alive per team and reinforcements tuned by map size, so a big map (Hoth, Endor) is not mostly walking - a table for the maintainer (PS2 defaults, contact, kills / min, round length per map), then proposed defaults he can override in the menu
status: archived
issue: 93
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #93](https://github.com/sprucey149/openbf/issues/93).

## Why
The maintainer, 2026-10-02: "a smaller map like Coruscant can feel like there is more action because you walk less, things are closer and so there is more fighting. Maps like Hoth or Endor are so big it feels like most of the time is just spent walking, which is more boring. Maybe there could be better per-map defaults to keep any map roughly the same length in time, and have sort of similar kills per minute - while noting that part of the fun of each map is that gameplay changes, so kills per minute shouldn't match on Coruscant and Hoth; the Hoth number could be increased a bit by looking at the default number of units on the field at a time. Overridable by the user too of course."
Earlier the same day (BOTD, his annoyance 2): modern hardware allows more - e.g. 16 -> 32 alive per team and a larger reinforcement count - but "if everyone is more spread out than they already are that wouldn't be as fun."

## The maintainer's rules for this card (2026-10-02, later the same day)
- **HELD until the bot fundamentals are done in their entirety**: "this card shouldn't happen until the fundamental components of the AI are figured out in their entirety. If the AI at a fundamental level is broken, throwing more bots in the game will not improve the gameplay experience." So: not before SAIR, BOTD (the maintainer has read the note) AND the build cards BOTD cuts (mover, avoidance, positions, team sense) have landed. Planner: when BOTD opens its build cards, add them to this card's deps. No `first:`.
- **The PC numbers first.** "The better hardware definitely improved the gameplay experience in this aspect, so the PC numbers should be looked at first - that version will be more true to modern hardware and what can be handled and still be a fun playable experience." The PC script's unit counts are the baseline candidate (33 of 57 missions differ; mostly PC 32 a side vs PS2 16-20); the scaled runs go UP from the PC's numbers, and the PS2's are the 'before' column.
- **Defaults are for a mid-class 2020 laptop, not this machine.** the maintainer's PC (i9-13980HX, RTX 4000 Ada) is well above the target. The cost table comes from card PERF (its reference-hardware budget); a default must hold 60 fps on that budget, and higher counts are the user's override.

## What to do (measure and propose - the maintainer picks the defaults from the table; no default changes without his word)
1. **The originals' numbers per map and mode, both games**: units alive per team (the script's `SetUnitCount` / `units`, the PS2's own caps and memory pools), reinforcements, any per-map AI count the PS2 lowers or raises, and what the PS2's menu lets a player change. From the mission scripts (`engine/missions/*.ron`, the Lua in `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/SourceCode/Common/scripts/`) and live RAM where a script does not say.
2. **Each map's size in the terms that matter**: playable area, mean route length between enemy posts, the mean walk from a spawn to first contact (seconds) - from the sweep's samples and BOTM's heat map.
3. **The table, 8 seeds per map, at the PS2 default**: round length, kills / min, kills / min per unit alive, share of unit-time with an enemy within 20 m and 60 m (contact), seconds from spawn to first shot fired / first hit taken (the 'walking' number), deaths per reinforcement minute; the PS2's own values beside them where a capture exists (KAM1 17.3 kills / min, re-round-stalls section 4).
4. **Then the same table at 1.5x, 2x and 3x units alive** (reinforcements scaled so the round length stays in a band) on the big maps first (BF1 / BF2 Hoth, Endor, Geonosis, Kashyyyk, Naboo plains) and two small ones (Coruscant, Tantive, Death Star) as the control: does contact rise, does the walk time fall, do hotspots still exist (top-3 cells' share of kills), does any map stop finishing, do bots jam at doors and bridges (recycles, stuck runs, team kills).
5. **Cost**: sim ms per tick and frame time at 32, 64, 96, 128 bots (30 Hz sim, 60 fps render), the 0-alloc rule, pool sizes (unit slots, bolts, the black box ring), draw cost of the soldiers. State the largest count that holds 60 fps on the maintainer's machine with margin. Quote PERF's ceiling the same way (the maintainer, 2026-10-15): counts stop at 500 a side; a map that holds 60 fps there reads "not tried above 500 a side" with PERF's fitted estimate of how far it could go (range and assumptions), never a bare number.
6. **Propose** per-map defaults (units alive, reinforcements) with the reasoning in one line each, aiming at: round length in a common band (say 10-20 min, the PS2's own range), time-to-first-contact under a bar on big maps, kills / min raised on the big maps but NOT equalised - each map keeps its character (the maintainer: Coruscant and Hoth should not match). Small maps probably stay at the PS2's count.
7. **The override**: the instant-action menu's unit count / reinforcement setting per match (what the PS2 offers, extended to the new range); the mission file carries the default, the menu the override. A build card after the maintainer's pick.

## Acceptance
- `analysis/unit-counts.md` with the tables (PS2 default, the scaled runs), the cost table, the proposal, and what was NOT measurable.
- STATUS "For the maintainer": the proposal as a short table (map | PS2 default | proposed | round length | kills / min | walk-to-contact before -> after).
- No default changed in this card. Build cards opened: the per-map defaults (after the maintainer's pick) and the menu override.

## Notes
- **Order matters: this comes AFTER the bots close with the enemy (CONT) and the bot-quality numbers exist (BOTM).** On BF2 Kamino a bot has an enemy within 20 m 2-5 % of the time against the PS2's 25 %: more units would hide that defect, not fix it. Measure on the main where CONT has landed, and say which main.
- Part of the maintainer's bot programme (JITR, FFLN, BOTM, SAIR, BOTD, OBST); BOTD's per-match route tilt (his annoyance 2) interacts with this - measure the counts without the tilt first.
- New probe binaries run under the memory cap (INBOX 2026-10-01 22:00).
- Already in the mission files (`engine/missions/*.ron` header comments): the PC and PS2 scripts DISAGREE on units alive on many maps (e.g. BF2 Hoth: PC 32 a side, PS2 19; the port takes the PS2 value). The PC numbers are the designers' own answer for stronger hardware: put them in the table as a third column and as the first candidate for the proposal.

- Planner 2026-10-02 17:45: deps += BOTV, as PERF (the maintainer's 17:20 / 17:40 notes: BOTD's build cards land before UCNT; they are listed in BOTV's deps).

## Planner note
Planner 2026-10-21 20:50: the maintainer's 18:10 decision - special-class counts scale with the team size - is card CSCAL; this card's table uses it.
