---
id: DAIS
title: BF2 Kamino hub 46, after SKIRT: bots on the pod's dais walk at points through its rail (43 of the 87 stalls SKIRT's prototype leaves, 20 per 100 passes against the PS2's 5) - the PS2's bots cross the dais in a median 1.5 s and leave by its four corner ramps (134 of 146 visits): measure the port's visits beside them, trace one, then build the way out
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer, 2026-10-11, of BF2 Kamino: bots "miss ... go to the side of it, try running at the side for a few seconds, then
go back a few feet" - hub 46, the pod, the spot of his own scene. Card SKIRT's RE (2026-10-12,
`analysis/re-soldier-ground.md` 7.5) sorted the stalls there: with the soldier's body, the ledge rule and the steep
probe put right (SKIRT's prototype) hub 46 goes from 52 to 20 stalls per 100 passes, and **what is left is not the
body**: 43 of the 87 are `wall slide` against `kam_bldg_pod2` with the bot ON the dais or the skirt (y 61.4-61.6),
walking at a point outside the rail ring (38 of main's 228 are the same). The pod's dais (floor 61.6) is ringed by a
rail: a ramp up from outside, a 0.7-0.9 m wall from inside. Card OBST2's note: hub 46's centre lies inside the ring
and a leg leaving it has its line through a barrier. The PS2 stalls there 5 times per 100 passes.

## Evidence
- `engine/artifacts/lead_tmp/skirt/out/hub46_K11WP_spots.txt` (the prototype's 87 stalls as clusters: feet cell, the
  point walked at, the steering's answer) and `hub46_n0_spots.txt` (main's 228); snapshots `skirt/snap_K11WP`,
  `snap_n0` (BF2 Kamino, seeds 60-67, 3 each).
- **The PS2's bots cross the dais and leave it by its four corner ramps** (`skirt/tools/dais_ps2.py` on card MOUTH's
  three 10 Hz captures, 24 min): 146 visits, a median 1.5 s on it (p90 7.0 s, 25 over 5 s, the longest 16.4 s); 134
  leave DOWN at a corner (SW 54, SE 35, NW 25, NE 20), 3 go up over the rail, 9 end there.

## Acceptance
1. **Measure first** (this card's phase): the port's twin of `dais_ps2.py` on snapshot trails (main with card SKIRT
   landed) - visits, seconds on the dais, how each leaves - beside the PS2's 146 / 1.5 s / 134 by the corners; and
   for the `wall slide` stalls on the dais: the leg, the point walked at and which route step put the bot inside
   the ring (one case traced tick by tick before any remedy, RULES 2).
2. Then the cause named and built: hub 46 at most 10 stalls per 100 passes (`mouth/tools/site.py`
   "hub46:-15.3,102.1,20", 8 seeds x 3 min; the PS2 5, SKIRT's prototype 20), `wall slide` against `kam_bldg_pod2`
   there at most 10 (main 38), a median time on the dais within 1 s of the PS2's 1.5 s; the maintainer's scene
   (`skirt/tools/scene.sh`) at most 3 trails over 6 reversals; 24 seeds x 32 maps: rounds finished, recycles, floor
   deaths and lost not worse.

## Notes
- Depends on SKIRT for the base (its rules change who gets onto the dais at all: more bots climb the skirt).
- Not the body: the soldier cannot walk out over the rail from inside on the PS2 either (3 of 146 went up).
