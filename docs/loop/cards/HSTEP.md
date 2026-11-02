---
id: HSTEP
title: The hover's tick in the PS2's order: drive, then the contacts, then the spring tick - the port ticks its springs BEFORE its contacts (one tick late), so a parked hailfire or snail tank never comes to rest and every hover bobs about 3x the PS2's band
milestone: M2
value: 4
phase: build
status: done
deps:
files: engine/crates/core/src/vehicle/hover.rs,engine/crates/core/src/vehicle/hover/tests.rs
declares: sweep
needs:
effort:
branch: hstep
resume: landed 46fb02ad 2026-10-16
created: 2026-10-07
alpha: yes
---

## Why
`core::vehicle::hover::step` runs drive → springs → contacts. Both PS2 games run **drive → collide pass → spring tick**
(task keys 0 / 10 / 15; `analysis/re-vehicle-rest.md` §5 [O], which settles `re-hover.md` §2's [H]). In the port each
spring tick therefore pushes with the compressions of the pose one tick back, and that lag feeds every bounce:
- a parked hailfire (and snail tank) never comes to rest — BF1 GEO1's swings over 14 cm with up to 4.8° of tilt, as
  wide in seconds 20–40 as in 10–20; the PS2's stands on its springs, still to 3 mm over 8 states;
- every parked hover bobs in a band about 16 cm wide where the PS2's is 3–5.5 cm;
- driven hovers get the same one-tick-late ground reaction (not measured).
Filed by the VREST research pass (2026-10-07); value 4 because it is on every hover of every map, showcase maps
included (NAB1 / TAT1 speeders, UTA1 / MYG1 fighter tanks).

## Evidence
- The order: `re-vehicle-rest.md` §5 — the collision world registers its own task at prio −10 (B1:1920, B2:1806), the
  hover's controllable at 0 and its physics task at −15; 134 of 134 touching BF2 hovers hold their pending
  compressions at 0 in RAM (the spring tick ran last).
- The experiment: `lead_tmp/vrest/hover_step_order.patch` (an env switch, `VREST_ORDER_OLD=1` = today's order; the
  old order reproduces main's census byte for byte). Against 114 parked PS2 hovers, |PS2 y − port y| after 40 s:
  within 2 cm 31 → 92, within 5 cm 71 → 112, mean 4.78 → 1.58 cm; hailfire (4 pads) −0.110 → −0.002 m.
  Files: `lead_tmp/vrest/port_census_{old,new}_{10s,40s}.txt`, `trace_hailfire_{old,new}.txt`,
  `ram_vs_port_{old,new}.txt`.

## Notes
- The two parked rows still over 5 cm after the reorder are not this card's: BF2 FEL1's pad `snail31` (a combat
  speeder, +0.35 m) and one BF1 NAB1 fighter tank (+0.09 m).
- Walkers were not examined ([H]: a −15 registration sits in the walker's code too, B1:132532): 30 of 47 parked
  walkers are within 2 cm today.

## Note from WALKB1's RE (2026-10-07)
The walker has the same order defect: its contacts run after its legs in the port, between the fall and the legs on
the PS2 (`analysis/re-walker.md` §13.1, §16). That is card WCON, not this one; the two do not share files.
