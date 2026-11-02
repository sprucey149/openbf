//! Play a headless conquest round on every baked map until one side wins, and
//! say how long it took — the "does a round actually finish" smoke test.
//!
//! ```text
//! cargo run --release -p game --example round_to_outcome [max_ticks] [seed] [map ...]
//! ```
//!
//! Map defaults come from `game::conquest_defaults`, including calibrated
//! tickets. `SWEEP_COMBATANTS=N` overrides both teams' combatant slots (1..64)
//! for explicit stress/control runs. `SWEEP_TICKETS=A:B` overrides tickets;
//! `SWEEP_TICKET_PCT=N` scales map-default tickets when no explicit pair is set.
//! `SWEEP_TEAM_PCT=N` scales map-default combatants the way Instant Action's
//! Team size percentage does (`ui::population::team_counts`: halves up, the
//! pair stops at 32 together) when `SWEEP_COMBATANTS` is not set.
//! `SWEEP_MISSION_TICKETS=1` takes the mission's own tickets (scaled by
//! `SWEEP_TICKET_PCT`) instead of the map defaults: with `SWEEP_COMBATANTS=16`
//! it is the 16-per-side setup the difficulty tiers were tuned at.
//! Both values and the resolved roster are printed in each `setup` row.
//!
//! NOBODY is human here by default: slot 0 is vacated before the opening wave.
//! The opening roster and respawn targets are the same combatant counts used
//! by Instant Action. A `SWEEP_PROXY` replaces one ordinary bot, occupying
//! one slot; `SWEEP_SLOT0=player` keeps an idle human in one team-0 slot.
//! `SWEEP_NOFILL` is obsolete: there is no additional filler above the roster.
//! `SLOT0_AT=x,y,z` stands a retained human there, invincible, for scene replay.
//!
//! Missions supply factions, classes, post ownership, death regions and rules.
//! Shared common props are merged into collision, as in the game.
//!
//! ## Which teams a map fields
//! Each team is a `game::roster::FactionRoster`: every class the mission
//! names, with that class's own ODF constants, health, and primary weapon,
//! registered once per weapon NAME (two classes with the same rifle share one
//! `Sim::weapon_classes` index). The AI then come out of `Sim::spawn_ai`,
//! which draws from the registered classes using `Sim::ai_class_weights`
//! and the shared weighted selection in `Sim::ai_class`. No bot has an
//! additional slot above the configured population. The initial
//! idle player's template remains class 0 (the mission's first class).
//!
//! Three things can make a team something other than its mission's mix, and
//! all three are printed:
//! - a map with NO mission RON (`mission_for` finds none) takes the faction's
//!   default class list;
//! - a mission whose classes are in no baked side LVL takes the same default
//!   list — no shipped mission does today: all ten class RONs are baked
//!   (`assets_baked/classes/`) and `game/tests/roster.rs` pins that every
//!   mission's mix resolves without a fallback;
//! - a map whose class RONs will not load at all falls back to the built-in
//!   trooper, so the sweep still plays the round.
//!
//! Shared `common_<game>` props are merged so their collision is in the world
//! exactly as in the game.
//!
//! `SWEEP_ASSET_DIR` selects an isolated baked asset root; `SWEEP_MISSION_DIR`
//! selects its matching missions. These permit cross-source comparisons without
//! replacing the development bake. `SWEEP_HASH_EVERY=N` emits read-only state
//! checkpoints every N ticks for comparisons of the complete round.
//!
//! ## Parallel (Phase A.3)
//! Each map's sim is single-threaded and independent of every other map's, so
//! the maps run on WORKER THREADS: `SWEEP_THREADS` of them if that env var
//! parses as a `usize`, else `available_parallelism()`, capped at the number
//! of selected maps. Each worker builds its map's whole report into a
//! `String`; STDOUT is written only after every worker has joined, in map
//! discovery order, so the sweep's stdout is byte-identical run to run and to
//! the serial output it replaced. PROGRESS goes to STDERR as maps finish
//! (`<key> done (n/m, s s)`, `s` = that map's own wall time), which is the
//! only thing a redirect of stdout loses.
//!
//! The last stdout line is the SCOREBOARD over the maps that ran:
//!
//! ```text
//! sweep: finished F/M  stuck_max median A (finished-only B)  >10k C maps  sum D
//! ```
//!
//! The 32-map finished count is a high-variance metric (PLAN Log, "Phase A.2
//! landed": intermediate variants flipped the winner on 5-8 maps for small
//! changes), so the steadier columns sit next to it: the LOWER median of
//! `stuck_max` over every map that ran, the same median over the maps that
//! reached an outcome, how many maps are over 10,000 stuck ticks, and the sum.
//! `-` where there is no map to take a median of.
//!
//! ## The stuck census — `STUCK_CENSUS`
//! A MEASUREMENT for the no-outcome maps (queue item SK): which units are
//! stuck at the end of the round, where, and what the planner and the probe
//! say about their leg. `STUCK_CENSUS=1` lists every ALIVE unit whose
//! `Route::stuck_run` exceeds 5,000 ticks at the last tick (`STUCK_CENSUS=<n>`,
//! n ≥ 2, sets another threshold), with a per-map summary, and names the unit
//! that set the map's `stuck_max` (tracked after every tick) and what it was
//! doing then (card BSTK: `foot`, `seated` or `droid`), and (card WHY) the
//! round's failed walks by code (`failed walks N [wall .. recycled ..]`,
//! `core::ai::why::FAILS`: a window's failure and what came of it; card
//! ARCM: the arcs closed to a bot, the arcs a team shunned, the legs drawn
//! round one and the legs that kept one for want of another way). It goes to
//! STDERR, after every worker has joined and in map order, so STDOUT is
//! byte-identical with and without it and two censuses `cmp` like two sweeps.
//! It only READS the sim — every query is one the AI or the verbose dump
//! already makes — so the round it describes is the round the sweep played.
//! Each unit's line ends in `=> <verdict> [<flags>]`; the verdict is the
//! first of these that holds (the order is this census's, not the AI's):
//! - `not-walking` — the steering did not walk last tick (`SteerPick::Idle`:
//!   holding a fight, standing on its defence spot, or no goal), yet the
//!   progress window still ran — it counts any stick deflection, the D11
//!   strafe dodge included;
//! - `off-graph` — `locate` found no containing hub and no arc corridor, only
//!   the NEAREST hub (the planner's `locate` never fails outright on a
//!   non-empty graph);
//! - `no-arc` — the leg is the planner's STRAIGHT fallback: neither the
//!   authored table nor the BFS had a hop;
//! - `wall` — the steering's own probe (`wall_sweep` of the stepped capsule,
//!   `PROBE_DIST`) toward the leg's current sub-point hits collision;
//! - `body` — another alive soldier's capsule stands in that probe's path
//!   (soldiers are not in `wall_sweep`; the sim only pushes them apart);
//! - `open` — none of the above: the way is clear and still no progress.
//!
//! Flags beside it: `under-terrain` (the heightfield is > 1 m above the feet
//! at an uncut point) and `dodging` (the D11 strafe is live).
//!
//! A wall slide's progress resets `stuck_run`, so a unit sliding in place
//! reads as unstuck (queue CEN). The census also lists every alive unit that
//! has held one leg (`next_hub`, `dest_hub`) for `NO_PROGRESS_TICKS` while
//! its best XZ distance to the leg's waypoint gained under `NO_PROGRESS_GAIN`
//! (and is not already within that of it): a second summary line
//! (`index:ticks`), and the same per-unit line tagged `noprog unit` (a unit
//! that is both reads `stuck unit` with its `noprog` field filled). Only
//! `stuck` units enter the verdict tally, hubs and clusters.
//!
//! ## What stops a walking bot — `OBST_CSV`
//! `OBST_CSV=<dir>` (cards OBST, PARR) writes `<dir>/<map>_s<seed>.csv`: every
//! alive bot on foot every 15 ticks (`game::unit_probe::Probe` with the
//! steering's stop test and its body, the aim point and whether a soldier can
//! stand on it, the nearest other soldier; card OBSV: the walk's kind, the
//! vehicle it boards, and the vehicle whose hull shut the way - its slot,
//! kind, class, speed, whether it is empty and how far its middle is), and
//! per map `_bodies.csv`,
//! `_posts.csv` and `_nav.csv`. It only reads the sim: stdout is unchanged.
//! The census scripts are in `engine/artifacts/lead_tmp/obst/tools/` and
//! `lead_tmp/parr/` (not in git).
//!
//! ## Deaths by cause — `SWEEP_DEATHS`
//! `SWEEP_DEATHS=1` counts every death of the round per team from the sim's
//! own events — `Event::Kill` (shot or blasted by someone) and each
//! `Event::EnvDeath` cause — and adds one STDERR line per map, in the
//! census's place and order:
//!
//! ```text
//! deaths <key>: team0 kill K region R fall F vehicle V blast B floor X under U lost L stalled S suicide Q hero_exhausted H | team1 …
//! ```
//!
//! ...and (card FFLN) who the fire landed on, from the sim's own counters: the
//! kills credited to a unit, those of a unit of the killer's own team, the
//! killer's own; the bolts that hit a unit and those that hit a friend; the
//! bot-ticks a shot waited for a friend to leave the line of fire:
//!
//! ```text
//! teamfire <key>: kills K team T self S bolt_hits H friend_hits F line_held L
//! ```
//!
//! ...and the team board's roles (card TEAM): Cover roles given, and the
//! AI seconds spent with one and on a sniper's spot (Overwatch):
//!
//! ```text
//! roles <key>: cover_given N cover_s C overwatch_s O
//! ```
//!
//! ...and the flank routes (card FLNK): the ordered post pairs with an
//! authored main route, those with a flank route, the Flank roles given and
//! the AI seconds with one, how the roles ended (A the last leg into the
//! post, D a death, T another objective, O anything else) and the stalled
//! bots recycled while holding one, the legs
//! walked along a flank route, the soldier hubs
//! no AI soldier on foot stood at
//! (feet within max(radius, 3 m), every 8 ticks; SAIR's `hubuse.py` rule;
//! bar B11) and each route's share this match (`from>to:share`):
//!
//! ```text
//! flank <key>: pairs P routes R given G flank_s S ends A/D/T/O stalled S legs L hubs_never N of H (X %) shares a>b:0.21,...
//! ```
//!
//! ...and, when a team came to stand clamped (no reinforcement beyond its
//! living units) at 3 alive or fewer, the endgame (card STEND): how long
//! the round went on from there and how that team's last units died:
//!
//! ```text
//! endgame <key>: team T clamped at tick C alive a/b | ended|open N ticks later | its deaths since: kill K stalled S other O
//! ```
//!
//! `SWEEP_SHOTS=1` (card STTAP) adds a `shots <key>:` line the same way: per
//! weapon class `name[single]:pulls/lives` — the ticks a unit holding it
//! fired on (a multi-bolt salvo is one pull) over the unit-lives that ended
//! the round with it; `[single]` marks a `TriggerSingle` class. And a
//! `standoff <key>:` line: the longest time two enemy bots stood within 25 m,
//! each seeing the other, with neither dying, and how many such pairs lasted
//! 3,000 ticks.
//! Between them a `grenades <key>:` line (card GRAI): the grenades the bots
//! threw, in all and per weapon class.
//!
//! `SNIPE_CENSUS=1` adds a `snipe <key>:` line: the AI seconds on foot with a
//! scoped rifle in hand and of those on a sniper's spot, the distance of
//! each scoped-rifle shot at a unit (median, p90, max, and per 20 m band,
//! the last 100 m and over), and the samples (every 15 ticks) where an
//! enemy soldier on foot stood 60-128 m off in the sniper's view cone with
//! a clear line, with those where the sniper saw it.
//!
//! `SPAWN_CENSUS=1` (card CWSP) adds a `spawns <key>:` line the same way:
//! where each team's units appeared — the opening wave (the alive units
//! after the first tick), the respawns of the first 60 s, and the whole
//! round's — per spawn source: a live command walker of the unit's team
//! within [`SPAWN_AT_WALKER`] m (`<class>#<vehicle slot>`), else the post
//! owning the nearest spawn node.
//!
//! ```text
//! spawns <key>: team0 opening 16 [cp1 4, imp_walk_atat#0 6, …] to60s 3 […] round 212 […] | team1 …
//! ```
//!
//! And a `spawnrule <key>:` line (card BSPN), the referee of WHICH post a
//! bot takes: at every bot respawn the posts its team could spawn at that
//! tick are read, and per team the line gives, by the node count of the
//! post taken (`n<nodes>`), the spawns seen beside the count two rules
//! expect — an even draw over the eligible posts (the PS2's: every
//! conquest post's `AISpawnWeight` is the default) and a draw over the flat
//! node list. Respawns with one eligible post are left out (no rule shows).
//!
//! ```text
//! spawnrule <key>: team0 192 [n2 61 / post 64.0 / node 31.2, n6 …] | team1 …
//! ```
//!
//! It only reads `Sim::events`, so STDOUT stays byte-identical with it on
//! (card SWPN; `scripts/sweep_seeds.py` reads it — KAM1's falls were the
//! clearest single number in STALL).
//!
//! ## The recycle list and the shares (card RCYC) — with either of the two
//! EVERY use of the last resort (`core::ai` `RECYCLE_AFTER`: a bot that made
//! no net progress, `DeathCause::Stalled`) is a defect to close, so each is
//! one STDERR line saying where and on what leg it happened:
//!
//! ```text
//! recycle <key>: seed S tick T unit U gen G team X class C pos (x,y,z) leg F->N dest D [src] wp m locus Hh floor_dy d steer s objective o
//! ```
//!
//! (`wp`: XZ metres to the leg's waypoint; `locus`: `H`ub / `A`rc / `N`earest = off the graph, with its hub;
//! `floor_dy`: the feet against the highest floor on that hub's vertical
//! line.) `scripts/sweep_seeds.py` sums them per map and lists the spots
//! (6 m cells); a spot recycled on two seeds goes on the card that owns it.
//!
//! Card UNSN: a stalled bot no human sees is moved on to a hub past its
//! failing link instead (`core::ai::unseen`); each move is the same defect,
//! one line with where it stood (`from`, the spot to close), where it went
//! and the hub there:
//!
//! ```text
//! relocate <key>: seed S tick T unit U team X from (x,y,z) to (x,y,z) locus Hh D m
//! ```
//!
//! `floor_dy` reads a ROOF over the hub as its floor, so each recycle on a
//! leg has a second line against the floors the routing itself uses (card
//! STLVL): the leg hub's resolved floor (`Sim::nav_floor`; `-` unresolved),
//! the feet against it, the XZ distance to the hub and its radius, the
//! waypoint's height and the level test:
//!
//! ```text
//! recycle_lv <key>: seed S tick T unit U hub H hub_floor f feet_dy d hub_d m hub_r m wp_y y wp_dy d last 0|1 level L
//! ```
//!
//! `scripts/sweep_seeds.py` counts the ones on ANOTHER STOREY: at the hub
//! (within its radius, 6 to 10 m) and more than 3 m off its floor.
//!
//! `POSITIONS=1` prints the map's position table as built at load (card
//! TPOS, `core::ai::tactic`): per source the points tried, standable and
//! covered, the rays and seconds, and per post the positions within 30 m.
//!
//! `FLOOR_GRID=x,z,half,step,y_top` prints the first floor under `y_top` on
//! a grid round a point, with the hubs near it (authored height, radius,
//! resolved floor), their arcs, the posts and their spawn nodes: what a
//! "wrong floor" spot looks like, and the map's penned spawn nodes with
//! their seats (`fg seat`, card LLEG). `NAV_FLOORS=1` prints every hub's resolved
//! floor, every arc the floor walk or card TGAP's wall test found
//! unwalkable and every trapped hub (`nf <key> …`): diff
//! two builds' lists before trusting a change to `Sim::resolve_nav_floors`.
//!
//! The stuck census misses a jam that oscillates (BF2 DEA1's doors pushed
//! units back and forth and reset every stuck window), so `STUCK_CENSUS`
//! also prints, from one sample per alive unit per second:
//!
//! ```text
//! shares <key>: unit-s N | walk-in-place W (p %) | locus H h % A a % N n % | steer idle .. fwd .. slide .. fan .. back .. ledge .. % | slide-in-N q % | rim R (p %) on-level L longest S s | contact20 p % | retarget R in F foot-s | stand20 S in F foot-s (p %) [behaviour n, …] | seated S (p %) foot F | top t0(x,y,z):n …
//! ```
//!
//! `locus` (the fidelity gate's `offgraph_pct` is its `N`) is of the units
//! ON FOOT only - the `F` samples with no `mount`, AI and player - and
//! `slide-in-N` of those in `N`: a gunner at a turret seat or a pilot
//! crossing open ground is not a soldier who walked off the graph (card
//! FOFG; card TURR's KAM1 +8.65 was 7.28 points of seated gunners). `seated`
//! is the alive-unit samples in a vehicle or turret seat, of `N`. Every other
//! share is of all `N` alive-unit samples.
//!
//! and under it (card FGHT) `behave <key>: foot-s N | travel n fight n evade n
//! capture n guard n board n follow n recover n none n | B12 cover C
//! at-position P of fight F (p %) modes none .. stand .. walk .. pause ..
//! move .. take .. cover .. chosen-covered .. at-cover-hint ..`: the on-foot
//! AI samples by what the bot was doing, and (card TPOS,
//! `analysis/bot-design.md` B12) of the Fight samples those at a Tactician's
//! position with cover from the target, those at one at all, all of them by
//! `FightState` mode, of the COVER ones those whose position was chosen with
//! cover from the target, and those at an authored Cover hint's position
//! (the PS2's own B12 reading: attack time at Cover hints, SAIR 4.6); then
//! `tactician <key>: queries Q ceiling C (p %)`: the Tactician's queries and
//! those in which a candidate with cover from the target passed its filters
//! (B12's ceiling: the bar is the lower of 20 % and half of it).
//!
//! Then (card HYST) `hyst <key>: flips F kept K | wakes hit H near N seen S |
//! wake changes [why n ...]`: positions replaced by another on one think,
//! queries whose winner did not beat the position held by the margin, the
//! early thinks by what woke them, and the behaviour changes those thinks
//! made by their WHY code; `tfunnel`'s last column, `x-closing`, is the
//! covered candidates the closing filter took.
//!
//! `HIT_CENSUS=1` (card REACT) adds `fire <key>: under-fire-s N | slow p % |
//! spell3 p % | fight-s N slow p % spell3 p % | bursts B lived L moved M (p %)
//! | longest S s`: the AI unit-seconds on foot within 2 s of a hit, the share
//! slower than 0.5 m/s, the share inside a spell of 3 s or more within 1.5 m
//! of where it began; the same for a bot whose behaviour is Fight; the hits
//! that open a burst, those whose bot lived 2 s more and of those the ones
//! then 1.5 m or more from where they were hit.
//!
//! `contact20`: the share of samples with an enemy alive within 20 m (card
//! CONT; `lead_tmp/offg/tools/contact.py`'s measure — the PS2 has 17-25 % on
//! BF2 Kamino, 32 % on Polis Massa); `retarget`: how often a living AI unit
//! on foot is given another target post, in how many on-foot unit-seconds
//! (the PS2: one per 233-323, `analysis/re-conquest-goal.md` section
//! 7). `walk-in-place`: moving faster than 1 m/s and under 1 m (XZ) from there
//! ten seconds later (`lead_tmp/endings/walk_in_place.py`'s measure), with
//! its five biggest 6 m cells per team; `slide-in-N`: the share of the
//! off-graph samples that were a wall slide (card OFFG's numbers).
//!
//! `rim` (card BOWL; the maintainer, 2026-10-01: bots on BF2 Kamino's hulls): the
//! samples of a unit BEYOND a barrier from its three nearest hubs
//! (`Sim::rim_drop`) and `RIM_UNDER` or more under the nearest one's floor —
//! standing on a hull past a rim rail; `on-level`: how many of those the
//! level test (`Sim::level_at`) does not call off the level, which is the
//! hole the card closed (4.5 m of hull read ON); `longest`: the longest
//! unbroken stay of one unit out there, in seconds.
//!
//! `RIM_LOG=1` adds one STDERR line each time a unit comes onto a hull (the
//! same test, every tick), in the census's place (`rim cross <key>: …`:
//! where, what the steering had picked, how far something other than its own
//! velocity moved it that tick), and
//! `LEVEL_PROFILE=x0,z0,x1,z1,y,step` prints the level test along a line
//! before the round (`profile <key>: …`: the ground under `y`, its normal,
//! `level_at` and `rim_drop`) — BF2 Kamino's y 60.3 platform rim is
//! `0,102.1,30,102.1,75,0.5`.
//!
//! `LOCUS_CSV=<file>` (card OFFG) reads positions from a CSV with `x`, `y`
//! and `z` columns — a PS2 soldier log, `lead_tmp/stall/ps2/<map>_soldiers.csv`
//! — and prints where the port's planning graph and level test put them
//! (`locus-csv <key>: rows N | locus H .. A .. N .. % | level on .. off ..
//! unknown .. %`): the PS2's own off-graph share, the target for ours.
//!
//! ## The music — `MUSIC_LOG`
//! `MUSIC_LOG=<1|2>` prints the battle's music as that Lua team hears it, a
//! line per change to stderr (card AMBM; `game::music::HeadlessLog`). It
//! only reads the sim.
//!
//! ## A black box snapshot — `BLACKBOX_AT`, `BLACKBOX_ON`
//! The playtest black box (`game::blackbox`, card BLKB) on a headless round:
//! `BLACKBOX_AT=<tick>` writes the last 60 s at that tick,
//! `BLACKBOX_ON=lost|recycle|stuck[:<ticks>]` at the first lost death, the
//! first recycle, or the first no-progress run of that many ticks (900).
//! One file per map, `<BLACKBOX_DIR>/<key>_s<seed>.json` (default
//! `artifacts/playtest/headless/`, never the player's own `last.json`); read
//! it with `--example blackbox_read -- <file>`. The recorder only reads the
//! sim: the round's result and hash are the same with it on.
//! `BLACKBOX_TIMING=1` prints what it cost per tick and per write.
//!
//! ## Jet census — `JET_CENSUS`
//! `JET_CENSUS=1` (issue #72) prints one `jets <key>:` line: the bot jet classes'
//! hops and their spacing, hover attacks, each jet-only route link by direction
//! (started / arrived / died / gave up) and how those soldiers died to the world
//! (`JetCensus`).
//!
//! ## Droid census — `DROID_CENSUS`
//! `DROID_CENSUS=1` (card DROIDAI) prints one `droids <key>:` line: the bots'
//! remote droid launches per carrier-minute, the share made with the body
//! exposed, the share of carrier time spent flying, and how the droids ended
//! (`DroidCensus`).
//! `BLACKBOX_ON=end` writes at the round's end (card DPRX: with a proxy,
//! the file's `session` block is the proxy's whole-round tally, the input
//! `scripts/human_profile.py` reads).
//!
//! ## The human proxy and the difficulty check (card DPRX)
//! Run settings, all optional (stderr only: stdout is unchanged with them):
//! - `SWEEP_TEAMS=<t0>:<t1>` sets each team's tier (`Sim::team_difficulty`);
//! - `SWEEP_ERA=cw` reads `missions/<key>_con_cw.ron` where there is one;
//! - `SWEEP_FLYER_TRACE=1` adds a per-second line per ridden flyer (`=<n>`, one every n
//!   ticks) to the flyer census row;
//! - `SWEEP_MISSION_DIR=<dir>` reads the missions from another folder (the
//!   faction-swap copies of a balance run);
//! - `SWEEP_PROXY=<team>:<average|good>[,skill=N][,reaction=TICKS]
//!   [,view=X][,circles=0|1][,strafe=P][,roll=P][,move=P][,grenade=P]
//!   [,hand=0|1][,hfloor=RAD][,hacquire=RAD][,hsettle=S][,hup=M][,head=0|1][,hunts=0|1][,class=N|-1][,mend=SHARE][,picks=0|1]`
//!   makes the first bot of `<team>` on foot after the opening wave a HUMAN
//!   PROXY (`core::ai::proxy`): the sim treats it as a human for every
//!   human-only rule, it plays with that strength's fixed profile (the
//!   `key=value` items override single fields: the calibration loop's knobs),
//!   and it keeps the flag through its deaths.
//!
//! `MATCHLOG_DIR=<dir>` with a proxy writes the proxy's match log
//! (`game::matchlog`, the file the windowed game writes for its player) as
//! `<dir>/<map>_s<seed>.jsonl` at the round's end: the calibration loop's
//! input for the proxy side (`scripts/human_profile.py`).
//!
//! Every round then prints one `round` line to stderr:
//! `round <map> s<seed> <tier> winner <0|1|-> margin <t1 - t0> tickets a/b`
//! and, with a proxy, `proxy <team> <strength> kills K deaths D shots S hits
//! H acc A life_s L` (its tally is `game::blackbox::session`'s: the same
//! definitions as a human's black box).
//!
//! `round_to_outcome --difficulty-check [--strength good|average]
//! [--seeds N] [--tiers easy,normal,...] [--ticks N] [map ...]` runs, per
//! tier, every map with the proxy on team 0 and on team 1 for seeds 1..=N
//! (the map's side bias cancels in the average; it is never corrected), and
//! prints per tier the proxy team's wins, mean margin, the proxy's K/D and
//! accuracy, and PASS / FAIL against `core::ai::proxy::TARGETS`. It plays 16
//! per side on the missions' tickets, the setup the tiers were tuned at (not
//! the map defaults); `SWEEP_COMBATANTS` / `SWEEP_TICKETS` override. Without
//! maps it runs [`CHECK_MAPS`]. `SWEEP_THREADS` (default 8) rounds at once.
//!
//! ## Animation census — `ANIM_CENSUS`
//! `ANIM_CENSUS=1` (card ANIML) runs every soldier's animator beside the
//! round (no renderer) and prints `game::animator::census`'s lines: the shots
//! fired on the move that show the shoot clip over a movement clip, and per
//! weapon set how often the upper body plays a clip of its own. It also
//! counts what every soldier on foot holds (card WHELD): `hands:` lines of
//! unit-ticks by player / bot, the slot in hand (the class's primary, another
//! channel-0 slot, channel 1) and the model drawn for it
//! (`soldier_vis::weapon_in_hand`, `DrawPlan::weapon_in_slot`; `hidden` = a
//! thrown weapon's `HideOnFire`, `-` = a slot without a model).
//!
//! ## Unit samples — `UNIT_SAMPLES`
//! `UNIT_SAMPLES=<dir>` writes `<dir>/<key>_s<seed>.csv`: every alive unit
//! once a second (30 ticks) — where it stands, how fast it moves, where the
//! planning graph says it is (`H`ub / `A`rc corridor / `N`earest = off the
//! graph), the floor under it against its locus hub's, what the steering
//! picked, whether it has a target, and its drop under the playable level
//! (`Sim::level_at`: 0 on it, -1 unknown) and whether it holds a position
//! (card HOLD: 2 holding, 1 advancing after a hold), and its leg (card OFFG:
//! the arc, the hubs it walks from and to, the goal's hub, metres to the waypoint;
//! `artifacts/lead_tmp/offg/tools/legs.py` ranks the legs bots fail on).
//! It only READS the sim, so STDOUT is
//! byte-identical with it on. `artifacts/lead_tmp/fable2/samples.py` reads it
//! (standing-still share, off-graph share, below-the-deck units).
//! `TRACE_UNIT=<index>[,<from>,<to>]` prints that unit every 10 ticks to
//! STDERR (position, steering, leg, lost state, level).
//!
//! ## Heavy-weapon census — `HEAVY_CENSUS`
//! `HEAVY_CENSUS=<dir>` (card HVYW) writes `<dir>/<key>_s<seed>.hv.csv`: what
//! the bots on foot fired, by weapon and by the shooter's target, and who
//! destroyed which vehicle (`HeavyCensus`); beside it `<key>_s<seed>.tk.csv`,
//! one line a team kill by a bot on foot (the weapon, blast or bolt, where
//! the victim stood against the shooter's line to its target). Read-only.
//!
//! ## Bot-quality samples — `BOT_SAMPLES`
//! `BOT_SAMPLES=<dir>` (card BOTM) writes `<dir>/<key>_s<seed>.csv`: every
//! alive unit four times a second (the black box trail's rate) — position,
//! health, seat, the steering's pick and slide side, hold, target and its
//! range, the post it walks at and its objective's — and
//! `<dir>/<key>_s<seed>.ev.csv`: every shot, hit and kill with both units'
//! teams and the actor's speed.
//! `scripts/bot_quality.py` reads both (and the PS2's RAM samples in the same
//! columns): reversals, path / net, spacing, team kills, contact, map use.
//! `BOT_SAMPLES_SECS=<n>` stops the rows after n seconds of the round (the
//! PS2's samples are 3 to 9 minutes long). It only READS the sim.

use core::sim::PadState;
use game::walk::WalkSim;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// `writeln!` into a report `String`, which cannot fail.
macro_rules! outln {
    ($out:expr, $($arg:tt)*) => {{ let _ = writeln!($out, $($arg)*); }};
}

/// Card DPRX: one round's run settings (module header). Read from the
/// environment, except in `--difficulty-check`, which sets them per round
/// on the worker's thread ([`CFG`]).
#[derive(Clone)]
struct Cfg {
    difficulty: core::skill::Difficulty,
    teams: [Option<core::skill::Difficulty>; 2],
    tickets: Option<[u32; 2]>,
    combatants: Option<u32>,
    team_pct: u32,
    ticket_pct: u32,
    /// Tickets from the mission (scaled by `ticket_pct`) instead of the map
    /// defaults: `--difficulty-check` plays the tiers' tuning setup.
    mission_tickets: bool,
    era_cw: bool,
    mission_dir: Option<std::path::PathBuf>,
    /// The proxy's team, profile and strength label.
    proxy: Option<(u8, core::ai::proxy::Profile, String)>,
}

