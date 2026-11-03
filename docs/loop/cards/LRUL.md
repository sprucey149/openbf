---
id: LRUL
title: Loop rules in loopctl: audit by risk (item 3) and the M-ALPHA picker — `alpha: yes` first at equal value, `defer: post-alpha` and `acceptance: needed` never picked while other work exists, the counts in the scoreboard, the card rules printed in `orient`
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX note 2026-09-29 22:40 (approved loop improvements), items 3 and 6. Both change `scripts/loopctl.py`, so they are one card.
- **Item 3, audit by risk.** All 5 audits of vehicle or behaviour research found problems (HOVR, VDTH, VTURB, VWLK,
  VENT). The 3 audits of rendering research found none (SUND, TALPH, DSHN), because pixel pairs already check those.
  The JWAL audit (PARTLY) fits the same pattern.
- **Item 6.** The planner tagged the cards on 2026-09-30 (`alpha: yes`, `defer: post-alpha`, `acceptance: needed`).
  Only the values enforce that order today (deferred value-4 cards were lowered to 3, with `value_pre_alpha` kept).
