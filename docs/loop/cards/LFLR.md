---
id: LFLR
title: BF2's lamp glows that are not light: EntityLight lens flares (the red haze at UTA1's lamp heads), light cones (LightBeam: UTA1's lamp cones, DEA1/HOT1/KAM1 cones) and the EntityLights' own flicker; the port draws none (the command post's hologram went to card CPHOL, 2026-10-12)
status: archived
issue: 68
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #68](https://github.com/sprucey149/openbf/issues/68).

## Why
BPNT's RE (`analysis/re-point-lights.md` §10) split the card's "lamp glows" into light (BPNT) and three additive draws
the PS2 makes beside the lights. The PS2 frames show them on the showcase map UTA1 and on every BF2 conquest post:
- **The red haze at UTA1's lamp heads.** `engine/artifacts/pcsx2/ref/bf2_uta1/walk/pairs/cp2_con_cp1a__sheet.png` b180,
  `cp5_con_cp6__sheet.png` b180. It is `uta1_post_flare`'s lens flare plus `uta1_post_spot`'s light cone.
- **DEA1 cp3's red column.** `pcsx2/ref/bf2_dea1/walk/pairs/cp3_cp3__sheet.png` b000 (210,60,330,400): PS2
  (94,68,74) vs port (35,37,43). It is the enemy CP's hologram beam.
- **The white columns at neutral CPs.** UTA1 cp2 b180 SW frame: `lead_tmp/bpnt/frames/u180__ctl.png`.
- **The spinning faction icons over owned CPs.**

The knock-outs prove these are not lighting: they stay when every model point light is patched out
(`lead_tmp/bpnt/frames/sheet_nopoint.png`).

## Evidence
`analysis/re-point-lights.md` §2 and §10, read by helpers A and B (`lead_tmp/bpnt/AGENT_A.md` §3–§5, `AGENT_B.md` §2).
The lead re-read the GS words.

- **Lens flare** (EntityLight FlareIntensity > 0; manager "LensFlare" FUN_00466d08, geometry FUN_00468238 / FUN_00468a28):
  - Procedural, camera-facing, in view space:
    - two 3-point stars, tips (2R, 4R) and (7R, 2R), angle θ = R·π/3 + π(x + 2y)/z;
    - a 7-segment ring, radius 0 → R → 4R.
  - Colour and size:
    - vertex RGB/2; alpha A·FlareIntensity/2 at the centre, /4 at R, 0 at 4R;
    - R = (|ConeInitialWidth0| + |ConeInitialWidth1|)/4, which is 0.5 m by default.
  - GS state: ALPHA 0x48 (additive), Z GEQUAL, no Z write, no fog.
  - Alpha × the visible fraction of a 2R test quad (a soft occlusion read back through a 64×16 buffer); culled past
    DrawDistance (200 m).
  - With a cone, the flare shows only along the cone's axis: alpha × c⁸.
- **Light cone** ("LightBeam" FUN_00469628, ALPHA 0x48, TEST 0x51001, untextured Gouraud):
  - Shape: a cone along the hardpoint's local +Y. Length ConeLength·(1 − ConeFadeLength) plus a fade tail. It widens
    from ConeInitialWidth/2 to ConeWidth/2.
  - Colour and alpha: colour = the class Color; alpha ∝ ConeFadeFactor·(1 − (v·axis)²).
  - uta1_post_spot: 8 m down, half-width 1 → 4, (60,30,27).
- **CP hologram** (FUN_002186c8 → the same LightBeam renderer):
  - **Beam:** 3.5 m tall, radius 0.5 → 1.0 m, the viewer's team-relation colour (own (1,86,213), enemy (200,100,100),
    neutral (192,192,192)); alpha I·0.25·(1 − cos²).
  - **Icon:** the owner's `com_icon_*` model (texture `com_icon_controlzone`, translucent, no Z write), unlit, alpha
    I·0.5, spinning at 1.2566 rad/s at HoloHeight 3.5 m. A neutral CP has no icon.
  - **I:** 2 s fades on a capture, random pops and flicker dips.
- **Flicker** (FlickerType) modulates flare and cone like the light (BPNT §2).

## Acceptance
- UTA1 cp2_con_cp1a b180 (the SW frame `lead_tmp/bpnt/frames/u180__ctl.png`, or the library pair): the port shows the
  red haze round the lamp head and the downward cone.
- DEA1 cp3_cp3 b000 box (210,60,330,400) moves from (35,37,43) toward the PS2's (94,68,74), within ~15 levels.
- A neutral CP's white beam and an owned CP's spinning icon are visible at UTA1 cp2 / cp3.
- The flare hides behind geometry (the soft occlusion or a depth-tested equivalent).
- Modern-PC choices go into FEEL_NOTES if made: e.g. per-pixel occlusion instead of the 64×16 readback.
- The declared goldens move; sim hash unchanged (render-only).

## Notes
- Depends on BPNT for the baked EntityLight instances (the carriers' AttachOdf hardpoints) and the CP hologram lights'
  runtime colours.
- The open [H] items are in re-point-lights §12: stage order; 0x1de206a6, the beam's base width.

- 2026-09-30 (BPNT build): what the build left for this card. The bake keeps a `Light` class only as far as its omni
  (`bake::entity_light::LightClass`: colour, OmniRadius, Static) and writes a prop's attached lights only when
  OmniRadius > 0 (`ManifestClass::lights`), so `uta1_post_spot` (the cone, OmniRadius 0) is NOT in the manifest: extend
  `light_class` / `attach` with FlareIntensity, ConeLength / ConeWidth / ConeInitialWidth / ConeFade* and keep the
  radius-0 rows (a new field or a second list). The posts' light positions are `game::point_light::command_post`
  (post + 0.32 m). The red pool under UTA1's lamp is in: floor (0,330,200,470) 159/55/44 against the PS2's 151/71/63 —
  the missing green and blue there are this card's cone and flare.

## Note (2026-10-03, cleanse bake, card CLNB)
- The EntityLight's OWN flicker (`FlickerType` / `FlickerPeriod`, `analysis/re-point-lights.md` §2 "Flicker", B2
  `FUN_002278f8`) is read but neither baked nor run: `bake/src/entity_light.rs` bakes a steady omni, so UTA1's
  `uta1_bldg_tower_flicker` (StrobeRandom 0.75) does not flicker. No other card owns it; this card's flare and cone
  take the same intensity I, so build the light's flicker here with them (the class fields into `ManifestClassLight`).

## Note (card B1DK's RE, 2026-10-05)
- `dea1_light_flicker` (200,200,255) r 24, StrobeRandom 0.75, Synchronize, hangs on `dea1_bldg_beam_generator` at a
  hardpoint the model lacks, so the PS2 makes it at the generator's origin (-105.0, 49.98, -107.93); card B1DK bakes
  it steady. It is one more flicker light for this card's list.
- A post's hologram (light, beam, icon) sits at the post BUILDING's `hp_hologram` bone: 0.320 m up on
  `com_bldg_controlzone`, 1.363 m on the large post (`lead_tmp/b1dk/re/hp_census.py`) - the port's constant is 0.32
  for both. `analysis/re-kamino-decks.md` §6.

## Note (planner 2026-10-12 23:40): the hologram is card CPHOL's now
- The maintainer's INBOX note of 19:05 asked for the command post's hologram soon: its beam, icon, fades and the DEA1 cp3 /
  UTA1 cp2-cp3 acceptance lines above moved to **CPHOL** (value 5). This card keeps the lens flares, the light cones
  and the EntityLight flicker at value 4. CPHOL builds the LightBeam cone draw first; start from its module here
  (`deps:` now names CPHOL so the cone is not built twice).

- 2026-10-13 07:40 (CPHOL landed dcfd692): the post's beam and icon are built (`game/src/holo.rs`, `render/src/light_beam.rs`); a
  lamp's light cone can reuse `render::Beam` (the same renderer, re-point-lights 15). Left for this card from the
  hologram: its flare (for g < 0 the beam's +0x124 alpha = A g^8 x HoloFlareIntensity, size +0x120, 15.6) and the
  icon sheet's UV animation (scan lines). The post light now takes the hologram's intensity (`point_light::dimmed`).

## Build (2026-10-22, lane 1): built, parked on the lost RTX
- Branch lflr 6ea5b03: bake glow keys + axes (`bake/src/entity_light.rs`, `ManifestLightGlow`), `render/src/lens_flare.rs`
  (ParticleShape::Flare), the cone = the lightbeam fan with `top` = ConeFadeFactor, `game/src/entity_glow.rs` (lamps,
  flicker, the flickering omnis at I). re-point-lights §16 holds what was read.
- Acceptance run: 8 new unit tests (2 mutations caught: the coned flare's sign, the Synchronize share); the UTA1 bake replay
  gives uta1_post_spot's cone 8 m down the hardpoint's +Y (axis y < -0.9), half-width 1 -> 4, factor 1 (RAM: len 6.4 + 1.6,
  half-w 1 -> 4, CFF 1). Parity over the 11 BF2 maps whose manifests change (scratch maps bake, `lead_tmp/lflr/parity_branch`
  vs main 9193a4c): UTA1 7.012 -> 6.875, DEA1 8.155 -> 7.952, the rest within +-0.04. UTA1 cp2_con_cp1a b180 box
  (0,0,200,330) round the lamp: PS2 91.9/78.3/76.4, main 70.0/64.7/66.8, branch 81.5/71.2/69.6; the haze and the
  downward cone are visible (`lead_tmp/lflr/lflr_sheet.png`, `diff_cp2.png`), at about half the PS2's added level (the
  additive blend in linear light: card ADDB). The flare is cut by the lamp dish's depth like the PS2's GEQUAL test.
- Left: the hologram's own flare + icon UV animation -> card HOLOF (value 2).
