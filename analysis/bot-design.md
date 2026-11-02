# OPUS DESIGN (loop lane 2, 2026-10-03, card BOTD) - [O] UNAUDITED; no build card is taken before the maintainer has read it (historical proposal baseline; see status note below)
# The bot design: what the best AI soldier for this game is

**Status note (2026-10-23 cleanup).** This 2026-10-03 proposal uses main e336f2e as its code baseline and includes later dated amendments. Its original read-before-build hold was released: [FGHT](../docs/loop/cards/FGHT.md) records the maintainer’s “BOTD read, go” on 2026-10-05 and BOTD done. Historical hold/build-order wording below describes the original proposal. Consult [CRATES](../engine/CRATES.md) and the owning cards for current implementation and acceptance; this note does not declare every proposal complete.

**What this is.** the maintainer, 2026-10-02: "are we just implementing so many bandaid fixes to a fundamentally flawed bot
algorithm ... think more about high level what the best ai bot looks like for this game". This note is that: one
architecture for the foot soldier's AI, argued from what the PS2 does (`re-soldier-ai.md`, "SAIR"), what the port's
bots do beside it (`bot-quality.md`, "BOTM"), seven measurements made for this note (section 1) and the outside
literature (section 15). It writes no engine code. Its build cards (section 12) wait for the maintainer's read (INBOX).

Tags: [P] the PS2 does it (SAIR section given); [B] BOTM; [M1..M7] measured for this note (section 1; tools and
output under `engine/artifacts/lead_tmp/botd/`); [C] the port's code at main e336f2e; [D] derived; [X] an outside
reference (section 15); OURS = a choice of the port's - every number in an OURS rule is PROVISIONAL until its card
has measured it.

## 0. For the maintainer - one page

**Is it bandaids on a flawed algorithm?** In three measurable ways, yes - and the cure is not a cleverer bot. It is
the PS2's own skeleton, with better decisions inside it.
1. *One function, many writers.* A bot's whole tick is `think` (792 lines). Its stick is written in four places, the
   point it walks at falls out of a chain of eight sources tried one after the other (way back, grenade, held
   spot, vehicle, route, order, post, memory), and 24 places assign its route [M7]. Nothing arbitrates and
   nothing commits, so two rules that disagree take turns: the stutter you photographed. The PS2 does not work
   like that: every unit has an order queue, ONE agent at a time, and one mover holding ONE move order and ONE
   aim order [P 1.4, 1.5].
2. *The steering feels its way with collision sweeps.* Every tick, for every bot, the port asks the collision world
   whether the next metre is a wall, a ledge, a slope or a hull, then slides, flips or fans. That one layer is
   **53-71 % of the whole simulation's CPU time** on four maps [M2], and it is where bots pace along walls.
3. *It ignores what the disc already says.* The level designers ringed buildings and props with AI barriers. The
   PS2's bots walk up to them - a quarter of their time is spent within a metre of one - and almost never stand
   inside one: **0.3-1.2 % of their time, against the port's 1.1-3.9 %**. And **where a port bot is stuck against
   a building or a prop it is inside a barrier in 36 % of the cases, inside its half-metre margin in 72 %, and
   within a metre of one in 84 %** [M3]. The PS2 tests the barriers at every step from where the bot stands
   [P 2]; the port tests them once per leg, from the leg's line.

**What the best bot for Battlefront is** (section 2): not the smartest one. It (a) always visibly does something
with a purpose, (b) moves like a soldier - no jitter, no walking into things, no standing in a row, (c) makes the
series' big, readable fights at the posts, (d) fights from a place - behind something, at its weapon's range, out
of a friend's line -, (e) gets better with the difficulty in what it DOES, never in damage or sight through walls,
and (f) is cheap, so the counts can go up.

