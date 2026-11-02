---
id: CWSP
title: The AT-AT and the AT-TE are spawn posts: on the PS2 14 of BF1 Hoth's 24 Imperials (6 of 16 on BF2) spawn 8-9 m from an AT-AT and one of them pilots it from the first seconds; the port spawns nobody there and no AT-AT is ever boarded
milestone: M2
value: 5
phase: build
status: done
deps: WAI
files: engine/crates/core/src/sim/spawn.rs,engine/crates/core/src/vehicle/walker.rs,engine/crates/game/src/vehicle_class/walker.rs,engine/crates/game/src/vehicle_pads.rs,engine/crates/game/src/walk.rs,engine/crates/game/src/main/battle.rs,engine/crates/game/examples/round_to_outcome.rs
declares: sweep; sim
needs:
effort:
branch: cwsp
resume: landed 69ba2670 2026-10-02
created: 2026-10-02
alpha: yes
---
## Why
Measured by card WAI2 (2026-10-02, `analysis/re-walker-ai.md` section 13, `reports/20261002_0520_card_WAI2.md`). On the PS2 the
command walker (`commandwalker`: BF2 FUN_0035baa0, vtable 0x62be40; BF1 matrix header 0x53e1e8) is a walker plus a
command-post component (`re-vehicles.md` section 4: B1 FUN_002a2968 -> FUN_002a0b28, B2 FUN_0035bd30 -> FUN_00359df0;
SpawnPointCount, SpawnPointLocation, ValueBleed) and its team spawns at it:
- BF1 HOT1: 14 of the 24 Imperials are first seen 8-9 m from an AT-AT (9 at one, 5 at the other); a pilot sits at
  12.3 s / 14.2 s (2-4 s after spawning), a gunner at 13.1 s / 14.2 s, and both walk from ~18 s at 2.0 m/s.
- BF2 HOT1: 6 of 16 (and a respawn at 34.9 s); pilots at 17-24 s; one AI each for the whole 240 s run, 414-452 m
  across the map, (229,51) -> (-198,90) and (206,120) -> (-200,55).
- BF2 GEO1: 2 clones of the opening wave and two respawns appear 7-20 m from the AT-TE; piloted from 18.5 s.
The port has no such post (`grep -ri commandwalker engine/crates`: the class is a Walker and nothing else), so every
Imperial spawns at the fixed posts: no bot comes within 168 m of an AT-AT on BF1 HOT1 or 205 m on BF2 HOT1 in any of 4
seeds (`lead_tmp/wai2/census2/`: `far:over_40m` on 24724 of 24724 thinks), and the sweep reads `imp_walk_atat 0/2`
at every sampled time on both games. The player cannot spawn at one either.

## Evidence
- `engine/artifacts/lead_tmp/wai2/ps2/`: `hot1_*.csv`, `hot1L_*.csv` (BF2 HOT1, 60 s and 240 s), `geo1*_*.csv`,
  `b1hot1_*.csv`; `load_an.py` / `load_an1.py` / `long_an.py` print the tables; README.md there.
- Port: `lead_tmp/wai2/census2/bf?_hot1_s*.txt` (the `claim census:` lines), built from `lead_tmp/wai2/census.patch`.

## RE result (2026-10-02, `analysis/re-command-walker-post.md`, docs d3242d7; report `reports/20261002_0825_card_CWSP.md`)
Acceptance 1 is DONE (one Opus verifier: 56 of 59 SUPPORTED, 3 PARTLY folded in; RAM of six states, five live runs,
one live BF2 spawn-screen session). What the build needs:
- The walker owns an ordinary command post from its init to its kill; the pad's team (team 1 on both Hoths and both
  Geonosis maps) for its whole life; no capture region. The pad rebuilds the walker (Hoth 120 s, Geonosis 45 s) and
  the post returns with it.
- Its team spawns at six CLASS points in the walker's LIVE frame, one drawn at random, on the ground beside it:
  AT-AT `SpawnPointLocation` (x y z angle) 8 0 0 5 / 8 0 4 5 / 8 0 -4 5 / -8 0 0 355 / -8 0 4 355 / -8 0 -4 355;
  AT-TE -6 0 4 0 / 6 0 4 0 / -7 0 0 0 / 7 0 0 0 / -6 0 -4 0 / 6 0 -4 0 (both games; already in the class RONs, hashes
  844603213 `SpawnPointCount` and 2691158341 `SpawnPointLocation` — no rebake). PS2: BF1 Hoth 14 opening spawns
  within 0.13 m of an authored point; BF2 respawns keep appearing beside the walking AT-AT, 395 m from its pad.
