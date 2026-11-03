---
id: OBSR
title: OBST's wall-follow holds bots at three places the old steering let go: BF1 Tatooine's rock gap (showcase; 19 lives of 100 s+ in 8 rounds, the parent 1), BF1 Naboo's ruins column (showcase; stays of 200 s+ 2 -> 15, unseen by the recycle watch) and Bespin's rim barriers (deaths under the floor +31 %) - cure each by its named rule, with tests the parent's steering fails
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
From AUDIT OBST (2026-10-07, PARTLY). Card OBST (e9ce6db) halves the stalls over all maps and that holds (2.41 % ->
1.25 % of 10 s windows, recycles 466 -> 244 over 32 seeds). But at three places a bot that goes round on one side,
immune to the progress window, now stays far longer than the bot that paced and was given up - two of them on
showcase maps - and the landing's two headline numbers do not see them: a "stalled" window is under 3 m net, and the
recycle watch restarts 5 m from where its count began, so a bot going round counts as moving on. All three stand on
main (b870afc; 55f055d differs in the game crate only).

## Evidence (`engine/artifacts/lead_tmp/audit-obst/README.md`; verifier `verify/VERDICTS.md`: 17 SUPPORTED, 2 PARTLY, 0 NOT of 19)
1. **BF1 Tatooine 1** (showcase), leg 78 -> 77: hub 77 (radius 0.6 m, point (-430.0, 19.4, 80.3)) lies in a gap
   between `tat1_bldg_rock_rubble_pillar` and `tat1_bldg_rock_rubble`. Seeds 9-16, within 14 m of (-427, 77): lives 11
   -> 34, unit-seconds 173 -> 6,688, lives of 100 s or more 1 -> 19, recycles 3 -> 14 (12 on that leg); main 37 / 7,353
   / 22 / 13. `recycle tat1` over seeds 9-40: 12 -> 37; stalled windows 1.29 -> 2.45 %; laps 16 -> 53 per 1000.
   **The rule is OBST's rule 2** (an inside corner is turned along the next wall on the same hand): off, 9 lives /
   170 unit-s / 1 / 0 recycles on the leg - the parent's numbers. Trace (`bb/land/tat1_s11.json`, unit 28): 3.2 m
   from the point it meets the pillar, turns along the rubble on the same hand and walks AWAY for 3 s (to 13.6 m)
   with the straight way open - the leave rule holds it, the window does not fail it while it moves on - then the
   window fails, it runs back, and again, for 120 s. The parent flipped to the other tangent and passed.
   Rule 2 off is no cure: all maps 1.25 -> 2.06 % stalled.
2. **BF1 Naboo Plains** (showcase), `nab1_prop_ruins_column2` at (-107, -108.3, 61) (bots on a straight leg `-1 -> 2`,
   58 m from their point; 4 at once in `bb/nab1/bf1_nab1_s1.json`). Seeds 1-8 x 540 s, within 12 m: lives 33 -> 54,
   unit-seconds 3,423 -> 7,188 (4.1 -> 8.3 % of all on-foot time on the map), lives of 200 s or more 2 -> 15, the
   longest stay per round up to 228 s -> up to 478 s. Recycles there over seeds 9-40: 102 -> 26. They did not leave:
   `stall::watch` restarts its count when the bot is `RECYCLE_NET` (5 m) from where the count began (outside the
   endgame), and a bot going round moves further than that (verifier's replay: 12 of 13 recycled lives never reach a
   110 s count, median 48 restarts; 14 more still there after a median 218 s and 854 m walked). A third of the
   32-seed fall in recycles (76 of 222) is this spot.
3. **BF1 Bespin Platforms**: deaths under the kill floor 1,039 -> 1,361 over seeds 9-40 (32.5 -> 42.5 a round, t +4.9;
   27 of 32 seeds up), with the window's immunity (rule 1) off 1,035, with rule 2 off 1,004. New spots: the box
   x -114..-102, z -210..-186 151 deaths (0 with either rule off; 0 lives ended there on the parent), (156, 144) 36,
   (-108, -108) 25, (84, -132) 23. `bb/bes1/bf1_bes1_s9.json`, unit 9: spawned at (-87.7, 108.0, -210.9), a commitment
   at a rim BARRIER (`steer slide STOP Barrier`), 5 s along the rim AWAY from its point (35.0 -> 37.7 m), dead at
   (-102.8, 82.6, -197.9). In `avoid::pick` the test "the wall beside it has stopped" asks the collision world only
   (`sim.collision.wall_sweep(.., SIDE_PROBE)`); a barrier is not in it. The OBST report's reading ("bots that used
   to stall there walk on into the floor leak") does not hold: with rule 1 off Bespin has the parent's falls AND the
   landing's recycles (5 of 32 rounds; parent 47, landing 6). Over all maps rule 1 off: floor deaths 1,415 -> 1,082
   (t -4.7), recycles 244 -> 268 (not resolved), rounds 1023 -> 1021 of 1,024, stalled 1.25 -> 1.32 % (not resolved).
   Barrier contacts lasting over 10 s: 0 % -> 26 % of barrier contact time (`census/{pre,land}/episodes.txt`).
