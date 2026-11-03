---
id: PXSW
title: The PCSX2 reference library was captured under HW Basic on llvmpipe, which drops the BF1 KAS2 dock lamps that SW and HW High draw (LANE3): re-grab a sample of the walkabout poses under --renderer sw, list every FINDINGS / card claim about a MISSING light, lamp, glow or emissive element that SW contradicts, and decide the library's renderer
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Every PS2 reference frame (PXC batches, PXWA/PXWB walkabouts, 748 walk views) was grabbed with the global ini: Renderer -1
(auto), which under Xvfb is OpenGL on llvmpipe, blending Basic. LANE3's control on one state shows HW Basic DROPS the two
cyan dock lamps on BF1 KAS2 cp5 (lamp box peak (75,73,75)) that the software renderer (74,179,183) and HW High
(78,180,185) draw. So a "the PS2 shows no light/glow here" reading, or a port glow judged too bright against a PS2 frame,
may be an emulator artefact. SW also costs a third of the CPU (2.23 vs 5.45 cores), at full speed.

## Evidence
`engine/artifacts/pcsx2/README.md` "Software vs HW renderer"; `lead_tmp/lane3/out/lamps_hw_hwhigh_sw.png`
(rows HW Basic, HW High, SW); `lead_tmp/lane3/exp.py` reproduces it (~40 s per run).

## Acceptance
- ≥ 3 maps per game: re-grab ≥ 4 walkabout poses each with `walk_bf{1,2}.py` under `PCSX2_RENDERER=sw` (or `--renderer sw`),
  and count the 8x8 blocks that differ by > 16 levels from the stored HW Basic frame, split by cause (lamp/glow/emissive vs
  battle motion).
- A list of every FINDINGS.md entry and open card that rests on a missing or dim light/glow in a HW Basic frame, each
  marked holds / contradicted under SW.
- A decision for the library (SW for all new captures, or HW High), written into the PCSX2 README; if contradicted
  findings exist, notes on their cards.

## Notes
The walkers take PCSX2_RENDERER from the environment through pcsx2_session.py (default hw). Opened by LANE3 2026-09-30.

## Note (2026-09-30, GLOW RE §6)
HW Basic vs SW on the same patched states: BF2 MYG1 (glow strips + HDR bloom) mean |d| 1.9 (luma 101.2 vs 99.7), BF1 TAT1
(DomeInfo Filter on / off) 1.6 / 1.2 — the library's glow, bloom and filter frames hold at these poses. Frames:
`lead_tmp/re/glow/ps2exp/frames/*__hw.png` vs `*.png`. Also: a save state saved under HW has an empty GS VRAM image; SW
states carry the full VRAM (GS.bin, 425-byte header) — `lead_tmp/re/glow/gsmem.py` decodes it.

## Planner 2026-10-02 23:55 - the HW path changed under this card (the maintainer's INBOX note, 18:40)
Since 2026-10-02 every HW session renders on the NVIDIA card through Vulkan (`Renderer = 14`), not on llvmpipe
(`pcsx2/README.md`, the UPDATE bullet). On LANE3's state the new path is nearer SW than the old one was (frame mean abs
1.22 against 2.01) and differs from the old path by mean abs 1.86, p99 7. The 954-pair `ref/` library and every
FINDINGS entry are still old-path frames. So this card now has three renderers, and its first question is cheaper:
- **Step 0 (one grab):** the KAS2 dock state under the new HW path - are the two cyan lamps drawn (the old path's box
  peak was (75,73,75); SW's (74,179,183))? If yes, new captures need no SW run for lamps and glows.
- The acceptance's sample is taken under the new HW path as well as SW; the blocks-over-16 count is given for
  old-vs-new HW and for new HW-vs-SW.
- **A decision for the library, with a number:** re-score the sample's poses with `parity.py` against the new frames.
  If the mean block error of the sample moves by under 0.3, the library stays and the README says "old-path frames,
  renderer bias about 2 levels"; if more, a card to re-capture the walkabout sets (they are scripted) and re-baseline
  the parity gate.
- Until then: a NEW PS2 frame compared pixel-for-pixel with a stored `ref/` frame carries about 1.9 levels of renderer
  difference that is not the game's - say which path each frame came from in a FINDINGS entry.
Value stays 4. No card was opened for a full re-capture: nothing measured says the library misleads.
