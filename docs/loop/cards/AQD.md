---
id: AQD
title: BF1's hub locate, corridor test and nearest hub are 3-D
milestone: M3
value: 2
phase: build
status: done
deps: BPO
files:
declares: marks: SWEEP
needs:
effort:
branch:
resume: MERGED into LOC3 by the planner 2026-10-03 03:35 (not built): LOC3 is the same change with the audit's measurements and a runnable acceptance
created: 2026-09-25 (from QUEUE.md)
defer: post-alpha
acceptance: needed
---

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
AQD. [TODO — ADDED 2026-09-25 (lane 1) by STK2's measurement; **[SWEEP]**; after BPO (same files); a check of the PS2's locate first — analysis/ may already hold it] **BF1 YAV1's aqueduct: a re-plan cuts the deck corner and drops the unit under it** — STK2's trace (`lead_tmp/stk2-census/yav1_u8.err`, HDL's round, seed 42): team-0 unit 8 on arc 110 (hub 4 → 83) re-planned at T10380 at (57.2, −61.5, 52.8), 17.5 m from hub 83 (radius 8.08); `hub_for_on` answers 83 (arc 110, t ≥ 0.5), so the new leg walks STRAIGHT from there at hub 84 (39.5, −63.9, −22.0), leaves the `yav_bldg_aquaduct` "bridge" deck (instance (40, −69.3, 17), deck y −63.3) at x ≈ 54 and drops 11 m to the ground between its piers (y −72.5..−73.2) by T10500. There it slides along the pier walls (z 28.6, then 16.4) for the rest of the round with `stuck_run` ≤ 1,080 (a slide's progress resets the window), because the leg to hub 84 still points south and `hub_has_floor` accepts hub 84 from below (the ground under the deck). Units 9 and 10 were in the same pocket by ticks 1,500–6,000. STK2's Hunt rule ends that round (a hunter reached unit 8) but the trap stands. analysis/RE_LOG.md:342 "Arrival and locate [V]": **BF1's hub locate, corridor test and nearest hub are 3-D** (`FUN_00265d38` / `FUN_00265d90` / `FUN_00265eb8`), the port's locate is XZ — "a real difference for BF1"; under the deck (dy ≈ 10 m against radii 8.1/8.4) BF1's 3-D test would not put unit 8 on arc 111 at all. Build (the faithful default): BF1's 3-D locate for BF1 maps (BF2's hub containment stays XZ, its corridor 3-D, per the same entry), PROVISIONAL where RE_LOG is [D]. GATE: the YAV1 trace — no unit ends under the deck; a `nav.rs` unit test (a point 10 m under an arc is not on it for BF1); the sweep re-baselined, every moved row named; sim_gates + `--sim-hash 100` + arenas verbatim (the arenas are TAT/flat — say which moved). Files: `core/src/nav.rs`, `core/src/ai/route.rs`.
