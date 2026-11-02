# OPUS RE (loop lane 2, 2026-10-02, card CONT) - [O] UNAUDITED: no Fable pass
# RE: the Conquest AI goal — how a PS2 bot picks its target post and when it picks again (BF2 and BF1)

> **AUDIT 2026-10-02 (loop lane 3, PARTLY — `engine/artifacts/loop/audits.md`, report
> `engine/artifacts/loop/reports/20261002_2300_audit_CONT.md`, evidence `engine/artifacts/lead_tmp/audit-cont/`).**
> Audited with the landing CONT (4c1460d5). Re-derived from both ELFs before this doc was read; one Opus verifier,
> 33 / 3 / 0 of 36.
> - **Held.** Sections 0-7: the draw, its weights and drops, the 1 s tick and its two triggers, the at-post helper's
>   one 5 s test, BF1's twin and its events, the retake call. The two live battles through an independent reader:
>   Polis Massa's three 20-valued posts took 368 of 556 draws (373.4 by value, 283.9 if uniform); in-life changes
>   after a life's first order one per 224 s (Polis Massa, by the game clock) and 333 s (Kamino).
> - **Qualified.** "0 of 578 draws took a friendly post" counts the kept draws: of 947 target changes one landed on
>   an own post (Polis Massa, member 31, t 339.5, before the life's first order; replaced a second later) — the
>   first pass can find nothing for one tick. Section 2: a BF2 passenger draws for itself when its driver holds no
>   post; BF1's passes the empty target on. Section 6: BF1's uniform fallback also needs two posts (B1:182191); BF1
>   also SETS a target without a draw when a vehicle unloads at a post (`FUN_00235488` B1:149679 and `FUN_0023d418`
>   B1:155612, through `FUN_00261e20`) and re-issues the kept one through `FUN_002622d8`.
> - **Missing from section 3 and from the build (card HUBM): the hub's mover mask.** Section 10.

Card CONT asked how a BF2 bot picks its post and when it re-picks, because the port's bots do not close with the enemy
(nearest enemy within 20 m: PS2 Kamino 17-25 % of alive unit time, port 2-5 %) and change objective every 10 s. Read
from both decomps and ELFs, BF2's mission scripts, the RAM of three reference save states, and two live 9-minute PS2
battles sampled four times a second (BF2 Kamino and Polis Massa, 16 AI a side, the player never spawned). Tags: [V] read
(lines cited), [D] derived, [H] hypothesis, [L] seen on the running PS2; `[O]` marks the load-bearing claims. B2 / B1 =
lines of `SWBFRNT2.ELF-decomp.c` / `SWBFRONT.ELF-decomp.c`. Scratch, tools and raw outputs:
`engine/artifacts/lead_tmp/cont/` (`goal_ram2.py`, `goal_ram1.py`, `cont_ps2.py`, `analyse.py`, `changes.py`,
`after_capture.py`, `value_census.py`, `ps2/`, `verify/`). Builds on `re-command-walker-post.md` section 8 (the pick's
candidate rule, first read there) and `re-cp-capture.md` (the post's fields).

## 0. The answers [O]

1. **A bot draws one enemy post at random and keeps it.** BF2 gives every AI member of a team to the team's Conquest
   goal; the goal hands a member a target post only when the member's controller has no order (after a spawn, or after
   an order was lost) or when its capture behaviour has finished. In two battles the PS2's bots on foot changed target
   **once per 233-323 alive unit-seconds**; the port changes objective once per 9.6.
2. **The draw ignores distance, enemies and crowding.** Candidates are the posts NOT friendly to the bot's team; each
   weighs its authored `Value_ATK` for the bot's faction (the mean of ATK and DEF when neutral); one uniform random
   number picks among them. No term for how far the post is, how many enemies stand on it, or how many team mates
   already go there. BF2 also drops a post its unit type cannot reach over the planning graph. Live: 0 of 578 draws took
   a friendly post; the draws spread evenly over the candidates' distance ranks (Kamino, 3 candidates: nearest 34 %,
   middle 28 %, farthest 39 %); on Polis Massa, where posts are worth 20 or 10, a top-valued candidate was drawn 118
   times of 185 (125.5 expected by value, 95.9 if uniform).
