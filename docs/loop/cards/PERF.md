---
id: PERF
title: Performance deep dive: profile the sim, the AI, the renderer and memory; find what bot count drops this machine below 60 fps and what a mid-class 2020 laptop could hold; then aggressively optimise (500 a side should be thinkable) - a standing PERF.md the maintainer's overnight summary reads from
status: archived
issue: 93
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #93](https://github.com/sprucey149/openbf/issues/93).

## ORDER (the maintainer, 2026-10-02 17:40) - HELD until the bot redesign is in
"Run PERF after the AI redesign and before the card that tests how many can run at a time and still hit 60 fps and what the defaults per map should be. So the defaults decisions land after the profiling card is done, when the updated bots are already in."
The order is: bot programme (JITR, FFLN, BOTM, SAIR, BOTD **and every build card BOTD cuts**) -> **PERF** (profile + optimise the game with the new bots in it) -> **UCNT** (bot counts at 60 fps, per-map defaults). No `first:`. Planner: when BOTD opens its build cards, add them to THIS card's deps and UCNT's; until then treat PERF as held even if its listed deps are done. The 60-fps bot-count tables are measured ONCE here, on the redesigned bots, and UCNT uses them.

## Why
The maintainer, 2026-10-02: "I would be interested in the results: what sims cause this computer to drop below 60 fps - and make sure it notes it somewhere easy so when I ask Claude what happened for the night, that's one of the stats it says. Our default definitely shouldn't be what this PC can handle, as this PC has a decently good CPU and GPU. We should think about what a mid-class laptop from 2020 can handle before choosing the defaults. Look into profiling the code and see if there are any ways to improve performance and optimise more - you never know when people will try running the game with 500 per team on the field at once, and the more optimised the game is and the better it runs now, the better. A full deep dive on game profiling, and aggressively look for places to speed up the code and improve performance and memory management."

