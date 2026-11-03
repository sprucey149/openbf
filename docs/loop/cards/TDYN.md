---
id: TDYN
title: BF2's runtime lights on the terrain: up to 4 non-Static omni lights (the command posts' glow, explosions) add falloff passes to a patch's top layer, and a projector light adds a textured pass (FUN_0056f208's lit branch); the port lights no terrain from them
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The PS2 lights the ground round BF2's command posts. The library shows a blue pool under BF2 YAV1's CP beam and a brighter
floor round NAB2's CPs: the PS2 is 1.2–2.1× the base rule there. The port's terrain gets nothing from such lights. BF1
has no such pass.

## Evidence
`analysis/re-terrain-lighting.md` §5 [O] (verifier E1–E4).
- FUN_0056f208's lit branch runs on a patch's top layer when the light set has a projector (+0x94) or omni lights
  (+0x78, count +0xa1).
- Each omni pass: colour = light RGB × HDR × max(0, 1 − dy²/range²), where dy is the light's height above the ground at
  its x, z. Texture 0x36462699, ALPHA 0x48 (additive).
- The projector pass uses the light's `Texture` and `PS2BlendMode`.
- In the terrain's prelit set only non-Static omnis count. No BF2 terrain map authors one (`lead_tmp/re/terl/light_census_bf2.txt`),
  so these are runtime lights.
- Pictures: `engine/artifacts/pcsx2/ref/bf2_yav1/pairs/cp3_landingzone_post1_v2.png`,
  `lead_tmp/re/terl/sheets/bf2_nab2__cp4_cp6_postA_v1.png`.

## Note (2026-09-30, FXB RE, `analysis/re-bolt.md` §5.2–§5.3 [O])
Most of this card's RE is answered there: texture 0x36462699 = `com_sfx_omniproj` (BF2 CORE.LVL, a 64×64 cos² blob);
the projection `FUN_0056ee88` looks straight down from 5 m above the light through a 90° frustum (a disc of radius
≈ 0.98·(5 + dy) m, whatever the range); per pass colour = rgb·HDR·max(0, 1 − dy²/r²)·128, vertex alpha 0x20; the gather
`FUN_00599040` (a patch takes a light whose range reaches its box; top 4 by lum·min(1, r²/4d²)/max(1, e²/3600); the
terrain draws 3). Ordnance is one runtime source: every BF2 bolt with a LightColor owns a non-Static omni (card
**FXBL** builds the pass for bolts). Still open here: which class owns the CP lights (HOT1 RAM: 6 omnis of range 8 / 16
at the posts, colours (200,100,100) / (1,86,213) / (40,165,104)). After FXBL lands, this card only adds those sources.

## Note (2026-09-30, BPNT RE, `analysis/re-point-lights.md` §3–§4, §8) — answered; build folded into BPNT
- **The CP lights' owner** is the CP's hologram (`com_holo_controlzone`, HoloLightRadius 8, HoloLightIntensity 1):
  - one non-Static omni at the building's `hp_hologram`, 0.32 m up;
  - colour = the team-relation table for the viewer: own (1,86,213), enemy (200,100,100), neutral (192,192,192),
    neutral locals (223,223,32); × I (fades, flicker dips).
- The other runtime omnis:
  - explosion LightFlash: min(1, 20·c/255), r 3 × LightRadius, +3 m, linear fade;
  - non-Static EntityLights, e.g. UTA1's tower flicker r 12;
  - flags / mines.
- HOT1's range-16 lights are EntityLights (`hothlight` OmniRadius 16), not CPs [D from AGENT_A's census].
- Every RE item of this card is answered (with `re-bolt.md` §5.3). The build — non-Static omnis on FXBL's terrain pass
  — is step 4 of **BPNT**'s build.
- Closed as merged (lane 3, reversible: reopen if BPNT drops step 4).