impl Cfg {
    fn from_env() -> Cfg {
        let var = |k: &str| std::env::var(k).ok();
        let tier = |s: &str| core::skill::Difficulty::parse(s);
        let teams = var("SWEEP_TEAMS")
            .and_then(|v| {
                let (a, b) = v.split_once(':')?;
                Some([tier(a), tier(b)])
            })
            .unwrap_or([None, None]);
        Cfg {
            difficulty: var("SWEEP_DIFFICULTY")
                .and_then(|v| tier(&v))
                .unwrap_or_default(),
            teams,
            combatants: var("SWEEP_COMBATANTS")
                .map(|v| v.parse::<u32>().expect("SWEEP_COMBATANTS must be 1..64"))
                .inspect(|n| assert!((1..=64).contains(n))),
            team_pct: var("SWEEP_TEAM_PCT")
                .map(|v| v.parse::<u32>().expect("SWEEP_TEAM_PCT must be an integer"))
                .unwrap_or(100),
            ticket_pct: var("SWEEP_TICKET_PCT")
                .map(|v| {
                    v.parse::<u32>()
                        .expect("SWEEP_TICKET_PCT must be an integer")
                })
                .unwrap_or(100),
            mission_tickets: var("SWEEP_MISSION_TICKETS").as_deref() == Some("1"),
            tickets: var("SWEEP_TICKETS").map(|v| {
                let (a, b) = v.split_once(':').expect("SWEEP_TICKETS requires a:b");
                [
                    a.parse().expect("integer tickets"),
                    b.parse().expect("integer tickets"),
                ]
            }),
            era_cw: var("SWEEP_ERA").as_deref() == Some("cw"),
            mission_dir: var("SWEEP_MISSION_DIR").map(std::path::PathBuf::from),
            proxy: var("SWEEP_PROXY").and_then(|v| match parse_proxy(&v) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("SWEEP_PROXY={v}: {e}; no proxy");
                    None
                }
            }),
        }
    }
}

/// `<team>:<strength>[,key=value...]` (module header).
fn parse_proxy(v: &str) -> Result<(u8, core::ai::proxy::Profile, String), String> {
    use core::ai::proxy::hand::Hand;
    let (team, rest) = v.split_once(':').ok_or("want <team>:<strength>")?;
    let team: u8 = team
        .trim()
        .parse()
        .ok()
        .filter(|t| *t < 2)
        .ok_or("team 0 or 1")?;
    let mut items = rest.split(',');
    let name = items.next().unwrap_or("").trim();
    let strength = core::ai::proxy::Strength::parse(name).ok_or("strength average|good")?;
    let mut p = strength.profile();
    let mut label = strength.label().to_owned();
    for it in items {
        let (k, x) = it.split_once('=').ok_or(format!("{it}: want key=value"))?;
        let f: f32 = x
            .trim()
            .parse()
            .map_err(|_| format!("{it}: not a number"))?;
        match k.trim() {
            "skill" => p.skill = f as u8,
            "reaction" => p.reaction_ticks = f as u64,
            "view" => p.view_mult = f,
            "circles" => p.circles = f != 0.0,
            "strafe" => p.react[0] = f,
            "roll" => p.react[1] = f,
            "move" => p.react[2] = f,
            "grenade" => p.grenade = f,
            // [ours] The hand (`core::ai::proxy::hand`): on/off and its numbers.
            "hand" => p.hand = (f != 0.0).then_some(p.hand.unwrap_or(Hand::DEFAULT)),
            "hfloor" => p.hand.get_or_insert(Hand::DEFAULT).floor = f,
            "hacquire" => p.hand.get_or_insert(Hand::DEFAULT).acquire = f,
            "hsettle" => p.hand.get_or_insert(Hand::DEFAULT).settle = f,
            "hup" => p.hand.get_or_insert(Hand::DEFAULT).up = f,
            "head" => p.headshots = f != 0.0,
            "hunts" => p.hunts = f != 0.0,
            "mend" => p.mend = f,
            "picks" => p.picks = f != 0.0,
            "class" => p.class = (f >= 0.0).then_some(f as u8),
            _ => return Err(format!("{k}: unknown key")),
        }
        label.push(',');
        label.push_str(it.trim());
    }
    Ok((team, p, label))
}

thread_local! {
    /// The settings of the round this thread plays ([`Cfg`]).
    static CFG: std::cell::RefCell<Cfg> = std::cell::RefCell::new(Cfg::from_env());
}

fn cfg() -> Cfg {
    CFG.with(|c| c.borrow().clone())
}

/// The maps `--difficulty-check` runs without a list: the GCW conquest maps
/// card DPRX's balance run found nearest to even (bots only, Normal, both
/// faction orders).
const CHECK_MAPS: [&str; 3] = ["bf1_yav2", "tat1", "bf1_nab2"];

/// The mission the sweep plays on `key`: `missions/<key>_con.ron` (or the
/// `_con_cw` one under `SWEEP_ERA=cw`), from `SWEEP_MISSION_DIR` when set.
fn mission_path(key: &str) -> std::path::PathBuf {
    let c = cfg();
    let dir = c.mission_dir.unwrap_or_else(|| {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"))
    });
    let cw = dir.join(format!("{key}_con_cw.ron"));
    let con = dir.join(format!("{key}_con.ron"));
    if c.era_cw && cw.exists() {
        cw
    } else if con.exists() {
        con
    } else {
        // A space map has only its Space Assault.
        dir.join(format!("{key}_ass.ron"))
    }
}

/// The mode of [`mission_path`]'s file, by its name.
fn mission_mode(key: &str) -> &'static str {
    let path = mission_path(key);
    if path.to_string_lossy().ends_with("_ass.ron") {
        "ass"
    } else {
        "con"
    }
}

/// [`mission_path`]'s mission, when the generator wrote one for this map.
fn mission_for(key: &str, out: &mut String) -> Option<game::mission::MissionDef> {
    let path = mission_path(key);
    if !path.exists() {
        return None;
    }
    match game::mission::MissionDef::load(&path) {
        Ok(m) => Some(m),
        Err(e) => {
            outln!(out, "{key:10} mission unreadable: {e:#}");
            None
        }
    }
}

/// What one map contributes to the footer. `None` on a map that never ran (it
/// failed to load), so the scoreboard counts only maps that played a round.
#[derive(Clone, Copy)]
struct MapStats {
    finished: bool,
    stuck_max: u16,
}

/// One map's whole report: the text it prints, its stuck census (STDERR,
/// empty unless `STUCK_CENSUS` or `SWEEP_DEATHS` is set), and its scoreboard
/// row.
struct MapReport {
    text: String,
    census: String,
    stats: Option<MapStats>,
    /// Card DPRX: the round's result and the proxy's tally.
    round: Option<RoundResult>,
}

/// One round's result (card DPRX): the winner, the margin (team 1's
/// tickets left minus team 0's) and, with a proxy, its team and tally.
#[derive(Clone)]
struct RoundResult {
    winner: Option<u8>,
    margin: i64,
    proxy: Option<(u8, game::blackbox::session::Session)>,
}

impl RoundResult {
    /// The `round` line (module header).
    fn line(&self, key: &str, seed: u32, tickets: (u32, u32), label: &str) -> String {
        let mut s = format!(
            "round {key} s{seed} {} winner {} margin {} tickets {}/{}",
            sweep_difficulty().label(),
            self.winner.map_or("-".to_owned(), |w| w.to_string()),
            self.margin,
            tickets.0,
            tickets.1
        );
        if let Some((team, m)) = &self.proxy {
            let lives = m.lives.len().max(1) as f32;
            s.push_str(&format!(
                "  proxy {team} {label} kills {} deaths {} shots {} hits {} acc {:.3} life_s {:.1}",
                m.kills,
                m.deaths,
                m.shots,
                m.hits,
                m.hits as f32 / m.shots.max(1) as f32,
                m.alive_ticks as f32 / core::TICK_HZ as f32 / lives
            ));
        }
        s.push('\n');
        s
    }
}

/// Card WAI: `n (class piloted/alive …)` — the live walkers with a bot at
/// the pilot seat, and per walker class how many of its live ones.
fn walker_pilots(sim: &core::sim::Sim, classes: &[String]) -> String {
    use core::unit::Controller;
    use core::vehicle::{SeatKind, VehicleKind};
    let mut per: std::collections::BTreeMap<u32, (u32, u32)> = Default::default();
    for (_, v) in sim.alive_vehicles() {
        if v.kind != VehicleKind::Walker {
            continue;
        }
        let bot = v
            .seats
            .first()
            .filter(|s| s.kind == SeatKind::Pilot)
            .and_then(|s| s.occupant)
            .and_then(|o| sim.unit(o))
            .is_some_and(|u| u.controller == Controller::Ai);
        let e = per.entry(v.class).or_default();
        e.0 += u32::from(bot);
        e.1 += 1;
    }
    let total: u32 = per.values().map(|e| e.0).sum();
    let list: Vec<String> = per
        .iter()
        .map(|(c, (p, n))| {
            let name = classes.get(*c as usize).map_or("?", String::as_str);
            format!("{name} {p}/{n}")
        })
        .collect();
    format!("{total} ({})", list.join(", "))
}

/// `SWEEP_DIFFICULTY` (env): which tier the whole sweep runs at
/// (`easy|normal|hard|elite`). Anything unparseable (`legendary` too) falls
/// back to the default (Normal) and is reported in the header.
fn sweep_difficulty() -> core::skill::Difficulty {
    cfg().difficulty
}

/// `SWEEP_HUMAN_TEAM` (env, card DTEAM): `0` or `1` counts one stand-in human
/// on that team for the per-team bot rule (`Sim::extra_humans`; no unit is
/// changed, so only the DTEAM rows move). `1,1` or `0,1` count one per
/// entry. Unset: none.
fn sweep_human_team() -> [u8; 2] {
    let mut n = [0u8; 2];
    if let Ok(v) = std::env::var("SWEEP_HUMAN_TEAM") {
        for t in v.split(',').filter_map(|t| t.trim().parse::<usize>().ok()) {
            if let Some(c) = n.get_mut(t) {
                *c += 1;
            }
        }
    }
    n
}

/// Is slot 0 a BOT this run (the maintainer's decision 2, 2026-09-03)? `SWEEP_SLOT0`
/// (env): `player`/`human`/`0` puts the idle human back, anything else — and
/// UNSET — makes it a bot. The sweep's default is the bot; `arena` reads the
/// same variable and defaults the other way (its published table was
/// baselined with the human).
fn slot0_is_bot() -> bool {
    !matches!(
        std::env::var("SWEEP_SLOT0").as_deref(),
        Ok("player" | "human" | "0")
    )
}

/// `SLOT0_AT=x,y,z` (with `SWEEP_SLOT0=player`): where the idle human stands.
fn slot0_at() -> Option<glam::Vec3> {
    let v = std::env::var("SLOT0_AT").ok()?;
    let mut it = v.split(',').map(|s| s.trim().parse::<f32>().ok());
    Some(glam::Vec3::new(it.next()??, it.next()??, it.next()??))
}

/// The census threshold when none is given: the queue item's 5,000 ticks.
const STUCK_CENSUS_DEFAULT: u16 = 5_000;

/// `STUCK_CENSUS` (env): `None` when unset (no census, no tracking); else the
/// threshold in ticks — the value itself when it parses as a number ≥ 2, so
/// `STUCK_CENSUS=1` means "on, at the default".
fn stuck_threshold() -> Option<u16> {
    let v = std::env::var("STUCK_CENSUS").ok()?;
    Some(
        v.parse::<u16>()
            .ok()
            .filter(|&n| n >= 2)
            .unwrap_or(STUCK_CENSUS_DEFAULT),
    )
}

/// `HIT_CENSUS` (env, card REACT): what a bot on foot does while it is being
/// hit — the `fire` line under the census. A bot is UNDER FIRE for
/// [`FIRE_TICKS`] after a hit; a SPELL is the run of under-fire ticks it
/// spends within [`FIRE_M`] of where the run began.
#[derive(Default)]
struct Fire {
    /// Per unit index.
    at: Vec<FireUnit>,
    /// Under-fire ticks; those slower than [`FIRE_SLOW`]; those inside a
    /// spell of [`FIRE_SPELL`] or more.
    ticks: u64,
    slow: u64,
    stood: u64,
    /// The same three for a bot whose behaviour is Fight.
    fight: [u64; 3],
    /// Hits that open a burst (none in the [`FIRE_TICKS`] before), those
    /// whose bot lived another [`FIRE_TICKS`], and of those the ones found
    /// [`FIRE_M`] or more from where they were hit.
    hits: [u64; 3],
    /// The longest spell (ticks).
    longest: u64,
}

/// [`Fire`]'s row of a unit.
/// Card WHELD, under `ANIM_CENSUS`: unit-ticks on foot by who, the slot in
/// hand and the model drawn for it.
struct Hands {
    plan: game::soldier_vis::DrawPlan,
    ticks: std::collections::BTreeMap<String, u64>,
}

impl Hands {
    fn new(teams: &[game::walk::TeamTemplate; 2]) -> Self {
        Self {
            plan: game::soldier_vis::DrawPlan::of(teams),
            ticks: Default::default(),
        }
    }

    fn observe(&mut self, sim: &core::sim::Sim) {
        for u in sim.units.iter().filter(|u| u.alive && u.mount.is_none()) {
            let hand = game::soldier_vis::weapon_in_hand(u, &sim.weapon_classes);
            let team = usize::from(u.team.min(1));
            let class = usize::from(u.spec.class);
            let slot = if u.active_channel == 1 {
                "ch1"
            } else if hand.slot == u.spec.weapon_slot {
                "primary"
            } else {
                "other"
            };
            let model = if hand.hidden {
                "hidden"
            } else {
                self.plan
                    .weapon_in_slot(team, class, hand.slot)
                    .unwrap_or("-")
            };
            let who = if u.is_player { "player" } else { "bot" };
            *self
                .ticks
                .entry(format!("{who} {slot} {model}"))
                .or_default() += 1;
        }
    }

    fn report(&self, key: &str) -> String {
        self.ticks
            .iter()
            .map(|(k, n)| format!("{key:10} hands: {k} {n}\n"))
            .collect()
    }
}

#[derive(Clone, Copy, Default)]
struct FireUnit {
    gen: u32,
    /// The tick of the hit last seen (0 = none).
    hit: u64,
    /// Where the spell began, and at which tick.
    from: glam::Vec3,
    since: u64,
    /// Fight samples in the unqualified spell (excluding its starting tick).
    fight_pending: u64,
    /// The hit whose answer is still awaited: where the bot stood, the tick.
    asked: Option<(glam::Vec3, u64)>,
}

/// Under fire this long after a hit (ticks).
const FIRE_TICKS: u64 = 60;
/// A spell is broken this far from where it began; a hit is answered by a
/// move this long (m, XZ).
const FIRE_M: f32 = 1.5;
/// Standing: slower than this (m/s, XZ).
const FIRE_SLOW: f32 = 0.5;
/// A spell this long is "found standing" (ticks).
const FIRE_SPELL: u64 = 90;

impl Fire {
    fn observe(&mut self, sim: &core::sim::Sim) {
        let now = sim.tick;
        for (h, u) in sim.alive_units() {
            let i = h.index as usize;
            if self.at.len() <= i {
                self.at.resize(i + 1, FireUnit::default());
            }
            let e = &mut self.at[i];
            let p = u.soldier.pos;
            let foot = u.controller == core::unit::Controller::Ai && u.mount.is_none();
            if e.gen != h.gen {
                *e = FireUnit {
                    gen: h.gen,
                    from: p,
                    since: now,
                    ..FireUnit::default()
                };
            }
            let Some(hit) = u.last_hit.map(|(_, t)| t).filter(|_| foot) else {
                *e = FireUnit {
                    gen: h.gen,
                    hit: u.last_hit.map_or(0, |(_, t)| t),
                    from: p,
                    since: now,
                    ..FireUnit::default()
                };
                continue;
            };
            if hit != e.hit {
                // A new hit; the first of a burst is the one whose answer is read.
                if hit.saturating_sub(e.hit) > FIRE_TICKS || e.hit == 0 {
                    self.hits[0] += 1;
                    e.asked = Some((p, now));
                    (e.from, e.since, e.fight_pending) = (p, now, 0);
                }
                e.hit = hit;
            }
            if let Some((from, _)) = e.asked.filter(|(_, t)| now >= t + FIRE_TICKS) {
                self.hits[1] += 1;
                self.hits[2] += u64::from(core::nav::xz_dist(from, p) >= FIRE_M);
                e.asked = None;
            }
            if now.saturating_sub(hit) > FIRE_TICKS {
                continue;
            }
            if core::nav::xz_dist(e.from, p) >= FIRE_M {
                (e.from, e.since, e.fight_pending) = (p, now, 0);
            }
            let spell = now - e.since;
            self.longest = self.longest.max(spell);
            let v = u.soldier.vel;
            let row = [
                1,
                u64::from(glam::Vec2::new(v.x, v.z).length() < FIRE_SLOW),
                match spell {
                    FIRE_SPELL => FIRE_SPELL,
                    k if k > FIRE_SPELL => 1,
                    _ => 0,
                },
            ];
            self.ticks += row[0];
            self.slow += row[1];
            self.stood += row[2];
            let fight = u.ai.behaviour == core::ai::Behaviour::Fight;
            self.fight[0] += u64::from(fight);
            self.fight[1] += u64::from(fight) * row[1];
            if spell > 0 && spell <= FIRE_SPELL {
                e.fight_pending += u64::from(fight);
            }
            self.fight[2] += match spell {
                FIRE_SPELL => e.fight_pending,
                k if k > FIRE_SPELL => u64::from(fight),
                _ => 0,
            };
        }
    }

    fn line(&self, key: &str) -> String {
        let pct = |a: u64, b: u64| 100.0 * a as f64 / b.max(1) as f64;
        format!(
            "fire {key}: under-fire-s {:.0} | slow {:.1} % | spell3 {:.1} % | fight-s {:.0} slow {:.1} % spell3 {:.1} % | bursts {} lived {} moved {} ({:.1} %) | longest {:.1} s\n",
            self.ticks as f64 / 30.0,
            pct(self.slow, self.ticks),
            pct(self.stood, self.ticks),
            self.fight[0] as f64 / 30.0,
            pct(self.fight[1], self.fight[0]),
            pct(self.fight[2], self.fight[0]),
            self.hits[0],
            self.hits[1],
            self.hits[2],
            pct(self.hits[2], self.hits[1]),
            self.longest as f64 / 30.0,
        )
    }
}

/// `JET_CENSUS` (env, issue #72): what the bots do with their jets.
#[derive(Default)]
struct JetCensus {
    since_last: std::collections::HashMap<u32, u64>,
    was_flying: std::collections::HashMap<u32, u64>,
    open: std::collections::HashMap<u32, (u16, u16, glam::Vec3, u64)>,
    hop_gaps: Vec<u64>,
    hops: u64,
    attacks: u64,
    /// (from hub, to hub) -> started, arrived, died, gave up.
    links: std::collections::BTreeMap<(u16, u16), [u64; 4]>,
    env_deaths: std::collections::BTreeMap<String, u64>,
    units: std::collections::HashSet<u32>,
}

impl JetCensus {
    const GIVE_UP: u64 = 30 * core::TICK_HZ as u64;

    fn observe(&mut self, sim: &core::sim::Sim) {
        for (h, u) in sim.alive_units() {
            if u.is_player || u.controller != core::unit::Controller::Ai || !u.consts.has_jet() {
                continue;
            }
            self.units.insert(h.index);
            let Some(f) = u.ai.jet.flight else {
                self.was_flying.remove(&h.index);
                continue;
            };
            if self.was_flying.insert(h.index, f.since) == Some(f.since) {
                continue;
            }
            if f.route {
                let arc = u.ai.route.and_then(|r| {
                    let arc = sim.nav.as_ref()?.arcs.get(r.via_arc? as usize).copied()?;
                    let from = if arc.a == r.next_hub { arc.b } else { arc.a };
                    Some((from, r.next_hub))
                });
                if let Some((from, to)) = arc {
                    self.links.entry((from, to)).or_default()[0] += 1;
                    self.open.insert(h.index, (from, to, f.goal, sim.tick));
                }
            } else if f.height.is_some() {
                self.attacks += 1;
            } else {
                self.hops += 1;
                if let Some(last) = self.since_last.get(&h.index) {
                    self.hop_gaps.push(sim.tick - last);
                }
            }
            self.since_last.insert(h.index, sim.tick);
        }
        let mut done = Vec::new();
        for (&i, &(from, to, goal, since)) in &self.open {
            let u = &sim.units[i as usize];
            let outcome = if !u.alive {
                Some(2)
            } else if core::nav::xz_dist(u.soldier.pos, goal) < 3.0
                && u.soldier.pos.y >= goal.y - 0.5
            {
                Some(1)
            } else if sim.tick > since + Self::GIVE_UP {
                Some(3)
            } else {
                None
            };
            if let Some(o) = outcome {
                self.links.entry((from, to)).or_default()[o] += 1;
                done.push(i);
            }
        }
        for i in done {
            self.open.remove(&i);
        }
        for ev in &sim.events {
            if let core::sim::Event::EnvDeath { unit, cause } = ev {
                if self.units.contains(&unit.index) {
                    *self.env_deaths.entry(format!("{cause:?}")).or_default() += 1;
                }
            }
        }
    }

    fn line(&self, key: &str) -> String {
        let secs = |t: u64| t as f32 / core::TICK_HZ as f32;
        let gaps = if self.hop_gaps.is_empty() {
            "-".to_string()
        } else {
            let n = self.hop_gaps.len() as u64;
            format!(
                "{:.1}/{:.1}/{:.1}s",
                secs(*self.hop_gaps.iter().min().unwrap()),
                secs(self.hop_gaps.iter().sum::<u64>() / n),
                secs(*self.hop_gaps.iter().max().unwrap())
            )
        };
        let links = self
            .links
            .iter()
            .map(|((a, b), n)| format!("{a}->{b}={}/{}/{}/{}", n[0], n[1], n[2], n[3]))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "jets {key}: units {} hops {} gap min/mean/max {gaps} attacks {} links [started/arrived/died/gave up] {links} env {:?}\n",
            self.units.len(),
            self.hops,
            self.attacks,
            self.env_deaths
        )
    }
}

/// `DROID_CENSUS` (env, card DROIDAI): what the bots do with their remote
/// droids. Per map, one `droids <key>:` line:
/// - `carrier-min`: minutes of bots alive on foot holding a remote weapon;
/// - `launches` and `/carrier-min`: bot droid throws;
/// - `exposed`: launches made with an enemy within 20 m of the body, or one
///   within 60 m with a line of sight to it (card DROIDAI's own test, measured
///   the same on every branch);
/// - `flying`: the share of carrier time the bot spent flying its droid (its
///   body standing), and the mean flight;
/// - `hit-flying`: body hits taken while flying; `died-flying`: carriers
///   killed while flying;
/// - `ends`: how the bots' droids ended (destroyed / expired / destruct /
///   owner died).
#[derive(Default)]
struct DroidCensus {
    carrier: u64,
    flying: u64,
    flights: u64,
    launches: u64,
    exposed: u64,
    hit_flying: u64,
    died_flying: u64,
    ends: [u64; 4],
    was_flying: std::collections::HashSet<(u32, u32)>,
}

impl DroidCensus {
    fn exposed(sim: &core::sim::Sim, u: &core::unit::Unit) -> bool {
        let body = u.capsule().center();
        sim.alive_units().any(|(_, e)| {
            e.team != u.team && {
                let d2 = e.soldier.pos.distance_squared(u.soldier.pos);
                d2 <= 20.0 * 20.0
                    || (d2 <= 60.0 * 60.0 && core::perception::los(sim, e.muzzle(), body))
            }
        })
    }

    fn observe(&mut self, sim: &core::sim::Sim) {
        use core::sim::Event;
        let bot = |h: core::unit::UnitHandle| {
            sim.unit(h)
                .filter(|u| !u.is_player && u.controller == core::unit::Controller::Ai)
        };
        let mut now = std::collections::HashSet::new();
        for (h, u) in sim.alive_units() {
            if u.is_player || u.controller != core::unit::Controller::Ai || u.mount.is_some() {
                continue;
            }
            let carries = u
                .held
                .iter()
                .any(|w| sim.weapon_classes[w.class].remote.is_some());
            if !carries {
                continue;
            }
            self.carrier += 1;
            if sim.controlled_droid(h).is_some() {
                self.flying += 1;
                now.insert((h.index, h.gen));
                if !self.was_flying.contains(&(h.index, h.gen)) {
                    self.flights += 1;
                }
            }
        }
        for ev in &sim.events {
            match ev {
                Event::Throw {
                    unit, weapon_class, ..
                } if sim.weapon_classes[*weapon_class as usize].remote.is_some() => {
                    if let Some(u) = bot(*unit) {
                        self.launches += 1;
                        self.exposed += u64::from(Self::exposed(sim, u));
                    }
                }
                Event::Hit { victim, .. }
                    if self.was_flying.contains(&(victim.index, victim.gen)) =>
                {
                    self.hit_flying += 1;
                }
                Event::Kill { victim, .. }
                    if self.was_flying.contains(&(victim.index, victim.gen)) =>
                {
                    self.died_flying += 1;
                }
                Event::DroidGone { end, .. } => {
                    self.ends[*end as usize] += 1;
                }
                _ => {}
            }
        }
        self.was_flying = now;
    }

    fn line(&self, key: &str) -> String {
        let min = self.carrier as f64 / 1800.0;
        let per = |n: u64| if min > 0.0 { n as f64 / min } else { 0.0 };
        let pct = |a: u64, b: u64| {
            if b > 0 {
                100.0 * a as f64 / b as f64
            } else {
                0.0
            }
        };
        format!(
            "droids {key}: carrier-min {min:.1} | launches {} ({:.2}/carrier-min) | exposed {} ({:.0} %) | flying {:.1} % of carrier time, {} flights, mean {:.1} s | hit-flying {} died-flying {} | ends destroyed {} expired {} destruct {} owner-died {}\n",
            self.launches,
            per(self.launches),
            self.exposed,
            pct(self.exposed, self.launches),
            pct(self.flying, self.carrier),
            self.flights,
            if self.flights > 0 { self.flying as f64 / 30.0 / self.flights as f64 } else { 0.0 },
            self.hit_flying,
            self.died_flying,
            self.ends[0],
            self.ends[1],
            self.ends[2],
            self.ends[3],
        )
    }
}

/// `SWEEP_SHOTS` (env, card STTAP): trigger pulls and unit-lives per weapon
/// class (module header).
#[derive(Default)]
struct Shots {
    /// Per weapon class: TriggerSingle, pulls, lives.
    by: std::collections::BTreeMap<String, (bool, u64, u64)>,
    /// Card GRAI: grenades the bots threw, per weapon class.
    thrown: std::collections::BTreeMap<String, u64>,
    /// The tick each unit last fired on: a multi-bolt salvo is one pull.
    last: Vec<u64>,
    /// Standoffs running: two enemy bots within 25 m, each seeing the other
    /// (sampled once a second; 5 s out of sight ends one), neither dead —
    /// `(index, gen)` twice → the first and the last tick seen so.
    pairs: std::collections::HashMap<[u32; 4], (u64, u64)>,
    /// The longest standoff: ticks, the two units, its first tick.
    longest: (u64, u32, u32, u64),
    /// Standoffs that reached 3,000 ticks.
    long: std::collections::HashSet<[u32; 4]>,
}

impl Shots {
    fn observe(&mut self, sim: &core::sim::Sim) {
        for ev in &sim.events {
            if let core::sim::Event::Throw {
                unit, weapon_class, ..
            } = ev
            {
                if sim.unit(*unit).is_some_and(|u| !u.is_player) {
                    let name = &sim.weapon_classes[*weapon_class as usize].name;
                    *self.thrown.entry(name.clone()).or_default() += 1;
                }
            }
            let core::sim::Event::Shot { unit, .. } = ev else {
                continue;
            };
            let i = unit.index as usize;
            let Some(u) = sim.units.get(i) else {
                continue;
            };
            if self.last.len() <= i {
                self.last.resize(i + 1, 0);
            }
            if std::mem::replace(&mut self.last[i], sim.tick) == sim.tick {
                continue;
            }
            let wc = &sim.weapon_classes[u.weapon_class];
            let e = self.by.entry(wc.name.clone()).or_default();
            e.0 = wc.trigger_single;
            e.1 += 1;
        }
        if !sim.tick.is_multiple_of(30) {
            return;
        }
        let sees = |a: &core::unit::Unit, b: core::unit::UnitHandle| {
            core::perception::contact_of(&a.ai, b).is_some_and(|c| c.seen_now)
        };
        let units: Vec<_> = sim.alive_units().collect();
        for (i, (ha, a)) in units.iter().enumerate() {
            for (hb, b) in &units[i + 1..] {
                if a.team == b.team
                    || a.soldier.pos.distance(b.soldier.pos) > 25.0
                    || !sees(a, *hb)
                    || !sees(b, *ha)
                {
                    continue;
                }
                let key = [ha.index, ha.gen, hb.index, hb.gen];
                let e = self.pairs.entry(key).or_insert((sim.tick, sim.tick));
                e.1 = sim.tick;
                let run = e.1 - e.0;
                if run > self.longest.0 {
                    self.longest = (run, ha.index, hb.index, e.0);
                }
                if run >= 3000 {
                    self.long.insert(key);
                }
            }
        }
        let alive = |i: u32, gen: u32| {
            sim.units
                .get(i as usize)
                .is_some_and(|u| u.gen == gen && u.health.cur > 0.0)
        };
        self.pairs
            .retain(|k, e| sim.tick - e.1 <= 150 && alive(k[0], k[1]) && alive(k[2], k[3]));
    }

    fn line(&mut self, key: &str, sim: &core::sim::Sim) -> String {
        for u in &sim.units {
            let wc = &sim.weapon_classes[u.weapon_class];
            let e = self.by.entry(wc.name.clone()).or_default();
            e.0 = wc.trigger_single;
            e.2 += 1 + u64::from(u.deaths);
        }
        let mut out = format!("shots {key}:");
        for (name, (single, pulls, lives)) in &self.by {
            let tag = if *single { "[single]" } else { "" };
            let _ = write!(out, " {name}{tag}:{pulls}/{lives}");
        }
        let _ = write!(
            out,
            "\ngrenades {key}: {} thrown by bots",
            self.thrown.values().sum::<u64>()
        );
        for (name, n) in &self.thrown {
            let _ = write!(out, " {name}:{n}");
        }
        let (run, a, b, from) = self.longest;
        let _ = writeln!(
            out,
            "\nstandoff {key}: longest {run} ticks (units {a} and {b} from tick {from}); {} of 3000 ticks or more",
            self.long.len()
        );
        out
    }
}

/// `SNIPE_CENSUS` (env): what the AI snipers do (module header).
#[derive(Default)]
struct SnipeCensus {
    /// AI unit-ticks on foot with a scoped rifle in hand; of those on a
    /// sniper's spot.
    rifle: u64,
    perch: u64,
    /// The distance of every scoped-rifle shot at a unit target, m.
    shots: Vec<f32>,
    /// Samples (every 15 ticks, a scoped bot on foot): an enemy soldier
    /// 60-128 m off, in the view cone, with a clear line (could be shot);
    /// of those, one the bot holds as seen.
    open: u64,
    open_seen: u64,
}

