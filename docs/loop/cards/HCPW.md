---
id: HCPW
title: Hover contact sidedness: at TAT1 cp4 yaw 0 the skiff wedges under the command-post overhang (a spring sphere 1.18 m past its length, pressed from above by n = −y) — is the PS2's mesh contact one-sided?
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The port's CollisionWorld::sphere_contacts is two-sided; a hover driven into TAT1's cp4 model at yaw 0 (game/examples/hover_drive.rs tat1 tat_hover_skiff cp4_vehiclespawn_10 0) wedges. Whether the PS2 would too is [H] (re-hover §11).
## Evidence
HOVR build report reports/20260926_1530_build_HOVR.md; the B2 contact FUN_0021fd98 and its collision query.
## Acceptance
RE answer ([O], verifier) on the PS2's triangle-sphere sidedness for vehicle contacts; then either a build change (one-sided contacts, declared sweep move) or a note that the wedge is faithful.

**Note (HOVR audit, 2026-09-28):** the PS2's contact loop FUN_00160da8 (B2:711-800) re-queries after each handled response. FUN_0016beb8 adds the entity's displacement to the query origin DAT_0066abf0 before FUN_0016bf10 finds the next contact, so push-outs never stack the way the port's summed-from-pos0 pass does (card HCON). The wedge may partly be that stacking.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — a skiff wedged under TAT1's command post. Value 3 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

- Planner 2026-09-30 21:45: `deps` + GSKIP. This card's diff lies under `engine/crates/core/`, which the selective gates skip for goldens, yet core feeds golden pixels (the gallery vantage's `ray_down` on COR1 / DEA1 / MYG1 / TAN1 / UTA1 / BF2 KAM1, the heightfield under grass and foliage, the walker pose). GSKIP corrects the rule first; `land.py` has no flag to force every group.