**The shape** (sections 3-4): six layers, each the only writer of one thing.
Commander (which post: the PS2's own rule, kept) -> Team board (who flanks, who takes the turret, who covers) ->
Behaviour (ONE runs at a time, with an entry, an exit and a minimum time; an interruption resumes what it
interrupted) -> Navigator (the disc's routes, a point in each hub rather than its centre) and Tactician (WHERE to
stand in a fight: scored positions with real cover, reserved so two bots never take the same one) -> Mover (the
ONLY writer of the stick: follow, arrive and STAND; round barriers, vehicles, turrets and other soldiers on one
remembered side) and Gunner (the only writer of aim and trigger).

**What you would notice.** A bot that stops, stands. A bot that walks goes round the console, the plinth, the
parked speeder and the other bots. A fight is fought from corners and crates on every map, not only where a
designer left a hint: 82 of the 185 posts have an authored Cover hint with real cover within 30 m; with generated
positions 154 have five or more covered spots [M6]. A turret is manned while it has something to shoot and left
when it has not. Some attackers take the other way round. Nobody waits in a corner at the end of a round. Easy
bots are slow and plain; Elite ones use everything.

**Your three annoyances** (section 6). Turrets: a seat is taken for a purpose and has five ways out. Dead parts of
maps: flank routes for a share of the attackers, and positions off the route - a tilt of the route weights cannot
do it, because on ten maps only 0-2 % of the route table's entries toward a post have a second way at all [M5].
Bots that stop: no behaviour ends in "idle", a failed walk always has a next step, and the sweep counts every
unexplained 20 s of standing.

**Decisions that are yours** (the default taken is in brackets; sections 7 and 9 have the detail):
1. The deviations list, D1-D16 (section 9): each is "the PS2 does X, we do Y, because Z". [all sixteen on]
2. Tiers as competence (section 7): the table - and whether Hard and Elite keep their damage bonus against you
   (x1.15 / x1.3 today; ours, never the PS2's). [drop it: nothing a human player could not do]
3. "Not as statues" (your 2026-10-01 rule): the random shuffle and strafe chances give way to a peek cycle and a
   move to another position every 5-12 s, the PS2's own rhythm. [replace]
4. A small garrison at a front-line post (1-2 bots stay as guards; on the PS2 nobody is told to stay - a capturer
   moves on 5 s after its order resumes). [off, as the PS2; the turret gunners are the defence]
5. How much of an attack flanks: a share of a post's attackers, drawn per match. [20 %, between 10 and 30]
6. Bots sprinting on long walks with no enemy near (the PS2's never sprint). [no]
7. The corpse shot (rebel troopers beside you shooting a dead enemy, SAIR 4.10). [not built]
8. Thinking slower away from the player, as the PS2 does (decisions 2-8 s apart). [no: one rate everywhere; PERF
   may ask for it at 500 a side]

**Build order** (section 12): 16 cards in a chain (6 new, 10 already on the board and held for this note), the
sweep green at each: the seams first (no behaviour change), then the mover, the avoidance, the fight, the
positions, the team, turrets, flanks, weapons, tiers. BOTV checks the bars of section 10 on main; then PERF and
UCNT, as you ordered.

## 1. What was measured for this note

All on this machine (i9-13980HX), the night of 2026-10-02; tools in `lead_tmp/botd/tools/`, output in
`lead_tmp/botd/out/`.

| # | question | result | tool -> output |
|---|---|---|---|
| M1 | What AI data does each map carry? | Cover hints 3-249 a map (BF1 Hoth 3, BF1 Naboo Plains 6, BF1 Kashyyyk Docks 6, BF1 Cloud City 11; BF2 Death Star 249; BF2 Yavin's 973 not checked for duplicates), Snipe hints 0-44, soldier barriers 77-759 (median 352), hubs 9-200 with a median radius of 1.3-9.8 m (BF1 Naboo Plains: 9 hubs, 39 m) | `nav_census.py` -> `nav_census.txt`; barrier counts from `barr_cover.py` |
| M2 | What does today's bot cost, and where? | The whole sim over a round's first 3,000 ticks, both teams at full strength (about 33 units), one thread, the sweep binary built from e336f2e: 0.15 (NAB1) / 0.27 (HOT1) / 0.29 (POL1) / 0.49 (KAM1) / 0.75 (MYG1) ms a tick. Of the stack samples inside `Sim::step` over whole rounds (207-900 a map): `think` 64-82 %, **`steer_dir` alone 53-71 %** (its stop test 45-67 %; ground rays 14-60 % of all samples, wall sweeps 17-41 %), perception 0.3-2.9 %, `retarget` 1.2-6.0 %, the commander 0.7-1.4 %, the soldiers' own wall pass 6.8-13.5 %. Heap calls show in 0.1-2.3 % of samples, by symbol inside parry3d's contact query (for PERF). A first profile of main's older binary (built 21:37, commit not recorded) read the same within sampling noise: `think` 55-83 %, `steer_dir` 50-69 % | `sample.sh` (eu-stack every ~30 ms) + `prof_keys.py` -> `profile_e336f2e.txt` (the first: `profile.txt`); timings `t3_*.time` |
| M3 | Are the things bots stall on the disc's barriers? | **All bots on foot** (Kamino, Polis Massa, Death Star, Kashyyyk Docks; the mission's switched-off barriers left out): the PS2's stand inside a soldier barrier in 0.8 / 0.3 / 1.2 / 0.3 % of their rows and within 1.0 m of one in 23-26 %; the port's inside one in 3.4 / 1.4 / 1.1 / 3.9 % and within 1.0 m in 14-17 %. **Stalled bots** (OBST's 16,224 stalled 10 s windows, 32 maps x 8 seeds, main 7b64fcd): class `body` (4,416): inside a bare barrier 36.0 %, inside one grown 0.5 m 72.0 %, within 1.0 m of one 84.0 %; a barrier lies across the first 5 m of the line to the leg's waypoint in 87.0 % (58.7 % not counting a barrier the bot stands in). `post` (3,416 with a route): inside 39.1 % / 61.6 %; `spot` (3,311 with a route): 31.2 % / 49.2 %. Controls: `open` 12.2 % / 21.3 %, `hull` 4.7 % / 5.8 %. (The WHOLE line to the waypoint crosses a barrier in 95.6 % of `body` windows, but a line of the same length in a random direction does in 90.6 %: that figure is no evidence and is not used.) | `inbarr.py` -> `inbarr.txt`; `barr_cover.py`, `barr_cross.py` -> `barr_cover.txt`, `barr_cross.txt` |
| M4 | How crowded is a bot's neighbourhood? | 16 v 16, 4 Hz rows after JITR (seeds 1-2, four maps): another soldier on foot within 4 m in 24-45 % of rows; 99th percentile 2-4, maximum 4-8; more than 6 within 8 m in 0.9 % of rows or fewer | `neigh.py` -> `neigh.txt` |
| M5 | Can a tilt of the route weights open the dead areas? Do other routes exist? | Entries of the soldier route table toward a post hub with 2 or more authored alternatives: **0-2 % on ten maps** (MYG1 0 of 580, POL1 0 of 1,104, KAM1 17 of 846). Another route (at most half its arcs on that pair's authored set) within +35 % of the authored length: MYG1 20 of 20 ordered post pairs, TAT1 22 of 30, YAV2 13 of 18, POL1 18 of 30, DEA1 18 of 30, KAS2 14 of 42, HOT1 7 of 12, UTA1 13 of 29, KAM1 3 of 17, NAB1 2 of 20 - a lower bound (one search a pair). Some of them only reuse ways authored for OTHER pairs (MYG1 8 of the 20); those that touch a hub off every authored post-to-post route: MYG1 8, TAT1 12, YAV2 8, POL1 16, DEA1 14, KAS2 10. Off-route hubs they add: MYG1 6 of 50, TAT1 13 of 73, YAV2 9 of 30, POL1 41 of 115, DEA1 22 of 60, KAS2 16 of 63, KAM1 20 of 75, HOT1 2 of 27, UTA1 1 of 35 | `flank.py` -> `flank.txt`: the disc plan that matches the port's Conquest graph; the port's post hubs, two TAT1 posts moved off the arc-less 525 m hub (card HUBM). The off-route counts equal SAIR 10b's on Kamino (75) and Polis Massa (115) |
| M6 | Where is there cover? Are the authored hints honest? What does a ray cost? | Of the 4,129 Cover hints in the maps' base layers, 1,194 (29 %) lie INSIDE a soldier barrier and 1,776 (43 %) inside one grown 0.3 m (`hint_in_barr.py`). The census (4,176 with the Conquest layers) keeps the 2,351 that lie outside the barriers and pass its other standing tests. Of those, 72 % have something solid at crouch height within 3 m along the stored +Z row (17 % along -Z: +Z is "ahead"), and 44 % give USABLE cover (shut at crouch-eye height within 3 m, and a 30 m lane either at standing height or from a 1 m lean). The hints inside barriers were not measured. Points 0.9 m outside the barriers' edges and corners: 59,872 standable, **20,443 with usable cover** (34 %); points in hub discs 2,416 of 25,932 (9 %). Posts with a covered Cover hint within 30 m: 82 of 185; with 5 or more covered candidates of any source: 154 (six showcase maps: 14 and 26 of 31). 2.53 M ray tests (a terrain ray and, when that is clear, a world ray) in 2.37 s of the census's whole work: **0.94 us each** (the verifier's re-run: 1.02); one map's table 8-208 thousand tests, under 0.2 s | `cover_census.patch` (a scratch hook in `round_to_outcome`, worktree `openbf-wt/botd-scratch` at e336f2e) -> `cover_census.txt`; `cover_sum.py` -> `cover_summary.txt` |
| M7 | How many writers has each output today? | `ai/brain.rs` at e336f2e (1,502 lines before its tests; `think` 792): the stick `pad.lx / ly` at 4 sites; the tick's move goal from a chain over 8 sources, then a conditional side-step offset; the held position through `step_to` (4 callers), `settle` (1) and `start_hold` (2); the slide side 5 writers (3 in `steer_dir`, 2 in `brain.rs`); the side-step counter 5; the spelling `st.route =` at 24 sites before the tests of `brain` (5), `route` (14), `squad` (4), `walker_drive` (1) | `grep` (the commands are in the session's report) |

And from the two notes this one builds on:
- **The PS2's bot** [P 0, 12, 13]: an order queue, one agent, helper state machines; decisions 0.1-8 s apart by the
  human's distance; a weighted draw over the authored route table at every hub and a random point in the next hub;
  barriers as the only obstacles; no avoidance of other units before contact; a fight that holds the weapon's range
  band and takes Cover hints by their arrow (21-49 % of attack time); no reaction to friends, health or ammunition;
  a turret boarded with a target in its arc and left after 10 s with no remembered threat.
- **The port beside it** [B, and `lead_tmp/botm/post_jitr/seeds.md`: main ac605ca, the lower median of 4 seeds,
  after JITR]: a bot that stays put walks 8.8 m in 10 s on Kamino (PS2 1.3 m) and 20.0 m on Kashyyyk Docks (1.3 m);
  a teammate within 1.5 m 8.7-17.9 % of the time (PS2 2.0-9.3 %); an enemy within 20 m 2.2 % on Kamino (PS2 15.1 %)
  and 2.7 % on the Death Star (18.6 %); the three busiest cells hold 19 % of Kamino's deaths (PS2 42 %); time in a
  seat 0 % on Kamino and Polis Massa (PS2 9.4 % and 18.0 %). FFLN and WAI2 have landed since and the table was not
  re-taken.

## 2. The best bot for this game

A Battlefront round has 32 to 64 bots; most die within a minute of meeting the enemy (20-36 deaths a minute on the
PS2 [B]); the player meets them in bulk - a line coming at a post, a knot behind crates, a turret swinging round.
Nobody duels one for long. So the best bot is not the most tactical one. It is the one whose every visible second
reads as a soldier doing something sensible:

| the bot ... | which means | measured by (section 10) |
|---|---|---|
| a. always has a purpose | every behaviour has an entry, an exit and a successor; nothing ends in "idle"; a walk that fails says so and something else follows | B9 unexplained standing, B10 recycles |
| b. moves like a soldier | one writer of the stick; it arrives and stands; it goes round what is in its way on one side, before touching it; it keeps its distance from friends | B1 pinball, B2 staying path, B3 spacing, B10 stalls |
| c. makes the series' fights | the PS2's own goal and routes; bots that hear a fight go to it; fights concentrate at the posts | B5 contact, B6 deaths a minute, B7 hotspots |
| d. fights from a place | a position with cover from its target, a line to it, at its weapon's range, not a friend's, not in a friend's line | B12 cover share, B4 team kills |
| e. is as good as its tier | Easy is slow and plain, Elite uses everything; never more damage, never knowledge it could not have | TIER's ordering, section 7 |
| f. is cheap | sense when deciding, not every tick; 2-D tests against the disc's own data instead of collision sweeps | B13 ms a tick |

Three things from outside shape it. (1) Pandemic's own aim was density: "There should always be something cool
happening", and maps were "designed and redesigned ... to concentrate the fights and increase their intensity"
[X13] - so (c) is a bar, and spreading bots out is done with care (6.2). (2) In Halo's playtests the enemies were
called "very intelligent" by 8 % of players, and by 43 % in the next test, after their hit points and damage had
been raised [X1]: a player credits what he has time to see, plainly. So behaviours here last seconds, change at
think rate, and show in stance and facing. (3) Every shooter AI this note read separates "what to do" from "where
to stand" from "how to get there" from "how to shoot", with one channel down to the body [X2, X4, X6, X7, X15] -
which is also the PS2's own split (agent, helpers, mover, fire controller [P 1.4, 1.5]).

What the design is NOT: a planner (F.E.A.R. plans because its soldiers have many actions per goal; its goals are
still a priority list that falls through when a goal has no plan [X6], and that is the part kept); a navmesh (the
disc's hubs, connections and barriers are the level designers' own description of where a soldier walks, and M3
says they describe it well); a learner; or a bot that knows what it has not seen.

## 3. The skeleton: one owner per output

```
 Commander (per team, 1 Hz) ----- order ----+
 The human's squad order -------- order ----+--> BEHAVIOUR (per bot, at think rate; ONE runs;
 Team board (roles, reservations, lanes) ---+     an interrupt is pushed in front and resumes)
 Perception (contacts, threat list) --------+          |
                                                       +-- asks --> TACTICIAN: a position (scored, reserved)
                                                       |
                         MoveRequest (one a tick) <----+----> AimRequest (one a tick)
                               |                                    |
                           NAVIGATOR (route, leg, waypoint)       GUNNER (aim tracker, trigger, weapon)
                               |                                    |
                           MOVER (follow, arrive, avoid)            +--> look sticks, FIRE, weapon keys
                               |
                               +--> move sticks, stance / roll / jump
```

| output | its ONLY writer | read by | today [C] |
|---|---|---|---|
| the order (capture post P, defend P, hunt, follow, hold an area, man turret T, ride vehicle V) | Commander; the human's squad order outranks it; the Team board adds the role | Behaviour | `commander::plan`, `squad::steer`, `ride::claim`, each writing route and goal fields directly |
| which behaviour runs, since when, until when at least, what it interrupted | Behaviour selector | every layer below | no such thing: optional fields (`route`, `hold`, `lost`, `ride`, `cmd`) and a priority chain in `think` |
| the position (point, stance, facing, reservation, expiry) | Tactician | the running behaviour | `start_hold`, `hold_at_hub`, the shuffle, the strafe, the dodge step, the step out of a friend's line |
| the MoveRequest (destination, arrive radius, pace, facing mode, stance wish) | the running behaviour | Navigator, Mover | the chain over 8 sources, then the side-step offset |
| the route (destination hub, leg, waypoint in the hub, corner list) | Navigator | Mover | `retarget`, `plan_leg`, `advance_route`, `stuck_window`, `squad`, `walker_drive`: 24 assignment sites |
| the move sticks; stance, roll and jump presses | Mover | the soldier's own code | 4 stick sites, 2 roll / stance sites, 1 jump site in `think` |
| the AimRequest (target unit or point; fire: free / hold / at a point) | the running behaviour, from perception's threat list | Gunner | inline in `think` |
| the look sticks, FIRE, weapon and grenade keys | Gunner | soldier and weapon code | inline in `think` (aim tracker, bursts, taps, the friend check) |

This is the PS2's shape, not a new one: an order queue (current + 5) -> one agent -> its helper state
machines -> the mover (`mv`: ONE move order at +0x20, ONE aim order at +0x70, request bits, a stance word) -> the
soldier's AI input `FUN_00249f80`, which runs the fire controller and the steering and writes the four sticks
[P 0, 1.4, 1.5]. An interrupt is pushed in front; a capture or a wait resumes after it, while a new order REPLACES
a running fight, boarding or unstick walk and the capture queued behind comes back; one order change per think.

**How often each layer runs.** Sensing is paid when a decision is made, not every tick (2f).

| layer | runs | today [C] | the PS2 [P 1.1-1.3] |
|---|---|---|---|
| Commander | per team, once a second | the same | the goal's 1 s tick |
| Team board | per team, once a second | - | - |
| Perception, Behaviour | per bot, every think (0.33 s; by tier, section 7), the phase by unit index | every 10 ticks, the phase by spawn tick | every 0.1-8 s by the human's distance (BF1 0.5 s; 1 s far from the camera and out of its view [P 1.6]) |
| Tactician | on entering a fight, on an event, else every 2-5 s | - | the cover draw every 2-5 s |
| Navigator | at a hub, on a new request, on a failure | every think and every tick | at a hub |
| Mover: choose a heading | every 3rd tick, the phase by unit index; held between | every tick: the sweep | every frame near the human, every 3rd or 5th away |
| Mover: arrival, last step; Gunner: aim, trigger | every tick | every tick | the tracker with the mover's rate |
| safety probe (one sweep and floor test 2 m ahead) | every 5th tick, only off ground the plan has checked | every tick, always | none: barriers, contact, stuck timers |

## 4. The layers

### 4.1 What a bot may know
- **From the disc** (loaded today [C `nav.rs`, `game/src/layout.rs`]): hubs, connections, the route table, barriers
  with the mission's switches, hint nodes, posts and their capture regions.
- **The position table** (OURS; built once at map load, a pure function of the map, outside the sim hash): every
  place a soldier might stand to fight, with what can be known about it without looking again.
  - Sources: Cover and Snipe hints [P 3]; the points 0.9 m outside each soldier barrier's four edge middles and four
    corners; each hub's centre and 8 points at 0.7 of its radius; each turret's seat.
  - Kept when a soldier can stand there: a floor on its storey or the open terrain, on the level
    (`Sim::level_at`), not in water, in no soldier barrier, head room - the census's test [M6]. The barrier rule
    is the PS2's own practice: its bots are inside one for 0.3-1.2 % of their time and within a metre of one for
    a quarter of it [M3], though 29 % of the authored Cover hints lie inside one [M6] - the edge is where its
    bots stand.
  - Per position: point, floor, kind, the hint's facing (its +Z row [M6]) and stance masks, the nearest hub (so the
    storey and a route), and two 16-bit masks over 16 directions: LOW cover (crouch behind it, stand to fire) and
    CORNER cover (lean to fire).
  - Size and cost [M6]: 300-5,700 standable points a map, 8-208 thousand ray tests, under 0.2 s at load.
- **Per tick, fixed size, no allocation:** a unit grid (8 m cells, units sorted by cell) that answers "who is within
  r of p" for perception, avoidance, the friend-in-line test and reservations - each of those walks every unit
  today [C `friend_in_line`, `spot_free`, the hunt loop in `think`, `perception::scan`]; the footprints of vehicles
  and turrets as rectangles; the grenades' hazard list [C `Sim::hazards`].
- **Knowledge is perception** [C the six contacts, kept]. Two exceptions exist and stay, both coarse: the commander
  knows who owns each post, and the hunt knows the enemy's HUB, not his position [C `route::retarget_hunt`]. The
  Team board adds one: a contact held by a friend within 20 m is passed on as "heard" - walked toward and aimed
  at, never fired at unseen.

### 4.2 Commander - kept
The PS2's Conquest goal as card CONT built it (4c1460d): one enemy post drawn by its authored value and kept until
done; no distance, enemy or crowd term [P re-conquest-goal 0]. The design leaves it alone. The port's fights are
today LESS concentrated than the PS2's (the three busiest cells hold 19 % of Kamino's deaths against 42 % [B]), so
an anti-crowding term would push the wrong way. One defect first: card HUBM (a hub with no arc for the mover is not
a hub; audit CONT, re-conquest-goal 10). OURS and kept: the hunt with nothing to capture and in the endgame (a side
at 10 tickets or fewer [C `HOLD_ENDGAME`]).

### 4.3 Team board - OURS, small
Per team, once a second, in unit-index order. The PS2 has nothing like it [P 6]; this is the least that gives the
turret, flank and "do not stack" wishes one owner.
1. **Reservations.** One bot per position: the PS2's own hint rule made general ([P 3]: one unit per hint, free 2 s
   after its last stamp). A position within 2 m of a reserved one is not offered [C JITR's `HOLD_SPACING`].
2. **Roles inside a post's group**, by capacity (Halo 3 fills tasks that carry a priority and a capacity [X3];
   Killzone's objectives carry an optimal number of bots [X15]):

| role | who | capacity | what changes for the bot |
|---|---|---|---|
| Assault | everyone not below | - | nothing: the authored route |
| Flank | attackers of post P standing at a hub with a flank route (6.2) | the match's flank share of the group; none in a group under 3 | the Navigator walks the flank route |
| Overwatch | `UnitType = scout` classes with a free Snipe hint in reach [P 3] | as the PS2 | the snipe stand, shortened (D13) |
| Gunner | the nearest free bot within 50 m of a turret whose purpose test passes (6.1) | 1 a turret, 2 a post | Board, then Man |
| Cover | while a friend captures P: others inside the region | 2 | a guard position facing the approach, not a second bot standing in the region |

   A role is kept for at least 20 s and until its task ends.
3. **Shared knowledge:** the contacts passed on (4.1), and each friend's line of fire for the Tactician's "not in a
   friend's line" term (card FFLN's own test [C `friend_in_line`]).

