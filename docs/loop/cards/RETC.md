---
id: RETC
title: The BF2 reticle's colours and place against PS2 frames: BHUD2 draws it (white / BF1's RAM red and green, at the window centre); capture a PS2 frame over an enemy, a friend and nothing, and match the RGB and the offset
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The port's HUD (`ui::hud`) has no reticle at all, on any map. On the PS2 the reticle is always drawn. It turns red over an
enemy and green over a friend out to the weapon's ExtremeRange (rifle 120 m), inside the acquisition cone. The colour
is the relation of the PlayerController loop's pick: B1 FUN_002d4b50 reads K+0x40+4·ch (B1:262194-262201) —
`analysis/re-aim-assist.md` §3.7 [O], card AIMF.

## Evidence
- `re-aim-assist.md` §3.7: the colours from RAM are enemy (200,100,100), friend (100,200,100), default (67,167,242).
- ps2live measured the crosshair at screen (320, ~178) of 640×480, ~5° above the camera axis.
- Weapons name a `ReticuleTexture`, e.g. the mortar's `reticule_rocket`.
- PLAN (the maintainer, 2026-09-03): BF2's HUD on every map. BF2's twin loop (B2 FUN_002b0548) keeps a separate 0.25-slope
  HUD pick at K+0x190 and the relation at K+0x1a0 (§3.9).

## Do / Acceptance
1. RE (bounded): BF2's HUD reticle draw — its texture per weapon, its size, where it sits on screen (4:3 → Hor+ anchor
   per COMPARISON_RULES), its colours, and which pick it colours by (B2 +0x190 / +0x1a0).
2. Then a build: draw it in `ui::hud` from the sim's reticle pick (AIMF keeps `reticle_pick[slot]` + relation).
3. A PCSX2 frame over an enemy and one over a friend as the reference (`ps2live.py` reads the colour already).
4. Decoration only: no sim move.
## Note from AIMF's landing (2026-09-28, 73632e1)
The pick and its relation are in the sim: `Sim::reticle_pick[slot]` = `Option<(UnitHandle, i8)>` (−1 enemy, 1 friend),
written by `acquire_magnet` every tick (not hashed). Units only — the PS2 also picks buildings/vehicles/props (§3.9's GEO1,
HOT1, YAV1 picks); the port's candidates are soldiers. The magnet soldier's pink tint in `game::soldier_vis::draws` is the
port's, not the PS2's — remove it when the reticle lands.

**Note (planner 2026-09-28):** the reticle is one element of the BF2 HUD (card BHUD, M3 3); draw it in the BF2 HUD's style so BHUD does not redo it.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — the reticle is part of the real HUD. Value 4 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Note from BHUD's RE (2026-09-30, `analysis/re-hud.md` §3)
BF2's reticle is DATA: `1playerhud` Group `player1reticule` at Viewport (0.5, 0.410624), moved by the event
`player1.weapon1.reticule.position`, α 0.65; Model3D meshes `hud_main_reticule` (Scale 0.26, α 0.52, colour from the
event `player1.weapon1.target.teamColor`, ColorChangeRate 0.001), `_bullseye`, `_outline` (black α 0.5), hit / critical-hit
flashes (rgb (1,86,213), hold 0.33 / 0.6 s), and BarSegmented rings: the clip (30 × `hud_clipbar_segment`, −135.3°…144.8°,
value `totalClipFraction`, shown 6 s after a change), heat (30 × `white`, −150°…150°) and refire bits. All meshes are
already baked (`common_bf2/models/hud_main_reticule*.glb`). On the PS2 frame (HOT1 postA v1) the outline measures 76 × 70
of 640 × 480; re-hud §2.4's mapping predicts 76.2 × 73.4. Not traced: what colours `target.teamColor` carries and when (the
publisher is `FUN_003b3398`'s target half). Suggest: BHUD2 draws the reticle group; this card keeps the colour feed
(which RGB for enemy / friend / none on BF2, re-aim-assist §3.9's +0x1a0 relation) and the PCSX2 over-enemy / over-friend
frames.

## Planner note (2026-10-02 04:30) - re-scoped after BHUD2
BHUD2 (ea3883c) draws the reticle group from the baked HUD tree and colours it by `Sim::reticle_pick`; the pink magnet
tint is gone. Items 2 and 4 of "Do" are done and the draw half of item 1 is `analysis/re-hud.md`. What is left
(value 4 -> 3, phase measure, the title rewritten):
1. One PCSX2 batch on a BF2 map: the reticle over an enemy, over a friend and over nothing (dump the live element
   first: `lead_tmp/bhud2/ram/m3d.py`). Acceptance: the port's reticle RGB in the same three cases within +-12 per
   channel of the PS2 frame's. BHUD2 took white for "nothing" from BF2 RAM and BF1's (200,100,100) / (100,200,100) for
   enemy / friend, PROVISIONAL on BF2.
2. The place: the PS2 group sits at Viewport (0.5, 0.4106), about 5 degrees over the camera axis; BHUD2 put it at the
   window centre because the port's camera looks along the aim (FEEL_NOTES). Measure where a PS2 bolt lands against
   the reticle at about 20 m and 80 m and say whether the port's bolt lands under its reticle the same way. No change
   unless it does not.
Close the card with the numbers if both hold. Reverse: `loopctl.py set RETC value=4 phase=re`.

- Planner 2026-10-02 14:45: AIMA (value 5, the maintainer's note) moves the reticle to where the shot's ray meets the screen. This card keeps the colours, texture and size only; measure after AIMA's build.

- Note from AIMA's RE (2026-10-02, `analysis/re-aim-assist.md` section 10.4): the reticle's PLACE is settled and is
  AIMA's build. Both games draw it at x = 0.5 and y = the projection of the aim origin + 1024 m along the aim (B1
  FUN_002d4b50, B2 FUN_0038e598 -> `player1.weapon1.reticule.position`): line 178.48 of 480 on BF1, 153.35 on BF2, at
  every pitch (the game's own stored value). Item 2 of the 04:30 re-scope ("where a PS2 bolt lands against the reticle
  at 20 m and 80 m") is measured: under the reticle within 2.5 lines out to the ray's reach (65 m), and 47-64 lines
  BELOW it when a wall or the ground is nearer than 5.5 / 5.2 m along the ray (section 10.5). For the colour batch: BF2's
  reticle mesh is not symmetric about its anchor - its drawn box is x 284..358, y 113..179 around the anchor at y
  153.35 (the box centre is 7 lines above the anchor).
