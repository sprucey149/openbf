# OPUS RE (loop lane 2, 2026-10-02, card SAIR) - [O] UNAUDITED: no Fable pass
# RE: the PS2 foot soldier's own AI, BF2 and BF1 - the think, the agents, the route, the fight, cover, turrets, hints

**The question** (card SAIR, the maintainer's bot programme). The port's soldier brain is ours; this note says what the PS2's
is, second to second, so card BOTD can keep what is good and beat what annoys. Read from both decomps and ELFs by
eight reader passes and the lead over two sessions (the first session died while its verifier ran; this one re-ran
its numbers, opened what it had left unread and finished the verifier). Measured on the running PS2: six PCSX2
battles sampled at ~10 Hz with the player spawned (BF2 Kamino and Polis Massa twice each, BF1 Kashyyyk Docks twice and
Naboo Plains once: every bot's controller, agent, think times, level of detail, sticks, mover and helper fields), and
39 save states read as RAM. Eight rules are replayed against that RAM (section 11).
Tags: [V] read (B2 / B1 = lines of `SWBFRNT2.ELF-decomp.c` / `SWBFRONT.ELF-decomp.c`, function named), [D] derived,
[H] hypothesis, [L] seen on the running PS2; `[O]` marks the load-bearing claims put to the verifier (section 14 has
its verdicts: 84 claims, 50 supported, 33 partly - every correction is folded in below -, 1 wrong and replaced).
Scratch, tools and raw data: `engine/artifacts/lead_tmp/sair/` (`AGENT_A..H.md` = the readers' transcriptions with
every line; `tools/`, `tools2/`; `ps2/`; `verify/`, `verify2/`; `checks/re_check_lead.txt`).

Conventions. BF2: `E` = the soldier entity, `C` = its Controllable = E+0x270 (sticks C+0x6c..+0x78 = E+0x2dc..+0x2e8,
eye C+0xd0, aim direction C+0xe0), `ai` = the UnitController = *(C+0xb4) (*(ai) = C), `mv` = ai+0x430 (the mover:
+0x10 request bits, +0x20 move order, +0x70 aim order, +0xa0 stance word, +0xa8 flags), `n` = ai+0x5a0 (the
controller's own number, 4.4). BF1: Controllable K = soldier + 0x2a0, controller = *(K+0xdc), its CommandFsm at
ctrl+0x14, the agent at ctrl+0x104, the number ctrl+0x2b0. "Think" = one controller update.
Builds on `re-conquest-goal.md` (which post), `re-ai-difficulty.md` (skill, aim error, perception numbers),
`re-walker-ai.md` (the shared steering), `re-squad-orders.md`, `re-grenades.md` section 7 (weapon choice), RE_LOG
"Barriers are the PS2's Obstacles" (the corner walk).

## 0. The answers

| item | the PS2 (BF2 unless said) |
|---|---|
| the brain | One UnitController per unit: an order queue (current + 5) mapped to an **agent** (UnitTrooperAgent = capture a post, UnitCombatAgent = fight, Board / UnBoard / Wait / Random / Follow / Hold ...), each running helper state machines. An interrupt (enemy seen, damage, a free vehicle) is pushed in front and the old order resumes; one order change per think (1.4) [O] |
| how often it thinks | **interval = agent factor x {4, 3, 2, 1, 0.25} s by a level of detail 0-4** set every 0.5 s from the nearest HUMAN: under 7 m 4, 7-20 m 3, over 20 m 2, minus 1 when he is outside the bot's 77.5 deg cone, minus 1 when his body was not drawn; raised to 3 / 4 inside a camera's view within ~28 / ~15 m. Factors: fighting 0.4, capturing 2.0, others 1.0. A capturing bot away from the player re-decides every 4-6 s, one fighting next to him every 0.1 s. 93-98 % of bot time is at levels 1-2 [O][L 99.9 % of 292,062 readings]. BF1: every unit every 0.5 s, every 1.0 s when over 45 m from the camera [O][L] |
| what it writes | The two move sticks (full deflection toward the move point in the soldier's own frame while the body turns to the AIM target: it strafes and backpedals), turn and pitch, the fire buttons, and stance / roll / jump / jet requests. Never sprint, never prone. A far, unseen soldier's input runs every other frame, in both games (1.5) [O] |
| walking a connection | Next hub = a weighted random draw (shared LCG) among at most 4 authored alternatives at EVERY hub, the arrival arc excluded; the point aimed at = a random point on a sphere of (hub radius - margin) round the next hub, per unit, per leg; BF2 soldiers aim \|v\| + 0.2 m ahead along the leg; arrival 1 m (last node 0.5 m) or the node's plane passed (2) [O] |
| barriers, other units | Barriers = local obstacles walked round by their corners. **No avoidance of other units before contact**: a contact push makes a 5 m side-step; stuck (2 m in 3 s / no progress 5 s) -> a +-10 m random walk (2) [O] |
| hint nodes | BF2: **Cover** hints in a fight by every soldier class (21 % of attack time on Kamino, 49 % on Polis Massa [L]); **Snipe** hints by `UnitType = scout` classes on the way to a post (stand 60-120 s); JetJump hints by jet classes in a fight; Mine hints by classes with a mine dispenser; the at-post "walk to a hint near the post" is dead code. One unit per hint. BF1 the same four plus a working at-post hint walk (3) [O] |
| the fight | Starts on sight (and on being seen by an AI enemy), a noise within 30 m, or damage. Target = the top of a 6-slot threat list. Movement = an engagement MODE picked from the bot's **own fixed number** (drawn once per controller), the level's weights and the open space: stand and shoot (a third of the bots crouched), circle at the weapon's optimal range, zig-zag, flank; a range band (back off / hold / advance 15 m); every 2-5 s a 50 % draw to go to a Cover hint. Fire whenever the loose aim test passes, moving or not. Ends when the target is beyond weapon range + 10 m or forgotten. No retreat, no low-health or low-ammo rule (4) [O][L] |
| team mates | Holds fire only when a HUMAN friend (within 50 m) is on its aim ray; never checks an AI friend. An AI never damages a human team mate; bots hurt bots at the friendly-fire option (100 %). The sight ray is blocked by any soldier, so a friend in the way hides the target. No spacing, covering or reaction to a friend (6) [O] |
| turrets | Boarded only with a live threat whose last position lies inside a turret weapon's arc (no range test), nearest unit within 50 m first - in BOTH games. Left after 10 s with NO live threat at a think; a remembered enemy anywhere, or an enemy turret 100-180 m away, keeps the gunner seated (7) [O][L] |
| difficulty | Per-team skill 1-20 (re-ai-difficulty) and the mode weights: Easy {3,1,1,1}, Normal {2,2,1,1}, **Elite {1,1,5,1}** (zig-zag x5 - not uniform as filed) (8) [O] |
| the maintainer's (a) turrets | The 10 s leave clock is reset by any threat the gunner remembers, enemy turrets included: on Kamino 32-58 % of a seated gunner's fight time is aimed at an enemy TURRET at a median 95-131 m, the rest at soldiers 70 m away; no health bail-out exists for turrets; damage from outside the arcs is the one thing that gets a gunner out early (10a) [L] |
| the maintainer's (b) dead areas | **Every hub the PS2's bots never visited lies off every authored post-to-post route**: Kamino 45 of 142 soldier hubs unvisited (all 45 off-route), Polis Massa 52 of 185 (all 52), Death Star 9 of 107 (7), BF1 Kashyyyk Docks 36 of 149 (10b) [L] |
| the maintainer's (c) BF1 stops | **A BF1 walk that fails is never retried and the board agent has no timer**: 14 bots in 37 save states stood in UnitBoardAgent with a failed walk, 12 of them heading for a turret 4-18 m above or below them; live, five stood 108-213 s. Only an interrupt (an enemy seen, a bolt within 7.5 m, damage) ends it. BF2 added a 30 s limit - which a failing walk resets, so BF2 stalls the same way (10c) [L] |

## 1. The machinery

### 1.1 BF2: the AI tick and the think [O]
- [V] `FUN_002af948` (B2:234062-234149), once per sim step: the PlayerControllers' own update; every 15th call
  (`DAT_005fe084`, ELF 15) `FUN_002af788` (the line-of-fire lists, 4.9); the vehicle notify scan `FUN_002e2e38`; then
  up to **10** controllers of the list `DAT_005fe08c` whose next-think time ai+0x2c4 has passed run the think
  `FUN_002b7530` and are re-inserted in order of that time; then EVERY controller runs the per-frame `FUN_002b7790`;
  last the route planner `FUN_002f3910` and the sight-ray queue `FUN_002e45d0` get their budgets.
- [V] The think (B2:239160-239263): ai+0x2c8 = now; when the body is not spawned (entity +0x228 bit 3 clear; set by
  the spawn routine `FUN_001e3c38`) next = now + 100000 and nothing else. Otherwise: zero the sticks and the mover's
  request bits (`FUN_002f4c78`), the level of detail (1.3), the free-space probe `FUN_002ae898` (ai+0x60 = 2.5 / 7.5 /
  12.5 / 17.5 m, at most every 2 s), the noise alert `FUN_002de708` (4.1), the threat list `FUN_002b9a28`, the order
  queue and agent `FUN_002b5e60`, three special-weapon rules (4.8), the perception scan `FUN_002e3aa0`
  (re-ai-difficulty 5); next = now + `FUN_002b6fe8`.
- [V] Per frame (`FUN_002b7790`, B2:239266-239306): the target tracker `FUN_002ae010` and the mover update
  `FUN_002bc1a8` (with the accumulated time) run every 5th frame at level 0-1, every 3rd at level 2, every frame at
  3-4; other frames only `FUN_002bc210`.
- [V] Chance draws written `FUN_002a3ff0(1, k) == j` are **1 in 2k**: the helper multiplies its upper bound by
  `DAT_006980a0` = 30 / `DAT_005fe084` = 2 (B2:225605-225613, 234460; RAM 2). BF1's `FUN_002325d0` doubles it in
  code. Both read as "odds per second at two thinks a second".

### 1.2 How often a bot thinks [O][L]
(Card AITHK, 2026-10-22: 1.1-1.3 and 1.6 re-checked as one rule, with the "think now" sites - a human-controlled
threat wakes a bot at the next step at any level - and the first think of a life: `re-ai-think-rate.md`.)
- [V] `FUN_002b6fe8` (B2:238925-238944): (the agent's vtable+0x50 factor; 1.0 with no agent) x {4.0, 3.0, 2.0, 1.0,
  0.25}[ai+0x560] (table ELF 0x621bc8). Factors: UnitTrooperAgent **2.0** (code at 0x2cba38), UnitCombatAgent **0.4**
  in its state 2 (`FUN_002ce8a0` = 0x3ecccccd), UnitFollowAgent 0.4 in its state 4, the CTF / Defend agents 0.8-2.0
  by state, every other agent 1.0 (AGENT_A Q1).
- [D] So: capturing (walking to a post, standing at a hint) 8 / 6 / 4 / 2 / 0.5 s by level; fighting 1.6 / 1.2 / 0.8 /
  0.4 / 0.1 s; boarding, in a turret, waiting 4 / 3 / 2 / 1 / 0.25 s. A message that yields an order re-thinks at
  once at levels 3-4 only (`FUN_002af840` from `FUN_002b7e58`).
- [L] Replay (`tools/think_replay.py`): ai+0x2c4 - ai+0x2c8 equals factor(agent) x table[level], both read in the
  same sample, on **291,678 of 292,062** readings in four captures (99.74-99.92 % each); the 384 others are
  re-scheduled "think now" entries (interval under 0.04 s). 484 readings of unspawned bodies show +100000.

### 1.3 The level of detail (BF2) [O][L]
- [V] `FUN_002b7b00` (B2:239522-239661), at most every 0.5 s and only inside a think: for each player record with
  +0x130 >= 0 (a human) that has a controllable: l = 3; 3-D distance under 7 m -> 4, over 20 m -> 2; -1 when the
  human is outside the bot's own view cone (`FUN_002a7230`, B2:227877-227936: inside 10 m always, else within 77.5
  deg of C+0xe0); -1 when the human's body node was not marked in the current render frame (`DAT_0060c908`; drawn or
  culled is [H]); level = the max over humans (0 with none). Then, unless 4, `FUN_001ba840(r x 3, 20 / 7 m,
  DAT_005fc4ec, pos)` raises it to 3 / 4: a frustum test over the local cameras with the far plane cut to D + zoom x
  8 m (`DAT_005fc4f4` = 8.0), i.e. ~28 m / ~15 m at zoom 1. ai+0x564 = the nearest human's record.
- [L] The cone term (`tools2/lod_angle.py`: bots over 22 m from the human and outside the camera's near zone, read
  within 0.15 s of their stamp): level 2 on 2,113 of 2,183 readings with the human inside 75 deg of the bot's aim
  (Kamino; Polis Massa 1,433 of 1,520) and level 1 on 642 of 701 beyond 90 deg (Polis Massa 1,298 of 1,391); the
  75-90 deg bin splits. The whole rule without the render-mark term (`tools/lod_raw.py`): 87-92 % exact on 10,777
  readings; the misses are one level, except with the player dead, when bots inside the death camera's frustum
  stayed at 0 on 139 of 829 readings (the camera list then holds another camera [H]).
- [L] Share of alive bot time by level: Kamino 0.3 / 30.9 / 62.4 / 4.2 / 2.2 % (levels 0-4), Polis Massa 0.4 / 58.9 /
  39.3 / 1.2 / 0.3 %. With no human alive every bot sits at level 0 (a first order 4.0 s after spawning against
  1.1-2.0 s, `bot-quality.md`).
- [D] What it means: away from the player the battle runs on 2-6 s decisions and a mover updated 6-10 times a
  second; in front of him at 1-10 decisions a second. Three more things run only near him: at level >= 3 a soldier's
  path gets detours (2), an appended path is smoothed (`FUN_002f0be0`, B2:280317-280377) and a new order re-thinks at
  once; at level 4 the corpse shot (4.10).

### 1.4 Orders, the queue and the agents [O]
- [V] Order type -> controller state (`FUN_002b8d60`, B2:240405-240470): 1-5 -> 1-5, 7 -> 8, 8 -> 9, 9..0xe ->
  10..0xf; 0 and 6 change nothing. The state's entry `FUN_002b70d0` (B2:238968-239125) builds the agent at ai+0x420;
  names from the strings stored beside the vtables:

| state | agent | vtable | role |
|---|---|---|---|
| 1 | UnitWaitAgent | 0x623858 | stand / man a turret (7.3) |
| 2 | UnitCombatAgent | 0x622b40 | the fight (4) |
| 3 | UnitWaitSecondaryAgent | 0x6239a8 | wait N ms |
| 4 | UnitBoardAgent | 0x622958 | walk to a seat and enter (7.2) |
| 5 | UnitUnBoardAgent | 0x623768 | get out |
| 8 | UnitRandomAgent | 0x623510 | the unstick walk (2) |
| 9 | UnitTrooperAgent | 0x623618 | capture a post: walk, snipe stand, serve, mines, at-post (re-conquest-goal 4) |
| 10-0xf | CTFOffense / Deathmatch / Defend / Destroy / Follow / Hold | 0x622ca8 ... 0x623408 | other goals, squad orders; the Deathmatch agent is state 0xb, `re-space-assault.md` section 17 |

- [V] A message (`FUN_002b7e58`, B2:239703-239740) goes to the agent's vtable+0x58; when it yields an order the
  agent's +0x4c chooses. 1 (the base agent, the trooper, wait ...): `FUN_002b60f0` pushes [new order, a copy of the
  current order] - **interrupt and resume**. 0 (UnitCombatAgent, UnitBoardAgent, UnitRandomAgent): `FUN_002b6050`
  pushes the new order in front of what is already queued, without the current one - a new order **replaces** a
  fight, a boarding or an unstick walk, and what was queued behind (the capture order) comes back afterwards. One
  push per queue update; the queue pops at the next think (`FUN_002b5e60`).
- [V/D] A finished agent (state 100) with an empty queue stays until something pushes an order; the goal's 1 s tick
  does that for a member with no order (re-conquest-goal 2).
- [L] Share of alive bot rows by agent (Kamino / Polis Massa): trooper on foot 56 / 57 %, combat on foot 15 / 12 %,
  combat in a seat 11.5 / 5 %, **board 11 / 9 %**, wait in a seat 3 / 5 %, none (between agents) 3 / 4 %, random 0.3 %.

### 1.5 What the AI writes into the soldier [O]
- [V] The soldier's AI input is **`FUN_00249f80(dt, E)`** (B2:164412-164718), slot +0xcc of the soldier's
  Controllable vtable 0x61e998 through `FUN_00255c80` (data word 0x61ea64). (`FUN_001f02e0`, which the first draft of
  this note named, is EntityDroid's twin - the verifier's one NOT SUPPORTED - and `FUN_0020b840` the flyer's.)
- [V] Its tail (B2:164685-164716): the fire controller `FUN_002bb1b0(dt, mv, aim)`, then the shared steering
  `FUN_002a54d8(dt, class+0x74c, class+0x750, FUN_00238068(E), class+0x758, C, move order, aim order, &fwd, &strafe,
  &yaw, &pitch)` (re-walker-ai 3) and the four results into E+0x2dc / +0x2e0 / +0x2e4 / +0x2e8. Class +0x74c / +0x750
  are MaxSpeed / MaxStrafeSpeed ([L] RAM: rifleman 7.0 / 5.25, sniper 8.0 / 6.0, wookiee 6.0 / 4.5).
- [V] In the steering (B2:226596-226843) a strafe speed over 1e-4 takes the strafe branch: the stick is the goal
  direction in the soldier's own frame, the larger axis at full deflection (scaled only inside the last node's
  0.5 m), and the body turns toward the AIM order (`FUN_002a50c0`): a soldier with a target faces it and runs
  sideways or backwards to its move point; with none it faces where it goes. No walk / run choice. One exception in
  `FUN_00249f80` itself: when the move point is nearer than one step (MaxSpeed x dt) the two move sticks are scaled by
  distance / step, so the last step does not overshoot (B2:164698-164712).
- [V] Its head (B2:164440-164463): every other call (entity id parity against a per-frame toggle) the whole input is
  skipped and the time carried over when `FUN_002a81c0(E)` returns 0: slower than 10 m/s, more than 45 m from the
  first camera and not inside a camera frustum cut at 70 + zoom x 8 m; never in a network game.
- [V] Stances and moves (the middle of the function, B2:164526-164645): mv+0xa0 0 or 4 -> stand, 1 or 2 -> crouch,
  3 -> the sprint call, falling back to stand; mv+0x10 bit 0 -> roll (and crouch after it), bit 3 -> jump, bit 2 ->
  jet (jump, then jet while airborne), bit 6 -> jet off. Who writes them: crouch - mode 2 (4.4), a cover hint's
  stance (4.6), after a roll; stance 3 - only the follow agent's walk to a cover hint over 5 m away and hover /
  walker evades [V for the sites read]; no prone value anywhere (RE_LOG "PrimaryStance"); roll - the evade (4.9) and
  the route's roll flag (2); jump - melee mode 7 and jump arcs; jet - arcs and the three jet states (4.7).
- [L] Sticks in a fight: moving rows show forward and strafe together 3-10x as often as forward alone
  (`tools/fight_bf2.py`); walking to a post with no aim, forward alone dominates. mv+0xa0 = 1 on ~10 % of AI rows.

### 1.6 BF1 [O]
- [V] `FUN_0025d988` (B1:179237-179329): every sim step each PlayerController updates; the UnitController list is
  spread over a **15-step round** (counter `DAT_0051814c`, `count/15 (+1)` units a step): each unit is visited once
  per 0.5 s. `FUN_0025d670` (B1:179129-179157) skips a visit, never two in a row, when `FUN_00236508`
  (B1:150492-150549) returns 0: an offline game, slower than 10 m/s, more than **45 m from the first camera** and not
  inside a camera frustum cut at 70 + zoom x 8 m (`FUN_0019fc10`; ELF 70.0, 8.0) -> a think every 1.0 s. The soldier's
  own update `FUN_0020aad0` (B1:119744-119765) uses the same test to run every other frame. No 4 s level, no
  distance-to-player count, no sleep.
- [L] Replay (`tools2/skip_bf1b.py`, raw ctrl+0x318 = "skipped this round", three captures): within 45 m 10 of 4,587
  readings (0.2 %); 45-78 m inside my model of the view 168 of 972 (17 %: the model's frustum is approximate);
  everywhere else 43.6-49.7 % of 60,879 (the every-other-round toggle).
- [V] The CommandFsm at ctrl+0x14 (B1:178625-178787): current order + 5 queued; replace / push-front / push [new,
  current] by the agent's "resumable" slot (1 for Wait / Scout / Trooper / Repair; 0 for Combat, WaitSecondary, Board,
  UnBoard, Random). The agent factory `FUN_00262bb8` (B1:182937-183060), names by RTTI: 1 UnitWaitAgent, 2
  UnitCombatAgent, 3 UnitScoutAgent (class AI kind 0) / UnitRepairAgent (kinds 3-4) / UnitTrooperAgent (others), 4
  UnitWaitSecondaryAgent, 5 UnitBoardAgent, 6 UnitUnBoardAgent, 9 UnitRandomAgent, 0xa UnitFollowAgent, 0xb
  UnitHoldAgent. Every agent think starts with the **InterruptHelper** (`FUN_00253cd0`, B1:171953-171988), the first
  of eight tests that yields an order wins: 1 a bolt passed within 7.5 m -> fight at that point (soldiers), 2 damage
  (5), 3 a perceived enemy -> fight, 4 a hostile unit made itself heard within min(its radius, 30) m and is inside
  weapon range -> face it, 5 a squad order, 6 a vehicle offer (7.5), 7 a timed wait, 8 the stuck test ->
  UnitRandomAgent. Helpers by RTTI: GroundGotoHelper (the walk), PatrolHelper (at a post), SnipeHelper, MineHelper,
  AttackPatterns with its CoverHelper and JetJumpHintHelper.
- Agent names: by RTTI (BF1) and the vtable strings (BF2) `FUN_00248cb8` / `FUN_002cb1c8` are UnitRandomAgent,
  `FUN_00248498` / `FUN_002caa40` UnitHoldAgent, `FUN_00247468` / `FUN_002c9418` UnitFollowAgent - as RE_LOG's
  2026-09-30 SQACK entry already corrected.

## 2. Walking a connection [O]

- **The next hub** [V]: no graph search. At every hub the follower (`FUN_002ed2d0`, B2:277381-277448) asks the
  chooser `FUN_002eb000` (B2:275669-275757) for the next hop toward the FINAL hub: the plan's per-hub table holds up
  to 4 alternatives per destination, each a byte = arc slot (3 bits) + weight (5 bits), weights summing to 31; the arc
  the unit arrived by is excluded (its weight comes off the total); r = the shared LCG (`DAT_0073d870`) % total picks
  one; when only the way back remains `FUN_002ea380` takes a random other arc. 3,432 of 5,000 disc hubs have one
  alternative per destination (RE_LOG plan census), so most hops are fixed. BF1: the same draw, `FUN_00266150`
  (B1:185741-185800), LCG `DAT_0066eed0`.
- **The point walked at** [V]: on reaching a hub the next hub's waypoint is drawn for this unit and this leg
  (`FUN_002ea128`, B2:275051-275109; BF1 `FUN_002657c8`, B1:185275-185332): hub centre + a random 3-D unit vector x
  max(0, radius - h) (h = 4 for a droideka) - a point on a sphere, so in XZ anywhere in the hub's disc and off its
  height; up to 100 re-draws while it lies inside a barrier (`FUN_002ed3d8`; BF1 `FUN_00267648`). No timer re-draws
  it. The last leg goes to the order's own point (capture: a random point of the post's region).
- **Steering along the leg** [V]: BF2 soldiers aim at their projection on the leg advanced by |velocity| + 0.2 m,
  clamped to the leg's end (`FUN_002f1d80` -> `FUN_002a5fa8`, B2:281157-281394, 227076-227156; walkers 20 m, hovers
  |v|); BF1 soldiers aim straight at the point (RE_LOG). Arrival (`FUN_002f1140` / `FUN_002f1320`): XZ within 1.0 m
  of a node, 0.5 m of the last (x 0.25 in speed mode 4), or the node's plane passed.
- **"The leg" is the follower's SEGMENT: the node just reached -> the node walked at** [O, card MOUTH, 2026-10-12;
  UNAUDITED]. `FUN_002a5fa8(lead, A, B, P, out)` writes A + u x (dot(P - A, u) + lead + 0.2), u = unit(B - A) in XZ,
  or B when that is past B or the two are under 1 cm apart (B2:227092-227149); all three of its calls are in
  `FUN_002f1d80` and pass A = follower + 0x10, B = follower + 0x20 (B2:281242, 281249, 281386; *(decompiler detail omitted)*, 281192). The advance `FUN_002f1470` (called only by the mover `FUN_002f1778`, B2:281005, 281066) steps
  follower[2] to the next list entry and calls `FUN_002f0e78(follower, follower + 0x20, follower + 8)` (B2:280791-
  280792), which copies its second argument to + 0x10 and the new node's point to + 0x20 (B2:280429-280440). So
  after EVERY node - a hub's point or a detour corner - the line a soldier is projected on starts at that node. The
  other four calls of `FUN_002f0e78`: a new path starts the line at the unit's own position (B2:280289, 280397) or
  with + 0x10 = + 0x20 (280285), an appended path keeps it (280375). Checks: `re_check.py fn` on all cited lines,
  `callers` on the three functions; one Opus verifier, 6 claims, 6 supported (it also counted the ELF's `jal` words:
  3 / 5 / 2, no data word). Live, BF2 Kamino, 8 min at 10 Hz (`engine/artifacts/lead_tmp/mouth/tools/
  follower_check.py`, the verifier re-ran it and marked its logic PARTLY - folded in): of 3,894 node changes between
  samples 2,004 moved + 0x10 onto the old + 0x20 to 1 mm (869 more had the two equal already, the 280285 start),
  1,008 began at the unit, 13 other, and 606 changes fell outside the sample filter; 1,882 of the nodes reached were
  NOT hub points (corners, goals) against 991 that were. The mover's move point lay on A -> B to 2 cm in 27,042 rows
  (6,329 of them with the feet over 0.1 m off that line; lead 5.6 / 6.75 / 7.2 m at 5 / 50 / 95 %), at the node in
  12,268, off the line in 625 (the contact step). BF1's follower has the same layout (array stride 0x90, BF2 0xb0;
  `tools/pathscan.py`) and aims straight at + 0x20. [H] not read: the list's node record beyond its point (+ 0x10)
  and flags (+ 0x20), and what `FUN_002f0be0`'s smoothing moves.
  **The port differs** (`ai/route.rs` `leg_point`, main fe043b4): with the corners behind it the bot is projected on
  `Route::from` -> waypoint, the one line the corner walk was planned to avoid. On BF2 Kamino's walkway hub 85 -> 49
  that pulls a bot off the mouth corner into the rail: 28 stalls on the four walkway legs in 24 bot-rounds, 0 with
  the line started at the last corner (a scratch switch; card MOUTH has the tables).
- **Barriers** [V, RE_LOG]: obstacles for the local layer, never graph costs: a leg that crosses one is re-planned by
  a small A* over the barrier's corners (r + 0.5 m), the walk step re-aims at a silhouette corner, side chosen once.
  BF2 adds dynamic barriers for buildings, hovers and flyers only (`FUN_002ee6f0`: 3 callers, none soldier code).
- **Other units** [V by search]: nothing steers a soldier round another unit before contact. The move chain
  (`FUN_00249f80` -> `FUN_002a54d8`; `FUN_002f1d80` / `FUN_002f1778`) reads the unit, its path and its push vector;
  the aim target is read for the facing only (`FUN_002a50c0`); none of the callers of the two proximity queries
  feeds the mover (AGENT_A Q6). After contact: the soldier's contact handler adds the contact normal (|n.y| < 0.3,
  contact kinds 8 / 0x10 / 0x20 / 0x40 [H: which bodies]) to E+0x4c0 (`FUN_0023cfd0`, B2:155881-155891) and the
  follower steers to **the unit's own position + 5 m along the perpendicular**, the side chosen once
  (B2:281267-281345); BF1 the same at soldier+0x970 (RE_LOG).
