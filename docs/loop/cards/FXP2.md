---
id: FXP2
title: Particle effects, part 2: game::fx runs the PS2 emitters (bursts, lifetime budget, 20 variants, Transformer closed forms) and draws PARTICLE/ANIMATED/BILLBOARD/SPARK/STREAK/GEOMETRY/EMITTER with the PS2 blends (re-fx §5-§6), replacing the placeholder quads
milestone: M1
value: 5
phase: build
status: done
deps: FXP1
files: engine/crates/game/src/fx.rs,engine/crates/game/src/fx/emitter.rs,engine/crates/game/src/fx/draw.rs,engine/crates/render/src/particles.rs,engine/crates/render/src/particles.wgsl,engine/crates/render/src/frame.rs,engine/crates/render/src/lib.rs,engine/crates/game/src/viewer/scene.rs,engine/CRATES.md
declares:
needs:
branch: fxp2
resume: landed b4a5d5b3 2026-09-30
created: 2026-09-30
alpha: yes
---

## Why
`game::fx` draws PROVISIONAL coloured quads; the PS2's effects are emitters with a well-defined runtime and draw
(`analysis/re-fx.md` §5–§6, [O], read in the ELF, the VU1 microcode and checked against RAM). This card makes the
port run and draw them; FXP3 wires what spawns them.

## Evidence
`analysis/re-fx.md` §5 (runtime: bursts, MaxParticles as a lifetime budget, the 20-variant spawner cache, spawn,
the Transformer's closed forms incl. the Move quirk, emitter colour, FadeInTime as an emitter fade-in), §6 (draw:
PARTICLE/ANIMATED squares √2·size′ wide rotated by π·frac(0.00314·rot), size′ = size·(1 + 0.2·clamp(lod,1,3)); the 4×4
atlas; BILLBOARD world planes by ⌊rot⌋; SPARK / STREAK ribbons; GEOMETRY models; ALPHA 0x44 / 0x48, TEST 0x51001 —
depth tested, never written, no fog; PARTICLE back-to-front; far fade 50/150/500 m by BoundingRadius over the last
20 %; density thinning), §8 (BF2 differences). PS2 frames of `ord_explosion` on grass:
`engine/artifacts/lead_tmp/fxp/ps2cap/crop_nab1_impact.png`.

## Notes
- The PS2 re-simulates particles from their burst history every frame; a stateful port that integrates the same
  closed forms at each particle's age is equivalent (re-fx §5.1).
- Modern where the PS2 was limited (decide, log in FEEL_NOTES): the 127-per-instance / 300-per-batch / 2000-per-frame
  caps and the split-screen halving are PS2 budgets.
- (FXP1, 2026-09-30) The data is baked: `effects.ron` (`manifest::FxSet`) in `common_<game>/`, every map dir and
  `side_bf2/effects/<side>/` (BF1 sides hold none) — look an effect up by `fnv1a_lowercase(name)` map → side → common.
  Values are the PS2's bit for bit (EE round-toward-zero; `mungefs::particle::ps2_mul/ps2_sub`). The emitter-level
  `hsv` is never read; the Spawner's `hsv` is the particles' mode. BF1 ignores `frame_size`/`looping` (fixed 4×4).
  Some named sheets exist in no loaded LVL (BF2 INGAME names boom4, explode3, flare0, redlaser, thicksmoke3,
  whitesmoke, dirt that no BF2 LVL holds; underlitesmoke2 only in GEO1/MUS1/MYG1) — the PS2 draws its fallback
  (re-fx §6.9 [H]); the bake prints each layer's list.

## Note from FXB (2026-09-30, lane 1)
Bolts now draw as the PS2's ribbons (`render::bolt`, a788a31): after the blend pass, far to near, core then glow per
bolt. The PS2 sorts bolts WITH the particle effects back to front (re-bolt §3.4): when the particle pass lands, merge
both into one far-to-near list (bolt key = head distance) rather than drawing one set after the other.
