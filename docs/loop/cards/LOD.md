---
id: LOD
title: The `modl` LOD switch rule: the port draws LOD0 forever (and discards the baked LOD chains); the PS2 switches to `LOWD` / `_low1` by some distance rule
status: archived
issue: 69
---

Historical technical record. Recorded state before archival was `done`. Current scope and ownership are on [issue #69](https://github.com/sprucey149/openbf/issues/69).

## Why
(2026-10-16) The PS2 draws a geometry's LOWD past the near scene (NAB1's leafy far crowns, UTA1's sinkhole ring, GEO1's
wreck); the port draws LOD0 there (the "dead forest"). Original text:
Faithful far looks (GEO1's Techno Union wreck) and PLAN §Optimisation B2 (the discs' LODs, "the next optimisation item")
both need the switch rule. Soldiers' `_low1` LOD rides the same question.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 20 and 3; `analysis/re-vu1-cull.md` §4 (two-station capture note); `FUN_00592c38`'s distance compare at
D:712968 may be it; `ManifestGeometry.lods` baked, `mungefs::entity` picks the best model; PLAN §Optimisation backlog B2.

## Notes
- 2026-10-19 (the maintainer's order, `first: 1`): the first speed item for weak hardware (Surface Pro 8, Intel Iris Xe). Far-draw cost is the likeliest big win after this; ISPD (`first: 2`) measures the rest on this machine's iGPU. Mind the file overlap: ISPD's slices wait for this card.
- 2026-09-29 (UTAS, lane 2): most of the rule is now read — `analysis/re-far-lowd.md` §2 [O]. NEAR scene: not a distance,
  a per-level triangle BUDGET (`FUN_005ace40`: largest-on-screen first, the first mask level whose running count fits the
  queue's budget, a density skip, LOD 3 as the fallback). FAR scene (past `NearSceneRange[1]·S`, shown beyond `[2]`):
  every geometry with a LOWD is drawn AS its LOWD (flags 3 → the far walker's fade branch at LOD 3). So "the PS2 switches
  to LOWD" = "beyond the near scene, per pixel". Still open for this card: the budgets' values (`DAT_007ec448` queues),
  whether to follow the PS2 at all (LOD0 everywhere is the modern choice), and GEO1's wreck ([H]: past the near scene
  the PS2 shows the hull's LOWD, whose rafters texture is opaque — the likely answer to HOLD 20, needs a PCSX2 frame).
- 2026-09-29 (PXWB, FINDINGS F61) [H]: BF2 NAB2 — through the rotunda's arches (`pcsx2/ref/bf2_nab2/walk/pairs/cp3_cp4__sheet.png` b090-b270) the port draws the far houses with big solid-yellow window blocks the PS2 does not show; a far-model / LOD texture difference to check when the LOD rule lands.
- 2026-09-30 (LEAF RE, `analysis/re-leafpatch.md` §10) [V measured]: BF1 NAB1's "dead forest" at a distance (PXWA F38,
  `pcsx2/ref/bf1_nab1/walk/pairs/cp1_cp2_b000.png`) is THIS card's, not LEAF's: in `bf1_nab1_cp1_cp2_postA_spawn.p2s` the
  leaf draw never got past its cull (`DAT_005d0010` = 0; nearest leaf object 237.5 m vs MaxDistance 120) yet the PS2
  shows round leafy crowns at 214–553 m, and a patched state that removes every leaf patch leaves the mid-distance round
  trees untouched. `nab_prop_trees2LOWD`'s texture `nab_prop_trees2lores` is a leafy crown billboard
  (`lead_tmp/re/leaf/tex/bf1_nab1_nab_prop_trees2lores.png`); the port draws LOD0's bare `nab_prop_trees2brnch` cards
  everywhere. The planner may want this card's value up (NAB1 is PXWA's worst case); check YAV/DAG/KAS tree LOWDs too
  (maps with `FarSceneRange` 0 — BF1 YAV1, BF2 YAV1/DAG1/NAB2 — have no far scene, so no LOWD there).

- 2026-10-16 (LOD RE, lane 2, `analysis/re-lod.md`) [O]: a gmod is ONE model in a size slot + an optional LOWD (only 2
  gmods per disc have a real LOD0/1/2 chain), so the PS2 picks model / LOWD / nothing. FAR scene (FarSceneRange > 0): every
  LOWD-bearing object is its LOWD past NearSceneRange b..c, per pixel; objects WITHOUT a LOWD are not drawn past c. NEAR
  scene: per-slot frame budgets (BF1 35000/20000/20000 nearest first; BF2 100000/20000/20000 largest first) - budgets -> inf
  changed nothing on the PS2 at 6 poses; BF2 adds a 5-px size cull and a density switch (LOWD past d = r*S*sqrt(208000 /
  count), threshold-tested on the PS2). Soldiers DO use `_low1` (re-softskin 7 was wrong): ~4 hi-res soldiers a frame.
  Build = the far rule; the budgets, the 5-px cull, BF2's near density switch, `_low1` and the no-LOWD cut past c are named
  deviations (STATUS questions). F61 (BF2 NAB2 has no far scene) may be the near density switch [H]: render that pose with
  the switch on as the build's side check.