- **Stuck** [V]: not 2 m from an anchor in 3 s, or no progress to the node for 5 s (droideka 2 m / 6 s / 6 s;
  `FUN_002f24e8`, B2:281420-281538) -> message 7 -> order 7 -> **UnitRandomAgent**: walk to random points +-10 m per
  axis, re-picked every 3 s, done once over 6 m from where it stuck at a barrier-clear spot (`FUN_002cb2c8` /
  `FUN_002cb580`, B2:253566-253735). BF1 the same through InterruptHelper test 8, done at 10 m - but only while the
  mover holds a path, so a bot whose walk has FAILED never counts as stuck (10c).
- **Near the player only** [V/H]: at level >= 3 a soldier's path gets detours (`FUN_002bd628`, B2:243867-243931):
  every 2 s to an item within 6 m (card PWAI), every 3 s a 4-point detour along the nearest barrier's side, every 2 s
  (1 in 2) through a hint 5-10 m ahead. On a walk whose request carries the roll bit (only the walk to a Cover hint,
  a coin flip) the bot rolls toward the node when 9-11 m from it (`FUN_002f1140`, B2:280609-280632).

**The mod docs' sentences, checked** (`community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/`):

| docs | the PS2 code |
|---|---|
| "secondary destinations along the connection are established to act as waypoints" (ZeroEditor_guide 330) | One per HUB, not along the connection: a random point on a sphere in the next hub, per unit, per leg [V] |
| "When the AI NPC reaches the Primary Destination Hub, it then re-evaluates the path ... along the shortest path" | Re-evaluated at every hub, from the editor's precomputed weighted table: a random draw among at most 4 authored hops, not a shortest-path search [V] |
| "looks for the closest command post that is not owned by its team" (game.md 32) | No: one enemy post drawn at random by Value_ATK, no distance (re-conquest-goal 3) [V] |
| "Barriers ... the AI recognize and calculate the area of in order to plot a course around objects" (game.md 29) | Yes: the corner walk; "objects on a level are not recognized" also holds - only barriers and contact [V] |
| "AI cannot chart a course to a command post that is within a barrier filtering out that AI type" | Yes: the planner refuses a goal that is not clear (RE_LOG); the walk then fails - and BF1 stays failed (10c) [V] |
| "When an AI comes into proximity with a Hint Node they will occupy it ... If after a certain amount of time the unit encounters no enemy units it will leave" (game.md 35) | No hint is taken "in proximity" on a walk except a scout's Snipe hint (60-120 s whatever it sees; an enemy SEEN ends it early - the opposite). Cover hints are taken BECAUSE of an enemy [V] |
| "Gun turrets ... are treated like Hint Nodes by the AI" | No hint code reads a vehicle; boarding is the 50 m vehicle scan with a target-in-arc test (7) [V] |
| Hint types "Snipe, Patrol, Cover, Access, JetJump, Mine, Land" | TYPE 3 (Access) is read by neither game and used by no disc; BF2 adds types 7 and 8 (vehicle cover) (RE_LOG) [V] |
| connection weights "create less favorable connections" | The editor bakes them into the 5-bit route shares; the runtime never sees a weight per connection [V] |

## 3. Hint nodes [O]

- [V] BF2 keeps every hint in one tree, queried by a kind mask (`FUN_002df6c0`, B2:267537-267622); the kind byte is
  set by the constructor (`FUN_002df120..270`, B2:267235-267313): TYPE "0" Snipe 1, "1" Patrol 2, "2" Cover 4, "4"
  JetJump 8, "5" Mine 0x10, "6" Land 0x20, "7" 0x40, "8" 0x80. Mode and Radius are ignored; PrimaryStance /
  SecondaryStance are stand / crouch bit masks, plus two lean bits. A unit carrying a melee-kind weapon (category 4,
  4.4) gets no hints at all.
- [V] **Occupancy**: one unit per hint, any team: a user re-stamps its hint every update; the hint is free 2 s after
  the last stamp (10 s for a Mine hint) (`FUN_002a62e0`, `FUN_002decd0`).

| hint | who, when | what | where |
|---|---|---|---|
| Cover | every soldier class, in a fight: a 50 % draw (75 % against a vehicle) every 2-5 s | nearest-scoring hint within min(40 m, weapon range) on the bot's side of the target and facing it; hide 2-4 s, fire 2-6 s or lean 2 m; firing throughout | 4.6 [O][L] |
| Snipe | `UnitType = scout` classes, walking to their post: every 8-12 s | nearest free hint within +-100 m per axis (at most 20, 11 examined) with no enemy within 12 m of the line to it and a route; stand U(60, 120) s; weapon range x 2 while standing | below [O][L] |
| JetJump | jump-jet classes in a fight, every 3-8 s | nearest within 40 m: walk there, jet 80 m along its direction | 4.7 |
| Mine | a unit with a mine dispenser, on its capture order, every 5 s | up to 3 hints within 100 m at own posts, no mine within 6.1 m (enemy) / 2.25 m (own): walk, lay, 3 s, next | AGENT_H 4.2 |
| Patrol | the at-post behaviour | **dead in BF2** (below); BF1: 3 in 4 a hint near the post, 30-45 s | re-conquest-goal 4 |
| "7", "8" | follow / defend helpers; hover vehicles in a fight | not opened beyond their queries | AGENT_C 3.3 |
| Land | flyers | - | - |

- [V] **The Snipe stand** (UnitTrooperAgent state 7): `FUN_002d7c30` (B2:262128-262160) only for class kind 0;
  `UnitType` (property hash 0x54a7aedf -> class +0x8d4, `FUN_00255418` B2:172120-172161): scout 0, assault 1, repair /
  support / special 2, pilot 3, trooper 4, default 5. On the discs scout = the four default snipers,
  rep_inf_ep2_sniper, ewk_inf_scout, Leia, the Naboo queen (disc census). Search `FUN_002d84d8` (B2:262505-262692),
  stand `FUN_002d7d60` (B2:262182-262214). An enemy seen hands it to a fight; afterwards a new trooper agent usually
  re-finds the same hint. The 15 s "suitability" re-check `FUN_002d8398` always returns 1, so
  `AISnipeSuitabilityDist` changes nothing.
- [L] The longest standing spells of BF2 bots on foot are snipe stands: 101-229 s on Kamino (three bots for a whole
  229 s capture), 65-168 s on Polis Massa (`tools/stall_bf2.py`).
- [V] **The at-post hint branch is dead in BF2**: the at-post helper keeps only Patrol hints whose CommandPost is its
  post (`FUN_002df858`, B2:267632-267656), and BF2's CommandPost getter `FUN_002deec0` is `return 0` and its setter
  empty (ELF 0x2deec0 / 0x2deec8) - a bot at its post always wanders 8-14 m for 20-30 s. **Correction to
  re-conquest-goal.md 4** ("else walk to one of up to ten hint nodes").
- [V] BF1 (AGENT_D 2, AGENT_G 5): SnipeHelper (UnitScoutAgent searches every think, stand U(60, 100) s; a trooper of
  AI kind 0 every 8-12 s; within 70 m), MineHelper, PatrolHelper (its hint walk works), JetJumpHintHelper, CoverHelper
  (5). A team under 26 reinforcements gets no snipe hints, no vehicle offers and no repair detours (`FUN_00236e20`).

## 4. The fight (BF2) [O]

### 4.1 How a fight starts [V] (AGENT_B 1; verifier B1-B3)
- **Seen**: the perception `FUN_002e3aa0` puts {1, 1, 2, 4, 5}[level] candidates per think through the sight ray;
  `FUN_002e43c8` (B2:270590-270681) sends the bot message 5 for a visible one -> `FUN_002c31e0` (B2:248099-248179): a
  fight order (type 2, sub-type 1) on that entity, when the attack matrix accepts the pair (4.4); a droideka ignores
  targets beyond 10 m. When the SEEN unit is AI-driven it gets message 5 about the seer: **being seen by an AI enemy
  starts a fight at once**.
- **Noise** (`FUN_002de708`, B2:266704-266880, every think): every enemy AI unit within min(the emitter's radius, 30)
  m: within 3 m -> fight; farther -> **go to the noise** (type 2, sub-type 3) and a threat-list entry. No ray.
- **Damage** (`FUN_002c28e8`, B2:247760-247997): only from a hostile team; attacker inside the bot's view distance
  for it and attackable -> walk to the attacker's position (sub-type 0) and add him to the threats; otherwise, a
  soldier only, a 20 m run away from a point within 1 m of itself (sub-type 2; a roll instead when 4 m are clear,
  1 in 2).
  Early on it calls `FUN_002c26b0` (B2:247809; B2:247649-247758, also called by four other combat helpers): when the attacker is more than 50 m away
  (squared distance over 2500.0, B2:247712) it passes the hit on to friends within 20 m (B2:247715); and when the
  attacker's +0x130 word is not negative (read as the human test [H]) and his team is hostile, it adds him to the threat list
  (`FUN_002b9bb0`, B2:247748) and makes him the attack target (`FUN_002b7a40`, B2:247749), at any range. No delay
  before the answer is found in either step. openbf's flinch and find-the-shooter search are OURS
  (re-ai-difficulty 9.4, playtest C1); the 50 m pass-on and the out-of-view run are not ported.
- **A shot passing** (`FUN_002a89b8`, B2:229082-229181, called by two ordnance updates, B2:309191 and 314368): every
  unit within 7.5 m of the ordnance gets message 2 with its position, at most once a second per ordnance and after a
  skill delay: a soldier not yet fighting gets an evade order (sub-type 2: run 20 m away from that point, or a roll);
  one fighting goes to its evade state. [Read by the lead after the verifier's pass; BF1's twin is the bolt alert.]
- Every fight order is pushed in front of the capture order, which resumes after it (1.4).

### 4.2 The combat agent and its helper [V] (AGENT_B 2)
UnitCombatAgent (`FUN_002c4c80`) runs an unnamed combat helper (vtable 0x623b58): states 2 go to a point (the
attacker's position, at least 15 m out), 3 look around (two look points, 0.4 s each once faced, then finished), 4 go
to the noise (at most 10 s), **5 attack**, 6 evade (20 m, 2 s), 7 the corpse shot (4.10). Its target = the threat
list's best entry, re-read every update (`FUN_002cdf10`, B2:255616-255664). The attack (`FUN_002cf458` state 5,
B2:256852-256930): no target -> look around -> finished; target beyond weapon range + 10 m (XZ) -> finished; the
matrix is re-run every update: rejected -> look around; a new target or another mode -> the attack restarts; else
the attack sub-helper (vtable 0x6247f0) runs the mode. **No clock ends an attack on a target in range.** A damage
message is ignored in states 5 and 6.
[L] Share of on-foot combat rows by helper state (Kamino / Polis Massa): attack 45 / 26 %, go to the noise 10 /
**54 %**, look around 19 / 17 %, evade 15 / 1.5 %, go to a point 10.5 / 2 %.

### 4.3 The weapon [V] (AGENT_H 2; re-grenades 7)
`FUN_002ab590` (B2:230907-231134), every matrix run: skip a weapon with no ammo, one barred by the explosive claim
or an award gate, one that scores 0; any in-range weapon (min < d < MaxRange) beats an out-of-range one; among
in-range weapons one with more than 2 rounds beats one with 2 or fewer; then the higher score = damage per second
against the target's type (`FUN_002aaba0`: the ODF `AITargetPerson / Animal / Droid / Vehicle / Building` mask must
allow the type; a weapon that cannot take half the target's max health in 10 s may be used by a foot soldier only
with missile / shell / beam / haywire ordnance, or grenade / sticky against something slower than 12 m/s, and only on
a hover, walker, slow or landed flyer or flagged building - else the soldier does not attack that vehicle at all).
The **grenade** gate `FUN_002ab288` only ever forces the grenade: in a 3 s window per 30 s per bot (n % 10), at once
against a droideka or a soldier standing in a cluster (over 3 of his own side and none of his enemies within 9 m),
then the skill's chance. **Explosive claim** (`FUN_002ab430`): one bot per team at a time may use a grenade or
missile weapon on a given soldier (10 s claims). Dispensers, repair tools, buffs and cloaks are never chosen here.

### 4.4 The engagement mode [O][L] (AGENT_B 3, AGENT_H 2.4; ELF tables read)
The attack matrix `FUN_002ac8f8` (attacker kind x target kind, B2:231782-231974) picks weapon and mode:
- A foot soldier never attacks a turret seat / passenger slot or an item (the matrix's default). **Corrected by
  audit HVYW (24.2): it DOES attack a flyer in the air** - the soldier x flyer cell chooses a weapon in both games;
  only BF2's too-weak test (22.1 step 5) asks whether the flyer is in the air.
- **Against a soldier, droid or droideka** (`FUN_002abc38`, B2:231214-231252), by the attacker's `UnitType`:
  scout: a rifle-kind weapon in range -> **mode 2**, out of range -> no attack; other types: a melee-kind weapon ->
  mode 7; an `OffhandWeapon` (weapon class +0x1e0 bit 55; category 2 of `FUN_002a9b78`) in range -> mode 2; else the
  **draw** `FUN_002aba38` (B2:231136-231212): codes {2, 3, 4, 5} with the level's weights (ELF 0x6215c8: Easy
  {3,1,1,1}, Normal {2,2,1,1}, **Elite {1,1,5,1}**), code 4 counted only when the free space ai+0x60 > 10 m, code 5
  when > 5 m; r = **n % total**, n = ai+0x5a0 = the controller's own number 0-100, written once in its constructor
  (`FUN_002b6c20`, B2:238826-238827); the walk subtracts every code's weight, eligible or not, so with 5-10 m of
  space the flank share lands on zig-zag; a drawn 2 becomes 3 outside the inner band.
- **Against a walker, hover or building** (`FUN_002abee0`, B2:231327-231404): a target with a weak spot (a hit
  location with a factor over 1) -> mode 6; else n even -> mode 5, n odd -> mode 2 when in range.
- [D] **Each bot has a fixed fighting style for a given level, target kind and space**: the "draw" gives the same
  answer every think; it moves only with the open space (re-measured every 2 s), the weapon category and the band.
  The same number decides its crouch in mode 2 (n % 3 == 1, `FUN_002d9ba0`) and its grenade window (n % 10).
- [L] Replay (`tools2/mode_replay3.py`, level 2 read in RAM, on foot, attack rows): scout classes with a soldier
  target 488 of 488 rows; other classes with a soldier target 1,834 of 1,926 (95.2 %) on Polis Massa and 3,417 of
  4,640 (73.6 %) on Kamino - the misses are mode 2 where the draw gives 3 (1,075 rows: the offhand-weapon path, the
  weapon is not in the capture [H]); turret targets 217 of 332 by the parity rule. Stored modes: Kamino 2 31 %, 3
  62 %, 4 2 %, 5 5 %; free space was 2.5 m on 74-93 % of combat rows, so zig-zag and flank are rare.

### 4.5 What each mode does [V] (AGENT_B 3.3, AGENT_F Q2)
- **2 stand and shoot** - stop; crouch if n % 3 == 1 (a third of the bots, always the same ones).
- **3 circle** - walk to the target's point + a random horizontal direction x rand(opt - 0.2 (opt - min), opt + 0.2
  (range - opt)); on arrival, or 1 in 12 per think, pause 1.9 s; repeat (opt / min / range = the weapon's OptimalRange /
  MinRange / MaxRange; defaults 30 / - / 60).
- **4 zig-zag** - while the target aims at the bot within 18 deg: 30 m sideways (alternating) and +-30 m along, the
  sign by the band every 3 s; else the band.
- **5 flank** - every 4 s, while the target aims at the bot within 60 deg: to the target's side at the optimal
  range; else the band.
- **6 behind** - every 4 s to the target's position minus its forward x min(optimal range, 30 m).
- **7 close** - melee-kind weapons: 25 m legs, +-90 deg within 5 m, jumps.
- **The band** (`FUN_002d9468`, B2:263270-263360): nearer than inner = min(opt, min + 3) -> back off 15 m; between
  -> stop; beyond outer = min(max(opt, range - 10), view distance - 10) -> advance 15 m.
- Every mode update first runs the checks of 4.6-4.7; the one that fires borrows the bot and gives it back.

### 4.6 Cover in a fight [O][L] (AGENT_F Q1, Q3.1; verifier B15-B17)
- [V] `FUN_002d9620` (B2:263362-263395), called from modes 2-5 (and 6 at a quarter): at the mode's first update and
  then every randint(2, 5) s a draw with p = the matrix's factor - **0.5 against a soldier, 0.75 against a
  vehicle**; on a hit the cover search; a hint found -> sub-state 10.
- [V] The search (`FUN_002d0550` -> `FUN_002d1790` -> `FUN_002d1928`, B2:257293-258071): Cover hints in a box of
  R = min(40 m, weapon range) round the bot, at most 30, free; a hint qualifies when it is on the bot's side of the
  target ((H - T).(T - me) < 0), its authored direction points at the target within 60 deg, and inner < |H - T| < R
  (all XZ); lowest 2.25 |H - me|^2 + |H - T|^2 wins. **No ray, no occlusion test: the hint's direction is the only
  cover geometry.** Never for a unit with a melee-kind weapon.
- [V] The cycle (cover FSM, vtable 0x623c08; `FUN_002d06e8` / `FUN_002d0c78`, B2:257364-257793): walk to the hint (a
  coin flip arms a roll at 9-11 m from it); **hidden** randint(2, 4) s in a stance drawn from PrimaryStance;
  **firing** randint(2, 6) s in a stance drawn from SecondaryStance, or - hints with lean bits, a coin flip - a 2 m
  side-step for randint(4, 7) s and back. Every 5 s the hint is re-scored (invalid -> another hint, or back to the
  mode); every randint(5, 12) s another qualifying hint is taken if there is one. The bot **aims and fires in every
  cover state**, "hidden" included: hidden and firing differ only by stance.
- [L] On-foot attack rows by sub-state (`mode` column = agent+0xc0): Kamino cover **21.3 %** (46 spells, median 1.1 s,
  max 5.2 s), circle 48 %, stand 23 %, flank 5 %, zig-zag 2 %, jet 0.4 %; Polis Massa cover **48.6 %** (28 spells,
  median 1.9 s, max 10.7 s), circle 36 %, stand 13 %, behind 1.4 %, jet 1.1 %.
- [D] The "lost sight for 2 s -> go to the last known point" check (`FUN_002d9b20`) cannot fire: the helper
  re-stamps its target every update; its state 0xf and sub-states 9 (run 100 m away) and 0x11 have no writer.

### 4.7 Jets [V] (AGENT_F Q2, AGENT_H Q6)
Jet classes with fuel, in a fight: every randint(6, 12) s a jump to a random clear spot 20-57 m away with open sky
(`FUN_002d9890` -> 0xc; the straight line is tested for barriers only where the script calls
`SetAllowBlindJetJumps(0)` - Kamino); jump-jet classes every randint(3, 8) s to a JetJump hint within 40 m (0xd);
hover-jet classes fly at a target 20 m or more away when the fuel lasts and no barrier crosses the line (0xe). On a
route the follower sets jump or jet from the connection's flags. The deathmatch agent jets straight up at random.

### 4.8 Special weapons, repair, mines [V] (AGENT_H Q3, Q4)
- The think's three soldier rules (`FUN_002b8160` / `FUN_002b8258` / `FUN_002b8400`, B2:239855-239996): cloak
  (WeaponInvisibility) when there is no threat; a friendly-target emitter (a buff) when threatened with 3 friendly
  units, itself included, in its range; an auto-turret dispenser when the threat is a soldier in a cluster.
- **Repair / heal / resupply** (UnitTrooperAgent state 6; `FUN_002d6828`, B2:261372-261565): `UnitType` repair /
  support / special / pilot only, every 5 s, friendly or neutral entities within 75 m that lack health or ammo the
  bot's repair tool or powerup dispenser gives: vehicles, turrets, buildings, droids and **HUMAN soldiers - never an
  AI soldier, never itself**; walk into range, hold the tool / drop one item; give up after 20 s out of position or
  8 s with no gain.
- **No behaviour is keyed to the bot's own health or ammunition** [V by search, verifier B28]: no retreat, no walk to
  a supply droid (no AI code reads the PowerupStation class), no disengage. Ammo only filters weapons. Own-health
  reads found: the hover / walker pilots' 20 % bail-out, the droideka's shield, UnitDestroyAgent (skips its engage
  block under 50 %), two flags `FUN_002e5c08` sets under 25 % [H: chatter].

### 4.9 Fire, sight, being hit [V] (AGENT_B 4-5, AGENT_H Q1, Q5)
- **Request**: every helper update: target tracked (seen) -> "fire when aimed"; only remembered -> fire at its last
  point when the aim is on it, the point is within 3 m of where it really is and the ray to it is clear.
- **Trigger** (`FUN_002bb1b0` B2:242120-242362; aim test `FUN_002ace58` B2:232087-232211): the aim within **22.5 deg**
  of the target point (45 deg melee), the barrel within the same of the aim, the point inside weapon range; beyond
  50 m the aim line must also pass within 20 m. Accuracy lives in the aim tracker's error (re-ai-difficulty 4), not
  here. Bursts / single taps / charge as re-ai-difficulty 4.4. **Firing and moving are independent**; only mode 2
  stands.
- **Sight** (`FUN_002a7648` -> `FUN_002a74a0`, B2:228019-228104): one ray from the eye to the target's aim point,
  layers 0x9e = terrain, buildings, vehicles, **soldiers** (and layer 4). Any soldier on the line, friend or foe,
  hides the target; no distance or foliage term in the test.
- **Line-of-fire veto** `FUN_002bb900` (B2:242364-242465): no shot while an entity of the controller's list (at most
  8) has its bounding sphere on the aim ray, any distance. `FUN_002af140` (B2:233773-233832) rebuilds the lists every
  15 steps: each **PlayerController's** entity (`DAT_005fe088` holds humans only, `FUN_002aef80`) is appended to the
  AI units within 50 m that his team holds as friend or neutral (mask 6). **A bot holds fire for a human friend, not
  for a human enemy, and never for an AI team mate.** Repair and dispenser weapons are exempt.
- **Hit**: in an attack, ignored; out of a fight, 4.1. The on-hit dodge chance (re-ai-difficulty 9.4) was not found
  to have a reader [H]. Rolls: the evade state and the roll toward a cover hint.

### 4.10 The corpse shot [V] (AGENT_F Q4)
`FUN_002cf100` (B2:256483-256586): at level 4 only, a `UnitType` trooper of the team whose faction index is 1, 127
times in 128: a dead enemy soldier within 5 m with no barrier between -> walk to it and fire at the body with a
pistol-kind weapon until the corpse goes (helper state 7), with a chatter line. [H: a taunt.]

### 4.11 The numbers a fight leaves [L] (`tools2/fight_spells_bf2.py`, `tools/fight_bf2.py`; card FGHT's PS2 side)
- Spells in the combat agent on foot: Kamino 156, median 2.2 s, p90 10.7 s, max 25 s, net displacement median 5.2 m
  (p90 14 m); Polis Massa 116, median 3.4 s, p90 9.9 s, max 38 s, displacement 9.1 m (p90 23 m).
- Distance to the aimed soldier at the first and last sample of a spell: Kamino 30.8 -> 26.9 m (median change -1.1 m;
  closed over 5 m in 37 %, opened over 5 m in 24 %); Polis Massa 9.6 -> 8.1 m (0.0; 29 % / 26 %). **A PS2 fight does
  not close on its target**: it holds its weapon's band, circles and takes cover. What brings bots together is the
  noise order (4.1): over half of Polis Massa's combat time is "go to the noise".
- Moving share by distance to the aimed soldier: under 10 m 52-80 %, 30-50 m 29-54 %, beyond 50 m 1-12 %.

## 5. BF1's fight [V] (AGENT_D 3, AGENT_G 1-4; verifier D13-D18)
- **States** of UnitCombatAgent (`FUN_002456e0`, B1:161413-161720): 2 face a point (15 m out), 3 look around (2
  looks), 4 fight - gives up beyond weapon range + 10 m; the choice is re-run every think and a new target or code
  re-enters it -, 5 run 20 m from a point or roll (4 s), 6 wait 4 s.
- **Style** [O]: ctrl+0x2b0 = randint(0, 100), written once in the controller's constructor (`FUN_002623c8`,
  B1:182730-182731). Against a soldier `FUN_00238b38` (B1:152006-152117) picks by n % 6 and the enemy skill
  (`DAT_005875f4` against 14 and 17): low (2|3, 2|3, 2|3, 3, 4, 5), middle (2|3, 2|3, 3, 3, 4, 5), high (2|3, 3, 4, 4,
  4, 5) - "2|3" = stand when in range, else ring; a sniper-kind class with a rifle always stands; a melee class
  chases (6). Against a vehicle: n even -> flank, odd -> stand. The same n: crouch in pattern 2 (n % 3 == 1), the
  grenade window (n % 10: 3 s in 30).
- **Patterns** (AttackPatterns, vtable 0x538090): 2 stand; 3 ring - a random bearing on the optimal-range ring round
  the target, 1.9 s pauses; 4 zig-zag - 30 m steps while aimed at within 18 deg, a 1-in-10 roll per step, else the
  band (near = min(opt, min + 10), far = min(max(opt, max - 10), view - 10), 15 m moves); 5 flank - every 4 s a
  quarter circle while aimed at within 60 deg; 6 chase. BF2's modes 2-5 are these, with a tighter inner band.
- **Cover** [O]: once per randint(16, 25) s (at once after any sub-state) with p = 0.5 (0.75 against a vehicle) the
  CoverHelper (RTTI `11CoverHelper`) looks for a hint within min(weapon range, 40) m, on the bot's side of the target
  and facing it within 60 deg, scored as in BF2; then hide 2-4 s, pop up 2-6 s or step out 4-7 s, stances from the
  hint's masks (prone allowed), re-scored every 5 s. Jet classes: an evade jump every 3-8 s, JetJump hints.
- **Fire**: the same request as BF2; the aim test a 22.5 deg cone (beyond 50 m within 20 m); zoom rifles need the aim
  line within 0.25 m, then one shot and U(2, 3) s. No movement test: fire while walking.
- **Hit** (`FUN_001bc928` -> InterruptHelper test 2 `FUN_00252520`, B1:171015-171186): a hostile hit worth over 1.0:
  attacker inside the view distance and attackable -> face him (perception does the rest); a seated gunner whose
  weapons cannot bear leaves the seat; otherwise a soldier runs 20 m in a random direction or rolls. No low-health
  rule found (a narrow search [H]).
- **Perception**: scan radius clamp(skill value, 75, 350) m, the 77.5 deg cone with NO all-round 10 m (BF2 has it),
  view distance (20 r + 30) m, x 2 for a flyer.

