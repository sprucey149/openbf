---
id: PWPX
title: Pickups against the PS2: a PCSX2 frame of an item beside the port's (size, look, BF2's 1 s fade, the shadow blob), and why the port's bots take 2-4x the PS2's pickups on NAB1 / HOT1 / MYG1
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
PWUP (landed 6bc4290, 2026-10-03) built pickups without two of its acceptance checks. (1) No PCSX2 frame of an item
was set beside the port's: the port frames are `engine/artifacts/lead_tmp/pwup/e2e/{bf1_nab1,bf2_uta1}/walk_00030.png`
(one item of each class in a row); `ref/states/bf1_nab1_sideselect.p2s` holds six items at known positions
(`lead_tmp/pwup/tools/ram_items.py`), the camera by RAM hold. (2) Pickups per minute over 8 sweep seeds: port NAB1
3.2, HOT1 1.8, MYG1 3.0, UTA1 3.2; PS2 1.2 / 0.4 / 1.1 / 4.9 (re-powerups 8). Candidates: the port tests the touch
every tick for every unit (the PS2 needs one of the two bodies to move, 5.7), the reach (0.80 m head-on, in the
bracket), the bots' lines.

## Acceptance
- A PS2 / port pair of an item of each BF1 class at the same pose (`ref/FINDINGS.md`): height on screen within 10 %;
  what differs (BF2's fade, the `player_shadow` blob) named and either built or carded.
- The pickup-rate table re-taken (`lead_tmp/pwup/census/pw_census.patch` on `round_to_outcome`, `PWUP_CENSUS=1`,
  8 seeds) with the cause of the gap split by who takes (a unit standing still at the drop / one walking by) and the
  moving-body rule built if it closes it: each map within a factor 1.5 of the PS2's, or the residual explained.
