---
id: KPOS
title: BF2 Kamino: objects drawn in the wrong place all over the map (kam1_bf2_2230_wrong_positions.png) — find each and its cause against the PS2 walk pairs
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's BF2 Kamino play: "things rendered in the completely wrong positions all over the map". In the frame the domed pod at the
end of the walkway sits on / over the path and a huge hull piece hangs above it; the sea to the left is a band with black behind it
(KSEA's far pass: BF2's far sea now ends at black until KSEAL). I cannot tell from one frame which objects are displaced — the
claim needs the PS2 pair. KAMR's RE checked the upside-down tower (authored, matches the PS2) and the far pieces and found no
displaced instance, so this is either a different set of objects, the near scene, or a placement bug the census did not test.

## Evidence
- <repo>/engine/artifacts/lead_tmp/playtest_0930/kam1_bf2_2230_wrong_positions.png: the pod at about x 580-1040, y 410-640 and the ceiling hull above it.
- Existing material: PXWB walk pairs `pcsx2/ref/bf2_kam1/walk/pairs/` (6 posts x 4 bearings + spawn views); the instance transforms
  in the map manifest vs the PS2's (`re_check.py`, a PCSX2 state's object table). parity: bf2_kam1 cp2_cp3 30.4 and 20.88 mean.
- Method: for every BF2 KAM1 instance, compare the port's world transform with the PS2's own (read the object table from RAM at a
  state: position, rotation) and list every one that differs by more than a few cm or a flipped axis; group by model; then see
  which are visible in the maintainer's frames. A rotation/scale/parenting rule (bone-parented props, `AttachOdf`, instance flip flags) is the
  likely class.

## Notes
Filed 2026-09-30 22:50 from the maintainer's third BF2 Kamino play session (three frames, `lead_tmp/playtest_0930/kam1_bf2_2230_*.png`); a claim in Why/Evidence marked as reading of the image is Claude's, not measured.

- Fable 2026-10-01 (the maintainer's chat session): ONE cause for KRMP + KGRY + KPOS, fixed in ec47e0b - the ocean step splices the draw list and the door poser kept its upload indices, so `pose_doors` moved ramps, decks and props to the doors' places (`lead_tmp/kamdoor/README.md`, `analysis/re-kamino.md` section 9). Nothing was wrong in the bake, the materials or the instance transforms; no headless path steps the water and poses doors together, which is why KAMR saw no misplaced object.
