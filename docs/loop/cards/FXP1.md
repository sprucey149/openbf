---
id: FXP1
title: Particle effects, part 1: mungefs parses fx__ ParticleEmitter trees (every key of analysis/re-fx.md §2, both games' layouts, §3's rules) and the bake carries each LVL layer's effects (COMMON/INGAME, sides, maps)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The real particle effects (the maintainer, INBOX 2026-09-28 (d)) — impacts, explosions, trails, vehicle damage — are `fx__`
`ParticleEmitter` trees the port does not read. `analysis/re-fx.md` (card FXP's RE, [O]) cracked the whole format and
checked the PS2's in-memory layout field by field against RAM in both games. This card is the data half: parse + bake.

## Evidence
`analysis/re-fx.md` §1–§4 (key table §2 with every hash, B1/B2 offsets and defaults; §3 parse rules; §4 RAM checks
`lead_tmp/fxp/ramcheck2.py` / `ramcheck_bf2.py`). Census `lead_tmp/fxp/census2.py`: BF1 128 effects / 701 emitters,
BF2 292 / 1702. Where they live: BF1 COMMON.LVL (109) + maps; BF2 INGAME.LVL (162) + sides + maps (§1).

## Notes
- The environment `Effect("…")` records in the same chunks stay unparsed here (FXC / WTHR / water).
- FXP2 (runtime + draw) and FXP3 (spawn hooks) follow; FXMF / FXB are the muzzle flash and the bolt.
