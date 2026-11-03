---
id: TEAMD
title: Special teams as options, played the conquest way (the maintainer 2026-09-28): the design for picking who fights on a map (any faction or special team) under conquest rules with no hunt score — data model, roster loading, AI class assumptions, VO fallbacks
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer (INBOX 2026-09-28, DECISION 2): the design for special teams as options played the conquest way (command posts,
capture; NO hunt scoring). PLAN Architecture #6 ("modes are composable rulesets over rosters") and §Modes & rules
(updated 2026-09-28 by the planner) already allow it.

## Acceptance
- A design note (`analysis/design-special-teams.md`): how a map's teams are chosen (any faction or special team per side),
  roster loading from the special side LVLs, conquest rules for teams that have no CPs of their own (hunt maps), AI
  class assumptions, VO/HUD-icon fallbacks, the menu. Then build cards (M4/M6). The maintainer reviews the design in STATUS.

## Notes
- Created 2026-09-28 by the planner. Deps TEAMS.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 1 (kept).
- Audit ANNC 2026-10-02 — the shell already lets the player pick any two factions, and it is today's only way to play a mission's second side. The commander's part of that (bleeding / low-reinforcement lines keyed by the script's team number, so the enemy's commander speaks) is card ANNC2, not this design card.