impl SnipeCensus {
    fn observe(&mut self, sim: &core::sim::Sim) {
        let scoped = |c: usize| sim.weapon_classes[c].ai_distance_table();
        for ev in &sim.events {
            if let core::sim::Event::Shot {
                unit, weapon_class, ..
            } = ev
            {
                let Some(u) = sim
                    .unit(*unit)
                    .filter(|u| !u.is_player && u.mount.is_none())
                else {
                    continue;
                };
                if !scoped(*weapon_class as usize) {
                    continue;
                }
                if let Some(t) = u.ai.target.and_then(|t| t.live_unit(sim)) {
                    self.shots
                        .push((t.capsule().center() - u.muzzle()).length());
                }
            }
        }
        let sample = sim.tick.is_multiple_of(15);
        for (_, u) in sim.alive_units() {
            if u.is_player || u.mount.is_some() || !scoped(u.weapon_class) {
                continue;
            }
            self.rifle += 1;
            if u.ai.snipe.cur.is_some_and(|c| c.arrived) {
                self.perch += 1;
            }
            if !sample {
                continue;
            }
            let eye = u.muzzle();
            let view = u.aim_frame().0;
            let mut open = false;
            let mut seen = false;
            for (h, o) in sim.alive_units() {
                if o.team == u.team || o.mount.is_some() {
                    continue;
                }
                let p = o.capsule().center();
                let off = p - eye;
                let d2 = off.dot(off);
                if !(3600.0..16384.0).contains(&d2)
                    || !core::perception::in_cone(view, off, d2, 0.0)
                    || !core::perception::los(sim, eye, p)
                {
                    continue;
                }
                open = true;
                seen |= core::perception::contact_of(&u.ai, h).is_some_and(|c| c.seen_now);
            }
            self.open += u64::from(open);
            self.open_seen += u64::from(open && seen);
        }
    }

    fn line(&mut self, key: &str) -> String {
        let hz = core::TICK_HZ as f32;
        self.shots.sort_by(f32::total_cmp);
        let n = self.shots.len();
        let q = |f: f32| {
            if n == 0 {
                0.0
            } else {
                self.shots[((n - 1) as f32 * f) as usize]
            }
        };
        let mut bins = [0u32; 6];
        for d in &self.shots {
            bins[((d / 20.0) as usize).min(5)] += 1;
        }
        format!(
            "snipe {key}: rifle_s {:.0} perch_s {:.0} | shots {n} median {:.1} p90 {:.1} max {:.1} | by 20 m {:?} | open60-128 {} seen {}\n",
            self.rifle as f32 / hz,
            self.perch as f32 / hz,
            q(0.5),
            q(0.9),
            q(1.0),
            bins,
            self.open,
            self.open_seen,
        )
    }
}

/// `SWEEP_DEATHS` (env): count deaths by cause (module header) when set.
/// A unit that appears within this far (m, XZ) of a live command walker of
/// its team spawned at it (the class points are 6-9 m from the origin).
const SPAWN_AT_WALKER: f32 = 10.0;

/// Where units appeared (`SPAWN_CENSUS`, module header): per team, per
/// source, the counts of the opening wave, the first 60 s' respawns and the
/// round's respawns.
#[derive(Default)]
struct Spawns(
    [std::collections::BTreeMap<String, [u32; 3]>; 2],
    /// `spawnrule`: per team, per node count of the post taken, the spawns
    /// seen, expected of an even draw over the eligible posts and expected
    /// of a draw over the flat node list.
    [std::collections::BTreeMap<usize, [f64; 3]>; 2],
);

