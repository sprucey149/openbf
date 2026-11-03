---
id: LEAK2
title: LEAK census, second pass: validate the closed class (ray parity; Endor reads 5-6k, mostly soldiers walking up to trees/rocks whose open meshes break parity), triage buried (under terrain, up to 114 runs) and float, add the checks LEAK left out (map-edge bounds with drives at the heightfield edge, BF2 door gaps, nav barriers, death-volume gaps), and name the crossed body per site
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
LEAK landed the instrument with per-map counts pinned as a gate, and every class accepted by mechanism in game/examples/leak_census_accepted.txt. The crossing classes (push/tunnel/ceiling/floor) are proven by traces; closed/buried/float are not yet, and some checks the LEAK card asked for are not built.

## Acceptance
- closed: a test that holds on open meshes (e.g. a reachability flood fill from the start, or parity per closed body only), validated on the KAS2 hut control (bf1_kas2 push sites round (4.9,-7.1)) and on Endor.
- buried / float: trace one of each, card the mechanism or drop the class.
- Drives at the map edge (offmap), through BF2 doors (closed and open), across nav barriers and around death regions.
- Each site names the body the axis crossed (CollisionWorld::ray does not return it today).
- Keep the census deterministic and within ~3 min.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — no soldier out of bounds. Value 3 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## AUDIT note (2026-09-30, LEAK → LEAK3)
The census's `offmap 0` on all 32 maps comes from its sampler, not from the maps.
- BF2 NAB2's 64 m blocks 562/563/584 (y 1.97–65.97) are never driven at: their side centres are 26 m over the street.
- The audit's band sampler (`lead_tmp/audit-leak/sites_a_band.txt`, samples at body height) finds 39 offmap sites
  (113 runs) beside them. Example: a sprint from (242.26, 7.86, 59.44) ends at (256.12, 7.86, 75.33), past the terrain
  grid's x max of 252.
- The map-edge check can start there. LEAK3 changes the sampler.