- Both spawn screens list it and the player can take it (live on BF2: D-pad onto the AT-AT, spawned beside it).
- An AI member draws a POST (BF1 uniform, BF2 by AISpawnWeight = uniform on Hoth / Geonosis), then a point of it:
  PS2 share per AT-AT a third (BF1 Hoth, 3 posts) / a quarter (BF2 Hoth, 4 posts). The port draws a node from the
  pooled list — that difference is card SPST, not this one.
- Death: no ticket or bleed effect in BF2 (the rule the port plays everywhere), so the walker's post is NOT a
  `CommandPost` in `Sim::cps` — a spawn source only. (BF1 counts its ValueBleed 10: recorded in the doc, not built.)
- Test vector (RAM, BF2 Hoth): walker at (228.79, -6.00, 51.06), right (0.308, 0, 0.952), forward (-0.952, 0, 0.308)
  -> point 0 (8, 0, 0) at (231.26, -6.00, 58.68), point 5 (-8, 0, -4) at (230.14, -6.00, 42.22).
- The opening wave: the PS2's walker exists before the first unit spawns; `setup_conquest` places the opening wave
  before any pad has spawned (doc section 10.3) — decide the order first.

## Build result (2026-10-02, commit 69ba267 on branch cwsp; report `reports/20261002_1156_card_CWSP.md`)
- 2a RAN: `core` tests `the_ram_walker_gives_its_six_points` (RAM vector: six points within 0.01 m of the transform by the
  RAM rows, 0.015 m of the tool's printed points — the rows are RAM's to 3 decimals; never the enemy's) and
  `a_dead_walker_gives_none_and_the_rebuilt_one_gives_again`; `game` test `the_command_walkers_carry_their_six_spawn_points`
  (both discs' AT-AT and AT-TE). Mutation-checked three ways (pool without walkers: both fail; no team test: the vector
  test fails; no matrix: the vector test fails).
- 2b RAN (`SPAWN_CENSUS=1`, seeds 1-8, `lead_tmp/cwsp/build/census/`): opening wave at a command walker — BF1 HOT1 12 of
  17 on every seed (null 0; PS2 14 of 24), BF2 HOT1 5 of 17 (null 0; PS2 6 of 16), BF2 GEO1 0 of 17 and 1 / 1 / 1 / 0 /
  0 / 0 / 1 / 1 Republic spawns at the AT-TE in the first 60 s (>= 1 on 5 of 8 seeds — NOT on every seed: the opening
  wave walks the pooled node list round-robin and its 17 units never reach the AT-TE's six points behind cp3 / cp6 / cp7's
  nodes; that is card SPST's post-then-point draw). Whole round: 463 of 674 / 543 of 1063 / 101 of 500 spawns at a walker.
- 2c reported: `imp_walk_atat` piloted at 60 s / 180 s — BF1 HOT1 2/2 on 8 of 8 seeds / 2/2 on 7, 1/2 on 1 (null 0/2
  everywhere; PS2 2/2); BF2 HOT1 0/2 at both on every seed, and BF2 GEO1's AT-TE 0/1 — units now spawn beside them and
  still do not board: card WAI2 (note added there). BF1 GEO1's AT-TEs 2/2 at 60 s on 8 of 8. 30 s is not sampled by the
  referee and no AT-AT path was compared.
- 3 RAN: 8 seeds x 32 maps beside the null variant (`lead_tmp/cwsp/build/sweep_{null,cwsp}/seeds.md`): 256 of 256 rounds
  finish in both; only the five maps with a command walker change hash (BF1 HOT1 / GEO1 / RHN1, BF2 HOT1 / GEO1); the
  null variant is byte-identical to the gate's seed-42 baseline on all 32 maps. Win split team0/team1: BF1 HOT1 0/8 ->
  0/8, BF2 HOT1 0/8 -> 0/8 (median round 558 s -> 822 s), BF1 GEO1 0/8 -> 1/7, BF2 GEO1 0/8 -> 0/8, BF1 RHN1 0/8 -> 0/8.

## Notes
- Not built here: the PS2's pull-back ray from the walker's body (the port seats the unit with `spawn_ground`,
  PROVISIONAL); BF1's `commandhover` posts (hovernaut / MTT, Rhen Var Harbor — same six points, not a showcase map); the
  spawn screen's cursor does not steer the respawn (card SPSEL) — the walker is listed, by its class `Label` ("AT-AT";
  the AT-TE authors none and reads "Command walker", ours).
- BF2's AT-AT also fails the port's route test (best drop-hub gain -14 / -24 m): card WAI2's build note (c).
- A player-visible gap on a showcase map (Hoth), so value 5; M3 is touched too (where a team spawns).
