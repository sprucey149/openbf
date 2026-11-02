---
id: DOORS
title: Sliding doors: a bot goes through the centre of a door's opening, entered square-on, and WAITS up to 2 s at a door that is closed or still opening instead of turning back (the maintainer's INBOX 2026-10-12 16:00, ask 2) - on BF2 Kamino two team-mates walked at the side of the sliding door, met its leaf, turned into the wall behind and repeated
milestone: M3
value: 5
phase: build
status: done
deps: MOUTH2
files: core/src/ai/steer.rs, core/src/sim/state.rs
declares: sweep
needs:
effort: high
branch: doors
resume: landed 4aaa2a25 2026-10-12
created: 2026-10-12
alpha: yes
---
## Why
The maintainer's playtest of BF2 Kamino conquest, 2026-10-12 (INBOX 16:00, ask 2 - read it first; it is under `## Done` now). His
eyes, the clone-facility post in the sliding-door room: after he took the post two clone team-mates walked at the SIDE
of the sliding door, the door slid and its leaf blocked them, they turned and walked into the wall behind, went back,
tried again, and repeated. "Had they waited about half a second, or walked through the exact centre of the doorway,
they would have passed on the first try." Deviating from the PS2 is approved where it makes bots play better.

## Evidence
- Code (main 5fbbbd4): `route::fill_detour` (core/src/ai/route.rs:559) - a leg the corner search cannot resolve is
  walked straight at the waypoint and left to the wall probe, "and on the discs it is always a BF2 door". Nothing in a
  bot knows a door is there or opening: `door.rs` opens a door while a soldier's feet are inside an `AnimationTrigger`
  sphere (3-5 m) and moves the leaf over (frames - 1) / 30 s; `ai/stall.rs`, `steer.rs` and `avoid.rs` read the leaf as
  a wall like any other.
- The doors: `analysis/re-bf2-doors.md` (50 door entities on 5 BF2 maps, each across an authored planning arc).
- **Card MOUTH2 (running when this was filed) measures one of them:** BF2 Kamino's pod room door, hubs 94 -> 98, 47
  stalls per 100 passes against the PS2's 4; every port bot aims at ONE point inside the room and crosses at the east
  jamb, the PS2's are spread over the opening and some hold two nodes down the door's middle
  (`engine/artifacts/lead_tmp/mouth/out/kam1_sites.md`, `mouth/tools/ps2leg.py`, `site.py`, `trace.py`). Start from
  MOUTH2's report: whatever it builds at that door is this card's first variant, not a second design (`deps: MOUTH2`).
- No snapshot of the maintainer's own case: his F9 (`engine/artifacts/playtest/last.json`, seed 75, ticks 3270-5055) was taken
  for the defenders at cp5. Which door he means is not pinned - find it from the post he had just taken in that file's
  events, or reproduce with a headless round and `BLACKBOX_ON=stuck`.

## Notes
- 2026-10-12 16:58 planner: filed from the maintainer's note. `phase: measure` with empty `files:` (the build will touch
  `core/src/ai/route.rs`, `ai/steer.rs` or `ai/avoid.rs`, `ai/stall.rs`, perhaps `door.rs` for "is this leaf moving";
  route.rs is also OBSV2's, TGAP's and FLNK's - give the build `deps:` on whichever of them is in flight).
- The PS2's bots are not known to wait at a door: check `analysis/re-soldier-ai.md` 2 and 4 for a door or "blocked by
  an entity that will move" test before calling (b) ours.

