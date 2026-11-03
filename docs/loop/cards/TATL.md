---
id: TATL
title: BF1 Tatooine (showcase TAT1, and TAT2) is 12-15 luma darker than the PS2: parity median dluma -15.1 on TAT1 (20 of 29 pairs below -10), -12.3 on TAT2, the largest systematic brightness gap on a showcase map — shaded walls read blue-grey where the PS2's are pink-lilac; attribute it by surface (terrain, buildings, sky, soldiers) and card the cause
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
TAT1 is an M-ALPHA showcase map, and PARITY (landed 144b2f0) says it is the darkest of all 32 maps against the PS2:
the port reads 12-15 luma below the PS2 across the whole map, not at one post. A player sees a dull, grey Tatooine.

## Evidence
- `loop/parity/latest.csv` at d748ac2: TAT1 29 pairs, dluma (port - PS2) mean -14.3, median -15.1, 20 of 29 below -10,
  range -32.7 .. -2.5; dp99 -32.4. TAT2 29 pairs, mean -12.3, dp99 -29.5. The other showcase maps: NAB1 median -4.8
  (dp99 -37.4), UTA1 -6.7, YAV2 -5.9, MYG1 -3.6, HOT1 +7.4.
- Worst pair walk/cp3_cp2_b180 (-32.7): `lead_tmp/planner_0830/tat1_cp3_cp2_b180.png` (PS2 left, port 09-29 right): the
  shaded wall is pink-lilac on the PS2, blue-grey and darker in the port; the lit sand is paler/brighter on the PS2.
- The 09-29 port frames in `pcsx2/ref/tat1/walk/port/` predate NIBL/LEAF; re-render at main first (memory: re-render stale
  defect pictures first).

## Notes
- Planner 2026-09-30 08:30: opened from the PARITY scoreboard (the first planner to rank by it). Measure only: no code.

## Note (2026-09-30, GLOW RE, `analysis/re-glow.md` §3) — the cause is found: card BFLT
Every BF1 map runs the DomeInfo Filter on the PS2 (TAT1: Threshold 50, Intensity 50): frame += 50/128·max(0, frame↓2 − 50).
Measured on the PS2: TAT1 frame luma 144.5 with it, 119.5 without. Applied to the port's 24 TAT1 walk frames it moves the
median dluma −12.8 → −0.1 (TAT2 is BF2's map: threshold 150, +4 luma). The measure this card asks for can stop here; the
build is BFLT (value 5). Close TATL when BFLT lands and TAT1's parity dluma is within ±5.