## 6. Team mates [O]
- [V] **Line of fire**: both games veto a shot only for a HUMAN on the aim ray (BF2 4.9; BF1 `FUN_0023cfc8`
  B1:155289-155382, list writer `FUN_0025d718` B1:179160-179236: humans within 50 m, relation mask 6, at most 8). An
  AI team mate on the line is shot through - unless he hides the target from the sight ray (BF2's includes soldiers).
- [V] **Friendly fire**: BF2 `FUN_001dfb38` (B2:91316-91326), BF1 `FUN_001bc190` (B1:64354-64361): a friendly AI
  attacker does **no damage to a human**; every other friendly hit is scaled by the options' friendly-fire percentage
  (`DAT_005fcbe0` / `DAT_00517508`; 0.99999994 in the battle states' RAM of both games) - bots hurt bots in full.
  [L] Card FFLN (`re-blaster-damage.md` 6.3) counted 11 team kills in 91 AI kills (12.1 %) in a live 515 s Kamino
  battle. Its "no PS2 line-of-fire rule" holds for AI friends; the veto above is for the human alone.
- [V] No spacing, covering, regrouping, target sharing or reaction to a friend being hit, in either game (AGENT_B 8,
  AGENT_F Q5h, AGENT_G 2c): positions are computed from the bot, its target and the target's occupants; the only
  crowd rules are hint occupancy, the explosive claim, the seat reservations and the cluster test (which counts the
  TARGET's neighbours).
- [V] Following the player: BF2 contextual follow / stop, BF1 D-pad orders to everyone in 30-45 m (re-squad-orders).
  BF2's follow agent walks its followers to Cover hints 5-30 m from the leader.

## 7. Turrets and vehicles [O]

### 7.1 Who is told, who accepts (BF2) [V] (AGENT_C 1; verifier C1-C3)
- The scan `FUN_002e2e38` visits each registered vehicle once per 31 AI steps; skips one moving faster than 3 m/s, one
  with no free seat, one standing inside a barrier; gathers up to 30 objects of a friendly or neutral team within
  `DAT_00698454` = 50 m, nearest first, and sends their AI controllers message 1. No chance roll, no cap, no test of
  the bot's order. Classes with NoEnterVehicles are never told.
- Acceptance `FUN_002c2428` (B2:247557-247608): soldiers only; not a team under `ForceAIOutOfVehicles`; a free seat
  for the team (reservations live 10 s);
  the member's board cooldown over; a vehicle that reports a speed (hover, walker, flyer) -> yes; **a turret (speed
  0) -> only with a live threat in the agent's target holder whose last point lies inside a turret weapon's yaw /
  pitch arc** (`FUN_002ad408`, B2:232302-232420 - no range test). With `SetStayInTurrets` (Endor's three scripts
  only): every `UnitType` but support and pilot boards with no target.
- [L] 39 of 45 and 63 of 71 Kamino board spells (all turrets) began from the combat agent; on Polis Massa (hovers)
  32 of 45 began from the capture agent.

### 7.2 UnitBoardAgent (BF2) [V] (`FUN_002c3fb0` / `FUN_002c42d8`, B2:248749-248999; verifier C4)
Walk to the seat; finished 30 s after the state was entered, or 10 s at the vehicle; within r1 + r2 + 1.5 m -> the
enter request. **When the path follower is not running the agent re-enters its walking state, which stamps the
clock again** - so a walk that fails at once is re-issued at every think and the 30 s never runs out.
[L] (`tools2/board_bf2.py`) Kamino: 55 board spells, 565 s; 6 of them 32-85 s long (291 s) with the bot standing
34-44 m from its turret and the stamp re-written 11-42 times; they ended in a fight or the capture's end. Bots that
did board walked 60-80 m in 13-20 s.

### 7.3 Leaving a turret (BF2) [V] (AGENT_C 2; verifier C5-C8)
- A seated gunner's controller maps the capture order (and 9-0xc) to UnitWaitAgent. Its update (`FUN_002cd2e0`,
  B2:255069-255160): on a seat whose entity reports speed 0, at each think: a live threat -> clock = now; **no live
  threat and the clock older than `DAT_00697e40` = 10 s -> get out**. Any think with a threat - any distance, any
  direction, seen or remembered - resets it, and each fight from the seat ends in a fresh wait agent with a fresh
  clock.
- Other exits: damage from an enemy outside every weapon arc (B2:247958-247981); an unboard order (+ 20 s board
  cooldown); a seated bot reaching its post. **No low-health bail for turrets**: `FUN_002a95a8` (every AI occupant
  out) is called only by the hover and walker updates under 20 % health for one AI in four, and two flyer sites.
- [L] The 10 s rule (`tools2/turret_leave_bf2.py`, Kamino): of 36 seated wait spells 30 ended in a new fight after a
  median 1.8 s; 6 ended on foot, five of them with the clock at 9.6-11.9 s, one at 1.8 s (another exit).
- [L] Kamino, two captures: 32 seated spells, 15 over 60 s, the longest 185 s; 15-17 % of alive AI rows seated;
  77-79 % of seated time in the combat agent, 18 % in the wait agent. **What a seated gunner fights**
  (`tools2/gunner_targets_bf2.py`): an enemy turret entity on 32 % / 58 % of seated combat rows at a median 131 / 95 m
  (p90 177 m), a soldier on foot on 59 % / 37 % at a median 73 / 69 m (max 174-195 m). One gunner sat 91 s with no
  enemy soldier on foot within 100 m at any sample.

### 7.4 Vehicles (BF2, the foot side) [V] (AGENT_C 5)
Boarding a vehicle needs no target; the agent follows a moving one. Leaving: under 20 % health for one AI in four;
all-AI crews told out by the vehicle; a driver reaching its post (out + 5 s cooldown unless fast and wanted);
pilots after landing (+ 15 s). The walker side: re-walker-ai 8, 13.

### 7.5 BF1 [V] (AGENT_D 4; verifier D7, D8, D11)
- Offers (`FUN_00241f18`, B1:159089-159204): each vehicle once per 7 rounds (3.5 s) when it has a free seat and moves
  under 3 m/s, to up to 30 friendly or neutral units within 50 m (a team under 26 reinforcements gets none). A
  building (turret) skips the "2 m clear for a soldier" test. InterruptHelper test 6 draws one offer at random; the
  running agent must allow it, and for a seat with speed 0 that needs a tracked target the turret's weapons can bear
  on (`FUN_002441e0`, B1:160595) - **the same turret rule as BF2**.
- UnitBoardAgent (`FUN_002446e8` / `FUN_00244848`, B1:160767-160945): ONE walk to the vehicle's position; each think
  checks the vehicle exists, may be entered, its reservation (a 3 s record the think itself refreshes) and speed
  under 2 m/s; boards inside r1 + r2 + 0.5 m. **No timer and no look at the walk's result.**
- Leaving a turret: UnitWaitAgent `FUN_0024cb60` (B1:166785-166850): the same 10 s with no tracked target.

## 8. Difficulty [V]
- What the level changes is in re-ai-difficulty 2-4: per-team skill (BF2 enemies 4 / 14 / 17, the human's AI team
  mates 6 / 11 / 11), and from skill the aim error, tracking, burst length, grenade chance, view distance. The level
  has four readers in BF2 and all are listed there (AGENT_H Q7).
- **Correction to re-ai-difficulty.md 2.4 / 9.4**: the third level's mode weights are {1,1,5,1} (ELF 0x6215e8), not
  {1,1,1,1}: Elite bots with over 10 m of open ground zig-zag 5 times in 8. And the mode is not drawn afresh: n is
  fixed per controller (4.4). BF1's twin is the skill-tier table of section 5.
- Not a difficulty term, but the largest single lever on how bots feel: the level of detail (1.3) - a bot in front
  of the player (level 4) decides 12-16 times as often as the same bot at levels 0-1.

## 9. BF1 against BF2

| topic | BF1 | BF2 |
|---|---|---|
| think rate | every 0.5 s; 1.0 s over 45 m from the camera and out of its near view | factor x {4, 3, 2, 1, 0.25} s by the nearest human's distance, the bot's cone and the camera |
| soldier input | every frame; every other frame far and unseen | the same skip; the mover itself every 5th / 3rd frame at low levels |
| interrupts | InterruptHelper polled first in every agent think | messages delivered to the agent, pushed as orders |
| capture walk that fails | "done" - the at-post helper then waits for the post to change hands, no retry (10c) | the capture agent ends; the goal gives a new target within 1 s |
| re-pick of the post | events only | the goal's 1 s tick for a member without an order |
| board agent | no timer, one walk | 30 s / 10 s limits; the walk re-issued when it stops (and the 30 s with it) |
| turret rule | target the turret can bear on; 10 s without a target -> out | the same |
| low tickets | under 26: no vehicle offers, no snipe hints, no repair detours | none found |
| perception | 77.5 deg cone | the cone + 10 m all round; view multiplier per level |
| heard | within min(radius, 30) m and weapon range: face it | within 30 m: walk to it (10 s); within 3 m: fight |
| a shot passing within 7.5 m | face that point (a fight order at it), then look around | evade: run 20 m away from it or roll, then look around |
| fight style | n % 6 by enemy skill tier | n % total by level weights and open space |
| cover | once per 16-25 s, p 0.5; prone allowed | every 2-5 s, p 0.5; stand / crouch only |
| at-post hints | Patrol hints used | dead code: always wander |
| stuck walk | done over 10 m away | done over 6 m away |
| squad | D-pad orders to all within 30-45 m | reticle follow / stop, followers take cover hints |
| agents | Wait, Combat, Scout / Repair / Trooper, WaitSecondary, Board, UnBoard, Random, Follow, Hold | + CTFOffense, Deathmatch, Defend, Destroy; scout and repair folded into the trooper agent |

## 10. The maintainer's three annoyances

### 10a. Turrets are a bot trap
- **The rule** (7.1-7.3, 7.5; the same in both games): a bot is offered every parked vehicle and turret within 50 m;
  it takes a turret only while it has a live threat whose last known point the turret's weapons can bear on. It gets
  out after 10 s of thinks with no live threat; when hit from outside every arc; on an unboard order; or dead.
- **Why they "rarely leave unless they're being shot at or die"** [V/L]: (1) the clock is reset by ANY live entry of
  the threat list - not "a target this turret can hit": a remembered soldier (the list forgets by score decay, about
  10 points a second), an enemy beyond weapon range, an enemy turret; (2) the matrix lets a gunner attack buildings,
  and a turret perceives another turret at 100-180 m (view distance grows with the target's radius): on Kamino a
  third to a half of a seated gunner's fight is an enemy turret at a median 95-131 m; (3) every fight from the seat
  ends in a new wait agent with a new 10 s; (4) there is no health bail-out for turrets and no "the fight moved"
  test; (5) nothing counts tickets or enemies left (BF1 stops OFFERING under 26 tickets, which keeps seated gunners
  where they are). So two gunners who see each other's turrets hold each other in the seat to the end of the round.
- [L] PS2 Kamino: 15-17 % of alive bot time seated, 15 of 32 seated spells over 60 s, the longest 185 s (a 4-minute
  capture cuts longer ones); one gunner 91 s with no enemy soldier on foot within 100 m.
- Not in the code: any link between a turret and a post, a quiet-time limit while a far target is remembered, an
  endgame rule.

### 10b. Dead parts of maps
- **The rule** (2, 3): a bot walks the authored route table from where it stands to its target post's hub, a random
  point in each hub on the way. Off that route it goes only: to a Cover hint within 40 m in a fight; to a Snipe hint
  within 100 m (scout classes); to a Mine hint at an own post; to a turret or vehicle within 50 m; 30 m toward a
  noise; 15-30 m by its fight mode; 8-14 m round its post; +-10 m when stuck. Nothing explores, patrols or picks a
  route by what happened on it.
- [L] Hub census (`tools/hubuse.py`: a hub is visited when an alive AI soldier's feet come within max(radius, 3 m) of
  its centre in XZ; `tools/hubroutes.py`: the hubs on some authored route between two post hubs):

| map | PS2 AI unit-seconds on foot | soldier hubs never visited | of them off every post-to-post route | connections never walked |
|---|---:|---:|---:|---:|
| BF2 Kamino | 32,519 | 45 of 142 (32 %) | 45 | 48 of 168 |
| BF2 Polis Massa | 21,338 | 52 of 185 (28 %) | 52 | 48 of 216 |
| BF2 Death Star | 16,100 | 9 of 107 (8 %) | 7 | 8 of 128 |
| BF1 Kashyyyk Docks | 21,059 | 36 of 149 (24 %) | (no post list in the capture) | 37 of 190 |
| BF1 Naboo Plains | 7,795 | 0 of 8 | - | 0 of 11 |

  75 of Kamino's 142 soldier hubs, 115 of Polis Massa's 185 and 38 of the Death Star's 107 lie on NO post-to-post
  route; the bots still reached 30, 63 and 31 of those (spawn points, fights, cover, snipers, turrets).
- The never-visited hubs (`lead_tmp/sair/hubuse_<map>.txt` has positions): **Kamino** Hub0, 5-17, 23-33, 39, 64, 124,
  127-129, 136, 141, 142, 145, 146, 148, 149, 162-165, 167-169 (the platforms round (73..161, 99..147) and (67..153,
  248..294) and the west end); **Polis Massa** Hub82, 121-131, 149, 153, 168-178, 181, 182, 187, 188, 192-195, 210-212,
  215, 218, 220-222, 227, 236, 241, 243, 252, 254, 260-263, 273; **Death Star** Hub14, 16, 17, 44, 53, 81, 86, 120, 122;
  **Kashyyyk Docks** Hub96-99, 101, 121, 123, 124, 141, 149-152, 155, 159-163, 173-175, 177-179, 184, 185, 194-202.
- Busiest: Kamino Hub53 holds 27 % of all hub-time, the Death Star's Hub47 13 %, Kashyyyk's Hub100 17 %.
- So "you just know where everyone is going" is the route table: 69 % of disc hubs have one authored next hop per
  destination, and the per-hub random point is the only spread. Coruscant was not captured.

### 10c. BF1 bots that just stop
- **Not** a sleep and not a missing goal: BF1 has no AI sleep (1.6: the far level only halves the think and the
  frame rate) and the stopped bots hold live orders.
- **The mechanism** [V]: a BF1 walk is requested once. If the route answer holds no path, the GroundGotoHelper sets
  its failed flag (helper+0x20), goes to its stopped state 7 and from then on clears the unit's mover every think
  (`FUN_002509b0` B1:169749-169751, `FUN_00250868` B1:169681-169695). Nobody who owns a walk reads that flag:
  - UnitBoardAgent (7.5): no timer, no walk test - it waits to be inside the vehicle's reach, refreshing its own seat
    reservation, for as long as the vehicle stands;
  - UnitTrooperAgent: "walk done" (true in state 7) counts as arrived; the at-post helper then walks "to the post"
    again and waits for the post to turn friendly, with only a one-shot 5 s check that lets an enemy post pass
    (`FUN_0024baf0` B1:166111-166186, `FUN_00255fd0` B1:173508-173558);
  - UnitRepairAgent re-requests and fails again [L; its think not opened].
  The stuck rescue needs a live mover path, so it never fires for a failed walk.
- **What wakes it**: the InterruptHelper - an enemy inside the bot's cone and view distance, a shot passing within
  7.5 m, damage, a squad order. That is "when you get close they start moving and shooting". After the fight the
  capture order resumes (and can fail again); the board order does not.
- **Why late in a round**: a stalled bot is freed by events - death, an enemy in view, the vehicle taken or
  destroyed. With two or three enemies left there are none. Under 26 tickets no new offers are made, so no new board
  stalls start, but the standing ones stay.
- [L] RAM census (`tools2/stall_census_bf1.py`, 37 BF1 save states, 1,462 AI on foot): 43 bots (2.9 %) with a failed
  walk and not fighting; 75 in the board agent, 59 of them heading for a turret; **14 board bots with a failed walk:
  12 turrets, 2 hovers - the turrets 4.5-18 m above or below the bot, 19-47 m away** (Kashyyyk Docks 11, Rhen Var
  Harbor 1, Mos Eisley 2). The offer is a 50 m sphere and a turret is exempt from the "clear for a soldier" test, so
  bots are sent to tower and roof turrets they have no route to.
- [L] Live (`tools2/stall_raw_bf1.py`, Kashyyyk Docks, the 17-minute-old state, 213 s): bots 8, 19, 62, 34 stood
  109-213 s in board state 2 with the walk failed and no mover path; bot 40 147 s with the walk ended short of the
  seat; bots 7, 12, 26 (repair agents) 82-122 s re-requesting a failed walk; bot 20 100 s in the at-post helper's
  wait with a failed walk; two snipers stood at hints (not stalls). On Naboo Plains no spell over 60 s.
- **What BF2 changed** [V/L]: the capture agent ends when its mover fails and the goal re-draws within a second;
  the board agent has 30 s and 10 s limits. But a walk that fails at once is re-issued every think and the 30 s
  restarts (7.2): six Kamino bots stood 32-85 s at 34-44 m from a turret, freed only by a fight. BF2 shows it less
  because a fight (and so an interrupt) is never far on its maps, and its bots also stand for other reasons: snipe
  stands of 1-4 minutes, and thinks 4-8 s apart away from the player.

## 11. Replays against live RAM

| # | rule | data | result | residual |
|---|---|---|---|---|
| 1 | BF2 think interval = factor(agent) x table[level] (1.2) | 4 captures, 292,062 readings | 291,678 exact (99.87 %) | 384 "think now" re-schedules |
| 2 | BF2 level of detail: the cone term (1.3) | 2 captures, far bots, 6,290 fresh readings | inside 75 deg: level 2 on 96.8 % / 94.3 %; beyond 90 deg: level 1 on 91.6 % / 93.3 % | the 75-90 deg bin splits; the render-mark term is not modelled |
| 3 | BF2 level of detail, whole rule without the render mark | 4 captures, 10,777 readings | 87-92 % exact | one level, except the dead-player camera (139 of 829) |
| 4 | BF2 engagement mode = f(n, level, space, target kind, UnitType) (4.4) | 2 captures, 7,386 on-foot attack rows with an aimed entity | scouts 488 / 488; others vs soldiers 95.2 % (Polis Massa), 73.6 % (Kamino); vs turrets 65 % | mode 2 where the draw gives 3 - the offhand-weapon path, not in the capture [H] |
| 5 | BF2 gunner leaves after 10 s with no threat (7.3) | Kamino, 36 seated wait spells | 5 of 6 exits at 9.6-11.9 s idle; 30 spells ended in a fight first | 1 exit at 1.8 s (another cause) |
| 6 | BF2 a turret is boarded from a fight, a vehicle from anything (7.1) | 4 captures, 206 board spells | Kamino 87-89 % from the combat agent; Polis Massa 71 % from the capture agent | - |
| 7 | BF1 think skip: over 45 m and outside the 78 m view only (1.6) | 3 captures, 66,438 readings | within 45 m 0.2 % set; beyond, out of view, 43.6-49.7 % | 45-78 m in my view model 17 % (approximate frustum) |
| 8 | BF1 a failed walk is never retried (10c) | 1 capture + 37 save states | 8 bots standing 82-213 s with the failed flag set and 1 stopped short of its seat; 14 board bots failed in the census | why each route fails was not derived |

Measured, not replayed: the agent, helper-state and sub-state shares (1.4, 4.2, 4.6), the fight spells (4.11), the
seated spells and gunner targets (7.3), the hub census (10b). Every number above was re-run by the lead from the
stored captures (`lead_tmp/sair/ps2/`); the tools are `tools/` (first session) and `tools2/` (this one).

## 12. PS2 | the port | difference

The port as of main 142dd4b (`core/src/ai/`, `commander.rs`, `perception.rs`; AGENT_E.md is the full reading with
file:line, taken at b1cffcc; JITR ac605ca and FFLN 142dd4b landed since and are folded in). D = a plain defect
against the PS2 (a card or a note on one, section 14); B = a choice for BOTD.

| item | the PS2 does | the port does | difference |
|---|---|---|---|
| think rate | factor x {4, 3, 2, 1, 0.25} s by the player's distance, the bot's cone and the camera; BF1 0.5 / 1 s | every 10 ticks (0.33 s) for every bot; aim, fire and steering every tick | B: the port's bots all decide like a PS2 bot standing beside the player |
| orders | a queue of orders -> one agent at a time; interrupt and resume | optional fields on one state, priority lists, one pad writer | B (BOTD's layers) |
| next hub | weighted random draw over the authored table per hub, shared LCG | the same table and draw, a per-unit hash for the LCG; BFS fallback | matches (the fallback is ours) |
| point in the hub | a random point on a sphere of the hub's radius, per unit per leg | the same since card HUBP (2026-10-06, `ai/route.rs` `hub_point`): per unit per leg from the unit's hash, again while in a barrier; OURS: only a point the hub's own floor and the floor walked from the unit lead to, the centre for a vehicle and on a stuck unit's re-plan; 5 m ahead on the leg's line (waypoint to waypoint) when off it | was **D** (single file through hub centres). After HUBP a team mate within 1.5 m, 8 seeds: KAS2 9.5 -> 2.9, DEA1 18.8 -> 11.6, KAM1 9.9 -> 5.8, POL1 18.9 -> 13.9 % (PS2 2.0 / 9.3 / 6.6 / 5.7); what is left is bots standing at posts and POL1's hubs under 2 m (cards TEAM, TPOS) |
| leg steering | projection + \|v\| + 0.2 m (BF2), straight at the point (BF1); arrive 1.0 m / 0.5 m / plane passed | straight at the point; arrive 1.5 m or the plane | D (small): arrival floor and look-ahead |
| barriers | corner walk per leg, side chosen once | corner walk (A* over corners), once per leg | matches in kind |
| walls, props | not seen: only barriers and the contact push | a capsule sweep 1 m ahead: slide, flip, fan, back, ledge | B: ours; OBST |
| other units | none before contact; contact -> 5 m side-step from the unit's own position | none in the AI; the sim pushes overlapping soldiers apart; a side-step only from the stuck window | D: the contact side-step is the PS2's unit avoidance; B for more |
| stuck | 2 m in 3 s / no progress 5 s -> +-10 m random walk until 6 m away | 60 ticks under 0.3 m -> re-plan + 5 m side-step; lost walk; 120 s stall recycle | B |
| perception | cone 77.5 deg + 10 m (BF2); one sight ray that SOLDIERS block; {1..5} candidates per think by level | the cone and 10 m; a ray over terrain, bodies and hulls, not soldiers; up to 6 contacts | D (small): on the PS2 any soldier on the sight line hides the target; FFLN's line check covers the friend case |
| hearing | every enemy within min(radius, 30) m: walk to it up to 10 s; within 3 m fight | shots within 30 m and whoever hit me become contacts; no walk toward them | **D**: the "go to the noise" order is missing - over half of Polis Massa's fight time (FGHT) |
| a shot passing | within 7.5 m: BF2 run 20 m away or roll; BF1 face the point | nothing (only the grenade flee) | D |
| being hit | out of a fight: walk to the attacker, or a 20 m dash / roll; in an attack: ignored; no delay before the answer | attacker becomes a contact, fired at after a search if unseen (OURS) and a 0.2 s flinch (OURS); a dodge by chance (side-step, strafe or roll) | B |
| fight movement | a per-bot style: stand / circle at optimal range / zig-zag / flank; a band: back off 15 m inside, advance 15 m beyond | walks its route; stops within 25 m of a seen enemy (HOLD) or holds at a hub; JITR's commit times | **D/B**: no fight order, no range band, no modes (FGHT) |
| cover in a fight | Cover hints within 40 m facing the target: 21-49 % of attack time; hide / fire / lean cycle | Cover / Snipe hints as hold or defend spots; none chosen against a target | **D/B** (BOTD's tactical positions; the PS2's test is the cheap version) |
| fire | whenever the aim is within 22.5 deg (and inside range); bursts by skill; while moving | yaw and pitch error under 0.035 rad (2 deg); reaction timer; bursts 2-6; while walking | B: the port's trigger is 11x tighter and has a reaction time the PS2 lacks |
| weapons | best in-range weapon by damage per second against the target type, every combat think; grenade windows; one grenade thrower per target | the primary only, never switched; no grenade | **D**: card GRAI |
| vehicles as targets | only with ordnance that can hurt them; modes 6 (behind) / flank / stand by n | targets a vehicle only through its rider | D (with GRAI) |
| stances | crouch for a third of the bots when standing to shoot; stance from a cover hint; never prone (BF2), never sprint | a hint's stance on a hold; roll by tier chance | B |
| jets | jet classes hop every 6-12 s in a fight, use JetJump hints, fly arcs | never jets | D: card JETP |
| own health, ammo | no rule at all | no rule | matches (B: the maintainer wants better) |
| line of fire | holds fire for a HUMAN friend on the aim ray only | no shot while a friend, AI or human, is within 0.75 m of the aim line out to the target + 5 m (FFLN) | B: a named improvement (FEEL_NOTES); the PS2's own veto covers the human only |
| friendly fire | AI never hurts a human friend; bots hurt bots at the option's percentage | scale 1.0; a bot's bolt is dropped on a human of its team (FFLN) | matches |
| spacing, team sense | none | none in the AI; JITR's 2 m between holders | B |
| snipe hints | scout classes only: stand 60-120 s on the way to the post, range x 2 | scoped weapons take a Snipe hint as a hold / defend spot | D/B |
| at a post | BF2 wander 8-14 m for 20-30 s; BF1 Patrol hints | stands at a random point of the region; a defender at a Cover / Snipe hint within 25 m | D (small): BF2's wander is missing; the hint use is BF1's |
| turrets | board with a target in the arc within 50 m; leave after 10 s with no threat | never boards a turret | **D/B**: seat time 0 % against the PS2's 9-18 %; the maintainer wants a purpose rule, not the PS2's |
| vehicles | board any parked one within 50 m; bail under 20 % health (1 in 4) | claims one within 40 m when the post is over 80 m away; no bail | D: WAI2, VBAIL |
| engineers, pilots | repair vehicles / turrets, heal and resupply HUMANS within 75 m; lay mines at Mine hints | none | **D** (a card, section 14) |
| cloak, buff, auto-turret | three one-line rules | none | D (with the above) |
| difficulty | skill per team; mode weights; nothing else | skill curves + tier columns (view, reaction, pressure, damage) | B |
| the corpse shot | rebel troopers beside the player shoot a dead enemy | none | B (skip?) |
| failure | BF1 none (stalls); BF2 limits that a failing walk resets | lost return, stall recycle, dwell give-up | B: the port already does better; keep no agent without an exit |
| map use | post-to-post routes only: 8-32 % of hubs never visited | the same table | B: the maintainer's weight tilt |
| facing on a walk | the body turns to the aim order, and there is an aim order only in an attack (1.5, 4.2); a vehicle its weapons cannot hurt is not attacked (4.3); no shared contacts (6). PC BF2 fact: the trooper agent runs its attack helper in its Goto and Patrol states (`UnitTrooperAgent::UpdateStatePostVision` 0x0066a980 -> `AttackHelper::Aim` 0x00447c50), aiming at a visible threat or its last known point while walking | since playtest C3 (2026-11-10), a bot walking a leg faces its walk unless its target can be fought: seen, within its longest weapon range + 10 m, and not a rider it cannot hurt; told records and remembered riders are not faced on the walk; ground rounds only (`ai/behave/brain.rs`) | matches the PS2; BF1 Hoth's travelling bots facing more than 60 deg off their walk 13.5 -> 4.9 % (4 seeds, Hard; 4.1-4.5 % with no target) |

## 13. What in the PS2's bot is weak or annoying by its own code (facts; BOTD decides)

1. **Decisions slow down with distance from the player** (1.2-1.3): 4-8 s between thinks behind him or far away;
   the mover at 6-10 Hz. A capturing bot can walk 50 m past an enemy between two looks.
2. **No awareness of friends**: not in steering (single contact push), not in fire (humans only), not in target
   choice, not in positions. Bots shoot bots at full damage.
3. **A fixed style per bot**: the same bot always stands, circles or flanks; a third always crouch. Nothing reads
   the situation except the open space and the range band.
4. **Cover is a facing test**: a Cover hint counts when its arrow points at the target - no ray. A hint behind a
   pillar that does not block that enemy is taken; real cover without a hint is never used. The bot fires from the
   "hidden" stance too.
5. **No reaction to health, ammunition or odds**: no retreat, no heal, no regroup; a hit during an attack is ignored.
6. **The trigger is loose** (22.5 deg): shots leave while the aim is still swinging; accuracy is only the error model.
7. **A fight never closes or pursues**: beyond range + 10 m it is dropped; inside, the bot orbits its optimal range.
   "Go to the noise" is the only approach, and it is deaf beyond 30 m.
8. **Turrets** (10a): any remembered enemy keeps a gunner seated; turret-against-turret duels; no health bail; BF1
   and BF2 both send bots to turrets they cannot reach.
9. **Walks that fail** (10c): BF1 never retries and never times out; BF2's board limit restarts.
10. **Routes** (10b): a table walk with no memory; a quarter to a third of a map's hubs unused; one hot hub.
11. **Stuck handling is a random walk** of +-10 m; no wall or prop is seen before the unit is against it.
12. **Snipers stand 1-2 minutes at one hint** whatever happens short of seeing an enemy, then usually return to it.
13. **Engineers serve only humans and machines**: an AI never heals or resupplies another AI, or itself.
14. **Dead code that looks like features**: BF2's hints at posts, the snipe suitability re-check, "lost sight ->
    search the last known point", "run away" (sub-state 9); BF1's prone at hints is live, BF2's is not.
15. **Grenades by the clock**: a 3 s window every 30 s per bot, not by the situation - except against a cluster.
16. **Vehicles are not targets** for a rifleman at all (the "too weak" rule): he ignores a tank until someone with a
    launcher or a grenade sees it.

## 14. Checks, the verifier, what stayed open

**Self-checks** (`checks/re_check_lead.txt` holds the output): `re_check.py word` on the interval table 0x621bc8,
the mode tables 0x6215b8 / c8 / f8, 0x5fe084 = 15, 0x5fcbe0 = 1.0, BF1 0x517438 = 70.0 / 8.0; `writers` on
`DAT_00697e40` (3: default, `SetStationaryWaitTime`, `SetStayInTurrets`), `DAT_00697e44` (2), `DAT_00697e30` (2),
`DAT_005fcbe0` (1), `DAT_00698454` (2), `DAT_006980a0` (1), `DAT_005fdfe2` (2), BF1 `DAT_005873a4` (3),
`DAT_00517508` (1), `DAT_0051814c` (5); `callers` on `FUN_002b7e50` (1), `FUN_002a95a8` (4), `FUN_002ee6f0` (3),
`FUN_002c2428` (1), `FUN_002e2e38` (1), `FUN_002baa20` (7), `FUN_002bb900` (1), `FUN_002d9620` (6), `FUN_002d0550`
(2), `FUN_002aef80` (1), `FUN_002cf100` (1), BF1 `FUN_002638b8` (1), `FUN_0023cfc8` (1), `FUN_00236508` (2),
`FUN_00236e20` (4). `tools/citecheck.py` over this note: 145 line-function pairs, 133 inside the named function, the
12 others ranges that span the two or three functions named with them. Live defaults read in
save-state RAM: level 2, stationary wait 10.0 (both games), StayInTurrets 0, notify radius 50, friendly-fire scale
0.99999994 (both), `DAT_006980a0` 2, BF1 skill thresholds 14 / 17. The absence claims rest on the searches listed
in AGENT_A (Q6), AGENT_B (5), AGENT_F (S1-S12), AGENT_H (1.4, 4.3) and the verifier's own.

**Verifier** (Opus, claims and citations only; `verify/` = the first session's unfinished pass, 14 claims; `verify2/`
= this session's, 70 claims, 26 minutes, three sub-verifiers, 21 of their 51 verdicts re-opened by the lead
verifier). Together 84 claims: **50 SUPPORTED, 33 PARTLY, 1 NOT SUPPORTED**. Every correction is in the text above:
- NOT SUPPORTED: "the soldier's AI body is `FUN_001f02e0`" - that is EntityDroid's; the soldier's is `FUN_00249f80`
  (1.5 re-derived from it by the lead, with its every-other-frame skip and last-step scaling).
- PARTLY, load-bearing: the line-of-fire list was stated the wrong way round (it holds the nearby human friends);
  UnitBoardAgent and UnitRandomAgent also replace rather than resume; the contact push needs a contact kind; own-health
  reads exist outside the conquest path (4.8); BF1's turret offer DOES test the target (7.5); the BF1 at-post helper
  has a one-shot 5 s check; BF1's cover draw is once per 16-25 s; cover scores are XZ; the snipe box is +-100 m per
  axis; the state-5 update has two early outs; Elite weights {1,1,5,1} (first pass).
- Added after the verifier's pass and read by the lead only: 4.1 "a shot passing" (`FUN_002a89b8`), the BF2 skip test
  `FUN_002a81c0`, the 1-in-2k draws.

**Still [H] / not opened**: what the render-frame mark on the human's body means (1.3) and which camera the spawn
screen leaves; the contact kinds 8 / 0x10 / 0x20 / 0x40; why a given walk to a turret fails (BF1 and BF2: the route
start was not replayed); the mode-2 residual on Kamino (4.4); the on-hit dodge fields' reader; BF1's UnitRepairAgent
and DroidRepairAgent thinks, SnipeHelper and MineHelper bodies; hint TYPEs "7" / "8"; the BF2 threat list's lifetime
in seconds (decay read, not measured); `FUN_002eea68`'s "clear" sense (taken from its users); whether shipped ODFs
set `AITargetPerson = 0` on launchers; Coruscant's outside turret (no capture).

**Corrections this note makes to others**: re-conquest-goal.md 4 (BF2 has no hint walk at a post); re-ai-difficulty.md
2.4 (Elite weights {1,1,5,1}; the mode is a fixed function of the controller's number); re-blaster-damage.md 6.3
(a line-of-fire veto does exist, for a human friend).

**Cards**: new HUBP (the waypoint in the hub's sphere), ENGR (engineers and pilots serve vehicles, turrets and the
human; mines, cloak, buff, auto-turret); notes on FGHT (sections 4, 4.11), GRAI (4.3-4.4), JETP (4.7); everything
else in section 12 is BOTD's to decide. Card SAIR closed.

## 15. Read after the note: the noise radius, and what card FGHT measured (2026-10-07)

Card FGHT's build session wrote 15.1-15.3; **card FGHT2's session (2026-10-09) ran the verifier on 15.1** (23
claims: 19 supported, 3 partly, 1 not supported - the corrections are the "Verifier" bullets of 15.1 and are folded
into its summary), opened BF1's twin (15.4) and measured where the bots die and at what range (15.5), which
corrects 15.2's reading of the zone. Nothing landed rests on 15.1.

### 15.1 The radius a unit is heard at (BF2) [O]
4.1 says "within min(the emitter's radius, 30) m" and left the radius unopened.
- `FUN_002de708` (B2:266751-266759): r = C vt(+0x14)+0x38 (); 100 when the byte ai+0x80 is set, which is then
  cleared; r = min(r, 30); when the emitter is no soldier (class test `DAT_00689828`) or its byte E+0x7e3 / 255 is
  under 0.1, r = max(r, 8); nothing is sent when r <= 0. The query is `FUN_001aef10(r, pos, .., 0xaf, self, 1)`:
  mask 1 = enemies (AGENT_H 1.2). Callers: the think (B2:239236) and `FUN_002b02f8` (B2:234449).
- A soldier's vt+0x38: the controllable's vtable is 0x61e998 (C+0x14 on 29,846 of 29,846 soldier rows of the Kamino
  and Polis Massa captures, `lead_tmp/sair/ps2/*_v2_raw.pkz`); the word at 0x61e9d0 is 0x002558b8, a thunk to
  `FUN_0024dbf0` (B2:167302-167347): max(half of `FUN_00326968` of the object at E+0x270 when there is one [H: what
  it is was not read], clamp(class+0x8d0 x |velocity|, 0, 50)).
- class+0x8d0 = 5.0: the soldier class constructor `FUN_00250488` writes 0x40a00000 (B2:169621), and all 33 live
  soldier classes of `ref/states/bf2_kam1_ai16_gcw_sideselect.p2s` and `bf2_kam1_walk_cp0_cp1_b000.p2s` read 5.0.
  The three block copies in `FUN_0024f0b0` (B2:168330, 168365, 168400) were not opened [H].
- ai+0x80 is set by the trigger `FUN_002bb1b0` (B2:242358): the unit fired. It is 1 on 5,324 of 29,846 rows (18 %).
- E+0x7e3 is 0 on 28,866 of 29,846 rows (97 %); what the byte is was not read [H].
- **Verifier (2026-10-09), four corrections, each re-read by the lead:**
  - the "object at E+0x270" term is the WEAPON in the hands: `FUN_002f5950` returns the object of the
    controllable's current slot, and `FUN_00326968` a float out of a handle picked by the weapon's state (states 1-4
    the handle at +0xe0, states 0 and 6 the one at +0xe8 behind the virtual +0xdc, else 0). What the float is (a
    sound's audible distance, by its place [H]) was not read. The radius is max(half of it, the speed term);
  - ai+0x80 has a second writer: `FUN_002b0548` (B2:236095-236097) sets it when bit 0 of C+0x38, +0x3c, +0x44 or
    +0x60 is set; it is the PlayerController's update (ELF word at 0x621a14 = vtable 0x6219f8 + 0x1c; the string
    after that vtable is "PlayerController"). And the trigger `FUN_002bb1b0` sets it when the trigger BUTTON is
    down at its end, not when a shot leaves. **A human who holds fire is heard at 30 m like a bot**;
  - the 8 m floor is a soldier's default: `DAT_00689828` is the soldier's own class id and the constructor
    `FUN_00234a68` zeroes E+0x7e3 (B2:149997); the vtable 0x61e998 is in the text too (B2:149409, same function);
  - two schedules reach the pass: the think (`FUN_002b7530`, the timed queue) and `FUN_002b02f8`, a bare wrapper
    called from `FUN_002af788`'s loop over the humans' list `DAT_005fe088` every 15 steps (B2:234007).
- **So a BF2 soldier (a bot at its think, a human every 15 steps) is heard by every enemy AI unit within
  min(max(half the weapon's term, 5 m per m/s of its speed up to 50), 30) m - 30 m outright when its trigger was
  down since the last pass - and never under 8 m, standing still included.** [O]
- No other function writes a constant to class+0x8d0: the constructor's 5.0 is the default; `FUN_00250dc8`
  (B2:169890) copies the parent class's value, `FUN_0024f0b0` / `FUN_0024f350` write a matrix into the soldier
  ENTITY at the same offset, B2:766053 is unrelated [receipt:53034a73] (the verifier's raw scan of the stores agrees).

Checks run: `re_check.py fn b2 266751 266754 266756 266769 266822 266838` (all `FUN_002de708`), `fn b2 169621`
(`FUN_00250488`), `fn b2 242358` (`FUN_002bb1b0`), `fn b2 236096` (`FUN_002b0548`), `word b2 0x61e9d0 1 --u32`
(0x002558b8), `word b2 0x621a14 1 --u32` (0x002b0548), `callers b2 FUN_0024dbf0` (1: the thunk), `callers b2
FUN_002de708` (2), `callers b2 FUN_002b02f8` (1: `FUN_002af788`). Claims and verdicts:
`engine/artifacts/lead_tmp/fght2/verify/{CLAIMS,VERDICT}.md`. Stayed [H]: what the weapon's float is; what E+0x7e3
means (its two non-zero writers `FUN_002376a8` and `FUN_0040eae0` were not opened); that E+0x550 is the velocity.

### 15.2 The noise order is NOT what brings the PS2's bots within 20 m [L]
4.11 and section 12 name "go to the noise" as the likeliest source of card FGHT's contact gap (an enemy within 20 m:
BF2 Kamino 3 % on the port against 13-15 %). Built on the port (hearing by 15.1, the walk over the graph to the
noise's hub, at most 10 s, a look, the fight on sight) and measured over 8 seeds (`lead_tmp/fght/ab/k2_f1`):
- its share of on-foot time is near the PS2's (Kamino 2 %, Polis Massa 17 %; the PS2's 10 % and 54 % of a combat
  time of 20-28 %), and contact does not move: Kamino 3.5 -> 3.2 %, Polis Massa 25.3 -> 22.4 %;
- on Polis Massa ten bots of one team walked back and forth on the floor over one sniper they heard and could not
  reach until the last resort took them (`lead_tmp/fght/bb/bf2_pol1_s4.json`; recycles 7 -> 20 in 8 rounds). With a
  storey rule and a 10 s pause after a walk that found nobody the share falls to 3-4 %, and on top of the landed
  fight it makes Polis Massa worse (`k5_f18` against `k6_f19`: deaths a minute 18.7 -> 13.8, a teammate within
  1.5 m 12.5 -> 21.7 %, recycles 10 -> 27).

Where the PS2's contact is (`bf2_kam1_v2_units.csv`, 72,165 AI samples, 12.6 % with an enemy within 20 m):
- **by state**: 38 % of those samples are bots in order type 8 with no combat helper - on their capture order, not
  fighting (8.5 % of that state's time); 24 % in the attack (44 % of the attack's time is inside 20 m); 14 % in
  order type 4; 8 % go to the noise; 5 % in a seat;
- **by place**: the zone x -40..10, z 80..145 holds 21 % of all AI samples - 11,633 of team 2 [H: its home posts]
  and 3,505 of team 1, 76 % of the latter with an enemy within 20 m. On the port (8 seeds) the same zone holds
  12.5 %, and the other team's samples there are 0.9 % of all (the PS2: 4.9 %): **the PS2's attackers get into the
  defenders' home zone and brawl there; the port's are stopped on the way** [CORRECTED by 15.5: the box is the
  ground between the posts, the port's attackers get as near their target post as the PS2's do, and what differs
  is the range at which the two sides hurt each other];
- **the look rate** (1.2): a bot on its capture order thinks, and so scans, every 4-6 s away from the player. On the
  port (every 0.33 s) a 4 s rate for a bot that holds no contact takes Kamino from 3.5 to 5.4 % and the Death Star
  from 2.7 to 7.7 %; 6 s with no hold at a hub takes Kamino to 7.1 % and BF1 Kashyyyk 2 to 9.9 % (the PS2: 5.3). The
  strongest single lever found, and not enough.

The port's own cause was nearer: its bots stood at the old close hold's 25 m (distance to the target at a fight's
first sample: median 24.5 m, quartiles 20.5 and 25.6), just outside the measure. Tools: `lead_tmp/fght/tools/`
(`tab.py`, `bars.py`, `near.py`, `mates.py`); the switch build is `lead_tmp/fght/knobs_k6.patch` (`FGHT=<bits>`).

### 15.3 A passing shot (Evade) on the port [L]
The run from an enemy's bolt passing within 7.5 m (4.1), built for bots out of a fight, 20 m away for at most 2 s,
no skill delay, on top of the noise and the fight (`k5_f3` against `k5_f11`): pinball windows on the Death Star
1.5 -> 4.0 % (the PS2 1.5), reversals per bot per 10 s on BF1 Kashyyyk 2 1.10 -> 1.40 (the PS2 0.66). Not landed;
the PS2's skill delay and its "once a second per ordnance" are what the build lacks.

### 15.4 BF1's twin of the noise pass [O] (card FGHT2, 2026-10-09; the verifier's B1-B8)
- `FUN_002375e8` (B1:151160-151213): r = the virtual at vtable(*(K+0x294)) + 0x2c on the controllable K
  (B1:151177); r = min(r, 30) (B1:151178); nothing when r <= 0; else `FUN_00196b40(r, pos, DAT_00517d98, 0xaf, self,
  1)` (B1:151181) and a message to each found unit's controller (`FUN_00263640`, B1:151204). **No trigger byte and
  no 8 m floor**: between the virtual call and the query only the min with 30 touches r. Two direct callers,
  `FUN_0025de30` and `FUN_0025de50` (B1:179337, 179347).
- A soldier's slot: the vtable at K+0x294 is 0x532be0 on 75,663 of 79,051 soldier blocks of three captures
  (`lead_tmp/sair/ps2/bf1_*_raw.pkz`, S+0x534; the rest 0x52ffc8, not opened), installed by `FUN_001fee80`
  (B1:111408); the ELF word
  at 0x532c0c (= +0x2c) is 0x00210c80, a thunk (B1:123847) to `FUN_0020c6d8` (B1:121138-121180): BF2's
  `FUN_0024dbf0` with BF1's offsets - max(half the weapon's term `FUN_002849b8`, clamp(class+0x1910 x |v|, 0, 50)).
- class+0x1910 = **3.0**: the soldier class constructor `FUN_0020d940` writes 0x40400000 (B1:122122), the only
  store to that offset in B1 [receipt:f9dd8249]. BF2's factor is 5.0.
- **So a BF1 soldier is heard within min(max(half the weapon's term, 3 m per m/s of its speed), 30) m; a held
  trigger does not make it 30 m and a soldier standing still with a silent weapon is not heard at all.** The
  weapon's term (`FUN_002849b8`, B1:208620-208651: weapon states 1-5 the handle at +0xfc, states 0 and 8 the one
  at +0x104) follows the firing state, so firing does widen it by that float [H: its size was not read].

### 15.5 Where the bots die and at what range: BF2's slow think is the PS2's close contact [L] (card FGHT2)
Tools `engine/artifacts/lead_tmp/fght2/tools/` (`lives.py`, `rng.py`, `aware.py`; outputs in `out/`). PS2: the
captures with a target post per member (`lead_tmp/botm/ps2/*_player_*`, the player spawned; `lead_tmp/cont/ps2/*`,
the player parked) and SAIR's (`lead_tmp/sair/ps2/*_units.csv`, the aimed entity and the helper state). The port:
main 853ecf0, 8 seeds, the first 540 s, with a read-only observer (objective, capture-region flag, post owners;
its null run prints the gate baseline byte for byte).

| per AI life (PS2: player spawned / parked) | Kamino PS2 | port | Polis Massa PS2 | port | Death Star PS2 | port |
|---|---|---|---|---|---|---|
| an enemy within 20 m, % of AI samples | 15.7 / 17.0 | 3.6 | 31.6 / 31.9 | 22.3 | 19.0 / 24.4 | 5.8 |
| distance to the target post at death, median m | 156 / 150 | 134 | 156 / 120 | 137 | 92 / 73 | 106 |
| died within 30 m of the target post, % | 4.8 / 3.7 | 8.8 | 6.8 / 10.6 | 10.6 | 9.3 / 20.4 | 10.4 |
| lives that reach its capture region, % | 0.0 / 5.5 | 3.5 | 4.1 / 8.3 | 6.4 | 7.8 / 16.9 | 4.9 |
| lives that get within 20 m of the post, % | 4.8 / 10.1 | 6.4 | 8.1 / 15.0 | 12.8 | 14.1 / 33.9 | 7.8 |
| AI samples within 40 m of a post the other team holds, % | 1.3 / 3.8 | 2.2 | 4.8 / 8.1 | 6.4 | 7.7 / 9.9 | 8.7 |
| the nearest enemy at death, median m | 8 / 9 | 22 | 6 / 6 | 10 | 9 / 8 | 21 |
| died with an enemy inside 10 m, % | 56 / 56 | 23 | 77 / 71 | 51 | 56 / 61 | 18 |
| died in a fight (PS2: a fight order), % | 49 / 42 | 93 | 34 / 39 | 95 | 67 / 53 | 93 |

- **The port's attackers are not stopped on the way**: they die as far from their target post as the PS2's and as
  few reach it (the Death Star's parked capture aside). 15.2's box (x -40..10, z 80..145) is the ground between
  Kamino's posts; the attacking team's share of samples there is 2.1 / 5.1 % on the PS2, 0.2 % on the port.
- **What differs is how near the enemy is when a bot is hurt.** By the aimed entity (`rng.py`, SAIR's four BF2
  captures): half of the damage a unit takes from enemies aiming at it is dealt from under 12 / 13 m on Kamino and
  7 / 6 m on Polis Massa; the port's from under 28 and 17 m. Damage per second of aiming, inside 10 m | 20-30 m:
  the PS2 65-90 | 18-29 hp, the port 21-33 | 11-26 - **a PS2 bot's aimed weapon is worth two to four times the
  port's inside 10 m and about the same beyond 20 m** (card CLSE).
- **The PS2's bots are far less AWARE** (`aware.py`; on foot, by the nearest enemy's distance, share in a fight
  order): inside 5 m the PS2 38-57 %, the port 77-99 %; at 10-20 m 32-45 against 57-85 %. Of the damage taken on
  foot, **45-70 % lands on a PS2 bot that is not in a fight order** (seven captures, three maps); the port's 2-7 %.
  Of the PS2's not-fighting samples with an enemy inside 20 m, 73-87 % are on the capture order, the enemy is inside
  10 m in 40-52 % and outside the 77.5 deg cone in only 22-24 %; 57-60 % of those enemies are not fighting either.
  By level of detail the share fighting is 37-47 % at levels 1-2 (93-98 % of bot time; level 4: 74 % on 113 samples).
- **BF1 is not like this**: Kashyyyk Docks (teams 1 and 2; the capture has no target post) has an enemy within
  20 m 5.3 / 5.1 % of on-foot time against the port's 4.7 %, and 23 % of its deaths with an enemy inside 20 m
  against 26 %. BF1 thinks every 0.5-1.0 s (1.6); the 4-8 s capture think is BF2's level of detail (1.2, 1.3).
- **Knock-in on the port** (a switch binary on 853ecf0, `tools/knob_patch.py`, null = main byte for byte; 8 seeds x
  32 maps): a bot with NO contact looks every 6 s, ray-tests one candidate and hears a shot or feels a hit only at
  that think (BF2's level 1) - an enemy within 20 m Kamino 3.5 -> **10.3 %**, Polis Massa 22.4 -> 27.9, Death Star
  5.9 -> 15.8, all 32 maps 10.67 -> 21.45 % (up on 32, paired t +20 over the seeds); died with an enemy inside 10 m
  56 / 79 / 59 % (the PS2's 56 / 71-77 / 56-61); lives reaching the region 11-14 %; kills a minute 30.8 -> 25.9,
  last-resort respawns 67 -> 98, BF1 Geonosis unfinished in 2 of 8 rounds; BF1 Kashyyyk Docks 4.9 -> 13.1 % (the PS2
  5.3: wrong for BF1). The 4 s look alone: 13.97 % (Kamino 6.0, Death Star 7.6). Awareness stays above the PS2's
  (inside 5 m 66-76 % fighting; 11-15 % of damage outside a fight), so the PS2's remaining share is its queued
  reaction to messages and its short fights (a fight spell: the PS2 2.2-3.4 s median, 4.11; the port 4.3-5.5 s).
- **Read:** BF2's close contact is a product of its slow think away from the player (bots walk into each other and
  are shot before they fight back), as its standing-still share is (bot-design 10, "reported, not gated"). Whether
  the port takes it is the maintainer's decision (card FGHT2's `needs:`); it contradicts bot-design 7's 0.33-0.5 s decisions.

## 16. Audit FGHTA (2026-10-09): who the scout rule picks, BF1's twin, and the think a circler's stop is drawn at

Read first-hand for AUDIT FGHTA (card FGHTA, b75d36b), then put to two verifiers with the claims and citations only:
14 claims, 9 supported, 5 partly (wording; folded in below), 0 not supported. The `re_check.py` runs are in
`engine/artifacts/lead_tmp/audit-fghta/verify/re_checks.txt`, the verdicts in `wf/RESULTS.md` there.

### 16.1 The rule's key in both games [O]
- [V] BF2 `FUN_002abc38` (B2:231214-231252) is called by the three soldier-target wrappers (B2:231255-231298) with
  the attacker class's UnitType - class +0x8d4, read through the class vtable 0x61e8d8 slot +0x2c = 0x2565b8
  (`jr ra` / `lw v0,0x8d4(a0)`) - and once by the matrix `FUN_002ac8f8` (B2:231852) with a constant 4. UnitType 0
  with weapon category 1 in range -> mode 2 before the draw; category 1 out of range -> mode 0; another category ->
  the draw `FUN_002aba38`. Other types: category 4 -> mode 7, else the draw.
- [V] UnitType (hash 0x54a7aedf): BF2 `FUN_00255418` (B2:172120-172159) gives scout 0, assault and anti-armor 1,
  support, repair and special 2, pilot 3, trooper 4, anything else 5 (also the class constructor's value,
  B2:169622). BF1 `FUN_002108a0` (B1:123628-123670): scout 0, anti-armor 1, assault 2, support / repair / special 3,
  pilot 4, trooper 5, else 6; stored at class +0x1914 (`FUN_0020f2e0`, B1:123124).
- [V] The weapon category `FUN_002a9b78` (B2:229933-229971): an `OffhandWeapon` (hash 0x2ecd946f -> class +0x1e0
  bit 55, B2:320343-320349) -> 2; a weapon whose vtable slot +0x60 answers -> 4; else by the base animation type of
  `AnimationBank` (hash 0xa7678f8f -> class +0x20, the text after the first `_`, B2:320606-320615; the ELF table
  0x5fd848 = rifle, bazooka, tool, pistol, melee; the weapon constructor writes -1, B2:319487): rifle -> **1**,
  tool -> 6, melee -> 4, bazooka or unset -> 3, pistol -> 0. "In range" = MinRange < d < the weapon's maximum
  (`FUN_002ab590`, B2:231062-231082); the second flag is the middle half of that band.
- [V] BF1 has the same rule in `FUN_00238d10` (B1:152120-152170): UnitType 0, category 1, in range -> pattern 2;
  category 1 out of range -> 0; UnitType 2 with category 4 -> pattern 6; else the draw. It has a gate BF2 lacks
  (it returns unchanged when the target object's +0x7c is set) and a fourth caller with a constant 5 (B1:152695).
  **BF1's draw `FUN_00238b38` (B1:152006-152117) has ONE range flag**: a drawn 2 stands wherever the target is in
  range and becomes 3 only out of range; BF2's drawn 2 becomes 3 outside the MIDDLE of the band (4.4). Section 5's
  "a sniper-kind class with a rifle always stands" reads: while in range.
- [V][L] **Who is a scout** - a disc census by two independent readers (every soldier-rooted class chunk of every
  LVL; `re/unit_census.py`, `wf/K6/`): BF1 59 classes, 178 class-weapon pairs, 7 under the rule (the three Rebel
  marksmen, the two scout troopers, the clone sharpshooter and the Tusken hunter with their sniper rifles); BF2 162
  classes, 28 pairs under it (every `*_inf_*sniper*` class of the four armies with its two sniper rifles). Each of
  the 35 rifles has ZoomMax >= 6, TriggerSingle 1 and YawSpread 0, so the port's `ai_distance_table()` picks all 35.
  Three more classes carry such a rifle with `UnitType trooper`: BF1 `cis_inf_assassindroid` and
  `cis_inf_battledroid_sniper` (rifle `cis_weap_inf_sniperrifle`, `AnimationBank bdroid`) and BF2
  `rep_inf_ep2_jettrooper_sniper`. The assassin droid is the CIS sniper of 19 of BF1's 44 mission scripts (3-5 a
  team; the battle droid sniper of the two Naboo `_h` scripts, the jet trooper sniper of BF2 `kam1c_c`). Live: class
  +0x1914 = 5 for `cis_inf_assassindroid` and 0 for `rep_inf_clone_sharpshooter` in two Clone Wars states
  (`bf1_geo1_walk_cp0_cp6_b000`, `bf1_kam1_cw_cis_spawnmap`; the class by its name hash at +0x14, vtable 0x532b20
  at +0x1c). **So the PS2's assassin droid takes the trooper's draw** (section 5: stand in range for 1 to 3 bots of
  6 by the skill tier, else ring, zig-zag or flank); the port, keyed on the weapon, stands every one of them
  (card FGHTB).

### 16.2 The think a circler's stop is drawn at [L]
- [V] The 1-in-12 draw of 4.5 (`FUN_002da1a8`, B2:263780) is made once per think of the bot's controller, when the
  walk is unfinished and the five checks of 4.6-4.7 (B2:263772-263776) all pass; an arrival sets the pause without
  a draw. The multiplier `DAT_006980a0` = 30 / `DAT_005fe084` (ELF 15) is written by `FUN_002b0318` alone
  [receipt:6c4c853f], a static initialiser reached from the stub at 0x2b0358 (`jal` at 0x2b0364; the stub's address
  is in the constructor table at 0x60e784); RAM 2 in twelve BF2 states. The think period is 0.4 x {4, 3, 2, 1, 0.25}
  s by level (1.2): `FUN_002c4d18` answers 0.4 in every state of UnitCombatAgent's helper, and the helper is updated
  only from the think (`FUN_002b7530` -> `FUN_002b5e60` -> ... -> `FUN_002cf458` -> `FUN_002d8f30`).
- [L] SAIR's two BF2 captures, on-foot attack rows in mode 3 by the level read in the same row
  (`re/ps2_mode3_level.py`): Kamino level 1 32.3 %, level 2 48.8 %, level 3 11.6 %, level 4 7.4 % of 2,461 rows;
  Polis Massa 52.7 % / 47.3 % / 0 / 0 of 846. The think period read in the attack rows of any mode: 1.20 / 0.80 /
  0.40 / 0.10 s (medians, n 1,483 / 2,735 / 382 / 392 on Kamino). The chance alone is therefore 0.069 / 0.104 / 0.208 / 0.83 stops a second
  by level, and 0.07-0.10 for 81 % (Kamino) and 100 % (Polis Massa) of the PS2's circle rows. Card FGHTA's 0.208 a
  second (the port's 1 in 14.4 thinks) is level 3's - a bot near the player and in his view (1.3). On main 296fa0c,
  8 seeds x 32 maps, 1 in 28.8 a think is not told apart from it (an enemy within 20 m +0.19 points, paired t +0.8).
- Tools: `engine/artifacts/lead_tmp/audit-fghta/` - `README.md`, `re/{unit_census,ps2_mode3_level,obs,totals}.py`,
  `tools/{knob_patch,mk_mut}.py`, `wf/RESULTS.md` (the workflow's tables), `wf/K6/` (the second census).

## 17. Card CLSE (2026-10-09): a close fight's damage, and the aim error the PS2 really applies

> **AUDITED 2026-10-12: PARTLY** (section 19). 17.2 stands and is seen on the running PS2; every number of 17.1 and
> 17.4 reproduces. Not measured here: beyond 30 m the landed rule took the port from 1.7-1.8 to 2.2-2.3 times the PS2's
> damage per aimer-second, and the bots now die farther from the enemy (card CLSE4).

Section 15.5 left a gap: per second that a bot aims at a living enemy, the PS2 deals 65-90 hp inside 10 m and the
port 21-33. This section splits both sides the same way, names the cause in the code and knocks it into the port.
Tools and data: `engine/artifacts/lead_tmp/clse/` (`README.md`). PS2: two new BF2 captures (`ps2/bf2_{kam1,pol1}_w`,
`tools/sair_bf2_w.py` = SAIR's sampler plus the weapon in hand of every soldier at every sample). The port: main
177140d, 8 seeds, the first 540 s, a read-only observer (`tools/observer_patch.py`: weapon, trigger, aim offset,
shot and hit rows; its seed-42 rows equal the gate baseline byte for byte).

### 17.1 The split: it is damage per shot, not firing time [L]
An aimer = a living AI on foot whose aim target is a living enemy (15.5's definition); damage = every fall of
health split over the enemies aiming at the victim; "firing" = a shot left in the last 0.25 s. A PS2 shot = the
weapon in hand has state word +0xb8 = 1 with its time in state +0xbc restarted (against the flash end time +0xcc,
which a shot rewrites: 1,635 agree, 525 shot only, 215 flash only over the two captures - counts are good to about
a fifth). `out/table_ps2.txt`, `out/table_port_main.txt`; 5 m bins in `out/split_*.txt`.

| per aimer-second | Kamino PS2 0-10 / 10-20 / 20-30 m | port | Polis Massa PS2 0-10 / 10-20 / 20-30 m | port |
|---|---|---|---|---|
| aimer-seconds in the sample | 97 / 148 / 152 | 1,412 / 2,570 / 3,039 | 152 / 147 / 58 | 5,328 / 8,622 / 7,218 |
| damage, hp | 70.6 / 42.9 / 32.5 | 33.4 / 28.6 / 25.1 | 80.8 / 47.4 / 16.5 | 20.6 / 12.7 / 10.5 |
| firing, % of aimer time | 33 / 29 / 29 | 33 / 30 / 27 | 34 / 38 / 31 | 24 / 13 / 9 |
| damage while firing, hp/s | 200 / 126 / 83 | 83 / 66 / 56 | 203 / 107 / 40 | 65 / 43 / 37 |
| damage while not firing, hp/s | 7 / 9 / 12 | 9 / 12 / 13 | 17 / 10 / 6 | 7 / 8 / 8 |
| rifles (both teams): shots a second in hand | 1.8-1.9 / 1.6-1.8 / 1.8-2.1 | 1.8-2.4 / 1.9-2.2 / 1.9-2.5 | 1.3-2.1 / 1.6-2.1 / 1.4 | 1.4-1.7 / 0.8 / 0.6-0.7 |
| rifles: falls of the target's health per shot | 0.57-0.71 / 0.43-0.51 / 0.21-0.29 | 0.24-0.25 / 0.19-0.22 / 0.14-0.20 | 0.58-0.88 / 0.34-0.44 / 0.09-0.29 | 0.15-0.23 / 0.21-0.22 / 0.20-0.22 |
| weapon in hand inside 10 m: rifle / pistol / shotgun, % | 55 / 15 / 8 | 30 / 34 / 11 | 52 / 22 / 9 | 31 / 30 / 17 |

- **Firing time and rate are the same** (inside 10 m a third of aimer time on Kamino on both; a rifle in hand fires
  1.3-2.4 shots a second on both). The card's first caveat (a port target held without firing) is not the gap.
- **A shot is worth three times as much inside 10 m on the PS2**: rifles pooled over both maps, 0.64 falls of the
  target's health per shot (245 shots) against the port's 0.20 (3,406 shots); the falls themselves are the same
  size (60 / 70 / 80 hp on both). The PS2's share falls with range to the port's flat 0.15-0.25 beyond 20 m.
- **The weapon in hand** is the second term: inside 10 m a PS2 aimer holds a rifle for 52-55 % of the time, the
  port's for 30-31 % (pistols 15-22 % against 30-34 %; a pistol in the port deals 32-41 hp per aimer-second by its
  own hits against a rifle's 40-67). Noted on card GRAIA.
- The PS2's aimers and their targets move (median 3.5-4.6 m/s inside 10 m, both); the port's stand (median 0).
- The port inside 5 m: the aim sits a median 14-16 deg off the target's centre (the PS2's view 9-10 deg off feet
  + 1 m), over 22.5 deg for 18-21 % of aimer time; 0.40 hits per shot by its own hit rows.

### 17.2 The error vector between the tracker and the aim point (BF2) [O]
re-ai-difficulty 4.3 stops at the radius; re-walker-ai 5 left the vector [H] ("k from the distance; not
transcribed"). Read in `FUN_002a50c0` (B2:226418-226595, the aim point of every AI unit) and `FUN_002a4cc8`
(B2:226211-226413); one Opus verifier, 11 claims: 9 supported, 2 partly - both corrections folded in.
- `FUN_002a50c0`: with a target object T (aim order type 0 with a live handle and a weapon, or type 1 with the
  controller's current target) it calls `FUN_002a4cc8(out, ctl, P, T, class)` (B2:226534-226537; class = the
  current weapon's, `*(*(FUN_002f58f8(ctl) + 0x228) + 100)`) and aims at **P + off x k** (B2:226581-226592), k from
  the distance d of P: **k = d / D + 0.5, at most `cap`** (B2:226553-226569), D = class +0x1ec, cap = class +0x1f0
  (20.0 and 10.0 when the class pointer is null). A type 1 order with a miss float > 0 replaces the vector by the unit's own
  ai+0x50 with y cubed and k by d x miss (B2:226570-226580).
- `FUN_002a4cc8`, in order: (1) nothing unless T has a controllable (T's slot +0x74) with a controller object at
  +0xb4 (B2:226252-226257). (2) Direction: the shooter's own unit vector ai+0x50 (B2:226284), or - when class bit
  62 of +0x1e0 is set, or the SHOOTER's entity moves at over 0.95 of its top speed (B2:226258-226283) - its (x, y)
  normalised (`FUN_002ae6e0`) and laid on the two perpendiculars of the line of fire (B2:226286-226333). (3) Times
  **the float at +0x28 of the TARGET's controller object** and class +0x1e8 (B2:226338-226351). (4) y clamped to
  [0, U(2, 3)) - never below the aim point - unless T is a `DAT_00689368` class in state 2 (B2:226352-226361). (5) If
  the shooter's entity carries a human (`FUN_002a7fd8` >= 1: an occupant whose controller's +0xc0 is >= 0,
  B2:228520-228546) the vector is **zeroed**; else if the TARGET's carries none it is **x 0.3** (`0x3e99999a`,
  B2:226365-226379); a target that carries a human keeps it whole (B2:226363-226389; the verifier read the zeroing
  in the ELF at 0x2a4fd0-0x2a4fdc - the export looks as if y survived). (6) x 0.1 when T is a `DAT_00689368` class
  and the shooter's state is 1 (B2:226393-226406).
- `FUN_002a4cc8`'s second caller `FUN_0031f2e8` (B2:312746) adds the vector with no k (not opened further [H]).
- **So a bot's error against another bot is 0.3 of its error against the player**, and it grows with range:
  x 0.75 at 5 m, x 1.5 at 20 m, x 2.5 at 40 m, x 5.5 at 100 m.
- [L] The class fields in RAM (`tools/wclass_err.py`, 28 weapon classes of the Kamino and Polis Massa start
  states): +0x1ec = 20.0 and +0x1f0 = 10.0 on every one; +0x1e8 = 1.0 but the two shotguns' 1.25 and the two
  sniper rifles' 0.6; bit 62 clear on every one (`out/wclass_err_*.txt`).

### 17.3 Whose radius it is: every unit carries its own [L], correcting re-ai-difficulty 4.3
The tracker at the start of a controller object describes the unit ITSELF as a target, and a shooter reads its
target's (17.2 step 3):
- [L] its first word is the unit's own controllable on 12,079 of 14,781 rows of the Polis Massa capture (the rest
  are seated); the vector at +0x10 is the unit's own velocity (within 0.5 m/s on 664 of 963 aimer rows, nearest on
  891; the aimed target's on 72) - so `FUN_002ae010` grows the error by 0.455 x |change of the unit's own velocity|
  / top speed a think, not by a displacement (the verifier: the differenced vector is slot +0x4c's, the one
  `FUN_002ae4d0` divides by top speed) (`out/tracker_vec.txt`).
- [L] radius / err (+0x28 / +0x20) is 0.95 at rest and rises with the unit's OWN speed (1.08-1.16 over 5 m/s) and
  not with its aimed target's (0.95 / 0.95 / 0.78-0.95 at own rest) (`tools/radius_model.py`, `out/radius_model.txt`):
  the (1 + 0.6 x speed / top speed), the 2 x `FUN_002a76b8` size and the state factors are the unit's own.
  The size term class +0x370 is 0.34-0.51 in RAM over eight Kamino states (0.46-0.48 in the Clone Wars ones;
  zero until first asked).
- [L] the human player's controller object holds the same block: err 0.589 on Polis Massa and 0.779 on Kamino (the
  floors of skill 17 and 14: the ENEMY team's, as the "cross" getters say), radius = 0.692 x err = 2 x 0.346
  (`out/human_tracker.txt`). A bot aims at the player with the player's own radius, whole.

### 17.4 Knocked into the port [L]
A switch binary on main 177140d (`observer_and_knockin_177140d.patch`, env `CLSE`; null = the gate baseline byte
for byte): bit 1 = x 0.3 against a unit that is no player, bit 2 = x (d / 20 + 0.5), bit 8 = y never negative.
8 seeds; `out/table_port_v3.txt`, `out/split_port_v*.txt`, `tools/kpm.py`.

| | main | x 0.3 | x 0.3 and k | + y >= 0 | the PS2 |
|---|---|---|---|---|---|
| Kamino, hp per aimer-second 0-5 / 5-10 m | 32 / 34 | 51 / 48 | 56 / 50 | 55 / 52 | 102 / 53 |
| Polis Massa 0-5 / 5-10 m | 22 / 20 | 35 / 29 | 35 / 28 | 31 / 27 | 83 / 79 |
| Kamino 0-10 m (x 0.3 and k) against the PS2 | 33.4 | | **52.4 = 0.74 x** | | 70.6 |
| Polis Massa 0-10 m | 20.6 | | **30.7 = 0.38 x** | | 80.8 |
| rifle hits per shot by its own hit rows, 0-5 m | 0.46-0.50 | 0.65-0.78 | 0.64-0.71 | 0.62-0.78 | |
| kills a minute Kamino / Polis Massa / all 32 maps | 17.4 / 17.9 / 30.6 | 18.0 / 18.9 / - | 19.8 / 19.9 / 32.5 | 18.7 / 17.9 / 32.2 | |
| team kills, % of kills, all maps | 1.9 | | 1.3 | 1.2 | |
| rounds finished, 8 seeds x 32 maps | 256 | | 256 | 256 | |

The vector is half of the gap on Kamino's first 5 m and brings its 0-10 m figure to 0.74 of the PS2's; Polis
Massa stays at 0.38. What is left there is not named: the port's firing share inside 10 m is 24 % against 34 %,
its aimers hold a target they do not see for 11-31 % of the time, and its rifles are in hand half as often.

**Landed 77f4c70** (`core/src/ai/gunner_foot.rs::error_vector`, BF2 rounds only): y >= 0, nothing for a bot that
rides with a human, x 0.3 against a target that carries no human, x (d / 20 + 0.5, at most 10). Its BF2 rows are
the "+ y >= 0" column's byte for byte (136 of 136 rounds) and its BF1 rows main's (120 of 120): Kamino inside
10 m 53.1 hp per aimer-second (0.75 of the PS2's), Polis Massa 28.9 (0.36), kills a minute over all maps 31.7,
team kills 1.5 % of kills. Left for cards CLSE2 (the block of 17.3, the size term, the class factor, BF1) and
CLSE3 (Polis Massa's rest).

### 17.5 Checks, and what stayed open
- `re_check.py fn` on the 22 cited lines (all in the functions named); `re_check.py callers`: `FUN_002a4cc8` 2,
  `FUN_002ae6e0` 1, `FUN_002ae4d0` 1. The verifier's two corrections: the human-carrying shooter's vector is
  zeroed whole (I had "y kept"); the growth term differences a velocity (I had "position") - then seen in RAM.
- **BF1's twin is NOT read** [H]: by three constant patterns B1 has one hit of the 0.3 word (`FUN_001dd4c8`, another
  thing), one of U(2, 3) (`FUN_0023c698`, the sniper's cooldown) and the 0.95 squared test 0 times
  [receipt:8d2b685e] - a pattern miss, not an absence; a call-graph search from BF1's aim tracker `FUN_00236700` is
  owed before the rule goes to a BF1 map. **Read since (18.5, card CLSE2): BF1's aim point has no error vector.**
- [H] the port's hits beyond 20 m equal the PS2's with an error that is larger at every range under 60 m: the
  weapon's own spread (re-ai-difficulty 4.1's x 1.0-3.0 and the distance table) was not compared.
- [H] the state factors of `FUN_002ae4d0` (7: x 0.4, 2: x 2) were not matched to a stance; the stance word E+0x7e4
  = 1 reads 1.43 x on Kamino and 1.04 x on Polis Massa at rest. **Answered (18.2): the two factors never apply to
  a soldier; the stance comes in by another call, crouch x 1.5 (0.95 x 1.5 = 1.43), sprint / jump x 2, roll x 3.**

## 18. Card CLSE2 (2026-10-09): the aim error's block end to end (BF2), its three class keys, and BF1's aim point, which has no error

Section 17 scaled the error VECTOR. This section reads the BLOCK under it (`FUN_002ae010`, `FUN_002ae7b8`,
`FUN_002ae4d0`, `FUN_002a76b8`), names the weapon class's keys, and reads BF1's aim point from `FUN_00236700`'s
side of the call graph. Three knock-in captures on the running PS2 settle whose radius a shooter takes.
Tools and data: `engine/artifacts/lead_tmp/clse2/` (`tools/`, `out/`, `ps2/`, `verify/claims.md`,
`verify/verdicts.json`). Verified by a workflow of five agents, claims and citations only (18.8).

### 18.1 The block a unit carries, end to end (BF2) [O]
Every controller object starts with the block (`t` below; a bot's `ai`, the human's PlayerController too). It
describes the unit ITSELF as a target (17.3): the entity its code asks for is the controller's own - for a soldier
the slots +0x24 and +0x28 of the vtable at C+0x34 (0x61e948; ELF words 0x61e96c / 0x61e970) both return
(C + 0x18) - 0x288 = E.
- **Who runs it.** `FUN_002ae010(dt, t)` has two callers. A bot: `FUN_002b7790` (B2:239268-239311) counts a byte at
  ai+0x2d0 down every frame and sums dt into ai+0x2d4; when the byte goes negative it calls
  `FUN_002ae010(the sum, ai)` and reloads the byte with 4 when the level of detail ai+0x560 is 0 or 1, with 2 when it
  is 2, with 0 otherwise - every 5th update (0.167 s at 30 a second), every 3rd, every one. The human: `FUN_002b0548`
  (B2:234692), slot +0x1c of the PlayerController's vtable 0x6219f8 (ELF word 0x621a14, the class label
  "PlayerController" behind the table; [L] the human's controller holds that vtable word at +0x90 on 179 of 179 rows
  of the first 200 raw samples, the bots' 0x621f58 / 0x6220b8) - every call, with its own dt (`lwc1 $f12, 1424(sp)`
  at 0x2b05c0, the local `FUN_003d7a20` is handed first).
- **Decay** (B2:233039-233049): `d = dt x rate`, rate = `FUN_002a2dc0(flag)` = blend(-0.1, -1.1) by (skill - 1) / 19;
  `FUN_002ae7b8(d, t)`, or `d x 0.5` for an EntityFlyer. The flag is `FUN_002a8300(the unit's team) ^ 1`, and in
  `FUN_002a29d8` the flag 1 reads the table {6, 11, 11} and 0 reads {4, 14, 17} (B2:224697-224705): a unit of the
  human's team is given the ENEMY AI's skill and an enemy the skill of the human's AI team mates - the skill of who
  shoots at it. Not capped at 20 here (the clamp argument is 0). [L] the auto-balance moves it in a round: the
  floors on Polis Massa step 14 -> 15 -> 16 -> 17 on one team and 11 -> 10 -> 9 -> 8 on the other in 240 s.
- **A budget** (B2:233051-233056): `t+0x24 = clamp(t+0x24 - d x 0.5, 0, 5)` - it RISES by dt x |rate| / 2 a think
  (0.13 / 0.31 / 0.39 / 0.47 a second at skill 4 / 11 / 14 / 17), from 0 at construction (B2:232982).
- **Growth** (B2:233057-233081; ELF 0x2ae18c-0x2ae1e8): `v` = the vector of the entity's slot +0x4c - for a soldier
  E+0x550, its velocity (ELF word 0x61ebd4 = 0x256428: `addiu v0, a0, 1360`); `top` = slot +0x68 = class +0x74c,
  MaxSpeed (0x61ebf0 = 0x24ea70). `delta = (t+0x10) - v` over x, y and z; `t+0x10 = v`; then
  `FUN_002ae7b8(|delta| x 0.455 / max(top, 0.1), t)`. So the error grows with the unit's CHANGE of velocity between
  two thinks - a start or a stop 0.455, a reversal at full speed 0.91 - and a unit running straight adds nothing.
- **`FUN_002ae7b8(x, t)`** (B2:233284-233322): a positive x is first cut to the budget and taken off it; `err` (t+0x20)
  += x; then err = the floor when under it, else min(err, 2.0). Floor = max(0, blend(1.6, 0.4)) by the same flag
  (`FUN_002a2df8`), 1.5 with no controllable. Growth is paid from the budget: over time the error can rise by at
  most half of what it decays, plus what was saved up (at most 5).
- **The direction** (B2:233083-233150; ELF 0x2ae1f0-0x2ae49c) is the SHOOTER's own (17.2 step 2 reads t+0x50): three
  LCG floats in [-0.5, 0.5) x 2 are added to the unit vector at t+0x30, which is normalised again; the quaternion at
  t+0x40 is multiplied by (that axis x sin dt, cos dt) (`FUN_0053a070` with (dt + dt) x 0.5; its table 0x739860 holds
  (sin, cos) pairs [L]) - a turn of 2 x dt radians about the wandering axis - and t+0x50 = that quaternion applied
  to (0, 1, 0). The offset direction TUMBLES at up to 2 rad/s; it is not drawn anew each think.
- Last, `t+0x28 = FUN_002ae4d0(t)` (B2:233151-233152).

### 18.2 The radius (`FUN_002ae4d0`, B2:233158-233236) [O]
`radius = s x (3 - 2 x vis) x min(err x m, cap)`:
- `m` = the float of slot +0x34 of the controllable's vtable (C+0x14), times `1 + 0.6 x |v| / max(top, 0.1)`
  (B2:233189, 233206-233220). **For a soldier that slot is `FUN_00255898` -> `FUN_0024ea00(E)` (B2:167951-167968),
  a factor by the stance word E+0x7e4: crouch (1) 1.5; prone, sprint and jump (2, 3, 4) 2.0; roll (5) 3.0; else
  1.0.** It is inside the cap. Not in re-ai-difficulty 4.3.
- `cap` = max(0, blend(2.2, 1.2)) (an EntityFlyer blend(4.0, 1.2)) by the flag of 18.1; 1.5 with no controllable.
- `s` = 2 x `FUN_002a76b8(E)` (18.3), x 0.4 when the entity's slot +0xe0 returns 7, x 2 when it returns 2
  (B2:233221-233232). **A soldier's slot +0xe0 (ELF word 0x61ec68 = 0x256490) is `jr ra; addiu v0, zero, 1`: neither
  factor ever applies to a soldier** (17.5's [H] is closed: the "state" is another class's, not a stance).
  s = 1.0 with no entity. `vis` = t+0x7c (re-ai-difficulty 5.3).
- [L] **Replayed over RAM** (`tools/bf2_radius_replay.py`, `out/radius_replay_*.txt`): radius / (min(err x m, cap) x
  (3 - 2 vis)) is one number per class on 92.6 % of 14,243 rows (Kamino) and 85.4 % of 12,555 (Polis Massa, the cap
  following the floor as the skill moves) to within 2 %; on the crouched rows its median is 1.000 with the stance
  factor and 1.500 without (2,095 and 761 rows; 1,963 more in a third capture). The 2.0 and 3.0 arms are seen only
  under the cap (the ELF's jump table 0x61e780 is their evidence).

### 18.3 The size term (`FUN_002a76b8`, B2:228108-228163) [O]
- Class +0x370 when positive. Else, for the unit kind of `FUN_002a6138` (0 soldier or none of its classes, 1
  `DAT_00689298`, 2 walker, 3 droideka, 5 flyer, 6 hover, 7 MountedTurret `DAT_0068c8f8` or `DAT_0068c950`, 8
  EntityBuildingArmed, 9 `DAT_00688e98`, 10 `DAT_00689510`, 11 `DAT_006897c0`): a hover with a positive class
  +0xbe4 returns 0.5 x sqrt(+0xbe4^2 + +0xbe8^2); everything else takes the box at +0x80 / +0x90 of
  `*(slot +0x40 object + 0x1c)` and `len = sqrt(dx^2 + dz^2)` - the HORIZONTAL diagonal (`VMUL.xz` 0x2a778c,
  `VADDz.x` 0x2a7790; y is not used) - and returns 0.5 x len for a hover, or for a walker on a team whose record
  +0x60 is 4; else **0.25 x len, stored in class +0x370** (B2:228152-228160).
- A soldier's slot +0x40 returns `*(E + 0x160)`, its model (ELF word 0x61ebc8). [L] The box is the model's bind
  pose box and equals the baked GLB's POSITION bounds: `rep_inf_ep3trooper` (-0.929 -0.001 -0.233) to
  (0.929 2.083 0.204) in RAM and in `side_bf2/soldiers/rep_inf_ep3trooper.glb`; 0.25 x hypot(dx, dz) = class +0x370
  to four decimals on all 10 classes of a Kamino state (`tools/size_box.py`, `tools/glb_box.py`).
- So s = 2 x size follows **the model's arm span in its bind pose**, over the 31 baked BF2 soldier models
  (`out/glb_box_bf2.txt`): 0.93-1.02 on the 22 whose arms are out (clones, stormtroopers and the bothan 0.95-0.96,
  the battle droid 0.93, the wookiee 1.02), 0.63-0.70 on eight whose arms hang (the rebel soldier 0.692, vanguard,
  marksman, Tantive trooper, the imperial gunner and the two AT-AT crew) and 0.48 on the super battle droid. The
  per-class numbers of 18.2's replay are these, class by class.

### 18.4 The weapon class's three keys [O]
- The weapon class's setter `FUN_00328fb0` (slot +0x20 of its vtable 0x627d50): **`AIBubbleSizeMultiplier`**
  (fnv 0xe60cd1c3) -> class +0x1e8 (B2:320831-320835), **`AIBubbleScaleDistDivider`** (0x70bb9aa6) -> +0x1ec
  (B2:320211-320215), **`AIBubbleScaleClamp`** (0x41d6cec0) -> +0x1f0 (B2:320022-320026). Defaults 1.0 / 20.0 / 10.0
  (the constructor `FUN_00327e98`, B2:319552-319557); a class starts as its parent's copy (B2:319739). So 17.2 reads:
  vector x `AIBubbleSizeMultiplier`, k = d / `AIBubbleScaleDistDivider` + 0.5, at most `AIBubbleScaleClamp`.
- [L] The BF2 disc (`tools/bubble_scan.py`, `out/bubble_bf2.txt`; 69 / 25 / 25 PROP chunks, a second raw scan
  agrees): the multiplier is 1.25 on `com_weap_inf_shotgun`, 0.6 on `com_weap_inf_sniper_rifle`,
  `com_weap_award_sniper_rifle`, the two `*_weap_bldg_boxturret` and `all_weap_moncal_roundturret`, 0.85 on
  `com_weap_inf_bulldog`, 0.4 on `tur_weap_laser_auto`, 0.1 on `com_weap_inf_defenseturret`, 1.0 written out on
  `com_weap_award_shotgun`; the divider is 100 on the three building turrets and 50 on the defense turret, and
  those four clamp at 3.0. Every other class keeps 1.0 / 20 / 10 (the children by their parent's copy).
- **BF1 reads none of the three**: no hash in its decomp, none in its ELF as a data word or a `lui` + `ori` /
  `addiu` pair (the BF2 control finds each once; BF1's own keys such as `ShotDelay` are found the same way), and
  its disc authors none (0 chunks) [receipt:257b62c4].

### 18.5 BF1: the aim point has no error [O]
- The chain: the soldier's AI input `FUN_0020aad0(dt, S)` copies the aim order (S+0x4e0 type, +0x10 point, +0x20
  target handle) and calls the shared steering `FUN_00233500` (B1:119893-119912), which calls **`FUN_00233200`
  (B1:148003-148112), BF1's twin of `FUN_002a50c0`**: the same three branches on the order's type, the same 30 m
  fallback, the same last call (the stick toward a point, `FUN_00232b78` / `FUN_002a45a0`).
- Type 0 with a target and a weapon: `FUN_00232a70` (B1:147665-147707): the target's point by its slot +0x5c, plus
  (its velocity - the shooter's) x dt, then the weapon's lead `FUN_0026d3f0` (a linear or a ballistic intercept
  from weapon+0x80; B1:191618 on). **Nothing else**: 66 instructions with no float constant, no LCG draw, no
  radius, no factor by range [receipt:257b62c4]. Where BF2 has the class read, `FUN_002a4cc8`, d / D + 0.5 and
  `point += off x k`, BF1 has nothing that writes the point.
- A soldier's slot +0x5c (ELF word 0x532dac = 0x20c680): its matrix x the entry [posture S+0xbe0] of the table at
  0x586350 - **feet + 1.4 m standing, 0.6 crouched, 0.2 prone** (written by the static initialiser B1:124339-124361;
  [L] the same in RAM).
- BF1 has no twin of the block either: 0.455 (0x3ee8f5c3) is nowhere in its decomp or ELF, nor 2.2 (the cap); the
  one -1.1 is a best-dot search's start value (`FUN_00258b50`) [receipt:257b62c4]. Its skill has four curves (aim
  acceleration, damping, the two weapon multipliers; re-ai-difficulty 2.2) and no floor, cap or decay.
- [L] **On the running PS2** (`tools/bf1_aim_off.py`, three captures): with shooter and target both under 0.3 m/s
  the view ray of a BF1 bot passes its target's axis at a median 0.06-0.09 m at EVERY range (10-20 m 0.06-0.08,
  20-40 m 0.064-0.066, over 40 m 0.06-0.10; n = 1,012-1,580 a capture) and its height at the posture point within
  a median 0.005-0.04 m. The same instrument on BF2 reads 0.20 / 0.20-0.23 / 0.32 m. A bot with a fire target
  (the handle K+0x188 the fire controller `FUN_0023cd40` resolves) holds a type 0 order on 83-88 % of rows, at that
  same soldier on 96-97 % of them; its type 1 orders point a median 47-53 m away (`tools/bf1_fire_target.py`).
- So a BF1 bot's misses are its aim tracker's lag and its weapon's spread (x the two skill multipliers) and
  nothing else. The port puts BF2's whole radius on a BF1 bot's aim point (17.4: "a BF1 round is unchanged").
- The port's form of the (velocity - shooter's) x dt step (playtest C5, `ai::gunner_foot::swept_lead`): a bot on
  foot thinks before anyone moves, and its bolt sweeps whole ticks against targets that have already moved, so the
  aim point is the target one tick on, less the shooter's tick, plus the flight in whole ticks (a droid, which moves
  after the bolts, takes no first tick). Ground rounds and foot gunners only, a port scope choice: seats and space
  battles keep the plain intercept (on the PS2 only the fire and range checks pass 0). A bot sniper on a 6 m/s runner at 40 m: 0.08 m behind before, under
  0.01 m after (aim error off).

### 18.6 Knocked in on the running PS2: the vector takes the TARGET's radius, and nothing else moves the aim [L]
`tools/sair_bf2_poke.py` = section 17's sampler + a write of class +0x370 (18.3) for every soldier class of one
team; Kamino, 240 s each, the player's team (team 1) poked; `tools/bf2_radius_replay.py` parts B to E. The view
ray's horizontal miss of the target's axis, shooter and target standing still:

| capture | team 1's radius | shooters of team 2 (their targets are team 1): 0-20 / 20-40 / over 40 m | shooters of team 1 (targets team 2, radius 0.92) |
|---|---|---|---|
| untouched (17's) | 0.54-0.76 | 0.177 / 0.195 / 0.303 m (n 152) | 0.198 / 0.296 / 0.406 m (n 50) |
| size 0.000001 | 0.0000 | **0.060 / 0.060 / 0.060 m** (n 411; 66 % under 0.1 m) | 0.286 / 0.352 / 0.463 m (n 148) |
| size 1.0 | 1.56 | 0.290 / 0.659 / 1.877 m (n 193) | 0.261 / 0.258 / 0.407 m (n 139) |

- The shooters whose TARGETS lost their radius lost their error; the shooters whose OWN radius was zeroed kept
  theirs. The reading of 17.2 step 3 holds: every shooter of one unit takes that unit's own radius.
- With the radius gone the miss is BF1's flat 0.06 m: the vector is the only thing between a BF2 bot's aim and its
  target, and 0.06 m is the instrument's own constant (eye against barrel [H]), not an error.
- **The 0.3 of 17.2 step 5, measured**: miss / (the target's radius x k) has a median 0.533 at the human (n 73,
  size 1.0: his radius 1.56, a miss of 3.3 m at 20 m and more) and 0.174 at bots (n 259): **0.33 against the
  code's 0.30**. The bot figure is 0.17-0.19 in all four BF2 captures.

### 18.7 The PS2, the port, what card CLSE2 builds
| | the PS2 (BF2) | the port (main a4df1b5, `ai/gunner_foot.rs`) |
|---|---|---|
| whose error | the TARGET's block: one err, one radius per unit, the human's too | one err per SHOOTER (`AiState::err`) |
| growth | 0.455 x the unit's change of VELOCITY (3-D) / MaxSpeed a think, paid from a budget that fills at half the decay (at most 5) | 0.455 x the target's DISPLACEMENT a think / MaxSpeed, no budget: a straight runner adds 0.455 x speed / top a second |
| cadence | every 5th frame at LOD 0 / 1 (6 Hz), 3rd at LOD 2, each frame beyond; the human each frame | every think (10 ticks, 3 Hz) |
| inside the cap | err x stance (1.5 crouch, 2 sprint / jump, 3 roll) x (1 + 0.6 v / top) | err x (1 + 0.6 v / top) |
| size | x 2 x 0.25 x the model box's horizontal diagonal: 0.48 / 0.63-0.70 / 0.93-1.02 by model | x 1.0 |
| weapon class | x `AIBubbleSizeMultiplier` (1.25 shotgun, 0.6 sniper rifle, 0.85 bulldog, 0.6 / 0.4 / 0.1 turrets); k's D and cap per class (100 or 50, 3.0 on the turrets) | 1.0, 20, 10 |
| direction | the shooter's vector tumbling at up to 2 rad/s | a new uniform direction every think |
| BF1 | NO error: the lead point of feet + 1.4 / 0.6 / 0.2 m | the whole BF2 radius |

### 18.8 Checks, the verifier, what stayed open
- `re_check.py fn` on every cited line (33 B2, 8 B1: all in the functions named); `callers`: `FUN_002ae010` 2,
  `FUN_002ae4d0` 1, `FUN_002ae7b8` 2, `FUN_00233200` 3, `FUN_00233500` 6; `word` on 14 vtable slots and the constants;
  `absent` + a `lui` / `ori` scan with a BF2 control for the four constants [receipt:257b62c4].
- **Workflow `clse2-re-verify`, 5 agents** (four batches over 33 claims; a second, separate verifier for the six
  absence / never / twin claims): **32 SUPPORTED, 1 PARTLY, 0 NOT SUPPORTED; the second verifier supported all six
  and overturned none.** PARTLY: my list of `FUN_002a6138`'s kinds stopped at 6 (it goes on to 7-11; folded into
  18.3). The verifiers re-ran four scripts byte for byte, found the weapon class's vtable for themselves, and
  added: s = 1.0 with no entity; the skill is not capped at 20 in these getters; the 2.0 / 3.0 stance arms are not
  seen in RAM under the cap; the box object is per model, not per class; BF1's `FUN_0023bc48` -> `FUN_0023b8a0`
  does draw LCG floats, for a 30-point list at K+0x78, not for the aim.
- Made after the verifier ran and not put to it: the three knock-in captures (18.6), the BF1 fire-target count
  (18.5), the PlayerController's vtable and its dt (18.1), the (sin, cos) order of the table (18.1) - [L] / [D].
- [H] not opened: the size branches of vehicles (a hover's +0xbe4 / +0xbe8, a walker's 0.5) and the flyer's states
  7 and 2; the slot +0x5c of BF1 targets that are no soldier; `FUN_0031f2e8`, the vector's second caller (17.2);
  which BF2 getters the auto-balance moves beside the floor. The 0.06 m constant of the instrument is not explained.
- Not measured: BF1's hits per shot on the PS2 (section 17's weapon sampler is BF2's); 18.9 has its damage rate.

### 18.9 BF1's damage per aimer-second, the PS2 against the port [L]
The referee for the BF1 rule, by 17.1's definitions (an aimer = a living AI on foot whose aim order is type 0 at a
living enemy soldier; damage = every fall of health split over the aimers at the victim). PS2: SAIR's three BF1
captures (`tools/bf1_split.py`, `out/bf1_split_ps2.txt`; the aim order is in the raw packets only, so an aimer row is
0.5 s; 32-52 % of the health that fell had no aimer at the victim and is left out). The port: main 177140d's
observer binary (`lead_tmp/clse/bt/k1`), 8 seeds, the first 540 s, `clse/tools/split_port.py` with `BINS3=1`
(`lead_tmp/clse/sw/c2_bf1_base`, `c2_bf1_v3`).

| hp per aimer-second, 0-10 / 10-20 / 20-30 / over 30 m | Kashyyyk Docks | Naboo Plains |
|---|---|---|
| the PS2 (aimer-seconds: 36 / 156 / 535 / 2,395 and 49 / 151 / 218 / 552) | 16-35 / 31-33 / 20 / 12-13 | 28.6 / 24.7 / 17.5 / 17.6 |
| the PS2, the three captures pooled | 27.1 / 28.4 / 19.2 / 13.4 | |
| the port, main (the whole radius) | 20.9 / 17.0 / 18.1 / 16.1 | 14.3 / 16.4 / 17.1 / 14.3 |
| the port, the vector x 0.3 x (d / 20 + 0.5) (`CLSE=3`: a stand-in, not BF1's rule) | 27.9 / 21.0 / 21.5 / 17.5 | 18.8 / 17.8 / 19.6 / 14.9 |

- Inside 20 m the port deals 0.5-0.8 of the PS2's damage per aimer-second on both maps (the PS2's 0-10 m sample is
  thin: 85 aimer-seconds in all); beyond 30 m the two are level. A smaller vector moves the port toward the PS2 and
  not past it: the port's hits per shot go 0.34 / 0.26 -> 0.50 / 0.33 on Kashyyyk and 0.29 / 0.21 -> 0.38 / 0.23
  on Naboo inside 10 / 20 m.
- The PS2's BF1 rate inside 10 m is under 0.4 of its BF2 rate (27 against 71-81, 17.1) although BF1 aims exactly:
  a rate is not the aim error alone ([H] BF1's team mates fire at skill 4 on Normal, re-ai-difficulty 2.1, and its
  aimers mostly stand: median speed 0.1 m/s inside 10 m).

### 18.10 Built (2026-10-12, card CLSE2, main 8ba75a7 + d57ab7d): BF2's five rules are in the port, BF1's is held back
- In the port for a round under BF2's rules: the block per unit (`core::unit::AimBlock`, thought by `Sim::step` - a
  bot's every 5th tick, the human's every tick), the stance factor and the size term (`ai::gunner_foot::radius`,
  `stance_factor`, `game::roster::AIM_SIZE`), the weapon class's three keys (`WeaponClass::ai_bubble_*`), the
  tumbling direction. 18.7's port column is main BEFORE this. Port's own choices, each named in the code: one base
  skill for decay and floor (the PS2: the skill of the team that shoots, moved by auto-balance); no level of
  detail (every bot at 5 ticks); the quaternion product's order ([D]: `quat x turn`) and its normalisation; `Fall`
  takes 1.0.
- [L] The port, 8 seeds x 32 maps (`lead_tmp/clse2/sw/`, each rule behind a switch, then stacked): no rule moves a
  BF2 table past the seed noise, as 18's close expected - hp per aimer-second at 0-10 / 10-20 / 20-30 / over 30 m
  on Kamino 49.7 / 41.6 / 33.3 / 22.1 -> 52.7 / 40.8 / 35.7 / 24.5 (the PS2 70.6 / 42.9 / 32.5 / 10.1), on Polis
  Massa 29.5 / 15.8 / 15.1 / 21.5 -> 29.0 / 16.5 / 15.3 / 20.4 (the PS2 80.8 / 47.4 / 16.5 / 7.0). The gap to the
  PS2 inside 10 m and beyond 30 m is not the error block's: card CLSE4.
- **BF1's rule (18.5) is NOT in the port.** Built and measured: Kashyyyk Docks 20.6 / 17.5 / 18.4 / 15.9 -> 32.7 /
  22.6 / 23.2 / 19.9 and Naboo Plains 14.1 / 17.4 / 17.8 / 13.8 -> 16.2 / 22.6 / 22.5 / 17.8 hp per aimer-second
  (the PS2: 18.9's table) - toward the PS2 inside 20 m and past it beyond; and the port's BF1 bots then meet less
  (an enemy within 20 m: 10.9 -> 8.1 % of unit time over the 15 BF1 maps, every map down). The port's BF1 far fire
  is over the PS2's already, and exact aim adds to it. A BF1 round keeps the shooter's own tracked error (the model
  before section 17, PROVISIONAL and known wrong) until the far fire is measured and fixed: card CLSE2B.

## 19. Audit CLSE (2026-10-12): 17.2 re-derived and seen on the running PS2; what the rule does beyond 30 m [O]

AUDIT of card CLSE (77f4c70): PARTLY. 17.2 was re-read from the B2 export and the ELF words before section 17 was
opened, then given as claims and citations only to two verifiers (13 SUPPORTED, 6 PARTLY on wording, 0 NOT; the four
absence / twin claims went to both, none overturned). Evidence `engine/artifacts/lead_tmp/audit-clse/` (`README.md`,
`wf/RESULTS.md`, `ps2/`). Checks run: `re_check.py fn` on B2:126201, 226273, 226352, 226354, 226375, 226394, 226399,
226561, 226566 (`FUN_002125c0`, `FUN_002a4cc8`, `FUN_002a50c0`), 319487 (`FUN_00327e98`), 320025, 320214, 320834
(`FUN_00328fb0`); `re_check.py callers` `FUN_002a4cc8` 2, `FUN_002a50c0` 4, B1 `FUN_00233200` 3, `FUN_00232a70` 3.

### 19.1 What holds, and what the audit adds [V]
- 17.2 steps (1)-(6) and the range factor stand as written. ELF words: 0x2a4fd0 / 0x2a4fd4 / 0x2a4fdc are
  `sw zero, 8 / 4 / 0(sp)` past `blez v0` at 0x2a4fc8 (the shooter's human count is 1 or more: the vector zeroed
  whole); `0x3e99999a` is built at 0x2a4ffc / 0x2a5000 past `bnel v0, zero` at 0x2a4ff0 (the target's count is 0).
- `FUN_002a7fd8` walks the entity's own list (`FUN_002690c8` = entity + 0x240) and counts the controllables with a
  controller object (+0xb4) and a word +0xc0 of 0 or more. [L] C+0xc0 is 0 on the human's controllable and -1 on
  each of the 39 bots' in all 480 raw samples of CLSE's Kamino capture.
- The three weapon-class fields have THREE writers, not two: the base constructor `FUN_00327e98` (B2:319552-319557:
  20.0, 10.0, 1.0), the setter `FUN_00328fb0` (18.4) and the COPY constructor `FUN_003283d8` (B2:319739-319741,
  15 callers: one per derived class) - 18.4's "children inherit by BASE" is that copy.
- The disc: 18.4's census stands - 10 classes author `AIBubbleSizeMultiplier`, 4 `AIBubbleScaleDistDivider` and
  `AIBubbleScaleClamp` (the three turrets of the SPA levels 100 / 3.0, `com_weap_inf_defenseturret` 50 / 3.0);
  re-read by three more scanners (the auditor's, each verifier's). The three hashes are absent from every PROP chunk
  of the BF1 disc and from B1 [receipt:d92490df]; BF1's aim point takes no vector (18.5; both verifiers).
- The port's `error_vector` has none of: the class factor, the U(2, 3) upper clamp, the full-speed construction, the
  x 0.1 at a flyer, the flyer's exemption from the clamp. Its doc comment lists the first three; the last two are
  in 17.2 only.

### 19.2 The rule on the running PS2 [L]
`ps2/live_rule.py` on CLSE's two captures (the raw packets, 2 samples a second): for an AI on foot S whose aim order is
type 0 at a soldier T, both slower than 0.5 m/s, the signed sideways offset in the horizontal plane between S's view
ray (C+0xd0 along C+0xe0) and T's axis, against X = (V_S . p) x R_T x F - V_S = S's own ai+0x50, p = the horizontal
normal of the view ray, R_T = T's ai+0x28, F = the class +0x1e8 of the weapon S holds.

| bot targets, range | pair samples | mean k = d / 20 + 0.5 | slope of the offset on X | slope / k | correlation |
|---|---|---|---|---|---|
| 0-10 m | 24 | 0.77 | 0.256 | **0.33** | 0.84 |
| 10-20 m | 64 | 1.28 | 0.398 | **0.31** | 0.91 |
| 20-30 m | 79 | 1.68 | 0.509 | **0.30** | 0.43 |
| 30-45 m | 48 | 2.35 | 0.587 | 0.25 | 0.37 |
| beyond 45 m | 36 | 3.10 | 0.595 | 0.19 | 0.41 |

The slope grows with range as k does and is 0.3 of it inside 30 m: the 0.3 and the range factor, seen. Against the
human the slope on k x X is 0.85 (8 samples, all inside 10 m: the pinned player is seldom aimed at) against the bots'
0.25 - 3.3 times, the 1 / 0.3 of step (5). |V| is 1.000 on all 2,244 pair samples. Class fields seen in hand:
(1.0, 20, 10) 2,043 times, the shotgun's 1.25 188, the sniper rifle's 0.6 13.

### 19.3 What the landed rule does beyond 30 m, and where a bot dies [L]
Card CLSE's tables stop at 30 m (17.1, 17.4). With its own instrument (`clse/tools/split_*.py`, `BINS3=1`; the PS2's
intervals are a block bootstrap of the one capture a map):

| hp per aimer-second | 0-10 m | 10-20 m | 20-30 m | beyond 30 m |
|---|---|---|---|---|
| Kamino, the PS2 | 70.6 (57.5-83.3) | 42.9 | 32.5 | 10.1 (6.8-13.7) |
| port: parent -> landed (main 188bc17) | 33.4 -> 53.1 (57.7) | 28.6 -> 40.2 (42.1) | 25.1 -> 33.4 (37.9) | 17.6 -> **21.7** (23.2) |
| Polis Massa, the PS2 | 80.8 (68.6-92.2) | 47.4 | 16.5 | 7.0 (2.0-15.5) |
| port | 20.6 -> 28.9 (31.5) | 12.7 -> 17.7 (18.1) | 10.5 -> 14.3 (16.3) | 12.6 -> **16.3** (22.3) |
| share of Kamino's aimer time, the PS2 / parent / landed, % | 15 / 9 / 5 | 23 / 17 / 12 | 24 / 20 / 21 | 38 / 54 / 63 |

- Inside 30 m every bin moved toward the PS2; beyond it the port went from 1.74 and 1.79 times the PS2's to 2.15 and
  2.32 (main 2.30 and 3.17). 17.5's [H] ("beyond 20 m equal") held only as a pool of a low 20-30 m and a high bin
  beyond. A rifle shot beyond 30 m on Kamino: the PS2 9.0 hp, parent 10.3, landed 13.3; fired 1.5-1.6 times a second
  of rifle aimer time on the PS2, 2.26 in the port. 53 % of the PS2's aimer time beyond 30 m is outside a fight order.
- Total damage credited over the 17 BF2 maps, parent -> landed: inside 10 m -11 % (the aimer time there -38 %),
  beyond 30 m +13 %. A rifle's falls of health per shot inside 10 m: 0.20 -> 0.29 (the PS2 0.64).
- **Where a bot dies** (15.5's rows; 8 seeds, the first 540 s, per-seed paired t; the PS2 from three captures a map,
  `ps2/ps2_deaths.txt`):

| 17 BF2 maps | parent | landed | t | Kamino | the PS2's Kamino | Polis Massa | the PS2's |
|---|---|---|---|---|---|---|---|
| an enemy inside 10 m at death, % | 30.3 | 25.0 | -13.5 | 27.2 -> 18.3 | 42-70 | 59.0 -> 46.4 | 68-73 |
| no enemy within 20 m at death, % | 42.0 | 47.6 | +8.3 | 49.6 -> 59.1 | 12-26 | 11.9 -> 18.8 | 15-17 |
| the nearest enemy at death, median m | 16.5 | 19.0 | +7.6 | 19 -> 23 | 7-12 | 7.6 -> 10.1 | 6-7 |
| lives that get within 20 m of an enemy, % | 76.7 | 71.4 | -8.4 | 64.0 -> 55.8 | | 94.2 -> 92.8 | |
| an enemy within 20 m, % of unit-seconds | 12.63 | 10.27 | -7.4 | 4.1 -> 2.5 | 12-20 | 22.8 -> 21.1 | 23-24 |

  The same on main with the rule switched off and on (29.5 -> 25.6 % inside 10 m, t -5.3; contact 12.14 -> 9.66 % over
  24 more seeds, t -18). The fall of contact is four fifths shorter contact spells (4.64 -> 3.88 s) and one fifth
  fewer bots getting close. All of it is the 0.3 (with the 0.3 alone off: contact 12.24 %, kills a minute 28.5
  against the rule-off build's 30.3); the range factor and the clamp move nothing resolved between bots.
- The PS2's bloom inside a bot's burst (card SPRD's tight hold for bots is OURS) is a tenth to a fifth of the far
  fire (23.2 -> 20.8 and 22.3 -> 17.6 hp per aimer-second beyond 30 m) and moves neither row above. The cause is
  card CLSE4's.
- Against a human (an idle human on Kamino, 8 seeds): hits per bolt on him 0.563 -> 0.669 inside 10 m (t +5.8),
  0.524 -> 0.467 at 10-20 m (t -7.2), 0.449 -> 0.395 at 20-30 m (not resolved) - the crossing at 10 m that
  d / 20 + 0.5 puts there.

## 20. Card CLSE4 (2026-10-12): the far fire split by weapon - the trigger's range test and the Barrage keys [O]

OPUS RE (2026-10-12) - [O] UNAUDITED. Hops: the split per weapon on both sides (three PS2 captures a map, 8 port
seeds on main 7d206bc) -> two PS2 rules the port lacks, read in both games -> both knocked into the port behind
switches. Checks run: `re_check.py fn` on every cited line, `callers` for `FUN_002a9c30` / `FUN_002acd68` /
`FUN_002ad1a0` / `FUN_002ad228` / `FUN_002f5a10`, `word b2 0x628344 2 --u32`, the census receipt below; one opus
verifier on 21 claims (`lead_tmp/clse4/verify/claims.md`): 16 SUPPORTED, 5 PARTLY (each folded in below), 0 NOT.
Evidence, tools and every table: `engine/artifacts/lead_tmp/clse4/` (README.md).

### 20.1 The split [L]
Captures: CLSE's `bf2_{kam1,pol1}_w` and two more a map (`clse/ps2/bf2_*_c4a`, `_c4b`; `clse/tools/run_w.sh`).
Port: main 7d206bc with CLSE's read-only observer (seed 1 x 17 maps = the pristine binary's rows byte for byte).
`clse4/tools/wsplit2_{ps2,port}.py` -> `wtable.py`, `wbudget.py`, `wdist.py`, `whand.py`, `bursts.py`.
Per cell PS2 | port; a port shotgun "shot" is one trigger pull; hp = health falls split over the victim's aimers.

| beyond 30 m | aimer time % | firing % | shots / s | hp / shot | hp / aimer-s | in a fight % |
|---|---|---|---|---|---|---|
| Kamino, all | 578 s \| 6877 s | 21 \| 28 | 0.97 \| 1.35 | | **14.4 (10.1-18.6) \| 24.5 (22.4-27.3) = x 1.70** | 56 \| 85 |
| rifle | 52 \| 31 | 34 \| 48 | 1.64 \| 2.41 | 11.9 \| 13.1 | 19.5 \| 31.5 | 63 \| 93 |
| pistol | 10 \| 16 | 0 \| 40 | **0.00 \| 1.62** | - \| 15.1 | 11.4 \| 24.4 | 63 \| 57 |
| shotgun | 10 \| 15 | 11 \| 14 | 0.36 \| 0.57 | 16.3 \| 38.9 | 5.9 \| 22.1 | 52 \| 92 |
| sniper rifle | **0.9 \| 19.9** | 14 \| 9 | 0.58 \| 0.36 | - \| 68.4 | 0 \| 24.9 | 11 \| 93 |
| Polis Massa, all | 161 s \| 2033 s | 20 \| 13 | 0.88 \| 0.62 | | **7.4 (6.5-8.8) \| 20.4 (6.4-29.4) = x 2.75** | 35 \| 84 |
| rifle | 59 \| 29 | 28 \| 20 | 1.32 \| 1.01 | 5.3 \| 25.1 | 7.0 \| 25.4 | 47 \| 85 |

(20-30 m and every other weapon: `clse4/out/table_{kam1,pol1}.txt`. Kamino 20-30 m: 34.3 | 35.7 = x 1.04, rifle
1.91 | 2.19 shots a second; Polis Massa 20.4 | 15.3 = x 0.75.)

- **Per aimer-second hides the aimer time.** As hp credited per 100 living-AI foot-seconds (`out/budget_*.txt`)
  Kamino beyond 30 m is 58.5 | 146.0 (x 2.49; 16 % | 44 % of all damage is dealt there): sniper rifles 0 | 29.6,
  rifles 40.9 | 58.8, pistols 4.8 | 22.6, shotguns 2.3 | 19.3. So of the 87.5 excess: **snipers 34 %, rifles 20 %,
  pistols and incinerators 22 %, shotguns 19 %**. The weapon-time mix alone explains nothing of the per-aimer ratio
  (the port's per-weapon rates on the PS2's mix: 24.8).
- **A PS2 bot fires its weapon only inside its `MaxRange`** (`out/wdist_ps2.txt`, both maps, shots per aimer-second
  inside half the range / to the range / the 10 m beyond / farther): rifle (64 m) 1.78 / 1.68 / 0.09 / 0.00, pistol
  (24) 1.64 / 1.80 / 0.07 / 0.00, shotgun (48) 0.67 / 0.72 / 0.05 / 0.00, incinerator (8) 0.89 / 0.77 / 0.00 / 0.00
  (4 m distance cells). The port's gate is the bolt's reach (128 m): pistol 1.22 / 0.97 / 1.07 / 1.45, incinerator
  3.3 / 2.4 / 2.1 / 2.1. Inside the range the PS2's rate does not fall with distance.
- **A PS2 rifle fires 3-4 shots and waits 2 s** (`out/bursts_kam1.txt`): bursts of 3 (40 %) and 4 (27 %), mean
  2.99, pauses 1.34 / 1.97 / 2.27 s (quartiles); the port's bursts run together (mean 6.1 at a 0.45 s gap, a
  quarter of the shot gaps are 0.45 s). The PS2 fires as much outside a fight order as in one (rifle beyond 20 m
  1.51 against 1.70 shots a second; "firing and moving are independent", 4.9); the port only in Fight.
- **The PS2's snipers hardly shoot**: a sniper rifle is in hand 21.5 % of Kamino's AI foot time (16 % on Polis
  Massa) and its holder is an aimer 0.4 % of that; 99 % of it is the trooper agent's state 7, the snipe stand (3),
  standing still, an enemy within 64 m 8 % of the time (`out/ps2_holder_kam1.txt`; 23 shots in 3,051 unit-seconds).
  The port's: in hand 13.9 %, an aimer 10.2 % of it, a fifth of the far aimer time at the average rate. The port's
  shorter stand is bot design D13's named deviation - this is what it costs.
- Polis Massa's ratio is mostly not fire: the port holds a target 3-4 x the PS2's share of time at 10-30 m (a
  quarter of it unseen) and a far aimer is credited with falls dealt by a near one (its own hits beyond 30 m: 15.5
  hp per aimer-second against the 20.4 credited).

### 20.2 The trigger's range test, both games [O]
- BF2 `FUN_002bb1b0` (B2:242120): with the fire bit of the mover's flags (`+0xa8` bit 7) clear and the
  fire-when-aimed bit (bit 8) set, the want byte becomes `FUN_002ad228`'s answer (B2:242149-242159) - the request of
  4.9 for a tracked target. A set bit 7 passes with no test. `FUN_002ad228` (B2:232274-232300) -> `FUN_002ad1a0`
  (B2:232236-232268): the aim test `FUN_002ace58` on the lead point AND the 3-D distance from the eye (`C+0xd0`)
  to the TARGET's own aim point (its vtable `+0x58` point, no lead) strictly under `FUN_002acd68` (B2:232263
  *(decompiler detail omitted)*).
- `FUN_002acd68` (B2:232018-232033) = `FUN_002a9c30` on the class of the weapon in hand (B2:229977-230011): class
  `+0x118` = **`MaxRange`** (setter hash 0x43bcebe6, B2:320016-320018, which also clears the doubling bit 0x35 - a
  class with its own `MaxRange` is never doubled; constructor default 60), times the controller's factor `+0x100`,
  and when class `+0x100` (max of `LockOnRange` and the ordnance's reach value) is under that product, max(the
  range, class `+0x100`). Factor 1: the range itself, whatever the cap. Factor 2: clamp(cap, R, 2R). The factor is
  2.0 only from `FUN_002d7e58` (B2:262331, the snipe stand, 3; one `jal` in the ELF) and is put back to 1.0 by
  `FUN_002f4a50` / `FUN_002f4c78`.
- BF1 is the same: `FUN_0023cd40` (B1:155162-155174) -> `FUN_0023a0d8` (B1:153094-153124) -> `FUN_0023a050`
  (B1:153058-153091, 3-D, strict) -> `FUN_00239c68` -> `FUN_00237de0` (B1:151448-151484): class `+0xec`
  (`MaxRange`, B1:210139-210141, default 60), the same factor (`+0x184`, 2.0 from `FUN_00257550` only) and cap
  (class `+0xd0`).
- The port: `gunner_foot.rs` gates a hand weapon by `ordnance.max_range()` (velocity x life span: 128 m for the
  pistol's bolt). `WeaponClass::ai_range[2]` already holds the key.

### 20.3 The burst controller's three keys, both games [O] (closes card AIBST; corrects re-ai-difficulty 4.4)
- BF2 `FUN_002bac98` (B2:241918-242021), for a class that passes the cannon type test and has bit 0x32 of
  `+0x1e0` clear (B2:242163-242165; a set bit goes to the timed hold `FUN_002baf70`): want 0 -> state 0, and the
  next burst starts without a pause (`+0xfc` = -100). State 1: n = trunc(min + U[0, max - min) x s), at least 2
  (B2:241953-241962; s = (level - 1) / 19). State 2: start when the clock passes the last burst's END `+0xfc` +
  (1.6 - lerp(0.1, 0.9; s)) x delay (B2:241973), and only if the burst is affordable against the weapon's heat /
  ammo meter (B2:241974-241987). State 3: until n shots or the meter is down to one salvo. (AUDIT, 23.2: the n
  test counts ROUNDS spent, so a weapon with unlimited rounds - every seat gun - is not held to n; the meter is
  heat.)
- min / max / delay = class `+0x214` / `+0x218` / `+0x21c` (vtable 0x628310 slots `+0x34` / `+0x38` =
  `FUN_00331488` / `FUN_003314c0`, ELF words at 0x628344; `FUN_0032b0d8`). **Constructor defaults 3 / 6 / 2.0 s**
  (`FUN_00327e98`, B2:319525, 319548-319550). Keys by hash 0x1aa5ec35 / 0x04bae44b / 0xffcc84b2 (B2:320300-320301,
  320458-320459, 320952-320953) = **`BarrageMin` / `BarrageMax` / `BarrageDelay`**. So a BF2 bot's burst is 3 at the
  lowest level and 3-5 at the highest (max is never reached), and its pause 3.0 s down to 1.4 s.
- Census of the baked class data [receipt:c35231ee] (447081525 / 79357003 / 4291593394): on the BF2 disc only six
  `com_weap_veh_fly_*` classes (chaingun 10 / 20 / 0.5, multilasers, recoilless and scoutlasers 8 / 16 / 0.5,
  stingers 3 / 15 / 0.5, torpedo 2 / 8 / 0.5) and `com_weap_inf_defenseturret` (4 / 8 / 0.5) and their children
  write them - every infantry weapon keeps the defaults; on the BF1 disc no class writes them [receipt:c35231ee].
  Live: `+0x214` / `+0x218` / `+0x21c` = 3 / 6 / 2.0 in the RAM of all 20 weapon classes in hand on Kamino
  (`clse4/out/wclass_burst_kam1.txt`).
- BF1 `FUN_0023c9c8` (B1:155009-155104; reached when `FUN_0023c758` fails, class `+0xcc` is 0 and the cannon type
  test passes, B1:155182-155189): n = `FUN_002324e0(min, max)`, a uniform integer **including max** with no skill
  term and no floor (B1:147388-147403), the pause the class's delay flat, no skill factor (B1:155058). Class
  `+0x1bc` / `+0x1c0` / `+0x1c4`, the same three hashes (B1:209909-209912, 209945-209949, 210559-210563), defaults
  3 / 6 / 2.0 (`FUN_00285a98`, B1:209514-209516): **3-6 shots, then 2 s**.
- The port: `core/src/ai.rs` `BURST_MIN` 2, `BURST_MAX` 6, `PAUSE_BASE_S` 0.5 - placeholders ("no RE"). Its pause
  is a quarter of the PS2's.
- [H] not opened: who sets the mover's bits 7 / 8; the meter gate's numbers per weapon; the hash that sets bit 0x32.

### 20.4 Knocked into the port [L]
One scratch binary on main 7d206bc (`clse4/proto_sw2_7d206bc.patch`; env word `CLSE4`: 1 / 2 the range test on BF2 /
BF1 rounds, 4 / 8 min 3 and delay 2.0 on foot / in a seat on BF2 rounds, 16 the same on BF1 rounds with BF2's draw;
unset = main byte for byte, seed 1 x 17 maps). 8 seeds, the first 540 s, per-seed paired t against the base:

| the 17 maps under BF2's rules | main | range (1) | bursts (4) | both (5) | + seats (13) | the PS2 |
|---|---|---|---|---|---|---|
| deaths with an enemy inside 10 m, % | 26.3 | 26.6 | 28.6 (t +4.9) | 28.5 (t +3.6) | 28.7 (t +5.3) | |
| deaths with none within 20 m, % | 45.9 | 45.0 | 44.3 | 42.1 (t -7.3) | 41.9 (t -10.9) | |
| contact20, % | 10.27 | 10.70 | 11.37 (t +2.9) | 11.29 (t +2.4) | 11.45 (t +3.3) | |
| kills a minute; team kills % | 31.4; 1.2 | 32.1; 1.2 | 32.0; 1.4 | 30.3; 1.3 | 30.9; 1.3 | |
| Kamino: inside 10 m %; contact % | 19.0; 2.9 | 19.2; 2.9 | 20.6; 3.2 | 23.8; 3.7 | | 38-70; 16-23 (the three captures) |
| Kamino hp per aimer-second, 0-10 / 10-20 / 20-30 / 30+, x the PS2 | 0.68 / 1.03 / 1.04 / **1.70** | 0.62 / 1.06 / 0.88 / 1.40 | 0.50 / 0.90 / 0.90 / 1.43 | 0.55 / 0.88 / 0.76 / **1.24** | | |
| Polis Massa, the same | 0.36 / 0.38 / 0.75 / 2.75 | 0.37 / 0.40 / 0.75 / 2.55 | 0.33 / 0.34 / 0.65 / 2.86 | 0.31 / 0.35 / 0.64 / 2.44 | | |

136 of 136 rounds end in every column. With both, Kamino's rifle beyond 30 m fires 1.38 shots a second (the PS2
1.64) in bursts of mean 2.90 (2.99) with a median pause of 1.70 s (1.97). 14 BF1 maps (`b1_v0`, `b1_v2`, `b1_v18`):
the range test alone contact 10.75 -> 10.26 % (t -2.0), deaths inside 10 m 22.7 -> 22.0 %; with the bursts 11.93 %
(t +3.2) and 25.6 % (t +5.4), kills a minute 33.6 -> 30.5, 112 of 112 rounds.
- Both rules cut fire at every range, so the bins inside 20 m - already under the PS2's - fall further (Kamino
  0-10 m x 0.68 -> 0.55). What is missing there is not the trigger: a PS2 bot fires outside a fight order and on
  the move (20.1), and the port's fights are 85-93 % of its aimer time against the PS2's 56 %.
- Not moved by either: the snipers' share (D13), the shotgun's damage per blast beyond 20 m (port 70 / 39 hp at
  20-30 m / beyond against the PS2's 42 / 16 on 27 / 20 blasts - thin), Polis Massa's target time.

### 20.5 Built (card CLSE4, main a490579, 2026-10-12) [L]
Both rules are in the port for both games: `WeaponClass::barrage` / `barrage_delay` read the three keys (defaults
3 / 6 / 2.0 s) and `gunner_foot::fire_discipline` draws by the game's rule (BF2: max(2, trunc(min + U[0, max - min)
x t)), pause (1.5 - 0.8 t) x delay; BF1: uniform min..=max, the delay flat), the pause counted from the burst's last
shot, a seat's gun by its own class; a hand weapon fires only strictly inside `ai_range[2]` (`MaxRange`), 3-D from
the eye, and inside the bolt's reach. The placeholders 2 / 6 / 0.5 are gone.
- **Named choices, not the PS2's:** the x 2 on the range at a snipe stand (20.2) is not built; a burst's start does
  not ask the weapon's heat / ammo meter; a bot that stops wanting to fire still owes the pause it was in (the PS2
  starts the next burst at once, `+0xfc` = -100); a seat's gun keeps the bolt's reach as its only range test.
- **Measured**, main 2e12961 against the branch with the read-only observer, 8 seeds, the first 540 s, per-seed
  paired t (`engine/artifacts/lead_tmp/clse4/out2/`, `sw/{m2_obs,br_obs,m2_b1,br_b1}`):

| | main | built | t | the PS2 |
|---|---|---|---|---|
| 17 maps under BF2's rules: deaths with an enemy inside 10 m, % | 25.8 | 29.1 | +5.2 | |
| deaths with none within 20 m, % | 46.5 | 41.2 | -7.7 | |
| contact20, % of unit time | 11.14 | 12.24 | +5.2 | |
| kills a minute; team kills, % of kills | 33.1; 1.24 | 32.6; 1.47 | -0.7; +3.5 | |
| Kamino: deaths inside 10 m, %; none within 20 m, % | 17.4; 59.2 | 21.7; 50.7 | +1.7; -2.5 | 38-70; 12-26 |
| Kamino hp per aimer-second, 0-10 / 10-20 / 20-30 / 30+ m, x the PS2 | 0.62 / 1.04 / 1.06 / 1.67 | 0.60 / 0.98 / 0.78 / **1.23** | | 1 |
| Kamino rifle beyond 30 m: shots a second; hp a shot | 2.32; 13.0 | 1.39; 16.0 | | 1.64; 11.9 |
| Kamino rifle bursts: mean shots; median pause, s | 6.17; 1.30 | 2.93; 1.67 | | 2.99; 1.97 |
| Polis Massa, hp per aimer-second as above | 0.33 / 0.36 / 0.68 / 1.85 | 0.33 / 0.33 / 0.67 / 2.66 | | 1 |
| 14 BF1 maps: deaths inside 10 m, %; contact20, % | 22.7; 10.75 | 25.7; 11.48 | | |
| BF1: kills a minute; team kills, % of kills | 33.6; 0.72 | 30.9; 0.91 | -4.3; +3.2 | |

  Rounds that end: 136 of 136 and 112 of 112 in both builds; 256 of 256 on the gate's 32 maps x 8 seeds.
- **Costs and what is left (card CLSE5):** team kills rise a fifth of a point in both games; Kamino's 20-30 m bin
  goes from over the PS2 to under it (the rifle 1.32 shots a second there against 1.91); Polis Massa's far bin did
  not come down (1.85 -> 2.66 with seed ranges 5.4-17.0 and 14.7-26.0 hp against the PS2's 7.4 on 14 falls: thin,
  and its excess is target time, 20.1); the bins inside 10 m stay at 0.6 and 0.33 of the PS2's. BF1 Tatooine's
  shut footpath (card TGAP, hubs 78 -> 77) takes more lives with the longer lives: 211 -> 266 recycles in 96 seeds
  (t +2.4), 199 and 256 of them at that one spot.

## 21. Audit CLSE2 (2026-10-12): the block replayed think by think on the running PS2; stance EVENTS add error and a mounted unit's block is its vehicle's - neither is in the port [O]

AUDIT of card CLSE2 (8ba75a7 + d57ab7d): PARTLY. 18.1-18.4 were re-read from the B2 export and the ELF words before
section 18 was opened, then given as claims and citations only to two verifiers working apart (24 claims each: 17
SUPPORTED and 7 PARTLY from both, 0 NOT; the PARTLY ones are line ranges of the auditor's that stop short - and one
"only the decay and the growth change the error" claim of the auditor's own that BOTH overturned: 21.2). Evidence
`engine/artifacts/lead_tmp/audit-clse2/` (`README.md`, `wf/RESULTS.md`, `re/claims.md`, `ps2/`). Checks run:
`re_check.py fn` on B2:120159 (`FUN_00208e28`), 162190 (`FUN_00246ed0`), 162306 (`FUN_002471b0`), 162343
(`FUN_00247440`), 162630 (`FUN_002476a0`), 126201 (`FUN_002125c0`), 186504 / 186603 / 186863 (`FUN_00269d98`,
`FUN_00269f38`, `FUN_0026a340`); `re_check.py callers` `FUN_002ae728` 5, `FUN_002ae010` 2, `FUN_002ae7b8` 2 in the
export's text; `re_check.py word` 0x621860 (5 words), 0x2ae728 (38 words), 0x2f7968 (3 words); the word
`jal 0x2ae7b8` (0x0c0ab9ee) counted over both ELFs: 6 in BF2's, 0 in BF1's [receipt:852fbf1c].

### 21.1 What holds [V] [L]
- 18.1-18.4 stand as written. Added first-hand: class +0x370 (18.3) also has a property setter, key 0x4217f84f in
  `FUN_0026a340` (B2:186860-186864), and a scan of every file of the BF2 disc finds that key in ONE `PROP` chunk:
  `rep_walk_oneman_atst` = 2.0 (`SIDE/REP.LVL`, BASE `walker`) - so every soldier class takes the box (both
  verifiers' own scans agree). 18.4's census read a third time by another reader's scanner: 69 / 25 / 25 chunks on
  10 / 4 / 4 classes, the port's class data equal to the disc on 292 class names over the 17 BF2-rule maps
  (`wf/K3/`). 18.3 in RAM on 21 states of 15 maps other than Kamino: 56 soldier classes, each cache = 0.25 x
  hypot(dx, dz) of its box, the 43 classes the port fields within 0.02 % of `roster::AIM_SIZE` (`wf/K4/`).
- [L] **The port's own function on the PS2's rows.** Two live captures of BF2 Kamino with one row per game update
  (`ps2/block_live.py`: every member's block t+0..0x80, the counter t+0x2d0, the summed dt t+0x2d4, the level of
  detail, the soldier's stance word, velocity and class; the last clean read of each clock value): 126 s and 95 s,
  21,294 and 22,815 thinks of the block. `core::unit::AimBlock::think`, fed each think's err, budget, velocity
  before and after, summed dt, class MaxSpeed and the skill's rate and floor (a scratch test, `tools/mk_replay.py`),
  gives the PS2's err and budget within 1e-4 on 20,979 of 21,272 thinks (98.6 %; the human's 2,089 of 2,090) and
  22,644 of 22,773 (99.4 %), with the quaternion and the direction on the same rows (the rest are row pairs that
  span a respawn, a gap of the sampler, or the events of 21.2). On the 2,806 thinks of the first capture that grow
  by 0.05 or more the rule as built fits 97.5 %; a horizontal change of velocity only 50.0 %, a fixed top speed of
  7.0 87.6 % (the classes' 6.0 / 7.0 / 8.0 are used), growth not cut to the budget 62.6 %, growth before the decay
  34.8 % (`ps2/block_replay.py`, `ps2/bf2_kam1_a_replay.txt`).
- [L] The rate, the floor and the cap are ONE skill level a team (18.1's flag), read off the blocks: the rate
  solved from the budget is 0.784211 on 2,817 of 2,833 still thinks of the human's team (level 14: floor 0.778947,
  cap 1.515789) and 0.626316 on 1,726 of 1,748 of the other team's (level 11: 0.968421 / 1.673684); the most
  common err after a think is that floor on 5,797 and 4,401 thinks, and of the thinks whose radius sits at a cap
  4,127 of 4,181 fit cap = 2.2 - (level - 1) / 19 (3,930 of 3,977 of them a standing bot on the move). The port thinks every unit's block with the tier's one level, 17 on
  Normal (floor 0.589, cap 1.358; `core/src/skill.rs`, the lead's table). A control with the two live levels put
  into the port's blocks (`AUDIT_LVL=14,11`, 8 seeds x the 17 BF2-rule maps; level 17 for both = main's rows hash
  for hash) lowers the damage per aimer-second beyond 30 m 15.51 -> 14.55 (t -3.6, 8 of 8 seeds) and raises the
  deaths with an enemy inside 10 m 29.1 -> 30.1 % (t +2.4): the level is 6 % of the port's far fire, not its cause.
- [L] The radius with its speed term, which 18.2's rest rows and the port's 80-row test cannot show: at the think,
  radius = s x (3 - 2 vis) x min(err x m, cap) fits 11,585 of 11,709 thinks of a standing bot moving over 1 m/s
  (98.9 %; the velocity is the one stored at the think, t+0x10, over the class's MaxSpeed), 4,606 of 4,658 still
  standing, 1,795 of 1,797 crouched (m 1.5); the 75 jump and 18 roll thinks all sit at the cap. An agent's replay
  of two captures made three days after the fixture's (`clse/ps2/bf2_kam1_c4a`, `bf2_pol1_c4a`) gives the same:
  5,352 of 5,394 moving rows within 1e-4 m (`wf/N4/`).
- [L] The direction: quat' = quat x (axis' sin dt, cos dt) in that order on 20,979 of 21,272 thinks (the other
  order 18) - 18.10's [D] for the product's order is seen. The PS2's quaternion is not normalised and its length
  drifts to 0.9995 (least 0.997) in two minutes; the port normalises (named).
- [L] Cadence in play: of a bot's thinks 59 % came 0.1 s apart (level of detail 2), 31 % 0.167 s (level 1) and 9 %
  every update (levels 3 and 4) - about half of a bot's time at each of the first two. The port's "every 5th tick"
  is the slower half (named: one level).

### 21.2 Stance events add error (`FUN_002ae728`, 0x2ae728) - not in 18 and not in the port [O]
- The export shows `FUN_002ae728(t, kind)` only as a jump table it could not recover (B2:233267-233279). The ELF
  words: `sltiu v0, a1, 5`; the table at 0x621860 = 0x2ae754, 0x2ae754, 0x2ae76c, 0x2ae784, 0x2ae79c; 0x2ae754
  `lui at, 0x4000; mtc1 at, f12; jal 0x2ae7b8` (2.0), 0x2ae76c `lui at, 0x3f80` (1.0), 0x2ae784 and 0x2ae79c
  `lui at, 0x40a0` (5.0). So it calls `FUN_002ae7b8(x, t)` - 18.1's add: cut to the budget and paid from it, then
  the floor and the 2.0 cap - with **x = 2.0 for kinds 0 and 1, 1.0 for kind 2, 5.0 for kinds 3 and 4**. Four of
  the six `jal 0x2ae7b8` words of the ELF are these (the other two are 18.1's decay and growth).
- Its five callers pass the block `*(entity + 0x324)`: kind 0 from `FUN_002471b0` (B2:162306), the entry of the
  ROLL (state 5); kind 1 from `FUN_00247440` (B2:162343), the entry of the SPRINT, only when `FUN_00247bf8(E, 3)`
  made the change; kind 2 from `FUN_00246ed0` (B2:162190), the entry of the JUMP (state 4); kind 3 from
  `FUN_002476a0` (B2:162630), the jet (states 6 / 7); kind 4 from `FUN_00208e28` (B2:120159), another entity class
  [H: which one is not read]. Both verifiers found the function on their own, each from the ELF, while trying to
  falsify the auditor's claim that only the decay and the growth move t+0x20.
- [L] **Seen on the running PS2.** The human's block thinks every update, so the event shows as what the think
  rule does not give at the update his stance word goes 0 -> 4 (the second capture, he jumps 16 times): err
  0.7789 -> 1.7789 with the budget down by exactly 1.0000 on 10 jumps; the whole budget (0.2603, 0.2483, 0.3330,
  0.1830) when it held less than 1.0, on 4; +0.9752 to the 2.0 cap with the budget still down 1.0000, on 1; a
  jump on an empty budget, 1, adds 0. His 17 landings (4 -> 0) change the block by 0
  (`ps2/bf2_kam1_b_human_bumps.txt`). Bots: of 59,402 pairs of rows that have the same think of the block err and
  budget moved on 9 - seven jumps (+1.000 on four, the budget's or the cap's remainder on three), one roll (to
  2.0) and one pair with the stance unchanged (`ps2/bf2_kam1_a_bumps.txt`).
- What it means in play: a jump adds a metre of error to every bot that aims at the unit, a roll or a sprint
  start takes the error to its 2.0 cap, and it decays at the skill's rate (0.63-0.78 a second in the captures:
  1.3-1.6 s back to the floor). The budget refills at half that rate, so a second jump at once gains little. The
  port has the stance FACTOR of 18.2 while the stance lasts and the floor again the tick after: card JINK.
- The PS2's bots do not sprint and BF2's soldier cannot lie prone: 0 of 86,652 unit-updates of living bots on foot
  in the first capture hold stance word 2 or 3 (stand 87.6 %, crouch 11.3 %, jump 0.6 %, roll 0.2 %). The port's
  bots lie prone 0.6-3.5 % of their foot time on every BF2-rule map (`Soldier`'s stance cycle stand -> crouch ->
  prone runs under both games' rules) and 18.2's 2.0 then applies to a stance BF2's soldier cannot enter (a note on
  card JINK).

### 21.3 A mounted unit's block is its vehicle's [L] [O]
- The controller hangs off the controllable the unit currently drives: of 267 mounted bots in 49 library states
  the block's owner word t+0 is the vehicle's controllable on 256 and the remote droid's on 11, the soldier's own
  on 0, and the soldier's controllable holds a null controller word (an agent's table, `wf/K5/tab.txt`); on
  Kamino's turrets live, read by the auditor, 1,813 of 1,851 thinks. So 18.2's entity is the VEHICLE: its velocity
  (up to 94 m/s for a flyer) and top speed, the constant 1.0 for the stance slot (0x2f7968: `lui at, 0x3f80;
  mtc1 at, f0; jr ra`), and 18.3's size by kind.
- The size factor s = 2 x `FUN_002a76b8`, read at rest as radius / ((3 - 2 vis) x err): building turrets 2.01-4.16
  (`tur_bldg_laser` and `tur_weap_built_gunturret` 2.009, on 1,758 live thinks too), `rep_walk_oneman_atst` 4.00
  (21.1's authored 2.0), `imp_walk_atst_jungle` 3.86, `cis_walk_spider` 8.60, `imp_walk_atat` 12.46; flyers 4 x
  the cached size (18.2's x 2: snowspeeder 7.63, tri-fighter 9.75); hovers near rest 1.57 (speeder bike) to 4.13
  (snail tank), which agree with 0.4 x hypot(class +0xbe4, +0xbe8) [H]; a remote droid 0.17. A foot soldier's is
  0.48-1.02. A seat's or a turret seat's controllable resolves to the parent vehicle's entity.
- The port takes the seated SOLDIER's size, stance and MaxSpeed, and his velocity is zeroed every tick
  (`core/src/sim/seats.rs`), so a rider's block sits at the floor: its radius is 2 to 25 times smaller than the
  PS2's. 9-10 % of the port's aimer time on the 17 BF2-rule maps is at a mounted target. Card VAIM.

### 21.4 What the five rules do in a round [L]
- Every number of the landing reproduces from rebuilt binaries (rows byte for byte, 8 of 8 seeds; `wf/RESULTS.md`),
  and the landing with all five rules switched off prints its parent's rows on 256 of 256 rounds but the hash.
  Its two readings do not hold as written: "kills a minute 32.5 -> 31.4" is 1.8 % MORE kills over rounds 5 %
  longer (t +3.0), and gone on today's main (t -0.2); and "inside the seed noise" leaves out the far bin - damage
  per aimer-second beyond 30 m over the 17 maps 19.70 -> 20.70 (t +2.8) at the landing and 14.67 -> 15.51 (t +4.7,
  8 of 8 seeds) on main with the rules off -> on, where they also cost 0.5 points of the time with an enemy within
  20 m (t -2.5) and 1.0 point of the deaths with an enemy inside 10 m (t -2.4). Faithful rules, added to a far fire
  that is over the PS2's for other reasons (20): the cost is small and is CLSE5's.
- A single rule switched off alone moves each round measure by less than 8 seeds resolve on both bases
  (`sw/k1json/`).
- Left out of the port and first named here: 21.2, 21.3; the visibility term t+0x7c is live on the PS2
  (`FUN_002aec38`: under 1 on 3.5 % of a bot's time on Kamino, 0.0 on 1.5 %, a mean factor of 1.048) where the
  port holds 1.0 (re-ai-difficulty 5.3 has it PROVISIONAL); the radius is cached at the think on the PS2 (stance
  and speed up to 5 updates old) and computed at the aim in the port.

## 22. Card HVYW (2026-10-12): the weapon a foot soldier may use on a VEHICLE - the suitability test of both games, read whole [O]

Read for card HVYW (heavy weapons with a purpose) before building. Hops: the weapon choice B2 `FUN_002ab590` /
B1 `FUN_002387b0` (4.3, re-grenades 7.1) -> its one suitability test, B2 `FUN_002aaba0` (B2:230554-230784) / B1
`FUN_002380a8` (B1:151570-151776), arguments (attacker, weapon, weapon CLASS, target, &score). Checks run:
`re_check.py fn b2 230620 230662 230711 230776`, `fn b1 151652 151681 151762` (all seven lines in those two
functions), `callers` (one each: B2:231054, B1:151945), and ONE opus verifier given the claims and citations only -
9 claims: 5 SUPPORTED, 4 PARTLY, 0 NOT; it read every compare from the instructions as well and its corrections
are in the text below (`engine/artifacts/lead_tmp/hvyw/verify/verifier_report.md`). Class names are the strings
each id registers (`FUN_005393c0` / B1 `FUN_00444b88`), read by the verifier, not by this session.

### 22.1 BF2 (`FUN_002aaba0`) [O]
1. A weapon class with no ordnance: WeaponRemoteClass scores a fixed 1000 (unless its +0x240 is a flyer class),
   WeaponMeleeClass 1000 against a soldier, droideka, walker, hover or armed building (B2:230578-230609); any
   other such class is rejected after the mask (230657). An EntityTrap target is rejected (230616-230619).
2. **The mask** (230622): weapon class +0x124 - the `AITarget*` word (setter `FUN_00328fb0`: Person 1, Animal 2,
   Droid 4, Vehicle 8, Building 0x10) - must hold the bit of the target's health type (target+0x228 & 7). A clear
   bit rejects the weapon, with two exemptions: a droideka target with the Vehicle bit set (230624-230629), and
   soldier against soldier when the target's controller passes the cluster test `FUN_002aec90` (230632-230653:
   more than 3 non-hostile soldiers within 9 m of it and none hostile, 4.3). **So against a vehicle a refusing
   mask always rejects**: the disc's rocket launchers write `AITargetPerson 0`, `AITargetVehicle 1`,
   `AITargetBuilding 1`; the wrist rocket writes no AI key and `TargetVehicle 0`
   (`engine/artifacts/lead_tmp/hvyw/tools/wdump.py bf2`).
3. **Too weak** (230661-230662): score = `FUN_002aa948` (the rate of re-grenades 7.5); `score x 10.0 <= max health x
   0.5` (target+0x174: the damage function divides by it, B2:91373-91383). NOT too weak -> accepted with the score,
   whatever the ordnance (230778-230780).
4. Too weak, and the ATTACKER is a hover, walker, armed building or mounted turret (230663-230688): accepted when
   the score is over 0. A foot soldier goes on:
5. The TARGET must be (230692-230727) a flyer in state 0, 3 or 1 (waiting, landing, taking off) or slower than
   16 m/s (speed^2 < 256.0); or a hover; or a walker; or a building with bit 0 of its +0x26e set (the flag is
   "the class's MaxHealth is finite", a destructible building: audit HVYW, 24.3). Else rejected.
6. The ORDNANCE class (230729-230778), a hierarchical is-a tested in this order: grenade, sticky -> accepted only
   against a target slower than 12 m/s (speed^2 < 144.0); beam, haywire, missile, shell -> accepted; any other
   (bullet, laser, emitter ...) rejected. **Haywire derives from sticky** (ctor `FUN_0031ab90` B2:309511-309519),
   so it takes the 12 m/s gate and its own branch is dead; beacon derives from sticky too. 4.3's "missile / shell /
   beam / haywire" is corrected by this.
So a rifle that does not take half a tank's pool in ten seconds is never fired at it, a rocket is (by its
ordnance, though weak), and a bot with nothing that passes does not attack that vehicle at all.

### 22.2 BF1 (`FUN_002380a8`): the same shape, three differences [O]
- The mask is the `Target*` word (weapon class +0xf8; BF1 has no AI mask: the hash of `AITargetPerson`,
  0x48e05b6f, has 0 hits in B1's export [receipt:0b484166], an [H] as far as the export's holes go) and it is asked
  ONLY of a soldier against a soldier (B1:151649-151655), with no cluster and no droideka exemption: a BF1 launcher
  (`TargetPerson 0`) is refused for every soldier target, and every weapon is put to a vehicle's too-weak test.
- Too weak (151680-151682): min(the rounds issued, 50) x one shot's damage <= max health (target+0xc) x 0.5 - no
  rate (re-grenades 7.5 correction 2).
- The target of a weak weapon must be a hover, a walker, or a building whose +0xe0 is non-zero (not identified
  [H]) (151705-151719); the function holds no flyer test [receipt:0b484166] (the verifier: no 256.0 and no flyer
  class id in 0x2380a8-0x238678). The six ordnance classes and the 144.0 gate are BF2's (151721-151767).
- Ahead of the mask (151608-151633), not in BF2: a SOLDIER's WeaponCannonClass weapon scores a fixed 1000 when
  the target's class field +0x7c is non-zero (not identified [H]) - past the mask and the too-weak test.

### 22.3 The port, and what card HVYW builds [L]
Census on main fb4a2f1, 8 seeds x 32 maps, full rounds (`lead_tmp/hvyw/sw/main`, `tools/hvcen.py`): the rocket
and torpedo launchers are in a bot's hands for 215,600 s and fire 147 shots, 0 of them at a vehicle; the wrist
rocket is never drawn; bots on foot fire 106,655 shots of other weapons at riders; 3 vehicles fall to a heavy
weapon on foot. Cause: a rider's kind was the RIDER's (a person), so the launcher's mask refused every vehicle.
Built on branch `hvyw` (WIP, not landed): 22.1 steps 2, 3, 5, 6 and 22.2's for a target that rides
(`arsenal::against_vehicle`), "does not attack at all" as no shot and no fight position, and four named
deviations of OURS (the maintainer, 2026-10-12): a heavy weapon (missile ordnance whose mask refuses a person, or not the
soldier's first slot) is used on a soldier only in a crowd of 3 within 9 m with none of the shooter's side, never
inside 6 m; its mask is not asked against a vehicle (the wrist rocket); a bot holding one prefers a vehicle as its
target; a weapon that reloads is kept against a vehicle. Not built: step 1, 4, the flyer states, the building
flags (every turret is taken: PROVISIONAL), BF1's cannon 1000. First measurement of the first build (8 x 32, one
block; "heavy" here by weapon NAME - rocket, torpedo, bulldog, EMP launcher - which also counts three person
weapons): heavy shots 1,742 -> 11,787 (at a vehicle 342 -> 5,813, at a group 221 -> 4,166), vehicles destroyed by
a heavy weapon on foot 3 -> 142, rounds 256 / 256, team + self kills 1.28 -> 1.32 % of kills; a missile is still
a straight bolt with no blast in the port (card UORD), so a rocket at a group hits one man.

## 23. Audit CLSE4 (2026-10-13): both trigger rules hold and are read live from the PS2's burst state; a weapon with unlimited rounds is not held to n shots [O]

`# OPUS RE (2026-10-13) - [O] UNAUDITED` - AUDIT of card CLSE4's landing (main a490579, section 20): **PARTLY**.
Re-derived from the two exports and the ELF words before section 20 was read; hops: B2 `FUN_002bb1b0` ->
`FUN_002bac98`, `FUN_002ce108` -> `FUN_002ad1a0` -> `FUN_002acd68` -> `FUN_002a9c30`, `FUN_003037e8`,
`FUN_002a29d8` / `FUN_002a3528`; B1 `FUN_0023cd40` -> `FUN_0023c9c8`, `FUN_0024d9f0` -> `FUN_0023a050` ->
`FUN_00237de0`, `FUN_002840c0`. Evidence and tools: `engine/artifacts/lead_tmp/audit-clse4/` (README.md; `ps2/` =
the RAM reads, `out/` = the port's tables, `wf/RESULTS.md` = the workflow). Tags: [V] read here, [L] live or
measured, [D] derived, [H] one reader only.

### 23.1 What holds [V] [L]
- **The landing's numbers**: every quoted figure reproduces from binaries rebuilt at 2e12961 and a490579, and the
  audit's sample files are the session's byte for byte (272 of 272 on the BF2-rule maps, 224 of 224 on the BF1
  maps, 96 of 96 Tatooine rows, both builds). With both rules switched off the landing's tree gives the parent's
  rows hash for hash on 8 of 8 seeds; the landing's own 15 mutations each fail a test on main 1aa04dc.
- **The range test** (20.2) [V]: read as written in both games by three verifiers. Added: the three entity types
  that keep an un-authored `MaxRange` undoubled are `EntitySoldier`, `EntityDroideka` and `EntityDroid` in both
  games (type names at the registered addresses, B2:172985 / 113966 / 104323; two verifiers) - so a soldier's
  class default is 60 m, the port's `ai_range[2]` default, and a hover's, walker's, flyer's or turret's is 120 m
  (the port has no seat range test). The factor `+0x100` goes back to 1.0 at the top of EVERY think
  (`FUN_002b7530` -> `FUN_002f4c78`, B2:239209; `FUN_002f4c78` is that reset, not a constructor), and the 2.0 has
  one writer, one `jal` [receipt:983acdd5].
  Live [L], six BF2 captures (`wf/K4`, a second reader with its own parser): of 2,380 rifle shots at a tracked
  soldier 1 is beyond 64 m where 41.4 are expected with no test (Poisson p 4.5e-17), of 448 pistol shots 3 beyond
  24 m where 166.9 are expected, commando pistol 0 of 183 beyond 32 m (17.8), shotgun 1 of 136 beyond 48 m (24.6),
  incinerator 0 of 26 beyond 8 m (132.4).
- **The burst's draw and pause** (20.3) [V] [L]: the state machine is in RAM and was read at 2 Hz from captures
  other cards made - BF2 the trigger block `ai + 0x430`: state `+0xec`, the last burst's end time `+0xfc` (-100
  after a reset), the rounds at the burst's start `+0x100`, n `+0x104`; BF1 the K block: `+0x1d8` / `+0x1e0` /
  `+0x1e4` / `+0x1e8` (`tools/ps2_burst_bf2.py`, `ps2_burst_bf1.py`).
  BF1 (three captures, 837 draws after a served pause): n = 3 / 4 / 5 / 6 on 21.6 / 26.6 / 25.2 / 26.5 %, the two
  teams alike - uniform, the maximum included, no skill term; the least (now - end time) over 1,333 firing
  samples is 2.00 s in each capture - the delay flat. The first live reading of the BF1 twin.
  BF2 (six captures): team 1, the human's side, draws 3 / 4 on 64.1 / 35.9 % of 298 and never 5 - level 11's
  63.3 / 36.7 / 0 - with the least pause 2.17 s (2.158); team 2 on Kamino 51.0 / 47.8 / 1.2 % of 255 - level 14's
  48.7 / 48.7 / 2.6 - with 1.94-1.97 s (1.905). On Polis Massa team 2 climbs inside a capture: the least pause by
  minute 2.04 -> 1.92 -> 1.83 -> 1.67 s (level 17: 1.653) and five-shot draws 0 of 12 -> 1 of 21 -> 7 of 15 ->
  5 of 13 - the skill rubber band of re-ai-difficulty 6 (`FUN_002a3528`, B2:225273, every 15 AI ticks, +-4), seen
  live for the first time. The port draws at level 17 for both teams all round (its tier design).
- One claim of the AUDITOR's own was wrong and both verifiers said so: the cannon classes override the two
  getters (`FUN_00331488` / `FUN_003314c0`), as 20.3 has it. One caller of each burst function, by `jal` count
  [receipt:b71d8be3] (B2 `FUN_002bac98`) [receipt:e799f579] (B1 `FUN_0023c9c8`).

### 23.2 The n-shot test counts ROUNDS - a weapon with unlimited rounds fires on [O] (not in 20.3, not in the port)
- State 3 ends when the meter test passes OR `n <= rounds at the burst's start - rounds now` (B2:241999-242000;
  B1:155084-155085). Rounds = B2 `FUN_003037e8(weapon + 0x90)` (B2:294548): `(reserve in clips + the clip's
  fraction) x RoundsPerClip`, and **0x7fffffff when the magazine word is 0x7fffffff or the reserve is FLT_MAX**;
  B1 `FUN_002840c0` (B1:208239) the same. For such a weapon both figures are 0x7fffffff, the difference is 0 and
  n never ends the burst (two verifiers, U1 / U2: SUPPORTED by both).
- What makes a weapon unlimited (two verifiers, U3, the same correction from both): `RoundsPerClip` (hash
  0x7bff9ec9) authored under 1 stores 0x7fffffff (B2:294437-294439, B1:210092-210094; the constructor's default is
  1 in both games), OR the OWNER's slot holds an unlimited reserve - BF2: no `WeaponAmmo` (0xdc19bd13) on the
  slot, or one under 1 (the slot bit 0x20000000 is the owner constructor's default, B2:111944, 113125); BF1:
  `WeaponAmmo` under 1 ([H] a BF1 slot with no `WeaponAmmo` at all: one verifier reads a byte fill of -1 there,
  not followed).
- The meter is HEAT (two verifiers, U4): `weapon + 0x94` -> current / `EnergyBar`, drained by `HeatPerShot`
  (class `+0x90`, 0x67cffd1a) x `ShotsPerSalvo` (class `+0x294`), refilled by `HeatRecoverRate`. A burst starts
  only when n shots' heat fits in what is left and ends when one more salvo no longer fits (`+0xf8` = 1 - cost -
  0.0001). With `HeatPerShot` 0 neither test ends it: the gun fires for as long as the bot wants to.
- So for a seat gun - `FUN_002bb1b0` is the one trigger function of the soldier, droid, droideka, hover, walker,
  flyer and mounted turret updates (seven `jal` words; classes by position [D]) - and for an unlimited hand weapon
  the PS2's burst is "until the heat is one salvo short of full, or the want drops", then the class's pause, then a
  new burst once n shots' heat fits. n is only the start condition.
- **Live** [L] (`ps2/burst_state_mounted.txt`, `burst_state_wclass.txt`, `burst_unlimited_bf1.txt`): BF2, six
  captures - a MOUNTED bot's rounds-at-start word is 0x7fffffff on 3,404 of 3,405 firing samples; it is in the
  firing state for 28.4 % of its samples; of its 407 firing runs 72 % last 1.0 s or more, 43 % 2.5 s or more and
  91 over 6 s, with n drawn 3-5 as on foot. A rifle on foot (rounds counted on 1,332 of 1,340 firing samples):
  12-16 % and 1-2 %. BF1, three captures - bots on foot with unlimited rounds (2-4 units a capture, all of team
  1): 67-68 % of firing runs last 1.0 s or more and 15-32 % 2.5 s or more; with counted rounds 4-28 % and 2-6 %.
- **The port** counts n shots for every weapon and every seat (`gunner_foot::fire_discipline`, called from
  `gunner::aim` too) and since CLSE4 pauses 1.65 s (BF2) / 2.0 s (BF1) after them where the parent paused
  0.35-0.75 s. Measured on main 1aa04dc, the seat's Barrage rule off -> on (8 seeds x 32 maps): shots per
  mounted-bot-second with a target 0.155 -> 0.135 (-12.9 %, t -4.7; `wf/K3`), every lost shot from a class with
  the default keys (the combat speeder's missile 2,046 -> 1,237, the gun turret 561 -> 367); kills by mounted
  bots on the BF1-rule maps 74.5 -> 60.9 per 100 minutes (-18 %, t -2.7). No shot in any sweep came from a class
  that authors the keys, so "a seat's gun takes its own class's" is exercised by no round and by no test.
  Class data (`tools/wcensus.py rounds`), by the PORT's test (`infinite_ammo`: `RoundsPerClip` unset or under 1)
  and not single-shot: BF1 35 seat classes (all with heat) and 6 infantry; BF2 78 seat classes (41 with heat, 37
  without) and 16 infantry (`cis_weap_inf_repeater` 0.09 heat a shot, `com_weap_inf_chaingun` 0.02, the auto
  turrets). The PS2's test is the owner's slot (above), which the port keeps apart (`GunWeapon::infinite`, the
  roster's spare clips): the count by slot is card BRSTU's first step.
- Knocked in as an upper bound (`AUDIT_CLSE4=32` in `bin/rto_mainsw3`: no shot count for a seat gun or a hand
  weapon with `RoundsPerClip` under 1; the weapon's own overheat stands in for the meter test): mounted shots per
  target-second 0.162 -> 0.235 on the BF2-rule maps and 0.113 -> 0.121 on BF1's, mounted kills 148 -> 173 and
  58 -> 71 per 100 minutes, the repeater and the chaingun 0.48 -> 2.87 shots per second in hand; deaths with an
  enemy inside 10 m +0.15 (t +0.4), kills a minute -0.44 (t -1.7), 256 of 256 rounds (`out/mounted.txt`). Card
  BRSTU builds the rule.

### 23.3 The forgiven pause, sized [L]
- 20.3 / 20.5 name it as not built: want 0 puts the state to 0 and the next burst owes no pause. On the PS2 579
  of 1,369 burst starts of a rifle on foot follow such a reset (42 %: team 1 47 %, team 2 39 %;
  `ps2/reset_share_bf2.txt`), and 27.1 % of its rifle pauses are under 1.5 s against the landing's 10.7 %.
- With the pause always served the landing's rifle fires UNDER the PS2's rate in every range bin on Kamino: 1.38 /
  1.19 / 1.32 / 1.39 shots per aimer-second at 0-10 / 10-20 / 20-30 / over 30 m against 1.78 / 1.65 / 1.91 / 1.64
  (the parent 2.23 / 1.96 / 2.22 / 2.32). Drawn at the PS2's levels (14 / 11) it falls to 1.01 / 0.98 / 1.04 /
  1.16. Damage per aimer-second, port over PS2: x 0.62 / 1.04 / 1.06 / 1.67 -> x 0.60 / 0.98 / 0.78 / 1.23 - two
  bins toward 1.0 and two away, the sum of |ln| 1.08 -> 0.97; Polis Massa x 0.33 / 0.36 / 0.68 / 1.85 -> x 0.33 /
  0.33 / 0.65 / 2.66 (main 1aa04dc 2.63), three away, 3.15 -> 3.59.
- Knocked in (`AUDIT_CLSE4=16`: a bot not on target this tick owes no pause; the port's own on-target test, which
  is tighter in angle than the PS2's 22.5 degrees): the rifle 1.65 / 1.32 / 1.33 / 1.45, the ratios x 0.61 / 0.96
  / 0.88 / 1.19, deaths with an enemy inside 10 m +0.14 points (t +0.5), none within 20 m +1.08 (t +2.9), contact
  -0.40 (t -1.9). It brings the 0-10 m rate to 93 % of the PS2's and the 20-30 m damage from x 0.78 to x 0.88,
  and it is NOT what holds the middle bins' rate down (1.32 against 1.65-1.91); card CLSE5 has the rest.

### 23.4 Smaller points
- By rule on main (`wf/K1`, 8 seeds, paired t against both rules off): the foot's Barrage rule carries the
  landing's closeness (deaths with an enemy inside 10 m +1.9 points BF2, +2.8 BF1); the range test alone moves
  none of it (BF2 +0.09, t +0.1), lowers BF1's contact (-0.50, t -2.0) and kills a minute (-2.07, t -3.7), and
  carries Tatooine's recycles (8 -> 27 in 8 seeds; the card's 211 -> 266 over 96 seeds reproduces, t +2.4, 474 of
  477 at card TGAP's footpath).
- "A pistol 128 m" was the code's bound, not the play: on the parent no pistol shot was at a target beyond 100 m
  and 1 of 112,478 beyond 64 m; 48.7 % were beyond 24 m (`wf/K2`).
- `cis_weap_inf_wrist_trishot` authors `MaxRange` 0 and fires nothing since the landing (159 shots -> 0, Geonosis);
  the PS2's R is 0 for it too [D].
- `TriggerSingle` is ON from the base weapon class's constructor and OFF from the cannon class's (B2:319538 /
  324647, B1:209496 / 211881; three verifiers): the port's default is off for every class, so it bursts BF2's 11
  `remote` guided-rocket classes where the PS2 taps them (67 of 10,027 mounted shots in a sweep: nil today).
- Barrage keys on the disc: 39 BF2 classes carry authored keys (26 write them, 13 more by their base) - 35 flyer
  weapons, the defence turret and three space turrets (`tur_weap_spa_*`, not in 20.3's list); the BF1 class files
  hold 0 of the 78 hits [receipt:42ad5ca0].
- [H], read by one verifier each and not built: `FUN_002b8028` holds fire for 2 s after the unit's own shot hit
  something under half way to the target while it stands within 1 m of where it shot (B2:239801); `FUN_002ace58`
  asks a sideways miss under 20 m beyond 50 m; `FUN_002bb900` vetoes on a listed sphere in the line.

### 23.5 Checks
- `re_check.py fn` on every cited line (B2 241934-242005 in `FUN_002bac98`, 232264 `FUN_002ad1a0`, 229990
  `FUN_002a9c30`, 294548 `FUN_003037e8`, 225273 `FUN_002a3528`; B1 155025-155055 `FUN_0023c9c8`, 147400
  `FUN_002324e0`, 208239 `FUN_002840c0`); the key hashes computed (MaxRange 0x43bcebe6, BarrageMin 0x1aa5ec35,
  BarrageMax 0x04bae44b, BarrageDelay 0xffcc84b2, RoundsPerClip 0x7bff9ec9).
- Workflows `audit-clse4` (21 agents) and `audit-clse4-verify2` (2): five verifiers, claims and citations only.
  First list, 20 claims: 16 SUPPORTED, 3 PARTLY, 1 NOT (the auditor's own wording on the getters); the 8 absence,
  twin and open claims were read twice with the same verdict both times. Second list (23.2), 5 claims read twice:
  U1, U2, U4 SUPPORTED by both, U3 and U5 PARTLY by both with the same corrections (taken above).
- Stayed [H]: a BF1 slot with no `WeaponAmmo`; the classes behind the seven callers (by position); which unit
  holds the unlimited-rounds weapon of BF1's team 1 in the captures (no weapon rows in BF1's packets).

### 23.6 Audit BRSTU (2026-10-21): the start / end test's slots decoded; the state's other writer [O]
- [V] The burst's two meter slots, B2 (card BRSTU had them [H]): the cannon class vtable 0x628310 holds `FUN_00331460`
  at +0x2c and `FUN_00331478` at +0x30 (ELF words 0x62833c / 0x628340). `FUN_00331460` = `lwc1 f1,0x294(a0);
  cvt.s.w; lwc1 f0,0x90(a0); mul.s` = (float)class[+0x294] x class[+0x90]; `FUN_00331478` = (float)class[+0x294]
  (both decoded from the ELF bytes: the export has no body for either). class+0x294 is written under hash 0x3b38558 =
  `ShotsPerSalvo` (`FUN_00330810`, B2:324789-324790), +0x90 is `HeatPerShot` (0x67cffd1a). So FUN_002bac98's start
  cost is ceil(n / ShotsPerSalvo) x ShotsPerSalvo x HeatPerShot and its end mark one SALVO's heat; `SalvoCount`
  (0x9260ea63, B2:301213 in `FUN_0030ec88`) is in neither. A trigger that commits SalvoCount > 1 salvos can therefore
  pass the end mark by the rest of its salvos [D] (whether the PS2's salvo runs on after the release: [H]).
- [V] The burst state has a second writer: B2 `FUN_002bbed0` (B2:242662) sets AI+0xec = 0 (with the weapon index
  AI+0x108, AI+0xd8 = -1.0, bit 9 of AI+0xa8 cleared); `FUN_002ab590` calls it only when the chosen weapon is not
  the current one (B2:231011 under *(decompiler detail omitted)*), the other seven callers when the AI is handed a weapon by kind. B1
  `FUN_0023d3f0` (B1:155524) sets AI+0x1d8 = 0 the same way. So a weapon switch ends a burst owing no pause; a seat
  with one gun is reset only by its want.
- Checks: `re_check.py fn` on B2 241980 (`FUN_002bac98`), 324788 (`FUN_00330810`), 242662 (`FUN_002bbed0`), 231011
  (`FUN_002ab590`), 301213 (`FUN_0030ec88`), B1 155524 (`FUN_0023d3f0`); `re_check.py word b2 0x62833c 2` (0x00331460,
  0x00331478); the hashes computed (ShotsPerSalvo 0x3b38558, SalvoCount 0x9260ea63, HeatPerShot 0x67cffd1a). Workflow
  `audit-brstu`: two verifiers, the same 10 claims and citations; both SUPPORTED the slot decode and the machine; my
  claim "the state is written only in the machine" fell with both (the writers above). The port's knock-ins and
  numbers: `engine/artifacts/lead_tmp/audit-brstu/EVIDENCE.md`.

## 24. Audit HVYW (2026-10-15): section 22 holds; a foot soldier DOES attack a flyer in the air, and 22.1's building flag is "MaxHealth is finite" [O]

`# OPUS RE (2026-10-15) - [O] UNAUDITED` - AUDIT of card HVYW's landing (main 3fba385, section 22): **PARTLY**.
Re-derived before section 22 was read: B2 `FUN_002aaba0` (230554-230784) and B1 `FUN_002380a8` (151570-151776) read
whole; hops B2 `FUN_002ac8f8` (the attack matrix, 231782) -> `FUN_002a6138` (target category) -> `FUN_002ac2b8` ->
`FUN_002abee0` / `FUN_002ac270` -> `FUN_002ab590`; B1 `FUN_002397d8` -> `FUN_00234c00` -> `FUN_002392a0` ->
`FUN_00239260` -> `FUN_002387b0`; B2 `FUN_00267bf0` (the entity base ctor), `FUN_002ab430` -> `FUN_002e0338`. Checks:
`re_check.py fn b2 231822 231489 231495 230690 230695 230700 230712 231044 185257`, `fn b1 152675 152346 152331` (each
in the function named), `absent b2 '[0x183]'` [receipt:5878f5dc], `callers b2 FUN_002c42d8` (0: a vtable slot, data
word at 0x622990). ONE workflow `audit-hvyw` (22 agents) with two blind verifiers given the same 12 claims (citations
only): 10 SUPPORTED by both, H5 answered alike by both (one marked the claim's guesses NOT SUPPORTED), H12 NOT SUPPORTED
by both. Evidence: `engine/artifacts/lead_tmp/audit-hvyw/` (`wf/results.json` V1 / V2, `wf/RESULTS.md`).

### 24.1 What holds [O]
Every claim of 22.1 / 22.2 that the build uses: the too-weak test (B2 score x 10 against target+0x174 x 0.5, +0x174 the
MAX health - the damage function `FUN_001dfb38` divides by it, B2:91416; BF1 min(rounds ISSUED, 50) x one shot against
target+0xc x 0.5, `FUN_00238090` = clips issued (+0x94, set only in the weapon ctor) x rounds a clip), the six ordnance
classes in order (grenade, sticky -> 12 m/s; beam, haywire, missile, shell -> accepted; haywire and beacon derive from
sticky), BF2's mask with its two exemptions, BF1's mask asked soldier against soldier only, BF1's function without a flyer
test (no 256.0 word and no flyer id in 0x2380a8-0x238678).

### 24.2 A foot soldier attacks a flyer in the air - 4.4's first bullet corrected [O]
The matrix does NOT refuse a soldier x flyer pair. B2 `FUN_002ac8f8` sends target category 5 (`DAT_00689368`, the flyer
type, `FUN_002a6138`) to `FUN_002ac2b8` (231822), which calls `FUN_002abee0` (state word +0x60c == 0 and a CommandFlyer)
or `FUN_002ac270` (231489-231495) - both choose a weapon with `FUN_002ab590`. B1 `FUN_002397d8` sends category 5
(`DAT_005849f0`) to `FUN_002392a0` -> `FUN_00239260` -> the chooser `FUN_002387b0` with no test (152675, 152331).
"In the air" is asked by the weapon choice only on BF2's too-weak path (22.1 step 5: state 0, 1 or 3, or under 16 m/s;
230690-230712); BF1 asks nothing. So in BOTH games a weapon that is NOT too weak for a flyer is used on it at any speed
and height; a too-weak one only in BF2, only on a waiting / landing / taking-off or slow flyer. 4.4's categories 7 (turret
seat / passenger slot), 10 (item) and 11 do fall to the matrix's default (the zeroed mode: no attack). [H] Of the other
reads of the state word in the AI range [receipt:5878f5dc], `FUN_002c42d8` (vtable 0x622970, unnamed) drops a flyer
target not in state 0 - it walks to within the two bodies' radii + 1.5 m and claims the target with kind 1, which reads
as boarding, not the attack. The port's `arsenal::against_vehicle` applies this gate only to a weak weapon (issue #131,
November 12, 2026).

### 24.3 22.1 step 5's building flag [O]
Bit 0 of the halfword at +0x26e (bit 48 of the u64 at +0x268) is "the class's MaxHealth is finite": the entity base ctor
`FUN_00267bf0` writes `(class+0x2ec < FLT_MAX) << 0x30` (B2:185257-185258); class+0x2ec defaults to FLT_MAX (186491) and
is set only by the `MaxHealth` key (0x19971f1b, 186808). BF1's +0xe0 is the same test of its class+0x4c (`FUN_00221200`,
B1:135459). So a weak weapon with missile / shell / beam ordnance may be used on a DESTRUCTIBLE building (both verifiers;
neither found another writer).

### 24.4 Open [H]
- `FUN_002ab430` (from the chooser, B2:231038-231051, when its commit flag is set) throttles GRENADE (`DAT_00699d10`) and
  MISSILE (`DAT_00699d50`) ordnance per TARGET and team through `FUN_002e0160` / `FUN_002e0338(target, team, 4)` /
  `FUN_002e03e0`, with no test of the target's type; the capacity is the target class's virtual at vtable+0xa4 (kind 4),
  not read for a vehicle. 4.3's "on a given soldier" is narrower than the code. The port claims thrown weapons only.
- The health type at +0x228 is rewritten by building and soldier state code (6 / 5 / 4: B2 `FUN_001e2d40` 93228,
  `FUN_001e3638`, `FUN_001e3c38`, `FUN_00257010` 174218, `FUN_00258050` 174361; one verifier): a wrecked building is asked
  the mask bits 0x20 / 0x40.

### 24.5 The explosive claim on a vehicle: as many per team as the hull has positions (card HVYA) [O]
`# OPUS RE (2026-10-21) - [O] UNAUDITED` - card HVYA step 4 (24.4's first bullet). BF2 only; BF1's twin was not opened [H].
Hops: B2 `FUN_002ab590` (the chooser, 231038) -> `FUN_002ab430` (230849-230870) -> `FUN_002e0160` / `FUN_002e0338`
(268148) / `FUN_002e03e0`; the target's primary vtable slot +0xa4 per class (the vtable each class ctor stores at offset 0,
named by its IsA slot +8 -> type-id global -> `lead_tmp/sair/tools2/typeids.py`). Checks: `re_check.py fn b2 230860 230861
230863 268156 136593 176395 124024 185244 130734 130751 130785 175510 114408` (each in the function named below), `word b2`
0x61cd14 / 0x61f8f4 / 0x61c35c / 0x61ec2c / 0x619b1c / 0x61b5ac (the +0xa4 words), the hole `FUN_0026bb30` disassembled
(`lead_tmp/re/timelimit/mipsdis.py bf2 26bb30`: `jr ra; addiu v0, zero, 1`), `absent b2 "[0x94] = " "+ 0x250) = "`
[receipt:7a34149a]. One blind opus verifier, 6 claims: 4 SUPPORTED, 2 PARTLY (wording, folded in below).
- [O] The throttle is for GRENADE and MISSILE ordnance (`DAT_00699d10` OrdnanceGrenadeClass, `DAT_00699d50`
  OrdnanceMissileClass; any other ordnance passes) and asks nothing of the target's type: the shooter passes when it already
  holds a kind-4 claim on the target (`FUN_002e0160`) or `FUN_002e0338(target, its team, 4)` >= 1, and the claim is recorded
  (`FUN_002e03e0`); else the chooser skips the weapon (231039-231041). `FUN_002e0338` = max(0, capacity - the kind-4 claims
  held on the target by that TEAM), capacity = the target's virtual +0xa4 (kind).
- [O] Capacity of kind 4 by class: EntitySoldier `FUN_00249f30` 1 (any kind but 3); EntityBuilding, EntityDroid,
  EntityDroideka `FUN_0026bb30` 1; EntityHover `FUN_002219f0` and EntityWalker `FUN_0025b278` (kind 3: 4) and EntityFlyer
  `FUN_0020ea80` = the int at entity+0x250 (`FUN_002690c8` = entity+0x240, +0x10).
- [O] entity+0x250 is zeroed by the base ctor `FUN_00267bf0` (185244) and counted up by the hover ctor `FUN_00219918` once
  for the hull (130734), once per mounted turret of the class's 8-slot list at class+0xc68 (130751, `FUN_0026bf88`, a
  MountedTurret) and once per passenger slot (130785, `FUN_00271808` PassengerSlot, built when class+0xc60 != 0, count
  class+0xc64); the walker ctor `FUN_00259970` (175510 / 175527 / 175561) and the flyer ctor `FUN_001fff10` (114408 / 114428 /
  114464) the same; the other writers are destructors' decrements and other classes whose +0xa4 does not read it
  [receipt:7a34149a].
- So on BF2 at most ONE soldier of a team fires a rocket or throws a grenade at a given SOLDIER (or droid, building) at a
  time, and at most N at a hover / walker / flyer, N = 1 + its turrets + its passenger slots: the hull's positions.
- [H] How long a claim lives: `FUN_002e03e0` stamps each record with `FUN_004240f8` (a clock read); its expiry was not traced
  (4.3's 10 s is the grenade's).
- The port (card HVYA): `ArmState::claim` covers thrown weapons on any target, one per team; a missile is not claimed. Named
  in `ai/arsenal.rs`'s header as a deviation; the build is card HVYB.


### Minefield hints in the port (November 12, 2026)

The ground brain checks nearby Minefield hints during a capture order.
A proximity mine goes through the usual weapon switch and trigger. The
mover follows the normal authored graph, with a bounded approach and a
short hold after placement. BF2 FUN_002d4908 and FUN_002d4a60
(B2 259977–260071) establish dispenser capacity and the three-second
hold. Detpack detonation and native floor selection remain open (#132).
