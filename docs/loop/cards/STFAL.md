---
id: STFAL
title: The AI does not step off walkways: a floor check on every step the steering commits (slide, fan, side-step, corner), and a probe that sees the low hulls the wall pass hits (KAM1 99% fall deaths vs the PS2's 1.6%; KAS2's parked AT-STs)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/steer.rs,engine/crates/core/src/ai/brain.rs,engine/crates/core/src/ai.rs
declares: sim; sweep; arena
needs:
branch: stfal
resume: landed e09c8548 2026-09-30
created: 2026-09-30
alpha: yes
---
## Why
STALL C1 + C8 (`analysis/re-round-stalls.md` §2).

**C1.** On KAM1, 270 of 276 deaths at seed 42 are the sea's death region. Over 9 seeds that is 8.17 environment
deaths/min against 0.12 kills/min, and KAM1 finishes 0/9.

The PS2's KAM1 (live RAM, 420 s): 2 falls in 123 deaths (1.6 %), and 3 of 26,529 samples below the decks.

The port's own lateral moves carry units off 5–8 m bridges:
- the wall slide and fan (ai.rs: "OURS and not on the PS2");
- the 5 m side-step;
- off-line corner points.

A fall that lands alive strands the unit (STLVL): KAM1's rims (69 % of its no-progress episodes), MUS1's 59.4 level.

**C8.** At BF1 KAS2's Empire spawn three parked AT-STs stand 2 m ahead. The wall pass pushes units back off their hulls
every tick, while the step-lifted probe reads clear (seed 5, run 20,520).

## Evidence
- `lead_tmp/stall/all/eps_s*.err` (`DEATHS`, `ENVD`, `EP`).
- `kam1_u23.err`: one unit, 12 lives, 11 falls.
- `maps/kam1_{a,b,c,d}.png`.
- `ps2/bf2_kam1_*`.
- KAS2: `TRACE_UNIT=7` seed 5 (`VEH` lines).

## Landed (2026-09-30, e09c854; report `reports/20260930_1705_card_STFAL.md`)
Acceptance against 8 seeds (`lead_tmp/stfal/v4_32/`): KAS2's Empire-spawn cluster 18 → 3 episodes (seed 6 only) —
nearly met. KAM1 env deaths 98.6 % → 96.0 % (5.14/min) and below-deck episodes 555/803 → 214/925 — NOT met; their
causes are the downhill hop (new card STHOP, which carries the KAM1 ≤ 5 % line and BES1's PS2 measurement) and
wrong-floor legs (STLVL, which carries the ≤ 1 % line). DEA1 pit deaths 3.62 → 0.54/min: the PS2's 3.0/min come with
its fighting, which DEA1's doors block (note on STDOR). BES1 falls 11.3 → 7.1/min; the PS2 is not measured.

## AUDIT 2026-10-01: PARTLY (`reports/20261001_1845_audit_STFAL.md`, `lead_tmp/audit-stfal/README.md`)
Held: every number above and in the commit message (rebuilt 6c3c8ea and e09c854, own parser, 9 seeds). The check does its
job: a unit's own step off a floor over a drop of more than 4 m, in a direction the rule refuses, 1,776 events (847
deaths) -> 383 (166) in 8 seeds. The hull probe: KAS2's spawn, stays of 600 ticks or more 32 -> 3, unit-ticks 162,289 ->
28,541.
Not held: (1) "KAM1's remaining falls are the downhill hop and wrong-floor legs": of 1,370 sea deaths 69 % are hops,
21 % (289) are PUSHES and 10 % the unit's own move. `separate_units` moves soldiers with no floor check, and units the
probe holds at a rim are pushed over it: pushed deaths 139 -> 521 over all maps (97 -> 451 by the strict rule), 245 of the 346 at KAM1's east bridge
(a leg across 36 m of open sea). (2) The waypoint clause: a hub on a HIGHER deck over open ground reads as a waypoint on
that ground and switches the check off (test: a unit walks off a 10 m pillar); it passes 1,280 drops of more than 4 m in
8 seeds, 10 of them deaths. (3) DEA1's pit line and its reason: see DPIT. (4) A unit at `Ledge` does not re-plan out
(note on RCYC). (5) The 20,520-tick stuck run quoted for KAS2's spawn (steer.rs, re-round-stalls C8) is another unit's, a
defender crowd at (225, 17, -5); the spawn unit stayed 48,649 ticks with a stuck run of 8,220.
On main 58fa0e3 (after STHOP, GRND, KSTK, KFLR, BNDS, RAIL): KAM1 sea deaths 0; deaths that leave a floor grounded over a
sheer drop 723 -> 4 (pushed 521 -> 2).
