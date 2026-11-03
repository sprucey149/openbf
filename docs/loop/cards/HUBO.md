---
id: HUBO
title: On both Hoth maps half a bot's legs still walk at the hub's centre (BF2 HOT1, a showcase map: 52 %; BF1: 62 %; hubs of 16 m and more: 24 %): the floor walked from the bot's feet breaks 2-4 m out at a 1.3-2.5 m rise and the centre is taken - give those legs their point without stranding bots; plus HUBP's residue (the arrival plane on a rider's leg to a post, the barrier test without the unit's radius, three rules no test pins)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card HUBP (408bc9c) gives a bot its own point of the next hub - the PS2's waypoint - but takes the point only where
the floor walked from the hub's centre AND from the bot's feet reaches it; after 8 refused tries it walks at the
centre as before. Over all 32 maps that is 6.7 % of legs. On Hoth it is 62 % (BF1) and 52 % (BF2, a showcase map),
and 24 % at hubs of 16 m and more: the open maps and wide hubs, where the PS2's spread is the widest (its rule puts
a Hoth waypoint 16-25 m off the centre; the port's mean there is 4-10 m). Per unit-second on foot on a leg the
waypoint is the hub's centre 54 % (BF1 HOT1) and 45 % (BF2 HOT1) of the time, 10 % over all maps. "Bots file
through hub centres" (the maintainer, 2026-10-02) is still half true on Hoth. From AUDIT HUBP (2026-10-06, PARTLY).

## Evidence (`engine/artifacts/lead_tmp/audit-hubp/README.md`; verifier `verify/VERDICTS.md`, 11 / 3 / 0 of 14)
- PS2, live RAM, 1,058 distinct path waypoints with a hub (`tools/wp_ram.py`): 972 on the hub's sphere at its full
  radius, 43 at radius - 4 (the droideka; 33 of them on a hub of 4 m or less = the exact centre), 33 the goal, 10
  on a space map. XZ offset / radius mean 0.78 (BF1) / 0.80 (BF2), median 0.83; no full-radius row within 0.05 r.
- Port, the observer (`AUDIT_HUBP_LOG`, 316,425 calls, 4 seeds x 32 maps, `tools/obs_read.py obs_main --maps`):
  drawn 93.3 % (the first draw, i.e. the PS2's own point, 78.8 %), XZ offset / r 0.728 against 0.793 for the PS2's
  rule by the same numbers. BF1 HOT1 38.3 %, BF2 HOT1 46.8 %, BES1 85 %, END1 86 %, NAB1 89 % (first draw 30 %).
- One port test off at a time on Hoth, 8 seeds (`hv_*`): BF2 / BF1 drawn 48.3 / 37.1 % landed; 94.8 / 92.2 % with
  no walk from the feet; 51.6 / 37.7 % with no level and margin tests. So the walk from the feet is the refuser.
- Its control: of 6,257 legs the feet test sent to the centre (2 seeds x 32), the same walk to the hub's CENTRE
  fails in 6,213 (99.3 %). The test says the bot's own floor does not lead on, not that the point is worse.
- How it breaks (`AUDIT_HUBP_WHY4`, seed 1): on Hoth always "no ground" 2-4 m from the bot (2-4 % of a 107-120 m
  line) where the surface is 1.3-2.5 m ABOVE the walk - a step the 0.6 m ground probe cannot climb; BF2: 53 % of
  the refusals in three 12 m cells at x -264, z -12..0, on legs to hub 6 (r 45 m); BF1: (-264, -12), (-204, -30),
  (-138, 138). On BES1 it is open air (a drop, 89 %), on POL1 another storey (58 %): there the test is right.
- Tried in the audit, NOT good enough:
  - no feet test: the spread is the PS2's (XZ / r 0.71-0.79) and a teammate within 1.5 m 8.9 -> 7.7 % (BF2), 8.4 ->
    7.7 % (BF1), but rounds with a no-progress unit at the end 0 -> 4 of 8 on BF2 HOT1 (2-3 bots 4.6 m from their
    point of hub 61, leg 60 -> 61, a wall between) - HUBP's own first build, `hubp/ab/full_var`.
  - the feet test only where the centre passes it (`AUDIT_HUBP_FEETREL`, 8 x 32): drawn 98.3 %, but recycles BF1
    HOT1 6 -> 18 and BF2 GEO1 30 -> 53, a teammate within 1.5 m KAM1 5.8 -> 7.0 and POL1 13.9 -> 15.4 (BES1's
    recycles 29 -> 12). A bot that starts a leg in a trench needs the arc's own line to get out of it.

## Acceptance
- Observer census, 4 seeds (`obs_run.sh` + `obs_read.py --maps`): drawn at least 85 % of legs on BF1 HOT1 and BF2
  HOT1 and at hubs of 16 m and more; per unit-second the waypoint at the centre at most 15 % on both Hoth maps
  (today 54 / 45 %); all maps at least 95 % drawn (today 93.3 %).
- 8 seeds x 32 with bot quality, then seeds 9-40 on the maps that move (`perround.py`): rounds with a no-progress
  unit at the end on BF2 HOT1 0 of 8 (today 0); recycles per round not up beyond |t| 2.5 on any map; "a teammate
  within 1.5 m" not up on KAM1, POL1, DEA1, KAS2 (today 5.8 / 13.9 / 11.6 / 2.9 %) and down on both Hoth maps
  (today 8.4 / 8.9 %).
- Items 1-3 below: each with its test, mutation-checked.

## The small items, same file (one session pays the gates once)
1. **The arrival plane reaches a rider.** `hub_reached` puts the plane at the leg's WAYPOINT for every leg with an
   arc; `walker_drive.rs` (card WAI2) overwrites a mount's waypoint with the post (62-86 m past the hub), so a
   tauntaun's hub is "reached" at the post's plane, not the hub's - HUBP said "a vehicle keeps the centre". Only
   Hoth has such legs (30 of 32 maps none); 10 of 64 rounds change state, the ride census does not (230 vs 233 m a
   ride BF1, 274 vs 272 BF2; `hoth_land` / `hoth_far`). It is also the one round in 288 where HUBP with the draw
   switched off is not its parent (BF1 HOT1 seed 6; with `AUDIT_HUBP_OLDPLANE` it is). Fix: a waypoint that is not
   a point of its hub takes the hub's own plane, or the driver keeps its own arrival. Test: a mount's leg with the
   waypoint moved to a post arrives where it did before 512f678.
2. **The barrier test has no radius.** PS2 [V, verifier C5-C7]: a waypoint is re-drawn while it lies in a barrier
   GROWN by the unit's radius (`FUN_002c0220` B2:245930-245935 passes `FUN_002a76b8(unit)` to `FUN_002eea68`, whose
   overlap `FUN_002ed840` B2:277758 / 277772 compares with half extent + r; a soldier's r is a quarter of its box's
   horizontal diagonal, cached). `hub_point` asks `b.contains(p)` = `contains_r(p, 0.0)`. Use `contains_r` with the
   soldier's radius. (Also PS2: 1 + up to 100 draws, the port 100 in all; the 100th point is kept even when it
   still fails; a droideka's inset is 4 m, so on a hub of 4 m or less it walks at the exact centre.)