## Measured 2026-10-12 (lane 2; kit `engine/artifacts/lead_tmp/doors/`, README there)
**The census (step 1), main 4dbb460 (= MOUTH2 + DEFS), 8 seeds x 3 min, `tools/census.py`** - a door's disc is 6 m; a
stall is card MOUTH's (`pstall.py`); a reversal is a 0.5 s step of 0.25 m or more turned over 90 degrees.
| map | doors visited | door bot-min | near visits | door stalls | per 100 near | per 20 bot-min | reversals per 20 | worst door (stalls / near visits, top-3 lives' share) |
|---|---|---|---|---|---|---|---|---|
| BF2 Kamino | 10 | 20.1 | 579 | 1 | 0 | 1.0 | 29.9 | fdl-1 1 / 149 |
| BF2 Kamino, PS2 (3 captures, 153 route bot-min) | 10 | 5.8 | 133 | 5 | 4 | 17.4 | 41.7 | fdl-1 3 / 48, pod room `open` 2 / 38 |
| BF2 Death Star | 4 | 15.1 | 459 | 22 | 5 | 29.1 | 38.4 | dea1_prop_door_blast 12 / 208 (42 %) |
| BF2 Polis Massa | 7 | 38.5 | 1050 | 41 | 4 | 21.3 | 14.0 | pol1_prop_door 21 / 196; door1 20 / 200 (95 %: three lives) |
| BF2 Mustafar | 22 | 58.9 | 1823 | 85 | 5 | 28.9 | 19.0 | door_cont_4 21 / 104 (38 %) |
| BF2 Tantive IV | 14 | 68.9 | 1713 | 192 | 12 | 60.7 | 45.0 | rpodroom1 45 / 149 (18 %); rpodroom2 41 / 169 |
**Kamino's bar is met on main** (worst door 1 per 100; the PS2's own doors 4 per 100). Per-door tables: `out/cen_<map>_main.txt`.

