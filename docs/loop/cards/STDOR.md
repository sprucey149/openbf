---
id: STDOR
title: DEA1's blast doors open outward: rig their prims (and drawn leaves) relative to bones whose bind is off the origin, so the opened leaves clear the doorway the PS2's soldiers walk through
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/bake/src/bind_pose.rs,engine/crates/bake/src/door.rs,engine/crates/bake/src/map.rs,engine/crates/game/examples/pose_render.rs,engine/crates/game/src/assets.rs
declares: rebake=maps; sweep; leak; goldens=maps/bf2_dea1.png,maps/bf2_mus1.png,walkabout_stats.ron
needs:
effort:
branch: stdor
resume: landed 87ee1211 2026-10-01
created: 2026-09-30
alpha: yes
first: 1
---
## Why
STALL C3 (`analysis/re-round-stalls.md` §2 C3). `dea1_prop_door_blast` is the only door class whose bones bind away from
the origin: `bone_leftdoors` at x −5, `bone_rightdoors` at +5. The geometry bound to each bone lies on the opposite side.

The bake (`bake/src/door.rs`) applies the bone's model-space delta to prims whose XFRM it reads as model space
(`game/src/assets.rs`, `m * d * local`). So the opened leaves slide INTO the doorway: all six bodies overlap x
−67.4..−58.7 at open = 1, and the shut door has a 1.3 m slit.

## Evidence
- `lead_tmp/stall/doorcheck.txt`: 16 of DEA1's 24 blast-door bodies move INWARD. Every other door on BF2 MUS1, TAN1, KAM1
  and POL1 moves outward or up.
- The walk probe stops at z −73.7 with the door open.
- The Empire's knot at (−60.8, 50, −72.8): 1.73 M stalled unit-ticks on 8 of 9 seeds. DEA1 finishes 4/9.
- **PS2** (live RAM, 420 s, `lead_tmp/stall/ps2/bf2_dea1_soldiers.csv`, `dea1_doors.py`): soldiers cross blast door 2
  21 times and door 0 18 times, all through the centre band (|along| < 4.3 m).

## Notes
- [H]: which composition the engine applies (bone-relative XFRM, or a bind-relative delta) and whether the drawn
  segments need it too.
- The PS2 crossings are the check.

## Note from STFAL (2026-09-30)
STFAL (de4ba72) stops DEA1's bots walking into its pits: 3.62 → 0.54 env deaths/min over 8 seeds, and DEA1 finishes
4/8 → 0/8. Its blast-door jams (C3) had been cleared by those deaths; now they hold (`lead_tmp/stfal/v4_32/`). The
PS2's 3.0 falls/min come with 14 kills/min of fighting; judge DEA1's pit rate again once its doors let the fight happen.

## Note from the maintainer's "rounds that do not end" pass (2026-10-01, main 4603e9e, `lead_tmp/endings/README.md`) — `first: 1`
- DEA1 is the one map that NEVER ends: 0/8 at 120,000 ticks (67 min), 0.4 kills / min against the PS2's 14.0, every round
  cut at 102-140 v 16 tickets with 16 v 16 alive (team 1 bled to its alive count, then nothing).
- Seed 1 (`samples/bf2_dea1_s1.csv`, `walk_in_place.py`): team 0 walks in place at (-84, 50, -108) = blast door 0, team 1
  at (-60..-66, 50, -72) = blast door 2. An enemy is within 60 m 79 % of the time, through the door.
- STUCK_CENSUS reads 0 units stuck over 5,000 ticks on this map: the door pushes units back and forth and resets the
  stuck window. Judge the knot with `walk_in_place.py` (15.1 % of unit-seconds before), not `stuck_max`.
- Added acceptance: DEA1 finishes 7/8 or more inside 60,000 ticks and runs 10 kills / min or more over 8 seeds.

