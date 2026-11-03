---
id: FXP4
title: Particle effects, part 4: GEOMETRY emitters draw their model (debris chunks: BF1 39 / BF2 212 emitters) and EMITTER emitters replay their child per parent particle (BF1 48 / BF2 85) — re-fx §5.10, §6.6; FXP2 draws every other type
status: archived
issue: 49
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #49](https://github.com/sprucey149/openbf/issues/49).

## Why
FXP2 landed the emitter runtime and the PARTICLE/ANIMATED/BILLBOARD/SPARK/STREAK draws; GEOMETRY and EMITTER emitters
are skipped (`game::fx::draw` module doc), so explosions lose their debris chunks and child trails.

## Evidence
`analysis/re-fx.md` §5.10 (EMITTER: each parent particle, up to 31, replays the child template along its path in 0.3 s
steps, tinted by the parent's colour — [D] outline, AGENT_A 7: read FUN_002b34f8 before building), §6.6 (GEOMETRY: the
model at the particle, scaled by size, no sizeMul, turned about Y by the raw rotation in radians, under the emitter
node matrix; missing model → nothing; ADDITIVE flag 0x400001 B1 / 0x800001 B2), census table §2.

## Acceptance
- A unit test per rule (GEOMETRY transform: scale = size, yaw = raw rot rad; EMITTER: ≤ 31 parents, 0.3 s steps, parent tint).
- A `fx_capture` sheet of one GEOMETRY effect (a BF2 vehicle explosion) and one EMITTER effect, viewed beside a PCSX2 crop.
- Gates: nothing moves but ci counts (visual only, no golden frames a live effect).
## Notes
- FXMF RE (2026-09-30, `analysis/re-muzzle-flash.md` §4.3, card ADDM): the GEOMETRY models the census names include
  additive segments the bake stored OPAQUE with their alpha dropped (BF2 `com_sfx_lightningball1-3`, `geo_blast_ring`:
  MTRL 0x10044 → GS 0x48, no Z write). FXMF builds the additive tinted model draw these want; ADDM keeps their texture alpha.

- FXMF build (2026-09-30): `render::ParticleShape::Mesh` + `game::soldier_vis::FlashModel` (load a common GLB's triangles
  and sheets, place by a matrix, emit additive particle draws) is the reusable path for GEOMETRY particles.
