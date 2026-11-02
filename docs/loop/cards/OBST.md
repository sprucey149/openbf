---
id: OBST
title: Bots walk straight into solid props (a round control-panel block, a cp's plinth, turrets, parked vehicles, other bots) instead of going around them: measure what the steering sees, then give a bot obstacle awareness on its way to its goal
milestone: M3
value: 5
phase: build
status: done
deps: PARR,BOTD,MOVR
files: engine/crates/core/src/ai/avoid.rs,engine/crates/core/src/ai/mover.rs,engine/crates/core/src/ai/steer.rs,engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/nav.rs,engine/crates/game/examples/round_to_outcome.rs
declares: sim; sweep; arena
needs:
effort: high
branch: obst
resume: landed e9ce6db6 2026-10-07
created: 2026-10-02
alpha: yes
first: 6
---

## Why
The maintainer, 2026-10-02 (playing, screenshot of a BF1/BF2 interior with a round raised control-panel block and a command post):
"This guy is just trying to run into this wall over and over. Is he trying to stand directly on top of the cp or
something? ... a lot of bots tend to run into walls - in this room there is the little round control panel area and a
lot of bots rather than walk around it just walk straight forward into it. Can we help the bots be more wall aware
instead of just walking into walls? They have a destination, but they are aware of waypoints and walls and vehicles and
turrets and things and other bots on their way and can go around obstacles."

## Measured (2026-10-02, lane 2, main 7b64fcd) - steps 1 and 2 are done; this is what the build is for
Evidence and tools: `engine/artifacts/lead_tmp/obst/` (README.md; `all/census.txt`, `all/spots.md` = the 20 top spots
of every map with the body's name, `all/bodylegs.txt`, `all/episodes.txt`; the instrument is
`tools/obst_census.patch`, `OBST_CSV=<dir>` on `round_to_outcome`). 32 maps x 8 seeds x 10 min, every bot on foot every
0.5 s. A WINDOW is 10 s of one life; STALLED = walking in 16+ of its 20 samples and under 3 m net.

**Step 1 - the cause, by stalled windows (16,224 of 300,866 = 5.39 %):**
| class | windows | share | what it is |
|---|---|---|---|
| a map body on a hub-to-hub leg | 4,416 | 27 % | THIS CARD. The bot paces along the body: 93 % of these windows have 3+ flips of the slide side, mean speed over 3 m/s in 87 %. The body is over 8 m wide in 84 % (buildings, bunkers, piers), 3-8 m in 12 %, under 3 m in 4 % |
| last leg at a point that is not a post (defence spot, a hunted enemy's point, an approach leg) | 4,232 | 26 % | LLEG; TAT1 alone 1,256 |
| last leg at a post's own centre | 3,426 | 21 % | new card PARR (below) + LLEG |
| nothing shuts the way, full speed, no net move | 2,162 | 13 % | the objective flips (BF2 KAM1 arc 40: attack3 / attack4 / attack2 every 1.5 s, the bot turns round each time) - CONT |
| strafing in a firefight | 1,054 | 6 % | not a defect by this measure |
| a vehicle hull | 447 | 3 % | BF2 HOT1 271 of them |
| another soldier within 1.2 m | 197 | 1 % | |
| drop / barrier / steep | 253 | 2 % | |

- **The maintainer's question, "is he trying to stand on the cp": yes.** The last leg aims at the post's centre and arrives only
  within 1.0 m of it (`CORNER_ARRIVE`, `brain.rs` `arrived_at_post`). BF1's posts carry their own solid plinth
  (`com_bldg_major_controlzone`, a cylinder about 2.6 m across): the closest a bot gets is 1.5 m (median of 662
  windows on 10 BF1 maps), so it pushes on the plinth until its objective changes. On BF2 Kamino cp0 (the room of the
  black-box snapshot: the sample depot and the round console platform) the post's centre is 3.0 m over the floor on
  top of `kam_bldg_central_sample_depot ctf1`; bots get to 1.6 m and slide on the depot's wall: 189 windows, the map's
  top spot in all 8 seeds. In that room (15 m of the post) 69 % of all walking time aims at the centre and 91 % of that
  is blocked; 84 % of all walking time in the room is blocked.
- **The PS2 does not walk at the centre** (`analysis/re-conquest-goal.md` section 4, [V]): the capture behaviour's
  destination is the post's position + a random point of its capture region, and "at the post" is "inside the capture
  region" (`FUN_002a6d18`).
- What-if, not a landing (`OBST_ARRIVE=2.0`, `lead_tmp/obst/arr2/`): arrival at 2.0 m instead of 1.0 m takes the
  plinth windows 740 -> 43, the Kamino depot 189 -> 5, all stalled windows 5.39 % -> 4.74 %. Rounds finished inside
  the 10 minutes 195 -> 190 of 256 (seed 6: 25 -> 20), so the radius alone is not the rule to land.
- **The legs that hit a body** (`all/bodylegs.txt`, 4,379 windows on an arc): the bot is over 1.5 m off the arc's own
  line in 81 %, and behind or in the first fifth of the leg in 75 % - it left the hub from its rim. But 74 % are still
  inside the corridor the two hubs' radii span, so the authored corridor does not promise a clear line: this is
  avoidance, not a graph fix. 574 legs; 36 of them hold half the windows (BF2 GEO1 hub 9 -> 12 / 13 / 0 round
  `geo_bldg_bunker_1`: 494; BF1 KAS1 107 -> 106 `kas1_bldg_hut_1`: 128; TAT2 14 -> 15 `tat2_bldg_wall0138`: 92).
- **How it looks in passing** (`all/episodes.txt`): a walking bot starts a contact episode (steering not straight) 3.8
  times a minute and spends 16.5 % of its walking time in one. Large bodies 9.4 % of walking time (41 % of it in
  episodes over 10 s); bodies under 3 m 1.4 % (0.52 episodes a minute, 11 % over 10 s), and the biggest of those is the
  post's own plinth (30 % of the small-body time), then `com_weap_gunturret`, `com_item_weaponrecharge`, trees, crates.

**Step 2 - what the steering sees** (`steer.rs` `stop_ahead`, `collision/walls.rs` `wall_sweep_body`, read):
- SEEN, 1.0 m ahead (`PROBE_DIST`), by the stepped capsule's sweep: every soldier-layer map body - buildings, props,
  crates, consoles, doors at their current pose, placed turrets (`com_weap_gunturret` is a map body) - and every
  vehicle hull (`HullFilter::All`; the hull's low parts by a second test at the probe's end).
- NOT SEEN: other soldiers. Only the separation pass pushes two apart afterwards. Measured cost is small: a soldier
  stands within 1.2 m ahead in 3.4 % of walking samples, speed there 4.9 against 5.7 m/s.
- So no body is invisible. What is missing is range and memory: the bot learns of a body 1 m before it, takes the
  tangent, and drops the side the moment the straight way opens, so on anything wider than a few metres it paces.

## Notes
- Related and already landed: OFFG (ramps were walls), BOWL (rim rails), STLVL (levels), STARC (walking in place at the
  POL1 radar door and similar - if one rule covers both, close STARC's part and say so there), RCYC (the last-resort
  recycle that hides this defect). CONT is about contact with enemies, not obstacles.
- New probe binaries run under the memory cap (INBOX 2026-10-01 22:00).

## Black box evidence (card BLKB, 2026-10-02) - a first spot, with the body's name
The tool this card should use now exists: `cargo run -p game --example blackbox_read -- <file>` (root `CLAUDE.md`).
Its first windowed snapshot (BF2 Kamino conquest, seed 42, 16 v 16, built at ce63165 + BLKB; file
`engine/artifacts/lead_tmp/blkb/win1/playtest/prev1.json`, 34.5 s into the round) already shows the complaint at the
Rebel spawn post (cp1, hub 100, around (-184, 68.3, 33)):
- bot #7 (wookiee, attack cp2): steering blocked (slide / fan) for 27.5 s with 6.3 m net, at (-197.4, 68.3, 31.4):
  `Wall`, body 1096 `kam_bldg_central_sample_depot ctf1`;
- bot #3: blocked 15.0 s (net 4.9 m) on the same body, then sliding on body 1014 `kam_bldg_console_platform
  kam_bldg_console_platform10` 0.59 m ahead;
- bot #1 (defend cp0, LAST leg to hub 100, waypoint 3.0 m away): sliding on body 1096 0.5 m ahead, stall watch 11 s -
  the "is he trying to stand on the post" case: its aim point is behind the depot's wall.
The headless round at the same seed shows the same body (`lead_tmp/blkb/new/bb/bf2_kam1_s42.json`: unit 0 blocked 30.5 s,
net 0.6 m). Step 1's "name the collision body" is `Probe::body` + `names.bodies` in the file; `BLACKBOX_ON=stuck` on
`round_to_outcome` snapshots the first 30 s stuck run of a round.

- 2026-10-02 (the maintainer's bot programme): this card is one of six - JITR, FFLN, BOTM, SAIR, BOTD (see any of them). The mod-tools docs (`community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/game.md`) say the PS2's AI does NOT see objects, only authored BARRIERS: check first whether the round console in the maintainer's screenshot has a barrier on the disc and whether the port's corner walk uses it. Use the black box (`engine/artifacts/playtest/`, `blackbox_read`).
- PARR 2026-10-02 (landed 6305dd9): the `OBST_CSV` observer is in main (`round_to_outcome`, module header) - no patch needed; census scripts `lead_tmp/obst/tools/`, plus `lead_tmp/parr/pushing.py` / `pushatk.py` (push samples near a post). Base for this card = `lead_tmp/parr/census_v2` (pre-GREN) or re-run on main. BF2 Kamino's depot is yours: 130 windows of bots LEAVING cp0 on the arc leg hub 100 -> 103 with `kam_bldg_central_sample_depot` between (0 last legs). The census's `post` class means 'aims at the post's centre' and is now only the fallback: judge last legs by `post` + `spot`.

## Planner 2026-10-02 17:45 - waits for BOTD (deps PARR,BOTD)
The maintainer's order is JITR, FFLN, BOTM, SAIR, BOTD, then OBST; `first:` orders only among eligible cards, and with PARR landed OBST would have been taken before SAIR / BOTD exist. BOTD's sketch item 3 replaces the fan / slide / sidestep patches with ONE general avoidance (barriers, props, vehicles, turrets, soldiers) - this card's wall-follow is that layer's first case, so it is built from the design: BOTD rewrites What to build here (keep Measured and the acceptance) or cuts a successor and closes this. Undo: `loopctl.py set OBST deps=PARR`.

## Note from JITR (2026-10-02)
A slide-side memory (keep the side 1.5 s after the way ahead opens, `AiState::slide_last` / `slide_keep`, `SLIDE_KEEP`) landed with JITR for HOLDERS only. Tried for every walker it was a wash over 3 maps x 8 seeds x 2 snapshots (`lead_tmp/jitr/tools/jitter.py`): KAM1 jitterers 16 -> 6 and side-flip windows 38 -> 27, but KAS2's stuck-spot pacers (52,66) / (170,66) 6 -> 28. If OBST wants it for leg walkers, the switch is the `holding.is_some()` guard in `brain.rs` at the `steer_dir` call. JITR's re-run of the maintainer's scene: `SWEEP_SLOT0=player SLOT0_AT=x,y,z` on `round_to_outcome` (an idle invincible human at a point) + `BLACKBOX_AT`.

## BOTD (2026-10-03) - "What to build" is REPLACED by the design's avoidance (bot-design.md 4.7, D3); Measured and Acceptance above stay
**New evidence** (bot-design.md M2, M3; `lead_tmp/botd/tools/inbarr.py`, `barr_cross.py`, `sample.sh`): the PS2's bots stand inside a soldier barrier for 0.3-1.2 % of their time (the port's 1.1-3.9 %) and within 1.0 m of one for 23-26 %. Of this card's 4,416 `body` stalls the bot is inside a bare barrier in 36.0 %, inside one grown 0.5 m in 72.0 %, within 1.0 m of one in 84.0 % (controls: `open` 12.2 / 21.3 %, `hull` 4.7 / 5.8 % inside). The bodies bots stall on are the ones the disc's barriers ring, and the port's bots stand inside the keep-out the PS2 respects. (The whole line to the waypoint crossing a barrier, 95.6 %, is no evidence: a random line of that length does in 90.6 %.) The steering's probes are 53-71 % of the whole sim's CPU.
**Build** (`ai/avoid.rs` + `ai/mover.rs`; `steer_dir`'s slide / flip / fan / back, `slide*`, the side-step are deleted):
1. Barriers every step from where the bot stands (the PS2's rule, SAIR 2): the segment to the aim point against the soldier barriers grown by the body radius + 0.5 m (a grid over the barriers); aim at the silhouette corner on the committed side; the side chosen once per obstacle (the shorter way) and kept until it is behind; several in the way: `nav::detour`'s order, re-aimed each step. A bot never walks into a barrier; one that is in one leaves by the nearest edge first; a goal in one is refused.
2. Parked vehicles' and turrets' footprints are rectangles passed by their corners the same way; a body the probe hits where no barrier lies becomes a disc remembered 3 s.
3. Soldiers and moving vehicles: every 3rd tick, 15 candidate velocities (7 directions at full and at half speed about the wanted heading, and "wait") scored by the distance from the wanted velocity, the distance from the current velocity, a side cost (keep right; ties by unit index) and 1 / time to collision within 2.5 s over at most 6 neighbours inside 4 m. Detour's weights 2.0 / 0.75 / 0.75 / 2.5 to start (bot-design.md 15, X10).
4. The sweep becomes the safety probe: `way_ahead` + `floor_ahead` 2 m ahead every 5th tick, only off ground the plan has checked (a leg's corridor less its barriers; a segment validated when a position was chosen).
**Acceptance, added:** **B10** stalled windows <= 2 % (5.39 % at 7b64fcd) with the body, spot and post classes each halved; **B13** ms a tick at 33 units at most half of bot-design.md M2's (0.49 ms KAM1, 0.75 ms MYG1) and the steering under 25 % of the stack samples; a scene test - 8 bots each way through a 3 m corridor pass without a stall (mutation-checked); the share of bot time inside a soldier barrier (`inbarr.py` on BOT_SAMPLES) <= the PS2's on KAM1, POL1, DEA1, KAS2; the residual list: `body` stalls NOT within 1 m of a barrier (16 % today).
Build step 4, after MOVR. In BOTV's deps.

## Planner 2026-10-03 03:35 - two recycle spots nobody owns (bar B10: "no spot recycles on 2 or more seeds")
- **BF2 KAM1 (-12..-18, 60, 108), leg 46 -> 44**: recycles on 2 of 8 seeds before JARC, on 4 after it (KAM1 2 -> 7 in
  all; (84, 54, 144) on 2 more), one seed's stuck_max 11,940. Untraced. It is 10 m from the maintainer's F9 scene
  (-4, 60.3, 108.6). Rows: `lead_tmp/jarc/sw_null`, `lead_tmp/jarc/sw_jarc`.
- **BF2 HOT1 (-264, -12..0)** (a showcase map): 13 -> 30 recycles over HUBM's 24 seeds; audit HUBM reads the map's rise
  (59 -> 77 over 32 seeds, the spot 25 -> 37) as inside seed noise (t +0.6). The spot itself recycles on many seeds
  either way. Probe: `BLACKBOX_ON=recycle round_to_outcome 60000 <seed> bf2_hot1`.
Class both with this card's census (`OBST_CSV` + `lead_tmp/obst/census.py`) when it is re-taken; a spot that is a
last leg goes to LLEG, a body on a leg stays here.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**
- 2026-10-05 planner: `effort: high` (the builds the maintainer's top concern rests on; a build runs at medium by default). Undo: `loopctl.py set OBST effort=`.

- 2026-10-06 MOVR landed (6061eec): `mover::run` now PICKS the steering's heading on every 3rd tick
  (`HEADING_PERIOD`, phase by unit index) and holds it - alone it took recycles 73 -> 27 on nine maps and KAM1's
  pinball windows 2.2 -> 1.47 %; the avoidance goes into that pick. The walk answers `Walk` and the bot carries
  `Behaviour` (not in the black box's unit row yet: add it with the Mover's status here). `stayers.py`
  (`lead_tmp/movr/tools`) splits a staying window's path by state: after MOVR 70 - 85 % of it on KAM1 / KAS2 is bots
  sliding or running at a wall. The PS2's stuck clock, radii and leg aim wait for this card: MOVR2.

## Landed 2026-10-07 (e9ce6db) - in part; report `loop/reports/20261007_0048_card_OBST.md`
Built: `core/src/ai/avoid.rs` - one side per obstacle (a commitment the progress window does not swap), an inside corner
turned along the next wall, the leave rule (left only when the free straight way ends nearer the aim point than the walk
has been; on its own level), a small body walked by before it is touched. `steer_dir` and JITR's slide memory deleted.
- Census `base` -> `final` (main 6061eec, 8 seeds x 32 maps x 10 min, `lead_tmp/obst/ab/`): stalled windows 2.44 % ->
  1.13 %; body 2,832 -> 808; spot 1,311 -> 490; post 414 -> 183; GEO1 hub 9's bunker legs 290 -> 51; contact on bodies
  under 3 m 0.58 % -> 0.20 % of walking time. Full rounds: 256 / 256 -> 256 / 256, recycles 105 -> 64.
- NOT built (card OBST2): barriers every step, vehicle / turret footprints, soldiers in velocity space and the corridor
  scene, the probe's cost (B13: unchanged), `Behaviour` in the black box's row.
- A defect of this landing, found by its own scene check and fixed by card OBSF: a holder circled a crate's rim on the
  early turn (the maintainer's scene 4 trails over 6 reversals of 451, worst 17; before OBST 3 of 500, worst 9).
- The two recycle spots the planner parked here (2026-10-03) were NOT classed by spot. Whole-map recycles over 8
  seeds: BF2 KAM1 1 -> 0 (base 6061eec), 4 -> 3 (today's main 9cd9b52); BF2 HOT1 1 -> 17 (base 6061eec), 6 -> 4
  (today's main; 8 over 24 seeds). The swing between bases says 8 seeds do not settle them: OBST2's residual list.
- Tools: `lead_tmp/obst/tools/` - `run.sh <bin> <name> [ENV..]` (the census of one binary, 2.5 min), `cmp.py <run>..`
  (totals, per map, recycles), `sw.sh <bin> <name>` (sweep_seeds --botq of a binary; `SEEDS=9-24`), `scene_seeds.sh`
  (the maintainer's scene, 40 snapshots). The runners `cd` into a worktree for the assets (the card's is gone: point them at a
  scratch one). Kept: every run's census / seeds.md / sweep rows in `ab/`, the three patches, `scene_final/`,
  `scene_fix3/`; the binaries and the BOT_SAMPLES rows were deleted (22 GB).

- AUDIT 2026-10-07: PARTLY - the census and sweep numbers hold on today's base (2.41 -> 1.25 % stalled, recycles 466 -> 244 over 32 seeds, laps 35 -> 22 per 1000 windows); three places hold bots longer than before and the tests do not referee them: card OBSR. Evidence `lead_tmp/audit-obst/README.md`, report `reports/20261007_0255_audit_OBST.md`.