## Note from the AUDIT of STFAL (2026-10-01) - the note above is wrong about the pits
The PS2's 3.0 falls / min on DEA1 do NOT come with its fighting: none of its 21 fallers lost health in the 6 s before
leaving the deck. 12 fall into the pit beside cp3's spawn node (-118.9, 52.0, -235.8), 11 of them 5-25 s after spawning
there; 6 fall into the gap at the Tak bridge (x -170..-172.5, z -207..-196, from both ends), which `dea1g_con` opens
when Panel-Tak is destroyed; 3 at two other edges. Do not expect working doors to bring the port's pit rate back: its floor probe (STFAL)
refuses those steps and its bridges are solid. Card **DPIT** carries the pits and the bridge; STDOR's acceptance is the doors
and the fight only. Evidence `lead_tmp/audit-stfal/README.md`.

## Built (2026-10-01, lane 1, branch `stdor` 6220be4 + 87ee121) — results against the acceptance
The [H] is settled: a door's `BNAM` segments AND its `PRNT` prims are stored in their bone's frame (`re-bf2-doors.md`
§10). One rule in `bake::bind_pose` (doors now go through `pose_bnam`; `BindPose::rest_prims` for the prims). No game
code changed; `bake::door`'s motion tables are byte-identical.

**The acceptance line "every non-DEA1 door's baked rig is byte-identical" is REPLACED.** MUS1's four door classes
bind their bones tilted 20.8 deg, so the same rule tilts them — and the PS2 draws them tilted (pairs below). The rigs
of KAM1, POL1 and TAN1 (bones at the identity) ARE byte-identical: scratch bake of every map vs the shared bake =
7 files differ (bf2_dea1 manifest + blast door GLB; bf2_mus1 manifest + 4 door GLBs), 5,508 the same. This closes BNR.

| acceptance | before | after | evidence (`lead_tmp/stdor/`) |
|---|---|---|---|
| doorcheck, 5 BF2 door maps: no INWARD body | DEA1 16 INWARD | 0 INWARD; TAN1 / KAM1 / POL1 rows identical | `doorcheck_before.txt`, `doorcheck_after.txt` |
| walk probe crosses blast door 2's centre | stops at z -73.73 | reaches z -95.4; door 0 too (x -85.2 -> -63.7) | `walk_door2_*.txt`, `walk_door0_*.txt` |
| DEA1 knot gone | walking in place 14.0 % (s1), top cells the two doors | 2.3 / 3.0 / 2.6 % (s1, s2, s5), no door cell in the top 8 | `samples_after/`, `endings/walk_in_place.py` |
| DEA1 finishes >= 7/8 in 60,000 ticks, >= 10 kills / min | 0/8, 229 kills in 8 rounds | 8/8, median 609 s, 1,073 kills in 87.4 min = 12.3 / min (PS2 14.0) | `sw_before/seeds.md`, `sw_after/seeds.md` |
| drawn door matches the PS2 shut and open | a 1.3 m slit shut; leaves across the doorway open | shut with the PS2's seams; the same diamond hole while opening | `pics/sheet_bf2_dea1_door2_far_b000.png`, `pics/sheet_dea1_door2_opening.png` |
| MUS1 (new) | upright panels in front of slanted frames | panels in their frames as the PS2 | `pics/crop_cp5_cp6_b180.png`, `pics/crop_cp2_cp3_b090.png`; MUS1 8/8 -> 8/8 |

- Leak census (declared): DEA1 sites 255 -> 259 (closed 508 -> 595: runs that now go THROUGH doors 0 and 2 and end in
  corridors the census's ray parity already reads as enclosed, accepted class LEAK2), MUS1 334 -> 304; new 0.
- DEA1 has 4 `lost` recycles and 4 region deaths over the 8 seeds (for RCYC's census).
- Left over: a smaller walking-in-place cluster at (-162, 51, -162), 91-133 unit-s a round (it was there before).
- New PS2 references: `pcsx2/ref/bf2_dea1/walk/door2_far_b000`, `door2_near_b000` (+ states). `pose_render` takes
  `POSE_DOORS_OPEN=<0..1>`.
