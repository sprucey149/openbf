---
id: FALPH
title: The frame's alpha plane leaks into pictures and maybe the window: since DSHN kept the disc gloss alpha on pass sheets, the unit/map goldens carry low alpha on droid pixels (SBD sheet 7.6 % of pixels < 200), so any compositing viewer shows white ghosts; render/src/device.rs picks caps.alpha_modes.first(), so a compositor listing PreMultiplied first would show droids see-through on the desktop — write opaque alpha in the final output, prefer CompositeAlphaMode::Opaque, and audit every reader of destination alpha
milestone: M0
value: 5
phase: build
status: done
deps:
files: engine/crates/render/src/device.rs,engine/crates/render/src/capture.rs,engine/crates/render/src/shader.wgsl,engine/crates/game/tests/common/mod.rs
declares: goldens=maps/bf1_bes1.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_hot1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/bf2_cor1.png,maps/bf2_geo1.png,maps/bf2_kam1.png,maps/bf2_kas2.png,maps/bf2_mus1.png,maps/bf2_myg1.png,maps/bf2_pol1.png,maps/bf2_tan1.png,maps/bf2_uta1.png,maps/tat1.png,maps/tat2.png,tat1_hud.png,tat1_overview.png,tat1_spawn.png,units/bf1_all_inf_pilot.png,units/bf1_all_inf_wookie.png,units/bf1_cis_inf_bdroid.png,units/bf1_cis_inf_sbdroid.png,units/bf1_imp_inf_darktrooper.png,units/bf1_rep_inf_pilot.png,units/bf1_rep_inf_trooper.png,units/bf2_all_inf_bothanspy.png,units/bf2_all_inf_engineer.png,units/bf2_all_inf_marksperson.png,units/bf2_all_inf_soldier.png,units/bf2_all_inf_vanguard.png,units/bf2_all_inf_wookie.png,units/bf2_cis_inf_bdroid.png,units/bf2_cis_inf_magnaguard.png,units/bf2_cis_inf_sbdroid.png,units/bf2_imp_inf_atatcommander.png,units/bf2_imp_inf_atatpilot.png,units/bf2_imp_inf_darktrooper.png,units/bf2_rep_inf_ep3armoredpilot.png,units/bf2_rep_inf_ep3trooper.png,units/bf2_rep_inf_pilot.png,units/bf2_rep_inf_trooper.png
needs:
branch: falph
resume: landed 37098015 2026-09-29
created: 2026-09-29
---

## Why
The maintainer's INBOX note (2026-09-29 18:45) arrived while DSHN was landing; DSHN's goldens were committed (30c4a14) before
it was filed. DSHN keeps the disc gloss alpha on every sheet a specular-pass segment draws (the PS2's pass weight,
`analysis/re-spec-gloss.md` §7). The base pass writes that alpha into the frame, so:
1. **Pictures:** `render::capture::encode_png` keeps the RGBA plane for the goldens. A viewer that composites over white
   shows the droids as white ghosts. The RGB is right: the "viewed" check flattens to RGB. `compare_golden`
   (`game/tests/common/mod.rs`) compares channels 0..3 only, so the gate never saw the alpha.
2. **The window (unknown, maybe player-visible):** `render/src/device.rs:56` configures the surface with
   `caps.alpha_modes.first()`. If KDE Wayland + NVIDIA Vulkan lists PreMultiplied/PostMultiplied first, the desktop
   composites through the low alpha and the droids, dark troopers and KAM1 decks look see-through.
3. **Other readers:** the DSHN audit found that no pipeline blends by destination alpha (`shader.wgsl` `fs_spec` reads
   the base texel's alpha, not Ad). Re-check this for the water, blend, bloom/post and HUD passes.

## Evidence
- Planner check, 2026-09-29 19:0x, on the committed goldens:
  - `engine/goldens/units/bf1_cis_inf_sbdroid.png`: RGBA, 7.63 % of pixels have alpha < 200. That is the droid,
    and it is the same 7.63 % "changed" in 30c4a14's table.
  - `bf1_cis_inf_bdroid.png`: 3.86 %.
  - The stormtrooper: 0 %.
- The maintainer's numbers on the droid pixels: SBD alpha 255 → mean 31 (p10 0, p90 80); battle droid → mean 107.
- The walkabout already writes RGB for this reason (`encode_png_rgb`, defect reports C §0 / D (3)).

## Notes
- The maintainer can play now: the 18:26 bake has DSHN. If the droids look see-through in the window, that is item 2.
- Its own card rather than a DSHN reopen: DSHN's colour result stands (the PS2 pairs and the audit reproduced it).
