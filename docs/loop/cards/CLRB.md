---
id: CLRB
title: The port clears the frame to the FOG colour, the PS2 clears to black (RGBAQ 0): BF1 KAS2's pure-red band below the horizon (every KAS2 golden, 4,514 px at Beach Head) is the port's clear colour showing through a gap it draws nothing in
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/viewer/look.rs
declares: goldens=maps/bf1_kas2.png,maps/bf2_end1.png,maps/bf2_geo1.png,maps/bf2_nab2.png,maps/bf2_uta1.png,maps/bf2_yav1.png
needs:
branch: clrb
resume: landed a2ba3407 2026-09-28
created: 2026-09-28
---

## Why
The most glaring single-map defect: a pure (255,0,0) band under the forest backdrop in every BF1 KAS2 golden and PXC pair,
where the PS2 shows dark pixels. KASF (`analysis/re-fog.md` §9, [O]) proved it is not fog. It is the frame's CLEAR colour.
- `game::viewer::clear_color` (look.rs:296–301) returns the map's fog colour when the map has fog. The code calls this
  "PROVISIONAL … our choice, not a decoded PS2 behaviour".
- KAS2's fog colour is a real, live (255,0,0), whose ramp starts at 1000 m, so it is never visible as fog on the PS2.
- The band is a gap the port draws nothing in: below the dome's bottom ring (0.0°) and beyond the water/terrain edge at
  z = +1024.

## Evidence
- `engine/artifacts/lead_tmp/re/kas2-fog/probe/`: every one of the 4,514 red pixels (Beach Head `v1_sea`, rows 128–135)
  and the 1,082 at Dock One spawn (rows 128–131) turns exactly (0,255,0) when only the clear colour changes.
- The PS2 under those pixels is dark: median (10,20,11) / (13,21,13) / (7,16,9)
  (`pcsx2/ref/bf1_kas2/{cp0_cp2_beachhead_v1_sea,cp5_cp10_dock_one_spawn,cp0_cp2_beachhead_v4}.png`).
- The PS2 never uses the fog colour for a clear. BF1's clear packet `FUN_0049aa68` writes RGBAQ 0 (black, alpha 0) at all 4
  call sites; BF2's `FUN_00581830` does the same (`re-sky-pass.md` §5.2: the far target clears to black). The fog colour's
  only readers are fog passes (re-fog §9.2).

## Notes
- The band's full PS2 content is dark (black far target) plus, on KAS1/KAS2, the sky shells drawn MIRRORED below the
  horizon (card REFL, value 4). CLRB alone turns red into black, much closer to the PS2's (10,20,11); REFL fills it in.
- Created 2026-09-28 by KASF (lane 2), report `engine/artifacts/loop/reports/20260928_1009_re_KASF.md`.
