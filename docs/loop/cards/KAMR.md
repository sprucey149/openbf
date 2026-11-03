---
id: KAMR
title: Kamino bring-up (BF2 and BF1 KAM1): the sea's hard edge and flat far plane, far-scene pieces floating or upside down, invisible floors, misplaced objects — the maintainer's play session; parity bf2_kam1 22.5, bf1_kam1 26.6 (the two worst maps)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX 2026-09-30 11:10: he plays Kamino and sees "invisible floors/platforms, far image looking wrong, the sea
nearby is wavy but then there's a hard cutoff and it looks flat and a different colour, things aren't rendering or
appearing where they are supposed to". Kamino is also the loop's worst parity: `parity/latest.csv` bf1_kam1 26.6,
bf2_kam1 22.5 (all maps 11.6); BF2 KAM1 cp2_cp3 30.4 and BF1 KAM1 cp4_cp4 30.2 / cp1_cp10 29.9 / cp5_cp11 29.4 are four
of the five worst posts. Alpha: the showcase maps do not cover Kamino, and the maintainer plays it.

## Evidence
- The maintainer's frames `lead_tmp/playtest_0930/kam1_play_5.png` … `kam1_play_11.png`; Claude's port overview
  `kam1_port_overview_sea_boundary.png` (`--cam-overview`, default settings).
- **Sea edge:** a foam-wave disc around the platforms, then a flat dark-green plane to the horizon (`kam1_play_7`,
  the overview). BF2 KAM1: `NearSceneRange (0.8, 60, 90)`, `FarSceneRange 800`, no `LowResTerrain` sheet → what draws past
  90 m is the far scene. Leads: the ocean sheet's radius (`water.glb`, `ocean: true`), FARC (the far WATER pass), what
  the flat plane is (terrain base? a far water sheet with the wrong texture or fog?). PS2:
  `pcsx2/ref/bf2_kam1/pairs/cp5_cp5_post5_v4_sea.png`, `v5_sea`, `v6_sea` (dark rainy water, no edge).
- **Far scene:** a flat grey ellipse in the sky near (865,180) of `kam1_play_7`; an upside-down bell tower and dark poles
  hanging from the top of `kam1_play_8`; a sparse distant stilt city. Leads: a far piece with a flipped transform;
  UTAS's `bake::far_lowd` rule (UTA1 only so far); the sky dome (3 segments V-clamped on this map).
- **Invisible floors — the maintainer's answer (2026-09-30 ~18:45, no need to ask him again):** it is the **inclined floating ramps
  (runways) that connect the circular platforms**. He can WALK on them (collision is there) but the ramp's surface is not
  drawn. There are SEVERAL on the map (BF2 KAM1 at least; check BF1 too); he photographed one — it is in one of
  `lead_tmp/playtest_0930/kam1_play_5.png` … `kam1_play_11.png`, so look at those frames for the ramp. The census should find
  every walkable inclined surface with no drawn mesh under it; the likely leads are a ramp model the port skips or draws
  one-sided/back-face-culled from above, a mesh dropped by the bake or in a LOD/far-scene split, or a collision-only
  object.
  **The maintainer's two frames (2026-09-30 ~19:00):** (a) `kam1_play_11.png` — the ramp going DOWN directly in front of the player
  (toward the round platform ahead) is the invisible one: the rail lights and lamp housings draw, the walkway between and
  under them does not. (b) `kam1_play_7.png` — "above my head, a little to the right": a short spur that sticks out of a
  round platform with a turret on it; its RAILS render (the lit strip at about x 1030–1510, y 340–375 of the 2000-px view,
  floating over the sea, with the turret-like block at its right end near x 1815) but the main floor walkway under them
  does not. The maintainer's reading, not a census result; the pixel positions are mine from the image. Original note: the maintainer stands where nothing is drawn, or sees things where nothing should be (ask the maintainer for a spot
  via STATUS only if the census below cannot find it). KAM1's skipped classes are only `com_snd_amb_static` (80) and
  `kam1_prop_mist` (11): skipped models are not the cause. Related: BFC, ELFR2, SKIPC.
- **Misplaced objects:** the PXWB walk pairs `pcsx2/ref/bf2_kam1/walk/pairs/` (6 posts × 4 bearings + spawn views).
- **Also in the frames (fold in or card if real):** a short white diagonal streak at the left of `kam1_play_5` near
  (830,270) — an FXB laser ribbon or a stray draw? (The teal squad markers and the doubled "Follow me" line are SQMK.)

## Outcome (RE, 2026-09-30, lane 3) — `analysis/re-kamino.md` ([O], verifier 21 supported / 1 partly / 0 not)
Split into build cards; this card is closed. Report `reports/20260930_1930_card_KAMR.md`.
- **KSKY** (5): the sky dome x 0.6 (`Effect("Lightning")` SkyDomeDarkenFactor; RAM 153). Prototype parity
  bf1_kam1 26.62 -> 21.80, bf2_kam1 22.49 -> 19.20.
- **KSEA** (5): the sea ends at the far pass (whole 160 m tiles reaching 640 m), black behind, BF2's far ocean fogged;
  `pose_render` steps the ocean. **KSEAL** (4): BF2's ocean lit ((8,19,19) vs the port's (26-49, 38-57, 34-55)).
- **GRND** (5, re, deps STHOP): the one-ray ground query falls through floor slits onto the hidden disc; the PS2 stands.
- Far pieces: AUTHORED — `kam_bldg_tower1` hangs by design (2 instances) and is flipped to stand on the domes (5);
  the PS2 shows the same. Nothing to build.
- Existing cards that carry the rest of the parity gap: GLOWB (the decks' highlight; BPNT landed during the pass and
  brought BF2's decks level: bf2_kam1 22.49 -> 20.80 at 5c73167), new card **B1DK** (BF1's decks 1.5-2 x dark), WTHR (rain,
  bolts, flashes), LFLR (lamp flares, the post's beam), FOGN / LOD (far objects lighter on the PS2), BFC (BF1 cp0's black
  slab), TENS (BF1's y = 0 floor), TBIND (deck turrets), DUSTE (mist), BRGT (BF1 filter).
- The umbrella acceptance moves to the builds: Kamino <= 15 on both games is KSKY + KSEA + KSEAL + GLOWB + B1DK + WTHR;
  re-measure after each (`parity.py run --maps bf2_kam1,bf1_kam1`).
- NOT FOUND: the grey ellipse of `kam1_play_7`, and where the maintainer stood for `kam1_play_8/10/11` (no stored pose or probe
  view reproduced them; ruled out: a model with a missing texture, the campaign-layer sky pads, the sky shells).
  Default taken: none needed to build the four cards; a spot from the maintainer (or a frame with the debug overlay) would settle it.
- LOW, not followed: two small droids (health / ammo) on the PS2's lower pad at cp5 `v4_sea` / `v5_sea` that the port
  frame does not show.
- The floor census (`lead_tmp/kamr/tools/floor_census.rs`) is a scratch example; GRND commits it. All 32 maps:
  `lead_tmp/kamr/census/all_maps_cell1.md` — leads on MUS1 (collision 0.5-0.8 m over the drawn floor on 15,160 m2),
  END1, MYG1, UTA1 are on GRND's card.

- Planner 2026-09-30 21:45: the maintainer's "invisible floors" (the inclined ramps between the round platforms, INBOX 18:45) were NOT answered by this card's split — GRND is the floor slits. The ramps are card **KRMP** (M1 5, alpha).
