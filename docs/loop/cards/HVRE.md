---
id: HVRE
title: The hover integrator, transcribed: BF2's tick FUN_0021ca80, spring points FUN_0021f5d0, drive FUN_0021d8b8, contact and the position step — the physics of 21 vehicle classes
milestone: M2
value: 5
phase: re
status: done
deps:
files:
declares:
needs:
branch:
resume:
created: 2026-09-25
---

## Why
Every hover and treaded class — 21 of the discs' 53 vehicle classes, all ground vehicles but the walkers — runs ONE
integrator, and PLAN locks BF2's handling for ground vehicles. VEH0 located it and read its equations; a faithful Rust
port needs the statement-level transcription before HOVR writes code.

## Evidence
`analysis/re-vehicles.md` §3 [O]: the functions (B2 tick FUN_00224de0 → FUN_0021ca80, spring FUN_0021f5d0, input
FUN_002245d0 → FUN_0021b470 → drive FUN_0021d8b8), props → offsets (§3.1), the identical ctor defaults, the equations
with B1 lines (§3.2), the BF1/BF2 differences (§3.3). Open there [H]: where pos += v·dt and the world sweep run (a shared
dynamic-body step), the strafe carve term, what entity[0xf0] / +0x360 is in the human yaw-rate branch, the B2 twin of
the contact callback (B1 FUN_001ee708 / FUN_001ee3e0), the 30·dt factor in BF2's aim-angle integration.

## Result (2026-09-26, lane 2) — DONE
`analysis/re-hover.md` [O], main 79fce68 (report `engine/artifacts/loop/reports/20260926_1417_re_HVRE.md`).
- **Verifier:** one foreground Opus agent, 53 claims: 46 SUPPORTED, 7 PARTLY (5 citations fixed; 2 substance folded
  in), 0 NOT SUPPORTED.
- **VCLS addendum:** all SUPPORTED. One refinement: the 0.5 spring-length factor also has an unnamed `%f` prop,
  0x77d6d149. No disc class authors it, and none puts a Body* modifier before its first AddSpringBody, so
  `vehicle_class` needs no change.
- **The skiff by hand:** rest 1.94 m (origin above flat ground; 1.98 m without jitter); 16.0 m/s after 3.31 s;
  1.0 rad/s at speed (AI after 0.5 s; pad peaks at 1.2 at 0.6 s, then 1.0 from 1 s).
- **The build is HOVR** (notes on its card).
