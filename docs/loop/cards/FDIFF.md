---
id: FDIFF
title: The first-divergence differ: compare two state traces (port vs port, or port vs PS2) and report the first tick and the first field that differ, with both values and the previous matching tick - plus a state-sync mode that re-seeds the port from the PS2 state at tick k and reports how many ticks it then stays within tolerance
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The output the oracle exists for: "the first mismatch is tick 412, group units, unit 0x61d1, field x: PS2 13.31 vs port 13.44" is something an agent cannot argue with. It also turns a pass/fail into a graded number. HONEST EXPECTATION: the port is a reimplementation, not a bit-exact port; free-running from a shared start the two traces will diverge early (RNG draws in a different order, float paths). So the useful modes are (a) port-vs-port (landing bisect, works from day one), (b) port-vs-PS2 with TOLERANCE (a unit matches if within eps), and (c) STATE-SYNC: copy the PS2 state into the port at tick k and measure how long a single system (e.g. one soldier's motion under the same pad) stays within tolerance - the per-system version that actually finds bugs.

## Evidence
- Trace format from SHASH (`engine/artifacts/oracle/README.md`), PS2 traces from PSHASH. Prior art named in the research note: OpenTTD desync docs, ISP RAS Trace Matcher, ICML 2026 verifier-guided matching.
- Port-side injection (c) needs a way to construct a `Sim` state from a unit list (spawn at pos/health/team) - the arena example (`game/examples/arena.rs`) already builds sims with placed units.

## Notes
Python, stdlib only. No engine change in this card (sync mode that needs one is FDIFF2).

## Implementation (2026-10-24)

Streaming version-1 differ implemented with exact hash and full-tuple tolerance
modes, physical-unit values, strict headers/layout/hash validation, optional
coarser-quanta comparison and explicit unavailable-group reporting. Self-test is
wired into gates_selftest.py. State injection split to FDIFF2; no engine changes.

Synthetic reference output:
```
first mismatch: tick 3, group units, unit 0x61d1, field x: A=13.0000 B=13.1250; previous matching tick 2
fdiff: matched 2 of 4 ticks first_divergence tick 3 group units field x
```
Existing port-binary fixture pairs: 600/600 matched, including full vs compact.
These are fixture checks; the real-map speed-mutation Acceptance is still owed.
PSHASH remains todo, so its conditional real PS2 comparison is not yet applicable.
