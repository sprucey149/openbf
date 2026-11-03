---
id: HTGTW
title: HTGT's wiring has no tests: 10 of 14 audit mutations of the vehicle pick survive all 1812 tests - pin the building turret's aim sphere install (slice 3b's whole fix), the readout's vehicle names from a built walk, the vehicle magnet in the sim hash, a turned turret's sphere centre, a moving vehicle's assist lead and a turret's Target kind
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
AUDIT 2026-10-21 HTGT (PARTLY). The vehicle / turret pick works on real maps (35 classes, 23 maps: every aimed try
with a clear line picked the vehicle), but most of what makes it work is held by no test. Ten of fourteen mutations of
the landing (90132e1) pass the whole suite (`cargo nextest --workspace --lib --bins --tests --example arena`: 1812
passed each time). The worst: deleting `v.aim_sphere = sphere;` in `vehicle_pads::install` brings back exactly the
defect slice 3b fixed (every building turret a radius-0 point the cone misses: HOT1's dish turret unpicked from 15 m),
and no test fails. Emptying the readout's vehicle names (`EventFeed::for_walk` or `make_walk`'s
`walk.vehicle_classes = ...`) gives every vehicle a blank name on the HUD, and no test fails (the feed test sets
`feed.vehicles` by hand). A ci-only change can undo the card silently.
Also: the card's "bar 1.0 -> 0.67 under rifle fire" (KAS2 IFT-X) came from a scratch hook that set the BF2 rifle's
vehicle damage scale from 0.1 to 1.0 (`lead_tmp/htgt/tools/htgt_face_scratch3.diff`, not said in the report). With
the real rifle the same 210 ticks take the bar 1.0 -> 0.967. The readout reads health every tick either way.

## Evidence
- Mutation table + runner: `engine/artifacts/lead_tmp/audit-htgt/tools/muts.py`, `mut_one.sh <ID>` (applies in the
  scratch worktree `openbf-wt/audit-htgt-mut-scratch` at 90132e1, runs the suite under ci.lock, restores + touches);
  logs `lead_tmp/audit-htgt/logs/mut_<ID>.log`. Control: 1812 passed, 52 skipped.
- SURVIVED (no test failed): M01 hash drops the vehicle marker byte; M02 hash writes a vehicle magnet as none;
  M03 a vehicle candidate's kind fixed at 3 (a building turret tested as a vehicle by the weapon's Target bits);
  M04 a vehicle candidate's velocity zero (assist lead / tracking); M05 the candidate centre ignores the vehicle's
  rotation (`pos + c`; it matters only for a centre off the pivot in x / z - the census measured the vertical offset alone,
  0.4-7.7 m, which a turn about y does not move, so the size of this one is unmeasured); M09 the held target kept
  without `live()`; M10 black box writes a vehicle as -1 - slot; M11 `EventFeed::for_walk` never fills vehicle
  names; M12 `make_walk` never sets `walk.vehicle_classes`; M13 `install` never stores `aim_sphere`.
- CAUGHT: M06 (blast sphere ignored: 4 tests), M07 (target hull not passed), M08 (target building not passed),
  M14 (readout id top bit).
- Rifle scale: `lead_tmp/audit-htgt/runA.log` (hook as the builder ran it: 0.98 / 0.90 / 0.82 / 0.73 / 0.67) vs
  `runB.log` (HTGT_REAL=1: 0.998 / 0.99 / 0.982 / 0.973 / 0.967); "HTGT scales before [1,1,1,0.1,0.1,0.1,0.1]".

## Notes
- Behaviour-neutral (tests only, plus any small seam a test needs): every gate should read identical; declare nothing.
- The pick itself held in the audit's census (`lead_tmp/audit-htgt/census3/`, `census_summary.txt`, hook
  `tools/census_hook_main.diff` on main 982999c): 5268 aimed tries at 15 / 40 m, 3345 picked the vehicle, all 1824
  misses had the acquisition's own LOS blocked and in none was the vehicle the first surface on the line to its centre.

## Fixture-first continuation (2026-10-24 10:33 UTC)
The original acceptance explicitly permits a synthetic armed-building fixture and remade mutation patterns. Those may be developed in an isolated loopctl worktree without waiting for the missing audit-htgt kit. Keep the real install/for_walk/setup paths in the test, derive expected values independently and preserve all six required FAIL rows. The HTGT card already records the rifle scale correction; do not re-open the same documentation gap without checking its Notes. Full CI/applicable landing gates and any required real-bake checks remain owed.

## GitHub checkpoint recovery (2026-10-24)