impl Spawns {
    /// The source of a unit that has just appeared at `pos`.
    fn source(sim: &core::sim::Sim, classes: &[String], team: u8, pos: glam::Vec3) -> String {
        let xz = |a: glam::Vec3| glam::Vec2::new(a.x - pos.x, a.z - pos.z).length();
        let walker = sim
            .command_walkers(team)
            .map(|(h, v)| (xz(v.pos), h.index, v.class))
            .filter(|w| w.0 <= SPAWN_AT_WALKER)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, slot, class)) = walker {
            let name = classes.get(class as usize).map_or("?", String::as_str);
            return format!("{name}#{slot}");
        }
        sim.cps
            .iter()
            .filter_map(|c| {
                let d = c
                    .def
                    .spawn_nodes
                    .iter()
                    .map(|n| xz(n.0))
                    .min_by(f32::total_cmp)?;
                Some((d, c.def.name.as_str()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or_else(|| "?".to_owned(), |(_, n)| n.to_owned())
    }

    fn observe(&mut self, sim: &core::sim::Sim, classes: &[String], tick: u64) {
        let mut add = |u: &core::unit::Unit, cols: &[usize]| {
            let src = Self::source(sim, classes, u.team, u.soldier.pos);
            let row = self.0[usize::from(u.team.min(1))].entry(src).or_default();
            for &c in cols {
                row[c] += 1;
            }
        };
        if tick == 1 {
            for u in sim.units.iter().filter(|u| u.alive) {
                add(u, &[0]);
            }
        }
        for ev in &sim.events {
            if let core::sim::Event::Respawn { unit } = ev {
                if let Some(u) = sim.unit(*unit) {
                    add(u, if tick <= 1800 { &[1, 2] } else { &[2] });
                }
            }
        }
        for ev in &sim.events {
            let core::sim::Event::Respawn { unit } = ev else {
                continue;
            };
            let Some(u) = sim.unit(*unit).filter(|u| !u.is_player) else {
                continue;
            };
            // The posts the team may spawn at, by name, with their nodes.
            let mut posts: Vec<(String, usize)> = sim
                .cps
                .iter()
                .filter(|c| c.spawnable_by(u.team))
                .map(|c| (c.def.name.clone(), c.def.spawn_nodes.len()))
                .collect();
            let walkers: Vec<String> = sim
                .command_walkers(u.team)
                .map(|(h, v)| {
                    let name = classes.get(v.class as usize).map_or("?", String::as_str);
                    format!("{name}#{}", h.index)
                })
                .collect();
            let per_walker = sim.walker_spawn_points(u.team).len() / walkers.len().max(1);
            posts.extend(walkers.into_iter().map(|w| (w, per_walker)));
            let src = Self::source(sim, classes, u.team, u.soldier.pos);
            let Some(taken) = posts.iter().find(|p| p.0 == src).map(|p| p.1) else {
                continue;
            };
            if posts.len() < 2 {
                continue;
            }
            let rows = &mut self.1[usize::from(u.team.min(1))];
            rows.entry(taken).or_default()[0] += 1.0;
            let nodes: usize = posts.iter().map(|p| p.1).sum();
            for (_, n) in &posts {
                let row = rows.entry(*n).or_default();
                row[1] += 1.0 / posts.len() as f64;
                row[2] += *n as f64 / nodes as f64;
            }
        }
    }

    fn line(&self, key: &str) -> String {
        let team = |t: usize| {
            let col = |c: usize| {
                let rows: Vec<String> = self.0[t]
                    .iter()
                    .filter(|(_, n)| n[c] > 0)
                    .map(|(k, n)| format!("{k} {}", n[c]))
                    .collect();
                let total: u32 = self.0[t].values().map(|n| n[c]).sum();
                format!("{total} [{}]", rows.join(", "))
            };
            format!(
                "team{t} opening {} to60s {} round {}",
                col(0),
                col(1),
                col(2)
            )
        };
        let rule = |t: usize| {
            let rows: Vec<String> = self.1[t]
                .iter()
                .map(|(n, r)| format!("n{n} {} / post {:.1} / node {:.1}", r[0], r[1], r[2]))
                .collect();
            let total: f64 = self.1[t].values().map(|r| r[0]).sum();
            format!("team{t} {total} [{}]", rows.join(", "))
        };
        format!(
            "spawns {key}: {} | {}\nspawnrule {key}: {} | {}\n",
            team(0),
            team(1),
            rule(0),
            rule(1)
        )
    }
}

fn deaths_on() -> bool {
    std::env::var_os("SWEEP_DEATHS").is_some()
}

/// Deaths per team (the victim's; a team above 1 counts as 1) by cause:
/// kill, death region, fall, destroyed vehicle, attackerless blast, the kill
/// floor, a landing under the map, a lost bot's recycle (card KFLR), a
/// stalled bot's (card RCYC), confirmed Respawn, hero time exhaustion.
#[derive(Default)]
struct Deaths([[u32; 11]; 2], Option<Clamp>, Vec<bool>);

/// Card FLNK: the hubs' visit test runs every this many ticks (4 Hz at
/// 30 ticks a second, about the PS2 samples' rate).
const HUB_VISIT_EVERY: u64 = 8;
/// ...and a hub is visited by AI feet within max(its radius, this) of its
/// centre, XZ (SAIR's `hubuse.py` rule).
const HUB_VISIT_MIN_R: f32 = 3.0;

/// The endgame (card STEND): the first tick a team stood CLAMPED — no
/// reinforcement beyond its living units — at [`CLAMP_ALIVE`] alive or
/// fewer, and how its units died from then on.
struct Clamp {
    team: usize,
    tick: u64,
    alive: [u32; 2],
    /// Its deaths since: shot or blasted by someone, stalled, anything else.
    deaths: [u32; 3],
}

const CLAMP_ALIVE: u32 = 3;

impl Deaths {
    fn observe(&mut self, sim: &core::sim::Sim) {
        use core::sim::{DeathCause, Event};
        // Card FLNK: which hubs an AI soldier on foot has stood at.
        if let Some(g) = sim
            .nav
            .as_ref()
            .filter(|_| sim.tick.is_multiple_of(HUB_VISIT_EVERY))
        {
            self.2.resize(g.hubs.len(), false);
            for u in &sim.units {
                if !u.alive || u.mount.is_some() || u.controller != core::unit::Controller::Ai {
                    continue;
                }
                for (h, seen) in g.hubs.iter().zip(self.2.iter_mut()) {
                    *seen |=
                        core::nav::xz_dist(h.pos, u.soldier.pos) < h.radius.max(HUB_VISIT_MIN_R);
                }
            }
        }
        if self.1.is_none() && sim.outcome.is_none() {
            let low = |t: usize| sim.battle.teams[t].tickets() <= CLAMP_ALIVE;
            if low(0) || low(1) {
                let mut alive = [0u32; 2];
                for (_, u) in sim.alive_units() {
                    alive[usize::from(u.team.min(1))] += 1;
                }
                self.1 = (0..2)
                    .find(|&t| low(t) && alive[t] > 0 && sim.battle.teams[t].tickets() <= alive[t])
                    .map(|team| Clamp {
                        team,
                        tick: sim.tick,
                        alive,
                        deaths: [0; 3],
                    });
            }
        }
        for ev in &sim.events {
            let (unit, k) = match ev {
                Event::Kill { victim, .. } => (victim, 0),
                Event::EnvDeath { unit, cause } => (
                    unit,
                    match cause {
                        DeathCause::Suicide => 9,
                        DeathCause::HeroExhausted => 10,
                        DeathCause::DeathRegion | DeathCause::Water | DeathCause::DamageRegion => 1,
                        DeathCause::Fall => 2,
                        DeathCause::VehicleDestroyed => 3,
                        DeathCause::Blast => 4,
                        DeathCause::KillFloor => 5,
                        DeathCause::UnderMap => 6,
                        DeathCause::Lost => 7,
                        DeathCause::Stalled => 8,
                    },
                ),
                _ => continue,
            };
            let team = sim
                .units
                .get(unit.index as usize)
                .map_or(0, |u| usize::from(u.team.min(1)));
            self.0[team][k] += 1;
            if let Some(c) = self.1.as_mut().filter(|c| c.team == team) {
                c.deaths[match k {
                    0 => 0,
                    8 => 1,
                    _ => 2,
                }] += 1;
            }
            // `SWEEP_DEATHS=2`: where each of the three KFLR causes took a unit.
            if (5..8).contains(&k) && std::env::var("SWEEP_DEATHS").is_ok_and(|v| v == "2") {
                if let Some(u) = sim.units.get(unit.index as usize) {
                    let p = u.soldier.pos;
                    eprintln!(
                        "  world death tick {} unit {} cause {} at ({:.1},{:.1},{:.1})",
                        sim.tick,
                        unit.index,
                        ["floor", "under", "lost"][k - 5],
                        p.x,
                        p.y,
                        p.z
                    );
                }
            }
        }
    }

    fn line(&self, key: &str, sim: &core::sim::Sim) -> String {
        let t = |d: &[u32; 11]| {
            format!(
                "kill {} region {} fall {} vehicle {} blast {} floor {} under {} lost {} stalled {} suicide {} hero_exhausted {}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8], d[9], d[10]
            )
        };
        let mut text = format!(
            "deaths {key}: team0 {} | team1 {}\n",
            t(&self.0[0]),
            t(&self.0[1])
        );
        // Card FFLN: who the fire landed on (the sim's own counters).
        let n = &sim.nav_stats;
        let _ = writeln!(
            text,
            "teamfire {key}: kills {} team {} self {} bolt_hits {} friend_hits {} line_held {}",
            n.unit_kills, n.team_kills, n.self_kills, n.bolt_hits, n.friend_hits, n.line_held
        );
        // Card TEAM: the team board's roles.
        let hz = core::TICK_HZ as f32;
        let _ = writeln!(
            text,
            "roles {key}: cover_given {} cover_s {:.0} overwatch_s {:.0}",
            n.cover_given,
            n.cover_ticks as f32 / hz,
            n.overwatch_ticks as f32 / hz
        );
        // Card FLNK: the flank table, this match's shares, the roles given
        // and the soldier hubs (on a soldier arc) no AI soldier stood at.
        if let Some(g) = sim.nav.as_ref() {
            let soldier: Vec<bool> = (0..g.hubs.len())
                .map(|h| {
                    g.arcs
                        .iter()
                        .any(|a| a.type_mask & 1 != 0 && a.touches(h as u16))
                })
                .collect();
            let total = soldier.iter().filter(|&&s| s).count();
            let never = soldier
                .iter()
                .enumerate()
                .filter(|&(h, &s)| s && !self.2.get(h).copied().unwrap_or(false))
                .count();
            let shares: Vec<String> = sim
                .flank
                .routes
                .iter()
                .map(|r| format!("{}>{}:{:.2}", r.from, r.to, r.share))
                .collect();
            let _ = writeln!(
                text,
                "flank {key}: pairs {} routes {} given {} flank_s {:.0} ends {}/{}/{}/{} stalled {} legs {} hubs_never {} of {} ({:.1} %) shares {}",
                sim.flank.pairs,
                sim.flank.routes.len(),
                n.flank_given,
                n.flank_ticks as f32 / hz,
                n.flank_ends[0],
                n.flank_ends[1],
                n.flank_ends[2],
                n.flank_ends[3],
                n.flank_stalled,
                n.flank_legs,
                never,
                total,
                100.0 * never as f32 / total.max(1) as f32,
                shares.join(",")
            );
        }
        if let Some(c) = &self.1 {
            let _ = writeln!(
                text,
                "endgame {key}: team {} clamped at tick {} alive {}/{} | {} {} ticks later | its deaths since: kill {} stalled {} other {}",
                c.team,
                c.tick,
                c.alive[0],
                c.alive[1],
                if sim.outcome.is_some() { "ended" } else { "open" },
                sim.tick - c.tick,
                c.deaths[0],
                c.deaths[1],
                c.deaths[2],
            );
        }
        text
    }
}

// Where the planning graph says a point is: shared with the black box.
use game::unit_probe::locus_of;

/// The recycle list (module header, card RCYC): one line per bot the last
/// resort gave up, built from the tick's events. Read-only.
#[derive(Default)]
struct Recycles(String);

impl Recycles {
    fn observe(&mut self, key: &str, seed: u32, sim: &core::sim::Sim) {
        use core::sim::{DeathCause, Event};
        for ev in &sim.events {
            // Card UNSN: a stalled bot moved on unseen in place of its
            // recycle: the same defect, listed with both spots and the hub.
            if let Event::Relocated { unit, from, to } = ev {
                let (tag, hub, _) = locus_of(sim, *to);
                outln!(
                    self.0,
                    "relocate {key}: seed {seed} tick {} unit {} team {} from ({:.1},{:.1},{:.1}) to ({:.1},{:.1},{:.1}) locus {tag}{hub} {:.1} m",
                    sim.tick,
                    unit.index,
                    sim.units.get(unit.index as usize).map_or(0, |u| u.team),
                    from.x,
                    from.y,
                    from.z,
                    to.x,
                    to.y,
                    to.z,
                    (*to - *from).length(),
                );
                continue;
            }
            let Event::EnvDeath {
                unit,
                cause: DeathCause::Stalled,
            } = ev
            else {
                continue;
            };
            // The dead unit keeps its last position and AI state until its
            // slot respawns.
            let Some(u) = sim.units.get(unit.index as usize) else {
                continue;
            };
            let p = u.soldier.pos;
            let (tag, hub, dy) = locus_of(sim, p);
            let leg = u.ai.route.map_or_else(
                || "none".to_owned(),
                |r| {
                    let from = r
                        .via_arc
                        .and_then(|a| sim.nav.as_ref().and_then(|g| g.arcs.get(usize::from(a))))
                        .map(|a| if a.b == r.next_hub { a.a } else { a.b });
                    format!(
                        "{}->{} dest {} [{}]",
                        from.map_or_else(|| "last".to_owned(), |f| f.to_string()),
                        r.next_hub,
                        r.dest_hub,
                        r.source.tag()
                    )
                },
            );
            let objective = sim.commander[usize::from(u.team).min(1)]
                .of(unit.index)
                .map_or_else(|| "none".to_owned(), |o| o.label());
            // XZ metres to the leg's waypoint: a bot recycled a step from
            // its point was crowded off it.
            let wp = u.ai.route.map_or_else(
                || "-".to_owned(),
                |r| format!("{:.1}", core::nav::xz_dist(p, r.waypoint)),
            );
            outln!(
                self.0,
                "recycle {key}: seed {seed} tick {} unit {} gen {} team {} class {} pos ({:.1},{:.1},{:.1}) leg {leg} wp {wp} locus {tag}{hub} floor_dy {dy:.1} steer {} objective {objective}",
                sim.tick,
                unit.index,
                unit.gen,
                u.team,
                u.spec.class,
                p.x,
                p.y,
                p.z,
                u.ai.steer.label(),
            );
            // Card STLVL: the same recycle against the floors the ROUTING
            // uses — the leg hub's resolved floor (`Sim::nav_floor`, `-` when
            // unresolved), the waypoint's height and the level test.
            if let Some(r) = u.ai.route {
                let nf = sim.nav_floor(r.next_hub);
                let (hub_d, hub_r) = sim
                    .nav
                    .as_ref()
                    .and_then(|g| {
                        g.hub(r.next_hub)
                            .map(|h| (core::nav::xz_dist(p, h.pos), h.radius))
                    })
                    .unwrap_or((f32::NAN, f32::NAN));
                outln!(
                    self.0,
                    "recycle_lv {key}: seed {seed} tick {} unit {} hub {} hub_floor {} feet_dy {} hub_d {hub_d:.1} hub_r {hub_r:.1} wp_y {:.1} wp_dy {:.1} last {} level {:?}",
                    sim.tick,
                    unit.index,
                    r.next_hub,
                    nf.map_or_else(|| "-".to_owned(), |f| format!("{f:.1}")),
                    nf.map_or_else(|| "-".to_owned(), |f| format!("{:.1}", p.y - f)),
                    r.waypoint.y,
                    p.y - r.waypoint.y,
                    u8::from(r.is_last()),
                    sim.level_at(p),
                );
            }
        }
    }
}

/// Card TBRD's census (`BOARD_CENSUS=1`): a `BOARD` line on stderr for every
/// AI that takes a seat - the vehicle's kind and class, and from the unit's
/// last tick on foot the XZ metres to the vehicle's origin, the 3-D metres
/// from its body's centre to the origin, the vehicle's sphere radius, the
/// soldier's sphere radius and the gap left between the two spheres (the
/// PS2's board agent enters at a gap under 1.5 m, BF1 0.5 m); a `MISS` line
/// for every turret claim that ended with the unit on foot (the least gap
/// its walk came to, the XZ metres and height there, the claim's seconds);
/// and at the start a `TURRET` line per building turret (its building's
/// bodies, their farthest XZ corner and height span about its origin).
/// Read-only.
/// [`BoardCensus`]'s unit on its last tick: (generation, mounted, feet,
/// body centre, body radius).
type OnFoot = (u32, bool, glam::Vec3, glam::Vec3, f32);
/// [`BoardCensus`]'s open turret claim: (generation, turret, the least gap,
/// the XZ metres and height over the origin there, the first tick).
type TurretClaim = (u32, core::vehicle::VehicleHandle, f32, f32, f32, u64);

#[derive(Default)]
struct BoardCensus {
    /// Per unit index, its last tick.
    last: Vec<Option<OnFoot>>,
    /// Per unit index, its open claim on a turret.
    claim: Vec<Option<TurretClaim>>,
}

impl BoardCensus {
    fn observe(&mut self, sim: &core::sim::Sim, map: &str, classes: &[String]) {
        if self.last.is_empty() {
            for v in sim.vehicles.iter().filter(|v| v.instance.is_some()) {
                let bs: Vec<_> = sim
                    .collision
                    .bodies()
                    .iter()
                    .filter(|b| b.instance == v.instance)
                    .collect();
                let mut far = 0.0f32;
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for b in &bs {
                    for x in [b.aabb.min.x, b.aabb.max.x] {
                        for z in [b.aabb.min.z, b.aabb.max.z] {
                            far = far.max(glam::Vec2::new(x - v.pos.x, z - v.pos.z).length());
                        }
                    }
                    lo = lo.min(b.aabb.min.y - v.pos.y);
                    hi = hi.max(b.aabb.max.y - v.pos.y);
                }
                eprintln!(
                    "TURRET {map} {} bodies {} farxz {far:.2} y {lo:.2}..{hi:.2}",
                    classes.get(v.class as usize).map_or("?", String::as_str),
                    bs.len()
                );
            }
        }
        if self.last.len() < sim.units.len() {
            self.last.resize(sim.units.len(), None);
        }
        for (i, u) in sim.units.iter().enumerate() {
            if !u.alive || u.controller != core::unit::Controller::Ai {
                self.last[i] = None;
                continue;
            }
            let prev = self.last[i].filter(|p| p.0 == u.gen && !p.1);
            if let (Some(m), Some((_, _, feet, centre, rs))) = (u.mount, prev) {
                if let Some(v) = sim.vehicle(m.vehicle) {
                    let r = sim
                        .seat_setup
                        .get(v.class as usize)
                        .and_then(|s| s.as_ref())
                        .map_or(0.0, |s| s.radius);
                    let d3 = centre.distance(v.pos);
                    let (bc, bw) = sim
                        .blast_setup
                        .get(v.class as usize)
                        .and_then(|s| s.as_ref())
                        .map_or((glam::Vec3::ZERO, 0.0), |b| {
                            (b.sphere_centre, b.sphere_radius)
                        });
                    eprintln!(
                        "BOARD {map} {:?} {} seat {} bw {bw:.2} bc {:.2} xz {:.2} d3 {:.2} r {:.2} rs {:.2} gap {:.2}",
                        v.kind,
                        classes.get(v.class as usize).map_or("?", String::as_str),
                        m.seat,
                        bc.length(),
                        core::nav::xz_dist(feet, v.pos),
                        d3,
                        r,
                        rs,
                        d3 - r - rs
                    );
                }
            }
            let cap = u.capsule();
            // A `MISS` line for a turret claim that ended with the unit on
            // foot: the nearest its walk came.
            if self.claim.len() < sim.units.len() {
                self.claim.resize(sim.units.len(), None);
            }
            let now = (u.mount.is_none())
                .then(|| u.ai.ride.and_then(|r| r.board))
                .flatten()
                .filter(|&vh| core::ai::man::is_turret(sim, vh));
            if let Some(c) = self.claim[i].filter(|c| Some(c.1) != now || c.0 != u.gen) {
                if u.mount.is_none() && c.0 == u.gen {
                    let cls = sim
                        .vehicle(c.1)
                        .and_then(|v| classes.get(v.class as usize))
                        .map_or("?", String::as_str);
                    eprintln!(
                        "MISS {map} {cls} gap {:.2} xz {:.2} dy {:.2} secs {:.1}",
                        c.2,
                        c.3,
                        c.4,
                        (sim.tick - c.5) as f32 / core::TICK_HZ as f32
                    );
                }
                self.claim[i] = None;
            }
            if let Some(vh) = now {
                if let Some(v) = sim.vehicle(vh) {
                    let r = sim
                        .seat_setup
                        .get(v.class as usize)
                        .and_then(|s| s.as_ref())
                        .map_or(0.0, |s| s.radius);
                    let gap = cap.center().distance(v.pos) - r - cap.radius.max(cap.height * 0.5);
                    let c = self.claim[i].get_or_insert((u.gen, vh, f32::MAX, 0.0, 0.0, sim.tick));
                    if gap < c.2 {
                        *c = (
                            u.gen,
                            vh,
                            gap,
                            core::nav::xz_dist(u.soldier.pos, v.pos),
                            u.soldier.pos.y - v.pos.y,
                            c.5,
                        );
                    }
                }
            }
            self.last[i] = Some((
                u.gen,
                u.mount.is_some(),
                u.soldier.pos,
                cap.center(),
                cap.radius.max(cap.height * 0.5),
            ));
        }
    }
}

/// A seated spell of [`TurretCensus`]: (generation, first tick, bolts, the
/// last bolt's tick, over 60 s with no bolt in 20 s yet).
type Spell = (u32, u64, u64, u64, bool);

/// Card TURR's census, the `turrets:` line (`ROUND_VERBOSE=1`, a map with
/// building turrets): the share of AI time in a building turret's seat, the
/// seated spells (over 60 s; over 60 s with no shot in their last 20 s -
/// bar B8: none), the bolts fired a seated minute, the gunner-ticks seated
/// once the gunner's team had been in the endgame for a second (B8: none),
/// the gunners' hits and kills on soldiers, how claims ended and the
/// seated time per turret. Read-only.
#[derive(Default)]
struct TurretCensus {
    /// AI unit-ticks alive, and of those in a building turret's seat.
    ai: u64,
    seated: u64,
    /// Per unit index, its open spell.
    open: Vec<Option<Spell>>,
    /// Closed spells: (ticks, bolts, the quiet flag).
    spells: Vec<(u64, u64, bool)>,
    endgame: u64,
    endgame_since: [Option<u64>; 2],
    /// Seated ticks per vehicle slot.
    by_turret: Vec<u64>,
    /// Per unit index, its open claim on a turret (generation, turret), and
    /// how claims ended: [boarded, died on the way, dropped].
    claim: Vec<Option<(u32, core::vehicle::VehicleHandle)>>,
    claim_end: [u64; 3],
    /// Hits on soldiers by gunners in building turrets, and the kills.
    hits: u64,
    kills: u64,
}

impl TurretCensus {
    const LONG: u64 = 60 * core::TICK_HZ as u64;
    const QUIET: u64 = 20 * core::TICK_HZ as u64;

    fn observe(&mut self, sim: &core::sim::Sim) {
        let tick = sim.tick;
        for team in 0..2u8 {
            let e = &mut self.endgame_since[usize::from(team)];
            *e = match (core::ai::man::endgame(sim, team), *e) {
                (true, None) => Some(tick),
                (true, s) => s,
                (false, _) => None,
            };
        }
        let mut seen = vec![false; sim.units.len()];
        if self.open.len() < sim.units.len() {
            self.open.resize(sim.units.len(), None);
        }
        if self.by_turret.len() < sim.vehicles.len() {
            self.by_turret.resize(sim.vehicles.len(), 0);
        }
        for (h, u) in sim.alive_units() {
            if u.controller != core::unit::Controller::Ai {
                continue;
            }
            self.ai += 1;
            let Some(m) = u.mount.filter(|m| core::ai::man::is_turret(sim, m.vehicle)) else {
                continue;
            };
            let i = h.index as usize;
            seen[i] = true;
            self.seated += 1;
            self.by_turret[m.vehicle.index as usize] += 1;
            let bolts = sim
                .events
                .iter()
                .filter(|e| matches!(e, core::sim::Event::Shot { unit, .. } if *unit == h))
                .count() as u64;
            if self.open[i].is_some_and(|o| o.0 != h.gen) {
                self.close(i, tick);
            }
            let o = self.open[i].get_or_insert((h.gen, tick, 0, tick, false));
            o.2 += bolts;
            if bolts > 0 {
                o.3 = tick;
            }
            // (A gunner's exit is asked at its think: a second's grace.)
            o.4 |= tick - o.1 > Self::LONG && tick - o.3 > Self::QUIET + core::TICK_HZ as u64;
            if self.endgame_since[usize::from(u.team).min(1)]
                .is_some_and(|s| tick - s >= core::TICK_HZ as u64)
            {
                self.endgame += 1;
            }
        }
        for (i, s) in seen.iter().enumerate() {
            if !s {
                self.close(i, tick);
            }
        }
        for e in &sim.events {
            if let core::sim::Event::Hit {
                shooter,
                health_left,
                ..
            } = e
            {
                let seated = sim
                    .unit(*shooter)
                    .and_then(|u| u.mount)
                    .is_some_and(|m| core::ai::man::is_turret(sim, m.vehicle));
                if seated {
                    self.hits += 1;
                    self.kills += u64::from(*health_left <= 0.0);
                }
            }
        }
        if self.claim.len() < sim.units.len() {
            self.claim.resize(sim.units.len(), None);
        }
        for i in 0..sim.units.len() {
            let u = &sim.units[i];
            let now = (u.alive && u.mount.is_none())
                .then(|| u.ai.ride.and_then(|r| r.board))
                .flatten()
                .filter(|&vh| core::ai::man::is_turret(sim, vh))
                .map(|vh| (u.gen, vh));
            if let Some((gen, vh)) = self.claim[i].filter(|&c| Some(c) != now) {
                let k = if !u.alive || u.gen != gen {
                    1
                } else if u.mount.is_some_and(|m| m.vehicle == vh) {
                    0
                } else {
                    2
                };
                self.claim_end[k] += 1;
            }
            self.claim[i] = now;
        }
    }

    fn close(&mut self, i: usize, tick: u64) {
        if let Some((_, start, bolts, _, quiet)) = self.open.get_mut(i).and_then(Option::take) {
            self.spells.push((tick - start, bolts, quiet));
        }
    }

    fn line(&mut self, sim: &core::sim::Sim, classes: &[String]) -> Option<String> {
        let turrets: Vec<_> = sim
            .vehicles
            .iter()
            .enumerate()
            .filter(|(_, v)| v.kind == core::vehicle::VehicleKind::Turret)
            .collect();
        if turrets.is_empty() {
            return None;
        }
        for i in 0..self.open.len() {
            self.close(i, sim.tick);
        }
        let n = &sim.nav_stats;
        let long = self.spells.iter().filter(|s| s.0 > Self::LONG).count();
        let quiet = self.spells.iter().filter(|s| s.2).count();
        let longest = self.spells.iter().map(|s| s.0).max().unwrap_or(0);
        let mut lens: Vec<u64> = self.spells.iter().map(|s| s.0).collect();
        lens.sort_unstable();
        let median = lens.get(lens.len() / 2).copied().unwrap_or(0);
        let fired = self.spells.iter().filter(|s| s.1 > 0).count();
        let bolts: u64 = self.spells.iter().map(|s| s.1).sum();
        let minutes = self.seated as f64 / (60.0 * f64::from(core::TICK_HZ));
        let mut by: Vec<(usize, u64)> = turrets
            .iter()
            .map(|(i, _)| (*i, self.by_turret.get(*i).copied().unwrap_or(0)))
            .filter(|b| b.1 > 0)
            .collect();
        by.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let by: Vec<String> = by
            .iter()
            .take(6)
            .map(|&(i, t)| {
                let v = &sim.vehicles[i];
                format!(
                    "{} ({:.0},{:.0},{:.0}) {} s",
                    classes.get(v.class as usize).map_or("?", String::as_str),
                    v.pos.x,
                    v.pos.y,
                    v.pos.z,
                    t / u64::from(core::TICK_HZ)
                )
            })
            .collect();
        Some(format!(
            "    turrets: {} | seat share {:.2} % of AI time | spells {} (with a shot {fired}, median {:.1} s, over 60 s {long}, over 60 s with no shot in its last 20 s {quiet}, longest {} s) | bolts a seated minute {:.1}, hits {} kills {} | seated in the endgame {} ticks | offers {} (defence {}) boards {} (claims boarded {}, died on the way {}, dropped {}) | exits 1:{} 2:{} 3:{} 4:{} 5:{} unreached {} | by turret [{}]",
            turrets.len(),
            100.0 * self.seated as f64 / self.ai.max(1) as f64,
            self.spells.len(),
            median as f64 / f64::from(core::TICK_HZ),
            longest / u64::from(core::TICK_HZ),
            if minutes > 0.0 { bolts as f64 / minutes } else { 0.0 },
            self.hits,
            self.kills,
            self.endgame,
            n.man_offers,
            n.man_defence_offers,
            n.man_boards,
            self.claim_end[0],
            self.claim_end[1],
            self.claim_end[2],
            n.man_codes[1],
            n.man_codes[2],
            n.man_codes[3],
            n.man_codes[4],
            n.man_codes[5],
            n.man_codes[6],
            by.join(", "),
        ))
    }
}

/// One unit's sample of a second ago, ten seconds ago, … (`Shares`).
#[derive(Clone, Copy)]
struct Past {
    tick: u64,
    gen: u32,
    team: u8,
    pos: glam::Vec3,
    speed: f32,
}

/// Slots of the per-behaviour counters: one per `core::ai::Behaviour`
/// (its last variant + 1).
const N_BEHAVIOURS: usize = core::ai::Behaviour::Pickup as usize + 1;

/// The shares line (module header, card RCYC): walking in place, the locus
/// (of the units on foot, card FOFG) and the steering, from one sample per
/// alive unit per second. Read-only.
#[derive(Default)]
struct Shares {
    n: u64,
    /// Samples that were moving and had come under 1 m ten seconds later.
    wip: u64,
    /// ...by `(team, x, y, z)` cell: 6 m in XZ, 1 m in y.
    cells: std::collections::BTreeMap<(u8, i32, i32, i32), u32>,
    /// Units on foot (no `mount`) by locus: `H`, `A`, `N`, none.
    locus: [u64; 4],
    /// Samples in a vehicle or turret seat (not in `locus`).
    seated: u64,
    /// idle, fwd, slide, fan, back, ledge.
    steer: [u64; 6],
    slide_in_n: u64,
    /// Samples on a hull past a rim (module header), ...
    rim: u64,
    /// ...those the level test does not call off the level, ...
    rim_on: u64,
    /// ...and the longest unbroken stay out there (samples = seconds).
    rim_longest: u32,
    /// Per unit index: its generation and its stay so far.
    rim_stay: Vec<(u32, u32)>,
    /// Per unit index: its last ten samples, by `(tick / 30) % 10`.
    past: Vec<[Option<Past>; 10]>,
    /// Samples with an alive enemy within [`CONTACT_M`].
    contact: u64,
    /// AI samples on foot, and those whose target post was another than at
    /// the same life's sample before.
    foot: u64,
    retarget: u64,
    /// Per unit index: its generation and target post at its last on-foot
    /// sample with an objective.
    target: Vec<Option<(u32, Option<u32>)>>,
    /// B9 (card MOVR, `analysis/bot-design.md` 6.3, 10) — UNEXPLAINED
    /// STANDING: AI samples on foot, and those inside a spell of
    /// [`STAND_SECS`] or more within [`STAND_M`] of where it began, with no
    /// target, in a behaviour that has no reason to stand
    /// (`Behaviour::stands`: not Capture / Guard / a seat).
    stand_foot: u64,
    stand: u64,
    /// ...by the behaviour at the sample (`Behaviour as usize`).
    stand_by: [u64; N_BEHAVIOURS],
    /// Card FGHT: every on-foot AI sample by its behaviour — the `behave`
    /// line.
    behave_by: [u64; N_BEHAVIOURS],
    /// B12 (card TPOS): the Fight samples at a Tactician's position with
    /// cover from the target (`core::ai::tactic::covered_from_target`), and
    /// those at one at all.
    cover_fight: u64,
    at_position: u64,
    /// ...and the Fight samples by `FightState::mode` (0 ..= 6), and of the
    /// COVER ones those chosen with cover from the target.
    fight_mode: [u64; 7],
    chosen_covered: u64,
    /// ...and the COVER ones at a position of an authored Cover hint.
    at_cover_hint: u64,
    /// ...and every Fight sample in one of [`FIGHT_SPLIT`]'s classes.
    fight_split: [u64; 13],
    /// Per unit index: its generation, where the spell began, its samples.
    stand_at: Vec<(u32, glam::Vec3, u32, [u64; N_BEHAVIOURS])>,
}

/// The `fightsplit` line's classes of a Fight sample (card TPOS): no
/// position and the last query found none (waiting to ask again), no
/// position otherwise, card FGHT's own stand / walk / pause / move, walking
/// to a covered / an open position, at a covered one with cover from the
/// target (B12), more than B12's 1.5 m off it, the target not behind its
/// cover (gone round), no contact held, at an open one.
const FIGHT_SPLIT: [&str; 13] = [
    "none-wait",
    "none",
    "own",
    "take-covered",
    "take-open",
    "B12",
    "off-spot",
    "round",
    "no-contact",
    "open",
    "endgame",
    "vehicle-target",
    "own-wait",
];

/// B9's spell: this long (s = samples)...
const STAND_SECS: u32 = 20;
/// ...within this of where it began (m, XZ).
const STAND_M: f32 = 2.0;

/// `contact20`'s distance (m).
const CONTACT_M: f32 = 20.0;

/// A unit beyond a rim is on a hull when it is this far under the nearest
/// hub's floor (m): the census's own threshold, over a kerb.
const RIM_UNDER: f32 = 0.3;

/// A Fight sample's [`FIGHT_SPLIT`] class.
fn fight_class(sim: &core::sim::Sim, u: &core::unit::Unit) -> usize {
    use core::ai::FightState as F;
    let st = &u.ai;
    let held = st
        .tpos
        .held
        .and_then(|i| sim.positions.positions.get(i as usize));
    if sim
        .battle
        .teams
        .iter()
        .any(|t| t.tickets() <= core::ai::HOLD_ENDGAME)
    {
        return 10;
    }
    if st
        .target
        .and_then(|h| h.unit())
        .and_then(|h| sim.unit(h))
        .is_some_and(|t| t.mount.is_some())
    {
        return 11;
    }
    match st.fight.mode {
        0 if st.tpos.retry_at > sim.tick => 0,
        0 => 1,
        F::TAKE if st.tpos.covered => 3,
        F::TAKE => 4,
        F::COVER if st.tpos.covered => {
            let Some(q) = held else { return 1 };
            let c = st.target.and_then(|h| core::perception::contact_of(st, h));
            if core::ai::tactic::covered_from_target(sim, u) {
                5
            } else if core::nav::xz_dist(u.soldier.pos, q.pos) > core::ai::tactic::NEAR_ENOUGH {
                6
            } else if c.is_some() {
                7
            } else {
                8
            }
        }
        F::COVER => 9,
        _ if st.tpos.retry_at > sim.tick => 12,
        _ => 2,
    }
}

impl Shares {
    fn observe(&mut self, sim: &core::sim::Sim, tick: u64) {
        use core::ai::SteerPick;
        if !tick.is_multiple_of(30) {
            return;
        }
        let slot = ((tick / 30) % 10) as usize;
        for (h, u) in sim.alive_units() {
            let i = h.index as usize;
            if self.past.len() <= i {
                self.past.resize(i + 1, [None; 10]);
                self.rim_stay.resize(i + 1, (0, 0));
                self.target.resize(i + 1, None);
                self.stand_at
                    .resize(i + 1, (0, glam::Vec3::ZERO, 0, [0; N_BEHAVIOURS]));
            }
            let p = u.soldier.pos;
            // B9: unexplained standing.
            if u.controller == core::unit::Controller::Ai && u.mount.is_none() {
                self.stand_foot += 1;
                let b = u.ai.behaviour;
                self.behave_by[b as usize] += 1;
                if b == core::ai::Behaviour::Fight {
                    self.cover_fight += u64::from(core::ai::tactic::covered_from_target(sim, u));
                    self.at_position += u64::from(u.ai.fight.mode == core::ai::FightState::COVER);
                    self.fight_mode[usize::from(u.ai.fight.mode).min(6)] += 1;
                    self.chosen_covered += u64::from(
                        u.ai.fight.mode == core::ai::FightState::COVER && u.ai.tpos.covered,
                    );
                    self.fight_split[fight_class(sim, u)] += 1;
                    self.at_cover_hint += u64::from(
                        u.ai.fight.mode == core::ai::FightState::COVER
                            && u.ai
                                .tpos
                                .held
                                .and_then(|i| sim.positions.positions.get(i as usize))
                                .is_some_and(|q| q.source == core::ai::tactic::Source::Cover),
                    );
                }
                let (gen, at, n, _) = self.stand_at[i];
                let idle = u.ai.target.is_none() && !b.stands();
                if idle && gen == h.gen && n > 0 && core::nav::xz_dist(at, p) < STAND_M {
                    self.stand_at[i].2 = n + 1;
                    // The spell counts whole once it is long enough.
                    let add = match n + 1 {
                        STAND_SECS => u64::from(STAND_SECS),
                        k if k > STAND_SECS => 1,
                        _ => 0,
                    };
                    self.stand += add;
                    if n < STAND_SECS {
                        self.stand_at[i].3[b as usize] += 1;
                    }
                    if n + 1 == STAND_SECS {
                        for (sum, pending) in self.stand_by.iter_mut().zip(self.stand_at[i].3) {
                            *sum += pending;
                        }
                    } else if n + 1 > STAND_SECS {
                        self.stand_by[b as usize] += 1;
                    }
                } else {
                    let mut pending = [0; N_BEHAVIOURS];
                    pending[b as usize] = u64::from(idle);
                    self.stand_at[i] = (h.gen, p, u32::from(idle), pending);
                }
            } else {
                self.stand_at[i].2 = 0;
            }
            self.contact += u64::from(
                sim.alive_units()
                    .any(|(_, o)| o.team != u.team && (o.soldier.pos - p).length() < CONTACT_M),
            );
            let objective = sim.commander[usize::from(u.team).min(1)].of(h.index);
            match objective.filter(|_| u.controller == core::unit::Controller::Ai) {
                Some(o) if u.mount.is_none() => {
                    self.foot += 1;
                    let now = (h.gen, o.cp());
                    self.retarget +=
                        u64::from(self.target[i].is_some_and(|was| was.0 == now.0 && was != now));
                    self.target[i] = Some(now);
                }
                // A seat or no objective breaks the run: the PS2's count is
                // of bots on foot.
                _ => self.target[i] = None,
            }
            if sim.rim_drop(p).is_some_and(|d| d >= RIM_UNDER) {
                self.rim += 1;
                self.rim_on += u64::from(!sim.level_at(p).is_off());
                let stay = &mut self.rim_stay[i];
                *stay = (h.gen, if stay.0 == h.gen { stay.1 + 1 } else { 1 });
                self.rim_longest = self.rim_longest.max(stay.1);
            } else {
                self.rim_stay[i] = (h.gen, 0);
            }
            // The slot about to be overwritten is this unit ten seconds ago.
            if let Some(was) = self.past[i][slot].filter(|w| w.gen == h.gen && w.tick + 300 == tick)
            {
                if was.speed > 1.0 && core::nav::xz_dist(was.pos, p) < 1.0 {
                    self.wip += 1;
                    let cell = |v: f32| (v / 6.0).round() as i32 * 6;
                    *self
                        .cells
                        .entry((
                            was.team,
                            cell(was.pos.x),
                            was.pos.y.round() as i32,
                            cell(was.pos.z),
                        ))
                        .or_insert(0) += 1;
                }
            }
            self.past[i][slot] = Some(Past {
                tick,
                gen: h.gen,
                team: u.team,
                pos: p,
                speed: glam::Vec2::new(u.soldier.vel.x, u.soldier.vel.z).length(),
            });
            self.n += 1;
            // A rider sits where its seat is: off the graph is a foot matter.
            let l = if u.mount.is_some() {
                self.seated += 1;
                None
            } else {
                let l = match locus_of(sim, p).0 {
                    'H' => 0,
                    'A' => 1,
                    'N' => 2,
                    _ => 3,
                };
                self.locus[l] += 1;
                Some(l)
            };
            let s = match u.ai.steer {
                SteerPick::Idle => 0,
                // (Card OBST: walking by a body seen ahead is an open walk.)
                SteerPick::Fwd | SteerPick::Round => 1,
                SteerPick::Slide => 2,
                SteerPick::Fan(_) => 3,
                SteerPick::Back => 4,
                SteerPick::Ledge => 5,
            };
            self.steer[s] += 1;
            self.slide_in_n += u64::from(l == Some(2) && s == 2);
        }
    }

    fn line(&self, key: &str) -> String {
        let pct = |a: u64, b: u64| {
            if b == 0 {
                0.0
            } else {
                100.0 * a as f64 / b as f64
            }
        };
        let n = self.n;
        let on_foot: u64 = self.locus.iter().sum();
        let mut top: Vec<_> = self.cells.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top = top
            .iter()
            .take(5)
            .map(|((t, x, y, z), c)| format!("t{t}({x},{y},{z}):{c}"))
            .collect::<Vec<_>>()
            .join(" ");
        use core::ai::Behaviour as B;
        let by = [
            B::None,
            B::Travel,
            B::Fight,
            B::Evade,
            B::Board,
            B::Follow,
            B::Recover,
            B::Pickup,
        ]
        .iter()
        .filter(|b| self.stand_by[**b as usize] > 0)
        .map(|b| format!("{} {}", b.tag(), self.stand_by[*b as usize]))
        .collect::<Vec<_>>()
        .join(", ");
        let behave = [
            B::Travel,
            B::Fight,
            B::Evade,
            B::Capture,
            B::Guard,
            B::Board,
            B::Follow,
            B::Recover,
            B::Pickup,
            B::None,
            B::Overwatch,
        ]
        .iter()
        .map(|b| format!("{} {}", b.tag(), self.behave_by[*b as usize]))
        .collect::<Vec<_>>()
        .join(" ");
        format!(
            "shares {key}: unit-s {n} | walk-in-place {} ({:.1} %) | locus H {:.1} % A {:.1} % N {:.1} % | steer idle {:.1} fwd {:.1} slide {:.1} fan {:.1} back {:.1} ledge {:.1} % | slide-in-N {:.1} % | rim {} ({:.2} %) on-level {} longest {} s | contact20 {:.1} % | retarget {} in {} foot-s | stand20 {} in {} foot-s ({:.2} %) [{}] | seated {} ({:.1} %) foot {} | top {}\nbehave {key}: foot-s {} | {behave} | B12 cover {} at-position {} of fight {} ({:.1} %) modes none {} stand {} walk {} pause {} move {} take {} cover {} chosen-covered {} at-cover-hint {}\n",
            self.wip,
            pct(self.wip, n),
            pct(self.locus[0], on_foot),
            pct(self.locus[1], on_foot),
            pct(self.locus[2], on_foot),
            pct(self.steer[0], n),
            pct(self.steer[1], n),
            pct(self.steer[2], n),
            pct(self.steer[3], n),
            pct(self.steer[4], n),
            pct(self.steer[5], n),
            pct(self.slide_in_n, self.locus[2]),
            self.rim,
            pct(self.rim, n),
            self.rim_on,
            self.rim_longest,
            pct(self.contact, n),
            self.retarget,
            self.foot,
            self.stand,
            self.stand_foot,
            pct(self.stand, self.stand_foot),
            by,
            self.seated,
            pct(self.seated, n),
            on_foot,
            if top.is_empty() { "-" } else { &top },
            self.stand_foot,
            self.cover_fight,
            self.at_position,
            self.behave_by[B::Fight as usize],
            pct(self.cover_fight, self.behave_by[B::Fight as usize]),
            self.fight_mode[0],
            self.fight_mode[1],
            self.fight_mode[2],
            self.fight_mode[3],
            self.fight_mode[4],
            self.fight_mode[5],
            self.fight_mode[6],
            self.chosen_covered,
            self.at_cover_hint,
        ) + &format!(
            "fightsplit {key}: fight {} | {}\n",
            self.behave_by[B::Fight as usize],
            FIGHT_SPLIT
                .iter()
                .zip(self.fight_split)
                .map(|(k, v)| format!("{k} {v} ({:.1} %)", pct(v, self.behave_by[B::Fight as usize])))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

/// `RIM_LOG` (module header, card BOWL): one line each time a unit comes
/// onto a hull past a rim, with what moved it. Read-only.
#[derive(Default)]
struct RimLog {
    /// Per unit index: its generation, where it was last tick and whether
    /// that was on a hull.
    prev: Vec<Option<(u32, glam::Vec3, bool)>>,
    text: String,
}

impl RimLog {
    fn observe(&mut self, key: &str, sim: &core::sim::Sim, tick: u64) {
        self.prev.resize(sim.units.len(), None);
        for (i, u) in sim.units.iter().enumerate() {
            if !u.alive || u.controller != core::unit::Controller::Ai {
                self.prev[i] = None;
                continue;
            }
            let p = u.soldier.pos;
            let drop = sim.rim_drop(p).filter(|d| *d >= RIM_UNDER);
            let was = self.prev[i].filter(|q| q.0 == u.gen);
            if let (Some(drop), Some(q)) = (drop, was.filter(|q| !q.2)) {
                // What moved it this tick beyond its own velocity (XZ): the
                // separation pass, a wall push.
                let (d, own) = (p - q.1, u.soldier.vel / 30.0);
                let pushed = glam::Vec2::new(d.x - own.x, d.z - own.z).length();
                let r = u.ai.route;
                outln!(
                    self.text,
                    "rim cross {key}: tick {tick} unit {i} team {} pos ({:.1},{:.2},{:.1}) drop {drop:.2} grounded {} speed {:.1} vy {:.1} pushed {pushed:.3} steer {} slide {} hold {} lost {} target {} leg {:?}->{:?} sidestep {} level {:?}",
                    u.team,
                    p.x,
                    p.y,
                    p.z,
                    u.soldier.grounded as u8,
                    glam::Vec2::new(u.soldier.vel.x, u.soldier.vel.z).length(),
                    u.soldier.vel.y,
                    u.ai.steer.label(),
                    u.ai.slide,
                    u.ai.hold.is_some_and(|h| tick < h.until) as u8,
                    u.ai.lost.is_some() as u8,
                    u.ai.target.is_some() as u8,
                    r.and_then(|r| r.via_arc),
                    r.map(|r| r.next_hub),
                    r.map_or(0, |r| r.sidestep),
                    sim.level_at(p),
                );
            }
            self.prev[i] = Some((u.gen, p, drop.is_some()));
        }
    }
}

/// `POSITIONS=1` (card TPOS): the map's position table (`core::ai::tactic`)
/// as built at load - per source the points tried, standable and covered, the
/// rays and the seconds - and per post the standable and covered positions
/// within 30 m on its storey. Read-only.
fn positions_line(key: &str, sim: &core::sim::Sim) -> String {
    use core::ai::tactic::Source;
    let mut text = String::new();
    if std::env::var_os("POSITIONS").is_none() {
        return text;
    }
    let t = &sim.positions;
    let s = &t.stats;
    let all: u32 = s.standable.iter().sum();
    let cov: u32 = s.covered.iter().sum();
    outln!(
        text,
        "positions {key}: standable {all} covered {cov} | rays {} in {:.3} s",
        s.rays,
        s.secs
    );
    for src in Source::ALL {
        let i = src as usize;
        outln!(
            text,
            "positions {key}: {:13} tried {:5} standable {:5} covered {:5}",
            src.name(),
            s.tried[i],
            s.standable[i],
            s.covered[i]
        );
    }
    let mut near = Vec::new();
    for (i, cp) in sim.cps.iter().enumerate() {
        let pf = sim.ground_under(cp.def.pos, false);
        t.within(cp.def.pos, 30.0, &mut near);
        let on = |k: &&u32| (t.positions[**k as usize].pos.y - pf).abs() <= 3.0;
        let n = near.iter().filter(on).count();
        let c = near
            .iter()
            .filter(on)
            .filter(|k| t.positions[**k as usize].covered())
            .count();
        outln!(
            text,
            "positions {key}: post {i} {:12} within 30 m: standable {n:4} covered {c:4}",
            cp.def.name
        );
    }
    text
}

/// Bound the diagnostic's in-memory report to one million cells.
fn parse_floor_grid(v: &str) -> Result<(f32, f32, f32, f32, f32, i32), &'static str> {
    let a = v
        .split(',')
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "expected x,z,half,step,y_top numbers")?;
    let &[cx, cz, half, step, top] = a.as_slice() else {
        return Err("expected five numbers: x,z,half,step,y_top");
    };
    if !a.iter().all(|x| x.is_finite()) || half < 0.0 || step <= 0.0 {
        return Err("values must be finite, half nonnegative and step positive");
    }
    let n = (half / step).round();
    // Before casting or iterating: (2 * 499 + 1)^2 = 998001 cells.
    if !n.is_finite() || n > 499.0 {
        return Err("grid exceeds one million cells; reduce half or increase step");
    }
    if ![cx - n * step, cx + n * step, cz - n * step, cz + n * step]
        .iter()
        .all(|x| x.is_finite())
    {
        return Err("grid coordinates overflow");
    }
    Ok((cx, cz, half, step, top, n as i32))
}

/// `FLOOR_GRID=x,z,half,step,y_top` (card STLVL): the floors on a grid round a
/// point — the first floor under `y_top` at each cell — with the map's posts
/// and the hubs near it (authored height, radius, resolved floor). Read-only.
fn floor_grid(key: &str, sim: &core::sim::Sim) -> String {
    let mut text = String::new();
    let Ok(v) = std::env::var("FLOOR_GRID") else {
        return text;
    };
    let (cx, cz, half, step, top, n) = match parse_floor_grid(&v) {
        Ok(grid) => grid,
        Err(reason) => {
            outln!(text, "floor_grid {key}: invalid FLOOR_GRID: {reason}");
            return text;
        }
    };
    outln!(text, "floor_grid {key}: centre ({cx},{cz}) step {step} from y {top}; rows z ascending, columns x ascending");
    for iz in -n..=n {
        let z = cz + iz as f32 * step;
        let mut row = format!("fg z {z:7.1} |");
        for ix in -n..=n {
            let x = cx + ix as f32 * step;
            let f = sim.ground_under(glam::Vec3::new(x, top, z), false);
            if f <= core::collision::NO_GROUND {
                row.push_str("     .");
            } else {
                let _ = write!(row, " {f:5.1}");
            }
        }
        outln!(text, "{row}");
    }
    for (n, y) in &sim.spawn_seats {
        outln!(
            text,
            "fg seat ({:.1},{:.1},{:.1}) stands at {y:.1}: its floor {:.1} is a pen",
            n.x,
            n.y,
            n.z,
            sim.ground_under(*n, false)
        );
    }
    if let Some(g) = sim.nav.as_ref() {
        for (i, h) in g.hubs.iter().enumerate() {
            let d = glam::Vec2::new(h.pos.x - cx, h.pos.z - cz).length();
            if d - h.radius <= half * 1.5 {
                outln!(
                    text,
                    "fg hub {i} ({:.1},{:.1},{:.1}) r {:.1} floor {:?} d {d:.1}",
                    h.pos.x,
                    h.pos.y,
                    h.pos.z,
                    h.radius,
                    sim.nav_floor(i as u16)
                );
            }
        }
        for (i, arc) in g.arcs.iter().enumerate() {
            let near = |h: u16| {
                g.hub(h).is_some_and(|h| {
                    glam::Vec2::new(h.pos.x - cx, h.pos.z - cz).length() - h.radius <= half * 1.5
                })
            };
            if near(arc.a) || near(arc.b) {
                outln!(
                    text,
                    "fg arc {i} {}-{} mask {:#x} flags {:#x} unwalkable a>b {} b>a {}",
                    arc.a,
                    arc.b,
                    arc.type_mask,
                    arc.flags,
                    sim.arc_unwalkable(i as u16, arc.a),
                    sim.arc_unwalkable(i as u16, arc.b)
                );
            }
        }
    }
    for (i, cp) in sim.cps.iter().enumerate() {
        outln!(
            text,
            "fg cp {i} {} pos ({:.1},{:.1},{:.1}) ground {:.1} hub {:?} capture {:?}",
            cp.def.name,
            cp.def.pos.x,
            cp.def.pos.y,
            cp.def.pos.z,
            sim.ground_under(cp.def.pos, false),
            cp.nav_hub,
            cp.def.capture
        );
        for (p, _) in &cp.def.spawn_nodes {
            if glam::Vec2::new(p.x - cx, p.z - cz).length() <= half * 2.0 {
                outln!(
                    text,
                    "fg spawn cp {i} ({:.1},{:.1},{:.1}) ground {:.1}",
                    p.x,
                    p.y,
                    p.z,
                    sim.ground_under(*p, false)
                );
            }
        }
    }
    text
}

/// `LEVEL_PROFILE=x0,z0,x1,z1,y,step` (module header): the level test along
/// a line, one row per step. Empty when unset or unparseable.
fn level_profile(key: &str, sim: &core::sim::Sim) -> String {
    let mut text = String::new();
    let Ok(v) = std::env::var("LEVEL_PROFILE") else {
        return text;
    };
    let a: Vec<f32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
    let &[x0, z0, x1, z1, y, step] = a.as_slice() else {
        return text;
    };
    let (p0, p1) = (glam::Vec2::new(x0, z0), glam::Vec2::new(x1, z1));
    let n = ((p1 - p0).length() / step.max(0.01)) as usize;
    for i in 0..=n {
        let q = p0.lerp(p1, i as f32 / n.max(1) as f32);
        let (f, nrm) = sim.ground_under_n(glam::Vec3::new(q.x, y, q.y), false, y);
        let feet = glam::Vec3::new(q.x, f, q.y);
        outln!(
            text,
            "profile {key}: d {:5.1} x {:7.2} z {:7.2} y {f:8.2} ny {:+.2} level {:?} rim_drop {:?}",
            i as f32 * step,
            q.x,
            q.y,
            nrm.y,
            sim.level_at(feet),
            sim.rim_drop(feet).map(|d| (d * 100.0).round() / 100.0),
        );
    }
    text
}

/// `LOCUS_CSV=<file>` (module header): the locus and level shares of the
/// positions in a CSV. Empty when unset, unreadable or without the columns.
fn locus_csv(key: &str, sim: &core::sim::Sim) -> String {
    let mut text = String::new();
    let Some(csv) = std::env::var_os("LOCUS_CSV").and_then(|f| std::fs::read_to_string(f).ok())
    else {
        return text;
    };
    let mut lines = csv.lines();
    let cols: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |name: &str| cols.iter().position(|c| *c == name);
    let (Some(cx), Some(cy), Some(cz)) = (col("x"), col("y"), col("z")) else {
        return text;
    };
    let (mut n, mut locus, mut level) = (0u32, [0u32; 3], [0u32; 3]);
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let at = |c: usize| f.get(c).and_then(|v| v.parse::<f32>().ok());
        let (Some(x), Some(y), Some(z)) = (at(cx), at(cy), at(cz)) else {
            continue;
        };
        let p = glam::Vec3::new(x, y, z);
        n += 1;
        match locus_of(sim, p).0 {
            'H' => locus[0] += 1,
            'A' => locus[1] += 1,
            'N' => locus[2] += 1,
            _ => {}
        }
        match sim.level_at(p) {
            core::sim::Level::On => level[0] += 1,
            core::sim::Level::Off(_) => level[1] += 1,
            core::sim::Level::Unknown => level[2] += 1,
        }
    }
    let pct = |k: u32| 100.0 * k as f32 / n.max(1) as f32;
    outln!(
        text,
        "locus-csv {key}: rows {n} | locus H {:.1} A {:.1} N {:.1} % | level on {:.1} off {:.1} unknown {:.1} %",
        pct(locus[0]),
        pct(locus[1]),
        pct(locus[2]),
        pct(level[0]),
        pct(level[1]),
        pct(level[2]),
    );
    text
}

/// `BLACKBOX_AT` / `BLACKBOX_ON` (module header): the playtest black box
/// (`game::blackbox`, card BLKB) on a headless round, and when it writes.
struct HeadlessBox {
    bb: game::blackbox::BlackBox,
    path: std::path::PathBuf,
    at: Option<u64>,
    /// `lost` / `recycle` / `stuck`, and the stuck run that counts (ticks).
    on: Option<String>,
    stuck_ticks: u16,
    written: bool,
    /// `BLACKBOX_TIMING=1`: what each recorded tick cost (µs), for the line
    /// [`HeadlessBox::finish`] prints.
    timing: Option<Vec<f32>>,
}

impl HeadlessBox {
    fn new(key: &str, seed: u32, scene: &game::assets::Scene, w: &WalkSim) -> Option<HeadlessBox> {
        let at = std::env::var("BLACKBOX_AT")
            .ok()
            .and_then(|v| v.parse().ok());
        let on = std::env::var("BLACKBOX_ON").ok();
        if at.is_none() && on.is_none() {
            return None;
        }
        let (on, stuck_ticks) = match on.as_deref().map(|o| o.split_once(':').unwrap_or((o, ""))) {
            Some((kind, n)) => (Some(kind.to_owned()), n.parse().unwrap_or(900)),
            None => (None, 900),
        };
        let dir = std::env::var_os("BLACKBOX_DIR").map_or_else(
            || game::blackbox::default_dir().join("headless"),
            std::path::PathBuf::from,
        );
        let mut meta = game::blackbox::Meta::for_run(
            key,
            "con",
            &scene.manifest.source_game,
            seed,
            sweep_difficulty().label(),
        );
        // The sweep's mission (`mission_for`), when the map has one.
        if mission_for(key, &mut String::new()).is_some() {
            meta.mission = mission_path(key)
                .file_name()
                .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        }
        let bb = game::blackbox::BlackBox::start(
            true,
            meta,
            w,
            scene.collision_body_labels(),
            dir.clone(),
        )?;
        Some(HeadlessBox {
            bb,
            path: dir.join(format!("{key}_s{seed}.json")),
            at,
            on,
            stuck_ticks,
            written: false,
            timing: std::env::var_os("BLACKBOX_TIMING").map(|_| Vec::new()),
        })
    }

    /// Record the tick; write once, at the tick or at the first event asked for.
    fn observe(&mut self, w: &WalkSim, t: u64) {
        use core::sim::{DeathCause, Event};
        let started = std::time::Instant::now();
        self.bb.tick(w, None);
        if let Some(v) = self.timing.as_mut() {
            v.push(started.elapsed().as_secs_f32() * 1e6);
        }
        if self.written {
            return;
        }
        let mut hit: Option<(String, Option<u32>)> =
            (self.at == Some(t)).then(|| (format!("manual: tick {t}"), None));
        match self.on.as_deref() {
            Some(kind @ ("lost" | "recycle")) => {
                let want = if kind == "lost" {
                    DeathCause::Lost
                } else {
                    DeathCause::Stalled
                };
                for ev in &w.sim.events {
                    if let Event::EnvDeath { unit, cause } = ev {
                        if *cause == want && hit.is_none() {
                            hit = Some((format!("{kind}: unit {}", unit.index), Some(unit.index)));
                        }
                    }
                }
            }
            Some("end") if w.sim.outcome.is_some() => {
                hit = Some(("end: the round's outcome".to_owned(), None));
            }
            Some("stuck") if hit.is_none() => {
                hit = w
                    .sim
                    .alive_units()
                    .find(|(_, u)| u.ai.route.is_some_and(|r| r.stuck_run >= self.stuck_ticks))
                    .map(|(h, _)| (format!("stuck: unit {}", h.index), Some(h.index)));
            }
            _ => {}
        }
        let Some((reason, unit)) = hit else { return };
        self.written = true;
        let started = std::time::Instant::now();
        let snap = self.bb.snapshot(&reason, None, unit);
        let copied = started.elapsed().as_secs_f32() * 1e3;
        match game::blackbox::write_to(&self.path, &snap) {
            Ok(()) => eprintln!("blackbox: wrote {} ({reason})", self.path.display()),
            Err(e) => eprintln!("blackbox: {}: {e}", self.path.display()),
        }
        if self.timing.is_some() {
            eprintln!(
                "blackbox timing: snapshot copy {copied:.2} ms (the frame's share of a hotkey write), encode + write {:.1} ms (its own thread in the game), {} units in the last frame",
                started.elapsed().as_secs_f32() * 1e3 - copied,
                snap.frames.last().map_or(0, |f| f.units.len()),
            );
        }
    }

    /// Card DPRX: the proxy's profile in the file's environment list, so a
    /// dry run's file says what it measured.
    fn proxy_note(&mut self, p: &core::ai::proxy::Profile, label: &str) {
        self.bb.note_env(
            "SWEEP_PROXY",
            &format!(
                "{label} (skill {} reaction {} view {} circles {} react {:?} grenade {})",
                p.skill, p.reaction_ticks, p.view_mult, p.circles, p.react, p.grenade
            ),
        );
    }

    /// `BLACKBOX_TIMING`: the recorder's cost per tick over the round.
    fn finish(self, key: &str) {
        let Some(mut v) = self.timing else { return };
        if v.is_empty() {
            return;
        }
        v.sort_by(f32::total_cmp);
        let n = v.len();
        eprintln!(
            "blackbox timing {key}: {n} ticks, per tick mean {:.1} us, p99 {:.1} us, max {:.1} us; ring {:.2} MB",
            v.iter().sum::<f32>() / n as f32,
            v[(n * 99 / 100).min(n - 1)],
            v[n - 1],
            self.bb.ring_bytes() as f32 / (1024.0 * 1024.0),
        );
    }
}

/// `UNIT_SAMPLES` (module header): one CSV row per alive unit per second.
struct Samples {
    path: std::path::PathBuf,
    csv: String,
}

impl Samples {
    fn new(key: &str, seed: u32) -> Option<Samples> {
        let dir = std::path::PathBuf::from(std::env::var_os("UNIT_SAMPLES")?);
        let _ = std::fs::create_dir_all(&dir);
        Some(Samples {
            path: dir.join(format!("{key}_s{seed}.csv")),
            csv: String::from(
                "tick,unit,gen,team,class,x,y,z,grounded,speed,locus,hub,hub_floor_dy,steer,target,stuck,objective,hub_dxz,hub_r,overhead,level,hold,arc,from,next,dest,wp_d\n",
            ),
        })
    }

    fn observe(&mut self, sim: &core::sim::Sim, tick: u64) {
        if !tick.is_multiple_of(30) {
            return;
        }
        // The row is `game::unit_probe::Probe`, the black box's too: one
        // field list, two readers.
        for (h, u) in sim.alive_units() {
            let r = game::unit_probe::Probe::of(sim, h, u, tick, false);
            let (hub_dxz, hub_r) = sim
                .nav
                .as_ref()
                .and_then(|g| g.hub(r.hub.max(0) as u16))
                .map_or((f32::NAN, f32::NAN), |hb| {
                    (core::nav::xz_dist(u.soldier.pos, hb.pos), hb.radius)
                });
            let _ = writeln!(
                self.csv,
                "{tick},{},{},{},{},{:.2},{:.2},{:.2},{},{:.2},{},{},{:.2},{},{},{},{},{hub_dxz:.1},{hub_r:.1},{:.1},{:.1},{},{},{},{},{},{:.1}",
                r.id,
                r.gen,
                r.team,
                r.class,
                r.pos[0],
                r.pos[1],
                r.pos[2],
                r.grounded as u8,
                r.speed,
                r.locus,
                r.hub,
                r.floor_dy,
                r.steer_label(),
                (r.target >= 0) as u8,
                r.stuck,
                r.goal_label(),
                r.overhead,
                r.level,
                r.hold,
                r.arc,
                r.from,
                r.next,
                r.dest,
                r.wp_d,
            );
        }
    }

    fn finish(self) {
        if let Err(e) = std::fs::write(&self.path, self.csv) {
            eprintln!("UNIT_SAMPLES: {}: {e}", self.path.display());
        }
    }
}

/// `OBST_CSV=<dir>` (module header) — every alive bot on foot
/// every 15 ticks with the steering's stop test, the body, the aim point and
/// whether a soldier can stand on it, and the nearest other soldier.
struct Obst {
    path: std::path::PathBuf,
    csv: String,
}

impl Obst {
    fn new(
        key: &str,
        seed: u32,
        scene: &game::assets::Scene,
        sim: &core::sim::Sim,
    ) -> Option<Obst> {
        let dir = std::path::PathBuf::from(std::env::var_os("OBST_CSV")?);
        let _ = std::fs::create_dir_all(&dir);
        let labels = scene.collision_body_labels();
        let mut b = String::from("body,shape,minx,miny,minz,maxx,maxy,maxz,soldier,label\n");
        for (i, body) in sim.collision.bodies().iter().enumerate() {
            let kind = match body.shape {
                core::collision::Shape::Mesh(_) => "mesh",
                core::collision::Shape::Sphere { .. } => "sphere",
                core::collision::Shape::Cylinder { .. } => "cyl",
                core::collision::Shape::Box { .. } => "box",
            };
            let _ = writeln!(
                b,
                "{i},{kind},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{},{}",
                body.aabb.min.x,
                body.aabb.min.y,
                body.aabb.min.z,
                body.aabb.max.x,
                body.aabb.max.y,
                body.aabb.max.z,
                body.layers.contains(core::collision::Layers::SOLDIER) as u8,
                labels.get(i).map_or("?", |l| l.as_str()),
            );
        }
        let _ = std::fs::write(dir.join(format!("{key}_bodies.csv")), b);
        let mut c = String::from("cp,x,y,z,push,hub\n");
        for (i, cp) in sim.cps.iter().enumerate() {
            let cap = core::collision::Capsule {
                base: cp.def.pos,
                height: 1.8,
                radius: core::unit::SOLDIER_RADIUS,
            };
            let push = sim.collision.push_out_soldier(&cap).length();
            let _ = writeln!(
                c,
                "{i},{:.2},{:.2},{:.2},{push:.2},{}",
                cp.def.pos.x,
                cp.def.pos.y,
                cp.def.pos.z,
                locus_of(sim, cp.def.pos).1
            );
        }
        let _ = std::fs::write(dir.join(format!("{key}_posts.csv")), c);
        if let Some(g) = sim.nav.as_ref() {
            let mut n = String::from("kind,i,a,b,x,y,z,r\n");
            for (i, h) in g.hubs.iter().enumerate() {
                let _ = writeln!(
                    n,
                    "H,{i},0,0,{:.2},{:.2},{:.2},{:.2}",
                    h.pos.x, h.pos.y, h.pos.z, h.radius
                );
            }
            for (i, a) in g.arcs.iter().enumerate() {
                let _ = writeln!(n, "A,{i},{},{},0,0,0,0", a.a, a.b);
            }
            let _ = std::fs::write(dir.join(format!("{key}_nav.csv")), n);
        }
        Some(Obst {
            path: dir.join(format!("{key}_s{seed}.csv")),
            csv: String::from(
                "tick,unit,gen,team,x,y,z,speed,steer,slide,sidestep,stop,stop_d,nx,nz,body,wpx,wpy,wpz,wp_d,arc,next,dest,src,cp,goal,stuck,stall,hold,target,lost,wp_push,near_d,near_team,near_ahead,walk,board,veh,veh_kind,veh_class,veh_speed,veh_empty,veh_d\n",
            ),
        })
    }

    fn observe(&mut self, sim: &core::sim::Sim, tick: u64) {
        if !tick.is_multiple_of(15) {
            return;
        }
        for (h, u) in sim.alive_units() {
            if u.is_player || u.mount.is_some() {
                continue;
            }
            let r = game::unit_probe::Probe::of(sim, h, u, tick, true);
            let p = u.soldier.pos;
            // Can a soldier stand on the aim point?
            let wp_push = u.ai.route.map_or(-1.0, |rt| {
                let aim = rt
                    .corner()
                    .map_or(rt.waypoint, |c| glam::Vec3::new(c.x, p.y, c.y));
                let mut cap = u.capsule();
                cap.base = aim;
                sim.collision.push_out_soldier(&cap).length()
            });
            // The nearest other soldier on foot, and whether it is ahead.
            let want = glam::Vec3::new(r.wp[0] - p.x, 0.0, r.wp[2] - p.z).normalize_or_zero();
            let mut near = (f32::MAX, 9u8, 0u8);
            for (h2, o) in sim.alive_units() {
                if h2 == h || o.mount.is_some() {
                    continue;
                }
                let d = glam::Vec3::new(o.soldier.pos.x - p.x, 0.0, o.soldier.pos.z - p.z);
                if (o.soldier.pos.y - p.y).abs() > 2.0 {
                    continue;
                }
                let l = d.length();
                if l < near.0 {
                    near = (
                        l,
                        (o.team == u.team) as u8,
                        (d.normalize_or_zero().dot(want) > 0.7) as u8,
                    );
                }
            }
            // Card OBSV: whose hull shut the way (the sweep's body's owner;
            // for the hull probe, which names none, the nearest vehicle),
            // whether it is parked and empty, and how far its middle is.
            let veh = (u.ai.walk.kind != 0 && u.ai.walk.dir != glam::Vec3::ZERO)
                .then(|| core::ai::way_ahead(sim, u, u.ai.walk.dir, u.ai.walk.low))
                .flatten()
                .and_then(|s| match (s.kind, s.body.and_then(|b| b.owner)) {
                    (_, Some(o)) => Some(o.index as usize),
                    (core::ai::StopKind::Hull, None) => sim
                        .vehicles
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| v.alive)
                        .min_by(|a, b| {
                            (a.1.pos - p)
                                .length_squared()
                                .total_cmp(&(b.1.pos - p).length_squared())
                        })
                        .map(|(i, _)| i),
                    _ => None,
                })
                .and_then(|i| sim.vehicles.get(i).map(|v| (i, v)));
            let veh_cols = veh.map_or_else(
                || String::from("-1,-,-1,0.00,0,0.00"),
                |(i, v)| {
                    format!(
                        "{i},{:?},{},{:.2},{},{:.2}",
                        v.kind,
                        v.class,
                        v.vel.length(),
                        v.is_empty() as u8,
                        glam::Vec3::new(v.pos.x - p.x, 0.0, v.pos.z - p.z).length()
                    )
                },
            );
            let _ = writeln!(
                self.csv,
                "{tick},{},{},{},{:.2},{:.2},{:.2},{:.2},{},{},{},{:?},{:.2},{:.2},{:.2},{},{:.2},{:.2},{:.2},{:.1},{},{},{},{},{},{},{},{},{},{},{},{wp_push:.2},{:.2},{},{},{},{},{veh_cols}",
                r.id,
                r.gen,
                r.team,
                r.pos[0],
                r.pos[1],
                r.pos[2],
                r.speed,
                r.steer_label(),
                r.slide,
                r.sidestep,
                r.stop,
                r.stop_d,
                r.stop_n[0],
                r.stop_n[1],
                r.body,
                r.wp[0],
                r.wp[1],
                r.wp[2],
                r.wp_d,
                r.arc,
                r.next,
                r.dest,
                r.src,
                r.cp,
                r.goal_label(),
                r.stuck,
                r.stall,
                r.hold,
                r.target,
                r.lost,
                near.0.min(999.0),
                near.1,
                near.2,
                r.walk,
                r.board,
            );
        }
    }

    fn finish(self) {
        if let Err(e) = std::fs::write(&self.path, self.csv) {
            eprintln!("OBST_CSV: {}: {e}", self.path.display());
        }
    }
}

/// `BOT_SAMPLES` (module header): the 4 Hz rows and the fire events
/// `scripts/bot_quality.py` reads.
struct BotSamples {
    path: std::path::PathBuf,
    csv: String,
    ev: String,
    /// `BOT_SAMPLES_SECS`: the last tick sampled.
    until: u64,
}

impl BotSamples {
    fn new(key: &str, seed: u32) -> Option<BotSamples> {
        let dir = std::path::PathBuf::from(std::env::var_os("BOT_SAMPLES")?);
        let _ = std::fs::create_dir_all(&dir);
        Some(BotSamples {
            path: dir.join(format!("{key}_s{seed}.csv")),
            csv: String::from(
                "t,unit,gen,team,human,x,y,z,hp,veh,stance,steer,slide,hold,target,target_d,cp,goal_cp,beh\n",
            ),
            ev: String::from("t,kind,a,a_team,b,b_team,x,z,a_speed\n"),
            until: std::env::var("BOT_SAMPLES_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .map_or(u64::MAX, |s| s * 30),
        })
    }

    fn observe(&mut self, sim: &core::sim::Sim, tick: u64) {
        use core::sim::Event;
        if tick > self.until {
            return;
        }
        let secs = tick as f32 / 30.0;
        // The actor's slot, team and horizontal speed; a dead victim's slot
        // still holds its team and where it fell.
        let who = |h: core::unit::UnitHandle| {
            sim.units
                .get(h.index as usize)
                .map_or((-1, 0.0, 0.0, 0.0), |u| {
                    let v = u.soldier.vel;
                    (
                        i32::from(u.team),
                        u.soldier.pos.x,
                        u.soldier.pos.z,
                        glam::Vec2::new(v.x, v.z).length(),
                    )
                })
        };
        for ev in &sim.events {
            let (kind, a, b) = match *ev {
                Event::Shot { unit, .. } => ("shot", unit, None),
                Event::Hit {
                    shooter, victim, ..
                } => ("hit", shooter, Some(victim)),
                Event::Kill { shooter, victim } => ("kill", shooter, Some(victim)),
                _ => continue,
            };
            let (at, ax, az, speed) = who(a);
            let (bi, bt, x, z) = b.map_or((-1, -1, ax, az), |b| {
                let (bt, bx, bz, _) = who(b);
                (b.index as i32, bt, bx, bz)
            });
            let _ = writeln!(
                self.ev,
                "{secs:.3},{kind},{},{at},{bi},{bt},{x:.1},{z:.1},{speed:.2}",
                a.index
            );
        }
        // Four samples a second: ticks 8, 15, 23, 30, ...
        if (tick * 2) % 15 >= 2 {
            return;
        }
        for (h, u) in sim.alive_units() {
            let r = game::unit_probe::Probe::of(sim, h, u, tick, false);
            let _ = writeln!(
                self.csv,
                "{secs:.3},{},{},{},{},{:.2},{:.2},{:.2},{:.0},{},{},{},{},{},{},{:.1},{},{},{}",
                r.id,
                r.gen,
                r.team,
                u8::from(r.human),
                r.pos[0],
                r.pos[1],
                r.pos[2],
                r.hp,
                r.veh,
                r.stance,
                r.steer_label(),
                r.slide,
                r.hold,
                r.target,
                r.target_d,
                r.cp,
                r.goal_cp,
                u.ai.behaviour.tag(),
            );
        }
    }

    fn finish(self) {
        for (path, text) in [
            (self.path.clone(), self.csv),
            (self.path.with_extension("ev.csv"), self.ev),
        ] {
            if let Err(e) = std::fs::write(&path, text) {
                eprintln!("BOT_SAMPLES: {}: {e}", path.display());
            }
        }
    }
}

/// `HEAVY_CENSUS=<dir>` (card HVYW; read-only): what the bots on foot fire
/// and at what, counted over the round into `<dir>/<key>_s<seed>.hv.csv` as
/// `kind,weapon,what,n,sum` — `shot` / `throw` per weapon by the shooter's
/// target (`veh_<kind>` a rider, `grp` a soldier with 3 or more of his side
/// on foot within 9 m of him, himself counted, `one` any other soldier,
/// `none`), `hand` the 4 Hz samples a weapon was in the hands, `vhit` the
/// hull damage per vehicle class by the weapon in the hands at the hit,
/// `vkill` the vehicles destroyed (`what` the class; by a unit on foot with
/// its weapon, a rider, or nobody), `kill` / `teamkill` / `selfkill`.
struct HeavyCensus {
    path: std::path::PathBuf,
    n: std::collections::BTreeMap<(&'static str, String, String), (u32, f32)>,
    /// One line a team kill by a bot on foot (`<key>_s<seed>.tk.csv`): the
    /// tick, the weapon in its hands, the bolt's age (0 = a blast), how far
    /// the victim is from the shooter, the shooter's target and how far it
    /// is, the victim's place along and off the line to that target, the
    /// victim's speed and the shooter's.
    tk: String,
}

impl HeavyCensus {
    fn new(key: &str, seed: u32) -> Option<HeavyCensus> {
        let dir = std::path::PathBuf::from(std::env::var_os("HEAVY_CENSUS")?);
        let _ = std::fs::create_dir_all(&dir);
        Some(HeavyCensus {
            path: dir.join(format!("{key}_s{seed}.hv.csv")),
            n: Default::default(),
            tk: String::from(
                "tick,weapon,age,d_victim,target,d_target,along,off,v_speed,s_speed\n",
            ),
        })
    }

    fn observe(&mut self, sim: &core::sim::Sim, classes: &[String], tick: u64) {
        use core::sim::Event;
        use core::vehicle::VehicleKind;
        // A weapon's name, `+H` behind a HEAVY one (the choice's own rule,
        // `core::ai::arsenal`: missile ordnance whose AI mask refuses a
        // person, or that is not the soldier's first slot).
        let named = |u: &core::unit::Unit, c: usize| {
            let Some(w) = sim.weapon_classes.get(c) else {
                return "?".to_owned();
            };
            let first = u.held.iter().map(|w| w.slot).fold(u.weapon_slot, u8::min);
            let slot = if u.weapon_class == c {
                u.weapon_slot
            } else {
                u.held
                    .iter()
                    .find(|h| h.class == c)
                    .map_or(first, |h| h.slot)
            };
            let heavy = w.ordnance.kind == core::weapon::OrdKind::Missile
                && (w.ai_target_kind & 1 == 0 || slot != first);
            format!("{}{}", w.name, if heavy { "+H" } else { "" })
        };
        // A slot's unit, alive or just dead, when it is a bot on foot.
        let foot = |h: core::unit::UnitHandle| {
            sim.units
                .get(h.index as usize)
                .filter(|u| u.gen == h.gen && u.mount.is_none() && !u.is_player)
        };
        let target = |u: &core::unit::Unit| -> String {
            let Some(e) = u.ai.target.and_then(|t| t.unit()).and_then(|t| sim.unit(t)) else {
                return "none".to_owned();
            };
            if let Some(m) = e.mount {
                return match sim.vehicle(m.vehicle).map(|v| v.kind) {
                    Some(VehicleKind::Hover) => "veh_hover",
                    Some(VehicleKind::Walker) => "veh_walker",
                    Some(VehicleKind::Flyer) => "veh_flyer",
                    Some(VehicleKind::Turret) => "veh_turret",
                    None => "veh_gone",
                }
                .to_owned();
            }
            let near = sim
                .alive_units()
                .filter(|(_, o)| {
                    o.team == e.team
                        && o.mount.is_none()
                        && o.soldier.pos.distance_squared(e.soldier.pos) <= 81.0
                })
                .count();
            if near >= 3 { "grp" } else { "one" }.to_owned()
        };
        let vclass = |c: u32| {
            classes
                .get(c as usize)
                .cloned()
                .unwrap_or_else(|| format!("class{c}"))
        };
        let mut add = |k: &'static str, w: String, what: String, x: f32| {
            let e = self.n.entry((k, w, what)).or_default();
            e.0 += 1;
            e.1 += x;
        };
        for ev in &sim.events {
            match *ev {
                Event::Shot {
                    unit, weapon_class, ..
                } => {
                    if let Some(u) = foot(unit) {
                        add("shot", named(u, weapon_class as usize), target(u), 0.0);
                    }
                }
                Event::Throw {
                    unit, weapon_class, ..
                } => {
                    if let Some(u) = foot(unit) {
                        add("throw", named(u, weapon_class as usize), target(u), 0.0);
                    }
                }
                Event::VehicleHit {
                    shooter: Some(s),
                    vehicle,
                    damage,
                    ..
                } => {
                    if let Some(u) = foot(s) {
                        let c = sim
                            .vehicles
                            .get(vehicle.index as usize)
                            .map_or(u32::MAX, |v| v.class);
                        add("vhit", named(u, u.weapon_class), vclass(c), damage);
                    }
                }
                Event::VehicleDestroyed { class, killer, .. } => {
                    let by = match killer {
                        None => "nobody".to_owned(),
                        Some(k) => {
                            foot(k).map_or_else(|| "rider".to_owned(), |u| named(u, u.weapon_class))
                        }
                    };
                    add("vkill", by, vclass(class), 0.0);
                }
                Event::Kill { shooter, victim } => {
                    if let (Some(u), Some(v)) = (
                        foot(shooter),
                        sim.units
                            .get(victim.index as usize)
                            .filter(|u| u.gen == victim.gen),
                    ) {
                        let k = if shooter == victim {
                            "selfkill"
                        } else if u.team == v.team {
                            "teamkill"
                        } else {
                            "kill"
                        };
                        add(k, named(u, u.weapon_class), String::new(), 0.0);
                        if k == "teamkill" {
                            let age = sim
                                .events
                                .iter()
                                .rev()
                                .find_map(|e| match *e {
                                    Event::Hit {
                                        shooter: s,
                                        victim: x,
                                        age,
                                        ..
                                    } if s == shooter && x == victim => Some(age),
                                    _ => None,
                                })
                                .unwrap_or(-1.0);
                            let (from, at) = (u.muzzle(), v.capsule().center());
                            let aim =
                                u.ai.target
                                    .and_then(|t| t.unit())
                                    .and_then(|t| sim.unit(t))
                                    .map(|e| e.capsule().center());
                            let (d_target, along, off) = aim.map_or((-1.0, -1.0, -1.0), |a| {
                                let dir = (a - from).normalize_or_zero();
                                let along = (at - from).dot(dir);
                                (a.distance(from), along, (at - from - dir * along).length())
                            });
                            let w = named(u, u.weapon_class);
                            let dv = at.distance(from);
                            let target = target(u);
                            let vs = v.soldier.vel.length();
                            let since = u.soldier.vel.length();
                            let _ = writeln!(
                                self.tk,
                                "{tick},{w},{age:.2},{dv:.1},{target},{d_target:.1},{along:.1},{off:.1},{vs:.1},{since:.2}"
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        if (tick * 2) % 15 < 2 {
            for (_, u) in sim.alive_units() {
                if u.mount.is_none() && !u.is_player {
                    add("hand", named(u, u.weapon_class), String::new(), 0.0);
                }
            }
        }
    }

    fn finish(self) {
        let mut text = String::from("kind,weapon,what,n,sum\n");
        for ((k, w, what), (n, x)) in &self.n {
            let _ = writeln!(text, "{k},{w},{what},{n},{x:.0}");
        }
        if let Err(e) = std::fs::write(&self.path, text) {
            eprintln!("HEAVY_CENSUS: {}: {e}", self.path.display());
        }
        let _ = std::fs::write(
            self.path.with_extension("").with_extension("tk.csv"),
            self.tk,
        );
    }
}

/// The longest stuck run any alive unit showed after any tick, and whose it
/// was: the census's own record of who set the scoreboard's `stuck_max`
/// (which the sim keeps only as a number).
#[derive(Clone, Copy, Default)]
struct LongestRun {
    run: u16,
    unit: u32,
    gen: u32,
    tick: u64,
    pos: glam::Vec3,
    /// Card BSTK: what the unit was doing when it set the record: `droid`
    /// (flying its remote droid), `seated` (in a vehicle or turret seat) or
    /// `foot`.
    doing: &'static str,
}

impl LongestRun {
    fn observe(&mut self, sim: &core::sim::Sim, tick: u64) {
        for (h, u) in sim.alive_units() {
            let run = u.ai.route.map_or(0, |r| r.stuck_run);
            if run > self.run {
                *self = LongestRun {
                    run,
                    unit: h.index,
                    gen: h.gen,
                    tick,
                    pos: u.soldier.pos,
                    doing: if sim.controlled_droid(h).is_some() {
                        "droid"
                    } else if u.mount.is_some() {
                        "seated"
                    } else {
                        "foot"
                    },
                };
            }
        }
    }
}

/// The no-progress census's window: a unit on one leg this many ticks whose
/// best XZ distance to the leg's waypoint gained under `NO_PROGRESS_GAIN`.
/// PROVISIONAL — CEN's pick, long against any leg a unit walks.
const NO_PROGRESS_TICKS: u64 = 6_000;
/// ... and the gain in metres that counts as progress (PROVISIONAL, CEN).
const NO_PROGRESS_GAIN: f32 = 2.0;

/// One unit's current leg as the no-progress census watches it.
#[derive(Clone, Copy)]
struct LegWatch {
    gen: u32,
    /// `(next_hub, dest_hub)`: a new pair is a new leg.
    leg: (u16, u16),
    /// The tick `best` last gained `NO_PROGRESS_GAIN` (or the leg began).
    since: u64,
    /// The best XZ distance to the waypoint as of `since`.
    best: f32,
}

/// Units that keep one leg without closing on it (module header, "The stuck
/// census"): a wall slide moves the unit enough to reset `stuck_run`, so the
/// stuck census alone misses it. Indexed by unit index.
#[derive(Default)]
struct NoProgress {
    units: Vec<Option<LegWatch>>,
}

impl NoProgress {
    fn observe(&mut self, sim: &core::sim::Sim, tick: u64) {
        for (h, u) in sim.alive_units() {
            let i = h.index as usize;
            if self.units.len() <= i {
                self.units.resize(i + 1, None);
            }
            let Some(r) = u.ai.route else {
                self.units[i] = None;
                continue;
            };
            let leg = (r.next_hub, r.dest_hub);
            let d = core::nav::xz_dist(u.soldier.pos, r.waypoint);
            match &mut self.units[i] {
                Some(w) if w.gen == h.gen && w.leg == leg => {
                    if d <= w.best - NO_PROGRESS_GAIN {
                        w.best = d;
                        w.since = tick;
                    }
                }
                slot => {
                    *slot = Some(LegWatch {
                        gen: h.gen,
                        leg,
                        since: tick,
                        best: d,
                    });
                }
            }
        }
    }

    /// How long `h` has held its current leg without progress, and the best
    /// distance it had, when that is at least `NO_PROGRESS_TICKS` and the
    /// unit has not arrived (within `NO_PROGRESS_GAIN` of the waypoint).
    fn stalled(
        &self,
        h: core::unit::UnitHandle,
        r: &core::ai::Route,
        tick: u64,
    ) -> Option<(u64, f32)> {
        let w = self.units.get(h.index as usize).copied().flatten()?;
        (w.gen == h.gen
            && w.leg == (r.next_hub, r.dest_hub)
            && tick - w.since >= NO_PROGRESS_TICKS
            && w.best > NO_PROGRESS_GAIN)
            .then_some((tick - w.since, w.best))
    }
}

/// The census's verdicts, in the order the first that holds is taken (the
/// module header says what each means).
const VERDICTS: [&str; 6] = ["not-walking", "off-graph", "no-arc", "wall", "body", "open"];

/// The stuck census for one map at the end of its round (module header,
/// "The stuck census"). Read-only: nothing here writes to the sim.
fn stuck_census(
    key: &str,
    w: &WalkSim,
    class_names: &[Vec<String>; 2],
    threshold: u16,
    longest: &LongestRun,
    noprog: &NoProgress,
) -> String {
    use core::collision::NO_GROUND;
    use glam::{Vec2, Vec3};
    let mut out = String::new();
    let sim = &w.sim;
    let alive: Vec<_> = sim.alive_units().collect();
    // What the summary needs from each stuck unit: its index, team, position,
    // the hub its leg walks to, and its verdict.
    let mut rows: Vec<(u32, u8, Vec3, u16, &'static str)> = Vec::new();
    let mut flag_under = 0usize;
    let mut flag_dodge = 0usize;
    let mut lines = String::new();
    // The no-progress units: `(index, ticks, best)`.
    let mut stalled: Vec<(u32, u64, f32)> = Vec::new();
    for (h, u) in &alive {
        let Some(r) = u.ai.route else {
            continue;
        };
        let stall = noprog.stalled(*h, &r, sim.tick);
        let stuck = r.stuck_run > threshold;
        if !stuck && stall.is_none() {
            continue;
        }
        if let Some((n, best)) = stall {
            stalled.push((h.index, n, best));
        }
        let pos = u.soldier.pos;
        let team = usize::from(u.team.min(1));
        let class = class_names[team]
            .get(usize::from(u.spec.class))
            .map_or("?", |n| n.as_str());
        let mover = core::nav::Mover::new(core::nav::SOLDIER, u.spec.jet);
        // The leg: the hub it comes FROM is the via-arc's other end.
        let g = sim.nav.as_ref();
        let from = r
            .via_arc
            .and_then(|a| g.and_then(|g| g.arcs.get(usize::from(a))))
            .map(|a| if a.b == r.next_hub { a.a } else { a.b });
        // The hub it walks to AS AUTHORED: that `y` is the editor's pick
        // height, not a floor (BF2 POL1 authors every hub at y 0 under floors
        // 13–33 m up — `NavGraph::locate_on`).
        let next_at = g.and_then(|g| g.hub(r.next_hub)).map_or_else(
            || "-".to_owned(),
            |hb| format!("({:.1},{:.1},{:.1})", hb.pos.x, hb.pos.y, hb.pos.z),
        );
        // The point the leg is walking at right now — `ai::route::sub_point`: the
        // current detour corner at the unit's own y, else the waypoint.
        let (sub, sub_kind) = match r.corner() {
            Some(c) => (Vec3::new(c.x, pos.y, c.y), "corner"),
            None => (r.waypoint, if r.is_last() { "goal" } else { "hub" }),
        };
        let dxz = core::nav::xz_dist(pos, sub);
        let floor_at = |p: Vec3| sim.ground_under(Vec3::new(p.x, pos.y, p.z), false);
        let sub_floor = floor_at(sub);
        // `ai::hub_has_floor` from this unit's level, for the same-level
        // locate the planner re-plans from (`ai::start_hub`).
        let on_floor = |hub: u16| {
            g.and_then(|g| g.hub(hub)).is_some_and(|hb| {
                let at = |y: f32| sim.ground_under(Vec3::new(hb.pos.x, y, hb.pos.z), false);
                let mut y = at(pos.y);
                if y <= NO_GROUND && hb.pos.y > pos.y {
                    y = at(hb.pos.y);
                }
                y > NO_GROUND
            })
        };
        let locus = g.and_then(|g| g.locate_unit_on(pos, mover, &on_floor));
        let replan = g.and_then(|g| g.unit_hub_on(pos, mover, &on_floor));
        let locus_txt = match locus {
            Some(core::nav::Locus::Hub(i)) => format!("hub {i}"),
            Some(core::nav::Locus::Arc(j, t)) => format!("arc {j}@{t:.2}"),
            Some(core::nav::Locus::Nearest(..)) => format!("NEAREST {}", replan.unwrap_or(0)),
            None => "none".to_owned(),
        };
        // The steering's own probe, aimed straight at the sub-point.
        let flat = Vec3::new(sub.x - pos.x, 0.0, sub.z - pos.z);
        let dir = flat.normalize_or_zero();
        let cap = u.capsule();
        let probe = (dir != Vec3::ZERO)
            .then(|| {
                sim.collision
                    .wall_sweep(&cap.stepped(), dir, core::ai::PROBE_DIST)
            })
            .flatten();
        let probe_txt = match probe {
            Some((toi, n)) => format!("WALL toi {toi:.2} n ({:.2},{:.2},{:.2})", n.x, n.y, n.z),
            None if dir == Vec3::ZERO => "at-point".to_owned(),
            None => "clear".to_owned(),
        };
        // A soldier in that probe's path: ahead within PROBE_DIST plus the
        // two radii, laterally inside them, at an overlapping height.
        let body = alive
            .iter()
            .filter(|(oh, _)| oh.index != h.index)
            .filter_map(|(oh, o)| {
                let oc = o.capsule();
                let rs = cap.radius + oc.radius;
                let off = o.soldier.pos - pos;
                let along = off.x * dir.x + off.z * dir.z;
                let lateral = (Vec2::new(off.x, off.z) - Vec2::new(dir.x, dir.z) * along).length();
                (dir != Vec3::ZERO
                    && along > 0.0
                    && along <= core::ai::PROBE_DIST + rs
                    && lateral < rs
                    && off.y.abs() < cap.height.max(oc.height))
                .then_some((along, oh.index))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let body_txt = body.map_or_else(|| "-".to_owned(), |(d, i)| format!("unit {i} @{d:.2}m"));
        let terrain = sim
            .heightfield
            .as_ref()
            .map(|hf| hf.height_at(pos.x, pos.z));
        let cut = sim.collision.is_cut(pos);
        let under = terrain.is_some_and(|t| t - pos.y > 1.0) && !cut;
        let dodging = sim.tick < u.ai.strafe_until;
        flag_under += usize::from(under);
        flag_dodge += usize::from(dodging);
        let not_walking = u.ai.steer == core::ai::SteerPick::Idle;
        let off_graph = matches!(locus, Some(core::nav::Locus::Nearest(..)));
        let no_arc = r.source == core::ai::RouteSource::Straight;
        let holds = [
            not_walking,
            off_graph,
            no_arc,
            probe.is_some(),
            body.is_some(),
            true,
        ];
        let verdict = VERDICTS[holds.iter().position(|&b| b).unwrap_or(VERDICTS.len() - 1)];
        let mut flags: Vec<&str> = Vec::new();
        if under {
            flags.push("under-terrain");
        }
        if dodging {
            flags.push("dodging");
        }
        let objective = sim.commander[team]
            .of(h.index)
            .map_or_else(|| "none".to_owned(), |o| o.label());
        let cp_dist =
            u.ai.target_cp
                .and_then(|c| sim.cps.get(c as usize))
                .map_or_else(
                    || "-".to_owned(),
                    |c| format!("{:.1}", core::nav::xz_dist(pos, c.def.pos)),
                );
        outln!(
            lines,
            "  {} unit {:3} team {} {class} run {} noprog {} pos ({:.1},{:.1},{:.1}) | leg from {} to {} at {next_at} dest {} arc {} [{}] | {sub_kind} dxz {dxz:.1} dy {:+.1} floor {} | locus {locus_txt} replan_hub {} | probe {probe_txt} | ahead_body {body_txt} | terrain {} cut {cut} | steer {} slide {} sidestep {} vel {:.2} {:?} | objective {objective} target_cp {:?} cp_dxz {cp_dist} | => {verdict} [{}]",
            if stuck { "stuck" } else { "noprog" },
            h.index,
            u.team,
            r.stuck_run,
            stall.map_or_else(|| "-".to_owned(), |(n, best)| format!("{n} best {best:.1}")),
            pos.x,
            pos.y,
            pos.z,
            from.map_or_else(|| "-".to_owned(), |f| f.to_string()),
            r.next_hub,
            r.dest_hub,
            r.via_arc.map_or_else(|| "-".to_owned(), |a| a.to_string()),
            r.source.tag(),
            sub.y - pos.y,
            if sub_floor > NO_GROUND {
                format!("{sub_floor:.1}")
            } else {
                "NO_GROUND".to_owned()
            },
            replan.map_or_else(|| "-".to_owned(), |h| h.to_string()),
            terrain.map_or_else(|| "-".to_owned(), |t| format!("{t:.1}")),
            u.ai.steer.label(),
            u.ai.slide,
            r.sidestep,
            u.soldier.vel.length(),
            u.soldier.stance,
            u.ai.target_cp,
            flags.join(" "),
        );
        if stuck {
            rows.push((h.index, u.team, pos, r.next_hub, verdict));
        }
    }

    let n0 = rows.iter().filter(|r| r.1 == 0).count();
    outln!(
        out,
        "census {key}: {} alive units stuck > {threshold} ticks of {} alive (team0 {n0}, team1 {}); longest run {} by unit {} gen {} ({}) at tick {} pos ({:.1},{:.1},{:.1}); nav_stats.stuck_max {}",
        rows.len(),
        alive.len(),
        rows.len() - n0,
        longest.run,
        longest.unit,
        longest.gen,
        longest.doing,
        longest.tick,
        longest.pos.x,
        longest.pos.y,
        longest.pos.z,
        sim.nav_stats.stuck_max,
    );
    outln!(
        out,
        "census {key}: {} alive units on one leg >= {NO_PROGRESS_TICKS} ticks gaining < {NO_PROGRESS_GAIN} m on its waypoint [{}]",
        stalled.len(),
        stalled
            .iter()
            .map(|(i, n, _)| format!("{i}:{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    // Card WHY: the round's failed walks by code (`NavStats::fails`).
    let fails = &sim.nav_stats.fails;
    outln!(
        out,
        "census {key}: failed walks {} [{}]; arcs closed {} shunned {} legs round {} kept {}",
        fails.iter().sum::<u64>(),
        core::ai::why::FAILS
            .iter()
            .zip(fails)
            .map(|(w, n)| format!("{} {n}", w.tag()))
            .collect::<Vec<_>>()
            .join(" "),
        sim.nav_stats.arcs_closed,
        sim.nav_stats.arcs_shunned,
        sim.nav_stats.shun_round,
        sim.nav_stats.shun_kept,
    );
    if rows.is_empty() {
        out.push_str(&lines);
        return out;
    }
    let tally = VERDICTS
        .iter()
        .map(|v| format!("{v} {}", rows.iter().filter(|r| r.4 == *v).count()))
        .collect::<Vec<_>>()
        .join("  ");
    outln!(
        out,
        "  verdicts: {tally}   flags: under-terrain {flag_under}  dodging {flag_dodge}"
    );
    // The hubs their legs walk to, most-shared first (ties: lower hub).
    let mut hubs: Vec<(u16, usize)> = Vec::new();
    for r in &rows {
        match hubs.iter_mut().find(|(h, _)| *h == r.3) {
            Some((_, n)) => *n += 1,
            None => hubs.push((r.3, 1)),
        }
    }
    hubs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    outln!(
        out,
        "  leg next_hub: {}",
        hubs.iter()
            .map(|(h, n)| format!("{h}x{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    // Where they stand: greedy clusters in unit order — a unit joins the first
    // cluster whose FIRST member is within 3 m in XZ and 2 m in y — then the
    // three biggest (ties: the earlier cluster).
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let near = |c: &Vec<usize>| {
            let p = rows[c[0]].2;
            core::nav::xz_dist(p, r.2) <= 3.0 && (p.y - r.2.y).abs() <= 2.0
        };
        match clusters.iter_mut().find(|c| near(c)) {
            Some(c) => c.push(i),
            None => clusters.push(vec![i]),
        }
    }
    let mut order: Vec<usize> = (0..clusters.len()).collect();
    order.sort_by(|&a, &b| clusters[b].len().cmp(&clusters[a].len()).then(a.cmp(&b)));
    for (rank, &c) in order.iter().take(3).enumerate() {
        let p = rows[clusters[c][0]].2;
        outln!(
            out,
            "  position {}: ({:.1},{:.1},{:.1}) x{} units [{}]",
            rank + 1,
            p.x,
            p.y,
            p.z,
            clusters[c].len(),
            clusters[c]
                .iter()
                .map(|&i| rows[i].0.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    out.push_str(&lines);
    out
}

/// What bots did in flyers (`core::ai::flyer_drive`), unless `OPENBF_FLYER_BOTS=0`.
#[derive(Default)]
struct FlyerCensus {
    was: Vec<(core::vehicle::flyer::FlightPhase, bool, bool)>,
    boarded: u32,
    took_off: u32,
    air_ticks: u64,
    low: Option<f32>,
    /// Airborne ticks under the terrain where no cut or hangar opens it.
    buried: u64,
    buried_worst: Option<(f32, glam::Vec3)>,
    high: f32,
    top: f32,
    falls: u32,
    deaths: u32,
    /// Of the deaths: no killer (the world or the map's edge), and of those the ones
    /// counting down outside the boundary on the tick before.
    deaths_world: u32,
    deaths_edge: u32,
    was_leaving: Vec<bool>,
    /// Ticks a craft was counting down outside the boundary.
    leaving: u64,
    /// Sign changes of a saturated pitch stick while flying, the porpoising count,
    /// and each craft's last saturated sign.
    pitch_flips: u32,
    pitch_sign: Vec<f32>,
    /// Summed |vertical speed| while flying, m/s x ticks.
    climb_sum: f64,
    /// Ticks flying with less than 2 m/s, a craft stalled or wedged.
    slow: u64,
    shots: u32,
    kills: u32,
}

impl FlyerCensus {
    fn observe(&mut self, sim: &core::sim::Sim) {
        self.observe_flights(sim);
        // After the events: read for the next tick's deaths.
        self.was_leaving.clear();
        self.was_leaving
            .extend(sim.vehicles.iter().map(|v| v.flyer.leaving.is_some()));
    }

    fn observe_flights(&mut self, sim: &core::sim::Sim) {
        use core::vehicle::flyer::FlightPhase;
        use core::vehicle::VehicleKind;
        // `SWEEP_FLYER_TRACE=<n>`: a line every n ticks (1 means every second).
        let trace = std::env::var("SWEEP_FLYER_TRACE")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|n| if n <= 1 { 30 } else { n });
        self.was
            .resize(sim.vehicles.len(), (FlightPhase::Landed, false, true));
        for (i, v) in sim.vehicles.iter().enumerate() {
            if v.kind != VehicleKind::Flyer {
                continue;
            }
            let (phase, held, alive) = self.was[i];
            let pilot = v.seats.first().is_some_and(|s| s.occupant.is_some());
            self.boarded += u32::from(pilot && !held);
            self.took_off +=
                u32::from(phase == FlightPhase::Landed && v.flyer.phase == FlightPhase::Takeoff);
            self.falls +=
                u32::from(phase != FlightPhase::Falling && v.flyer.phase == FlightPhase::Falling);
            self.deaths += u32::from(alive && !v.alive);
            if !v.is_empty() && trace.is_some_and(|n| sim.tick.is_multiple_of(n)) {
                let over = sim
                    .heightfield
                    .as_ref()
                    .map_or(0.0, |h| v.pos.y - h.height_at(v.pos.x, v.pos.z));
                eprintln!(
                    "flyer {i} t{} {:?} {:?} pos {:.0},{:.1},{:.0} over {over:.1} v {:.1} vy {:.1} in thr {:.2} turn {:.2} pitch {:.2} goal {:?} hp {:.0}",
                    sim.tick as f32 / 30.0,
                    v.flyer.phase,
                    v.flyer.ai.agent,
                    v.pos.x,
                    v.pos.y,
                    v.pos.z,
                    v.vel.length(),
                    v.vel.y,
                    v.input.throttle,
                    v.input.turn,
                    v.input.pitch,
                    v.flyer.ai.goal.map(|g| (g.x as i32, g.y as i32, g.z as i32)),
                    v.health.cur,
                );
            }
            self.pitch_sign.resize(sim.vehicles.len(), 0.0);
            if v.alive && !v.is_empty() && v.flyer.phase == FlightPhase::Flying {
                if v.input.pitch.abs() >= 0.9 {
                    let sign = v.input.pitch.signum();
                    self.pitch_flips += u32::from(self.pitch_sign[i] == -sign);
                    self.pitch_sign[i] = sign;
                }
                self.climb_sum += f64::from(v.vel.y.abs());
                self.slow += u64::from(v.vel.length() < 2.0);
                self.leaving += u64::from(v.flyer.leaving.is_some());
            }
            if v.alive && v.flyer.phase == FlightPhase::Flying {
                self.air_ticks += 1;
                if let Some(h) = sim.heightfield.as_ref() {
                    let over = v.pos.y - h.height_at(v.pos.x, v.pos.z);
                    self.low = Some(self.low.map_or(over, |l| l.min(over)));
                    let surface = glam::Vec3::new(v.pos.x, v.pos.y - over, v.pos.z);
                    if over < 0.0 && !sim.collision.is_cut(surface) {
                        self.buried += 1;
                        if self.buried_worst.is_none_or(|(w, _)| over < w) {
                            self.buried_worst = Some((over, v.pos));
                        }
                    }
                    self.high = self.high.max(over);
                }
                self.top = self.top.max(v.vel.length());
            }
            self.was[i] = (v.flyer.phase, pilot, v.alive);
        }
        let in_flyer = |h: core::unit::UnitHandle| {
            sim.unit(h)
                .and_then(|u| u.mount)
                .and_then(|m| sim.vehicle(m.vehicle))
                .is_some_and(|v| v.kind == VehicleKind::Flyer)
        };
        for ev in &sim.events {
            match *ev {
                core::sim::Event::Shot { unit, .. } if in_flyer(unit) => self.shots += 1,
                core::sim::Event::Kill { shooter, .. } => {
                    self.kills += u32::from(in_flyer(shooter));
                }
                core::sim::Event::VehicleDestroyed {
                    vehicle,
                    killer: None,
                    ..
                } => {
                    let flyer = sim
                        .vehicles
                        .get(vehicle.index as usize)
                        .is_some_and(|v| v.kind == VehicleKind::Flyer);
                    self.deaths_world += u32::from(flyer);
                    self.deaths_edge += u32::from(
                        flyer && self.was_leaving.get(vehicle.index as usize) == Some(&true),
                    );
                }
                _ => {}
            }
        }
    }

    fn line(&self, key: &str) -> String {
        let minutes = (self.air_ticks as f32 / core::TICK_HZ as f32 / 60.0).max(1e-3);
        format!(
            "flyers {key}: boarded {} took-off {} airborne {:.0} s clearance {:.1}..{:.1} m buried {} ticks worst {:?} top {:.1} m/s falls {} deaths {} (world {} edge {}) outside {:.0} s pitch flips/min {:.1} mean |vy| {:.1} slow {:.0} s shots {} kills {}",
            self.boarded,
            self.took_off,
            self.air_ticks as f32 / core::TICK_HZ as f32,
            self.low.unwrap_or(0.0),
            self.high,
            self.buried,
            self.buried_worst,
            self.top,
            self.falls,
            self.deaths,
            self.deaths_world,
            self.deaths_edge,
            self.leaving as f32 / core::TICK_HZ as f32,
            self.pitch_flips as f32 / minutes,
            self.climb_sum / self.air_ticks.max(1) as f64,
            self.slow as f32 / core::TICK_HZ as f32,
            self.shots,
            self.kills,
        )
    }
}

/// Play one map's round and BUILD its output instead of printing it, so the
/// maps can run on worker threads and still print in discovery order.
fn run_map(
    key: &str,
    path: &Path,
    max_ticks: u64,
    seed: u32,
    verbose: bool,
    census: Option<u16>,
) -> MapReport {
    let mut out = String::new();
    let mut scene = match game::assets::load_scene(path) {
        Ok(s) => s,
        Err(e) => {
            outln!(out, "{key:10} LOAD FAILED {e:#}");
            return MapReport {
                text: out,
                census: String::new(),
                stats: None,
                round: None,
            };
        }
    };
    // BF2 maps keep their command posts in a mode sublevel; without this
    // every BF2 map reports zero posts and stalls for the wrong reason.
    scene.apply_mode_layer(mission_mode(key));
    // Shared props (command-post models, crates, turrets) carry their own
    // collision; without them the sweep runs a world the game never has.
    let common_dir = path
        .parent()
        .map(|p| p.join(format!("common_{}", scene.manifest.source_game)));
    match common_dir.filter(|d| d.join("common.ron").exists()) {
        Some(cd) => match game::assets::load_common(&cd) {
            Ok(c) => {
                scene.merge_common(c);
            }
            Err(e) => outln!(out, "{key:10} common assets unreadable: {e:#}"),
        },
        None => outln!(
            out,
            "{key:10} NOTE: no baked common_{} — its props contribute no collision",
            scene.manifest.source_game
        ),
    }

    let mission = mission_for(key, &mut out);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));

    // The mission's ticket pools have to be in place before the sim
    // spawns anyone, so the battle census starts consistent.
    let mut sim = core::sim::Sim::empty(seed);
    // The sweep runs ONE tier per run so the acceptance can compare them:
    // `SWEEP_DIFFICULTY=hard target/release/examples/round_to_outcome …`.
    // Default Normal — the tier every earlier sweep implicitly measured (for
    // part of 2026-10-28 it was called Hard; its numbers never changed).
    sim.difficulty = sweep_difficulty();
    sim.extra_humans = sweep_human_team();
    // Card DPRX: `SWEEP_TEAMS`, a tier per team.
    sim.team_difficulty = cfg().teams;
    // BF1 maps have no 10 m omni in the AI's view cone (RE §9.1).
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    scene.install_water(&mut sim);
    scene.install_damage_regions(&mut sim);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.item_pool = game::roster::item_pool(&scene.manifest.source_game, &scene.manifest.map);
    let defaults = game::conquest_defaults::for_map(key, cfg().era_cw).unwrap_or(
        game::conquest_defaults::Defaults {
            combatants: [16; 2],
            tickets: mission.as_ref().map_or([250; 2], |m| m.tickets()),
        },
    );
    let counts = cfg()
        .combatants
        .map_or(defaults.scaled(cfg().team_pct, 100).combatants, |n| [n; 2]);
    let tickets = cfg().tickets.unwrap_or_else(|| match &mission {
        Some(m) if cfg().mission_tickets => m
            .tickets()
            .map(|n| core::sim::scale_reinforcements(n, cfg().ticket_pct)),
        _ => defaults.scaled(100, cfg().ticket_pct).tickets,
    });
    sim.battle = core::battle::Battle::new(tickets);
    if let Some(m) = &mission {
        m.apply_death_drains(&mut sim.battle);
    }
    sim.team_combatants = Some(counts);
    outln!(
        out,
        "    setup {key} combatants {}/{} tickets {}/{}",
        counts[0],
        counts[1],
        tickets[0],
        tickets[1]
    );
    // The two teams are the MISSION'S ROSTERS (D6): its factions — which
    // `setup_conquest` needs to pick each team's row out of a post's five
    // per-faction ATK/DEF values (PLAN "Phase B addendum") — and its class
    // MIX, every class with its own ODF constants, health and primary
    // weapon. A map with no mission RON, or one whose classes are in no baked
    // side LVL, takes the faction's default list (`game::roster`), and a map
    // whose class data will not load at all falls back to the built-in
    // trooper so the sweep still runs the round.
    let factions = mission
        .as_ref()
        .map(game::mission::MissionDef::factions)
        .unwrap_or_else(|| {
            game::roster::default_factions(&scene.manifest.source_game).map(str::to_owned)
        });
    let classes_dir = path
        .parent()
        .map(|p| p.join("classes"))
        .unwrap_or_else(|| std::path::PathBuf::from("classes"));
    let rosters = game::roster::FactionRoster::pair(
        &classes_dir,
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    );
    let mut tables: [Vec<core::unit::UnitClass>; 2] = [Vec::new(), Vec::new()];
    // `pair` returns its warnings so that they land in THIS map's ordered
    // output instead of on a stderr the worker pool interleaves (two sweeps
    // used to differ only by the order these reached it). They print after
    // the map's result line, verbose or not: a class a mission names and the
    // round could not field is a fact about the round.
    let roster_warnings: Vec<String> = match &rosters {
        Ok((_, warnings)) => warnings.clone(),
        Err(_) => Vec::new(),
    };
    let mut roster_lines: Vec<String> = Vec::new();
    let teams = match &rosters {
        Ok((rs, _)) => {
            // One weapon index per WEAPON, not per class: two classes with
            // the same rifle share it. A `Vec` scan, not a HashMap — the sim
            // never iterates a hash map (PLAN determinism).
            let mut names: Vec<(String, usize)> = Vec::new();
            let mut t = 0usize;
            rs.each_ref().map(|r| {
                for c in &r.classes {
                    let uc = c.register(&mut sim, &mut names);
                    tables[t].push(uc);
                }
                let f = r.fill_class();
                roster_lines.push(format!(
                    "    roster team {t} {}: {} classes, fill {} ({}); {}",
                    r.faction,
                    r.classes.len(),
                    f.class,
                    f.weapon.name,
                    r.classes
                        .iter()
                        .map(|c| format!(
                            "{} {}{}",
                            c.class,
                            c.min,
                            c.max.map_or(String::new(), |m| format!("-{m}"))
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                t += 1;
                game::walk::TeamTemplate::from_roster(
                    r,
                    &tables[t - 1][0],
                    r.classes
                        .iter()
                        .map(|c| {
                            let mut visual = game::walk::ClassVisual::from_roster(c);
                            visual.held_models = c
                                .held
                                .iter()
                                .map(|h| (h.slot, h.geometry.clone()))
                                .collect();
                            visual
                        })
                        .collect(),
                )
            })
        }
        Err(e) => {
            outln!(
                out,
                "{key:10} rosters unavailable: {e:#} (built-in troopers)"
            );
            let t = game::walk::TeamTemplate::default_trooper();
            factions.map(|faction| game::walk::TeamTemplate {
                faction,
                ..t.clone()
            })
        }
    };
    let class_names: [Vec<String>; 2] = match &rosters {
        Ok((rs, _)) => rs
            .each_ref()
            .map(|r| r.classes.iter().map(|c| c.class.clone()).collect()),
        Err(_) => [vec!["trooper".to_owned()], vec!["trooper".to_owned()]],
    };
    let mut nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    // The mission's start-of-round `plnp` blocks, unless
    // `OPENBF_PLNP_MISSION=0` (`MissionDef::apply_arc_blocks`).
    if let Some(m) = &mission {
        let blocked = m.apply_arc_blocks(nav.as_mut());
        if blocked != 0 {
            eprintln!("{key:10} route groups {blocked:#b} blocked (plnp)");
        }
        // ...and its Enable/DisableBarriers (card BNDS).
        m.apply_barriers(nav.as_mut());
    }
    let mut w = WalkSim::with_teams(sim, scene.heightfield(), glam::Vec3::ZERO, teams, nav);
    // The class tables go in BEFORE `setup_conquest`, which is what makes its
    // AI loop use `Sim::spawn_ai` (the mission's mix) instead of the team
    // template. The player is already in as team 0 slot 0 with class 0 —
    // the fill class — and never presses anything.
    w.sim.classes = tables;
    // Model collision — built AFTER apply_mode_layer (see
    // `Scene::collision_world`) and before the conquest setup, so the
    // spawns and the player relocation already stand on the platforms.
    scene.install_collision(&mut w.sim);
    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
        w.sim.fly_ceiling = a.fly_ceiling;
        w.sim.flight_limits = a.flight_limits;
        w.sim.landing_regions = a.landing_regions.clone();
        w.sim.ground_flyer_map = mission.as_ref().is_some_and(|m| m.ground_flyer_map);
    }
    w.add_posts(&layout);
    // The mission's AI settings (card MAIS), its capture bans by post.
    if let Some(m) = &mission {
        m.apply_ai(&mut w.sim);
    }
    // The vehicle spawn pads (card VPAD): the vehicles are in the sim state
    // (and its hash); bots board, drive, fire from seats and leave them.
    // They go in before the opening
    // wave, which spawns at the command walkers they are about to build
    // (card CWSP), as the game's own battle setup orders it.
    let factions = [w.teams[0].faction.clone(), w.teams[1].faction.clone()];
    let vehicle_classes = match game::vehicle_pads::install(
        &mut w.sim,
        &scene.manifest,
        &scene.dir,
        &classes_dir,
        &factions,
    ) {
        Ok(b) => b.classes,
        Err(e) => {
            eprintln!("{key:10} vehicle pads unavailable ({e:#})");
            Vec::new()
        }
    };
    // The proxy counts as a human for difficulty from the opening wave,
    // just as it does after set_proxy. Slot 0 is vacated before this census.
    if slot0_is_bot() {
        w.sim.vacate_player_slot(0);
    }
    let seated = w.sim.extra_humans;
    if let Some((team, _, _)) = cfg().proxy {
        w.sim.extra_humans[team as usize] += 1;
    }
    let opening = if slot0_is_bot() {
        counts
    } else {
        [counts[0].saturating_sub(1), counts[1]]
    };
    w.opening_wave(opening, true);
    let opened = [0u8, 1].map(|team| w.sim.alive_units().filter(|(_, u)| u.team == team).count());
    outln!(out, "    opening {key} alive {}/{}", opened[0], opened[1]);
    w.sim.extra_humans = seated;
    if !slot0_is_bot() {
        if let Some(p) = slot0_at() {
            w.sim.cheats.invincible = true;
            let h = w
                .sim
                .alive_units()
                .find(|(_, u)| u.is_player)
                .map(|(h, _)| h);
            if let Some(u) = h.and_then(|h| w.sim.unit_mut(h)) {
                u.soldier.pos = p;
            }
        }
    }

    // Card DPRX: `SWEEP_PROXY`, the first bot of its team on foot (lowest
    // slot) after the opening wave and slot 0's fill; its tally is the black
    // box's session tally, a human's definitions.
    let proxy_cfg = cfg().proxy;
    let mut proxy: Option<(usize, game::blackbox::session::Tally)> =
        proxy_cfg.as_ref().and_then(|(team, profile, _)| {
            let h = w
                .sim
                .alive_units()
                .find(|(_, u)| {
                    u.team == *team
                        && u.controller == core::unit::Controller::Ai
                        && !u.is_player
                        && u.mount.is_none()
                })
                .map(|(h, _)| h)?;
            w.sim.set_proxy(h, Some(*profile));
            Some((h.index as usize, game::blackbox::session::Tally::default()))
        });
    if proxy_cfg.is_some() && proxy.is_none() {
        outln!(
            out,
            "{key:10} SWEEP_PROXY: no bot on foot to make the proxy"
        );
    }
    // `MATCHLOG_DIR`: the proxy's whole-round match log.
    let mut matchlog = proxy
        .as_ref()
        .zip(std::env::var_os("MATCHLOG_DIR"))
        .map(|((i, _), dir)| {
            let dir = std::path::PathBuf::from(dir);
            let mut meta = game::blackbox::Meta::for_run(
                key,
                "con",
                &scene.manifest.source_game,
                seed,
                sweep_difficulty().label(),
            );
            meta.mission = mission_path(key)
                .file_name()
                .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
            let cw = w
                .teams
                .iter()
                .any(|t| t.faction == "rep" || t.faction == "cis");
            let mut log = game::matchlog::MatchLog::start(
                &meta,
                &w,
                *i as u32,
                "proxy",
                if cw { "cw" } else { "gcw" },
                &dir,
            );
            log.set_path(dir.join(format!("{key}_s{seed}.jsonl")));
            log
        });

    let idle = [PadState::default()];
    let mut t = 0u64;
    let mut longest = LongestRun::default();
    let mut noprog = NoProgress::default();
    let mut deaths = deaths_on().then(Deaths::default);
    let mut shots = std::env::var_os("SWEEP_SHOTS").map(|_| Shots::default());
    let mut snipe = std::env::var_os("SNIPE_CENSUS").map(|_| SnipeCensus::default());
    let mut droid_census = std::env::var_os("DROID_CENSUS").map(|_| DroidCensus::default());
    let mut jet_census = std::env::var_os("JET_CENSUS").map(|_| JetCensus::default());
    // `ANIM_CENSUS` (card ANIML): the soldiers' animators run beside the
    // round, without a renderer, and are counted.
    let mut anim = std::env::var_os("ANIM_CENSUS").map(|_| {
        let vis = game::soldier_vis::SoldierVisuals::load_cpu(
            scene.dir.parent().unwrap_or(&scene.dir),
            &scene.manifest.source_game,
            &w.teams,
        );
        let hands = Hands::new(&w.teams);
        (vis, game::animator::census::Census::default(), hands)
    });
    let mut recycles = (census.is_some() || deaths.is_some()).then(Recycles::default);
    let mut shares = census.is_some().then(Shares::default);
    let mut spawns = std::env::var_os("SPAWN_CENSUS").map(|_| Spawns::default());
    let mut fire = std::env::var_os("HIT_CENSUS").map(|_| Fire::default());
    let mut samples = Samples::new(key, seed);
    let mut bot_samples = BotSamples::new(key, seed);
    let mut heavy = HeavyCensus::new(key, seed);
    let mut blackbox = HeadlessBox::new(key, seed, &scene, &w);
    if let (Some(b), Some((_, profile, label))) = (blackbox.as_mut(), proxy_cfg.as_ref()) {
        b.proxy_note(profile, label);
    }
    let mut obst = Obst::new(key, seed, &scene, &w.sim);
    let mut rim_log = std::env::var_os("RIM_LOG").map(|_| RimLog::default());
    let mut profile = level_profile(key, &w.sim);
    profile.push_str(&locus_csv(key, &w.sim));
    profile.push_str(&floor_grid(key, &w.sim));
    profile.push_str(&positions_line(key, &w.sim));
    if std::env::var_os("NAV_FLOORS").is_some() {
        if let Some(g) = w.sim.nav.as_ref() {
            for i in 0..g.hubs.len() {
                outln!(
                    profile,
                    "nf {key} hub {i} {:?}",
                    w.sim.nav_floor(i as u16).map(|f| (f * 10.0).round() / 10.0)
                );
            }
            for (i, arc) in g.arcs.iter().enumerate() {
                let (a, b) = (
                    w.sim.arc_unwalkable(i as u16, arc.a),
                    w.sim.arc_unwalkable(i as u16, arc.b),
                );
                if a || b {
                    outln!(
                        profile,
                        "nf {key} arc {i} {}-{} dead {} {}",
                        arc.a,
                        arc.b,
                        u8::from(a),
                        u8::from(b)
                    );
                }
            }
            for i in 0..g.hubs.len() {
                if w.sim.hub_trapped(i as u16) {
                    outln!(profile, "nf {key} trapped {i}");
                }
            }
        }
    }
    let trace_unit: Option<(usize, u64, u64)> = std::env::var("TRACE_UNIT").ok().and_then(|v| {
        let p: Vec<u64> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        p.first().map(|&i| {
            (
                i as usize,
                p.get(1).copied().unwrap_or(0),
                p.get(2).copied().unwrap_or(u64::MAX),
            )
        })
    });
    // Card WAI: the walkers a bot pilots at 30, 60, 120 and 180 s, per class
    // (`piloted/alive`; card WAI2 added 30 s and 120 s, the PS2's sample
    // times in re-walker-ai §13).
    const WALKER_CENSUS_AT: [u64; 4] = [900, 1800, 3600, 5400];
    let mut walker_census = [(); 4].map(|_| String::from("-"));
    // ...and the vehicles destroyed, per class: with a killer / with none
    // (a crash, or a blast nobody owns).
    let mut vehicle_losses: std::collections::BTreeMap<u32, [u32; 2]> = Default::default();
    // Card VDROP: the HOVER losses the PS2's count is set against
    // (re-hover-ai §11), per class: rides (a bot takes the pilot seat),
    // then lost with a bot aboard [by a killer, with none, with none at
    // 10 m/s or more = a crash] and lost empty [by a killer, with none at
    // rest, with none moving]. Aboard and speed are the tick before's.
    let mut hover_losses: std::collections::BTreeMap<u32, [u32; 7]> = Default::default();
    let mut hover_before: std::collections::HashMap<core::vehicle::VehicleHandle, (bool, f32)> =
        Default::default();
    // `HOVER_LOSS_LOG=1`: each loss to stderr with the hull's last health
    // drops (tick, amount, speed before).
    let hover_loss_log = std::env::var_os("HOVER_LOSS_LOG").is_some();
    type Drops = (f32, Vec<(u64, f32, f32)>);
    let mut hover_drops: std::collections::HashMap<core::vehicle::VehicleHandle, Drops> =
        Default::default();
    // `MUSIC_LOG=<1|2>`: the battle's music as that Lua team hears it (card
    // AMBM), a line per change to stderr.
    let tickets = [0, 1].map(|i| w.sim.battle.teams[i].tickets());
    let mut music = game::music::HeadlessLog::from_env(path, mission.as_ref(), tickets);
    let mut turret_census = TurretCensus::default();
    let has_turrets = w
        .sim
        .vehicles
        .iter()
        .any(|v| v.kind == core::vehicle::VehicleKind::Turret);
    let mut board_census = std::env::var_os("BOARD_CENSUS").map(|_| BoardCensus::default());
    let exit_watch = std::env::var_os("EXIT_WATCH").is_some();
    let mut exit_prev: std::collections::HashMap<
        usize,
        (u32, core::vehicle::VehicleHandle, u8, bool, u32, f32),
    > = Default::default();
    let mut exit_last: std::collections::HashMap<(u32, u32), (u64, &'static str, bool)> =
        Default::default();
    let mut exit_counts = [0u32; 6];
    let mut exit_floor = [0u32; 2];
    let mut exit_other = [0u32; 2];
    let timing = std::env::var_os("SWEEP_TIMING").is_some();
    let hash_every = std::env::var("SWEEP_HASH_EVERY")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n > 0);
    let mut tick_us = Vec::new();
    let mut flyer_census = FlyerCensus::default();
    while t < max_ticks && w.sim.outcome.is_none() {
        let started = timing.then(std::time::Instant::now);
        w.step_tick(&idle);
        if w.sim.flyer_bots {
            flyer_census.observe(&w.sim);
        }
        if let Some(started) = started {
            tick_us.push(started.elapsed().as_micros() as u64);
        }
        t += 1;
        if hash_every.is_some_and(|n| t.is_multiple_of(n)) {
            outln!(out, "    checkpoint {key} {t} {:016x}", w.sim.hash());
        }
        if let Some((i, tally)) = proxy.as_mut() {
            tally.tick(&w.sim, Some((*i, "proxy")));
        }
        if let Some(m) = matchlog.as_mut() {
            m.tick(&w.sim, &core::sim::PadState::default());
        }
        // Scratch observer (VBAIL local validation, EXIT_WATCH=1, stderr only):
        // every AI hover/walker exit by unit generation + tick + vehicle, and
        // every world death of that unit generation within 90 ticks of it.
        if exit_watch {
            let destroyed: std::collections::HashSet<(u32, u32)> = w
                .sim
                .events
                .iter()
                .filter_map(|ev| match ev {
                    core::sim::Event::VehicleDestroyed { vehicle, .. } => {
                        Some((vehicle.index, vehicle.gen))
                    }
                    _ => None,
                })
                .collect();
            let mut now = std::collections::HashMap::new();
            for (i, u) in w.sim.units.iter().enumerate() {
                if u.controller != core::unit::Controller::Ai {
                    continue;
                }
                if let Some(m) = u.mount {
                    if let Some(v) = w.sim.vehicle(m.vehicle) {
                        if matches!(
                            v.kind,
                            core::vehicle::VehicleKind::Hover | core::vehicle::VehicleKind::Walker
                        ) {
                            now.insert(
                                i,
                                (
                                    u.gen,
                                    m.vehicle,
                                    m.seat,
                                    v.kind == core::vehicle::VehicleKind::Hover,
                                    v.class,
                                    v.health.cur / v.health.max.max(1e-6),
                                ),
                            );
                        }
                    }
                }
            }
            for (&i, &(gen, vh, seat, hover, class, frac)) in &exit_prev {
                let u = &w.sim.units[i];
                let still = u.gen == gen && u.mount.is_some_and(|m| m.vehicle == vh);
                if still {
                    continue;
                }
                let gone = destroyed.contains(&(vh.index, vh.gen)) || w.sim.vehicle(vh).is_none();
                let why = if gone {
                    "destroyed"
                } else if frac < 0.2 {
                    "lowhp"
                } else {
                    "voluntary"
                };
                let k = usize::from(!hover) * 3
                    + match why {
                        "destroyed" => 0,
                        "lowhp" => 1,
                        _ => 2,
                    };
                exit_counts[k] += 1;
                let name = vehicle_classes
                    .get(class as usize)
                    .map_or("?", String::as_str);
                let p = u.soldier.pos;
                eprintln!(
                    "vexit {key} t {t} unit {i} gen {gen} alive {} seat {seat} vehicle {}:{} {} {name} why {why} hp {frac:.3} at ({:.1},{:.1},{:.1})",
                    u.alive && u.gen == gen,
                    vh.index,
                    vh.gen,
                    if hover { "hover" } else { "walker" },
                    p.x,
                    p.y,
                    p.z
                );
                exit_last.insert((i as u32, gen), (t, why, hover));
            }
            exit_prev = now;
            for ev in &w.sim.events {
                if let core::sim::Event::EnvDeath { unit, cause } = ev {
                    if let Some(&(at, why, hover)) = exit_last.get(&(unit.index, unit.gen)) {
                        if t - at <= 90 {
                            let c = format!("{cause:?}");
                            if matches!(cause, core::sim::DeathCause::KillFloor) {
                                exit_floor[usize::from(!hover)] += 1;
                            } else {
                                exit_other[usize::from(!hover)] += 1;
                            }
                            let p = w.sim.units[unit.index as usize].soldier.pos;
                            eprintln!(
                                "vexitdeath {key} t {t} unit {} gen {} dt {} cause {c} after {why} {} at ({:.1},{:.1},{:.1})",
                                unit.index,
                                unit.gen,
                                t - at,
                                if hover { "hover" } else { "walker" },
                                p.x,
                                p.y,
                                p.z
                            );
                        }
                    }
                }
            }
            if !(t < max_ticks && w.sim.outcome.is_none()) {
                eprintln!(
                    "vexitsum {key}: hover destroyed {} lowhp {} voluntary {} floor90 {} other90 {} | walker destroyed {} lowhp {} voluntary {} floor90 {} other90 {}",
                    exit_counts[0], exit_counts[1], exit_counts[2], exit_floor[0], exit_other[0],
                    exit_counts[3], exit_counts[4], exit_counts[5], exit_floor[1], exit_other[1]
                );
            }
        }
        if let Some(b) = board_census.as_mut() {
            b.observe(&w.sim, key, &vehicle_classes);
        }
        if verbose && has_turrets {
            turret_census.observe(&w.sim);
        }
        if let Some(m) = music.as_mut() {
            m.tick(&w.sim);
            if w.sim.outcome.is_some() {
                m.finish(&w.sim);
            }
        }
        if verbose || hover_loss_log {
            for ev in &w.sim.events {
                if let core::sim::Event::VehicleDestroyed {
                    vehicle,
                    class,
                    killer,
                    pos,
                    ..
                } = ev
                {
                    if let Some(&(bot, speed)) = hover_before.get(vehicle) {
                        let e = hover_losses.entry(*class).or_default();
                        let k = match (bot, killer.is_some()) {
                            (true, true) => 1,
                            (true, false) => 2,
                            (false, true) => 4,
                            (false, false) if speed < 1.0 => 5,
                            (false, false) => 6,
                        };
                        e[k] += 1;
                        e[3] += u32::from(k == 2 && speed >= 10.0);
                        if hover_loss_log {
                            let name = vehicle_classes
                                .get(*class as usize)
                                .map_or("?", String::as_str);
                            let (hp, drops) = hover_drops.remove(vehicle).unwrap_or_default();
                            let tail: Vec<String> = drops
                                .iter()
                                .rev()
                                .take(6)
                                .rev()
                                .map(|(at, d, v)| format!("{at}:-{d:.1}@{v:.1}"))
                                .collect();
                            eprintln!(
                            "hoverloss {key} t {t} {name} at ({:.0}, {:.0}, {:.0}) bot {bot} killer {} speed {speed:.1} last hp {hp:.1} drops [{}]",
                            pos.x,
                            pos.y,
                            pos.z,
                            killer.is_some(),
                            tail.join(" ")
                        );
                        }
                    }
                }
            }
            let mut now = std::collections::HashMap::new();
            for (vh, v) in w.sim.alive_vehicles() {
                if v.kind != core::vehicle::VehicleKind::Hover {
                    continue;
                }
                let bot = v
                    .seats
                    .first()
                    .filter(|s| s.kind == core::vehicle::SeatKind::Pilot)
                    .and_then(|s| s.occupant)
                    .and_then(|o| w.sim.unit(o))
                    .is_some_and(|u| u.controller == core::unit::Controller::Ai);
                if bot && !hover_before.get(&vh).is_some_and(|b| b.0) {
                    hover_losses.entry(v.class).or_default()[0] += 1;
                }
                now.insert(vh, (bot, v.vel.length()));
                if hover_loss_log {
                    let e = hover_drops.entry(vh).or_insert((v.health.cur, Vec::new()));
                    if v.health.cur < e.0 {
                        let before = hover_before.get(&vh).map_or(0.0, |b| b.1);
                        e.1.push((t, e.0 - v.health.cur, before));
                    }
                    e.0 = v.health.cur;
                }
            }
            hover_before = now;
        }
        if verbose {
            if let Some(k) = WALKER_CENSUS_AT.iter().position(|&at| at == t) {
                walker_census[k] = walker_pilots(&w.sim, &vehicle_classes);
            }
            for ev in &w.sim.events {
                if let core::sim::Event::VehicleDestroyed { class, killer, .. } = ev {
                    let e = vehicle_losses.entry(*class).or_default();
                    e[usize::from(killer.is_none())] += 1;
                }
            }
        }
        if let Some(s) = spawns.as_mut() {
            s.observe(&w.sim, &vehicle_classes, t);
        }
        if let Some(s) = samples.as_mut() {
            s.observe(&w.sim, t);
        }
        if let Some(s) = bot_samples.as_mut() {
            s.observe(&w.sim, t);
        }
        if let Some(h) = heavy.as_mut() {
            h.observe(&w.sim, &vehicle_classes, t);
        }
        if let Some(b) = blackbox.as_mut() {
            b.observe(&w, t);
        }
        if let Some(o) = obst.as_mut() {
            o.observe(&w.sim, t);
        }
        // `TRACE_UNIT=<index>[,<from tick>,<to tick>]`: that unit every 10 ticks, to stderr.
        if let Some(spec) = trace_unit.as_ref() {
            if t.is_multiple_of(10) && t >= spec.1 && t <= spec.2 {
                if let Some(u) = w.sim.units.get(spec.0).filter(|u| u.alive) {
                    let r = u.ai.route;
                    eprintln!(
                        "trace {t} pos ({:.1},{:.2},{:.1}) vel {:.1} gr {} {:?} steer {} slide {} leg {:?}->{:?} wp {:?} corner {:?} sidestep {} stuck {} lost {:?} tgt {} lvl {:?}",
                        u.soldier.pos.x, u.soldier.pos.y, u.soldier.pos.z,
                        glam::Vec2::new(u.soldier.vel.x, u.soldier.vel.z).length(),
                        u.soldier.grounded as u8, u.soldier.stance, u.ai.steer.label(), u.ai.slide,
                        r.and_then(|r| r.via_arc), r.map(|r| r.next_hub),
                        r.map(|r| (r.waypoint.x.round(), r.waypoint.y.round(), r.waypoint.z.round())),
                        r.and_then(|r| r.corner()), r.map_or(0, |r| r.sidestep), r.map_or(0, |r| r.stuck_run),
                        u.ai.lost.map(|l| (l.confirmed, l.tries)), u.ai.target.is_some() as u8,
                        w.sim.level_at(u.soldier.pos),
                    );
                    // A rider: its vehicle (card STEND).
                    if let Some(v) = u.mount.and_then(|m| w.sim.vehicle(m.vehicle)) {
                        eprintln!(
                            "trace {t} rides {} {:?} at ({:.1},{:.2},{:.1}) seat {}",
                            vehicle_classes
                                .get(v.class as usize)
                                .map_or("?", |c| c.as_str()),
                            v.kind,
                            v.pos.x,
                            v.pos.y,
                            v.pos.z,
                            u.mount.map_or(0, |m| m.seat),
                        );
                    }
                }
            }
        }
        if census.is_some() {
            longest.observe(&w.sim, t);
            noprog.observe(&w.sim, w.sim.tick);
        }
        if let Some(d) = deaths.as_mut() {
            d.observe(&w.sim);
        }
        if let Some(s) = shots.as_mut() {
            s.observe(&w.sim);
        }
        if let Some(s) = snipe.as_mut() {
            s.observe(&w.sim);
        }
        if let Some(d) = droid_census.as_mut() {
            d.observe(&w.sim);
        }
        if let Some(j) = jet_census.as_mut() {
            j.observe(&w.sim);
        }
        if let Some((vis, c, h)) = anim.as_mut() {
            vis.update_with(&w, core::TICK_DT, Some(c));
            h.observe(&w.sim);
        }
        if let Some(r) = recycles.as_mut() {
            r.observe(key, seed, &w.sim);
        }
        if let Some(s) = shares.as_mut() {
            s.observe(&w.sim, t);
        }
        if let Some(f) = fire.as_mut() {
            f.observe(&w.sim);
        }
        if let Some(r) = rim_log.as_mut() {
            r.observe(key, &w.sim, t);
        }
    }

    if let Some(s) = samples {
        s.finish();
    }
    if let Some(s) = bot_samples {
        s.finish();
    }
    if let Some(h) = heavy {
        h.finish();
    }
    if let Some(b) = blackbox {
        b.finish(key);
    }
    if let Some(o) = obst {
        o.finish();
    }

    if !tick_us.is_empty() {
        tick_us.sort_unstable();
        outln!(
            out,
            "    timing {key} median_us {} p99_us {} max_us {}",
            tick_us[tick_us.len() / 2],
            tick_us[tick_us.len() * 99 / 100],
            tick_us[tick_us.len() - 1]
        );
    }
    let alive: Vec<_> = w.sim.alive_units().collect();
    let (mut ymin, mut ymax) = (f32::MAX, f32::MIN);
    for (_, u) in &alive {
        ymin = ymin.min(u.soldier.pos.y);
        ymax = ymax.max(u.soldier.pos.y);
    }
    let n0 = alive.iter().filter(|(_, u)| u.team == 0).count();
    let (a, b) = (
        w.sim.battle.teams[0].tickets(),
        w.sim.battle.teams[1].tickets(),
    );
    let tail = format!(
        "tickets {a}/{b}  alive {n0}/{}  y {ymin:.0}..{ymax:.0}  cps {}  stuck_max {}  hash {:016x}",
        alive.len() - n0,
        layout.cps.len(),
        w.sim.nav_stats.stuck_max,
        w.sim.hash()
    );
    match w.sim.outcome {
        Some(win) => outln!(
            out,
            "{key:10} team {win} wins at tick {t} ({:.0} s)  {tail}",
            t as f32 / 30.0
        ),
        None => outln!(out, "{key:10} NO OUTCOME in {t} ticks  {tail}"),
    }
    if w.sim.flyer_bots {
        outln!(out, "{}", flyer_census.line(key));
    }
    for w in &roster_warnings {
        outln!(out, "    {w}");
    }
    // `ROUND_VERBOSE=1`: why a round did not finish — every post's owner
    // and capture volume, and every alive unit with what it is doing.
    if verbose {
        match &applied {
            Some(a) => outln!(
                out,
                "    mission {}: posts kept {} (roster dropped {}, killed {}), {} re-owned, death regions {} found / {} missing {:?}",
                mission.as_ref().map_or("", |m| m.source.as_str()),
                a.kept,
                a.dropped_by_roster,
                a.killed,
                a.reassigned,
                a.death_regions.len(),
                a.missing_death_regions.len(),
                a.missing_death_regions
            ),
            None => outln!(out, "    mission: none found for {key}"),
        }
        // The planning graph, and what the follower had to fall back on.
        match &w.sim.nav {
            Some(g) => {
                let (jump, jet) = g.arc_flag_census(core::nav::SOLDIER);
                outln!(
                    out,
                    "    nav: {} hubs, {} arcs, {} barriers, {} hints; soldier arcs crossing a soldier barrier {}; fallbacks bfs {} straight {}; detoured legs {}; blocked-forward ticks {}; stuck_max {} ticks",
                    g.hubs.len(),
                    g.arcs.len(),
                    g.barriers.len(),
                    g.hints.len(),
                    g.arcs_crossing_barriers(core::nav::SOLDIER),
                    w.sim.nav_stats.bfs_fallbacks,
                    w.sim.nav_stats.straight_fallbacks,
                    w.sim.nav_stats.detoured_legs,
                    w.sim.nav_stats.blocked_ticks,
                    w.sim.nav_stats.stuck_max,
                );
                outln!(
                    out,
                    "    nav A.2: jump_excluded {jet} jet_jump arcs (jump arcs kept {jump}); level_rejected legs {} (unrouted {}); sweep_hits {} unit-ticks",
                    w.sim.nav_stats.level_rejected,
                    w.sim.nav_stats.level_unrouted,
                    w.sim.nav_stats.sweep_hits,
                );
            }
            None => outln!(
                out,
                "    nav: no planning graph — the AI walks straight at its post"
            ),
        }
        // Phase B: what the commander did. `attack_objs` is how many DISTINCT
        // posts each team ever attacked over the round — the design's
        // route-diversity acceptance — with the winning team's marked `*`.
        let used = w.sim.nav_stats.attack_used;
        let mark = |t: usize| match w.sim.outcome {
            Some(x) if usize::from(x) == t => "*",
            _ => "",
        };
        let names = |bits: u64| -> String {
            (0..64)
                .filter(|b| bits & (1u64 << b) != 0)
                .map(|b| format!("cp{b}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        outln!(
            out,
            "    commander: plans {}  assignment_changes {}  attack_objs team0 {}{} team1 {}{}  unroutable {} unit-ticks  unreachable_dwell_max {} ticks",
            w.sim.nav_stats.plans,
            w.sim.nav_stats.assignment_changes,
            used[0].count_ones(),
            mark(0),
            used[1].count_ones(),
            mark(1),
            w.sim.nav_stats.unroutable_objectives,
            w.sim.nav_stats.unreachable_dwell_max,
        );
        // Phase D: what the squad-command layer did. The sweep's player never
        // presses anything, so `commands 0` on every map is the acceptance —
        // a non-zero count here means a command-free round issued one.
        outln!(
            out,
            "    squad: commands {}  releases {}",
            w.sim.nav_stats.commands,
            w.sim.nav_stats.releases,
        );
        // Card VAI: the bots' vehicles — only on a map that has any.
        if !w.sim.vehicles.is_empty() {
            outln!(
                out,
                "    vehicles: slots {}  ai_claims {} (given up {}, lost {})  ai_boards {}  ai_gunners {}  ai_dismounts {}  ai_drive_ticks {}  seat_shots {}",
                w.sim.vehicles.len(),
                w.sim.nav_stats.ai_claims,
                w.sim.nav_stats.ai_claim_giveups,
                w.sim.nav_stats.ai_claim_lost,
                w.sim.nav_stats.ai_boards,
                w.sim.nav_stats.ai_gunners,
                w.sim.nav_stats.ai_dismounts,
                w.sim.nav_stats.ai_drive_ticks,
                w.sim.nav_stats.seat_shots,
            );
            let s = &w.sim.nav_stats;
            outln!(
                out,
                "    VBAIL: low-health driver rides {} bailed {} ({:.2}%) AI occupants {}",
                s.ai_bail_exposed,
                s.ai_bail_drivers,
                100.0 * s.ai_bail_drivers as f64 / s.ai_bail_exposed.max(1) as f64,
                s.ai_bail_occupants
            );
            // Card TURR: bots in building turrets.
            if let Some(l) = turret_census.line(&w.sim, &vehicle_classes) {
                outln!(out, "{l}");
            }
            // Card WAI: the walkers bots pilot, and how far they take them.
            let walkers = w
                .sim
                .vehicles
                .iter()
                .filter(|v| v.kind == core::vehicle::VehicleKind::Walker)
                .count();
            if !vehicle_losses.is_empty() {
                let list: Vec<String> = vehicle_losses
                    .iter()
                    .map(|(c, n)| {
                        let name = vehicle_classes.get(*c as usize).map_or("?", String::as_str);
                        format!("{name} {}/{}", n[0], n[1])
                    })
                    .collect();
                outln!(
                    out,
                    "    vehicles destroyed (killed / unowned): {}",
                    list.join(", ")
                );
            }
            if !hover_losses.is_empty() {
                let list: Vec<String> = hover_losses
                    .iter()
                    .map(|(c, n)| {
                        let name = vehicle_classes.get(*c as usize).map_or("?", String::as_str);
                        format!(
                            "{name} rides {} rider {}/{} crash {} empty {}/{}/{}",
                            n[0], n[1], n[2], n[3], n[4], n[5], n[6]
                        )
                    })
                    .collect();
                outln!(
                    out,
                    "    hovers lost (rider: killed / unowned, crash = unowned at 10 m/s or more; empty: killed / unowned at rest / moving): {}",
                    list.join(", ")
                );
            }
            let s = &w.sim.nav_stats;
            if walkers > 0 || s.ai_walker_rides > 0 {
                outln!(
                    out,
                    "    walkers: ai_piloted at 30 s {}  at 60 s {}  at 120 s {}  at 180 s {}  rides {}  driven {:.0} m ({:.0} m per ride)",
                    walker_census[0],
                    walker_census[1],
                    walker_census[2],
                    walker_census[3],
                    s.ai_walker_rides,
                    s.ai_walker_cm as f32 / 100.0,
                    s.ai_walker_cm as f32 / 100.0 / s.ai_walker_rides.max(1) as f32,
                );
            }
        }
        // Phase C: what the AI PERCEIVED over the round. `unperceived` is the
        // fairness rule of design decision 2 and must be 0 on every map — a
        // non-zero count is a bug, not a tuning question.
        let hits = w.sim.nav_stats.ai_hits;
        let ai_shots = w.sim.nav_stats.ai_shots;
        outln!(
            out,
            "    perception: ai_shots {}  unperceived {}  carry {}  hits {}  hit_rate {:.2}  contacts_max {}  heard {}",
            ai_shots,
            w.sim.nav_stats.unperceived_shots,
            w.sim.nav_stats.carry_shots,
            hits,
            if ai_shots == 0 {
                0.0
            } else {
                hits as f32 / ai_shots as f32
            },
            w.sim.nav_stats.contacts_max,
            w.sim.nav_stats.heard_contacts,
        );
        // Card FFLN: who the bots' fire lands on. A team kill is a kill of a
        // unit of the killer's own team (not himself); `line_held` is the
        // bot-seconds a shot waited for a friend to leave the line of fire.
        let s = &w.sim.nav_stats;
        outln!(
            out,
            "    friendly fire: kills {}  team_kills {} ({:.1} %)  self {}  bolt_hits {}  friend_hits {} ({:.1} %)  line_held {:.0} s  blast_held {}",
            s.unit_kills,
            s.team_kills,
            100.0 * s.team_kills as f32 / s.unit_kills.max(1) as f32,
            s.self_kills,
            s.bolt_hits,
            s.friend_hits,
            100.0 * s.friend_hits as f32 / s.bolt_hits.max(1) as f32,
            s.line_held as f32 / core::TICK_HZ as f32,
            s.blast_holds,
        );
        outln!(
            out,
            "    commander attacked: team0 [{}] team1 [{}]",
            names(used[0]),
            names(used[1])
        );
        for l in &roster_lines {
            outln!(out, "{l}");
        }
        // The MIX the round actually fielded: every unit ever built this
        // round (alive and dead alike) by the class it came out of.
        for (t, names) in class_names.iter().enumerate() {
            let mix = names
                .iter()
                .enumerate()
                .map(|(c, n)| {
                    let k = w
                        .sim
                        .units
                        .iter()
                        .filter(|u| {
                            usize::from(u.team.min(1)) == t && usize::from(u.spec.class) == c
                        })
                        .count();
                    format!("{n} {k}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            outln!(out, "    mix team{t}: {mix}");
        }
        // Did the collision world actually find the platforms? Compare
        // every spawn node's authored y with the ground the sim resolves
        // under it; a node more than a metre off is a node whose floor we
        // did not find (or one authored in mid-air).
        let mut off: Vec<(String, usize, f32, f32)> = Vec::new();
        let mut nodes = 0usize;
        for cp in &w.sim.cps {
            for (k, (pos, _)) in cp.def.spawn_nodes.iter().enumerate() {
                nodes += 1;
                let g = w.sim.ground_under(*pos, false);
                if (g - pos.y).abs() > 1.0 {
                    off.push((cp.def.name.clone(), k, pos.y, g));
                }
            }
        }
        outln!(
            out,
            "    spawn nodes {nodes}, {} more than 1 m off their authored y  (bodies {}, cutters {})",
            off.len(),
            w.sim.collision.bodies().len(),
            w.sim.collision.cutters.len()
        );
        // Worst five by |authored − resolved|, ordered by that magnitude
        // with the node's own order breaking ties (no float sort key).
        let mut worst: Vec<usize> = (0..off.len()).collect();
        worst.sort_by(|&i, &j| {
            let d = |k: usize| (off[k].2 - off[k].3).abs();
            d(j).total_cmp(&d(i)).then(i.cmp(&j))
        });
        for &i in worst.iter().take(5) {
            let (name, k, authored, ground) = &off[i];
            outln!(
                out,
                "      {name} {k} authored_y {authored:.1} ground_y {ground:.1}"
            );
        }
        for (i, cp) in w.sim.cps.iter().enumerate() {
            let r = &cp.def.capture;
            outln!(
                out,
                "    cp{i} {:12} owner {:?} pos {:.0} capture {:?} centre {:.0} size {:.1} w {:.1}  meters cap {:.1}/{:.1} neu {:.1}/{:.1}",
                cp.def.name, cp.owner, cp.def.pos, r.shape, r.center, r.size, r.w,
                cp.capture, cp.def.capture_time, cp.neutral, cp.def.neutralize_time
            );
        }
        for (h, u) in &alive {
            let target = u.ai.target_cp.and_then(|c| w.sim.cps.get(c as usize));
            let dist = target.map(|c| (c.def.pos - u.soldier.pos).length());
            let inside = target.map(|c| c.def.capture.contains(u.soldier.pos));
            // Terrain height under the unit and whether that point is
            // inside a terrain cutter: a unit whose feet are BELOW an
            // uncut heightfield is standing where the terrain LOS/bolt
            // rays think it is underground.
            let terrain = w
                .sim
                .heightfield
                .as_ref()
                .map(|hf| hf.height_at(u.soldier.pos.x, u.soldier.pos.z));
            let cut = w.sim.collision.is_cut(u.soldier.pos);
            // Nearest enemy and whether the line of sight the AI uses
            // (`core::perception::los`) is clear to it — separates "cannot
            // see anyone" from "will not engage". This is the OMNISCIENT
            // view; what the unit actually perceives is the `target` column
            // beside it (Phase C).
            let eye = u.muzzle();
            let nearest = alive
                .iter()
                .filter(|(_, o)| o.team != u.team)
                .map(|(oh, o)| {
                    (
                        (o.capsule().center() - eye).length(),
                        *oh,
                        o.capsule().center(),
                    )
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let los = nearest.map(|(d, oh, c)| {
                // BOTH tests `core::perception::los` runs — the terrain
                // sheet (with cutters), then the world's `coll`/`prim`
                // bodies — reported apart, so the column says not just that
                // the AI is blind but WHICH of the two is blinding it. They
                // are the AI's OWN halves (brief D5): this column cannot
                // drift from what the AI sees again.
                let terrain = !core::perception::los_terrain(&w.sim, eye, c);
                let world = !core::perception::los_world(&w.sim, eye, c);
                let how = match (terrain, world) {
                    (false, false) => "clear",
                    (true, false) => "BLOCKED terrain",
                    (false, true) => "BLOCKED world",
                    (true, true) => "BLOCKED both",
                };
                format!("{}@{d:.1}m {how}", oh.index)
            });
            // Where the unit sits on the planning graph, and the leg it
            // is walking: `route <dest>-><next> [table|bfs|straight]`.
            let locus = w.sim.nav.as_ref().map_or_else(
                || "-".to_owned(),
                |g| match g.locate_unit_on(u.soldier.pos, core::nav::Mover::SOLDIER, &|_| true) {
                    Some(core::nav::Locus::Hub(i)) => format!("Hub {i}"),
                    Some(core::nav::Locus::Arc(j, t)) => format!("Arc {j}@{t:.2}"),
                    Some(core::nav::Locus::Nearest(j, t)) => format!("Nearest arc {j}@{t:.2}"),
                    None => "none".to_owned(),
                },
            );
            let route = u.ai.route.map_or_else(
                || "none".to_owned(),
                |r| {
                    format!(
                        "{}->{}{} [{}]{}",
                        r.dest_hub,
                        r.next_hub,
                        r.via_arc.map_or(String::new(), |a| format!(" via {a}")),
                        r.source.tag(),
                        if r.is_last() { " last" } else { "" }
                    )
                },
            );
            let stuck = u.ai.route.map_or(0, |r| r.stuck_run);
            // Phase A.2: the WAYPOINT'S GROUND VERDICT, asked the way the
            // follower asks it — at this unit's own y — and the push the
            // wall pass actually applied to it last tick.
            let verdict = u.ai.route.map_or_else(
                || "-".to_owned(),
                |r| {
                    let at = |y: f32| {
                        w.sim
                            .ground_under(glam::Vec3::new(r.waypoint.x, y, r.waypoint.z), false)
                    };
                    let mine = at(u.soldier.pos.y);
                    if mine > core::collision::NO_GROUND {
                        return format!("floor {mine:.1}");
                    }
                    // The follower's fallback: the ground at the hub's
                    // AUTHORED height, when that is above the unit.
                    let authored = w
                        .sim
                        .nav
                        .as_ref()
                        .and_then(|g| g.hub(r.next_hub))
                        .map(|h| h.pos.y)
                        .filter(|&y| y > u.soldier.pos.y)
                        .map(at);
                    match authored {
                        Some(y) if y > core::collision::NO_GROUND => {
                            format!("floor {y:.1} (authored)")
                        }
                        _ => "NO_GROUND->rejected".to_owned(),
                    }
                },
            );
            let push = u.wall_push.length();
            // Phase B: the objective the commander gave this unit, the squad
            // (the objective's own key) it shares with a neighbour, and its
            // dwell counter.
            let objective = w.sim.commander[usize::from(u.team).min(1)]
                .of(h.index)
                .map_or_else(|| "none".to_owned(), |o| o.label());
            let squad =
                u.ai.squad
                    .map_or_else(|| "-".to_owned(), |k| format!("{k:08x}"));
            let dwell = u.ai.unreachable_dwell;
            // Phase A.1: what the steering picked last tick, and the
            // sub-waypoint it is aiming at (a detour corner, else the
            // leg's own waypoint).
            let steer = u.ai.steer.label();
            let sub = u.ai.route.map_or_else(
                || "-".to_owned(),
                |r| match r.corner() {
                    Some(c) => format!(
                        "corner {}/{} ({:.1},{:.1}) {:.1}m",
                        r.detour_at + 1,
                        r.detour_len,
                        c.x,
                        c.y,
                        (glam::Vec2::new(u.soldier.pos.x, u.soldier.pos.z) - c).length()
                    ),
                    None => format!(
                        "waypoint {:.0} {:.1}m",
                        r.waypoint,
                        core::nav::xz_dist(u.soldier.pos, r.waypoint)
                    ),
                },
            );
            // What this unit PERCEIVES: its target, how it knows about it and
            // how stale that is, how full its contact memory is, and where it
            // is in its burst (D15; `p` = pausing, D11).
            let burst = if w.sim.tick < u.ai.pause_until {
                format!("p{}", u.ai.pause_until - w.sim.tick)
            } else {
                u.ai.burst.to_string()
            };
            // D13/D15: the stance this unit DREW from its nearest authored
            // hint's allowed-stance mask (`-` when no hint asks for one), so
            // the census in the report can be read against what the units
            // actually did.
            let stance_want = match core::commander::stance_of_code(u.ai.want_stance) {
                Some(s) => format!("{s:?}"),
                None => "-".to_string(),
            };
            // D13: the squad order this unit carries, `-` for none.
            let cmd = match u.command {
                Some(c) => c.kind.label(),
                None => "-".to_string(),
            };
            let target = match u.ai.target {
                Some(t) => {
                    let c = core::perception::contact_of(&u.ai, t);
                    let d = t
                        .unit()
                        .and_then(|t| w.sim.unit(t))
                        .map_or_else(|| c.map(|c| c.last_pos), |o| Some(o.capsule().center()))
                        .map_or(f32::NAN, |p| (p - eye).length());
                    let how = match c {
                        Some(c) if c.seen_now => "seen",
                        Some(c) if c.heard => "heard",
                        Some(_) => "memory",
                        None => "GONE",
                    };
                    let age = c.map_or(f32::NAN, |c| c.age_s(w.sim.tick));
                    format!(
                        "{:?}@{d:.0}m {how} age {age:.1}  contacts {}  err {:.2}  burst {}  stance_want {}  cmd {cmd}",
                        t,
                        u.ai.contacts.iter().flatten().count(),
                        u.aim.err,
                        burst,
                        stance_want,
                    )
                }
                None => format!(
                    "none  contacts {}  err {:.2}  burst {}  stance_want {}  cmd {cmd}",
                    u.ai.contacts.iter().flatten().count(),
                    u.aim.err,
                    burst,
                    stance_want,
                ),
            };
            outln!(
                out,
                "    unit {:3} team {} class {} {:?} player {} pos {:.1} terrain {:?} cut {} stance {:?} vel {:.2} objective {objective} squad {squad} dwell {dwell} target_cp {:?} dist {:?} inside {:?} target {} nearest {:?} locus {locus} route {route} steer {steer} sub {sub} ground {verdict} push {push:.3} stuck {stuck}",
                h.index,
                u.team,
                class_names[usize::from(u.team.min(1))]
                    .get(usize::from(u.spec.class))
                    .map_or("?", |n| n.as_str()),
                u.controller,
                u.is_player,
                u.soldier.pos, terrain.map(|t| (t * 10.0).round() / 10.0), cut, u.soldier.stance,
                u.soldier.vel.length(), u.ai.target_cp, dist, inside, target, los
            );
        }
    }
    let mut census = census.map_or_else(String::new, |threshold| {
        stuck_census(key, &w, &class_names, threshold, &longest, &noprog)
    });
    if let Some(s) = &shares {
        census.push_str(&s.line(key));
        // Card TPOS: B12's ceiling - the Tactician's queries in which a
        // candidate with cover from the target passed its filters.
        let ns = &w.sim.nav_stats;
        census.push_str(&format!(
            "tactician {key}: queries {} ceiling {} ({:.1} %) | {} | ceiling-without-covered-pick {}\n",
            ns.tpos_queries,
            ns.tpos_ceiling,
            100.0 * ns.tpos_ceiling as f64 / ns.tpos_queries.max(1) as f64,
            core::ai::tactic::EV_NAMES
                .iter()
                .zip(ns.tpos_ev)
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(" "),
            ns.tpos_ev[16],
        ));
        census.push_str(&format!(
            "tfunnel {key}: {}\n",
            [
                "within",
                "storey",
                "nearest32",
                "x-band",
                "x-taken",
                "x-body",
                "x-line",
                "x-backward",
                "x-hazard",
                "x-barrier",
                "passed",
                "x-drop",
                "x-ray",
                "scored-no-pick",
                "x-closing"
            ]
            .iter()
            .zip(ns.tpos_funnel)
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(" ")
        ));
        // Card HYST: positions replaced on one think, kept by the margin,
        // and the early thinks with the behaviour changes they made.
        let wake_why: Vec<String> = ns
            .wake_why
            .iter()
            .enumerate()
            .filter(|(_, n)| **n != 0)
            .map(|(i, n)| {
                let tag = core::ai::why::CHANGES.get(i).map_or("?", |w| w.tag());
                format!("{tag} {n}")
            })
            .collect();
        census.push_str(&format!(
            "hyst {key}: flips {} kept {} | wakes hit {} near {} seen {} | wake changes [{}]\n",
            ns.tpos_flips,
            ns.tpos_kept,
            ns.wakes[0],
            ns.wakes[1],
            ns.wakes[2],
            wake_why.join(" "),
        ));
    }
    if let Some(f) = &fire {
        census.push_str(&f.line(key));
    }
    if let Some(d) = &deaths {
        census.push_str(&d.line(key, &w.sim));
    }
    if let Some(s) = &spawns {
        census.push_str(&s.line(key));
    }
    if let Some(s) = shots.as_mut() {
        census.push_str(&s.line(key, &w.sim));
    }
    if let Some(s) = snipe.as_mut() {
        census.push_str(&s.line(key));
    }
    if let Some(d) = &droid_census {
        census.push_str(&d.line(key));
    }
    if let Some(j) = &jet_census {
        census.push_str(&j.line(key));
    }
    if let Some((_, c, h)) = &anim {
        census.push_str(&c.report(key));
        census.push_str(&h.report(key));
    }
    if let Some(r) = &recycles {
        census.push_str(&r.0);
    }
    census.push_str(&profile);
    if let Some(r) = &rim_log {
        census.push_str(&r.text);
    }
    if let Some(m) = matchlog.as_mut() {
        let why = if w.sim.outcome.is_some() {
            "outcome"
        } else {
            "quit"
        };
        if let Some(p) = m.write(&w.sim, why) {
            eprintln!("matchlog: wrote {}", p.display());
        }
    }
    let round = RoundResult {
        winner: w.sim.outcome,
        margin: i64::from(w.sim.battle.teams[1].tickets())
            - i64::from(w.sim.battle.teams[0].tickets()),
        proxy: proxy.map(|(i, tally)| (w.sim.units[i].team, tally.out)),
    };
    census.push_str(&round.line(
        key,
        seed,
        (
            w.sim.battle.teams[0].tickets(),
            w.sim.battle.teams[1].tickets(),
        ),
        proxy_cfg.as_ref().map_or("", |p| p.2.as_str()),
    ));
    MapReport {
        text: out,
        census,
        stats: Some(MapStats {
            finished: w.sim.outcome.is_some(),
            stuck_max: w.sim.nav_stats.stuck_max,
        }),
        round: Some(round),
    }
}

/// The LOWER median of `v` (already sorted): index `(n − 1) / 2`, and `-` when
/// there is nothing to take a median of. No interpolation, so the printed
/// number is always one a map actually reported.
fn lower_median(v: &[u16]) -> String {
    if v.is_empty() {
        "-".to_owned()
    } else {
        v[(v.len() - 1) / 2].to_string()
    }
}

/// Combatants per side `--difficulty-check` plays without `SWEEP_COMBATANTS`:
/// the size the tiers were tuned at.
const DIFFICULTY_CHECK_PER_SIDE: u32 = 16;

/// Card DPRX: `--difficulty-check` (module header). One round per (tier,
/// map, proxy side, seed), `SWEEP_THREADS` at once; the verdict per tier
/// against `core::ai::proxy::TARGETS`.
fn difficulty_check(maps: &[(String, String, std::path::PathBuf)], args: &[String]) {
    use core::ai::proxy::{passes, target, Strength};
    use core::skill::Difficulty;
    let mut strength = Strength::Good;
    let mut seeds = 8u32;
    let mut ticks = 60_000u64;
    let mut tiers: Vec<Difficulty> = Difficulty::ALL.to_vec();
    let mut only: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().unwrap_or_default();
        match a.as_str() {
            "--strength" => strength = Strength::parse(&val()).unwrap_or(Strength::Good),
            "--seeds" => seeds = val().parse().unwrap_or(8),
            "--ticks" => ticks = val().parse().unwrap_or(60_000),
            "--tiers" => {
                tiers = val().split(',').filter_map(Difficulty::parse).collect();
            }
            m => only.push(m.to_owned()),
        }
    }
    if only.is_empty() {
        only = CHECK_MAPS.iter().map(|s| (*s).to_owned()).collect();
    }
    let selected: Vec<(&str, &Path)> = only
        .iter()
        .filter_map(|o| {
            maps.iter()
                .find(|(k, _, _)| k == o)
                .map(|(k, _, p)| (k.as_str(), p.as_path()))
        })
        .collect();
    if selected.len() != only.len() {
        eprintln!("difficulty check: some of {only:?} are not baked maps");
    }
    // The tiers (`core::ai::proxy::TARGETS`) were tuned at 16 per side on the
    // missions' tickets: the check plays that unless SWEEP_COMBATANTS /
    // SWEEP_TICKETS / SWEEP_TICKET_PCT say otherwise.
    let base = Cfg::from_env();
    let base = Cfg {
        combatants: base.combatants.or(Some(DIFFICULTY_CHECK_PER_SIDE)),
        mission_tickets: true,
        ..base
    };
    // (tier, map, side, seed)
    let mut jobs: Vec<(Difficulty, usize, u8, u32)> = Vec::new();
    for &d in &tiers {
        for m in 0..selected.len() {
            for side in [0u8, 1] {
                for seed in 1..=seeds {
                    jobs.push((d, m, side, seed));
                }
            }
        }
    }
    let threads = std::env::var("SWEEP_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(1, jobs.len().max(1));
    eprintln!(
        "difficulty check: proxy {} (PROVISIONAL profile {:?}), tiers {:?}, {} maps x 2 sides x {seeds} seeds, {} per side, {} rounds on {threads} threads",
        strength.label(),
        strength.profile(),
        tiers.iter().map(|d| d.label()).collect::<Vec<_>>(),
        selected.len(),
        base.combatants.unwrap_or(DIFFICULTY_CHECK_PER_SIDE),
        jobs.len()
    );
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<RoundResult>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                if k >= jobs.len() {
                    break;
                }
                let (d, m, side, seed) = jobs[k];
                let (key, path) = selected[m];
                CFG.with(|c| {
                    *c.borrow_mut() = Cfg {
                        difficulty: d,
                        proxy: Some((side, strength.profile(), strength.label().to_owned())),
                        ..base.clone()
                    };
                });
                let report = run_map(key, path, ticks, seed, false, None);
                eprint!(
                    "{}",
                    report
                        .census
                        .lines()
                        .filter(|l| l.starts_with("round "))
                        .map(|l| format!("{l}\n"))
                        .collect::<String>()
                );
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n.is_multiple_of(16) {
                    eprintln!("difficulty check: {n}/{}", jobs.len());
                }
                results.lock().expect("check results")[k] = report.round;
            });
        }
    });
    let results = results.into_inner().expect("check results");
    println!(
        "difficulty check: proxy {} (PROVISIONAL), both sides of each map, seeds 1..={seeds}",
        strength.label()
    );
    println!(
        "{:7} {:>6} {:>12} {:>7} {:>11} {:>8} {:>6} {:>6} {:>6}  verdict",
        "tier", "rounds", "proxy wins", "rate", "target", "margin", "K/D", "acc", "life_s"
    );
    let mut all_pass = true;
    for &d in &tiers {
        let (mut wins, mut n, mut margin) = (0u32, 0u32, 0i64);
        let (mut k, mut de, mut sh, mut hi, mut alive, mut lives) =
            (0u32, 0u32, 0u32, 0u32, 0u64, 0usize);
        let mut per_map: Vec<String> = Vec::new();
        for (m, (key, _)) in selected.iter().enumerate() {
            let mut side_w = [0u32; 2];
            let mut side_n = [0u32; 2];
            let mut side_m = [0i64; 2];
            for (j, r) in jobs.iter().zip(&results) {
                if j.0 != d || j.1 != m {
                    continue;
                }
                let Some(r) = r else { continue };
                let side = j.2;
                let own = if side == 1 { r.margin } else { -r.margin };
                let won = r.winner == Some(side);
                side_w[side as usize] += u32::from(won);
                side_n[side as usize] += 1;
                side_m[side as usize] += own;
                wins += u32::from(won);
                n += 1;
                margin += own;
                if let Some((_, s)) = &r.proxy {
                    k += s.kills;
                    de += s.deaths;
                    sh += s.shots;
                    hi += s.hits;
                    alive += s.alive_ticks;
                    lives += s.lives.len();
                }
            }
            per_map.push(format!(
                "    {key:10} proxy on t0 {}/{} ({:+.1})  on t1 {}/{} ({:+.1})",
                side_w[0],
                side_n[0],
                side_m[0] as f64 / f64::from(side_n[0].max(1)),
                side_w[1],
                side_n[1],
                side_m[1] as f64 / f64::from(side_n[1].max(1)),
            ));
        }
        let (lo, hi_t) = target(d);
        let ok = passes(d, wins, n);
        all_pass &= ok;
        let band = if hi_t >= 1.0 {
            format!(">={:.0}%", lo * 100.0)
        } else {
            format!("{:.0}-{:.0}%", lo * 100.0, hi_t * 100.0)
        };
        println!(
            "{:7} {:>6} {:>12} {:>6.1}% {:>11} {:>+8.1} {:>6.2} {:>6.3} {:>6.1}  {}",
            d.label(),
            n,
            format!("{wins}/{n}"),
            100.0 * f64::from(wins) / f64::from(n.max(1)),
            band,
            margin as f64 / f64::from(n.max(1)),
            f64::from(k) / f64::from(de.max(1)),
            f64::from(hi) / f64::from(sh.max(1)),
            alive as f64 / core::TICK_HZ as f64 / lives.max(1) as f64,
            if ok { "PASS" } else { "FAIL" }
        );
        for l in per_map {
            println!("{l}");
        }
    }
    println!(
        "difficulty check: {}",
        if all_pass { "ALL PASS" } else { "FAIL" }
    );
}

fn main() {
    let dir = std::env::var_os("SWEEP_ASSET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(game::assets::baked_root);
    let maps = game::assets::discover_maps(&dir);
    if maps.is_empty() {
        eprintln!(
            "no baked maps under {} — run scripts/rebake.sh",
            dir.display()
        );
        return;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--difficulty-check") {
        difficulty_check(&maps, &args[1..]);
        return;
    }
    let max_ticks: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(60_000);
    let seed: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);
    let only: Vec<String> = std::env::args().skip(3).collect();
    let verbose = std::env::var_os("ROUND_VERBOSE").is_some();
    // The tier goes to STDERR so the per-map result lines the acceptance
    // diffs stay exactly what they were.
    eprintln!("sweep difficulty: {}", sweep_difficulty().label());
    if sweep_human_team() != [0, 0] {
        eprintln!("sweep stand-in humans per team: {:?}", sweep_human_team());
    }
    eprintln!(
        "sweep slot 0: {}",
        if slot0_is_bot() { "bot" } else { "idle player" }
    );
    let census = stuck_threshold();
    if let Some(n) = census {
        eprintln!("stuck census: alive units stuck > {n} ticks at the last tick");
    }

    // Discovery order (`discover_maps` sorts by key) is the print order.
    let selected: Vec<(&str, &Path)> = maps
        .iter()
        .filter(|(key, _, _)| only.is_empty() || only.iter().any(|o| o == key))
        .map(|(key, _, path)| (key.as_str(), path.as_path()))
        .collect();
    let m = selected.len();
    if m == 0 {
        eprintln!("no baked map matches {only:?}");
        return;
    }
    let threads = std::env::var("SWEEP_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .or_else(|| std::thread::available_parallelism().ok().map(|n| n.get()))
        .unwrap_or(1)
        .clamp(1, m);

    // Each worker takes the next map index and stores its report in that
    // index's slot; nothing is printed to stdout until every worker is done.
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<MapReport>>> =
        Mutex::new(std::iter::repeat_with(|| None).take(m).collect());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= m {
                    break;
                }
                let (key, path) = selected[i];
                let started = std::time::Instant::now();
                let report = run_map(key, path, max_ticks, seed, verbose, census);
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                let secs = started.elapsed().as_secs_f32();
                eprintln!("{key} done ({n}/{m}, {secs:.0} s)");
                slots.lock().expect("sweep slots")[i] = Some(report);
            });
        }
    });

    let reports = slots.into_inner().expect("sweep slots");
    let mut all: Vec<u16> = Vec::new();
    let mut fin: Vec<u16> = Vec::new();
    for r in reports.into_iter().flatten() {
        print!("{}", r.text);
        // The census in map order after the join, like stdout (module header).
        eprint!("{}", r.census);
        if let Some(st) = r.stats {
            all.push(st.stuck_max);
            if st.finished {
                fin.push(st.stuck_max);
            }
        }
    }
    all.sort_unstable();
    fin.sort_unstable();
    let over = all.iter().filter(|&&s| s > 10_000).count();
    let sum: u64 = all.iter().map(|&s| u64::from(s)).sum();
    println!(
        "sweep: finished {}/{}  stuck_max median {} (finished-only {})  >10k {over} maps  sum {sum}",
        fin.len(),
        all.len(),
        lower_median(&all),
        lower_median(&fin),
    );
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use core::ai::Behaviour;
    use core::unit::{Controller, UnitHandle, UnitSpec};
    use glam::Vec3;

    #[test]
    fn every_death_cause_has_a_distinct_bounded_counter_for_both_teams() {
        use core::sim::{DeathCause, Event};
        let mut sim = core::sim::Sim::empty(7);
        let units = [0, 1].map(|team| {
            sim.spawn(&UnitSpec::trooper(team, Vec3::X * f32::from(team)))
                .unwrap()
        });
        sim.outcome = Some(0);
        for (team, unit) in units.into_iter().enumerate() {
            sim.events.push(Event::Kill {
                shooter: units[1 - team],
                victim: unit,
            });
            for cause in [
                DeathCause::DeathRegion,
                DeathCause::Fall,
                DeathCause::VehicleDestroyed,
                DeathCause::Blast,
                DeathCause::KillFloor,
                DeathCause::UnderMap,
                DeathCause::Lost,
                DeathCause::Stalled,
                DeathCause::Suicide,
                DeathCause::HeroExhausted,
            ] {
                sim.events.push(Event::EnvDeath { unit, cause });
            }
        }
        let mut counts = Deaths::default();
        counts.observe(&sim);
        assert_eq!(counts.0, [[1; 11]; 2]);
        let line = counts.line("fixture", &sim);
        assert_eq!(line.matches("suicide 1 hero_exhausted 1").count(), 2);
        // Deep water wears a droideka down: it counts with the death regions.
        let mut sim = core::sim::Sim::empty(7);
        let unit = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        sim.outcome = Some(0);
        sim.events.push(Event::EnvDeath {
            unit,
            cause: DeathCause::Water,
        });
        let mut counts = Deaths::default();
        counts.observe(&sim);
        assert_eq!(counts.0[0][1], 1);
    }

    fn bot(sim: &mut core::sim::Sim, pos: Vec3) -> UnitHandle {
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, pos)
        })
        .unwrap()
    }

    fn fire_spell(fight_at: impl Fn(u64) -> bool, move_at: Option<u64>) -> Fire {
        let mut sim = core::sim::Sim::empty(7);
        let h = bot(&mut sim, Vec3::ZERO);
        let mut fire = Fire::default();
        for t in 1..=FIRE_SPELL + 1 {
            sim.tick = t;
            let u = sim.unit_mut(h).unwrap();
            // Repeated hits keep the spell under fire beyond FIRE_TICKS.
            if t % 30 == 1 {
                u.last_hit = Some((h, t));
            }
            u.ai.behaviour = if fight_at(t) {
                Behaviour::Fight
            } else {
                Behaviour::Travel
            };
            if move_at == Some(t) {
                u.soldier.pos.x += FIRE_M;
            }
            let hash = sim.hash();
            fire.observe(&sim);
            assert_eq!(
                sim.hash(),
                hash,
                "a census must not change simulation state"
            );
        }
        fire
    }

    #[test]
    fn standing_fire_credits_the_fight_samples_in_the_qualifying_spell() {
        let enter = fire_spell(|t| t == FIRE_SPELL + 1, None);
        assert_eq!(enter.fight, [1, 1, 1]);
        assert_eq!(enter.stood, FIRE_SPELL);
        let leave = fire_spell(|t| t <= FIRE_SPELL, None);
        assert_eq!(leave.fight, [FIRE_SPELL, FIRE_SPELL, FIRE_SPELL - 1]);
        let steady = fire_spell(|_| true, None);
        assert_eq!(steady.fight, [FIRE_SPELL + 1, FIRE_SPELL + 1, FIRE_SPELL]);
        let moved = fire_spell(|_| true, Some(45));
        assert_eq!(moved.stood, 0);
        assert_eq!(moved.fight[2], 0);
    }

    #[test]
    fn fire_state_does_not_carry_a_pending_spell_into_a_new_generation() {
        let mut sim = core::sim::Sim::empty(8);
        let h = bot(&mut sim, Vec3::ZERO);
        let mut fire = Fire::default();
        for t in 1..=FIRE_SPELL {
            sim.tick = t;
            let u = sim.unit_mut(h).unwrap();
            u.last_hit = Some((h, t));
            u.ai.behaviour = Behaviour::Fight;
            fire.observe(&sim);
        }
        sim.units[h.index as usize].gen += 1;
        let next = UnitHandle {
            gen: h.gen + 1,
            ..h
        };
        sim.tick += 1;
        let tick = sim.tick;
        sim.unit_mut(next).unwrap().last_hit = Some((next, tick));
        fire.observe(&sim);
        assert_eq!(fire.stood, 0);
        assert_eq!(fire.fight[2], 0);
        assert_eq!(
            fire.hits[0], 2,
            "each life's first hit starts its own burst"
        );
        assert_eq!(fire.at[h.index as usize].fight_pending, 0);
    }

    #[test]
    fn standing_shares_keep_each_pending_samples_behaviour() {
        let mut sim = core::sim::Sim::empty(9);
        let h = bot(&mut sim, Vec3::ZERO);
        let mut shares = Shares::default();
        for i in 1..=STAND_SECS {
            sim.unit_mut(h).unwrap().ai.behaviour = if i == STAND_SECS {
                Behaviour::Recover
            } else {
                Behaviour::Travel
            };
            let hash = sim.hash();
            shares.observe(&sim, u64::from(i) * 30);
            assert_eq!(sim.hash(), hash);
        }
        assert_eq!(shares.stand, u64::from(STAND_SECS));
        assert_eq!(
            shares.stand_by[Behaviour::Travel as usize],
            u64::from(STAND_SECS - 1)
        );
        assert_eq!(shares.stand_by[Behaviour::Recover as usize], 1);
        assert_eq!(shares.stand_by.iter().sum::<u64>(), shares.stand);
    }

    #[test]
    fn floor_grid_rejects_invalid_or_unbounded_inputs_before_iteration() {
        for bad in [
            "0,0,1,0,100",
            "0,0,1,-1,100",
            "0,0,-1,1,100",
            "0,0,1,NaN,100",
            "0,0,1,inf,100",
            "NaN,0,1,1,100",
            "0,0,1,1,inf",
            "0,0,1e30,1e-30,100",
            "0,0,500,1,100",
            "0,0,oops,1,1,100",
        ] {
            assert!(parse_floor_grid(bad).is_err(), "{bad}");
        }
        assert_eq!(
            parse_floor_grid("2,3,1,0.5,100").unwrap(),
            (2.0, 3.0, 1.0, 0.5, 100.0, 2)
        );
        assert_eq!(parse_floor_grid("0,0,0,1,100").unwrap().5, 0);
        assert_eq!(parse_floor_grid("0,0,499,1,100").unwrap().5, 499);
    }

    #[test]
    fn heavy_csv_preserves_event_order_precision_and_handle_generations() {
        let mut sim = core::sim::Sim::empty(10);
        let a = bot(&mut sim, Vec3::ZERO);
        let b = bot(&mut sim, Vec3::new(0.0, 0.0, 3.0));
        // A nonzero vertical component verifies the existing 3-D speed contract.
        sim.unit_mut(a).unwrap().soldier.vel = Vec3::new(3.0, 4.0, 0.0);
        sim.unit_mut(b).unwrap().soldier.vel = Vec3::new(0.0, 0.0, 2.0);
        sim.weapon_classes[0].name = "fixture".into();
        sim.events = vec![
            core::sim::Event::Kill {
                shooter: a,
                victim: b,
            },
            core::sim::Event::Kill {
                shooter: b,
                victim: a,
            },
        ];
        let mut census = HeavyCensus {
            path: "unused".into(),
            n: Default::default(),
            tk: String::new(),
        };
        let hash = sim.hash();
        census.observe(&sim, &[], 1);
        assert_eq!(sim.hash(), hash);
        let from = sim.unit(a).unwrap().muzzle();
        let at = sim.unit(b).unwrap().capsule().center();
        let distance = at.distance(from);
        let reverse = sim
            .unit(a)
            .unwrap()
            .capsule()
            .center()
            .distance(sim.unit(b).unwrap().muzzle());
        assert_eq!(census.tk, format!(
            "1,fixture,-1.00,{distance:.1},none,-1.0,-1.0,-1.0,2.0,5.00\n1,fixture,-1.00,{reverse:.1},none,-1.0,-1.0,-1.0,5.0,2.00\n"
        ));
        let previous = census.tk.clone();
        sim.units[a.index as usize].gen += 1;
        census.observe(&sim, &[], 1);
        assert_eq!(
            census.tk, previous,
            "stale shooter and victim handles must not name a replacement life"
        );
    }
}
