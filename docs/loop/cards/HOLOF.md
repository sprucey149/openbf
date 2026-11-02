---
id: HOLOF
title: Command-post axial flare and authored scan-line animation
milestone: M1
value: 2
phase: build
status: done
deps:
files: engine/crates/game/src/holo.rs
declares:
needs:
effort:
branch:
resume:
created: 2026-10-22
---
## Why
The command-post beam was missing its axial flare, and its icon ignored
authored scan-line UV scroll. These are presentation-only gaps left by CPHOL.

## Evidence
`analysis/re-point-lights.md` §16 identifies HoloFlareIntensity, verifies
the native radius/alpha rule and reads every faction icon's material rates.
BF2 `FUN_00219148` sets class +0x84 from hash 0x4cecac28;
`FUN_00216948` copies it to the beam +0x194 read by `FUN_00469628`.

## Acceptance
- A unit test: a post seen from above (g < 0) draws one Flare with alpha A g^8 x the factor read from the class; from the side none (mutation-checked).
- The factor's key named from FUN_002172a8 / the holo class setter with re_check fn lines, or [H] with a RAM read of +0x194 on one BF2 save state.
- The scan-line animation: read the icon material's flags (MTRL) and the draw; build it or record that the PS2 does not scroll it.
- Goldens move only where a post is seen from above (likely none).

## Sprint implementation (2026-11-01)

The flare and per-material UV scroll are implemented. `HoloFlareIntensity`
(hash 0x4cecac28) is 1.0 on both discs; the fully axial radius is 0.75 m.
All eight native icon models were read: BF1 has float scroll rates; BF2's
Republic second segment scrolls and the other faction segments are static.
See `analysis/re-point-lights.md` §16 for the rechecked evidence.

The earlier reference to `entity_glow::flare` / `render::Flare` does not
exist at this source base; the hologram uses a procedural untextured
particle through the existing draw pipeline. Issue #68 is complete under
the sprint's scoped acceptance. The historical mutation, all-map golden
and paired original-game comparison remain unperformed, as recorded in
`reports/20261101_SPRINT_68.md`.
