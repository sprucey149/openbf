---
id: B1ME
title: BF1 Mos Eisley is not a port map: the port's `tat2` is BF2's; BF1's TAT/TAT2.LVL with tat2i / tat2r (Jawa locals) is on the disc and plays on the PS2 - bake it under its own key
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
UAUD1 (analysis/unit-audit-bf1.md §2). PLAN's north star is "all maps of both games"; BF1's Instant Action list has 16 maps and the port
bakes 15. `scripts/rebake.sh` bakes `$B2/TAT/TAT2.LVL` as `tat2`, and `scripts/gen_missions.py`'s MAPS has no BF1
TAT2. The PCSX2 library already holds BF1 Mos Eisley (`engine/artifacts/pcsx2/ref/bf1_tat2_mos_eisley/`, no port
pairs) and its live roster (`lead_tmp/unit-audit/ramcensus_bf1.txt`: Republic, CIS, 13-14 Jawas).

## Acceptance
- A new map key (e.g. `bf1_tat2`) baked (map, sounds, nav, classes), its two mission RONs from `gen_missions.py`
  (tat2i, tat2r), a golden viewed; the sweep, walkabout and PCSX2 pairs include it.

## Notes
- No record of a decision to drop it was found (PLAN, PLAN_LOG, STATUS, cards). If it was dropped on purpose, close
  this card with the reason.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).