**What the 358 stall rows near doors are** (`tools/classify.py`, the black box's rows over each stall): **6 at a leaf**
(all the Death Star's), 107 going round another body on a committed side (Tantive IV 70, Polis Massa 22), 172
**standing in a firefight** at the doorway (the walk flag flickers between target losses - not a walk that fails), 45
moving in a fight, 26 moving, 2 other. And every MEETING of a bot with a leaf (`tools/waits.py`): 784 on the five
maps, 773 through or still at it 3 s on, 11 went 2 m away (Death Star 6, Polis Massa 4, Tantive IV 1); the wait's
median 0.5 s, 2.0 s at most; **0 stuck events and 1 recover in them**.

**The traces (step 2).**
- **Death Star, the blast door at the Empire's spawn** (`dea1_prop_door_blast1`, 16 m trigger, 5 s): seed 62 unit 31,
  every pick printed (scratch `DOORS_TRACE`): `Door` at tick 68 (39 % open), `Steep` at tick 71, a side committed, 22 m
  along the hangar wall. The leaves part on a DIAGONAL (the bake's boxes are rolled 43.9 degrees): the edge that
  leaves last is a face at n.y 0.69 over the feet, the steep-floor probe's ray lands on it and Steep outranks Door.
  On main that unit commits a side there in the first 5 s of **31 of 32 rounds**. Seed 61 unit 24 at the door at
  (-83, -107): the same, 12 s away. BUILT (below).
- **Tantive IV, `rpodroom1` (leg 20 > 79, 36 of its 45 stalls):** the leg does not cross the door. The corner search
  hands out the corner (-278.9, 151.7), which lies INSIDE `barrier33` (the wall north of the corridor): the bot's
  point of hub 79 is within the search's radius (0.9 m) of that barrier, so `leg_ignores` drops it for the leg, and
  the chain round `barrier219` then goes through the wall (scratch `DOORS_DET`: 2 of 4 searches from hub 20). The bot
  meets `tan4_bldg_corridor_c`, commits a side, walks 22 m through the door beside it. Card CORN.
- **Polis Massa `pol1_prop_door1`** is card RLOOP's loop: a bot on the floor under its hub (y 22.4 against 25),
  recover 17-23, three lives hold 95 % of the door's rows.

**The three rules (step 3).**
- (b) the wait: card MOUTH2 built it and it holds on all five maps; the one hole was the steep probe at the Death
  Star's diagonal leaf. Built: a steep face in the box of an opening door's leaf is no steep floor.
- (c) no exemption needed: 0 stuck events and 1 recover in 784 leaf meetings, the longest wait 2.0 s against the 3 s
  window. Not built.
- (a) the centre line: NOT built. The PS2's lanes are 3 m either side (re-bf2-doors 11) and 773 of 784 meetings
  pass without it. What it would win is the aim point BEHIND A JAMB (a hub point or a path corner outside the
  opening): about 12 of Tantive IV's 150 door stalls in 8 seeds (the tech-room doors, legs 101 > 102, 96 > 21,
  12 > 13), Kamino's `podroom_door24` (leg 88 > 87, the point 4.0 m off the centre of a 7.5 m door: one bot in 32
  rounds) and the Death Star's leg 4 > 1 corner 6.9 m off the centre of a 10 m door. Counted for card CORN.
- A prototype of CORN's rule ("a corner inside a barrier the leg ignores is no node", scratch bit 2): Tantive IV door
  stalls 192 -> 140 and its round class 70 -> 32, but over all 32 maps (8 seeds x 2 min of snapshots) S-stalls 2566
  -> 2581, P 8977 -> 8854, Utapau's S 391 -> 471, recycles 39 -> 43: no general gain, NOT taken (`out/ab_k2.txt`).

## Built (branch `doors`, 4aaa2a2, on main bc10b82)
`steer::stop_ahead` drops a steep answer whose probe point lies in the box of a leaf of a door that is opening
(`Sim::opening_leaf_over`). **Acceptance, run:**
| check (main bc10b82 -> branch; seeds 60-91 = 32 rounds x 3 min a map) | main | branch |
|---|---|---|
| Death Star: leaf meetings that end 2 m further away | 18 of 364 | 5 of 374 (4 = one bot at a corner beside the door) |
| Death Star: unit 31 commits a side at the spawn's blast door in the first 5 s | 31 of 32 rounds | 0 of 32 |
| Death Star: door stalls (per 20 door bot-min) / reversals within 6 m (per 20) | 72 (28.9) / 102 (40.9) | 48 (19.1) / 43 (17.1) |
| Kamino: door stalls / reversals per 20 / worst door per 100 near visits (bar 10; PS2 4) | 3 / 29.1 / 4 (`door24` 1 of 26; every other door under 1) | 4 / 29.2 / 8 (`door24` 2 of 26: one more bot, aimed past the jamb, took the other side) |
| Tantive IV, Mustafar: door stalls / reversals | 712 / 634, 354 / 284 | identical |
| Polis Massa: door stalls / reversals | 77 / 47 | 79 / 47 |
| 24 seeds x 32 maps paired: rounds / recycles / maps whose rows move | 768 / 142 | 768 / 142; Death Star 24 seeds, Polis Massa 6, Mustafar 1 |
| Death Star alone, 96 seeds: death-region deaths (24 seeds read 1 -> 3) | 4 | 5 |
| the maintainer's Kamino scene: trails over 6 reversals, worst | 8 of 431, 21 | 8 of 431, 21 |
Tests: the Death Star's door from the bake's six boxes (through first try, no side; the control), the box-line
query; 11 mutations each fail one (`lead_tmp/doors/tools/mutate.py`). The scene test asked by the card ("a side
approach passes first try") is MOUTH2's `a_bot_walks_on_at_a_door_that_is_opening_and_is_through_first_try`.

## Landed 2026-10-12 20:26, main 4aaa2a2 (docs c1ad907: `analysis/re-bf2-doors.md` 12)
Gates: ci 1456 / 0 / 44; sweep MOVED (declared: the Death Star's row), re-baselined; fidelity 561 values PASS; sim,
simhash, arena, leak, goldens identical. Re-run on main's own binary (build 4aaa2a2): the Death Star's 8 seeds read
95 leaf meetings, 0 away, unit 31 commits a side in 0 of 8 rounds - as the branch. Follow-up: card CORN.