3. **Three rules no test pins** (mutations alive, `tools/mutate.sh`): every point ON the rim (the draw's y dropped)
   passes all 14 routing tests - pin the sphere law (200 units: mean XZ / r 0.79 +- 0.03, a tenth under 0.55);
   `HUB_POINT_LEVEL` widened to 50 m passes; the floor margin removed passes.

## Notes
- First candidate for the Hoth half: a leg that took the centre because the walk from the feet broke draws its
  point once that walk reaches the hub's centre (the bot is out of the trench) - a deferred draw, not the PS2's
  (it draws once per hub). Measure mid-leg changes beside it: today the same leg's waypoint moves once per 104 s
  of walking (BF2 GEO1 once per 19 s).
- deps MOVR: the mover's leg rule and recovery ladder change what a bot does at a step in front of it; measure on
  that mover. Planner: consider this card for BOTV's list.
- Seen in the audit, not explained: BF1 NAB2 kills / min 43.9 -> 40.4 over 40 seeds (t -3.3) after HUBP, BF2 NAB2
  37.1 -> 34.4 (t -2.6); BF2 UTA1 +11 % (t +3.9).
- The scratch binary and every switch: `lead_tmp/audit-hubp/observer_diverge_408bc9c.patch` (applies to 408bc9c;
  main's `ai/route.rs` moved with LOC3 - re-apply by `tools/observer_patch.py` + `tools/diverge_patch.py`).
- 2026-10-07 planner: MOVR landed (6061eec), so this card is free. **The audit's doc step was never done** (a STOP cut it): lift into `analysis/re-soldier-ai.md` section 2, from `lead_tmp/audit-hubp/CLAIMS.md` C5-C7, C11 and `verify/VERDICTS.md` - the waypoint's barrier test is the rectangle grown by the unit's radius and returns true when nothing is hit; the 100th re-drawn point is kept even when it fails; a flagged arc's leg is a 3-point object; a droideka on a hub of 4 m or less walks at the exact centre; the RAM census (972 of 1,058 waypoints on the hub's sphere). Do it as this card's first commit (Edit tool, not a shell write).
