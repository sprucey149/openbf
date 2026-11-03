---
id: COLN
title: How the PS2 tests a soldier against a collision mesh: its ray-vs-leaf test is one-sided with a convex edge test (read, not verified) while the port's meshes are two-sided fans - what a soldier's own test does with a leaf, and BF2 Mustafar's ten-sided ground leaf the port fans over 2,300 m2 too much
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
STARC (2026-10-02, `lead_tmp/starc/README.md`) found BF2 POL1's radar doorway filled by the bake: a face with a hole is a signed fan on the disc. That one is repaired (`mungefs::coll_fan`). Two things it left:
- **The PS2's leaf tests are not the port's.** B2 `FUN_00165070` (decomp 4252-4408), the ray-vs-LEAF test: normal from the first three STORED indices (the loader `FUN_00165420` keeps index 0 and reverses the rest), a hit only when n.dir <= -1e-5 (one-sided), and the point inside every edge (convex). Read once, no verifier. `lead_tmp/starc/tools/ps2ray.py` runs it on the POL1 wall: a ray going west through the doorway hits leaf 49, yet 23 PS2 soldiers walked west through it - so the SOLDIER's narrowphase is another test (`FUN_00166468` gathers a region's polygons whole; what tests them is not read). The port fans every leaf two-sided for soldiers, bolts and sight alike.
- **BF2 MUS1 `mus1_bldg_world_1` leaf 16**: ten-sided ground leaf, apex vertex 1.3 m over the rest, outline crosses itself; the port's fan covers 30,816 m2 against a signed 28,504. Not repaired, not understood (`lead_tmp/starc/base/coll_mus1_world_1.txt`).

## Acceptance
- `analysis/re-coll-narrowphase.md` (RE discipline, one verifier): the soldier-vs-leaf and ray-vs-leaf tests of both games, [V] with lines; whether a leaf blocks from its back.
- A census line: how many SOLDIER-layer leaf faces on the 32 baked maps a soldier or a bolt can reach from the BACK (the count a one-sided test would open), and what the port does at MUS1 leaf 16 against a PCSX2 walk or RAM sample there.
- Build cards only for what a player can see or walk through.
