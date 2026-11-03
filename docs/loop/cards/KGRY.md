---
id: KGRY
title: BF2 Kamino's flat grey ellipse floating in the sky (kam1_bf2_2230_invisible_rails.png, kam1_play_7): the RE could not find it at any stored pose
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's BF2 Kamino play (2026-09-30 ~22:30, on main after KSEA/VBIND): a large flat light-grey polygon floats in the sky above the
platforms, behind the HUD. It is the same object as the "grey ellipse" of kam1_play_7. It reads as an untextured / wrongly
drawn model or a sky-layer piece, not a cloud. KAMR's RE ruled out: map models with a missing texture, the three sky vehicle pads
(campaign layer only), the sky shells; and found it at no stored pose [H] (`analysis/re-kamino.md` §8).

## Evidence
- <repo>/engine/artifacts/lead_tmp/playtest_0930/kam1_bf2_2230_invisible_rails.png: the grey polygon at about x 680-1215, y 110-200 of the 2000-px view (two frames now show it,
  so it is easy to reproduce: it is at the player's pose in these frames, looking roughly level over the platform).
- <repo>/engine/artifacts/lead_tmp/playtest_0930/kam1_play_7.png: the same object, smaller, at about x 700-1030, y 170-195.
- Method: render the port at poses near where the maintainer stood (he stood on a platform edge next to a droid in both) with the draw
  list recorded (`pose_render`/a debug draw-id pass) and name the draw by its model/segment; then read why it is grey (default
  material, a missing sheet drawn as flat colour, a far-scene piece with no texture) and why it is there.

## Notes
Filed 2026-09-30 22:50 from the maintainer's third BF2 Kamino play session (three frames, `lead_tmp/playtest_0930/kam1_bf2_2230_*.png`); a claim in Why/Evidence marked as reading of the image is Claude's, not measured.

- Fable 2026-10-01 (the maintainer's chat session): ONE cause for KRMP + KGRY + KPOS, fixed in ec47e0b - the ocean step splices the draw list and the door poser kept its upload indices, so `pose_doors` moved ramps, decks and props to the doors' places (`lead_tmp/kamdoor/README.md`, `analysis/re-kamino.md` section 9). Nothing was wrong in the bake, the materials or the instance transforms; no headless path steps the water and poses doors together, which is why KAMR saw no misplaced object.
