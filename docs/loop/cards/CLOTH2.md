---
id: CLOTH2
title: BF2 cloth slice 2: the PS2's cloth step on the CPU - Verlet with gravity -9.8, the frame's inertia (Dampening x its acceleration), drag and random wind along the normals, three distance lists, sphere / cylinder / box pushes off the COLL shapes, 1 iteration, dt <= 0.039 - so skirts and capes swing as on the PS2 (analysis/re-cloth.md 4)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card CLOTH draws the cloths pinned rigid in their rest shape. On the PS2 they are simulated while drawn: the HOT1 save
state's on-screen snowtrooper skirt has its 36 free vertices 0.12-0.36 m off the rest shape (`analysis/re-cloth.md` 4,
`lead_tmp/cloth/tools/ramcloth.py --verts`); a running snowtrooper's skirt swings and flares, Vader's cape trails. Without
the step the thighs cut through a stiff skirt when the soldier walks.

## Evidence
`analysis/re-cloth.md` 3 (class keys, defaults: WindSpeed 10, Dampening 0.5, Drag -2, ParticleMass 1, MaxAcceleration 20,
velocity clamp 10 with no key, wind direction (1,0,0)), 4 (the step, in order: frame velocity / acceleration; forces in the
cloth frame; Verlet; pins skinned to their bones; the lists 3, 2, 1 then the COLL pushes, 1 iteration; normals), 5 (draw:
two-sided, front normal), 6 (the PS2 steps a cloth only while drawn; budget and memory switch not kept).
Constants are ELF words (doc 9). B2 functions: FUN_001dcdd0, FUN_001dd130, FUN_001dd048, FUN_001dca78, FUN_001ddf40,
FUN_001dd430, FUN_001ddad0 + FUN_001dd5a0 / FUN_001dd8c0 / FUN_001dd660, FUN_001de118.

## Build notes
- The bake (CLOTH's segment) also writes the cloth's data the step needs per model: rest positions in the cloth frame,
  UVs, triangles, F, the weight bones, the three lists with their rest lengths, the COLL shapes (bone, type, params,
  rows), XFRM and PRNT, the class values. The game keeps one state per drawn wearer (positions, old positions), steps it
  on the sim tick with dt = min(tick, 0.039), and uploads the vertices each frame (dynamic vertex buffer); the rigid
  segment of slice 1 is then replaced (or kept as the fallback when the step is off).
- The random wind draw: one r in [0, 1) per step per cloth; the PS2's LCG is shared with other systems, so draw from the
  port's own seeded stream (cosmetic, no sim effect) - say so as a named deviation.
- The cylinder's half length is 2 x param (FUN_001dd8c0), its axial push unsigned (+row1) - keep both literally.
- Step every wearer within view distance each tick (the PS2 steps only drawn ones; a named improvement: no rest-shape pop
  when a soldier walks into view).

## Acceptance
(1) A replay test feeds the step a recorded pose sequence and checks the PS2's rules by unit cases: a pinned vertex
follows its bone exactly; a free vertex at rest under gravity alone falls dt^2 x 9.8 in the first step; a stretched list
entry is restored to its rest length in one iteration (half each, all on the free end when one end is pinned); a vertex
inside a COLL cylinder ends on its surface; mutation-check each. (2) A running snowtrooper (port unit still or a short
strip at 3 poses): the skirt's hem moves off the rest shape by 0.1-0.4 m (the PS2 RAM's 0.12-0.36 m) and no hem vertex
sits inside a thigh / calf cylinder. (3) A PS2 | port pair of a running snowtrooper, viewed.

## Note from CLOTH (2026-10-16)
Slice 1 landed (f1cd4ac): cloth segments are in the soldier GLB with material `extras.cloth` (bake/src/cloth.rs), each
vertex rigid on one bone (pins on theirs, free on the nearest pin's). The rigid skirt cuts through the thighs on the unit
sheets' crouch / jump poses - this card's step fixes it. Keep AIM_SIZE's measure off the cloth (`engine/crates/game/tests/suite/roster.rs`).
