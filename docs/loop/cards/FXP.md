---
id: FXP
title: The real particle effects: muzzle flash, bolts and tracers, impacts on ground/walls/water, explosions — game::fx draws PROVISIONAL coloured quads (its 'HOLD FOR FABLE 4' is stale); RE the PS2's fx__ format and emitters, then build
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer (INBOX 2026-09-28 (d)): the real particle effects — muzzle flash, bolts and tracers, impacts on ground, walls and
water, explosions — are seen in every second of every battle. `engine/crates/game/src/fx.rs` draws PROVISIONAL coloured
quads and its header says "HOLD FOR FABLE 4" (planner read line 6): that hold is stale; there is no Fable on openbf now,
so it is the loop's own §5 RE.

## Notes
- Created 2026-09-28 by the planner. FXC (census of the other unparsed chunks: `shdw`, `CLTH`, `bnd_`, `font`) keeps the
  rest; its `fx__` part moves here. VDTH's explosions should consume this.

## Outcome (2026-09-30, RE session, lane 2)
RE done: `analysis/re-fx.md` ([O], commit 515b024; one Opus verifier 34 SUPPORTED / 6 PARTLY folded in / 0 NOT
SUPPORTED). Split: **FXP1** (mungefs parse + bake, value 5) → **FXP2** (runtime + draw; removes fx.rs's stale HOLD
note) → **FXP3** (spawn hooks + the PCSX2 pairs); **FXMF** (muzzle flash: three hardcoded presets, not an `fx__`
effect — RE) and **FXB** (the laser bolt's own draw — RE). PS2 reference frames of real impacts:
`engine/artifacts/lead_tmp/fxp/ps2cap/`. Report `engine/artifacts/loop/reports/20260930_0222_card_FXP.md`.
