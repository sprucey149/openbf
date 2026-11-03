---
id: FXBL
title: BF2 bolt lights: every BF2 bolt owns a non-Static omni light and the terrain adds com_sfx_omniproj projected straight down from 5 m above it (a ~5-7 m disc, x max(0,1-dy^2/r^2), vertex alpha 0.25) — HOT1's pink wash (re-bolt §5.2-§5.3); the port lights nothing
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX 2026-09-28 (d) (bolts, every second of every battle) and FXB's RE: the big pink wash round every red bolt
on BF2 HOT1's snow is the bolt's LIGHT on the terrain, not the bolt or a particle. Proved on the PS2: zeroing every
laser class's LightColor in live RAM removes it (frame-mean pink 1.9 -> -0.2), zeroing the glow does not
(`lead_tmp/fxb/ps2cap/sheet_hot1_knockout.png`); the §5.3 model predicts the PS2 frame from RAM pixel by pixel
(corr 0.933, slope 0.91 over 125,600 px, `lead_tmp/fxb/ps2cap/bf2_hot1_mid3_washcheck.png`).

## Evidence
`analysis/re-bolt.md` §5.2-§5.3 [O]:
- light: created when LightRadius > 0 and the LightColor alpha byte != 0; colour = LightColor bytes/255 (already x A/255
  by the setter), range = LightRadius; at the midpoint of the bolt's last step; with FadeLightColor lerps to
  EndLightColor over the damage-falloff window (age 0.4 -> 0.6 s on rifles); at the hit moves to hit + 0.5*normal;
  deleted with the bolt (one frame after an ordinary impact);
- terrain pass per light (<= 3 per patch after the top-4 gather by lum*min(1,r^2/4d^2)/max(1,e^2/3600); a patch takes a
  light when its box is within r): additive (ALPHA 0x148), depth tested, no depth write, no fog, CLAMP; colour =
  rgb*HDR*max(0,1-dy^2/r^2)*128 (dy = light height over the heightfield), vertex alpha 0x20 (0.25); texture
  com_sfx_omniproj (BF2 CORE.LVL, 64x64 grey cos^2 blob, NOT baked yet) projected straight down from 5 m above the light:
  s = .5+.5(L.z-P.z)/z', t = .5+.5(P.x-L.x)/z', z' = L.y+5-P.y -> a disc of radius ~0.98*(5+dy) m. On screen ~ c*f*T/4:
  a rifle bolt 1 m over snow adds ~+32 R / +4 G,B at the centre.
- BF1: bolts make NO light (fields dead, BF1's terrain light pass gated off) — nothing to do there.
- Models: BF2 models take the same lights (§5.4) — that is BPNT's path; not this card.

## Build notes (2026-09-30, lane 1)
- Built: `LaserLook` light keys (LightColor/EndLightColor setter, LightRadius, the ctor gate, the End fade);
  `BoltVis::omni_lights` (in flight: midpoint of the last step; reach update: midpoint of the last step; hit
  update: hit + 0.5·normal; FadeOutTime remnants lit while fading); `bolt_light::gather` (8-cell patches, box ≤ r,
  rank, first 3); `render::terrain_light` (omniproj projected down from L.y+5, CLAMP bilinear, ×c·f, As 0.25, added
  in gamma over a frame copy taken after the terrain's layers); `bake omniproj` → `common_bf2/com_sfx_omniproj.ktx2`.
- Found on the way: a bolt the sim spawns and resolves in one tick (a shot at the ground ≤ 5.3 m ahead) never
  reached `sim.bolts`, so neither FXB's ribbon nor its light was drawn; BoltVis now keeps it from the tick's Shot +
  struck BoltExpired events.
- Measured (scratch `lead_tmp/fxbl/`): HOT1 mid3 still, the RAM bolt light (6.15,0.52,−1.08): port added pink vs
  washcheck's prediction corr **0.981** (slope 0.90), vs the PS2 frame's pink corr 0.927 (PS2 = 0.98·port − 0.79).
  Firing pitched down (p8, 60 ticks): per shot 7.6 / 3.5 / 4.3 frame-mean pink (PS2 9.5 / 3.2 / 2.8); added
  frame-mean pink over the firing span **+3.04** (PS2 ctl/ctl2 +3.28/+3.04); area > 6 0.175 (PS2 0.24/0.23 — the
  port's snow sits 1.3 bluer at baseline, −1.5 vs −0.2).
