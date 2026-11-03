---
id: TMRK
title: World markers, part 2 — the feed: which entities the PS2 marks as attacker / locked / hint / hint-flag / objective / flag (the per-player tracker FUN_00179278, fills FUN_0017a908 / FUN_0017a900, the objective list DAT_0066af58 by FUN_0017cc00, Lua MapAddEntityMarker), their record distances (max / near / far) and when they expire — RE, then feed BMAP's Target element
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BMAP's RE (`analysis/re-hud-map.md` §5, 027dad9) read how the Target element DRAWS a marker record (styles by on-screen /
line of sight / kind, the ±0.9 NDC off-screen clamp, MinScale→MaxScale between the record's near / far distances, the
lock-on brackets) but not WHO gets a record: the per-player tracker (`*(DAT_00825210 + 4·player)`, 64 records at +8 ×
0x1c: entity, generation, team mask, max / far / near distance; marker states at +0x970) and the global objective list
(DAT_0066af58, 32 × 0x1c). Without it the port's world markers would be guesses: which attacker gets the red hint arrow
and for how long, what the lock-on marks, when a flag carrier or an objective shows.

## Evidence
- Fill: `FUN_00179698` (per frame) → `FUN_0017a908` / `FUN_0017a900`, the record pass `FUN_00179f60`, projection
  `FUN_00179c10`; list management `FUN_0017cc00` (objectives), `FUN_00179278` (tracker ctor); Lua `MapAddEntityMarker`
  / `MapRemoveEntityMarker` (grep the BF2 decomp's Lua callback table).
- No saved state holds a live record (spawn states): a PCSX2 session that gets the player shot / locked on (BF2 Conquest
  with AI) is needed to see records in RAM (`lead_tmp/re/bmap/ram.py` reads the tracker).

## Acceptance
- `analysis/re-hud-map.md` §5 (or a new re-hud-markers.md) lists, per marker kind, the trigger, the entity, the record's
  max / near / far distances and its lifetime, each [V] with re_check fn/absent/writers, one Opus verifier.
- One PCSX2 frame with an attacker or lock-on marker whose record (RAM) and on-screen position (±3 px) match the model.

## Notes
- Created 2026-09-30 by BMAP's RE session (lane 2). BMAP's build can draw markers from the port's own events meanwhile
  (PROVISIONAL), and switch to this feed when it lands.