4. **The tests do not referee these.** `a_wall_across_the_line_is_rounded_on_one_side` (20 m) is passed by the
   PARENT's steering tick for tick (173 / 122 ticks, 0 changes of side); a 40 m wall separates them (parent 1,277
   ticks and 19 changes, landing 356 and 0). The "parked hull" of `a_bot_walks_by_..._and_round_a_hull` is one
   placement: of 252 (36 yaws x 7 offsets) of that 2.4 x 4.8 m box 64 change side and 44 cost over 1 s of blocked
   picks (4 x 8 m: 101; 8 x 8 m: 144; 16 x 10 m: 187 of 252) - at a box's first corner the contact normal is the next
   face's, its tangent on the committed hand runs into the corner, `follow` asks the same wall three times and the
   "dead end" changes the side (`tests/land_e9ce6db.txt`, the trace at t 48). A switch that goes on along the last
   heading there takes the four to 7 / 17 / 20 / 25 - on the maps it removes a third of the changes of side and moves
   no gate number (recycles 244 -> 280 over 32 seeds, not resolved; the yard 247 -> 364 ticks): a pointer, not a fix.

## Notes
- Tools and every raw row: `engine/artifacts/lead_tmp/audit-obst/` (README.md). The scratch binary's switches
  (`tools/observer_patch.py`, e9ce6db): NO1 NO2 NO3 NO4 NOLEVEL TICKFIX CORNERFIX + `AUDIT_OBST_LOG`.
- One round of 1,024 does not end on either build; the landing's is BF2 MUS1 seed 33 (`bb/mus1/`): 14 attackers
  within 4 m of the last defender, who stands inside `mus1_bldg_lava_control` (stuck 546 s) - card ENDH's kind.
- Not for this card: the barrier keep-out, soldiers, the probe's cost (OBST2).

- LLEG 2026-10-07 (landed c0794eb): **item 2, Naboo's ruins column, was a spawn node INSIDE the column** - cp3's node (-107.2, -108.5, 63.2) lies in `nab1_prop_ruins_column2`'s closed mesh; a bot spawned there walked the walls from the inside (the 4.4 x 6.8 m box of its trail). The PS2 spawns the soldier on top of the column (y -104.24) and the port now does (`Sim::resolve_spawn_seats`). Stalled windows on the map 415 -> 13 and 379 -> 29 (8 seeds each), recycles 12 -> 1 in 16 rounds. Re-take this item's number (lives of 200 s+ within 12 m of (-107, 61)) on main before building anything for it; the wall-follow was not the cause.
- 2026-10-07 (card MOVR2's session, a note; main 6f6160b): spots the 24-seed x 32-map A/B kept naming, each present
  on the parent c0794eb too. (1) **BF2 Kamino, hub 121's notch** (27.1, 61.2, 136.1): a bot stands at a Ledge stop with
  `kam_bldg_aaa_bridge6` at 0.00 m, 5.3 m from hub 121's centre, until the 120 s recycle - 0 of 24 rounds on the
  parent's own binary but 2-3 in every variant that perturbs the walk (leg aim 2, node radius 2, give-up 3: 2, the PS2
  clock 8); the parent's one Kamino recycle is 6 m away (31.5, 61.2, 130.2). Snapshots
  `lead_tmp/movr2/bb/kam14/` and `kam8/` (unit 28 / unit 19). (2) **BF2 Kamino, the deck under hub 123**
  (57, 52.6, 142; floor_dy -11.3, leg 123->121, class 5, Ledge): 3 of 24 rounds with the leg aim, 0 on the parent -
  card RETW's kind (a walk back along the floor). (3) **BF1 Geonosis by hub 36** (100..117, -62.6, -11..-15; team 1,
  leg last->36 or 2->14): main's gate round (seed 42) has 3 recycles and one 8,220-tick no-progress run there (unit
  22) in a 1,718 s round; the parent has it on seed 10, the node-radius variant on seed 3. (4) **BF2 Mygeeto, hub
  78**: with the PS2's 3 s / 5 s stuck clock (not on main) team 0's bots follow `myg1_bldg_spiral_platform1`'s wall up
  onto the platform and die lost (98 lost deaths in 768 rounds against 21) - my reading [H]: the 2 s window's side-step is what takes
  them off that wall today; a wall-follow that does not climb would let the clock be measured again
  (`lead_tmp/movr2/bb/bf2_myg1_s1.json`, MOVR2=1 of `movr2_scratch_knobs.patch`).

