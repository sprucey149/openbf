---
id: BOWL
title: BF2 Kamino: bots walk over the rim rails onto the platforms' curved hulls (the maintainer, 2026-10-01, in play: 'figure out why the bots are going on this bowl') - the level test calls the first 4.5 m of hull ON the level, so the rail's barrier is no wall and a bot out there is never lost
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-01 22:00, playing BF2 Kamino (frame: two droids standing on the sloped hull outside the rail, below the
deck he stands on): "the ai units love going over the platforms onto the curved edges ... figure out why the bots are
going on this bowl!!!!" The cause is found and a one-line fix is measured. This card lands it properly.

## Root cause (measured on main d800f1b; everything in `engine/artifacts/lead_tmp/bowl/`, README first)
`Sim::level_at` (`core/src/sim/bounds.rs`) walks from a near hub toward the point and stops at the first barrier. It
then calls the point ON the level when the floor where the walk STOPPED is no more than `BOUND_DROP` (2 m) over the
point and the stop is within `LEVEL_SLACK` (10 m). At a rim the walk stops at the rail, on the deck, so every point
of the hull up to 2 m under the deck reads ON.
- **The profile** (`BOWL_PROFILE=0,102.1,30,102.1,75,0.5`, the y 60.3 platform, rail at x 7.5): the hull runs from
  x 7.5 to x 22.5 at n.y 0.83, then the sea. `level_at` says ON out to x 12.0 (4.5 m of hull) and Off only from x 12.5.
- **Consequence 1, the steering.** `steer::barrier_ahead` makes an edge barrier a wall only when the ground `EDGE_LOOK`
  (6 m) on along the WALK direction is missing or off the level. Any oblique approach, and every slide along the rail,
  lands that look inside the 4.5 m band, so the rail is no wall and the bot walks over it (RAIL, c36c8a2, made the rail
  itself climbable at 3.5 m/s).
- **Consequence 2, no way back.** `LostState` opens on `level_at(...).is_off()`. A bot on the first 4.5 m of hull is
  never lost, so it never walks back and BNDS's recycle never fires; it slides along the outside of the rail.
- **Consequence 3, the instrument.** `UNIT_SAMPLES`' `level` column and BNDS's "off the level 0.9 %" use the same
  test, so the measurement could not see what the maintainer sees.

## The numbers (seeds 1 and 2, 18,000 ticks, 32 bots; `an.py base_s*.err v1_s*.err ...`)
A scratch probe counts a unit as beyond a rim when a barrier lies between it and each of its 3 nearest hubs, and
buckets it by its drop under the hub's floor. "On the hull" = 0.3 m or more under.
| | seed 1 | seed 2 |
|---|---|---|
| main: unit-seconds on the hull, of all | 2,500 of 19,020 (13 %) | 3,454 of 18,953 (18 %) |
| ... of those, `level_at` says ON | 1,785 | 2,058 |
| main: rail crossings by what moved the unit | slide 225, fwd 90, airborne 51, fan 36, pushed 23, hold 7 | slide 300, fwd 91, airborne 70, hold 54, pushed 49, fan 42 |
| **V1**: a barrier-stopped walk vouches only for a point within 0.75 m under its stop | **36** | **13** |
| V2 alone: `barrier_ahead` looks across the barrier along ITS normal | 1,074 | 602 |
| V1 + V2 | 52 | 24 |
So V1 is the fix (about 99 % of hull time gone) and V2 adds nothing to it. With V1 the 10-minute tickets read
109 / 80 and 124 / 99 against main's 125 / 126 and 129 / 113: more fighting.

## Notes
- All of this is the port's own (PROVISIONAL). The PS2 never crosses a barrier at all (RE_LOG "Barriers are the PS2's
  Obstacles"); the edge rule exists because every barrier as a wall cost Hoth and Tatooine their rounds.
- Run new probe binaries under the memory cap (INBOX 2026-10-01 17:15).

## Note: the 8-seed x 32-map pair for V1 (2026-10-01 23:20, `lead_tmp/bowl/README.md`)
`sw_base` (main d800f1b) against `sw_v1` (`BOWL_V1=1`), 60,000 ticks, the pre-RCYC binary. The scratch worktree is gone;
`git apply lead_tmp/bowl/bowl_probe_v1_v2.patch` on d800f1b rebuilds it.
- Rounds 246 -> 250. **BF2 KAM1 0/8 -> 6/8**, kills 993 -> 1,452, lost + under-the-map deaths 25 -> 2.
- Down: BF2 MUS1 8/8 -> 6/8, GEO1 8/8 -> 7/8, UTA1 8/8 -> 7/8 - every one a clamped last survivor (1 v 16, 16 v 1,
  4 v 1) at the cut. RCYC was not in that binary; re-run on main with RCYC before reading them as this card's.
- `lost` deaths rise on BF1 HOT1 (33 -> 57), BF2 HOT1 (3 -> 9), BF2 UTA1 (71 -> 80): say whether those bots are really
  off the level (the INBOX's `level_at` false positives under Hoth's trenches) before landing.

## Audit (2026-10-02, PARTLY — `audits.md`, `reports/20261002_1400_audit_BOWL.md`, `lead_tmp/audit-bowl/README.md`)
Every landed number holds and the hull share by plain geometry went 26.2 % -> 0.11 % (the PS2's own: 0.01-0.26 %).
Missed: the rule calls a ramp behind a barrier off the level (BF1 RHN1's tunnel: bots go lost and turn back every
1.7 s; BF2 UTA1's ramp; both Yavins) and its ON band holds a bot outside a rail (323 s on main, seed 23). Card LVLB.