3. **Own posts are never a target while an enemy post can be attacked.** A bot defends only as a by-product: after
   capturing, it stays at the post (standing at hint nodes or wandering 8-14 m around it) until its capture order is
   interrupted and resumed, then leaves 5 s later. A post whose `Value_DEF` is 0 sends its capturers on at once.
4. **A re-pick comes from four things only (BF2):** no order (spawn; a reaction left the bot without one); the capture
   behaviour ended (5 s after arriving at a post the team can already spawn at; no path; the post's owner entity died;
   the bot sits in a vehicle or turret); the target became friendly with `Value_DEF` 0; a driver's pick is copied to its
   AI passengers. Nothing re-picks on a post changing hands, on enemies appearing, or on a timer.
5. **BF1 has the same pick and the same capture behaviour, but re-picks on events instead of polling:** on death, on a
   vehicle change, when the target post becomes friendly (every bot targeting it, at once) and 5 s after arriving at a
   friendly post (excluding that post). When a post is lost to neutral, up to five AI team mates within 60 m are sent to
   it. BF1 weighs team 1 by `Value_ATK` and team 2 by `Value_DEF` whoever owns the post, and has no reachability test.
6. **What the port does instead** (`core/src/commander.rs`, ours and PROVISIONAL): a re-plan every second, attack value
   divided by hops, minus 0.5 per enemy within 30 m, a crowd term, a cap, defend objectives. Every one of those terms is
   absent on the PS2. Section 8 lists the build.

## 1. The goal manager and the script (BF2) [V]