## Result (2026-10-07, landed d9db73c; report `reports/20261007_1025_card_OBSR.md`; everything in `lead_tmp/obsr/`)
Two rules landed, one item re-diagnosed, one decided as documentation:
- **Item 3, Bespin: cured by its rule.** A commitment whose last contact was a rim BARRIER gets no allowance from
  the progress window (`Round::rim`). Not at a drop: with drops too BF1 Hoth paced its trench edges (22 recycles in
  8 rounds against 0). OBST's `fix2` ("beside" = shut by anything) did nothing here (1,376).
- **Item 2, Naboo: already cured by LLEG** (132 unit-s, 0 lives of 200 s+ on main e54841a). 2(a), the watch, landed
  as `RECYCLE_ROAM` = 15 m: mid-round a bot is given up when it walks two minutes without getting 15 m from where
  its count began. The audit's form of it ("new ground while a commitment is live or the walk is in recovery") gave
  up WALKERS: 19 of 81 sampled recycles had crossed a box of over 60 m in their last two minutes (Polis Massa, where
  the allowance is spent 28 % of the time and `recover` never clears).
- **Item 1, Tatooine: NOT a steering defect.** The connection hub 78 -> 77 is shut for the port's soldier; no life
  crosses it on the parent either (0 of 8 rounds on both builds) - card TGAP. The watch bounds the stay (longest
  480 s -> 152 s). A hand memory at a failed window and "the open tangent first" were built and measured: no cure.
- **Item 5, the allowance: the doc now says what the code does** (once spent it does not come back in a life).
  Starting it again when a free walk gets nearer than the walk has been: recycles 206 -> 201, Bespin floor deaths
  1,078 -> 1,189 (all maps +3.8 a seed, t +2.5). Not taken. `recover`'s u8 still saturates at 255 (not done).
- **Item 4, tests:** `a_long_wall_is_rounded_on_one_side_in_good_time` (40 m: 151 ticks / the mirror under 500; the
  parent 1,277 ticks and 19 changes, audit `tests/`), `a_box_at_any_yaw_and_offset_is_rounded` (252 placements: 67
  change side, 42 cost over 1 s, 0 never passed; bounds 70 / 50; the parent 104), `a_commitment_at_a_rim_barrier_is_marked`,
  `a_bot_going_along_a_rim_barrier_is_not_excused_by_moving_on`, `a_bot_lapping_a_yard_it_cannot_leave_is_recycled`.
  Mutations, each failing exactly its test: `RECYCLE_ROAM` = 5; the window without `!rd.rim`; the mark never set.

| acceptance | bar | main e54841a | landed |
|---|---|---|---|
| Tatooine 1, seeds 9-16: unit-s within 14 m of (-427, 77) | <= 500 | 3,571 | **1,579 (not met: TGAP)** |
| ... lives of 100 s or more | <= 2 | 11 (longest 480 s) | **12 (longest 152 s; not met)** |
| `recycle tat1`, seeds 9-40 | <= 15 | 24 | **47 (not met: the watch sees them)** |
| Naboo Plains, seeds 1-8 x 540 s: lives of 200 s+ within 12 m of (-107, 61) | <= 2 | 0 | 0 |
| ... unit-s there | <= 3,500 | 132 | 140 |
| Bespin Platforms, seeds 9-40: floor deaths | <= 1,100 | 1,328 | 1,097 (t -4.9 by seed; the box 117 -> 0) |
| ... its recycles | <= 20 | 0 | **27 (not met: 24 at (0, -160), unseen before - card LAPS)** |
| all maps: stalled 10 s windows | <= 1.30 % | - | **not re-taken** (the census runner was not run) |
| all maps: lap windows per 1000 (seeds 1-8) | <= 22 | 20.61 | 20.00 |
| seeds 9-40: rounds finished | >= 1,023 | 1,021 | **1,021 (the same three rounds; not met on either)** |
| seeds 9-40: recycles | <= 270 | 197 | **284 (rim rule alone 206; +78 are laps now seen)** |
