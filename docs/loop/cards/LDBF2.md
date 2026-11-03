---
id: LDBF2
title: The BF2 PS2 loading screen for BF2 maps (card BSHL's RE): header (map / mode), the still, the Tips and Tricks box (level.common.tips.1..54) and the Loading dots, from LOAD.LVL's load config - today the port draws the still and one caption
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The port's loading screen (`ui/src/loading.rs`) draws a BF2 map's still and one caption "<MAP>   n/N". The PS2 BF2
loading screen has a header (map name top-left, mode top-right on black), the still, a centred translucent "Tips and
Tricks" box and "Loading" + dots filling bottom-left (`analysis/re-bf2-shell.md` §6, frames
`engine/artifacts/lead_tmp/re/bshl/ref_frames/loading_hot1_*.png`). The BSHL planner note asked for the tips and order
to be checked: the PS2 shows `level.common.tips.N` (1..54 in English) and changed the tip once in a ~13 s load.

## What
- Bake (`rebake=load`): LOAD.LVL's `load` config (tips prefix, `loadtipsbox_pieces` with its 12, 12, the colours, the
  `loading` dots texture, the per-map stills LVL) and the two textures. BF1 maps keep BF1's zoom sequence.
- First step, before drawing: MEASURE the screen's layout on the four Hoth frames (header text positions and font,
  the still's rect, the box rect and its text inset, the dots' position and count, the tip's change interval) and
  paste the numbers here; the config's nine unnamed root fields may hold some of them [H] (re-bf2-shell §6).
- Draw it for BF2 maps: the still (4:3, pillarboxed on a wide display), the header, the tips box with a tip from the
  baked text table, the dots growing with the load's progress.

## Acceptance (each RUN and pasted)
1. Pixel geometry at 640x480 against `loading_hot1_06.png`: the box rect and the header text baselines within 2 px of
   the measured PS2 numbers (step 1).
2. A unit test: the tip text comes from `level.common.tips.<n>` in the baked English table and is never empty for
   n in 1..54; Galactic Conquest maps (if any) use `level.gal1.tips`.
3. A golden of the Hoth loading screen VIEWED beside `loading_hot1_06.png` before land.py commits it.

## Notes
- [H] which tip shows first and how they rotate is engine code not read; pick at random per load and change every
  ~10 s (the one PS2 load showed one change in ~13 s) - a reversible feel call, FEEL_NOTES line.
