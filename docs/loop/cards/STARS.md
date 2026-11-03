---
id: STARS
title: DomeInfo Stars(): the PS2 draws a star field in the sky pass (after the dome: BF1 FUN_002e7d78, BF2 FUN_003c10a8, flags 0x20031) on BF1 KAS1, RHN2 and BF2 COR1, NAB2; the port draws no stars
status: archived
issue: 16
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #16](https://github.com/sprucey149/openbf/issues/16).

## Why
BF1's sky tick draws the `DomeInfo` `Stars` object right after shell 0 (the dome): `FUN_002e7d78(sky+0x104, 0x20031)`,
B1 280285–280288 (`FUN_002f2308`). BF2 does the same with `FUN_003c10a8(sky+0x285c, 0x20031)`
(`analysis/re-sky-dome-placement.md` §1). The loaders parse the block (B1 `FUN_002e29e0` → `FUN_002e6a50(sky+0x104, …)`, B2 `FUN_003bfc30`, 416521–416533).
The port has no star code: grep `stars` under `engine/crates` finds only a doc line in `mungefs/src/sky.rs`.
Found by card REFL's RE, 2026-09-29, lane 2.

## Evidence
- The `Stars` key, fnv1a-32 0x622b0026, occurs in BF1 KAS1.LVL and RHN2.LVL, BF2 COR1.LVL and NAB2.LVL, plus BF2
  GAL1 and SPA_SKY (raw byte search of every .LVL, `lead_tmp/re/refl/`).
- KAS1 authors: 0x836bde54(300), 0x69bf5463(1), 0xa435aaa4(0), 0xa9eee423(0), Color(255,255,255,255),
  0xda689440('fx_star'), 0x736837de(30), 0x7de4197d(5), 0x0d7ec5c0(0.2). `lead_tmp/re/refl/skykeys.py dump` prints them.
- Likely a count of 300, a texture `fx_star`, sizes and a twinkle. The preimages are not cracked.

## Acceptance
RE first, at max effort, then build:
- what the star draw builds: count, placement over the dome (random? elevation band?), size, colour, blend, twinkle;
- whether it shows on these maps at all (KAS1 is a dusk sky): capture a PCSX2 frame looking up on KAS1 and on RHN2;
- then a build card with the goldens it moves.

## Notes
Value 2 is PROVISIONAL, a small visual on four maps. Raise it if the PCSX2 frames show a visible field.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
