---
id: FXB
title: Laser bolts, build: the ordnance class's own draw — two camera-facing tapered ribbons (core LaserTexture NORMAL + lightglow ADDITIVE, re-bolt §3-§4) from the weapon's fire point, replacing the port's boxes (the BF2 light is FXBL)
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/weapon.rs, engine/crates/game/src/roster.rs, engine/crates/game/src/walk.rs, engine/crates/game/src/main/battle.rs, engine/crates/game/src/viewer/upload.rs, engine/crates/render/src/(new bolt ribbon pass + wgsl), engine/crates/render/src/frame.rs, engine/CRATES.md
declares:
needs:
branch: fxb
resume: LANDED a788a31 + b21de48 (report reports/20260930_0524_card_FXB.md). Follow-ups: FXBL (the BF2 light, from bolt_vis's tracked bolts); FXP2 should merge bolts into its far-to-near particle list; a true hp_fire needs the weapon bake to keep GLB nodes
created: 2026-09-30
alpha: yes
---

## Why
The maintainer's INBOX 2026-09-28 (d) lists "bolts and tracers" with the effects "seen in every second of every battle". The
bolt itself is not an `fx__` effect (`analysis/re-fx.md` §0/§7): it is the ordnance (`bolt` / `bullet`) class's own
renderable, and the port draws it as a box (`game::walk::bolt_draws`). The PS2 frames in re-fx §9
(`engine/artifacts/lead_tmp/fxp/ps2cap/sheet_hot1_fire.png`, `crop_nab1_impact.png`) show soft glowing bolt heads and,
on BF2 HOT1, a large pink light wash on the snow around each bolt.

## Build acceptance (planner 2026-09-30 05:00; the RE acceptance above is met)
- Unit tests: the LaserLength rule (a PROP order setting LaserLength after GlowLength resets it to 3L, BlurLength 15L,
  capped 128) and the §3.2 strip geometry (half-widths w, w/2, cap; glow 2w -> 1.6w; U 0 / 0.25 / 1) against
  hand-computed values; `--sim-hash` unchanged (render-only).
- Pair: the port at `lead_tmp/fxb/ps2cap/bf1_nab1_mid3_pose.json` with the RAM bolt of that state — the glow ribbon's
  width and length in px within ±2 px of `bf1_nab1_mid3_boltcrop.png`; one BF2 HOT1 mid state the same way (the core's
  colour = the class's LaserGlowColor, no pink wash — that is FXBL).
- Goldens: none expected to move (no golden shows a bolt in flight); if one does, view it and declare it.

## RE result (2026-09-30, lane 2, report `reports/20260930_0224_card_FXB.md`) — the build
`analysis/re-bolt.md` (fb1717b, [O], verifier 23 SUPPORTED / 2 PARTLY folded / 0 NOT SUPPORTED):
- Two camera-facing tapered ribbons per bolt (§3.2): core = LaserTexture, NORMAL, half-width w -> w/2 over
  min(len, LaserLength) + a w cap ahead of the head; glow = `lightglow`, ADDITIVE, 2w -> 1.6w over min(len, GlowLength)
  + a 2w cap; w = LaserWidth*(0.9*fade + 0.1); U 0 at the cap tip, 0.25 at the head, 1 at the tail; nose-on alpha
  split; depth test, no depth write, no fog; vertex bytes /255 (no overbright).
- Colours (§3.1, §3.5): core white x fade; glow = LaserGlowColor (x fade when fading); BF2 fades it to
  EndLaserGlowColor between the damage-falloff start and end when FadeLightColor is set.
- Lifecycle (§4): drawn length from 0 at the spawn point at the class Velocity up to max(L, GL, BL); at the impact the
  head stops and the bolt is drawn 2 more updates, or fades over FadeOutTime (w -> 0.1 w). Render-only in the port.
- The light is NOT this card: BF1 bolts make none; BF2's terrain wash -> **FXBL**. The bake's 4-bit textures (the laser
  textures and lightglow included) are nibble-swapped -> **NIBL** (independent; either order).
- `purplelaser_D` (BF1 cis remote droid) is on no disc: draw `violetlaser_D` (PROVISIONAL, the intended art).
- The port spawns the sim's bolt at the shooter's chest (`Unit::muzzle()`); the PS2 streak grows from the barrel, so
  the drawn origin must be the weapon's fire point (render-side; the sim ray origin stays).
- PS2 references for the pairs: `lead_tmp/fxb/ps2cap/` (NAB1 + HOT1 knock-out stills and 30 fps recordings, mid-fire
  save states with RAM bolts, `bf1_nab1_mid3_boltcrop.png` for the ribbon's size at 14 m).