- **Goal types.** `AddAIGoal` (B2:36558-36690) hashes the goal name (FNV-1a, `byte | 0x20`): conquest 0x5aabb29b → 1,
  destroy 0xc45b6745 → 2, defend 0x4fed19c1 → 3, deathmatch 0x73f5632e → 4, ctfoffense 0x9a8b1b9e → 5 (only when its
  region argument resolves), ctfdefense 0xe12bcbe0 → 6, follow 0x5b6c4f70 → 7, then `FUN_002e79a8(weight, manager
  DAT_005fe7a8, team, type, target)` (B2:36625). That function needs weight > 0, takes the first free of 20 slots at
  manager+0x1c and for type 1 builds a 0x20-byte Conquest goal (`FUN_002e7e08`, vtable 0x625178; B2:273204-273275,
  273383-273392). Goal: +0x00 member list head, +0x10 member count, +0x14 weight, +0x18 enabled (1 from the ctor
  `FUN_002e6e98`; its only other writer `FUN_002e71f0` is called from two other goal classes' updates), +0x19 team.
- **The script.** `ObjectiveConquest.Start` (objectiveconquest.luac function 7, pcs 74-98): when `self.AIGoalWeight > 0`,
  `AddAIGoal(self.teamATT, "Conquest", 100 * self.AIGoalWeight)` and the same for `teamDEF`. `Objective.AIGoalWeight` is
  1.0 (objective.luac main pc 22); `Objective.Start` first calls `ClearAIGoals` for both teams. So a conquest mission
  runs one Conquest goal of weight 100 per team. Ten conquest scripts add goals of their own: `Deathmatch` 100 for a
  locals team (geo1c, kas2c/g, nab2c/g, tat3c/g), `Conquest` for all three teams (end1g; tat2c/g with 1000), and on
  kas2c `Defend` / `Destroy` goals for the gate panel (30) and three barricades (10 each) beside the Conquest 100.
- **Which goal a member is in.** The manager updates when its time stamp (+0x18) has passed and sets it to now + 1.0
  (`FUN_002e7cf8`, B2:273335-273381): a **1 s tick**. Each tick every AI player record (array DAT_00661330, stride
  0x1a4; AI = record+0x130 < 0) that is in no goal of its own team is put (`FUN_002e7808` → `FUN_002e7580`,
  B2:273102-273133, 272959-273034) into the enabled goal of its team with the smallest `members / (team size × weight /
  sum of weights)` — members are spread over a team's goals in proportion to the weights. A record is also re-filed
  whenever it is prepared for a respawn (`FUN_0034ffe0` → `FUN_002e7708`, B2:346292). Then the tick calls every goal's
  update (vtable slot +0x1c).
- [L] RAM (`goal_ram2.py`, BF2 Kamino CW at 65 s): two goals, both Conquest, weight 100.0, teams 1 and 2, 19 and 20
  members (every AI record); the manager's next tick 0.03 s ahead of the mission clock.

## 2. When a member is given a target (BF2) [V][O]

The Conquest goal's update `FUN_002e7e48` (B2:273394-273514) does two things each second.

1. The base update `FUN_002e7110` (B2:272753-272812): for every member with a controller, read the controller's
   current order type (controller+0xb0). **Type 0 or 1 (no order) → assign.** Otherwise assign only when
   `FUN_002b7898(controller)` is true: the behaviour object at controller+0x420 reports finished (its state machine's
   state is 100; `FUN_002adc18`, B2:232752).
2. Its own loop, for every member whose order type is not 0xd (the squad follow order, `FUN_002b62e0`): if the member's
   target post (member+0x18) has a live owner whose team is friendly to the member's AND the post's `Value_DEF` for the
   member's faction is 0 → **assign now** (B2:273494-273498). Otherwise assign when the order type is 0 or 1.

"Assign" is `FUN_002e80b0(member, 0)` (B2:273526-273601; reached only from these two places — two `jal`s in the ELF):
- on foot: `post = FUN_002e8318(member, 0, 0)` (section 3), then `FUN_002e8260(member, post)`: an order of **type 8**
  ("capture this post"; `FUN_002b6568`, built nowhere else) replaces the controller's order queue, and member+0x18 = post;
- driving a vehicle (a turret gunner counts as its driver): the same, and every other AI occupant gets the same post;
- a passenger: the AI driver's target if it has one, else its own draw.

Consequences [D]:
- The excluded-post argument is always 0 in the Conquest goal: a bot can draw the post it just left.
- A member with no order is assigned twice in one tick (the base update, then the goal's own loop, because the order
  type only changes when the controller next thinks) and again every second until its controller thinks; the last draw
  before the think wins. A controller's think interval comes from a table 4, 3, 2, 1, 0.25 s by detail level
  (`FUN_002b6fe8`, B2:238925; how the level is set is not traced [D]). Live, with the player on the spawn map, an
  order arrived 4.0 s after the spawn (median 4.0, max 4.3 s over 346 lives) and the target changed 0-4 times in
  that time [L] — the samples ran every controller at the slowest interval; near a player the table gives 0.25-1 s.
- member+0x18 is not cleared by a death: the next life starts with a new draw anyway (order type 0).
- BF2 empties the hooks BF1 used (section 6): `FUN_002e7788` (the 5 s rule), `FUN_002e7910` / `FUN_002e7918` (post
  activated / killed) and `FUN_002e7920` (owner changed, called by `FUN_00359c08` B2:353007) are empty functions.

## 3. The draw (BF2) [V][O]

`FUN_002e8318(member, exclude, pass)` (B2:273642-273818), pass 0, over the scheduled posts:

| step | rule | lines |
|---|---|---|
| candidate | the post has no live owner, or the member's team's relation toward the owner's team is < 1 (enemy or neutral) | 273729-273753 |
| drop | the member drives a vehicle and the post's ban flags (post+0x118) match the vehicle class's type mask (class+0x330) | 273757-273765 |
| drop | it is the excluded post (never used by the Conquest goal) | 273766 |
| drop | its value ≤ 0 | 273769-273772 |
| drop | the member has a controller, a planning graph exists and `FUN_002ebf70(graph, controller, post position)` ≠ 1 | 273788-273808 |
| weight | `… none → (ATK + DEF) × 0.5 | 273863-273930 |
| draw | r uniform in [0, sum of weights); the first candidate, in list order, at which r ≤ the running sum | 273686-273698 |

- **Pass 1** (only when pass 0 yields nothing): every scheduled post through the same drops — the team's own posts,
  weighed by `Value_DEF`. If that yields nothing either: uniform over all scheduled posts (0 when fewer than two).
- **No distance, threat or crowd term.** Neither function computes a distance, counts units, or reads how many members
  hold a target (every call of both bodies listed by the verifier; no subtract / multiply / square root in either).
- **The reachability test** `FUN_002ebf70` (B2:276371-276474): the unit's hub mask (`class+0x330`; 0x20 → always 1), the
  hub nearest the unit and the hub nearest the position (`FUN_002a6c68`: the capture region's centre, else the owner's
  live position); the same hub → 1; otherwise whether a route exists (`FUN_002eb000`), or — when either end lies
  outside its hub — whether a planned route reaches the position within 100 steps (`FUN_002ec220`). It answers
  reachable / not; it is not a distance weight. The route search draws from the same generator as the pick.
- [L] BF2 Kamino CW state: the script blocks five planning-graph arc groups (`BlockPlanningGraphArcs('group1')`,
  connections 165 / 162 / 160 / 225) and **0 of 19 CIS bots target cp2** (14 cp3, 5 cp4); in the GCW battle, whose
  script blocks none, cp2 takes its share (29 of the 68 draws made while cp2, cp3 and cp4 were the candidates). That
  the blocked arcs are what cuts cp2 off is [H] (the graph was not walked); the chance of 0 in 19 with three equal
  candidates is 0.05 %.
- The values the discs author (`value_census.py`, all layers of each baked map): 10 on most posts; unequal on BF1 NAB2
  (30 / 50), RHN2 (15 / 20), TAT1 (`re-cp-capture.md`: 10 / 20 / 30 / 50 / 0 / 100), BF2 DAG1, KAS2 (100 / 300 / 1000),
  MYG1 (15 / 100 / 1000), NAB2, POL1 (20), TAN1, UTA1 (1 - 10); zeros on most BF2 maps (a walker's post is DEF 0).
  The port already resolves both arrays per team (`core/src/cp.rs` `value_atk` / `value_def`, `value_for`).

## 4. What the bot does with the target (BF2) [V]

- **The order queue.** A controller holds a current order (controller+0xb0: type, +0x14 the post) and up to five queued.
  "Set" replaces the queue (`FUN_002b5f10`); a reaction puts its order at the head and the current one behind it
  (`FUN_002b60f0`), so the capture order resumes afterwards ([L] the type-8 order sat queued under a current type-2 or
  type-4 order on 11 of 32 controllers of the Kamino state). The soldier controller (`UnitMobileController`, queue
  vtable 0x621f10) maps order type 8 to state 9 (`FUN_002b8d60`, B2:240439), and entering state 9 builds a NEW capture
  behaviour for the order's post (`FUN_002b70d0` case 9 → `FUN_002cb970`, B2:239027-239054). A turret gunner's
  controller (`UnitStationaryController`, 0x622070) maps type 8 to its idle state, a flyer's (`UnitFlyController`) to
  its own: neither runs the capture behaviour.
- **The capture behaviour** (vtable 0x623618; enter `FUN_002cba48` B2:253856-254039, update `FUN_002cc0a0`
  B2:254171-254570). State 1: at the post already → state 4, else state 2 ("at the post" is `FUN_002a6d18`
  B2:227625: inside the capture region; without a region, within max(10, unit radius + owner radius + 5) m, 20 m more
  for an owner that can move — read here, not put to the verifier). **State 2, walk:** destination = the post's
  position + a random point of its capture region. Leaves state 2 for: 4 on arriving (or when the mover reports
  done); **100 when the mover reports no path**; 3 to follow an AI-driven hover / walker no faster than the soldier,
  within 50 m, heading within 50 m of the same place, when the destination is ≥ 150 m away; 7 a hint stand (below);
  6 a repair / dispenser detour (a unit whose class kind is 2 or 3, every 5 s, within 75 m). **State 4, at the post:**
  runs the at-post helper; the behaviour is finished (100) when the helper is. A dead post owner ends it in any state.
- **The at-post helper** (vtable 0x6240e8; `FUN_002d53c8` B2:260525-260794, `FUN_002d5c30` B2:260796-260937):
  - **The 5 s rule:** once per start, at the first think 5.0 s after the start: if the post has no capture region, or
    the member's team can spawn at it (`FUN_00356880`), the helper ends → the capture behaviour ends → the goal draws a
    new target within a second. Otherwise nothing, and the test never runs again for this start.
  - in a vehicle → ends at once; owner not friendly → stand at the post's position + a random point of the region until
    it is (state 2: this is the capturing); owner friendly → one time in four (always for a non-soldier) wander to the
    post ± (a random x, z in [−1, 1)) × 8-14 m for 20-30 s, else walk to one of up to ten hint nodes near the post and
    stand there until 30-45 s after setting out; then decide again. A droideka at the post stops instead.
    **Correction (card SAIR, 2026-10-02, `re-soldier-ai.md` 3):** in BF2 the hint branch never runs - the helper keeps
    only hints whose CommandPost is its post and BF2's getter `FUN_002deec0` returns 0 (the setter is empty), so a
    BF2 bot at its post always takes the wander; the hint walk is BF1's (PatrolHelper).
  - So a capturer that arrived more than 5 s before the capture has passed its one test and **stays**, cycling hints
    and wanders, until some other order interrupts the capture order; the resumed order builds a fresh behaviour,
    which finds the bot at an own post and ends 5 s later [D]. One that arrived within 5 s of the capture leaves at
    its first think after the 5 s mark [L: m01, m11, m07 on Kamino left 4.5-7.5 s after the capture; m04, m12, m17 held
    the post to the end of the sample].

## 5. Orders that go in front of the capture order (BF2) [V][L]

`FUN_002c3730` (B2:248341-248525) turns a message into an order: an enemy seen and visible → type 2 with the target
(`FUN_002c31e0`); damage taken → type 2 at the attacker's position, or a 1 m evade step, and in one branch a **type-0
order** (`FUN_002c28e8`, B2:247982) — while a type-0 order is current the goal's tick takes the bot as having none and
draws again [D][L: 35 of Kamino's 61 in-life changes followed order type 0]; a free vehicle or turret → type 4 (enter
it). Type 2 runs `FUN_002c4c80` (0x250 bytes: the fight), type 4 `FUN_002c3f30`. Their rules, durations and how far a
fight pulls a bot from its route are **not opened [H]**. The hint stand (capture state 7; `FUN_002d7c30` B2:262128,
`FUN_002a8850` B2:229013, `FUN_002d7d60` B2:262182; read here, not put to the verifier): a unit whose class kind is 0
looks for a hint node every 8-12 s, walks to it and stands 60-120 s — which classes are kind 0 is [H] (snipers by the
share).

[L] share of alive AI time, two 9-minute battles (`analyse.py` section 3, `changes.py`):

| | Kamino | Polis Massa |
|---|---|---|
| capture order, walking to the post | 34.1 % | 48.4 % |
| fight order (type 2) | 27.7 % | 20.1 % |
| capture order, hint stand (state 7) | 19.3 % | 15.5 % |
| entering / manning a turret or vehicle (type 4) | 11.6 % | 5.1 % |
| no order (type 0) | 3.5 % | 5.6 % |
| capture order, at the post | 1.2 % | 1.5 % |
| target post held by the bot's own team | 9.2 % | 5.0 % |

A life: median 59 s (Kamino), 51 s (Polis Massa); **9 % and 8 % of lives reach their target's capture region**; the
median closest approach to the target is 131 m and 90 m. The battle is fought between the posts by bots crossing
toward each other's posts.

## 6. BF1 [V]

- **The draw** `FUN_002618b8(member, use_exclude, exclude)` (B1:182098-182205): candidates = scheduled posts the member's
  team cannot spawn at (`FUN_0029de80` = 0); dropped for a driver's ban match and, when asked, the excluded post;
  weight `FUN_002617f8` (B1:182069-182096) = `Value_ATK[faction]` for **team 1**, `Value_DEF[faction]` for **team 2**
  (and team 3 when it has a faction) — it does not look at the owner; the same weighted draw; else uniform over all
  posts. No value ≤ 0 drop, no reachability test, no second pass, no distance.
- **Assign** `FUN_00261c18` (B1:182255-182332) is BF2's `FUN_002e80b0` (driver shares, passenger copies, foot draws);
  `FUN_00261b48` issues the order (type 3 in BF1) and stores member+0x18.
- **When** (`FUN_00261dc0`, eight call sites): the member's init / reset (`FUN_00295b60`, `FUN_00297be0`), a team
  switch (`FUN_00295c40`), **death** (`FUN_00297440`, B1:222158), a **vehicle change** (`FUN_00296df8`), the at-post
  helper's **5 s rule** (`FUN_00255fd0`, B1:173509-173520: the same test as BF2's, then a draw that EXCLUDES this post)
  and a vehicle arriving at a post (`FUN_00255790`); and **every member targeting a post** when that post is activated
  (`FUN_0029cc00` → `FUN_002616a0`) or when it becomes friendly or stops being friendly to the member's team
  (`FUN_002a0738` → `FUN_00261f88`, B1:228958-229003, 182431-182469).
- **Retake.** When a post goes from friendly to neutral, `FUN_00262090` (B1:182471-182564) orders up to five AI units
  of the team within 60 m (not flyers, no ban match) to that post.
- BF1 has no goal manager, no 1 s tick and no "stay as defender": a capture sends every bot that targeted the post
  elsewhere at once. The at-post helper (`FUN_00255790` / `FUN_00255fd0`) is otherwise BF2's (the same 20-30 s wander,
  30-45 s hint stand, one-in-four draw).
- [L] `goal_ram1.py`, BF1 Kamino and Kashyyyk Docks mid-battle: 0 of 24 / 25 / 31 / 25 / 7 AI members hold a target
  their own team owns. On Kamino team 1 owned six posts and team 2 one: team 2's 25 bots spread over the six 5 / 6 / 5 /
  1 / 5 / 3, and all 24 of team 1's hold the one post it does not own.

## 7. Measured on the PS2 [L]

`cont_ps2.py <map>`: the kept 16-AI side-select state, SW renderer, the player left on the spawn map; every 0.25 s each
record's target post, controller order, behaviour state and position. Runs: `ps2/bf2_kam1_*` (mission time 36-560 s,
162 AI lives, 16,468 alive unit-seconds) and `ps2/bf2_pol1_*` (24-424 s, 185 lives, 12,008 unit-seconds).

| | Kamino | Polis Massa |
|---|---|---|
| draws at the start of a life / of a friendly post | 161 / 0 | 185 / 0 |
| draws inside a life / of a friendly post | 61 / 0 | 171 / 0 |
| rank by distance of the drawn post, 3 candidates (nearest / middle / farthest) | 34 / 28 / 39 % | 43 / 25 / 31 % |
| the same, 4 candidates | — | 28 / 22 / 21 / 29 % |
| in-life target changes, bots on foot | 45 = one per 323 unit-s | 43 = one per 233 unit-s |
| in-life target changes, seated bots (re-drawn every second, never acted on) | 16 | 128 |
| nearest enemy within 20 / 40 / 60 m, median | 17.0 / 31.5 / 43.6 %, 64 m | 31.9 / 50.6 / 65.5 %, 39 m |

- **The weighting** (Polis Massa, posts worth 20 / 20 / 20 / 10 / 10 / 10): of 185 spawn draws 118 took a top-valued
  candidate; 125.5 expected by value (z −1.2), 95.9 if uniform (z +3.3). Kamino's posts are all 10: its draws are
  uniform over the candidates (χ² 0.8 and 3.9 on 2 degrees of freedom for the two opening candidate sets).
- **Why a bot re-draws inside a life** (Kamino, all 61 changes): 35 after order type 0, 19 after its behaviour ended
  (5 at a post its own team held), 5 while entering a turret, 2 other.
- Kamino's contact differs between runs: 24.9 % within 20 m in card STALL's 420 s run (teams guessed from class
  pointers), 17.0 % here (teams read from the records). Take 17-25 % as the PS2's range, not one number.
- Both samples have the player on the spawn map, so every bot thinks at the slowest interval (section 2).

## 8. For the build (card CONT, `core/src/commander.rs`) [D]

The port's commander is a team planner that re-scores every unit every second. The PS2 has no team planner: each bot
holds one target. Build the PS2's rule and delete the terms it does not have.

1. **State:** one target post per AI unit (the existing `Objective::Attack(cp)` / `Defend(cp)` can carry it), kept
   across plans. No per-plan re-scoring.
2. **When a unit draws** — BF2 maps: (a) it has no target (first plan after a spawn); (b) its target is its own
   team's and `value_def` for its team is 0; (c) its target is its own team's and it has been inside the capture region
   5 s since it last arrived or last finished a fight (the PS2's defenders leave on their first interruption + 5 s;
   this is the port's stand-in for that, PROVISIONAL only in that substitution); (d) no route to the target. BF1 maps:
   (a); when its target becomes its own team's — every unit holding it, at once; 5 s after arriving at a post its team
   holds, excluding that post; and the retake call: a post lost to neutral takes up to five AI units of the losing
   team within 60 m.
3. **The draw** — candidates: posts not friendly to the unit's team (BF1: posts the team cannot spawn at); BF2 drops
   value ≤ 0 and posts the unit's type cannot reach on the route table (the port's `route_patch` already applies the
   scripts' blocked arcs); weight: BF2 `CpDef::value_for` as it stands; BF1 `value_atk` for the PS2's team 1,
   `value_def` for its team 2, whoever owns the post (check which port team index is the PS2's team 1). One uniform
   number from the per-unit hash (the port's rule: no shared LCG in the plan). Nothing drawn: BF2 own posts by
   `value_def`, then uniform over all posts; BF1 uniform over all.
4. **Delete:** hops in the score, `THREAT_W`, crowding, the objective cap, hysteresis, per-plan noise, `DEFEND_IDLE`,
   garrisons, defenders-per-attacker, majority / ticket biases. `Hunt` stays only as the port's own endgame (card
   STEND), not as an objective the draw can produce.
5. **Not in this build:** the fight order, the hint stand and the turret order (section 5) are the bot's own reactions;
   the port's brain has its own. If contact is still short after the build, they are the next RE (how long a fight
   holds a bot, and where it resumes).

Test vectors: Polis Massa's start (team 1 owns cp6con 20, cp4con 10, cp1con 10; team 2 cp5con 20, cp3con 10; cp2con
neutral 20): team 2's draw weights cp2con 20 : cp6con 20 : cp4con 10 : cp1con 10 (PS2 21 / 27 / 17 / 14 of 79), team
1's cp2con 20 : cp5con 20 : cp3con 10 (13 / 17 / 8 of 38). Referee numbers for the acceptance: section 7's table.

## 9. Checks run, the verifier, what stayed [H]

Hops: the Lua binding → the goal factory and ctors → the manager's tick → the base and Conquest updates → the assign →
the draw, its value and its graph test → the order ctor and the controller's queue → the order-to-state map → the
capture behaviour → the at-post helper → the owner-change hook; then BF1's twin of each from `FUN_002618b8`'s callers.

- Check 1 (`re_check.py fn`): the first and last line of every cited range, both corpora (74 BF2 and 26 BF1 line
  numbers; all in the named functions).
- Check 2 (absences): `re_check.py absent` plus the raw ELF (`scripts/tools/refs.py`, the verifier's `vrefs.py`):
  `FUN_002e80b0` two `jal`s; `FUN_002e8260` four, all in `FUN_002e80b0`; `FUN_00350658` and `FUN_002b6568` one each;
  `FUN_002e7708` one; the goal's update and assign slots dispatched only from the manager's tick and the two updates;
  no other builder of a type-8 order among the 58 calls of the order ctor; no distance / count term in the draw.
- Check 3 (the twin): BF1's draw, assign, order, at-post helper and owner-change handler opened with their callers.
- Check 4 (constants): vtables 0x625178, 0x624ff0, 0x623618, 0x6240e8, 0x621f10, 0x622070 as ELF words; 1.0 (tick),
  5.0, 22500.0, 2500.0, 50.0 (0x42480000), 75.0 (0x42960000), 8.0 / 14.0, 60.0 / 120.0, 1.0e8 (0x4cbebc20).
- Check 5 (flags): the goal's enabled byte (default 1; never cleared for a Conquest goal); record+0x130 < 0 = AI; the
  relation bytes (RAM: 1 own, −1 the other army, 0 team 0).
- PS2: `goal_ram2.py` on a Kamino state; `goal_ram1.py` on two BF1 states; two live runs (section 7).
- **Verifier** (one Opus agent in the foreground, claims and citations only, `verify/VERDICTS.md`): **46 SUPPORTED, 10
  PARTLY, 0 NOT SUPPORTED** of 56. PARTLY, all folded in above: six other goal hashes, not five, and ctfoffense needs
  its region (section 1); the ban flags are `re-cp-capture.md` line 122; one branch of the graph test gives up after 100
  route steps, and three null guards answer "unreachable" (section 3); the queue vtable 0x621f10 is the soldier
  controller's only — turret gunners and flyers map a type-8 order elsewhere (section 4); the queue's push functions do
  nothing while an order is pending; entering capture states 2 and 4 can divert to state 6, a repair / dispenser
  search (I had read it as a vehicle search); the ride needs a vehicle no faster than the soldier; a droideka at the
  post stops; one citation of the fight behaviour's ctor; one turret row in the RAM listing. Its qualifications, also
  folded in: the double assign per tick; the route search shares the draw's generator; a turret gunner counts as a
  driver; the 5 s test runs once per helper start.

Stayed [H]: what the fight order does and for how long (section 5); which classes take the hint stand; that Kamino CW's
blocked arcs are what removes cp2 (section 3); what a BF1 bot does when its capture behaviour ends without a re-pick
(no path); the other goal classes (Deathmatch for locals, Destroy / Defend on kas2c) — not opened.

## 10. Audit CONT (2026-10-02): the hub mask, and what the build left open [V]

- **A hub with no arc for the mover is not a hub.** At load a hub's mask is the OR of its arcs' type masks (BF1
  `FUN_002663a8` B1:185889-185908, hub+0x14; BF2 `FUN_002eb2b0` B2:275799-275815, hub+0x10). Every search for the hub
  of a position skips a hub whose mask has no bit for the unit: BF1 `FUN_00266678` (B1:185936; called for a path's
  start and for its goal, B1:186434 / 186437), BF2 `FUN_002eb918` and the draw's graph test `FUN_002ebf70`
  (B2:276411). [V; verifier D1-D3; `FUN_002eb918` read by the auditor only.] The port's `NavGraph::locate_on` had no
  such test (`core/src/nav.rs`; card WAI2, a582462, added it for vehicle types only, as PROVISIONAL), and
  `commander::plan`'s reach test reads each post's and each unit's hub from it.
- **What it costs.** BF1 Tatooine Dune Sea's hub 104 (radius 525 m, no arcs) is, in the port, the hub of cp7 and cp2
  and of every unit in the open: those two posts are unreachable from 112 of 115 hubs, the build's Hunt (below)
  takes over, and on main 449f006 **52.2 % of the map's on-foot unit-seconds are Hunt** (8 seeds; 0.2-1.0 % on BF2
  Kamino, Polis Massa and Endor), with a target change per 30 s and 111 recycles in 8 rounds (48 before the build).
  15 of the 32 maps have hubs a soldier can route nowhere from; on BF2 Kamino they make 56 post -> Hunt -> post blips
  in 8 rounds, 33 of them onto another post. With the filter prototyped: TAT1 1.2 %, 26 recycles, kills / min
  24.2 -> 32.1, 256 / 256 rounds; BF2 GEO1's recycles 61 -> 85 (card LLEG's cp7 spot)
  (`engine/artifacts/lead_tmp/audit-cont/README.md`).
- **The build against section 8.** Item 4 kept `Hunt` for the endgame only; the build also returns it when a unit can
  route to no post that is left to take — its answer to the first build's TAT1 (3 of 8 rounds finished), which is
  the hub above. Section 6's vehicle-change redraw (BF1 `FUN_00296df8`) and the vehicle arriving at a post
  (`FUN_00255790`) were not built, nor a vehicle's ban flags (the module header says the last). Four built rules have
  no test: BF1's exclusion after the 5 s test, the passenger rule, the reach drop in the offer, the no-route redraw
  (18 mutations, 14 caught).
- **A capturer stays — in both.** PS2, 21 capture events with the bot at the post: 5 left within 6 s (the one test),
  3 later, 13 stayed to death, the post's loss or the sample's end. Port (main, BF2 Kamino, the first 240 s of 8
  rounds): 1 of 25 left within 10 s, 3 within 20 s, 11 died first.
