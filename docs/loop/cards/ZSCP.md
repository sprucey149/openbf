---
id: ZSCP
title: The scope display: with a ZoomFirstPerson weapon zoomed (sniper rifles; BF2 also bowcaster, award and target pistols) the PS2 draws the weapon_scope quarter picture mirrored into four quadrants over the view and plays binocularzoomin / binocularzoomout while the zoom moves - the port's scope view (card ZOOM) has no overlay and no sound
milestone: M3
value: 4
phase: build
status: blocked
deps: ZOOM
files: engine/crates/ui/src/hud.rs,engine/crates/game/src/walk.rs,engine/crates/game/src/main/windowed.rs
declares: ci (+4 tests: hud_scope x2, walk x2); no golden, sim, sweep or bake move intended (the golden HUD/spawn tests only gain scope: None)
needs: cloud prerequisites: branch zscp, baked assets, missions and reference evidence absent
effort:
branch: zscp
resume: (private workflow note omitted)
created: 2026-10-06
alpha: yes
---
## Why
Card ZOOM's RE (analysis/re-zoom.md section 8, [O]): the PS2's scope is a HUD element of its own. Without it a zoomed sniper looks through a bare first-person camera.

## Evidence
- BF1 ScopeDisplay (update FUN_002d66a8 B1:262837-262969), BF2 twin FUN_0038f558: shown while the unit is alive, its zoom flag is set, the weapon is a first-person-zoom one (or the view is first person) and the camera's zoom S > 1.0; hidden otherwise.
- Four quads, each half the screen, one quarter picture (weapon_scope, baked for both games: assets_baked/common_bf{1,2}/textures/weapon_scope.ktx2) mirrored left / right and top / bottom. A look at it: engine/artifacts/lead_tmp/zoom/weapon_scope_bf1_mirrored_on_green.png (opaque dark surround, an octagonal window whose clearest alpha is 57 of 255, range ladders, a centre ring).
- Sounds: ScopeDisplayZoomIn / ScopeDisplayZoomOut = binocularzoomin / binocularzoomout (43 of 44 BF1 missions, 102 of 128 BF2); a rising S starts the first, a falling S the second; BF2 once per zoom movement. The rifle's shoulder zoom is silent.
- BF2's per-weapon ScopeTexture is stored and never applied on the PS2 (one scope texture on the disc).
- Not read (re-zoom.md section 11): whether the reticle is drawn inside the scope, what BF2 hides of its HUD under the scope, what Effect(ScopeBlur) draws (blocks in BF1 END1, GEO1; BF2 END1, FEL1, MUS1, GEO1, TAT3).

## Acceptance
- A pixel test: with the scope up the four quadrants are mirror images of each other and the frame's corners are the texture's opaque surround; with a rifle zoomed, or S <= 1, no overlay is drawn.
- A test that the zoom-in sound request is raised on the tick S starts to rise and the zoom-out one when it starts to fall, and neither for a weapon without ZoomFirstPerson.
- A PCSX2 pair: one PS2 frame of a marksman's scope (BF1 NAB2, class-right 2 in lead_tmp/zoom/ps2/zoom_probe.py reaches all_inf_marksman) beside the port's, which also settles the reticle question; the wide-window placement (the PS2 art is 4:3) is the build's presentation call, one line in FEEL_NOTES.

## Notes
Filed 2026-10-06 by the ZOOM RE session (lane 2). ScopeBlur is left out unless the pair shows it.
- 2026-10-07 (card ZOOM landed the scope VIEW): `Unit::scoped()` / `WalkSim::player_body_hidden()` are the display's test (zoomed on a `ZoomFirstPerson` weapon), `WalkSim::zoom_s` is the camera's S (the display wants S > 1; rising / falling S starts the two sounds). PS2 frame grabber for the pair: `engine/artifacts/lead_tmp/zoom/ps2/zoom_shot.py bf1|bf2` (R3 = key `4`; add `--class-right` as zoom_probe.py has it for the marksman). BF2's sniper was never reached live (re-zoom.md section 9): its second step and 0.6 m eye are [V] only.
- 2026-10-16 (card VTURZ built a building turret's zoom): a zoomed turret ALSO shows a scope frame on the PS2 (a hexagonal frame with range ticks: BF1 Naboo gun turret `lead_tmp/vturd/ps2/nab1_gty_7_zoom.png`, BF2 Mygeeto AA gun `myg1_zoom_2_zoom.png`; the same frame in both games, not the class's `FirstPerson` model - re-turret-draw 4.1, still unread which picture or code draws it). The port's test for a turret is `Sim::seat_zoom(player).is_some()` (the turret's zoom; `WalkSim::zoom_s` eases the same). Read what draws it when this card is taken and cover both cases; the turret's pair is `lead_tmp/vturz/mk_zoom_pairs.py`.
- 2026-10-22 03:14 (lane 1, build, branch zscp 1eab580, parked for the RTX): built. The PS2 pair (BF1 NAB2 marksman, class-right 2, S 2.0 and 8.0 read from RAM; `lead_tmp/zscp/ps2/zscp_shot.py`, SW session) settles section 11's reticle question for BF1: the HUD's bars, counts, minimap, weapon panel AND the reticle are drawn OVER the scope [M]; the picture spans the 4:3 screen; a dark one-PS2-line cross runs through the whole screen where the quadrants meet (most likely the GS bilinear wrap at the sheet's (1,1) edge; drawn by the port as `ui::hud_scope::SEAM`). Sheet `lead_tmp/zscp/pair/sheet_bf1_ps2_left_port_right.png`. Sounds: BF1 starts zoom-in at the show AND the next rising update (its show clears the latches), BF2 once per movement (latch +0x3a0); zoom-out at the hide; `FUN_003559b0(h, 0)` only drops the latch, it does not stop the voice (B1 decomp read this session). Not settled: BF2's scope frame (no sniper reached), the scope's opacity against the PS2 (a fit on the frame's floor was inconclusive; the bake keeps the disc's full-range alpha as for every HUD sheet).
- 2026-11-02 (maintainer decision): the port draws BF2's `weapon_scope` on every map, BF1 maps included, as with the rest of BF2's HUD. `game::hud_feed::scope_view` no longer picks a layer, and the BF1 common bake leaves out its copy (`bake::map::content::BF1_COMMON_UNUSED`). Nothing else in BF1 uses that sheet.