The human's squad orders (`squad.rs`, re-squad-orders) outrank the board, as today. It is not a squad leader, a
formation or a plan.

### 4.4 Behaviour - one runs; entry, exit, a minimum time
"[P]" columns are the PS2's own values; what is ours is marked.

| behaviour | entered when | it asks for | it ends when | at least | the PS2's |
|---|---|---|---|---|---|
| Travel | an order with a destination, nobody to fight | move: the order's point over the route (a capture's last leg ends at a drawn point of the post's region [C PARR]); aim: none, the gun level | arrived; a threat; a new order | 1 think | the trooper agent's walk |
| Fight | the threat list's best target is seen (or was, within the tier's memory) inside weapon range + 10 m [P 4.2] | the Tactician's position for (target, weapon), body to the target; aim at it, fire free | no target, or it is beyond weapon range + 10 m, for 2 s (OURS; the PS2 at once); then what it interrupted | 2 s (PS2 spells: median 2.2-3.4 s [P 4.11]) | combat agent, state 5 |
| Investigate | an enemy heard within min(its radius, 30) m; out of a fight, a hit by an attacker inside view distance [P 4.1]; the target lost from sight (D14) | walk to the point, facing it; fire when seen | Fight when seen; at the point two looks of 0.4 s; after 10 s [P 4.2] | 1 s | combat states 2-4. **Built on a patch, measured, NOT TAKEN** (cards FGHT, FGHT2; re-soldier-ai 15.2): it runs at the PS2's share of time and close contact does not move (Kamino 3.5 -> 3.2 %), and on Polis Massa a team mills over a sniper it cannot reach (deaths a minute 18.7 -> 13.8, last-resort respawns 10 -> 27 with the storey rule and a 10 s pause) |
| Evade | inside a grenade's warning radius (the tier's chance and delay [C `hazard_flee`]); out of a fight, a shot passing within 7.5 m or a hit by an attacker out of view [P 4.1] | 20 m from the point, or 3 m past the radius; or a roll (the PS2 rolls when 4 m are clear, 1 in 2 [P 4.1]; by tier, OURS) | done, or after 2 s [P 4.2] | runs to its end | combat state 6. The grenade entry is built; the passing-shot entry was **built on a patch, measured, NOT TAKEN** (re-soldier-ai 15.3: without the PS2's skill delay, pinball windows on the Death Star 1.5 -> 4.0 %, the PS2 1.5) |
| Capture | arrived in the target post's region | a covered position in or beside the region facing the approach, else the drawn point; another within 8-14 m every 20-30 s [P 3; BF1: Patrol hints, 30-45 s] | the order ends by the commander's rules (BF2: 5 s after arriving at a post the team already holds; re-conquest-goal 0) | 5 s | the at-post helper |
| Guard | the order Defend, or the role Cover | a position of the post's set facing the nearest enemy post's approach | the order or role ends | 10 s | (by-product only) |
| Overwatch | the role Overwatch and a free Snipe hint | stand at it; weapon range x 2 [P 3] | D13: 20-40 s with no target seen (BF2: 60-120 s, BF1 60-100 s, and after a fight usually the same hint again [P 3]), or hit there; not the same hint next | 15 s | trooper agent, state 7 |
| Man | the role Gunner | board; then the seat gun [C `gunner.rs`] | 6.1's five ways out | 10 s seated | the wait agent in a seat |
| Board / Ride | a vehicle claim [C `ride.rs`, WAI2] | as today | as today; card VBAIL adds the PS2's bail-out | - | board agent |
| Serve | card ENGR's tasks [P 4.8] | walk into range, use the tool | 20 s out of position, 8 s with no gain [P 4.8] | - | trooper agent, state 6 |
| Follow / Hold area | the human's order [C `squad.rs`] | as today, through a MoveRequest | the order is cleared | - | follow / hold agents |
| Recover | the Navigator or the Mover reports a failure: no route; the stuck test; off the level | the ladder of 6.3 | 2 m of net progress | until then | the random agent |
| Fall back (D10, Hard and Elite) | under 30 % health, a target, no cover | a covered position away from the threat and toward friends; firing | 8 s, or no threat for 3 s | 4 s | none |

**Interrupts** (after [P 1.4], where a capture or a wait resumes and a fight is replaced): Evade and Recover
interrupt anything. Fight and Investigate interrupt Travel, Capture, Guard, Overwatch and Serve. What was
interrupted resumes when the interrupter ends. One change per think; a behaviour
inside its minimum time yields only to Evade, Recover or the loss of its subject. **No behaviour is "idle":** a bot
without an order asks the commander (the PS2's goal gives one within a second [P 1.4]); with nothing to capture it
hunts. Halo 2's answer to "dithering (the rapid flipping back and forth between two or more actions)" has the same
shape: a fixed priority order with yes / no relevance, where only a higher entry may interrupt the running one, and
events enter as impulses that live "one or two seconds" [X2]. It rejected the alternative of scoring every behaviour
each tick and giving the last winner a bonus.

### 4.5 Navigator - kept, three of the PS2's rules restored, one added
- **Next hub:** the authored table's weighted draw at every hub, the arrival arc excluded, by a per-unit hash
  [P 2; C `nav.rs`] - as today. The fewest-arcs fallback for an empty table stays (ours).
- **HUBP:** the waypoint is a random point of the next hub's sphere (so of its disc), per unit per leg, re-drawn
  while it lies in a barrier (the PS2) or has no floor (ours) [P 2]. Today it is the centre, and bots file
  through it [B].
- **HUBM:** locate skips a hub with no arc for the mover (the PS2's `FUN_00266678` / `FUN_002eb918`).
- **LOC3:** a post's hub is the PS2's locate - BF1 a hub sphere, BF2 an XZ disc, both games the 3-D corridor with
  the smallest rA + rB, then the nearest arc in 3-D (BF2's flag tests) - `NavGraph::locate`, held against the
  PS2's stored result for 1,004 live path goals (`game/tests/locate_ps2.rs`). Where a UNIT stands keeps the port's
  XZ locate and its level rule (D17).
- **The leg:** aim at the bot's projection on the leg advanced by |v| + 0.2 m; arrive within 1.0 m of a node, 0.5 m
  of the last, or past the node's plane [P 2: BF2's rule, used for both games; BF1 aims straight at the point].
  Built by card MOVR2 (2026-10-07): the aim (`route::leg_point`). The radii stay the port's 1.5 m / 1.0 m, by
  measurement (D18).
- **The last leg ends outside every barrier:** the PS2's planner refuses a goal that is not clear [P 2]. Today 39 %
  of the bots stalled on a last leg at a post stand inside a barrier, 62 % inside one grown 0.5 m [M3].
- **Flank routes** (OURS, 6.2): per ordered post pair, at load, the best other route within +35 %, a hub list.
- **Every walk has an outcome** - Following, Arrived, Failed(why) - and the behaviour that asked handles all three. Card WHY
  (2026-10-28) records the why: a code on every failed walk and every behaviour change (`core::ai::why`), in the
  black box and counted per map by the sweep's census.

### 4.6 Tactician - OURS: the PS2's cover use, made honest
Asked with (bot, target or approach, weapon band, purpose) it answers one position or none. The order is the one
the literature agrees on - generate, filter with the cheap tests, score with few weights, confirm with the
expensive test from the best down [X4, X8]:
- **Candidates:** the table's positions within min(40 m, weapon range) of the bot [P 4.6: the PS2's radius], on its
  storey; the 32 nearest. (Killzone found that a search wider than a decision cycle of travel gives "a series of
  movements to promising locations without getting there" [X4]; the travel weight and the commitment below are
  the guard.)
- **Filters, all yes / no:** not reserved, and 2 m or more from a reserved one; outside every hazard; outside a
  friend's line of fire; no more than 15 m farther from the bot's objective than the bot is (a fight does not walk
  a capturer backwards; 15 m is the PS2's band move [P 4.5]); inside the weapon's band grown by 5 m, the band being
  inner = min(optimal, min + 3) to outer = min(max(optimal, range - 10), view - 10) [P 4.5].
- **Three tries, the first that yields a winner** (ordered fallbacks instead of more weights [X8]): (1) positions
  whose sector mask gives cover from the target; (2) any position that passes the filters - spacing in the open;
  (3) none: fight from where it stands and move 15 m by the band, as the PS2 [P 4.5].
- **Score** - non-negative terms that cannot cancel each other, as Killzone's [X4]; each 0 to 1; the weights are
  PROVISIONAL and tuned against B5 and B12:

| weight | term |
|---|---|
| 3 | cover with a line: 1 for LOW cover (stand to fire), 0.8 for CORNER cover (lean to fire); + 0.2 where an authored Cover hint's arrow points at the target within 60 degrees - the PS2's whole test [P 4.6] |
| 2 | near: 1 - travel / 40 m |
| 1 | cover from the other enemies the bot knows of |
| 0.5 | the bot's own bias, from its fixed number [P 4.4]: two bots in one place choose differently, one bot keeps a character |

- **Confirm** the winner with one ray from its firing point to the target; on a miss the next two.
- **Hysteresis** (card HYST, 2026-10-28): a query that may replace the position held (the 5-12 s timer, the open
  position's ask for cover) scores the held one beside its candidates, and another is taken only when it scores
  15 % more; each score term is held to 0-1. A walk that from inside the band closes on the target by over 4 m and
  40 % of the way to it, heading at it, is filtered out (The Last of Us' "rush forward to take cover").
- **Keep it** while its conditions still hold - reserved for this bot, cover from the target, inside the band, not
  compromised - asked each think with no scoring (the "still good enough" re-check [X8]). A position where the
  bot was hit twice, or that its target has walked round, is compromised and left (D7).
- **Cost:** 32 candidates of bit tests and 1-3 rays (0.94 us each [M6]) per query, a query every 2-5 s per fighting
  bot at most. Halo's firing-point rays were "about 60% of all AI cost" [X1]; here the cover is in the table.
- **At the position, the PS2's cycle** [P 4.6]: hidden 2-4 s in the primary stance, firing 2-6 s in the secondary
  stance or from a lean step; another qualifying position after 5-12 s. This cycle, not a random shuffle, is what
  keeps a bot from being a statue (the maintainer's decision 3; Halo's designers: "Random factors in behavior are rarely
  good" [X1]).

Why not the PS2's test alone: the arrow is honest (72 % of standable Cover hints have something solid ahead [M6])
but hints are missing where the fights are - 103 of 185 posts have no covered Cover hint within 30 m, and with the
barrier-edge points 154 of 185 have five or more covered positions [M6]. Killzone's bots score waypoints the same
way, from a cover table computed ahead of time per waypoint, stance and direction, trusted when it says "cover"
[X4]; F.E.A.R.'s soldiers "always try to stay covered" and only its coordinator keeps two from one node [X6].

### 4.7 Mover - the PS2's rules and one avoidance; the only writer of the move sticks, stance, roll and jump
In: the MoveRequest and the Navigator's current sub-goal. Its own state: the heading held, the side it is going
round on and round what, an arrived latch, the progress anchors.
1. **Arrive and stand.** Inside the arrive radius the stick is zero and stays zero until the request's destination
   moves more than 0.5 m or the behaviour changes. The last step is scaled so it does not overshoot [P 1.5]; a step
   under 3 m is walked at half stick [C JITR].
2. **Follow.** Full stick toward the aim point in the soldier's own frame while the body turns to the AimRequest's
   target - it strafes and backpedals - or to the path when there is none [P 1.5]. No sprint, no walk / run choice.
3. **Barriers, every step, from where the bot stands** [P 2]. If the segment to the aim point crosses a soldier
   barrier grown by the body radius + 0.5 m, aim at that barrier's silhouette corner on the committed side. The
   side is chosen once per obstacle - the shorter way round - and kept until the obstacle is behind. With several
   in the way the corner walk's A* [C `nav::detour`] gives the order, and each step re-aims: the PS2 stores no
   corner path. A bot never walks INTO a barrier and a goal inside one is refused; a bot that finds itself in one
   (spawned there, pushed, blasted) leaves by the nearest edge first. The PS2's bots are inside one for 0.3-1.2 %
   of their time [M3].
4. **Everything else in the way is seen before it is touched (D3).** A parked vehicle's or a turret's footprint is
   a rectangle; a body the safety probe hits where no barrier lies is a disc remembered for 3 s; both are passed
   by their corners, like barriers. A soldier or a moving vehicle is a disc with a velocity, and a standing bot a
   disc with none. Those are avoided in velocity space, Detour's way [X10]: every 3rd tick the Mover
   scores 15 candidate velocities - 7 directions at full and at half speed, fanned about the wanted heading, and
   "wait" - by four costs: the distance from the wanted velocity, the distance from the velocity it has now, a
   side cost, and 1 / time to collision within 2.5 s among at most 6 neighbours inside 4 m (Detour's own cap; the
   99th percentile here is 2-4 [M4]). The second and third costs ARE the hysteresis. Detour's weights (2.0, 0.75,
   0.75, 2.5) are the starting values. ORCA's exact solution [X9] is not the choice: section 15 says why.
5. **Head on:** both keep right (the side cost's default, which "reduces velocity selection flickering a lot in
   head-to-head and overtaking situations" [X10]); an exact tie goes by unit index.
6. **The safety probe.** Off ground the plan has checked - a leg's corridor between two hubs less its barriers, or
   a segment the Tactician validated when it chose the position - one capsule sweep and one floor test 2 m ahead
   every 5th tick (today's `way_ahead` and `floor_ahead` [C `steer.rs`], kept for this). A hit becomes a remembered
   disc; a missing floor is a wall.
7. **Progress.** The PS2's stuck test - under 2 m from an anchor in 3 s, or no progress to the node for 5 s [P 2] -
   reports Blocked to the behaviour, which goes to Recover. No random walk (the PS2's cure); no slide, flip or fan;
   no 5 m side-step off a timer.
8. **Stance, roll, jump** are asked for by the behaviour (a position's stance, Evade's roll, Recover's jump at a
   low wall) and pressed only here, under today's edge and lock rules [C the stance controller in `think`].

### 4.8 Gunner - kept, in one place; the only writer of the look sticks and the trigger
- **Target:** the top of the threat list with the PS2's hysteresis (1.5 s, +10 [C `perception::choose_target`]);
  the tier adds priorities (section 7).
- **Aim:** the tracker and the tracked-position error (re-ai-difficulty 4.2-4.3 [C]) - unchanged.
- **Trigger:** the tier's cone and reaction time (D9; today 2 degrees, and 5-15 ticks from the contact's first
  sighting or hearing), bursts and taps [C STTAP], no shot with a friend in the line [C FFLN; D4], under the
  behaviour's fire permission (free / hold / at a point).
- **Weapon and grenade:** card GRAI - the PS2's choice (the best in-range weapon by damage a second against the
  target's kind, the grenade window, one thrower per target per team [P 4.3]). The claim is an attack token [X12].
A seated bot's gun stays `gunner.rs` [C].

## 5. Commitment - what stops the stutter

| decision | asked again | kept at least | changed only when |
|---|---|---|---|
| the post | by the commander's rules | until done | the PS2's four triggers (re-conquest-goal 0) |
| the role | once a second | 20 s | its task ends |
| the behaviour | every think | its own minimum (4.4) | an allowed interrupter; one change per think |
| the position | its conditions every think; a new query on a failure, else after 5-12 s | until reached, then its hide / fire cycle | a condition fails: reservation, cover from the target, the band, compromised; or another scores 15 % more (card HYST) |
| a MoveRequest's destination | every think | - | it moves more than 0.5 m |
| the side round an obstacle | when a new obstacle is met | until that obstacle is behind | a dead end, once |
| the heading | every 3rd tick | - | turning costs, reversing costs most |
| arrived | - | until the request changes | - |
| the aim target | every think | 1.5 s | another scores 10 better |
| a snipe stand, a turret seat | every think | 15 s, 10 s | their own exits |

The maintainer's Kamino scene under these rules: two droids fighting from a hub would each hold a different reserved position
at least 2 m apart, stand there through a hide / fire cycle, and move only to another reserved position after
5-12 s or when hit twice; neither could be handed a new goal by a burst, a friend's line and a dodge in one second,
because none of the three writes a goal any more: only the Tactician does, and it keeps its answer while the
position's conditions hold.

## 6. The maintainer's three annoyances with the originals

### 6.1 Turrets are a bot trap -> a seat has a purpose and five ways out (D11; card TURR)
The PS2 [P 7, 10a]: a bot within 50 m is offered every free seat; it takes a turret only with a live threat whose
last point lies in a weapon arc (no range test); it leaves after 10 s with NO live threat - and any remembered
enemy, an enemy out of range, or an enemy turret 100-180 m away resets that clock. 15 of 32 seated spells on Kamino
ran over a minute; a third to a half of a gunner's fight was another turret. The port: bots never board a
stand-alone turret (0 % of their time in a seat on Kamino and Polis Massa against 9.4 % and 18.0 % [B]).

The design: a seat is a position of kind Seat, offered by the Team board (role Gunner, the nearest free bot within
the PS2's 50 m) when its **purpose test** passes:
- (attack) an enemy the team knows of is inside a weapon arc AND inside the weapon's range AND the muzzle has a
  line to him; or
- (defence) the turret is within 40 m of a post the team owns that has an enemy or neutral post as a neighbour on
  the route graph, and an enemy was seen within 60 m of that post in the last 30 s.

The gunner gets out when any of these holds:
1. 10 s with no target the turret can HIT - in an arc, in range, the line clear. The PS2's clock, but only a
   hittable target resets it.
2. 45 s in the seat with no shot fired in the last 20 s.
3. Hit from outside every arc [P 7.3], or the turret under 30 % health (the PS2 has no bail-out for turrets).
4. The team is in the endgame (10 tickets or fewer, or 3 or fewer alive): out, and hunt.
5. The purpose test has failed for 10 s (the fight moved; the post was lost).

After leaving, that turret is closed to the bot for 20 s (the PS2's unboard cooldown [P 7.3]). Boarding is a walk
with an outcome: a seat the walk cannot reach is closed to that bot for 60 s (BF1's bots stand for ever, BF2's
30 s limit restarts itself [P 7.2, 10c]). A turret fights a turret only inside its weapon's range. Coruscant's
outside turret (not captured by SAIR) is the plain case: no target in range and no front-line post near means
nobody boards it.

### 6.2 Dead parts of maps, the same hotspots -> flank routes and positions, not a weight tilt (D5; card FLNK)
The PS2 [P 10b]: a bot walks the authored route table; every hub the PS2's bots never visited lies off every
authored post-to-post route (Kamino 45 of 142 hubs).
- **The weight tilt the maintainer proposed has almost nothing to tilt.** Toward a post, the table offers a second next hop
  at 0-2 % of its entries on ten maps (none at all on Mygeeto and Polis Massa) [M5]. "You just know where everyone
  is going" is the table having one way.
- **Other ways do exist in the graph.** A route that shares at most half its connections with the authored one and
  is no more than 35 % longer exists for all 20 post pairs on Mygeeto, 22 of 30 on Tatooine Dune Sea, 13 of 18 on
  Yavin Arena, 18 of 30 on Polis Massa and the Death Star, 7 of 12 on BF2 Hoth, 13 of 29 on Utapau - and for only 3
  of 17 on Kamino (its bridges) and 2 of 20 on Naboo Plains (8 hubs of 39 m: there the hub's own disc is the
  spread) [M5]. Not every one crosses unused ground: on Mygeeto 8 of the 20 only reuse the ways authored for other
  post pairs, and 8 touch a hub the authored routes never use.
- **The rule:** at load, per ordered post pair, the best such route is stored. Per match a seeded draw sets each
  pair's flank share between 10 and 30 % of its attackers (the maintainer's "each match prefers slightly different ways").
  The Team board gives the Flank role to that share, never in a group under 3; everyone else walks the authored
  route, so the main way always keeps 70 % or more and no map is spread thinner than that. Unreal Tournament
  varies its attacks the same way - squads "pick an assault path at random (weighted by their priorities)" [X14];
  Killzone's bots penalise areas another squad already routes through [X15]. A later step, if the bars allow it:
  the share leans to the flank while friends die on the main route - a decaying count per hub, the Counter-Strike
  bot's "danger", which "causes bots to choose different routes each round" [X5].
- **How much of the dead ground it reaches:** 3-37 % of the off-route hubs on nine maps (Polis Massa 41 of 115,
  Kamino 20 of 75, Mygeeto 6 of 50, Utapau 1 of 35) [M5]. The rest is reached, as on the PS2, by positions: Cover
  and Snipe hints, turrets, and now the generated cover - the Tactician's 40 m reach is the PS2's own [P 4.6].
- **The guard rails are the bars:** contact, deaths a minute and the hotspot share must not fall (B5-B7).
  Pandemic concentrated the fights on purpose [X13]. Today the port's hotspots are WEAKER than the PS2's (19 %
  against 42 % on Kamino [B]), so the fights have to concentrate first (card FGHT) and FLNK comes after it.
- Unit counts per map are card UCNT, after PERF, as ordered.

### 6.3 BF1 bots that just stop -> no walk without an outcome, no behaviour without an exit (D6; cards MOVR, RETW)
The PS2 [P 10c]: a BF1 walk that fails is never retried, and the board agent has no timer; BF1's stuck rescue needs
a live path, so it never fires for a failed walk; only an enemy in view, a passing shot or damage wakes the bot.
BF2 added limits that a failing walk resets.
- **Every walk has an outcome** (4.5) and the behaviour that asked must handle Failed. A behaviour's table row is
  not complete without its exit and its time limit (4.4).
- **Recover is a ladder, not a random walk:** (1) plan again from the hub the bot stands in; (2) walk to the
  nearest hub it can stand level with, along the floor (card RETW); (3) give the goal up - the Tactician offers
  another position, or the commander draws again without this post; (4) after 120 s the recycle [C RCYC], which
  stays as the last resort and is counted, by spot, as a defect.
- **Unseen, not recycled** (D23, card UNSN): where step (4) would recycle a bot on the level, it is moved to a hub of
  its route past the failing link (within 60 m, on its floor, clear of bodies and soldiers) while no human player has
  a line to the bot or the spot; with a human watching, the recycle stays. Every move is still a defect in the census.
- **Failed links are remembered** (D20, card ARCM): step (1)'s re-plan does not draw again an arc whose walk failed
  twice running; the team drops an arc three of its bots failed; a back-and-forth at the waypoint fails the window at
  once.
- **The endgame hunts** (10 tickets or fewer, or 3 or fewer alive): nobody guards, snipes or mans a turret.
- **The sweep counts it** (B9): alive, on foot, not fighting, under 2 m in 20 s, and the behaviour is not Guard,
  Overwatch, Capture or Man. BOTM measures the PS2's own "idle 20 s and not fighting" at 3-21 % [B].

## 7. Difficulty as competence (D15; cards TIER then TCMP)

The PS2 [P 8; re-ai-difficulty 0, 9.2]: one skill number per team (BF2 enemies 4 / 14 / 17, the human's team
mates 6 / 11 / 11) feeding the aim tracker, the error, the bursts and the grenade chance; three sets of fight-mode
weights; at Elite the vehicle critical-hit zones for AI attackers. The port [C `skill.rs`]: skill 8 / 17 / 20 /
20 for both teams through the PS2's curves, plus tier columns of its own. The table keeps the PS2's skill curves
and adds what each tier DOES. Every number is OURS and PROVISIONAL; the feel is the maintainer's.

| | Easy | Normal | Hard | Elite |
|---|---|---|---|---|
| decides every (Behaviour, Tactician) | 1.0 s | 0.5 s | 0.33 s | 0.33 s |
| first shot after first sight (today's column) | 0.50 s | 0.30 s | 0.20 s | 0.17 s |
| aim sharpens the longer its target stays in view | the PS2's error decay by skill, kept | | | |
| bots that may fire at one human at once | 2 | 4 | no limit | no limit |
| hit tokens per human (card TOKN, built: the rest fire beside him; held 5 / 4 / 3 s; near miss 1.8 / 1.4 / 1.0 m) | 1 | 2 | 4 | off (6 pending the user) |
| fires when the aim is within | 6 deg | 3 deg | 2 deg | 2 deg |
| bursts beyond 40 m | full | full | short (2-3) | short (2-3) |
| takes a covered position, per opportunity | 25 % | 50 % (the PS2's draw) | 75 % | 90 % |
| fires from the hidden stance (the PS2 always does) | yes | yes | no | no |
| leaves a compromised position | never | after 8 s | after 3 s | at once |
| evasive step when aimed at (the PS2's zig-zag test, 18 deg) | 10 % | 30 % | 50 % | 65 % |
| roll when hit (today's column) | 20 % | 35 % | 50 % | 60 % |
| grenade | never | the PS2's window (3 s in 30) | + at a cluster, at a covered target | the same |
| target choice | nearest | nearest | + who aims at it, the capturer on its post | + the wounded, the gunner |
| view distance (today's column) | x 0.8 | x 1 | x 1.25 | x 1.5 |
| remembers a lost target; walks to its last point | 2 s; no | 3 s; no | 5 s; yes | 8 s; yes |
| passes contacts to friends within 20 m | no | yes | yes | yes |
| may take the Flank role | no | yes | yes | yes |
| covers a capturing friend | no | no | yes | yes |
| falls back when hurt | no | no | yes | yes |

For every tier: no shot through a friend, one bot per position, the avoidance, the Recover ladder. Never: a damage
multiplier, sight through walls, a position the bot or its team has not seen. Two columns of today's table need
The maintainer's word because PLAN's locked feature set names them: the **damage into a human** (x1.15 Hard, x1.3 Elite -
"entirely ours" [C `skill.rs`]) is dropped by default (decision 2); the **preference for a human target** (x1.3,
x1.6) stays - a human opponent shoots the dangerous player first too. Three cautions from outside: Halo REMOVED
its pause between seeing and reacting because players "assumed this Feature was a Bug", and its enemies grow more
accurate the longer the player stays out of cover [X1] - which is what the PS2's decaying aim error already
does, so the first-shot delay stays short and is a feel item for the maintainer; the Counter-Strike bot's Expert is "very
good (but still human)" [X5]; and limiting how many shoot at the player at once is the oldest fairness lever there
is (Half-Life gave each squad two attack slots [X11]; Amalur scales the budget with the difficulty [X12]) - it is
PLAN's "per-human pressure" layer, here only at Easy and Normal. Card TIER measures today's four tiers over 8
seeds before TCMP changes anything.

**2026-10-28 (the maintainer), card DRETIRE [ours]:** the damage into a human is gone at every tier ("Dying faster from a few
bullets is a cheap way to do this. We should make the competition actually stiffer, not just give the enemy better
weapons"): no `Tier::human_damage`, no `human_damage_on` host option. The human-target preference (pressure) is a
different lever, whom a bot picks, and stays.

**2026-10-28 (the maintainer), card DTEAM [ours]:** only humans change bot skill, per team (`skill::team_skill`). A team's
human surplus (its humans minus the other's, never below 0, capped at 3) lowers its own bots by the tier's offset
per surplus human: Easy 0, Normal -1 (floor 14), Hard and Elite -3 (floor 11, the PS2's teammates); they play the
row that skill falls in, so Elite's extras are off on them. The other team keeps the tier's row; equal humans
(0v0, 1v1, 2v2) keep both rows. Set at each bot's spawn; a 0v0 round is hash-for-hash unchanged. No map or side
bias is touched. Measured with one stand-in human on team 1 (`SWEEP_HUMAN_TEAM=1`), 8 maps x 8 seeds, team 1's
wins: Elite 29/64 -> 15/64 (margin -13 -> -51), Normal 32/64 -> 30/64 (-1 -> -13).

## 8. SAIR's table, a verdict per row

MATCH = build what the PS2 does; KEEP = the port's stays; BEAT = a named deviation (section 9). Rows are SAIR 12's.

| item | verdict | layer | card |
|---|---|---|---|
| think rate | BEAT D1: one rate for every bot, by tier, the phase by unit index | all | MOVR, TCMP |
| orders | MATCH in shape: an order, one behaviour, interrupt and resume, one change per think | Behaviour | SPL11, FGHT |
| next hub | MATCH (as today); + D5 a flank route for a share | Navigator | FLNK |
| point in the hub | MATCH | Navigator | HUBP |
| leg steering, arrival | MATCH the leg aim (BF2's look-ahead for both games; built, MOVR2); the 1.0 / 0.5 m radii NOT taken (D18) | Navigator, Mover | MOVR, MOVR2 |
| barriers | MATCH - every step from where the bot stands, never entered | Mover | OBST |
| walls, props | BEAT D3: no sweep steering; a probe every 5th tick; a body with no barrier is remembered | Mover | OBST |
| other units | BEAT D3: avoided before contact, one side kept | Mover | OBST |
| stuck | MATCH the test (2 m in 3 s; 5 s), BEAT the cure D6 | Mover, Behaviour | MOVR, RETW |
| perception | KEEP (the cone, 10 m, six contacts); the PS2's "a soldier on the sight line hides the target" is not copied | Perception | - |
| hearing | MATCH: go to the noise within 30 m, at most 10 s; within 3 m fight | Behaviour | FGHT |
| a shot passing | MATCH BF2: a bot not fighting evades or rolls | Behaviour | FGHT |
| being hit | MATCH out of a fight (walk to the attacker's point); BEAT D7 in one (a compromised position is left) | Behaviour, Tactician | FGHT, TPOS |
| fight movement | MATCH the band, the 15 m moves, the give-up at range + 10 m; BEAT D8: the style by situation and tier | Tactician | FGHT, TPOS |
| cover in a fight | BEAT D2: scored positions, the PS2's cadence and cycle kept | Tactician | TPOS |
| fire | KEEP the port's tight trigger and reaction time, now by tier (D9) | Gunner | TCMP |
| weapons | MATCH; D12 grenades by situation at Hard and Elite | Gunner | GRAI, TCMP |
| vehicles as targets | MATCH (only with ordnance that can hurt them) | Gunner | GRAI |
| stances | MATCH through the position's stance masks (a third crouch when standing to shoot) | Tactician, Mover | TPOS |
| jets | MATCH, later | Mover | JETP (post-alpha) |
| own health, ammo | BEAT D10 at Hard and Elite | Behaviour | TCMP; PWAI for pickups |
| line of fire | KEEP FFLN (D4) | Gunner | landed |
| friendly fire | MATCH | - | landed |
| spacing, team sense | BEAT D4 | Team board, Tactician, Mover | TEAM |
| snipe hints | MATCH the search; BEAT D13 the stand | Behaviour | TPOS |
| at a post | MATCH BF2's 8-14 m every 20-30 s, to scored positions; BF1's Patrol hints where authored | Behaviour, Tactician | TPOS |
| turrets | BEAT D11 | Team board, Behaviour | TURR |
| vehicles | MATCH | Behaviour | WAI2 (landed), VBAIL |
| engineers, pilots; cloak, buff, auto-turret | MATCH | Behaviour | ENGR |
| difficulty | BEAT D15 | all | TIER, TCMP |
| the corpse shot | not built (the maintainer's decision 7) | - | - |
| failure | BEAT D6 | Behaviour | MOVR, RETW |
| map use | BEAT D5 | Team board, Navigator | FLNK |

Things the PS2 does only near the player - item detours, hint detours, path smoothing [P 1.3, 2] - apply to every
bot under D1.

## 9. The named deviations - a record, not a switch

No "classic bots" option (the maintainer, 2026-10-02). Each line: what the PS2 does (SAIR), what we do, why.

| # | the PS2 | the port | why |
|---|---|---|---|
| D1 | decisions 0.1-8 s apart by the human's distance, his view and the camera [P 1.2-1.3] | one rate for every bot, by tier (section 7) | a battle away from the player should be the same battle; splitscreen has two players; SAIR 13.1 |
| D2 | cover = a Cover hint whose arrow points at the target; no ray; fires while "hidden" [P 4.6] | positions scored by real cover and a real line, generated where no hint is (4.6) | 103 of 185 posts have no covered hint near [M6]; the maintainer: "no positional sense in a fight"; SAIR 13.4 |
| D3 | barriers only; other units after contact (a 5 m side-step); props never [P 2] | one avoidance for barriers, props, vehicles, turrets and soldiers, before contact (4.7) | the maintainer's OBST complaint; SAIR 13.11 |
| D4 | no awareness of friends: shoots through AI team mates, no spacing [P 6] | no shot through a friend (FFLN, landed), one bot per position, 2 m spacing, fire lanes, contacts passed on | the maintainer's Kamino snapshot (8 team kills in 19); SAIR 13.2 |
| D5 | the route table only [P 10b] | a flank share per post pair, drawn per match (6.2) | the maintainer's annoyance 2; SAIR 13.10 |
| D6 | a failed walk stalls (BF1) or restarts its limit (BF2); stuck = a random walk [P 2, 10c] | outcomes, the Recover ladder, no idle state, the endgame hunt (6.3) | the maintainer's annoyance 3; SAIR 13.9, 13.11 |
| D7 | a hit during an attack is ignored [P 4.2] | a position hit twice or walked round is left (by tier). BUILT first as card REACT (2026-10-11): a bot on a position that is hit draws once per burst (1.5 s) over its tier's row - stays 65 / 35 / 20 / 10 %, strafes 2.5 m 20 / 30 / 30 / 30 %, rolls aside 5 / 15 / 20 / 25 %, moves to a ring point round the target it sees (else a spot within 7 m) 10 / 20 / 30 / 35 % (`skill::Tier::react`, `behave::react`; PROVISIONAL, the maintainer's to tune). It answers the hit, never looks for the shooter | SAIR 13.5; the maintainer, 2026-10-07: "if it doesn't die after being hit, it just stays standing there". Measured, 8 seeds x 32 maps per tier (`lead_tmp/react/sw`): of a fighter's seconds within 2 s of a hit, standing (under 0.5 m/s) 75.9 -> 67.6 / 60.0 -> 49.5 / 55.8 -> 43.1 / 54.3 -> 40.0 % |
| D8 | one fixed fight style per bot from its number [P 4.4] | the style follows the situation and the tier; the number is only a bias | SAIR 13.3 |
| D9 | fires inside a 22.5 degree cone, no reaction time [P 4.9] | the tier's cone (2-6 degrees) and reaction time | already the port's; a tier lever; SAIR 13.6 |
| D10 | no rule on own health or ammunition [P 4.8] | Hard and Elite fall back when hurt | the maintainer: "retreat / heal"; SAIR 13.5 |
| D11 | a turret is kept while any threat is remembered [P 7.3] | a purpose test and five exits (6.1) | the maintainer's annoyance 1; SAIR 13.8 |
| D12 | grenades by the clock (3 s in 30) except at a cluster [P 4.3] | the same at Normal; by situation at Hard and Elite | SAIR 13.15 |
| D13 | a sniper stands 60-120 s at one hint (BF1 60-100 s) and after a fight usually returns to it [P 3] | 20-40 s without a target, then another hint | SAIR 13.12 |
| D14 | "lost sight: search the last known point" is dead code [P 4.6] | measured, not landed: Investigate (see 4.4 and FGHT’s landed/not-landed amendment) | SAIR 13.14 |
| D15 | difficulty = one skill number and three mode weights [P 8] | tiers as competence (section 7) | the maintainer: "better tiers" |
| D16 | nothing counts tickets or enemies left [P 10a] | with nothing to capture, or in the endgame, the team hunts; nobody waits | already the port's (HOLD, CONT); rounds must end |
| D17 | a unit is located like a goal - 3-D (BF1 hub sphere; both games' corridors and nearest arc), no level rule - and a route from an arc starts at the end its draw runs along, else the farther end [RE_LOG "Arrival and locate", audit 2026-10-03] | where a unit stands (a route's start, a hunt's quarry, a boarding, an order's goal) is located in XZ, the first containing corridor, the nearer end, with the level rule (`NavGraph::locate_unit_on`, `route::level_hub`); a post is located as the PS2 does | measured, card LOC3, 32 seeds x 32 maps against main 9f2aa09 (`lead_tmp/loc3/ab/perseed_A_C.txt`): the PS2's unit locate and start cost 1,315 recycles against 838, 1,022 of 1,024 rounds finished against 1,024, 12 maps worse beyond seed noise (BF2 Utapau kills / min 27.8 -> 16.9, BF1 Bespin Platforms 30.0 -> 19.7, BF2 Polis Massa recycles / round 0.72 -> 5.72), one better (BF2 Dagobah 2.50 -> 0.25). PROVISIONAL: to be run again when the mover cards have landed (card LOC3B) |
| D18 | stuck = under 2 m from an anchor in 3 s, or no progress to the node for 5 s; a node is reached at 1.0 m, the last at 0.5 m [P 2] | the 2 s progress window with its re-plan and side-step, six failed windows give the goal up; a node at 1.5 m, the last at 1.0 m (`ai.rs`: `STUCK_WINDOW`, `GIVE_UP_AFTER`, `ARRIVE_MIN_RADIUS`, `CORNER_ARRIVE`) | measured twice, each rule behind its own switch in one binary: card MOVR on the old wall slide, card MOVR2 on card OBST's steering - 24 seeds x 32 maps against main c0794eb, paired by seed (`lead_tmp/movr2/ab/c_*/seeds.md`, `tools/perseed_rec.py`). **The clock** (with the window's own cure): recycles 199 -> 196, deaths off the level ("lost") 21 -> 98 (t +8.6) and in death regions 201 -> 320 (t +5.3) - BF2 Mygeeto's bots stay on the wall-follow round `myg1_bldg_spiral_platform1` (hub 78) until they are up on it; POL1's staying path 12.5 -> 15.7 m. **Node 1.0 m**: alone it passes (recycles 199 -> 153, t -1.7; B1 / B2 better in 11 of 12 cells), on top of the PS2's leg aim it adds nothing (156 -> 162) and "lost" deaths double (19 -> 40, t +2.8 against main). **Last node 0.5 m**: POL1 pinball 3.55 -> 3.90 %, staying path 12.5 -> 18.1 m, reversals 1.36 -> 1.85. **Give up at the 3rd failed window, not the 6th**: recycles 199 -> 155 (t -2.3), but POL1's staying path goes over its bar (14.3 m) and 3 rounds of 768 do not end against 1. The leg aim of the same paragraph IS the PS2's now (recycles 199 -> 156, t -1.8). To be run again after cards OBSR, OBST2 and TPOS: the stays at a wall and the holders' shuffles are what a tighter radius and a slower clock lengthen [H] |
| D19 | a bot boards a building turret once its body sphere is within the two radii + 1.5 m (BF2 `FUN_002c42d8`) / + 0.5 m (BF1 `FUN_00244848`), 3-D, and waits at a turret its walk cannot reach (BF1 with no timer, BF2 behind a 30 s clock the failing walk resets) [P 7.2, 7.5] | the same reach (card TBRD; before it the port boarded from radius + 5 m + 2 m in XZ, about 10 m, and 190 of 320 BF1 Bespin 2 turret boards came from over 2 m below or above the gun); a bot inside the reach in XZ whose centre is over 2 m above or below the turret's origin (`ai::OTHER_FLOOR`) gives the claim up at once and the turret is closed to it, where it stood out the claim's 30 s | the maintainer, 2026-10-20: bots "teleport" into turrets from about 10 ft; standing under a balcony gun for 30 s reads as stuck. The turret's radius is still the 3 m placeholder (`VEHICLE_RADIUS`; the PS2 reads its collision sphere, not baked for buildings) [H]. Measured 8 seeds x 32 maps (`engine/artifacts/tbrd/README.md`) |
| D20 | a BF2 walk that fails restarts its limit and may draw the same arc again; BF1's stalls on it; the stuck test is a clock (under 2 m in 3 s, or no progress for 5 s) [P 2, 10c] | an arc whose walk fails twice running, or on which the bot goes back and forth (three turns of over 1 m toward and away from its waypoint inside 5 s with no new ground; not while it goes round a body), is closed to that bot for 60 s; an arc closed by 3 of a team's bots in turn leaves that team's route draw for the round; a bot drawn round one walks the fewest-arcs way without it to the same goal; with no other way the arc is kept (`ai::arcm`, card ARCM) | modern.md A1 (Pogamut's failed-link report, Halo 2's failure memory) and A2 (Pogamut's distance detector); measured 6 seeds x 34 maps against card WHY (`lead_tmp/botre/runs/ARCM/seeds`): time in Recover 1.52 -> 1.47 % of foot time (1.26 -> 1.11 % without BF1 Bespin 1, whose failing arc has no other way), unexplained standing (stand20) 10,124 -> 9,164, recycles 25 -> 19, stuck_max per-map median 180 -> 120 and max 600 -> 360, 204/204 finished. The detector waits while a bot goes round a body under its allowance (card OBST's one-side rule). Without the hub-by-hub detour, table and detour taken in turn went round in loops (BF1 Rhen Var 1: 32 recycles against 2) |
| D21 | BF2: a bot thinks at the next step on a threat entry for a HUMAN unit only, else on its interval; BF1 on its round [`re-ai-think-rate.md` 4, 7]; a timer move takes "another qualifying hint" [P 4.6] | a hit, an enemy bolt within 2 m or a new enemy in sight within 20 m wakes a bot at the next tick (at most every 0.1 s); the timer and the open spot's ask keep the held position unless another scores 15 % more; no cover reached by running at the enemy (`ai::wake`, `tactic::choose`, card HYST) | modern.md B1-B3; measured 6 seeds x 34 maps against card ARCM (`lead_tmp/botre/runs/HYST/seeds`): position flips per fight-minute 2.02 -> 1.71, unexplained standing (stand20) 9,164 -> 8,389, time in Recover 1.47 -> 1.36 %, B12 cover 15.27 -> 15.61 %, contact20 13.81 -> 13.44 %, 204/204 finished, recycles 19 -> 19 |
| D22 | BF1 Yavin Temple authors 51 Cover hints [`lead_tmp/botre/xbox_pc.md` 2.2] | it also has BF2 Yavin Temple's (973 on the same graph, hubs 85 / 85): 409 added at bake, the rest deep in a barrier, off the graph or on BF1's own (`bake::donor`, card COVR); Bespin Platforms and Rhen Var Harbor not (a fan conversion's data, unchecked) | the bot design's hint gap (M1, M6); measured 24 seeds of BF1 Yavin 1 (`lead_tmp/botre/runs/COVR/seeds`): fight time at a Cover hint 335 -> 5,965 s, contact20 10.2 -> 11.3 %, kills per foot-minute 0.98 -> 1.05, B12 17.6 -> 17.7 %, body failures 290 -> 861 (BF2 Yavin's own rate), 24/24 finished |
| D23 | a bot that cannot go on stays where it is (BF1) or restarts its walk (BF2); nothing moves it [P 2, 10c] | a bot the stall watch gives up on the level (card RCYC's recycle, 120 s without net progress or 10 failed windows at one place) is moved to the far end of its failing leg, else the next hub or two toward its goal (60 m at most, its floor, clear of bodies and soldiers, on the level, no death region), when no human player has a line of sight (the bolts' ray, from his eye or from his camera 3 m behind) to the bot or the spot: within 15 m any way he looks, beyond inside 65 deg of his look; else recycled as before (`ai::unseen`, card UNSN). A move costs no ticket and is listed like a recycle | the user, 2026-10-28: a recycle costs the team a ticket for our defect; measured 6 seeds x 34 maps against card COVR (`lead_tmp/botre/runs/UNSN/seeds`, no human in the sweep): recycles 19 -> 6 with 11 moves (BF1 Bespin 1's arc 150 stall: 11 recycles -> 0, 9 moves of 7-29 m), 204/204 finished, stuck_max per map median / mean / max 120 / 158 / 360 both, Recover 1.36 -> 1.35 % |
| D24 | every bot's shot at a human can hit; the aim error, bursts and skill decide [P 4.9, 8] | hit tokens per human by tier (the user's table after the first play: Easy 2, Normal 3, Hard 5, Elite 7; Elite holds 2 s and misses by 0.7 m): only a holder's bolts and blasts hurt him; the others still fire, aimed 1.8 / 1.4 / 1.0 m beside his chest, a little high, and throw no grenade at him; a token is held 5 / 4 / 3 s, then goes to the nearest enemy bot on foot targeting him with a clear line (weighted by how nearly it aims at him; lowest index on a tie), the holder keeping it when nobody waits; bot-vs-bot and vehicle fire untouched. A holder's shot still goes through the aim error model and its hit through D12's multiplier (`ai::tokn`, card TOKN) | the user, 2026-10-28; Half-Life's squad attack slots and Amalur's per-difficulty budget (section 7). Measured with the arena's shadowing idle human (`ARENA_HUMAN=shadow`, 10 maps x 4 seeds x 9,000 ticks, `lead_tmp/botre/runs/TOKN`): damage taken per minute Easy 435 -> 171, Normal 573 -> 221, Hard 1,256 -> 1,275 (t +0.2; hits -8 %, t -0.8), Elite unchanged (every run identical) |
| D25 | a bot targets the nearest contact it perceives, with the tier's pressure weight for a human; nothing gangs bots up on one human [P 4.9; `analysis/difficulty-proposal.md` 1] | focus fire at Hard and Elite: a human's token holders (D24) are his focusers - a bot that SEES him (its own eyes at its last think; heard or told is not enough), within its reach (45 m on foot, 120 m with a scoped rifle, 90 m against a human in a vehicle or turret with a weapon that hurts it, never without one; only heavy weapons: no soldier), not capturing a post or defending one under threat, and holding no other human's token may take one and makes him its target; it keeps him 3 s after losing sight and walks to his last seen point within 25 m; no focus in a human's first 3 s after a spawn; the Tactician's spaced try keeps focusers 30 degrees apart round him; card FFLN's no shot through a friend stands (`ai::dfocus`, card DFOCUS) | the user, 2026-10-28. Arena proxies, 10 maps x 4 seeds x 9,000 ticks (`lead_tmp/botre/runs/DFOCUS`): Easy / Normal every run identical; damage per minute Hard +16 % (shadow, t +1.9; lone, t +1.8), Elite +2 % shadow (t +0.2), +20 % lone (t +2.5) |

**Deliberately NOT changed** (so nothing improves by accident): which post is drawn; the route-table walk; the
range band and its 15 m moves; go-to-the-noise at 30 m for at most 10 s; a fight given up at weapon range + 10 m;
the weapon choice and the one-thrower claim; an AI never hurts a human friend; the aim tracker, the error model
and the burst shapes; the 50 m vehicle offer; the 10 s length of the turret clock; one unit per hint with a 2 s
stamp; the snipe search; walking speeds (no sprint); no prone in BF2.

## 10. The bars - what card BOTV checks on main

"Today" = `lead_tmp/botm/post_jitr/seeds.md` (main ac605ca, 4 seeds) unless said; the PS2 column is BOTM's.
EQUAL = inside a band round the PS2's number; BEAT = deliberately better; maps: KAM1, POL1, DEA1, KAS2 until BOTV
adds PS2 captures on the showcase maps.

| # | measure | the PS2 | today | bar | kind |
|---|---|---|---|---|---|
| B1 | pinball windows | 1.2-3.3 % | 0.5-5.1 % | at most max(1.5 x PS2, 2 %) per map; the maintainer's F9 scene: no bot over 6 reversals in 10 s within 20 m | BEAT |
| B2 | a staying bot: path and reversals in 10 s | 1.3-9.3 m; 0.36-1.07 | 4.1-20.0 m; 0.72-6.47 | path at most max(3 m, 1.5 x PS2); reversals at most 2 x PS2 | EQUAL |
| B3 | a teammate within 1.5 m | 2.0-9.3 % | 8.7-17.9 % | at most the PS2's on each map | BEAT |
| B4 | team kills of kills | 12.1 % (KAM1, FFLN) | 0.6-3.1 % (FFLN) | at most 3 % on every map | BEAT |
| B5 | an enemy within 20 m | KAM1 15.1, DEA1 18.6, POL1 35.9, KAS2 5.3 % | 2.2, 2.7, 18.6, 4.4 % | x 0.67-1.5 of the PS2 | EQUAL |
| B6 | deaths a minute | 20.6-35.8 | 14.0-35.4 | x 0.67-1.5 of the PS2 | EQUAL |
| B7 | the three busiest cells' share of deaths | KAM1 42.3, POL1 31.6, DEA1 27.3, KAS2 18.8 % | 19.0, 22.9, 40.7, 7.7 % | at least 0.67 x PS2 | EQUAL (one side) |
| B8 | time in a seat (KAM1: turrets; POL1: turrets and hovers) | KAM1 9.4, POL1 18.0 %; 15 of 32 seated spells on Kamino over 60 s [P 7.3] | 0 % on both | KAM1 3-10 %; no seated spell over 60 s without a shot in its last 20 s; none seated in the endgame | BEAT |
| B9 | unexplained standing (6.3) | "idle 20 s, not fighting" 3.2-21.1 % [B] | new counter | at most 0.5 % of foot time on every map | BEAT |
| B10 | stalls | - | 5.39 % of 10 s windows (OBST, 7b64fcd) | at most 2 %; the body, spot and post classes each halved; no spot recycles on 2 or more seeds | BEAT |
| B11 | hubs never visited | KAM1 32 %, POL1 28 % [P 10b] | not measured | at most the PS2's, with B5-B7 holding | BEAT |
| B12 | fight time at a position with cover from the target | at Cover hints: 21 % (Kamino), 49 % (Polis Massa) [P 4.6] | new counter | at least 20 % at Normal on every showcase map | EQUAL or better |
| B13 | cost at 33 units | - | 0.49 ms (KAM1), 0.75 ms (MYG1) a tick; steering 65 / 71 % of the samples [M2] | at most half; steering under 25 % of the samples | BEAT |
| B14 | rounds | - | 256 of 256 over 8 seeds (the gate) | the same; lost and recycled not up | EQUAL |

Reported, not gated: standing still, 20 s spells and the running share - the PS2's values there come largely from
its slow think away from the player (D1). Tiers: kills a minute and hit rate ordered Easy < Normal < Hard <= Elite
over 8 seeds (card TIER's measure).

## 11. Today's code: kept, moved, deleted

Lines are whole files at e336f2e. "Moved" = the same code under its owner; "deleted" = the rule goes.

| file | lines | what happens |
|---|---|---|
| `core/src/commander.rs` | 1,887 | KEPT (the draw, the plan). `hint_spot`, `nearest_hint` and the stance masks MOVE to the position table. The bookkeeping `squad` field goes; the Team board replaces it |
| `core/src/perception.rs` | 889 | KEPT. Its candidate loop reads the unit grid; `choose_target` gains the tier's priorities |
| `core/src/skill.rs` | 573 | KEPT. `Tier` gains section 7's columns; `human_damage` goes if the maintainer agrees |
| `core/src/squad.rs`, `ai/command.rs` | 1,009 + 386 | KEPT. `squad::steer` writes a MoveRequest through the Follow / Hold-area behaviours instead of route fields |
| `core/src/nav.rs` | 1,838 | KEPT. Adds the barrier grid, the flank table, HUBM's locate |
| `ai/route.rs` | 1,397 | KEPT as the Navigator (`retarget`, `plan_leg`, `post_point`, the hub floors). `hub_waypoint` becomes HUBP's draw; `walk_point` and its 5 m look-ahead give way to the PS2's projection rule |
| `ai/steer.rs` | 952 | `steer_dir`'s slide, flip, fan and back are DELETED (`ai/avoid.rs` replaces them). `way_ahead`, `stop_ahead`, `floor_ahead`, `steep_ahead`, `ramp_clear` are KEPT as the safety probe and the black box's stop test. `barrier_ahead` folds into the barrier rule |
| `ai/brain.rs` | 2,695 | `think` is SPLIT by owner. To the Gunner (`ai/gunner_foot.rs`): the aim tracker, `seen_aim`, `fire_discipline`, `tap`, `burst_len`, `pause_ticks`, `slew`, `friend_in_line`. To the Mover (`ai/mover.rs`): the walk block, arrival, the stance controller, the roll. To the Navigator: `advance_route`, `hub_reached`. To the Behaviour (`ai/behave.rs`): the hunt target, `hazard_flee` (Evade), the lost return and `lost_jump` (Recover), the dispatch to `ride`. DELETED: `hold_at_hub`, `start_hold`, `spot_free`, the shuffle, the strafe on a burst, the close hold, the step out of a friend's line (the Tactician and the reservations replace them); the stuck window's side-step and re-plan (the PS2's stuck test and Recover replace them). `stand_ok` and `roll_clear` move to the position validation |
| `ai/state.rs` | 976 | `AiState` regrouped by owner: Behaviour {which, since, at least until, what it interrupted}; MoveRequest; Position; MoverState {heading, side, obstacle, arrived, anchors}; GunnerState {today's aim and trigger fields}. `HoldState`, `slide`, `slide_last`, `slide_keep`, `strafe_until`, `strafe_side` and `Route::sidestep` go. `SteerPick` becomes the Mover's status for the black box |
| `ai/stall.rs` | 408 | KEPT: the last resort (RCYC), counted in B10 |
| `ai/ride.rs`, `ai/walker_drive.rs`, `ai/gunner.rs` | 621 + 346 + 99 | KEPT, entered through Board / Ride / Man |
| `ai/draw.rs` | 350 | KEPT (new salts) |
| `core/src/ai.rs` | 693 | the constants follow their rules: the 20 `HOLD_*`, `SLIDE_KEEP`, `SIDESTEP_*`, `FAN_STEP`, `ARC_LOOKAHEAD`, `STUCK_*` go or are replaced; the header is rewritten as this note's summary |
| `sim/step.rs` `separate_units` | - | KEPT as the physical backstop (the PS2 has a contact push too [P 2]) |
| `game/src/blackbox.rs`, `unit_probe.rs` | - | the probe row gains the behaviour, the position, the Mover's status: the maintainer's F9 shows the new brain |
| new | - | `ai/behave.rs`, `ai/mover.rs`, `ai/avoid.rs`, `ai/tactic.rs` (the position table, the scoring), `ai/team.rs` (the unit grid, reservations, roles), `ai/gunner_foot.rs` |

## 12. The build order - 16 cards, each landable alone, the sweep green at each

All wait for the maintainer's read through `deps: BOTD`; all but ENGR and PWAI (value 3) are in card BOTV's `deps`. "New"
cards were opened by this session; the others were on the board, held for this note, and carry a "BOTD
(2026-10-03)" section now.

| # | card | what | after | accepted by |
|---|---|---|---|---|
| 0 | HUBM (not held) | locate skips a hub with no arc for the mover | - | its own (TAT1 hunting 52 % -> about 1 %) |
| 1 | SPL11 | THE SEAMS: `think` split by owner into `behave` / `mover` / `gunner_foot` with one MoveRequest and one AimRequest a tick; no behaviour change | BOTD | every gate PASS with nothing declared; M7's census reads one stick writer |
| 2 | HUBP | the waypoint is a point of the next hub's disc | SPL11 | its own: a teammate within 1.5 m at least half-way to the PS2's on the four maps |
| 3 | MOVR (new) | THE MOVER: the arrived latch and the 0.5 m deadband; the PS2's leg rule and arrival radii; the PS2's stuck test; the Recover ladder (steps 1, 3, 4); phases by unit index | HUBP | B1, B2, the maintainer's scene, B14 |
| 4 | OBST | THE AVOIDANCE: barriers every step and never entered; remembered bodies; vehicles, turrets, soldiers; the sweep becomes the probe; `steer_dir` deleted | MOVR | its own census bars, B10, B13 |
| 5 | LLEG | a last leg finds the ramp or the doorway | OBST | its own |
| 6 | RETW | Recover's step 2: back along the floor | MOVR | its own |
| 7 | FGHT | THE FIGHT: Fight / Investigate / Evade with interrupt and resume; go to the noise; the band; the give-up | MOVR | its own (KAM1 contact at least 12 %, POL1 at least 28 %), B5-B7 |
| 8 | TPOS (new) | THE POSITIONS: the table at load, the Tactician, reservations, the hide / fire cycle, Capture / Guard / Overwatch; card HOLD's rules deleted | FGHT, OBST | B12, B2; the table's counts equal M6's |
| 9 | TEAM (new) | THE TEAM: the unit grid, roles with capacities, fire lanes in the score, contacts passed on, cover for a capturer | TPOS | B3, B4; the grid equals the brute-force loops |
| 10 | TURR (new) | turrets with a purpose and five exits | TEAM | B8; one scene test per exit |
| 11 | FLNK (new) | flank routes and the per-match share | TEAM, FGHT | B11 with B5-B7 holding; the table's counts equal M5's |
| 12 | GRAI | the PS2's weapon choice and grenades | FGHT | its own |
| 13 | TIER, then TCMP (new) | measure today's tiers; then tiers as competence | TPOS, GRAI | the four tiers ordered over 8 seeds |
| 14 | ENGR | engineers, pilots, mines, cloak, buff | GRAI, TEAM | its own |
| 15 | PWAI | pickup detours (every bot, D1) | MOVR | its own |
| - | BOTV | the bars of section 10 on main | all | every bar PASS, or the maintainer's waiver |

Each card is one seam of section 3, so each deletes what it replaces in the same landing: nothing is built beside
the old rule and left switched off.

## 13. Risks and budgets

**Cost** [M2, D]. Today a bot's `think` costs about 2.9 us (NAB1) to 18 us (MYG1) a tick, 2.4-16 us of it the
steering's probes (the ms a tick over 33 units, times the profile's shares); a line-of-sight ray test costs
0.94 us [M6].

| layer | budget, us per bot per tick | how |
|---|---|---|
| Mover | 3 | the barrier rule on a grid; 15 candidate velocities x at most 6 neighbours every 3rd tick; the probe every 5th tick and only off checked ground |
| Gunner | 0.5 | as today |
| Perception | 1 | six rays a think; candidates from the grid |
| Behaviour, Tactician, Navigator | 0.5 | a table; 32 bit tests and 1-3 rays every 2-5 s; a draw at a hub |
| total | 5 | 64 bots: 0.3 ms a tick; 1,000 bots: 5 ms of the 33 ms tick |

The budget is a target [D]; B13 gates its first half and card PERF measures the rest. Four loops are O(units
squared) today and must go through the unit grid before 500 a side is thinkable: the hunt target in `think`,
`spot_free`, `friend_in_line`, and perception's candidate scan [C].

**Determinism.** Every draw is a per-unit hash (`ai/draw.rs`), as today. `think` stays a pure function of `&Sim`
that writes only its own state: a reservation is ASKED for in `think` and GRANTED in the write-back in unit-index
order; the loser asks again at its next think. The Team board walks units by index. The position and flank tables
are pure functions of the map built at load with `fmath` (the census used the host's `sin` / `cos`; the build must
not), identical on every machine and outside the hash, like the nav graph. The heading candidates use constant
sines and cosines.

**No allocation in a tick.** Reservation stamps: one fixed array per position. The unit grid: a counting sort into
buffers sized at load. Neighbours: at most 6, on the stack. The tables: built at load.

**What could go wrong.**
1. *The seam split is not hash-identical.* `think`'s order of evaluation has to be kept exactly. If it cannot be,
   SPL11 lands with `sim; sweep; arena` declared and a sweep inside the seed noise, and says so.
2. *The barrier rule meets a barrier the collision disagrees with* - a door barrier the mission opens later (only
   the load-time switches are replayed [C BNDS]), a hub disc lying partly in a barrier. The PS2 re-draws such
   waypoints [P 2]; OBST's census lists the stalls against a body that are NOT within a metre of a barrier (16 %
   today [M3]) as its residual.
3. *Narrow corridors* (Tantive IV, Polis Massa, Coruscant: median hub radius 1.3-1.5 m [M1]): two files meeting
   head on. "Wait" is a candidate and both keep right; a scene test sends 8 bots each way through a 3 m corridor.
4. *Open maps with few positions* (BF1 Naboo Plains: 77 barriers; BF1 Hoth: 3 Cover hints [M1]). The fallback is
   the PS2's own band movement; B12 is judged on the showcase maps first.
5. *Bots that live longer* in cover lengthen rounds. B6 (deaths a minute within x 0.67-1.5) is the guard, the
   tiers' cover chance the lever.
6. *The PS2 column is thin:* four maps, one capture each, 160-230 s [B]. An 8-seed per-map rate swings about 12 %.
   BOTV should add PS2 captures on two showcase maps before judging the EQUAL bands.
7. *The Team board and the Tactician are where a design balloons.* Both are capped here: five roles, no formation,
   no plan; four weights, 32 candidates, three rays.
8. *One file family.* Every card edits `core/src/ai/`, so the picker runs them one at a time: 16 cards are 16 or
   more sessions before BOTV.

## 14. Checks run, and what stayed open

- Every number tagged [M] was produced in this session by the tool named in section 1 and can be re-run from
  `lead_tmp/botd/tools/` (the cover census and the profile need the scratch worktree `openbf-wt/botd-scratch` and
  its build `lead_tmp/botd_target`; both are left for the cleanse). The verifier re-ran every tool and read its
  source (section 16); the lead then re-derived each figure the verifier corrected or added.
- Two controls carry M3: the PS2's own bots (inside a barrier 0.3-1.2 % of their time) and the stall classes that
  are not about bodies (`open`, `hull`). M5's off-route hub counts equal SAIR 10b's on Kamino (75) and Polis Massa
  (115), by a different tool. M2 was sampled twice, on two builds.
- **Open / not measured:**
  - M3 is geometry: where bots stand against the barriers. That the barrier rule then keeps a bot out and walks
    it round is OBST's acceptance, not shown here. By the port's own leg test asked from where the bot stands, a
    barrier would be reported in 68 % of the `body` stalls (the verifier's count, not re-derived).
  - M2 is a sampling profile on one machine: 207-900 stacks a map (NAB1's 207 is +-3.5 points at 50 %); each
    timing is one run.
  - M5's Death Star count of off-route hubs (60) differs from SAIR's (38): the post lists differ (the port's six
    conquest posts here). Not resolved. M5's "other route" is one penalised search a pair: a lower bound.
  - M6's thresholds (3 m, 30 m, a 1 m lean, eye heights 0.9 / 1.6 m) are mine; "cover" is in any of 16
    directions, not toward an enemy; the lean point is not tested for a floor. The 43 % of Cover hints inside a
    barrier's 0.3 m margin were not measured, and nothing measures how good the PS2's cover is in play.
  - M4 is 16 v 16; at 32 v 32 the neighbour cap of 6 has to be re-checked (UCNT).
  - Coruscant was not captured on the PS2 (SAIR 14) and is not in M5 (the port's graph there has 155 hubs, the
    disc's largest plan 156).
  - Section 7's numbers are untested; the weights of 4.6 and the costs of 4.7 are starting values.
  - The PS2's contact kinds for its side-step, and why a given walk to a turret fails, stay [H] in SAIR.

## 15. Outside references - ideas, never code

Read by a helper session from the sources themselves (text, OCR), with quotes and page or slide pointers:
`lead_tmp/botd/refs/REFS.md` (what it could not open is listed at its end). The lead re-opened three: X1's
playtest slide (the PDF), X2's article (the words on dithering and on the prioritized list are taken from it) and
X10's sample-pattern post. Every other quote rests on the digest; the verifier checked each against it.

| # | source | what this note takes from it |
|---|---|---|
| X1 | J. Griesemer, C. Butcher, "The Illusion of Intelligence: The Integration of AI and Level Design in Halo", GDC 2002 (slides and speaker notes) | The playtest (slide 16): "Very Intelligent" 8 % -> 43 % after the enemies' hit points and damage were raised; "If it isn't totally obvious, it's too subtle"; the reaction pause removed as a perceived bug; firing points chosen by line of sight, range, cover, friends, with rays "about 60% of all AI cost"; "Random factors in behavior are rarely good"; 20-25 actors on 15 % of an Xbox |
| X2 | D. Isla, "Handling Complexity in the Halo 2 AI", GDC 2005 (the Game Developer republication) | Dithering named as the thing to avoid "at all costs"; a prioritized list with yes / no relevance where only a higher sibling interrupts; stimulus impulses that live "one or two seconds"; cooldowns and per-target failure memory; "styles" as lists of allowed behaviours |
| X3 | D. Isla, "Building a Better Battle: The Halo 3 AI Objectives System", GDC 2008 (slides) | Tasks with priorities and capacities filled greedily by a cost, squads kept where they are unless a task switches; "AI can look really stupid with wrong H" |
| X4 | R. Straatman, W. van der Sterren, A. Beij, "Killzone's AI: Dynamic Procedural Combat Tactics", GDC 2005 (paper and GDC Europe slides) | Position evaluation over waypoints: claimed positions removed, additive non-negative weights (cover from the primary threat, line of fire, preferred distance, friends' lines of fire ...), a cover table per waypoint, stance and direction computed ahead of time (32 KB for 2,000 waypoints), rays only to confirm; the search range tied to the decision cycle; a goal kept "until the goal is achieved, the goal becomes unachievable, another goal becomes more desirable, or one of the goal's actions fails" |
| X5 | M. Booth, "The Official Counter-Strike Bot", GDC 2004 (handout) | The movement hierarchy with the view independent of movement; hiding spots never shared; "danger" added where a teammate dies so that "bots choose different routes each round"; tiers by reaction time, aim offset and attack delay, Expert "very good (but still human)"; "Blatantly mindless behavior breaks the illusion" |
| X6 | J. Orkin, "Three States and a Plan: The A.I. of F.E.A.R.", GDC 2006 | Goals by priority that fall through when a goal has no plan; soldiers that "always try to stay covered"; a coordinator that only keeps two soldiers from one node; a failure recorded before choosing again; behaviour made readable by speech |
| X7 | C. Reynolds, "Steering Behaviors For Autonomous Characters", GDC 1999 | Action selection / steering / locomotion with one vector passed down; blended behaviours "may cancel each other out at inopportune times", so priority, not a sum; a path as a spine with a radius |
| X8 | M. Jack, "Tactical Position Selection: An Architecture and Query Language", Game AI Pro (2013) ch. 26; Epic's Environment Query System documents | Generation, conditions, weights, with ordered fallbacks; "one or two" weights for predictable results; cheap filters first and rays from the best score down; inertia by re-checking the chosen point with its conditions only |
| X9 | J. van den Berg, M. Lin, D. Manocha, "Reciprocal Velocity Obstacles for Real-Time Multi-Agent Navigation", ICRA 2008; J. van den Berg, S. Guy, M. Lin, D. Manocha, "Reciprocal n-Body Collision Avoidance", ISRR 2009; the RVO2 library documents | Reciprocity: both share the dodge and choose near their current velocity, which is what stops the left-right oscillation; and its limits (below) |
| X10 | M. Mononen, the Digesting Duck posts of 2009-2011 and the Recast Navigation reference (DetourCrowd) | Sampled velocity avoidance: "two rings of 7 samples plus one sample at zero velocity" aligned to the desired velocity, four costs (desired velocity, current velocity, side, time to impact), 6 neighbours at most, usable down to 5 Hz; "never ever ever try to follow a path" - keep a corridor and re-aim every step; a move request that ends in result, blocked or done |
| X11 | L. Liden, "Artificial Stupidity: The Art of Intentional Mistakes", AI Game Programming Wisdom 2 (2003) | Planned weaknesses for the low tiers (move before the first shot, miss it, warn); two attack slots a squad in Half-Life; a visible reaction to a detected failure |
| X12 | M. Dawe, "Beyond the Kung-Fu Circle: A Flexible System for Managing NPC Attacks", Game AI Pro (2013) ch. 28 | A budget per target for who may attack, scaled with the difficulty - the PS2's one-thrower claim [P 4.3] is such a token |
| X13 | E. Gewirtz, "Star Wars Battlefront Wrap Report", RPG Vault, 17 Nov 2004; the official mod-tools documents (`community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/`) | Pandemic's aim: "There should always be something cool happening"; maps redesigned "to concentrate the fights". No account by Pandemic's programmers of how the bots decide was found; SAIR 2 checks the mod documents' sentences against the code |
| X14 | T. Karlsson (DICE), "An Introduction to AI in Battlefield1942" and "A Primer on Writing Strategies for the AI of Battlefield1942" (2003); S. Polge (Epic), "Unreal Tournament AI and Gameplay for Level Designers" (1999) and "Unreal Tournament 2003 AI for Level Designers" (2003); EA / DICE, "How Artificial Intelligence Has Evolved Star Wars Battlefront II" (2019) | An aggression share with a maximum of attacks and a minimum of defences; "It is generally better to attack than defend" (the maintainer's decision 4); assault paths picked at random by weight; "an optimal 'stop and shoot' distance for each weapon" |
| X15 | R. Straatman, T. Verweij, A. Champandard, "Killzone 2 Multiplayer Bots", Game AI Conference, Paris 2009; + R. Morcus, H. Kleve, "Hierarchical AI for Multiplayer Bots in Killzone 3", Game AI Pro (2013) ch. 29 | Commander -> squad -> bot for capture-and-hold; one virtual controller between AI and engine; a planner re-run at a fixed rate that replaces the running plan only for a better one; objectives with a weight and an optimal number of bots, assignments kept; a penalty on areas another squad already routes through |
| X16 | Halo: Combat Evolved decompilations (the maintainer's question) | They exist: `punpckhdq/halo` on GitHub is active (11.5 % of the code matched on 2026-10-02; an `ai` directory of 64 files of which only the debug unit is matched), `halo-re/halo` is stale. Status only: no file was opened, and none is needed - X1 describes the same structure |

**Why not ORCA** for the avoidance [X9, X10]. Its constraints are yes or no: in a dense or symmetric meeting there
can be no allowed velocity, and the paper's own fallbacks are to stop ("may also lead to a global deadlock") or to
"go with the flow". Walls have to be handed to it as segments - "four times more code", one of the library's
authors said in an interview - and still "do not make the robot move around them": the way round has to come from
the route layer in any case, which is where the PS2 and this design keep it (barrier corners). Its linear program
adds constraints in random order, so a replay needs a fixed order. Mononen tried it in Detour and kept sampling:
"sampling allows you to take the risk". The sampled costs also carry the hysteresis (the current-velocity and
side terms) that section 5 needs.

**Not taken:** a planner (X6), influence maps (X15), designer-scripted encounters (X1-X3: there is no designer
here - the disc's data is fixed), a garrison split by posts held (X14, X15: the maintainer's decision 4, default off).

## 16. The verifier's pass

One Opus verifier, given 81 claims and their citations only (`lead_tmp/botd/verify/CLAIMS.md` -> `VERDICTS.md`;
three sub-verifiers, every PARTLY re-opened by the lead verifier): **65 SUPPORTED, 16 PARTLY, 0 NOT SUPPORTED**. It
re-ran each measuring tool and read its source. Three of the PARTLY (M3, M2, M5) each held a figure that a tool or
an input error had changed; read strictly, those three claims were NOT SUPPORTED as first written (65 / 13 / 3).
Every correction is in the text above; the ones that changed what this note says:
- **M3's first headline fell.** The draft said "in 96 % of the stalls against a body the bot's line crosses a
  barrier". The number is right and worthless: a random line of that length crosses one in 90 %, and the line ran
  to the leg's waypoint, not to the point steered at. What stands is where the bots ARE - inside a barrier (36 %),
  inside its margin (72 %), within a metre (84 %) - against the two controls. The lead then measured the PS2's own
  bots against the barriers (`inbarr.py`), which is now the note's main evidence for the barrier rule.
- **M5:** two maps were wrong in the input - Tatooine's posts sat on the arc-less hub (card HUBM's defect), and
  Utapau's plan was the base world's, not the Conquest layer's. Corrected: TAT1 22 of 30 (was 12 of 12), UTA1 13
  of 29. Added: "another route" may reuse ways authored for other pairs.
- **M6:** "standable" mostly means "outside the barriers": 43 % of the authored Cover hints lie inside a barrier's
  margin and were never measured. A "ray" is a ray test of up to two queries.
- **M2:** the wall-pass row had counted the steering's own push-out; the binary's commit was not recorded. The
  profile was taken again on a build of e336f2e.
- **The port's code:** the eight sources of the walk point are not a strict order (a boarding claim outranks the
  hold); the reaction time runs from the contact's first sighting OR hearing.
- **Citations of SAIR:** a new order replaces a fight rather than interrupting it; the roll "when 4 m are clear"
  belongs to the damage reaction; BF1's snipe stand is 60-100 s; BF1 alone needs a live path for its stuck
  rescue; the HitLocation lever is re-ai-difficulty 9.2; a capturer stays until its order is interrupted and
  resumed, then leaves after 5 s.
- **Quotes:** Killzone's commitment rule has a fourth condition (an action fails); "four times more code" is from
  an interview, not the ORCA paper.

Not put to the verifier (added after its pass, the lead's own): `inbarr.py`'s table, `hint_in_barr.py`'s count,
the re-timed rounds and the second profile.
