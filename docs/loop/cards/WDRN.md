---
id: WDRN
title: Deep water drowns a soldier, as on both PS2s: while the surface is at or over his aim origin (feet + 1.80 m standing) a 3.0 s timer runs, then 30 damage every 1.0 s; while it is over his fire point (feet + 1.58 m) his triggers are dead - the port's soldiers walk the sea bed unharmed and shoot from under it
status: archived
issue: 70
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #70](https://github.com/sprucey149/openbf/issues/70).

## Why
RE + live measurement of card WADE (2026-10-06): `analysis/re-water.md` section 9.2 [O]. Both games' soldier update
(B1 `FUN_00205cc8` 117186-117270, B2 `FUN_00240bf8` 159994-160069) asks the water's height at the soldier's XZ
(LAYR + the 6 cm bob, only in a WET cell) and then:
- water > the weapon's fire point (feet + 1.583 m standing): both triggers are dead (B1 the two trigger byte pairs
  zeroed, B2 both words masked);
- alive and water >= the aim origin (feet + 1.800 m standing): a timer counts down from 3.0 s (a class constructor
  constant, no ODF key), then += 1.0 and 30 damage through the ordinary damage call; it reloads to 3.0 the moment the
  surface drops under the aim origin or he leaves the wet cell;
- nothing else: no float, no slow-down (6.47-6.50 m/s in 1.0-1.4 m of water against MaxSpeed 6.5), no depth limit.
The port has no water rule for soldiers (`Sim::water_level` serves ordnance only).

## Evidence (PS2, one row per game update: `engine/artifacts/lead_tmp/wade/ps2/out/*.csv`, `pcsx2/ref/<map>/wade/`)
- BF1 KAS1 (-72, -16), 7.28 m deep, 300 hp: under at 0.57 s, -30 at 3.570 s, then every 1.001 s, dead on the 10th tick.
- BF2 NAB2 (0, -40), 3.46 m deep, 400 hp: -30 at 3.604 s, then every 1.001 s, dead on the 14th tick (16.58 s).
- BF1 KAS2 1.50 m: no timer, triggers live; 1.85 m: triggers dead, the timer runs 2.7 s of every 3.33 s bob period,
  reloads, and never fires in 16 s; 2.93 m: six ticks of 30 seen.
- BF2 FEL1 (-121.66, -118.56): a DRY cell 4.78 m under the plane - the timer stays 3.0 (the wet-cell test).
- The aim origin is built from the class's per-stance section (B1 `FUN_0032f820` 317727): a crouched soldier's is
  lower. Not measured - PROVISIONAL: use the port's crouch eye height until a PS2 row says otherwise.

## What to build
- The water query for a soldier: HWAT's wet grid + LAYR + bob (dep). A soldier's water state per tick: fire-point
  test -> triggers ignored; aim-origin test -> the drowning timer (per soldier, reload 3.0, tick 1.0, damage 30,
  the kill credited as an environment death).
- Bots: nothing is known of how the PS2's AI treats deep water (not in this block); do not add avoidance here.
- Not here: how the water is DRAWN over him (card WADE), the wader's ripple and splash (note on FXP5).

## Acceptance
- Unit tests that replay the measured timelines: 300 hp at 7.28 m dead on the 10th tick, first damage 3.0 s after
  the aim origin goes under, ticks 1.0 s apart; 400 hp dead on the 14th; no damage at 1.50 m; dry cell under the
  plane: no timer; triggers dead at 1.60 m and live at 1.50 m; the timer reloads when the surface dips below the aim origin.
- The 8-seed sweep: paste the row of every map whose sim hash moves and say which unit drowned or held fire where
  (expected: few - BF1 KAS2's sea is under 1.8 m within 280 m of the village; BF2 NAB2's canal is 3.4 m deep).

## Notes
- 2026-10-06 planner: kept at value 3, `defer: post-alpha` (none of the six showcase maps has deep water; the maintainer was told in the WADE report and STATUS question 26 asks whether he wants it sooner - BF1 Kashyyyk, where he saw the wading soldier, is where he would meet it).
