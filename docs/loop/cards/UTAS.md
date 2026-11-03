---
id: UTAS
title: BF2 UTA1: the sinkhole reads as a hole in the world, not a pit
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## RE result (lane 2, 2026-09-29 — `analysis/re-far-lowd.md` [O], commit 4622b36)
- The PS2 DOES draw the far wall: `pcsx2/ref/bf2_uta1/cp4_con_cp2_b000.png` (the walkabout's own con_cp2 eye, bearing 0)
  is filled by the sinkhole's far cliff; the port at the same pose shows sky over black (`pairs/cp4_con_cp2_b000.png`).
- It is `uta1_bldg_cliffs`'s LOWD (the whole ring; LOD0 is only the near half), drawn by the PS2's far scene: a LOWD gives
  the LOD record flags 3 → far queue past NearSceneRange[1]·S → far walker's fade branch → LOD 3 = the LOWD slot.
  Three patched PCSX2 states each remove it and leave the port's exact picture (`lead_tmp/re/uta1-sinkhole/ps2exp/frames/sheet_b000_variants.png`).
- The port never draws a LOWD. Only UTA1's cliffs carry large LOWD-only geometry (census of 515 LOWDs).

## Why (rewritten by the planner 2026-09-28 from the maintainer's INBOX note; value 2 -> 5)
The binary question below is ANSWERED: not an orbit limitation. `engine/artifacts/walkabout/bf2_uta1/cp4_con_cp2.png`
(main 73632e1): at the con_cp2 platform, eye (-43.7, 3.3, 180.8), level pitch, bearing 0°, the port shows sky above the
horizon and clear-BLACK below it where the sinkhole's far side should be; the other bearings show black slivers at the
right edges where the cliff ends against the sky. The maintainer remembers the PS2 showing the far cliff, not sky.

## Evidence / what to do
- The PXC library can't settle it: all 5 UTA1 references are at cp3 (con_cp1), inside walkways. Capture con_cp2 (and any
  other post whose walkabout sheet shows black below the horizon) at the walkabout's bearings (PXWB's method: 0 AI,
  RAM teleport). Hole census: UTA1 cp4 37.5 / 6.1 / 0.6 / 13.2 % by bearing (`lead_tmp/walkabout_holes.txt`).
- If the PS2 draws a wall there, RE what it is:
  - a world model the port skips (the walkabout INDEX says 31 instances skipped on UTA1; list them with `PXC_SKIPPED=1`);
  - a sky DomeModel below the horizon (UTA1's dome stops at the horizon per the dome census; `uta1_sky_dome` is the only one);
  - or a layer the port drops (`re-lowr.md:164`: UTA1 has no heightfield).
- Then build it.

## Original QUEUE.md entry (verbatim, 2026-09-25)
UTAS. [TODO — ADDED 2026-09-17 (lane 1) by OCB's landing, from **The maintainer's flythrough report** item 1; [RE] first — the question is whether anything is wrong at all. FREE. Lower value than KASF and MUSS: it may turn out to be a flythrough-orbit limitation like bf2_yav1's, which is why the cheap test comes before any build.] **BF2 UTA1: the sinkhole reads as a hole in the world, not a pit** [RE] — the maintainer: the orbit shows a half-cylinder wall cut off against open sky and the FAR wall of the sinkhole never comes into frame; his expectation (and the movie beat it is modelled on) is that you can see across to it. Already on record and NOT sufficient: `analysis/PLAN_LOG_2026.md` ~1228 knows about "a ring of sky around the Utapau sinkhole" and added a clear-colour fallback — that fixes the COLOUR of the gap, not the geometry. The decisive question is binary: **is the far wall merely outside the flythrough orbit's radius/pitch, or is it not drawn from ANY angle near there?** Answer that FIRST and cheaply, from inside the game rather than from the orbit camera — walk or `--cam` to the sinkhole rim and look across. If it draws, this is a flythrough-tool limitation, the entry closes as documentation (one line in the 2026-08-31 flythrough-limitation note) and nothing is built. If it does NOT draw, then §5 RE: `analysis/re-lowr.md:164` lists **BF2 UTA1 among the maps with no heightfield** — establish whether that is related (does the far wall live in a layer the port drops?) or a red herring, with every citation's enclosing `FUN_...` named and any absence claim searched, then ONE opus verifier agent. GATE: either (i) a capture from the rim showing the far wall drawn, plus the documentation line — no code, no gate moves; or (ii) an `analysis/` section naming what is not drawn and why, an `RE_LOG.md` pointer, and a follow-on build entry with its own capture/golden gate. Files: `analysis/re-lowr.md` or a new `analysis/re-uta1-sinkhole.md`, `analysis/RE_LOG.md`, scratch under `engine/artifacts/lead_tmp/re/uta1-sinkhole/`.
