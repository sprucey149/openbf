---
id: SPUP
title: A sprint cannot be started while running uphill (the maintainer's playtest 2026-10-05): the entry needs 0.9 x MaxSpeed and a ramp holds the run under it - find which speed the PS2 compares (along the slope or planar) and whether its soldier can start a sprint on the same ramps, then make it start uphill
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-05: "it seems I can't start sprinting while walking uphill? That's a bug that should be fixed as well."

## Evidence
- The entry rule (`core/src/soldier.rs`, "Sprint enter/exit"): `planar_speed >= 0.9 * c.max_speed`, the HORIZONTAL speed. It is the PS2's predicate as read (`analysis/re-stance-roll.md`: FUN_00247ad8, "speed >= 0.9 x MaxSpeed"); which speed the PS2 compares is not written there.
- The maintainer's own snapshots (`playtest/*.json` of 2026-10-04/05, BF2 KAM1, the clone rifleman; sparse 0.5 s samples, stick full forward, on the ground, not sprinting): on the flat 6.89 m/s (31 samples; the sprint's 10.33 = 1.5 x 6.89, so MaxSpeed is 6.89 and the floor 6.20); up a 20-35 % ramp 6.23 m/s median, 4.64 the lowest (8 samples). So on Kamino's ramps the run sits AT the floor, and on anything steeper under it. Read by Claude in chat; the slope is from two positions 0.5 s apart, so it is rough.
- The same would explain his 2026-10-04 report (card BLKS): no sprint as the BF1 stormtrooper on KAS2's beach slope, fine on Kamino's flat decks.
- `input::SprintLatch` waits 1.5 s for the sim to start the sprint and then drops the click, so on a long slope one click does nothing at all.

## Notes
Filed 2026-10-05 by Claude in chat. Related: BLKS (the black box's sprint fields), CSPD.

## Result (2026-10-06, landed c1ae263; report `reports/20261006_1310_card_SPUP.md`)
- PS2 (BF2, live, 11 runs): `0.9 x MaxSpeed < v . forward row` (B2:160321-160329); the uphill run is MaxSpeed x cos^3, so
  the sprint starts at 1.7 / 9.8 / 13.3 / 14.8 deg (Kamino's ramp, 3 of 3) and never at 16.1 / 19.3. re-stance-roll 4.2b.
- The maintainer's case on Kamino was his stick: it peaks at 0.984, the run sat at 0.890 x MaxSpeed. `input::STICK_FULL` 0.95.
- Steeper slopes: ours - the start is taken against what the slope allows (`Soldier::sprint_start_speed`).
- Tests: `a_sprint_starts_while_running_uphill` (0 / 10 / 20 / 30 deg, both rules, within 1 s; fails on the old rule),
  `the_uphill_run_is_the_ps2s`. Sweep / arena / sim identical; leak re-baselined (declared, rises in the report).