## Part 1 - measure (this card; the report is the deliverable)
1. **A repeatable benchmark**: `game --example perf_bench` (or a mode of `round_to_outcome` + a windowed/offscreen render run): fixed seed, fixed camera path, maps small / medium / large (BF2 Coruscant, Kamino, Hoth; BF1 Endor, Geonosis), bot counts **16, 32, 64, 128, 256, 500 a side** (raise the pools as needed; say what breaks first - unit slots, bolts, sounds, draw calls). **500 a side is the CEILING (the maintainer, 2026-10-15): no count above it, no pool raised past it.** A map that still holds 60 fps (p99 frame under 16.7 ms) at 500 a side on a machine is written "not tried above 500 a side" for that map and machine - never a bare number - with an ESTIMATE beside it: fit sim ms and render ms against N (item 5's O(N) / O(N log N) / O(N^2) split), extrapolate to the N where p99 crosses 16.7 ms, and give the range with its assumptions (the dominant term, the fit's error, the limit that breaks first: unit slots, bolts, sounds, draw calls). Each run: sim ms per tick (mean / p50 / p99 / max) split by phase (AI think, steering sweeps, collision, movement, combat / bolts, vehicles, commander, nav), render ms per frame split (cull, soldiers' skinning, draw submission, GPU time), frame time mean / p99 / 1 % low, allocations per tick and per frame (should be 0 in core), peak RSS and VRAM, CPU threads used.
2. **Where 60 fps is lost on the maintainer's machine** (i9-13980HX, RTX 4000 Ada, 2560x1440): the bot count per map at which p99 frame time passes 16.7 ms, and whether the sim (30 Hz, single thread) or the renderer goes first. One table.
3. **The reference machine: a mid-class 2020 laptop.** Name it and justify it in the report (for example a 4-core / 8-thread 15-28 W CPU of that year with integrated graphics or an entry discrete GPU, 8 GB RAM, 1080p). No such machine is here, so build a PROXY and say how good it is: render on this laptop's Intel iGPU (renderD128 - see the machine notes) at 1080p; pin the game to 4 cores (`taskset`) and, where it can be done without root, a lower frequency cap, else scale the single-thread sim time by a published single-thread ratio (state the ratio and its source); cap memory with the systemd scope. Then the same table: the largest bot count that holds 60 fps (and 30 fps) on the proxy per map. This is the budget UCNT's defaults must fit.
4. **Profile**: `perf` + flamegraphs (or `samply`) on the sim at 64 and 256 a side and on the windowed game; a heap profile (dhat / heaptrack) for allocation sites and peak; cache behaviour where a hot loop is data-bound (`perf stat` cache-misses, struct sizes of `Unit`, the AI state, the collision bodies). List the top 15 costs with their share, and for each the likely remedy and its expected gain.
5. **Scaling law**: time against N bots - which parts are O(N), O(N log N), O(N^2) (unit-unit separation, perception, target choice, line-of-sight, the friend line-of-fire test FFLN adds, BOTD's position scoring) and where the curve bends.

## Part 2 - optimise (build cards opened by this card, each landable alone, each with a before / after on the benchmark and byte-identical sim hashes unless declared)
Candidates to weigh against the profile (do what the profile says, not this list): a spatial grid / hash for every neighbour query; perception and target choice staggered over ticks with a per-tick budget (AI level of detail far from any player - careful: the maintainer's annoyance 3 is BF1 bots that 'stop', so far bots think less often, never stop); cached line-of-sight; SoA / smaller hot structs; BVH or grid for the collision sweeps the steering makes; fewer sweeps per bot (BOTD's mover); the sim on its own thread from the renderer, AI think in deterministic parallel batches (the hash must not depend on thread count - D-040-style: decide and measure first); renderer: instanced soldiers, GPU skinning, animation LOD, impostors at range, frustum + distance culling of soldiers, batching by material, the HUD's cost; memory: pools sized by unit count, no per-frame Vec growth, texture / mesh sharing between instances; load time.
Order the build cards by (gain x likelihood) / effort; stop a card that does not pay.

## Part 3 - threading (the maintainer, 2026-10-02: "does this code already multithread the game? Look into it and figure out how it works for many different architectures and CPU sizes and powers")
**Today the game is single-threaded** (checked 2026-10-02, main d61a546): no rayon / crossbeam in any `Cargo.toml`; the only `thread::spawn` in the engine are the black box's file writer (`game/src/blackbox.rs`) and the bake's ogg pipe. The sim, the AI, the render submission and the asset load all run on the main thread (the audio and GPU drivers have their own). The maintainer's machine has 32 logical CPUs and uses one.
Measure and decide (a section of the report, then build cards):
1. How the frame's time splits between sim and render on one thread, per bot count - that is the ceiling a second thread buys (sim on its own thread: the renderer interpolates between two sim states).
2. Inside the sim: which phases are data-parallel (AI think per bot, perception, steering sweeps, position scoring) and which are ordered (combat resolution, spawns, RNG draws). **Determinism is the rule: the sim hash must be identical for 1, 2, 4, 8, 32 threads** - bots think on a read-only snapshot and write their own slot, results applied in unit order, no per-thread RNG. Prototype on AI think, measure the speed-up at 2 / 4 / 8 / 16 threads and 64 / 256 / 500 a side, and prove the hash.
3. **Different CPUs**: thread count chosen at start from the machine (physical cores, not logical; performance vs efficiency cores on hybrid Intel - the maintainer's i9 is 8 P + 16 E - and big.LITTLE ARM; a 2-core / 4-thread laptop; a 4-core 2020 laptop; a Steam Deck's 4c / 8t; a 16+ core desktop), a user override, never more threads than help. Measure pinned to P cores, to E cores, to 2 and 4 cores (`taskset`), and with SMT siblings only. Report where more threads make it SLOWER (small N: the overhead beats the gain - keep 1 thread below a bot count).
4. Architectures: x86-64 with and without AVX2 (no instruction newer than the baseline without runtime detection), aarch64 (builds? `cargo check --target aarch64-unknown-linux-gnu`), float determinism across them (the sim uses its own `fmath` - state whether hashes match across architectures or only within one, and what a replay / netcode later would need).
5. Also off the main thread: asset loading / level load, audio mixing, the black box (done), texture upload staging.

**The maintainer, 2026-10-20 17:05 (INBOX, standing):** (b) root causes first, threads last - fix the structure (load-time uploads,
per-frame allocation, AI bursts, O(N^2) queries) BEFORE adding threads, never use threads to hide a slow path; **Part 3 comes
after Part 2.** (c) Threading is automatic: the game picks its worker count from the machine (physical cores, leaving the
main, audio and driver threads their share); no setting the player must touch (an override may exist, the default is always
right); the sim result is identical for any count. The hitch hunt is card HITCH (not held), render interpolation card RINT.

## The standing record - `engine/artifacts/loop/PERF.md` (create it in this card; EVERY later card that changes a number appends one line)
Top of the file, always current: **the largest bot count per map that holds 60 fps on the maintainer's machine and on the reference proxy (at the ceiling written "500 a side held 60 fps (capped at 500, not tried further; estimated ~M)", INBOX 2026-10-15); sim ms / tick at 32, 64, 128, 256, 500 a side; the top 3 costs; date and commit.** Below it a dated log, newest first. The planner copies the top block into STATUS under a `## Performance` heading at every planner pass, so "what happened overnight" always has it.

## Acceptance
- `engine/artifacts/loop/PERF.md` with the top block filled; the report with the tables of 1-5, the flamegraphs under `lead_tmp/perf/`, the reference machine named and the proxy's error estimated.
- The benchmark runs from one command and is in `CRATES.md`; a `perf` gate line PROPOSED (not added): sim ms / tick at 64 a side must not rise more than 10 % per landing.
- Build cards opened and ranked. No behaviour change in this card; sim hashes untouched.
- STATUS has `## Performance`.

## Notes
- Run under the memory cap with a timeout (INBOX 2026-10-01 22:00): a 500-a-side run is exactly the kind that ran away to 46 GB. ONE heavy run at a time (`flock loop/ci.lock` rules apply to anything that loads all cores).
- Windowed runs are isolated (unset WAYLAND_DISPLAY, Xvfb / offscreen, never a window on the maintainer's desktop); GPU work takes `loop/gpu.lock`.
- This card runs on the redesigned bots (see ORDER). UCNT waits for it.

- Planner 2026-10-02 17:45: deps += BOTV (the "redesigned bots are on main" check). BOTD's build cards go into BOTV's deps, so this card waits for all of them without its own list being edited each time (the maintainer's 17:40 order).
- 2026-10-07 (the maintainer, via Claude): multithreading the SHIPPED game is NOT wanted now - the sim is about 1-2 % of the tick budget at 33 units (BOTD: 0.15-0.75 ms), so one thread has headroom for hundreds of bots. Revisit when this card's numbers show the sim passing its budget at the counts the maintainer wants (UCNT); the candidates then are bot thinking in fixed partitions (deterministic), map loading and audio mixing. The test tools use all cores on purpose; that is separate.

- Planner 2026-10-12 14:22, from OBST2 (closed today): **bar B13 of the bot design is not met and is this card's now.** The
  sim on the four cost maps (BF2 Kamino, Mygeeto, Polis Massa, BF1 Kashyyyk Docks), 33 units, one thread: 0.258 -> 0.187 ms
  a tick after OBST2's three cuts (x 0.72; the bar is 0.123). Where OBST2's last profile pointed next: `think`'s level
  test (an ask cache by position), the 1 m stop sweep, `plan_leg`, `level_hub`; re-profile the landed binary first
  (`lead_tmp/obst2/tools/sample.sh`, `prof_incl.py`, `perf.sh` with JOBS=1 and a null twin - RULES "Perf").
- 2026-10-12 17:11 planner, a cost reading for this card's baseline (the four B13 maps, one thread, the FIRST 3,000 ticks, 6
  seeds, one run at a time, run twice with the other lane's sweeps beside it, best reading per seed -
  `lead_tmp/planner_1012d/perf/`, `perf.sh` there): main 19708fa (before OBST2's cuts) 0.277 ms a tick, main 4ca302f
  (after them) 0.185, **main 5fbbbd4 (after MOUTH and CLSE2) 0.205 = x 1.11 of 4ca302f** (per seed 0.99 1.03 1.30 1.10
  1.05 1.18; sd 0.10 - not resolved under load, but no seed is under). CLSE2 gave every unit an aim block that thinks
  every 5th tick; MOUTH added the corner line. Card OBSV's 0.316 for main is this window on other seeds under more
  load, not a regression of 69 %. Re-measure on a quiet machine before quoting.
- planner 2026-10-19 16:25: **the bot-count benchmark half is deferred to near the end of the project** (the maintainer, 2026-10-19: "drop the idea of doing the bot benchmark for now"): `defer: post-alpha`. The profiling at today's bot counts is wanted now and lives in ISPD (render, whole frame) and AITHK (the AI's CPU). STATUS's Performance block reads ISPD's table until this card runs.
- planner 2026-10-21 12:55 (the maintainer's INBOX note of 10-21): **profiling tools here** - `perf` 7.2 (flat: `perf record -F 499 -o <f> -- <bin>`,
  `perf report --no-children --sort symbol --stdio`; `--call-graph dwarf` FAILS: SELinux / paranoid 2) and `valgrind` 3.27
  (callers: `--tool=callgrind` + `callgrind_annotate --tree=caller --inclusive=yes`; ~50x slower; also massif, dhat). No samply,
  heaptrack or flamegraph. Copy the binary, memory cap + timeout, instruction counts not wall clock under load. The maintainer's first
  finding (KAS2 16 v 16): `set_hull` = 37.6 % of sim instructions -> card HULLC (`first: 3`); next on his flat list: parry rays
  7 %, `height_at` 2 %, SipHash in `core`. Also in root `CLAUDE.md` "Profiling tools".
- planner 2026-10-21 17:15: HULLC is done (sim 16 v 16 now 0.13-0.15 ms / tick on KAS2 / NAB2 / GEO1, `loop/PERF.md`). Its
  leftovers for Part 2, from its `resume:` (shares of sim instructions on main 8882e85): parry AABB ray casts 6-8 %,
  `Barrier::entry_t` 7.3 % on NAB2, std SipHash maps in `core`; the level load costs 3-5x the 1,500-tick sim. Re-price
  them at the bot counts Part 1 reaches before filing a build card - at 32 units none is a frame-rate cost.
