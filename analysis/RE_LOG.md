# RE_LOG — reverse-engineering lab notebook

Subordinate to PLAN.md (which holds decisions); this file holds findings:
subsystem → source function addresses → what we learned → open questions.
Addresses are EE virtual addresses as they appear in the Ghidra decomps
(`analysis/SWBFRONT.ELF-decomp.c` = BF1, `analysis/SWBFRNT2.ELF-decomp.c` = BF2).

## Conventions
- BF1 = SWBFRONT.ELF (SLUS_208.98, Lua 4.0.1, zlib 1.1.4, GSM 06.10).
- BF2 = SWBFRNT2.ELF (SLUS_212.40, Lua 5.0.2, zlib 1.2.2, EENet).
- Named functions come from the in-binary luaL_reg-style binding tables
  (`analysis/bf1-bindings.txt` 483 fns, `analysis/bf2-bindings.txt` 769 fns),
  applied in `ghidra-proj/{bf1,bf2}`.
- Cite every ported constant/algorithm in Rust code with its source address.

## Anchors (carried over from initial recon, 2026-08-29/30)

### BF2 (SWBFRNT2.ELF)
| addr | what | notes |
|------|------|-------|
| DAT_005faf7c | lua_State* global | |
| FUN_003c1c38 | startup | root for main-loop hunt (A5) |
| FUN_003f7c68 | version beacon | |
| FUN_0051fbf8 | snprintf | libc |
| FUN_0051d8e8 | memset | libc |
| FUN_0051d734 | memcpy | libc |
| FUN_005220a0 | rand | libc LCG — ported in core/src/rng.rs (see §RNG) |
| FUN_001a6148 | Lua allocator | |
| FUN_0059d568 / FUN_0059d610 | pool create/destroy | |
| FUN_00282f78 | anim-set builder | |
| SetTeamAsFriend (+0x84) | team-relations write | team matrix layout clue |

### BF1 (SWBFRONT.ELF)
No verified anchors beyond the 483 named bindings yet. Startup/main-loop
identification is part of A4/A5.

## Container format (feeds A2/A6, confirmed by direct inspection 2026-08-30)
- All LVL files: `ucfb` magic; chunks = [4-byte ASCII tag][u32 LE size][payload][pad4].
- BF1 MISSION.LVL: `lvl_` → `scr_` children with NAME (plain string, e.g. "bes1a"),
  INFO, BODY.
- BF2 MISSION.LVL: `mcfg` chunks; NAME payload is a 4-byte hash, not a string.

## Audits
- A1 disc inventory: DONE → a1-inventory.md.
- A2 bake formats: → a2-bake-formats.md
- A3 subsystem map: → a3-subsystem-map.md
- A4 call-graph: → a4-callgraph.md
- A5 tick rate: → a5-tickrate.md
- A6 mission Lua: → a6-mission-lua.md

## Subsystem findings
(populated as RE proceeds; one section per subsystem, newest first DOWN TO the
2026-08-30 block that ends at the Collision-LAYERS section — the 2026-09-03,
09-07 and 09-08 sections after it are APPENDED at the end, so the last section
in the file is the newest of all. Checked 2026-09-08, cleanse C2.)

### AI difficulty, skill and feel constants — both engines (2026-09-01, Fable session 8) → `analysis/re-ai-difficulty.md`
Pointer entry; the report is the record (417 lines, function tables per
game, tags per claim). Lead-verified against the ELF data with the
report's own `elfrd.py`: BF2 per-level base skill — the human co-op team's
AI `DAT_00621390` = {6, 11, 11}, every other team `DAT_006213a0` =
{4, 14, 17}; BF1 clamp `DAT_00517db8/dbc` = 1..20, player {6, 4, 4}, enemy
{4, 14, 17} at `DAT_00517dc0..dd4`; `SetAIViewMultiplier` default 1.0
(`DAT_005fe71c`), `AISnipeSuitabilityDist` 70 (`DAT_005fdffc`),
auto-balance on (`DAT_005fdfac`). Headline facts [V]: ONE integer skill
1..20 per team, t = (s−1)/19; higher = better (the BF1 curves at skill 20
give spread ×1.0, AI dispersion ×0.0, 25× faster aim tracking); the shell
level only selects the base; BF2 `SetAIDifficulty` writes per-level
additive deltas; the aim model is a second-order tracker writing
weapon+0x134/+0x138 (spread × lerp(3,1), dispersion × lerp(2,0)) every aim
tick, a tracked-position error radius per target instead of a reaction
timer, burst/pause/charge/grenade/dodge rates in t; perception = 10 m
omni + a cone [D 77.5°] + view distance (20·radius+30)·mult·vis, no LOS
raycast located; BF2 auto-balance (SP/co-op) shifts SKILL ±4 by ticket
gap, never counts (corrects PLAN Log "AI decision", which credited BF2
with balancing by bot count). A3 correction: the Player/Enemy delta
variants write DAT_005875ec / DAT_005875f4 respectively.

### Barriers are the PS2's "Obstacle"s — what `BARR` does, the BF2 arc flags, the arrival test, local avoidance, squad commands (2026-09-01, Fable session 8, AI-data RE 2)
Resolves HANDOFF hold 17 (barrier semantics) and the Phase A data finding
"SOLDIER arcs cross SOLDIER barriers on every map". Read out of both
decomps with the VU0 broadcast fields taken from the ELF words where the
decomp's `_vaddbc` idiom is ambiguous (scratchpad `re_barriers/vu0dis.py
ELF functions.txt FUN_…` lists every COP2 op of a function with its dest
mask and bc field). Census tools: `re_barriers/crossing_census.py` (arcs
vs barriers with the engine's geometry, hubs inside barriers, a
re-implementation of the PS2 local planner run on every crossing leg, a
handedness test; output `census_r035.txt`), `census_details.py`
(unresolved legs by barrier name, dead hubs, sharpened handedness;
`census_details.txt`). PC-tool names stay [H].

**Headline.** A `BARR` is an obstacle for the LOCAL movement layer, never
for the graph: the engine's own name for the object is "Obstacle" (BF1
`FUN_00269dd8` names pool 0x588998 "Obstacle" and 0x5889e0 "PathNode"; the
same string at BF2 0x625838 from `FUN_002f06f0`) [V]. The routing tables
are walked without any barrier check; every hub-to-hub leg is then
handed to a local path builder that tests the straight leg against the
barriers the unit's type mask selects and, when it crosses one, runs an
A* over barrier corners to get around it. So an authored corridor that
crosses a barrier is NORMAL data, not an error, and the crossing census
survives the settled geometry (702 of 4,245 SOLDIER arcs on the 34 base
plans, base = the last top-level chunk — the port's per-map numbers 3 POL1 / 40 of 69 DAG1 / 58 KAM1,
MYG1 are reproduced exactly). `ai.showobstacles` / `ai.showobstaclesradius`
(`DAT_00517d4c` / `DAT_00517d54`, registered by `FUN_002370b0`) have no
reader in the release ELF — the drawing code is compiled out [V].

**Geometry, settled [V].** BF1 loader `FUN_00177828` → `FUN_00268500
(matrix, size, flag)` → node builder `FUN_00267818`: runtime barrier
(0x90 B) = +0x30 FLAG, +0x40 row0, +0x50 row1, +0x60 row2 (as stored),
+0x70 translation with z NEGATED, +0x80 SIZE.x, +0x84 SIZE.y, +0x88
SIZE.z, +0x8c hypot(SIZE.x, SIZE.z). The circle test `FUN_00267920(r,
barrier, p)`: `|dot(row0, t − p)| < SIZE.x + r && |dot(row2, t − p)| <
SIZE.z + r` (both dots are 3-D by the VU idiom at 0x26795c–0x267964,
harmless: every disc row1 is (0,1,0)). The corners `FUN_002679f0(r1, r2,
out, barrier, i)` are `t ± row0·(SIZE.x + r1 + r2) ± row2·(SIZE.z + r1 +
r2)` in the order +A+B, +A−B, −A−B, −A+B. **SIZE is a HALF extent**, row0
is the SIZE.x axis, row2 the SIZE.z axis, SIZE.y is stored and never
read. The segment test `FUN_00267d18(r, barrier, seg{p0, p1, centre,
halfLen}, &dist², &normal)`: coarse XZ reject `|centre − t|²_xz <
(halfLen + r + barrier.+0x8c)²` (VMUL.xz at 0x267d7c), then each edge of
the r-expanded rectangle against the query segment with y zeroed
(*(decompiler detail omitted)*, s and u in [0,1], nearest s wins, normal =
(−e.z, 0, e.x)); it is a pure XZ edge-crossing test with no inside case
(the start point is always known clear). BF2 keeps less: `FUN_002ed5a0`
stores +0x30 row2.x, +0x34 row2.z, +0x38 t.x, +0x3c t.z, +0x40 SIZE.x,
+0x44 SIZE.z, +0x48 hypot, +0x4c the NAME hash, +0x50 u64 `FLAG & 0x1f |
0x20`; `FUN_002ed840` rebuilds axis0 as (−row2.z, 0, row2.x) and tests
the same half extents; `FUN_002ed968` / `FUN_002edd00` / `FUN_002edbb8` /
`FUN_002ee088` are the corner / segment / push-out / silhouette twins of
BF1's `FUN_002679f0` / `FUN_00267d18` / `FUN_00267bd0` / `FUN_00268060`
(one constant differs: the push-out offset is 0.01 m on BF1, 0.1 m on
BF2). **Handedness**: the engine uses the rows exactly as stored in the
z-negated position frame — that IS the semantics — and the disc agrees
that this is the rectangle the designers placed: for the 1,548
non-axis-aligned barriers of aspect ≥ 4 with an instance within 8 m, the
yaw difference to that instance (mod 90°) peaks in the 0–5° bin as
stored (225 vs a flat 160–180 elsewhere) and DIPS there when the rows
are z-mirrored (82); < 2° as stored 154, mirrored 51 [D]. The port's
`Barrier { centre, axes = rows 0/2 xz, half = SIZE }` (game
`layout.rs`) is therefore the engine's rectangle; nothing to change.

**Polarity [V].** Every query gates a barrier with `FUN_002682a8` = `(FLAG
& unitMask) != 0` before testing geometry, so FLAG lists the types a
barrier BLOCKS; FLAG 0 is inert (8 barriers per disc). BF2's consumer,
not found last time: `FUN_002ee2c8(barrier, mask)` = `(flags & 0x1f &
mask) != 0`, gated by the enabled bit — a barrier with bit 5 clear is
skipped whenever `DAT_005fe8e0` is non-zero, and it is 1 in `.data`
(ELF 0x5fe8e0); only `FUN_002ef278` (the nearest-barrier-on-a-segment
query) sets it to its own *(decompiler detail omitted)* for the duration of the query and
restores it, and its three callers pass 1. So Enable/Disable/Toggle
(`FUN_001923a0` → `FUN_002f0650` by name hash → `FUN_002ee330` /
`FUN_002ee300`) really do open and close a barrier; every barrier is
loaded ENABLED regardless of the disc's bit 5 (the loader ORs 0x20), and
BF2 drops FLYER (bit 5 of the disc FLAG) exactly as the last entry said.

**The four BF1 queries and their users [V]** (spatial grid `DAT_00518240`
= `FUN_00197910(16.0, …)` over (±`DAT_00515cdc`, 0) built in
`FUN_002682f0`, barriers inserted by `FUN_00197b38(2·|SIZE|, grid, node,
t)`; the "Obstacle" pool is a growing free-list pool — `FUN_001916d0`
allocates past its 512-element block — so there is no barrier cap):
- `FUN_002686c0(r, p, mask, &hit)`: box p ± r (y unbounded) with callback
  `FUN_00268658` → returns TRUE when the position is CLEAR. Used by the
  graph follower `FUN_002509b0`, the move-to `FUN_0024f3f0`, the hold
  agent `FUN_00248d28`, the waypoint re-roll `FUN_00267648`, the push-out
  `FUN_002689d8`, the planner start `FUN_00269690`, `FUN_00241f18`,
  `FUN_0025aa08`.
- `FUN_002688a0(r, p0, &p1, &normal, mask)`: segment walk `FUN_00199b00`
  with callback `FUN_00268780` → clips p1 to the first barrier hit and
  returns the hit flag and the edge normal. Users `FUN_00235c08`,
  `FUN_002474e0` (fly agent), `FUN_0024fb88`.
- `FUN_00268c98(r, p0, p1, mask, &barrier, &dist²)`: gathers the box's
  barriers (callback 0x268c60 into `DAT_00588470`, count `DAT_00588924`)
  and returns the nearest one crossed. Users: the walk step `FUN_00250fc8`
  and the planner expansion `FUN_002691a0`.
- `FUN_00268f68(r, p0, p1, mask)`: callback `FUN_00268ef0` → "does the
  segment cross any barrier" (`DAT_00588988`). Users `FUN_0026c0d0` (the
  leg builder), `FUN_00235ff0`, `FUN_0025aa08`.
- `FUN_002689d8(r, ?, &pos, mask)` push-out: if `pos` is not clear, four
  candidates `FUN_00267bd0` (the closest point on each edge of the
  r-expanded rectangle, 0.01 m outward), nearest first, first clear one
  written back (x, z only). Users `FUN_00235c08 00235ff0 0024ef70 0024f020
  0024f3f0 0024fb88 00250fc8`.
Unit radius r = `FUN_0024f128` → `FUN_00235ad0(class)` = the XZ diagonal
of the class collision bounds (class+0x1c → +0x80/+0x90) × 0.25 (× 0.5
for an EntityHover, or an EntityWalker whose team slot +100 == 4); the
walk step uses 2r. The mask is `FUN_0024f160` → `FUN_00235be8` =
class+0x78 (AISizeType). The unit kinds that parametrise everything below
come from `FUN_00234c00`, which compares the class's name hash
(`FUN_00444b88` = fnv1a) against `DAT_005862e0` EntitySoldier = 0,
`DAT_00584428` EntityDroid = 1, `DAT_005868d8` EntityWalker = 2,
`DAT_00584498` EntityDroideka = 3, `DAT_005849f0` EntityFlyer = 5,
`DAT_00584ba8` EntityHover = 6, `DAT_005870c0` MountedTurret /
`DAT_00587120` PassengerSlot = 7, `DAT_005842f8` EntityBuildingArmed = 8,
`DAT_005842c8` EntityBuilding = 9, `DAT_00584e38` EntityItem = 10
(strings 0x532f48 0x52fb58 0x533c68 0x530278 0x530a20 0x531700 0x534528
0x534890 0x52f078 0x52e578 0x531b50).

**How a leg is walked, BF1 [V].** The graph agent `FUN_00250868` (state 2
→ `FUN_002509b0(this, 0)`; state 6 waits for the mover to be idle,
`FUN_0023b870` = unit+0x78 == 0, then `FUN_002509b0(this, 1)`).
`FUN_002509b0` polls the controller for a path result (`FUN_0024f1a0` →
`FUN_00263858` / `FUN_00263860`; a result is handed to the mover with
`… a NULL result sets this+0x20 = 1 and
drops the ConnectivityGraphFollower (state 7 = failed); otherwise it
advances the follower (`FUN_00267540`, the chooser walk of the last
entry), takes the next waypoint `FUN_00267538(f, 7)`, and if
`FUN_002686c0(r, waypoint, mask)` says the waypoint is inside a barrier
it re-rolls it with `FUN_00267648`: up to 100 draws of `FUN_002657c8(h,
&p, hub)` = hub + (random 3-D unit vector) × max(0, radius − h), h = 4.0
for an EntityDroideka (`FUN_00267700`) else 0 — the waypoint therefore
carries a random y offset up to ±radius, which only the XZ arrival test
below makes harmless. Then `FUN_0026c0d0(r, unit, from, to, mask, 1,
f+0x50)`: if `unit+0x28c < 11` and `FUN_00268f68` finds NO barrier on
the straight leg, a two-point AIPath {to, from} goes straight to the
mover (`… otherwise a PathRequest (0x50 B:
+0x20 from, +0x30 to, +0x40 r, +0x44 mask, +0x48 age) is queued on
`DAT_00518288`. **The planner** `FUN_0026c400(1.0)` (two calls from the AI
tick `FUN_0025d988`) sets a budget `DAT_00518298 = 10` fine segment tests
per tick (`FUN_00267d18` counts them in `DAT_00518244`; `FUN_0026c5b8`
stops the loop at 10), picks the oldest request (`FUN_0026c2a0`, ages
0..10), starts `FUN_00269690(r, from, to, mask)` and steps
`FUN_00269ac8` until done, then `FUN_00269cf0` builds the AIPath from the
goal's parent chain (a point with |y| < 1e-4 takes the previous point's
y) and `FUN_00263790` hands it over; failures count in `DAT_00518254`.
`FUN_00269690` fails at once if the goal is not clear; otherwise it
clears the open heap `DAT_00518238` and the closed list `DAT_0051823c`,
stores goal `DAT_00588a40`, the perpendicular `DAT_00588a50` = (dir.z, 0,
−dir.x) of start→goal, the band half-width `DAT_00588a60` = max(25 m,
|goal − start|), r `DAT_00588a64`, mask `DAT_00588a68`, and pushes the
start node. `FUN_002691a0(parent, point, depth)` is the expansion: depth
< 32; `|perp · (point − goal)| ≤ band`; the point is not in the
100-entry per-step visited list `DAT_00518250`; if `FUN_00268c98` finds
no barrier between parent and point the node is inserted or relaxed
(duplicates by `FUN_00269148`, |Δ|²_xz < 1e-8, in the open or closed
list; key `FUN_00268bd8` = 100000 − (|node − goal|_xz + g + 0.01) on a
max-heap, i.e. A* with the XZ distance as heuristic and g = parent.g +
|point − parent|_xz; new nodes only while the PathNode count
`DAT_00588a18` < 1024 `DAT_00588a14`); if a barrier IS crossed,
`FUN_00268060(r, barrier, parent, point, &left, &right)` returns the two
silhouette corners of the (r + 0.5 m)-expanded rectangle seen from the
parent (per corner v = norm(corner − parent): the min-dot corner on the
cross < 0 side and the min-dot corner on the cross > 0 side) and the
expansion recurses on both. `FUN_00269ac8` pops the best node, declares
success when it is the goal (`DAT_00518248 = 1`, done `DAT_0051824c`),
else closes it and expands it toward the goal. **The walk step**
`FUN_00250fc8` (agent state 4, `FUN_00251350`): push the unit out of any
barrier with radius 2r, then up to five times: step length = clamp(maxSpeed
× 1.5, 1, 30 m) toward the target; if `FUN_00268c98` says the step
crosses a barrier, `FUN_00268060` gives the two corners, the side is
chosen ONCE per target (this+0x70: 1 left / 2 right, the nearer corner)
and the target becomes that corner; the resulting {target, pos} path is
handed to the mover. Stats `DAT_00518040` (clear steps) /
`DAT_00518044` (unchanged target).

**BF2, same shape [V].** Pools and grid in `FUN_002ee388` (PathNode
0x699528 × 1024, barrier 0x6994d8 × 512 block, grid `DAT_005fe8ec` =
`… loader `FUN_00178ae8` →
`FUN_002ee588`. Follower ctor `FUN_002eca30` (locates both ends),
advance `FUN_002ed2d0`, waypoint re-roll `FUN_002ed3d8` (100 draws of
`FUN_002ea128`, h = 4 for kind 3 via `FUN_002ed4b8`), reachability
`FUN_002ec220` (walks ≤ 100 hops). The agents `FUN_002c0220` /
`FUN_002bfce0` do the clear test `FUN_002eea68` (with an ignore list —
the unit's own vehicle barrier), push-out `FUN_002eefc8`, and the leg
request `FUN_002f3528` (= `FUN_0026c0d0`; searcher `FUN_002f3910` via
`FUN_002efe20` / `FUN_002f0538`; nearest-on-segment `FUN_002ef278`).
**Dynamic barriers**: `FUN_002ee6f0(hx, hz, …, &slot, entity, hash)`
creates a barrier from an entity's transform with half sizes = half the
class collision bounds' XZ extents (class+0x658/+0x65c) when the class
property with hash 0xc1f062c6 (bool, class+0x654, parser `FUN_001e4970`;
the class ctor `FUN_001e4608` names "destructablebuilding"; the name
did not brute-force — [H]) is set; FLAG 0x1f, enabled. Creators
`FUN_001e2d40` (the entity base ctor of that family), `FUN_00200828`,
`FUN_0021a5d0`; `FUN_002ed678` (from `FUN_00202d08` / `FUN_0021b470`)
re-places a moving one — disabled while |v|² ≥ 36 (6 m/s), re-inserted
(`FUN_002ee988`) when it moved > 0.5 m or turned (dot < 0.99);
`FUN_002ee8f8` removes it. So on BF2 buildings and vehicles block the
AI locally through the same barrier grid; BF1 has no dynamic barriers.

**Census with the engine's geometry** (`crossing_census.py`, r = 0.35 m
for a soldier — the collision-bounds radius above is class data the
census does not have; r = 0 gives the "data" count): base plans of the
34 maps (BF2: the last top-level chunk), SOLDIER arcs vs barriers whose
FLAG has bit 0: 702 of 4,245 arcs cross at r = 0 (BF1 220 of 2,002, BF2
482 of 2,243), 839 at r = 0.35; 89 hubs have their centre inside a
SOLDIER barrier, 68 of them escape the 100-draw re-roll; the PS2 planner re-implemented (band, depth 32,
corners at r + 0.5, 1024 nodes) resolves 1,150 of the 1,262 crossing
legs (both directions, hub centres as end points), with a mean path
overhead of ×1.0–1.5 over the straight leg; **BF1: 100 % resolved on
all 16 maps.** The 112 unresolved legs are all BF2, and 27 of the 57
arcs involved are exactly the maps' `plnp` arc groups (POL1
Connection110/264 = its two plnp arcs; COR1 10/10; TAN1 3/3; MUS1 7/9;
DEA1 4/5), the rest cross barriers whose names the mission scripts
toggle (`frog`, `coresh1`, `hallway_f`, `barracks`, `liea`, `cambar1`,
`shield_03`, `start_room_barrier`, `conquestbar` all occur in
MISSION.LVL) or authored walls with no gap inside the band (GEO1
barrier452–474, UTA1 barrier401/458, KAM1 barrier187–257). That is the
`plnp` mechanism's purpose read from the data: an arc that runs through
a door barrier is unwalkable while the door is closed, and
`BlockPlanningGraphArcs` re-routes the tables around it. **Dead hubs**
(inside a SOLDIER barrier with no escape, and SOLDIER-routable): 20 —
HOT1 Hub43/44/51/53/59/61 inside `conquestbar` (mode-specific), MYG1
Hub110–113 inside `coresh1`, YAV1 Hub20–28 inside `stoptanks`, MUS1
Hub44, TAT3 Hub91; a unit routed to one of these fails the leg on the
PS2 too (the planner refuses an unclear goal).

**Meaning for the port.** (1) Barriers must NOT be corridor walls or
graph costs: keep the tables as they are and treat a barrier as an
obstacle for the steering layer — a leg that crosses one is expected
and is walked around its silhouette corners; the port's "barrier
crossings are data errors" diagnostic is wrong and its Phase A.1
steering should be the corner walk above (step clipping + corner
re-aim per tick, and the small A* when the straight leg is blocked).
(2) Polarity "blocks these types", half extents, rows as stored, z
negated — the port already has all four. (3) BF2 doors: honour the
enabled bit and the mission Lua's Enable/DisableBarriers; the `plnp`
patches are what keep units from queueing at closed doors.

**BF2 arc `flags` [V].** Readers of the runtime arc's +0xc: `FUN_002eb918`
skips `flags & 7` and `FUN_002ebb48` skips `flags & 1` when locating; the
chooser `FUN_002eb000`, the random-other-arc `FUN_002ea380` and the
follower never read them, so nothing is directional — a bit-0 arc is
walked both ways like any other and is merely never the answer to "which
arc am I standing on". Bits 1 and 2 are consumed in the move agents
`FUN_002c0220` / `FUN_002bfce0` (follower+0x50 → arc → +0xc): when the
arc just chosen has any flag, the normal leg request is replaced by a
three-point AIPath {from, from + 0.1 m·dir with node flags `((flags & 2)
<< 4) | (flags & 4 ? 0x40 : 0)`, to} (`FUN_002f2840` / `FUN_002f2998`)
sent as controller command 9. The mover `FUN_002f1778` on a node with
`flags & 0x60` writes the controller word ctrl+0x440 |= (flag 0x20 ? 8 :
4) and ctrl+0x4f0 = normalize(dir_xz + (0,1,0)) × 0.7071 — a jump request
45° up toward the node — consumed by the soldier's jump handlers
`FUN_00246ed0` (action 4) / `FUN_002476a0`; node flag 8 there is a
different action (clear, `FUN_002f1740(9.0)`, AI event 0x14) and 0x80 a
third (ctrl+0x440 |= 1 with +0x430 = the direction). So disc flag 2 =
"jump this leg" and 4 = "jet-jump this leg" in the sense that both make
the follower jump at the leg's start; which of the two control bits is
the jet-pack burst is [H] (the PC tool's jump = 2, jet_jump = 4 is
consistent). Bit 0 = the PC "one_way" [H] has no directional effect in
the ELF. BF1 has no equivalent — its arc is 12 B — and its jet troopers
use the JetJumpHint objects of the last entry [V].

**Arrival and locate [V], read from the VU0 words.** Arrival is decided by
the mover, not the graph: BF1 `FUN_0026a780` (|unit − node|²_xz <
R², `VMUL.xz` at 0x26a83c) or `FUN_0026a878` (the unit has passed the
node's plane: dot(seg, unit − node)_xz > 0, both vectors y-zeroed;
EntityFlyer only within 10 m); R = `FUN_00234e78` at the path's LAST node
(EntitySoldier/EntityDroid 1.0 m; EntityHover class+0x1788 × 0.5; every
other kind 4.0 m) and `FUN_00234db0` between (EntityFlyer class+0x1784 ×
0.25; EntityHover +0x1788 × 0.5; else 1.0 m); BF2 `FUN_002f1140` /
`FUN_002f1320` are the same shape (`VMUL.xz` at 0x2f1208 / 0x2f1358). A
hub is "reached" when the two-point leg ending at its re-rolled
waypoint is finished, so the hub sphere never decides arrival. The hub
SPHERE is used only to locate: BF1 `FUN_00265d38` is 3-D (0x265d54–
0x265d60: `VMUL.xyz`, `VADDbc.x vf0.w`, `VADDAbc.x .y`, `VMADDbc.x .z` =
dx²+dy²+dz²), and so are BF1's corridor test `FUN_00265d90` and
nearest-hub `FUN_00265eb8`; BF2's hub containment `FUN_002ea878` is XZ
(`VMUL.xz` at 0x2ea8ac) while its corridor `FUN_002ea908` (XZ bounding
circle, then 3-D projection and 3-D perpendicular distance) and nearest
arc `FUN_002ebb48` / `FUN_002eaae8` are 3-D. The random waypoint
(`FUN_002657c8` / `FUN_002ea128`) is a 3-D direction, the mover ignores
its y. Net: the port's XZ arrival is the PS2's; its XZ locate is a
superset of BF2's (hub) and a deliberate departure from the 3-D hub test
that both games use for the arc corridor — the same-level refinement
Phase A left open is a real difference for BF1 (3-D hub) and for the
corridor step on both, but it matters only where hubs sit far from the
walkable floor (POL1 y = 0 under 13–33 m floors, DAG1's cave), where the
PS2 itself falls through to "nearest arc".

**Local obstacle avoidance [V].** There is no obstacle-circle system and
no feeler: obstacles ARE the barriers, and the only other input is
contact. (a) Corners: the walk step and the planner above. (b) Contact
push: the soldier's collision handler `FUN_00202f38` (droideka
`FUN_001ce828`) adds each contact normal with |n.y| < 0.3 against a
qualifying kind (`FUN_00235ec8`) into soldier+0x970 (droideka +0x2090);
the mover's steering `FUN_0026aba8` reads it once per tick, offsets its
target 5 m along the path perpendicular on the side the push points to
(remembered in +0x980 / +0x20a0 while the push lasts), zeroes it, and
issues the move order `FUN_0023b320(−1, 1, unit, target, dir, …)`.
(c) Look-ahead: `FUN_00234830(d, a, b, pos, &target)` targets the point
d + 0.2 m ahead of the unit's projection on the current path segment,
d = |velocity| for EntityHover and 20 m for EntityWalker; soldiers and
droidekas aim straight at the node; flyers use the 3-D `FUN_00233d20`.
(d) Stuck: `FUN_0026b450` with per-kind thresholds `FUN_0026b3b0`
(EntityDroideka 2 m / 6 s / 6 s, everyone else 2 m / 3 s / 5 s): mover
+0x6c = 2 when the unit has not moved 2 m from its anchor within T1,
= 1 when its distance to the node stopped improving for T2, read by the
agents through `FUN_0023b880`. `ai.showunitpathingradius`
(`DAT_00517d5c`) draws the `FUN_00235ad0` radius; `aimode`
(`DAT_00517cf4`) gates the mover's debug lines. The port's feeler-based
side-step is therefore not the PS2's shape: the PS2 never probes ahead
of the unit except with the barrier grid; walls the barriers do not
cover are handled only by the 5 m contact side-step and the stuck
timers.

**Squad commands, bounded [V unless marked].** BF1: the command issuer
`FUN_00260f38` (called from the controllable-entity classes 001d9180
001ea9f8 00205cc8 00219ed8 00223850 002279e8 002613f8) broadcasts to
the team table (`DAT_00518388`+0x38 units, stride 0xe8) within 45 m for
command types 0–1 and 30 m for the others (LOS and a count cap not
read [H]); a receiver stores it at ctrl+0x254/+0x258 (`FUN_002631e0`),
`FUN_00253118` acts on it, and the agent factory `FUN_00262bb8`
(ctrl+0x14) builds 9 → UnitHoldAgent `FUN_00248cb8` (hold point = the
unit's position) and 0xb → UnitFollowAgent `FUN_00248498`. Follow
(`FUN_00248828`): the leader is ctrl+0x26c; more than 20 m away
decrements a 1.0 timer by 0.05 per update → state 100 (released) at 0;
each update with a 1-in-9 chance re-picks the goal (`FUN_00248518`): own
position + 10 m along the stored leader direction ctrl+0x2a0 rotated by
a random ±30° (`FUN_00232420(−0.5236, 0.5236)`), or, without one, a
random 3-D offset of up to 10 m per axis. Hold (`FUN_00249038`): radius
10 m (EntityDroideka 2 m) around the hold point; beyond it with a clear
spot → released (state 100); otherwise, when not being pushed and the
mover is idle for 3 s, wander to a random point within ±10 m
(`FUN_00248da8`). `SetAIFormationRadius(a, b)` → `FUN_00260ba8` writes
`DAT_0058831c` / `DAT_00588320`: a is read by `FUN_0023b8a0` (every
0.25 s a random 2-D offset × a in the leader's frame appended to the
follower's mover path, 30-node cap), b by the fly agent (`FUN_002474e0`
/ `FUN_00247b18`). BF2: the issuer is the player's soldier update
`FUN_00240bf8`: the target is the entity under the reticle (argument 2+0x3f0,
[D]) if `FUN_002692b0(player, target) ≥ 0` (the team-relation matrix
`PTR_DAT_005fee78`+0x84: not hostile) and it has a controller; command
0xf ("follow me") goes to it through `FUN_002b7e58`; if refused and the
player has a free follower slot (`FUN_002e0338(player, …, 3)` = class
slots vtbl+0xa4 minus current followers `FUN_002e0180` — a count cap
[D]), 0x12 then 0x10 are sent, AI event 0x16 fires and the Lua event
`CharacterIssueAICommand` (table 0x720cc0) carries the number that
`TranslateAICommand` (`FUN_001926d8`) turns into "followchr" 0x10,
"stopfollowchr" 0x12, "waitforpickup" 4, "getintovehicle" 0x13,
"getoutofvehicle" 0x14. The dispatcher `FUN_002b70d0` (ctrl+0xa0) builds
8 → UnitHoldAgent `FUN_002cb1c8` and 0xf → UnitFollowAgent `FUN_002caa40`;
that agent's command handler `FUN_002ca4f0` accepts 0x10 once
(agent+0x288) and, on 0x12 from its own leader, fires AI event 0x19 after
0.5–1.2 s and releases; its update `FUN_002cadf8` is BF1's (20 m
give-up, 1-in-9 re-pick via `FUN_002caaf0`, and it follows the leader's
point + 1.6 m up). Whether a follow/hold is cleared by the leader's death
was not read [H].

**Open / PROVISIONAL**: the BF2 class-property name for the dynamic
barrier flag (hash 0xc1f062c6) and which of the two control bits is the
jet-pack; the soldier collision-bounds numbers behind `FUN_00235ad0`
(the census assumed r = 0.35 m); squad LOS / count caps on BF1 and death
handling on both; the meaning of PathRequest age ≥ 11 (`unit+0x28c`),
which sends even a clear leg to the planner.

### `plan` / `plnp` / `Hint` / `BARR` — the authored AI data every ground map ships, measured (2026-09-01, Fable session 8, AI-data RE)
Format + consumer measurement for the port's own AI (PLAN Log "AI decision"),
so a parser can be written from the SPEC at the end of this entry. Tools
(scratchpad `re_plan/`): `dump_ai_data.py <lvl> [--json out]` (every hub,
arc, hint, barrier, region and `plnp` of a map as JSON), `census_ai.py`
(all 34 ground maps, invariants + distributions; output `census_out.txt`),
`assess_routes.py` (CP-to-CP routing), `hash_hint.py` (PROP-key brute
force), `glb_terrain.py` (samples the baked terrain), `hexrec.py`. Every
number below is from the PS2 bytes or the two decomps; PC-tool readings
(swbf-unmunge `handle_planning.cpp`) were used only as hypotheses and are
marked [H].

**`plan` chunk — INFO** [V]. 6 B = `u16 hubCount, u16 arcCount, u16 5`.
The third word is 5 on all 55 chunks of both discs and BOTH loaders read
it and throw it away (BF1 `FUN_002663a8` reads three u16 via
`FUN_00442050`, keeps two; BF2 `FUN_002eb2b0` via `FUN_003cb318`, same).
It is the number of routed unit types, hard-coded 5 in both hub readers.

**NODE, BF1** (`FUN_00265b50`, called per hub by `FUN_002663a8`) [V]:
fixed record of `40 + 20·hubCount` B = `char name[16]` (read into a
scratch buffer at 0x5883f8 and DISCARDED — the engine keeps no hub names),
`f32 x, y, z, radius`, `u8 arcSlot[8]` (index into ARCS, 0xff = empty;
prefix-packed on all 5,282 hubs of both discs), `u32 table[5][hubCount]`
(type-major). 16/16 BF1 NODE payloads equal hubCount × that. Runtime hub
= 0x30 B: +0 position (16 B, w unused), +0x10 radius, +0x14 typeMask (OR
of its arcs' masks, computed after load), +0x18 the 8 slots, +0x20 → 5
table pointers.

**NODE, BF2** (`FUN_002ea620`) [V]: VARIABLE record = `char name[16]`
(discarded), `f32 x, y, z, radius`, `u8 arcSlot[8]`, `u8 rowCount[5]`,
then for type t = 0..4, `rowCount[t]` rows of `hubCount` bytes (row k =
the k-th alternative). Σ over hubs equals the NODE size on all 39 BF2
`plan` chunks (18 ground maps with 1–5 chunks each, SPA1–3, and BF2 KAM1's
EMPTY chunk @0x4DA88: hubCount 0, 32-B payload — a parser must accept it).
The reader keeps at most 4 row pointers per type (memset of 0x50 B at
hub+0x1c = 5 × 4 pointers); the discs' maximum is 4 (`rows_per_type`
census; 3,432 of 5,000 hubs have exactly one soldier row). 671 B on TAT2
is only the average — Hub1 starts at +845, Hub2 at +1490.

**Table entry** [V]: one byte `weight<<3 | slot` — `slot` (0..7) picks one
of the hub's 8 arc slots, `weight` 0..31; 0 = no route (and the self
entry). BF1 packs up to four entries in one u32 (low byte first,
0-terminated — `alts_per_entry` 1: 1,386,296, 2: 535, 3: 107, 4: 19); BF2
puts each alternative in its own row and a 0 in row k ends the list. The
weights of one entry sum to 31 (1,386,877 entries) or 30 (80). Chooser
BF1 `FUN_00266150` / BF2 `FUN_002eb000`, identical: `type = index of the
lowest set bit of the unit's mask` (`FUN_00266088` / `FUN_002eaf38`);
`dest = index of the destination hub`; if the unit arrived by an arc, find
its slot, `budget = 31 - weight(entry using that slot)`; if budget < 1 the
arrival arc is the only way and `FUN_002659e8` / `FUN_002ea380` picks a
random OTHER arc of the hub whose mask allows the type; else
`r = rand % budget` (the 0x19660d LCG, `DAT_0066eed0` / `DAT_0073d870`),
walk the entries subtracting weights, take the first with the running
budget ≤ 0 whose slot is not the arrival slot; the arc is
`arcs[hub.arcSlot[slot]]` and the next hub is its OTHER end (arcs are
walked both ways — `FUN_00267540`: `next = arc.a == cur ? arc.b : arc.a`).
An EMPTY entry gives no arc: the authored table IS the router, there is
no graph search anywhere in either ELF.

**ARCS** [V]: BF1 (`FUN_00265c80`) 22 B = `char name[16]` (discarded),
`u8 hubA, u8 hubB, u32 typeMask`; BF2 (`FUN_002ea7d0`) 26 B = the same +
`u32 flags` at +22. Every hub index < hubCount, every slot's arc touches
the hub, every hub name and arc name is unique and NUL-terminated within
16 B (55/55 chunks). Runtime arc = 0xc B (BF1) / 0x30 B (BF2: +0 hubA*,
+4 hubB*, +8 mask, +0xc flags, +0x10 centre and +0x20 bounding radius =
max(half extended length, rA, rB), computed at the end of `FUN_002eb2b0`).
The BF2 `flags` are READ: `FUN_002eb918` skips arcs with `flags & 7` and
`FUN_002ebb48` arcs with `flags & 1` when locating the arc a point stands
on — so bits 0..2 mark arcs a unit cannot be localised on; disc values 0
(4,755 arcs), 1 (15), 3 (30), 4 (17), 7 (1); the PC tool names them
`one_way = 1, jump = 2, jet_jump = 4` [H].

**The six type bits, from the ELF** [V]: the entity-class property
`AISizeType` (fnv1a 0xaff22806, printed sign-extended in the decomp) is
parsed by BF1 `FUN_00221e48` / BF2 `FUN_0026a340`: the value string is
compared against `"SOLDIER" "HOVER" "SMALL" "MEDIUM" "HUGE" "FLYER"`
(ELF data 0x533ec0 / 0x61fbd8, 8 B apart) giving mask 1, 2, 4, 8, 0x10,
0x20, default 1. The five tables cover bits 0–4; FLYER has no table
(flyers do not route). The unit's mask is class+0x78 (BF1, read by
`FUN_0024f160` → `FUN_00235be8`) / class+0x330 (BF2 `FUN_002a7830`, which
turns HOVER into SOLDIER when `FUN_002696f0` says so — undecoded, [D]:
a rider/ownership condition). Arc masks and BARR flags on the discs use
all six bits (0x3f on 988 arcs, 5,427 barriers); `ai.showtypemask` is the
BF1 debug switch. The 902 table entries that route a type over an arc
whose mask excludes it (888 on BF2 UTA1: SOLDIER/HOVER over mask-12 arcs)
are authoring noise — the chooser never checks the arc mask.

**Load-time derivations** [V]: hub.typeMask = OR of its arcs' masks
(BF1 hub+0x14, BF2 +0x10); BF2 graph+0x10 = OR of all arc masks and
`FUN_002ec9b8(mask)` hands out the planner only if some arc allows the
mask (every Lua binding and AI caller goes through it). "Where am I on
the graph" is `FUN_00266678` (BF1) / `FUN_002eb918` (BF2): the containing
hub with the SMALLEST radius (`FUN_00265d38`: |p − hub|² < r²), else the
arc whose corridor contains the point (`FUN_00265d90`: distance to the
segment < rA·(1−t) + rB·t — an arc is a corridor whose half-width
interpolates the end hubs' radii), else nearest hub (`FUN_00266840`).

**Correction (audit CONT, 2026-10-02) [V]:** the hub loop of both functions SKIPS a hub whose typeMask has no bit
for the unit's mask (`FUN_00266678` B1:185936; `FUN_002eb918`, the same test on hub+0x10), so a hub with no arcs
(mask 0) is never the hub of a unit or of a goal point; and `FUN_00266840` walks the ARC array (stride 0xc, mask at
+8) — the nearest allowed arc, not the nearest hub [D: read, not verifier-checked]. The port's
`NavGraph::locate_on` was built without the test (card WAI2 added it for vehicle types only): card HUBM,
`re-conquest-goal.md` section 10.

**BF1 consumes the graph at runtime** [V] (A3 §AI evidence said the
data was BF2-only): 'plan' handler `FUN_00266d28` (registered for
0x6e616c70 in `FUN_00175d00`) stores the graph in `DAT_00518218`;
`FUN_00266d98` (get graph) is used by `FUN_0024f3f0` and `FUN_0024f020`,
the AI move-to that builds a `ConnectivityGraphFollower`
(`FUN_00266e18` ctor, `FUN_00266e50` start→goal setup with the unit's
mask, `FUN_00267540` advance) and a `PathFollower` (`FUN_002686c0` /
`FUN_002689d8`); both are in `a4-bf1-reachable.txt` (dataptr). BF2:
`FUN_002ec900` → `DAT_005fe8b0`; `FUN_002ec9b8` is used by
Toggle/Block/UnblockPlanningGraphArcs (`FUN_00192220/1922a0/192320`) and
by AI code `FUN_002a9150, 002bc378, 002beb58, 002bfce0, 002c3fb0,
002c62a8, 002d84d8, 002e8318`. BF2 frees the previous graph on every
'plan' chunk (last loaded wins); BF1 does not (one chunk per file). BF2
files carry 1–5 chunks: top-level plus one per trailing `lvl_` mode
layer; KAM1 (3) and MUS1 (3) have several TOP-LEVEL ones.

**Hub y is the editor's pick height, not a floor** [V, measured with
`glb_terrain.py` against the baked terrains]: median (hub.y − terrain)
is 0.0 on 26 of 27 baked maps with 86–100 % of hubs within 2 m (the rest
sit on models: BF1 HOT1 +10.8, RHN2 −14.6); BF2 POL1 is the exception —
ALL 189 hubs have y = 0.0 while its floors are at 13–33 m and the terrain
at 22–49 m. DAG1's in-cave hubs are at y 3.8 = the terrain OVER the cave
(floor −2..−4); BES2's hubs are all at −56..−45 (the street plane) while
the skyway posts cp2/cp4/cp7 sit at −33. So the PS2's arrival test cannot
be the 3-D sphere the decomp's VU code suggests with radii of 1–6 m; the
follower is XZ-driven and y comes from the ground under the unit [D].
Positions are in the world frame as-is (nearest-instance fit: BES2 hubs
10.9 m as-is vs 22.5 m z-negated; CP↔hub matches on every map).

**Anomalies on the discs**: 14 hubs with no arcs (BF1 TAT1 Hub39/Hub40
at y −2921 and Hub132 r 525; YAV2 Hub9/Hub90; BF2 DAG1 Hub1, GEO1 Hub10,
KAM1 Hub147, MUS1 Hub3, TAT3 Hub50 — most with radius 0), 11 hubs of
radius 0, three non-empty self entries (BF2 TAT3 Hub38, UTA1 Hub8 ×2
layers). None breaks the loaders.

**Addendum 2026-10-03 (card HUBM, landed af03b6c; audit CONT's claims
D1-D4):** a hub with no arc for the mover is never located - the hub's
mask is the OR of its arcs' type masks (BF1 `FUN_002663a8`, BF2
`FUN_002eb2b0`) and locate skips a hub without the mover's bit (BF1
`FUN_00266678`, BF2 `FUN_002eb918`); the last resort is the nearest ARC
the mover may use (BF1 `FUN_00266840`, BF2 `FUN_002ebb48`; read by the
build session, not put to a verifier), not the nearest hub. The port's
`locate_on` does both since HUBM; before it TAT1's 525 m Hub132 (port hub
104) was the hub of two posts and of the open dune sea. Still open: the
PS2's corridor step takes the containing arc with the smallest `rA + rB`,
the port the first in arc order.

**Audit 2026-10-03 (HUBM: OK; verifier 29 SUPPORTED / 3 PARTLY / 0 NOT of
32; evidence `engine/artifacts/lead_tmp/audit-hubm/README.md`):** the mask
and the last resort are now read from the instructions of both ELFs and
held against RAM. In 346 library states every hub's stored mask is the OR
of its slot arcs' masks (34,094 hub rows; BF1 graph `*(0x518218)`, BF2
`*(0x5fe8b0)`); TAT1's Hub132 holds mask 0. Of 1,004 distinct live path
objects (the goal's locate result is kept at path+0x30, the destination
hub at +0xc, the goal arc's other end at +0x10) a transcription of
locate + last resort reproduces 1,004; without the hub mask 985 - the 19
that fail are goals inside a hub with no arc for the mover, all stored as
"nowhere" (before de-duplication 24: 10 soldiers', all at Hub132 in three
TAT1 states, and 14 MEDIUM vehicles' on 7 maps); everything in XZ 726; the
first containing corridor 996. So
"Arrival and locate" above stands (BF1 sphere and 3-D arcs, BF2 XZ disc and
3-D arcs), and its "matters only where hubs sit far from the floor" is too
weak: the port's XZ `hub_for` is the PS2's hub or an end of its arc at 849
of 921 live soldier goals (802 before HUBM). New [V]: the last resort
returns an ARC and the path request picks the end - by the route draw
(`FUN_00266150` from each end; BF2 `FUN_002eb000`), else the end NEARER the
goal for a goal (B1 0x26716c) and, for a start, the FARTHER end stored as
the path's current hub, the one `FUN_00267540` routes from (B1
0x267050-0x26706c); the stored destination is the farther end in 39 % of
live arc goals (276 of 709). BF2's last resort also skips arcs with flag
bit 0 (0x2ebbc4; one live object shows it), which the port does not. No
live goal lies in two eligible hubs (a first-containing-hub reading scores
the same 1,004), so "smallest radius" rests on the code alone. The class
default mask is 1 (B1:135877, B2:186481); one more constructor in each
game writes 0x20 (B1:89322 `FUN_001dead8`, B2:124423 `FUN_0020f098`, read
by the verifier as the flyer class's). Card LOC3.

**`plnp` — BF2's dynamic-planning patches** [V], the data behind
`BlockPlanningGraphArcs`: 20 chunks on 11 ground files (COR1, DEA1, FEL1
×2, HOT1, KAM1 ×2, KAS2, MUS1 ×3, MYG1, NAB2 ×4, POL1 ×2, TAN1, YAV1) and
SPA1/3/6/7/8/9, always right after the `plan` it patches (top-level or in
the same `lvl_`). Reader `FUN_002eb758` (dispatched for 0x706e6c70 in
`FUN_00177058` → `FUN_002ec988`): `u8 groupCount` (must be < 9), `u8
namesPerGroup[groupCount]`, `u8 nameCount`, `u32 nameHash[nameCount]`
(fnv1a of ARC NAMES, grouped consecutively — KAS2's four groups are the
arcs `seawall1`, `woodl`, `woodc`, `woodr`; POL1's one group is
`Connection110` + `Connection264`; names resolved against the arc-name
census and the mission Lua constants), then `1 << groupCount` records of
`u32 rawSize, u32 zSize, u8 zlib[zSize]`. Record index = the blocked-group
bitmask: `BlockPlanningGraphArcs(name…)` → `FUN_002ebcc0` (name hash →
group index) → `FUN_002ebe28` → `FUN_002ec5f8(mask & ((1 << groupCount) −
1))`, which XOR-restores the current patch (`FUN_002ec818`), inflates the
record (`FUN_002ec768`, zlib into the +0x34 buffer sized by the largest
rawSize) and XOR-applies it. Patch layout (`FUN_002ec818`): per hub `u8
has`; if set, for t = 0..4 `u8 rows` then `rows × hubCount` bytes XORed
onto that hub's row k of type t. Verified on all 20: records tile the
payload exactly, every record inflates to rawSize, every patch parses to
the byte, patch rows ≤ the base hub's rows, record 0 is hubCount zero
bytes. Lua may also pass 1-based group numbers. Pinned: KAS2.LVL plnp
@0x434A8 (5626 B): `04 | 01 01 01 01 | 04 | eb a5 db 6d 4c d1 ca ec 51 dc
ca f3 ee f3 ca 02 | 3b 00 00 00 0c 00 00 00 78 9c …` (0x6ddba5eb =
fnv1a("seawall1"); record 0 raw 59 = hubCount).

**`Hint`** (inside every `wrld`, base and mode layers) [V]: `INFO{TYPE
str, NAME str, XFRM 48 B}` + `PROP*{u32 fnv1a(key), str}`; the XFRM
translation is NOT z-negated (nearest-instance fit 7.1 vs 11.7 m on DAG1,
9.7 vs 23.9 on POL1). Factories: BF1 `FUN_001771d8` hashes the TYPE
string (`FUN_00444b88` = fnv1a: "0" = 0x350ca8af, "1" = 0x340ca71c, "2" =
0x370cabd5, "4" = 0x310ca263, "5" = 0x300ca0d0, "6" = 0x330ca589, "7" =
0x320ca3f6, "8" = 0x3d0cb547) and constructs, per the vtables read out of
SWBFRONT.ELF (each vtable's slot 1 is the class's `__tf` typeinfo getter,
e.g. 0x5361e8 → `FUN_00240ce8` "9SnipeHint"): **0 = SnipeHint, 1 =
PatrolHint, 2 = CoverHint, 4 = JetJumpHint, 5 = MinefieldHint, 6 =
LandHint**, kind bit = 1 << type (third ctor argument). BF2
`FUN_00178630` handles 0, 1, 2, 4, 5, 6, 7, 8 (ctors `FUN_002df120..270`,
0x30 apart, in that order); neither game handles 3 and no disc uses it;
7 and 8 exist only on BF2 (PC names Access = 3, Fortification = 7,
VehicleCover = 8 [H]). TYPE census — BF1: "2" 1110, "0" 264, "5" 101,
"4" 40, "6" 27, "1" 13; BF2: "2" 3657, "0" 260, "7" 219, "5" 116, "4" 49,
"8" 11, "6" 2. Names are `hintnodeN` (5,856) or empty (13). BF1 creates
hints only while `DAT_00587730 < DAT_0058772c` (a capacity), and for any
hint with Radius > 0 whose kind is not 0x10 (JetJump) `FUN_00177558`
spawns four helper hints on a ring of that radius at 90° steps and links
them (the `SnipeHelper` / `JetJumpHintHelper` classes) [D].
**PROP keys resolved** (brute force, then confirmed byte-for-byte by the
setters BF1 `FUN_0023f648` / BF2 `FUN_002defe0`, which switch on exactly
these hashes): `0xbf0f1cd1 PrimaryStance`, `0x92c05a59 SecondaryStance`,
`0xec6ee012 Mode`, `0x0dba4cb3 Radius`, `0x844ccdb4 CommandPost`. BF1
stores PrimaryStance as a byte at +0x7c (only if > 0; default 1),
SecondaryStance `v & 7` at +0x7d (if non-zero; default 1) and
`(v & 0x18) >> 3` at +0x7e (CoverHint picks one of those two bits at
random, `FUN_0023fb20`), Mode as an int at +0x78 with 3 folded to 0,
Radius as f32 at +0x8c (+0x84 = 1 when > 0), CommandPost as the CP object
looked up by name (`FUN_002a2010`). BF2 keeps PrimaryStance `v & 3` in
bits 8–9 and SecondaryStance `v & 3` / `(v & 0x18) << 9` in bits 10–13 of
a u64 at +0x50, resolves CommandPost (`FUN_002deec8`) and IGNORES Mode and
Radius (no case). Disc values: PrimaryStance 1–7 (BF1 SnipeHints use
4–7; BF2 1–3 plus a few 4/6 that the `& 3` masks); SecondaryStance = a
stance 0–3 plus the 8/16 flags (27, 1, 16, 24, 8, 11, 18, 19, 10, 9 …);
Mode 0 (5,439), 3 (331), 2 (89), 1 (10); Radius on 229 BF1 hints (Cover
/ Minefield / Snipe) and 54 BF2 hints (2.0, 3.0, 10.0, 21.818182 …);
CommandPost on 109 BF1 and 2 BF2 hints (Minefield/Land/Patrol, "CP2" …). Key sets:
(Primary, Secondary, Mode) 4,830 hints; (Mode) 499; the rest add Radius
/ CommandPost.

**`BARR`** (inside `wrld`) [V]: `INFO{NAME, XFRM 48 B, SIZE f32[3],
FLAG u32}` + PROP children that both games read and discard (BF1
`FUN_00177af8`, BF2 `FUN_00178d98`). Readers BF1 `FUN_00177828` / BF2
`FUN_00178ae8`: the three rotation rows are kept as stored, the
translation's **z is NEGATED on load** (*(decompiler detail omitted)*), and
the disc agrees — barriers fit the instances only with z flipped (DAG1
6.7 m vs 13.7 as-is, POL1 26.0 vs 36.3, BES2 34.5 vs 48.7). SIZE: the
reader also computes |SIZE|; the BF2 object (`FUN_002ed5a0`) keeps only
size.x, size.z and hypot(x, z) plus the XZ components of row 2 and the
translation — barriers are 2-D oriented rectangles in XZ [D]; whether SIZE
is half or full extents is open [H: half, like `regn`]. FLAG = the
AISizeType mask: BF1 stores it raw at +0x30 and `FUN_002682a8` answers
`(flags & unitMask) != 0` for the barrier queries `FUN_00268658 / 268780
/ 268c98 / 268ef0`; BF2 stores `flag & 0x1f | 0x20` at +0x50 (bit 5 =
enabled; Enable/Disable/ToggleBarriers → `FUN_001923a0` → `FUN_002f0650`
finds barriers by NAME hash → `FUN_002ee330` / `FUN_002ee300`), so BF2
DROPS the FLYER bit; its mask consumer was not located (PROVISIONAL by
analogy with BF1). FLAG census — BF1: 31 (3100), 63 (2793), 1 (677), 8
(156), 62 (59), 32 (16), 30, 33, 29, 24, 0 (8), 16, 55, 48, 61; BF2: 63
(2634), 3 (1249), 31 (1121), 9 (625), 11 (449), 15 (272), 8 (224), 1
(164), 7 (160), 28, 12, 16, 24, 27, 26, 62, 0 (8), 2, 60, 30, 51, 55, 10,
13, 41. Names are `barrierN` (13,861) or authored group names
(`frontdoor1-`, `warroomdoor`, `shield_`, `atte` …) that the Lua toggles.

**Routing assessments** (`assess_routes.py`, soldier type 0, table
route = follow the max-weight entry; "A" = arc-connected):
- **BF2 DAG1 → cave post cp4** (capture (16, −2, −50)): YES. Every CP's
  hub table-routes to Hub20 (13.1, 3.8, −48.2), 2.6 m in XZ from the
  capture centre, by two entrances: west `Hub31 > Hub16 (−10,4,−66, inside
  cutter 0 = the entrance pit) > Hub22 > Hub21 > Hub20` and north
  `Hub14 > Hub24 > Hub15 (−3,4,−27, inside cutter 1) > Hub23 > Hub21 >
  Hub20`. Hub22/21/20 lie inside cave cutter 3 at floor height (−2) but
  their authored y is the terrain (3.8) — the port must carry y from the
  collision ground, the graph gives XZ only.
- **BF2 POL1 → cp5con / cp6con**: YES in XZ. All conquest CPs route
  to cp6con's hub Hub97 (108.6, 0, −79.1) and cp5con's Hub199 (−98.2, 0,
  −134.6) in 12–46 arcs; cp6camp (Hub237) is the only isolated post. The
  cp6con chain enters through cutter 4's doorway (Hub63/Hub240 at
  (109,−2)/(95,−3), terrain 22.6) and then runs 6 hubs under 43–47 m of
  terrain to the post at 13.1; the cp5con chain enters through cutter 1
  (Hub159..Hub165) and runs 13 hubs under 32–49 m of terrain. Hub y is 0
  everywhere on this map.
- **BF1 BES2**: 28 of 30 CP pairs table-route; `cp3 → cp4` and `cp5 →
  cp4` have NO table route although the arcs connect them (authored
  gaps; the PS2 has no fallback search either). All hubs are on the −50
  street plane, including those under the skyway posts cp2/cp4/cp7 at −33
  whose capture boxes span −38..−26; the XZ chains contain the climbs
  (cp1 → cp2 zig-zags through (−104..−127, 121..158) — a ramp).
Net: the authored graphs route all three maps in XZ; what stalls the
placeholder AI is not a missing route but the missing "keep the collision
ground under the feet while following XZ" and, on BES2, two pairs that
need a real search over the arcs.

**Open / PROVISIONAL**: whether the arrival test is XZ or 3-D (derived
from POL1 and DAG1, not read out of the VU code); the meaning of Mode
and of SecondaryStance's two flag bits; BF2 arc-flag names; the BF2
barrier mask consumer; barrier SIZE half/full and rotation handedness
under the z flip; which `lvl_` mode layer owns which BF2 plan (the port
should take the mode layer's chunk when present, else the last
top-level one); how PrimaryStance 1–3 map onto stand/crouch/prone [H,
BF2 masks to two bits].

#### SPEC for the parser (all little-endian; offsets are into the chunk payload)
```
plan                       (any depth; BF2: also inside lvl_ mode layers)
  INFO  u16 hubCount; u16 arcCount; u16 typeCount (=5, ignore)
  NODE  hubCount records, in order:
        BF1 (fixed 40 + 20*hubCount):
          +0   char name[16]   NUL-terminated, "HubN"
          +16  f32 x, y, z     world frame as-is
          +28  f32 radius
          +32  u8  arcSlot[8]  index into ARCS; 0xff = empty (prefix-packed)
          +40  u32 table[5][hubCount]  type-major; each u32 = up to 4 packed
               entry bytes, low byte first, 0 ends
        BF2 (variable):
          +0..+39 as BF1
          +40  u8  rowCount[5]
          +45  for t in 0..5: rowCount[t] rows of hubCount bytes (row k =
               k-th alternative; a 0 byte in row k ends the list for that dest)
        entry byte b: weight = b >> 3 (0..31), slot = b & 7 (into arcSlot);
        b == 0 -> no route; weights of one dest sum to 31.
  ARCS  arcCount records of 22 (BF1) / 26 (BF2) bytes:
          +0   char name[16]; +16 u8 hubA; +17 u8 hubB; +18 u32 typeMask
          BF2 +22 u32 flags (bits 0..2 = not-walkable-on: one_way/jump/jet_jump [H])
  types: bit0 SOLDIER, bit1 HOVER, bit2 SMALL, bit3 MEDIUM, bit4 HUGE,
         bit5 FLYER (no table). table index = lowest set bit of the mask.
plnp  (BF2 only, follows its plan)
  u8 groupCount; u8 namesPerGroup[groupCount]; u8 nameCount;
  u32 arcNameHash[nameCount] (fnv1a lowercase of arc names, grouped);
  (1 << groupCount) records: u32 rawSize; u32 zSize; u8 zlib[zSize]
  inflated record = XOR patch: per hub u8 has; if has: 5 x (u8 rows;
  rows*hubCount bytes XORed onto that hub's row k of that type);
  record index = blocked-group bitmask (record 0 = all zero).
wrld/Hint   INFO{ TYPE str, NAME str, XFRM 12 f32 (3x3 rows + t) } PROP*
  TYPE "0" Snipe "1" Patrol "2" Cover "4" JetJump "5" Minefield "6" Land
       "7","8" BF2-only (Fortification, VehicleCover [H]); XFRM as-is.
  PROP u32 key + NUL str: bf0f1cd1 PrimaryStance, 92c05a59 SecondaryStance
       (stance = v & 7, flags = (v & 0x18) >> 3), ec6ee012 Mode (3 -> 0),
       0dba4cb3 Radius (f32 text), 844ccdb4 CommandPost (CP name)
wrld/BARR   INFO{ NAME str, XFRM 12 f32, SIZE 3 f32, FLAG u32 } PROP* (ignore)
  world position = (t.x, t.y, -t.z); rows as stored; FLAG = type mask
  (BF2 keeps bits 0..4 + its own enabled bit); 2-D XZ rectangle.
```
Parse: walk every chunk; `plan` at any depth (record its `lvl_` path);
attach a `plnp` to the plan that precedes it; Hint/BARR from every `wrld`.
Route(s → d, mask): t = lowest set bit (< 5); at hub h take
`table[t][d]` (BF1: unpack the u32; BF2: rows[k][d] until 0), pick by
weight (or the max), `arc = arcs[h.arcSlot[slot]]`, `h = other end`;
empty ⇒ unroutable (port: fall back to BFS over arcs whose mask has bit
t). Arrival: XZ distance ≤ radius [D]; y from the ground query.
**Pinned test vectors** — BF1 `TAT1.LVL`: plan @0x2D0F0 (size 272456),
INFO payload @0x2D100 = `73 00 97 00 05 00`; NODE payload @0x2D110: hub 0
"Hub39" (−1093.6193, −2921.0146, −699.7124) r 94.165627, slots all 0xff,
tables all zero; hub 2 @0x2E358 "Hub33" (−438.51852, 20.513742,
115.34089) r 8.0, slots [80, 93, 132, ff…], table[0][3] = 0x000000f8 →
(weight 31, slot 0), table[0] has 111 non-zero of 115, table[1] 63;
ARCS payload @0x6EC44: arc 0 = `43 6f 6e 6e 65 63 74 69 6f 6e 36 33 00 00
00 00 03 05 01 00 00 00` = "Connection63" 3→5 mask 1; arc 1 @0x6EC5A
"Connection64" 5→6 mask 1; first Hint @0x31F2C4 TYPE "2" NAME
"hintnode60" t (−283.95245, 16.553478, 164.5722) PROPs PrimaryStance "2",
SecondaryStance "9", Mode "0"; first BARR @0x323400 "barrier9" t
(−100.21072, 0, 251.00842) ⇒ world z −251.00842, rows [(−0.42418706, 0,
−0.90557456), (0, 1, 0), (−0.9055742, 0, 0.4241879)], SIZE (1.5746852,
0, 1.7452064), FLAG 31. BF2 `TAT2.LVL`: plan @0x14914 (140524), INFO
@0x14924 = `c8 00 f2 00 05 00`; NODE @0x14934: hub 0 "Hub0" (132.69946,
0.001129, 11.74349) r 5.7018428, slots [2, 3, 176, ff…], rows [2, 1, 1,
0, 0]; type-0 row 0 @0x14961 (199/200 non-zero, dest 1 = 0xf9 → weight
31 slot 1), type-0 row 1 @0x14A29 (only dest 23 = 0x49 → 9/1 and dest
149 = 0x7a → 15/2), type-1 row @0x14AF1, type-2 row @0x14BB9; hub 1
@0x14C81 "Hub1" (108.8884, 0.001125, 12.022558) r 5.705001, slots [3, 4,
229, 230, ff…], rows [1, 1, 1, 0, 0]; ARCS @0x35574: arc 0 = `43 6f 6e
6e 65 63 74 69 6f 6e 31 00 00 00 00 00 06 07 07 00 00 00 00 00 00 00` =
"Connection1" 6→7 mask 7 flags 0; arc 1 @0x3558E "Connection5" 9→8 mask
7; first Hint @0x31389C TYPE "2" "hintnode0" t (126.6715, 0.052501,
84.41707) PROPs "3", "10", "0"; first BARR @0x31EEC4 "barrier2" t
(86.981903, 0, −24.37025) ⇒ world z +24.37025, SIZE (8.0713425, 0,
8.6180706), FLAG 31. plnp: KAS2.LVL @0x434A8 as above; POL1.LVL @0x367AC
(1786 B): 1 group of 2 (Connection110, Connection264), records raw 189
and 72745 B.

### `coll` / `prim` / `CUTR` — the collision data every map ships, measured (2026-09-01, Fable session 7)
Format measurement over the raw discs, for the walkable-collision design
(PLAN Log "Walkable model collision", HOLD #13). Tools:
`scripts/ucfb_tree.py --tag coll --peek`, plus throwaway Python dumpers
(scratchpad; the numbers below are pinned in `mungefs::coll` / `mungefs::terrain`
tests). a2-bake-formats.md had the OUTER `coll` layout [V] and CUTS → CUTR
listed; this pins the inner encodings.

**`coll` chunk** (top-level in map/common/side LVLs): `NAME` = the GEOMETRY
name (`entc GeometryName` / gmod name, e.g. `kam_bldg_bunker1` — not a LOD
modl name), `NODE` = the source collision node (`bunker_export`, `SHELL-Bottom`,
`reflective_floor` …), `INFO` 40 B = `u32 vertexCount, u32 nodeCount, u32
leafCount, u32 leafIndexTotal, f32 min[3], f32 max[3]`, `POSI` = vertexCount
× f32[3] in MODEL space, `TREE` = a flat run of `NODE` (6 B split data,
unneeded) and `LEAF` chunks. **`LEAF` = `u8 n` + 6 unknown bytes + `u16
index[n]`** (size always 7 + 2n): one convex planar polygon in perimeter
order — triangulate as a fan. [V] by consistency on 48/48 chunks over BF1
KAM1, BF2 KAM1, BF1 BES1: vertexCount == POSI/12, nodeCount/leafCount == the
child counts, leafIndexTotal == Σ n, every index < vertexCount, and 0 of
5,000+ four-index leaves are zig-zag ordered (signed-area bow-tie test). The
six unknown bytes are NOT a plane normal (each byte triple as i8 agrees with
the geometric normal on ≤ 25 % of leaves) — left undecoded. Winding is not a
facing signal (closed platforms have as many down-facing polygons as
up-facing). `POSI` is in the render model's frame: the INFO bbox equals the
baked GLB's POSITION bbox on every model compared (kam_bldg_bunker1,
kam_bldg_tower1, kam_bldg_mushplatform1, bes1_bldg_platform_med,
kas2_bldg_pier, …; the two that differ — kas1_prop_bamboo2, kas2_bldg_hut_square
— are simplified collision hulls inside the render bbox).
Pinned: KAM1.LVL `kam_bldg_bunker1` @0x1BA228: INFO (223, 216, 217, 820),
bbox (−9.366828, −2.9212205, −53.44321)..(9.3652325, 17.206034, −19.511934),
v0 = (1.8961419, 0.88118327, −21.585752), first LEAF n=3 (29, 40, 25), 386
fan triangles.

**`prim` chunk** (one per model): `INFO` = model name + `u32 count`, then per
primitive `NAME`, `PRNT` (parent node), `XFRM` (48 B, 3×3 rows + translation,
the wrld/skel layout), `DATA` = `u32 kind, f32 a, b, c`. **kind 0 = sphere
(radius)**, **kind 2 = cylinder (radius, FULL height), axis = local Y**,
**kind 4 = box (HALF extents)**; kind 3 seen twice with zero dims
(BES1 `kam_prop_antiaircraft`), skipped. Conventions measured: `kam_bldg_tower2
p_collision` cyl (6.26, 64.0) at y 32.17 spans 0.17..64.17; `kam_prop_crate`
box half 1.27 at y 1.29 sits on the ground; **`kam_bldg_mushplatform1
p_collision_floor` = cyl (102.5, 8.0) at (0, −4, 0) IS the Kamino platform
floor** (its top is the platform's y = 0) — so primitives must be part of
the ground query, not only the walls. Pinned: KAM1.LVL prim @0xD7F18
`kam_prop_pillar2` / `p_collision1` / `pillar2` / t (15.647264, 2.1523004,
−10.661763) / box (0.38737866, 2.3135357, 0.37333333); @0xD9EF4
`kam_prop_pillar` / `p_antena` cyl (1.1, 20.0) t (14.143256, 9.689061,
−13.764727). PROVISIONAL (ours): XFRM is treated as model space (PRNT's own
transform assumed identity — every prim checked sits inside its model's
render bbox); and the layer mask is read from the NAME (`*vehicle*`,
`*flyer*`, `*ordnance*` do not block soldiers — `kas2_bldg_hut_round
p_vehicle_3` is a 6 × 20 m cylinder around a hut soldiers walk into). The
PS2's real collision layer masks stay HOLD FOR FABLE.

**Coverage census** (instances whose class has geometry, per baked map):
BF1 KAM1 118 with collision / 0 without; BES1 109/0; KAS2 126/0; BES2 334/0;
RHN1 361/0; BF2 KAM1 343/0; UTA1 176/0; DAG1 105/2 (vines); TAN1 102/1
(`tan1_stars`); COR1 62/12 (sky buildings, windows); MUS1 122/5 (fx, one
`mus1_bldg_world_3`); DEA1 119/1; TAT1 137/0; BF1 HOT1 136/7 (rafters,
cables). Every walkable structure on the eleven stalling maps ships
collision data; what lacks it is decoration.

**`tern → CUTS → CUTR`** (terrain cutters; re-terrain-heights.md: the height
query wrapper returns −300.0 for a cut point [V]): `INFO` 20 B = `u32
planeCount` + **f32 bounding sphere (r, cx, cy, cz)** — decoded later the
same day: DAG1 cutter 3 = (33.64, 8.26, −2.20, −49.30) is centred on the cave
post at (16, −2, −50); BF2 POL1 cutter 6 = (24.72, 109.01, 45.33, −51.04) is
an entrance shaft at the asteroid surface. Used as the cheap reject before the
planes; `CUTS` itself carries an `INFO` (u32 cutter count) before its `CUTR`
children. `PLNS` = planeCount × `f32[4]`
(unit normal + d; the ±Y caps carry |n| = 1.002). **Inside ⇔ every plane
gives n·p + d < 0 (strict), tested on the point's own y.** Pinned on BF2
DAG1: the cave post cp4's capture centre (16, −2, −50) is inside cutter 3
(6 planes) and no surface post is inside any cutter; the terrain-level point
(16, 4.7, −50) above the cave is NOT inside (max +2.90), so the terrain stays
solid over the cave and the entrance is where the terrain surface passes
below the cutter's top cap. Counts: DAG1 4 (18/18/18/6 planes), TAT1 3,
RHN1 5, BES2 2, MUS1 0.


**What the ground rule then had to learn (same day, from the round sweep).**
(1) BF2 Polis Massa's posts cp5con/cp6con sit 21 m and 34 m UNDER an uncut
heightfield and none of the map's seven cutters contains them — the cutters
are the entrance shafts. So the terrain is a ONE-SIDED sheet: a unit more
than `TERRAIN_BURY` (2 m, PROVISIONAL) below it is legitimately underground
and the terrain does not apply. (2) The same sheet rule for rays: the
heightfield line-of-sight and bolt ray hit only where a ray CROSSES the
surface outside a cutter — before that, 29 units standing inside BF1 TAT1's
cp2 building (a cutter, 2.7 m under the heightfield) could not see each
other and never fired. (3) BF1 RHN2's cp4 plaza is terraced: a 3 m lip
between two units 17 m apart genuinely blocks the eye-height chord
(sampled on the baked grid) — the units stand still inside the contested
post; that one is the placeholder AI's, not the world's. (4) A grounded unit
pushed over a platform edge by unit separation must keep its y and fall,
not follow the ground under its new footprint (BF2 COR1: 16 AI spawned on
one node pushed each other 15 → −300 in a single tick, "grounded", and no
fall could ever kill them).

### Sky domes: the elevation band each covers, and `DomeInfo.Ambient` (2026-08-31, Opus session 5)
Measurement over the shipped meshes, not a format find. Tool:
`cargo run -p mungefs --example sky_dome_census -- <map.lvl> ...`, which
measures each `DomeModel` mesh at the scale the engine draws it (first shell
normalised to `SKY_SHELL_RADIUS`, `Offset` applied). Extends
`analysis/re-sky-lighting.md` §6, which had measured exactly two domes.

**Below the horizon.** 29 of the 32 baked maps have a base dome that reaches
below 0 degrees of elevation; seven reach -88 or lower. Only **BF1 KAS1
(0.0..90.0), BF1 KAS2 (0.0..90.0) and BF2 UTA1 (0.3..90.0)** stop at the
horizon. **Five maps** ship a shell that is ENTIRELY below it (six shells,
counting BF1 NAB1's two — and NAB1's `sky_mountains` is 74 % below rather
than wholly) — BF1 BES1
`bes1_sky_dome_clouds_2` (-57.0..-2.7, 60/60 vertices), BF1 NAB1 `sky_water`
(-11.8..-1.6, 32/32) and `sky_mountains` (74 %), BF1 RHN2
`rhenvar2_skydome_clouds_2` (-19.3..-3.5, 38/38), BF2 COR1
`cor1_sky_buildings_1` (-15.4..-2.3, 34/34), BF2 GEO1 `geo_sky_explosions`.
So "what is below the horizon" is authored art on nearly every map; what is
NOT implemented there is the `LOWR` / `LowResTerrain` sheet that fills the gap
under a parallax shell's bottom rim (BF2 MUS1 is the clearest case and ships
no `LOWR` at all).

**`DomeInfo.Ambient` takes exactly two values, and both are neutral.** Over
all 34 ground maps: `255,255,255` on 23, `256,256,256` on 2 (BF1 KAS2, BF1
NAB1 — over the 0..255 range), `128,128,128` on 4 (BF1 and BF2 HOT1, BF1 and
BF2 GEO1), absent on 5. **Every authored value is a grey; not one map tints
its dome.** The two values are the two neutrals the PS2 pipelines use, the
same pair the LUMI neutral rule classifies per map (see below). Reading 128 as
a literal 0.5 multiplier — which `bake::lighting` did until now — halved the
sky on exactly Hoth and Geonosis, on both discs. `bake::lighting::dome_ambient`
now renders both neutrals as unity and applies anything else.

**BF1 BES1's terrain names textures from other maps.** Its `tern` carries
`LTEX("kam1_water")` and `DTEX("yav_decal")`, and neither appears in any chunk
BES1.LVL ships (`kam1_water` is in KAM1.LVL). Byte-verified in the chunk tree.
So BES1's cloud-sea sheet cannot be textured from the map's own data — by us
or by the PS2 — and the engine now declines to draw it rather than painting it
an invented colour. It is the only map on either disc whose terrain layer
texture does not resolve.

**Every map's `modl` segments spell texture names with the artist's case.**
4,240 model materials across the 32 baked maps resolve to a baked texture only
after case folding (BF1 RHN2 bakes `rhn_bldg_interior_02`, its walls ask for
`Rhn_bldg_interior_02`). Not a format finding — a note that `tex_ NAME` and a
`segm`'s texture reference are not the same string, and any lookup between
them must fold case. Cost before it was found: 3,752 of ~24,000 draws rendered
flat grey.

### `tern -> WATR` cracked — the water twelve maps ship (2026-08-31, Opus session 4)
A2 listed `WATR` and did not decode it; no map drew water. It decodes
completely. Full spec in the `mungefs::water` module header; ported
(`mungefs::water`, `bake::water_glb`) and rendered.

Present on 12 of the 34 ground maps: BF1 KAM1/KAS1/KAS2/NAB2/YAV1, BF2
DAG1/DEA1/FEL1/KAM1/KAS2/NAB2/YAV1.

- `INFO` (24 B) = `u32 grid`, `u32` (**2** on all 14 shipped chunks), `f32
  cellSize`, `f32 halfExtentX`, `f32 halfExtentZ`, `u32` (1 on twelve, 4 on
  both Kashyyyk Docks). `grid * cellSize == 2 * halfExtent` EXACTLY on all
  14, which is what identifies the last two floats as HALF-extents and the
  sheet as centred on the origin. **Correction to the 2026-08-31 handoff
  note: the third float is the CELL SIZE, not the water height.**
- `WMAP` = `grid²/2` bytes exactly on all 14, i.e. one NIBBLE per cell, and
  the nibble is a boolean — across all twelve maps its value is only ever
  **0, 1 or 9** (9 on BF1 NAB2, BF2 NAB2 and BF2 DEA1, 1–8 cells each, always
  on the shoreline ring; bit 3 is undecoded). Layout is row-major `[z][x]`,
  HIGH nibble first. Both orderings were MEASURED, by sampling the terrain
  height under each cell and asking whether it lies below the water plane:
  row-major separates wet from dry by **+63 points** against **+13**
  transposed, and high-nibble-first wins on 6 maps and loses on none. On the
  maps with a real body of water it is unambiguous — BF1 KAS1 98.7 % of wet
  cells over low ground against 0.0 % of dry, BF2 NAB2 97.8 % against 1.7 %,
  BF1 KAS2 89.8 % against 0.0 % — and plotted, the mask draws Kashyyyk's
  island, Theed's city platform, Felucia's river and Yavin's pond exactly.
- `LAYR` = `[cstring texture][u32][f32 height][f32 x2][f32 x2][u8 x4]`, 28 B
  after the name on all 14. **`height` is the world Y of the water plane and
  is PROVEN twice**: by that waterline separation, and by `CPAT`, every one
  of whose 165 shipped vertices sits at exactly that Y on all three maps that
  ship patches, although the patch box allows a 1 m range. The `u32` is 10 on
  all 14. The rest is PROVISIONAL: two f32 pairs read as UV velocity and UV
  tile, and four bytes read as a colour.
- `CPAT` is zero-length on 11 of 14; BF1 NAB2 ships 4 `LPTC`, BF2 NAB2 8, BF2
  DEA1 1. `LPTC` = `INFO` (6 f32 = an AABB as xmin,xmax,ymin,ymax,zmin,zmax),
  `STRP` (the standard PS2 strip format) and `POSI` (3 × u16 dequantised into
  that AABB — the same convention `segm` uses; reading it as offset-binary
  gives triangle areas over 100 % of the tile, which is how the error showed).
  Each patch is one 64 m or 32 m tile and its triangles cover **3–24 %** of
  it: the open water left after the buildings are cut out.
  **ERRATUM 2026-09-17 (queue WP, `7ef36a8`; the 2026-09-17 water-patch entry below):** that box is NOT the
  cell — it is the patch's own bit-3 cell padded by half a cell on EVERY side, so it is twice as wide, exact on
  all 13 shipped patches. Over the cell a patch actually replaces the triangles cover **13–96 %**; "3–24 %" is
  the same area measured against the twice-as-wide box.

Two findings that change what this is worth:
- **BF2 DEA1 and BF2 KAM1 carry a `tern` that is `NAME` + `WATR` and nothing
  else**, so the water plane is the ONLY ground those maps have. `WATR` must
  be parsed independently of the heightfield.
- **BF1 KAM1's terrain is a flat sheet at y = 0** on 65,025 of its 65,536
  vertices (the other 511 are one edge row at −20.2). Kamino's visible "sea"
  before this session was that sheet, not water; its `WATR` plane at −8 sits
  below it and shows only past the terrain's edge.

NOT established, HOLD FOR FABLE: whether a `CPAT` patch REPLACES its tile's
`WMAP` cells or adds to them (the bake takes "replaces", because the
alternative puts two surfaces at one height); what bit 3 of a `WMAP` nibble
means; and what `LAYR`'s colour word is — read as RGBA it turns Felucia's
teal river red-black, read as BGRA it turns Kamino's ocean red, and neither
byte order survives the shipped art, so the bake records it and applies it to
nothing.

**Ruled out: `sky_ WaterInfo` is not this data.** It appears on 8 LVLs, and
7 of those (BF1 BES1/BES2/RHN1/RHN2/TAT1/TAT1H/TEST) have no `WATR` at all;
every one of the 8 is the identical all-zero `Enabled(0)` authoring default
(`TopColor(0,0,0) BottomColor(0,0,0) TopRange(50) BottomRange(30)
BottomDrop(5) Enabled(0)`).

### Fog: what it is NOT, and where BF1 really keeps it (2026-08-31, Opus session 4)
Bounded investigation over decoded `sky_` data, the PC install's own `.lvl`
sky chunks and the community RE docs — **no decompiled C was read**. The
question was HOLD FOR FABLE "what enables fog on the PS2". It is still open,
but three candidates are now ruled out and there is a much better lead.

**Ruled out — `Enable` (0xaf8bb8ce) is not a fog switch.** It is **1 on all
18 PS2 BF2 maps and on all 29 PC maps**, without exception. It varies only on
BF1, where it is 1559.0 on eight maps (BES1, BES2, GEO1, KAS2, NAB1, NAB2,
TAT1, TAT2) and 1.0 on the other eight. A field that is constant across two
whole platforms cannot be what selects fog per map.

**Ruled out — `FogRamp` (0xfaaa0e17) carries no per-map information.** It is
exactly 3.0 on all 16 BF1 maps and absent from every BF2 and every PC map.

**Parked — `WorldFogRange` (0xd6c706f2)** exists on exactly two maps, BF2
DAG1 (−1.7, −5) and BF2 MUS1 (60, −155), byte-identical on PS2 and PC. Both
pairs are descending and mostly negative, so it is not a near/far distance
pair; a HEIGHT band (Dagobah's mist at ground level, Mustafar's glow below
the walkways) fits, but two maps is not enough to decode.

**On BF2 there is no separate enable: a map has fog iff it authors
`SkyInfo.FogColor` with `far > near`.** The set of BF2 maps authoring NO
`FogColor` is IDENTICAL on PS2 and PC — COR1, DEA1, NAB2, TAN1, TAT2, UTA1 —
which is what says the absence is authorship and not a munge artifact.

**The lead: BF1 keeps its real fog under `DomeInfo.LowResTerrain`, and BF2
promoted exactly those values into `SkyInfo`.** BF1 stores a SECOND fog —
`LowResTerrain.FogColor` (3 or 4 components), `FogNear`, `FogFar` — on 15 of
its 17 map LVLs. On every map that exists in both games with fog, BF1's
`LowResTerrain` triple and BF2's `SkyInfo` pair are the same authored data:

| map | BF1 `LowResTerrain` | BF2/PC `SkyInfo` | |
|---|---|---|---|
| HOT1 | 203,201,203 / 100 / 1000 | 203,201,203 / 100..1000 | **exact, colour + both distances** |
| END1 | 144,153,156 / 90 / 350 | 144,153,156 / 90..350 | **exact** |
| YAV1 | 144,153,156 / 90 / 350 | 144,153,156 / 90..350 | **exact** |
| KAM1 | 0,0,0 / 400 / 1000 | 0,0,0 / 400..1356 | colour + near exact |
| GEO1 | 142,82,38 / 300 / 700 | 0,0,0 / 0..0 | different map/era |
| KAS2 | 255,0,0 / 1000 / 2000 | 104,106,108 / 0..0 | different map/era |

And on the three maps session 3 found broken by `SkyInfo.FogColor`,
`LowResTerrain` gives the plausible colour where `SkyInfo` gives nonsense:
TAT1 `0,0,10` (black) → **164,174,194** pale desert haze; BES1 `255,0,0`
(red) → **82,45,25** gold-brown sunset; GEO1 `252,252,252` (white) →
**142,82,38** orange Geonosis.

**It was implemented as an experiment, rendered, viewed and REVERTED, and the
reason is the point.** BF1 Hoth gains the same blizzard haze BF2 Hoth has
from byte-identical numbers, Endor recedes into grey-green, Tatooine gets a
subtle pale-blue distance — and **BF1 KAS2 turns bright red**, because its
`LowResTerrain.FogColor` is `255, 0, 0`: the same placeholder value, in the
other field. So the better source still needs a placeholder filter, i.e. it
swaps one set of heuristics for another rather than producing a rule. The
three heuristics in `bake::lighting::fog_from` are UNCHANGED. Captures:
`engine/artifacts/fog_rule/` (local, gitignored).

Also observed, not decoded: the 4th component of `FogColor` looks like a
density — BF1 HOT1 255 (blizzard), TAT1 80 (light haze), KAM1 160, GEO1 128,
KAS1 30, NAB2 0 (and NAB2's range 3000..6000 on a 2 km map is off anyway).
Suggestive, not established.

**HOLD FOR FABLE, sharpened**: does the BF1 engine apply
`DomeInfo.LowResTerrain`'s fog to the WHOLE WORLD, or only to the low-res
terrain sheet drawn outside the level — and what filters the placeholder
colours (`255,0,0`, `252,252,252`, `0,0,10`, `128,128,128`) that both fog
fields carry? A preimage search over 176,378 generated names found nothing
for the other unknown `sky_` hashes (`0x23e10337`, a `SkyInfo` sub-block that
carries its OWN `FogRange` on BF2 KAS2 and MYG1; `0x3c84cf36`, three args
sitting between `NearSceneRange` and `FarSceneRange`; `0xb1bfdd49`,
`0xfef82e0b`, `0xd246dcd4`, `0xa99aeb4f`, `0xb4c88558`).

### Terrain `COLR` cracked — it is LUMI in colour (2026-08-31, Opus session 3)
A2 recorded the terrain's patch-level `COLR`/`ALPH` as "not plain RLE arrays —
tried elem 1/2/4, none decode to a grid size". **`COLR` decodes at element
size 2**, on all 26 shipped terrains. Full spec in the `mungefs::terrain`
module header; ported (`Terrain::colr`) and consumed by `bake::terrain_glb`.

- Decoded length is exactly `patches × 81 × 2` everywhere: 165,888 B on the
  32×32-patch terrains, 41,472 on the 16×16 ones, 10,368 on BF2 TAT2's 8×8.
  81 = 9×9 = a patch's 8 cells plus the shared edge, i.e. ONE u16 PER VERTEX.
- The u16 is GS `PSMCT16` (R5 G5 B5 A1): the top bit is set on **100.0 %** of
  all 82,944 entries of every map checked, which is an opaque alpha and not
  what the top bit of an RGB565 blue does. Confirmed by eye too — read as
  5551, TAT1's COLR draws the dune field and the sarlacc basin; read as 565 it
  is confetti.
- The patch index and the 9×9 sample index are both COLUMN-major. Proven by
  reconstructing LUMI from COLR's green channel: mean absolute difference
  **0.31 of 255 on BF1 END1, 3.3–4.2 on TAT1/KAM1/GEO1** — the 5-bit
  quantisation error and nothing more — against 8–28 for all three other
  orderings. `mean(COLR) × 8 ≈ mean(LUMI)` on all 26 maps as well.
- **So COLR and LUMI are the same lightmap**, LUMI the 8-bit intensity and
  COLR the 5-bit colour. The bake keeps LUMI's precision and takes only the
  HUE from COLR, as a ratio to COLR's own green channel, so a grey lightmap is
  a no-op. Six terrains actually carry a hue (≥ 2 five-bit steps of channel
  spread on ≥ 5 % of vertices): BF1 KAM1 and BES1 99.2 % (they share a
  lightmap), BF1 RHN1 99.0 %, BF2 NAB2 97.4 %, BF1 BES2 65.3 %, BF2 DAG1
  23.1 % — then a gap to BF1 GEO1 at 1.8 %. The other twenty bake identically.
- Not decoded: `ALPH` (variable length, does not fall out at any element
  size), and the per-patch `PTCH -> COLR/ALPH` overrides.

### Re-check — the LUMI neutral rule holds (2026-08-31, Opus session 3)
The maintainer asked for the PROVISIONAL "neutral is 128 where the 99th percentile stays
under 204, else 255, classified per map" rule to be re-checked. **It stands,
and there is now a decisive cross-check for it**: two terrains ship in BOTH
games and classify differently.
- `hoth_main_1`: BF1 HOT1's LUMI is 128 on 99.2 % of texels (class 128); BF2
  HOT1's is 255 on essentially all of them (class 255). Same map, same base
  texture (mean brightness 226 in both). Under the per-map rule both render at
  1.0. Under a UNIFORM "128 is 1.0" rule BF2's Hoth would be twice as bright
  as BF1's for identical art.
- `yav_main_1`: BF1 and BF2 YAV1 ship the *same* lightmap byte for byte (mean
  103, p50 102, p99 165, max 255) and classify the same way. Consistent.
- The threshold has room: p99 is ≤ 174 on every class-128 terrain and ≥ 230 on
  every class-255 one, so 204 sits in a 56-wide gap.
- Still NOT established, still HOLD FOR FABLE: what the PS2 itself multiplies
  here (ambient, sun term, or a fixed 2× in the terrain pipeline).

### CORRECTION — `LOWR` lives under `tern`'s `PCHS` (2026-08-31, Opus session 3)
The 2026-08-30 caution said a `tern`-children walk "reported neither LOWR nor
LPTC while grep finds them in the same files, so that iterator is dropping
children somewhere — do not trust it as a census". **`Chunk::children()` drops
nothing.** `LOWR` is a child of `tern`'s **`PCHS`**, one level below where the
census looked. Pinned as a test
(`mungefs` `tat1_lvl::lowr_lives_under_terns_pchs_not_under_tern`).

Measured shape, NOT parsed and NOT ported (still HOLD FOR FABLE — what the PS2
draws outside the level):
- `tern` → `PCHS` → `LOWR` → { `INFO` (20 B), `PCHS` → `LPTC` × grid² }.
- `INFO` = u32 grid, u32 resolution, f32 extent, f32 min y, f32 max y. The
  `LPTC` count is exactly grid² on all 17 maps that carry one (64, 100, 121,
  196, 256, 324), and the second u32 matches that map's
  `sky_ DomeInfo.LowResTerrain.PatchResolution` (TAT1: 5 and 5).
- Extent is 2048 m on 14 maps, 1024 on two, 512 on two — a fixed low-resolution
  sheet around the level, which is what `LowResTerrain`'s `MaxDistance`
  (400–3000 m) and its own fog imply.
- `LPTC` payloads are NOT fixed size (TAT1 404/480/540 B, HOT1 fifteen distinct
  sizes), so they are compressed or variable-length per patch. Undecoded.

**This does NOT close the outside-the-level defect, and the census says why:
17 of 34 maps have no `LOWR` at all — including BF1 END1 and BF1 YAV1, which
are two of the maps that SHOW the defect.** So whatever the forest maps draw
out there, on those two it is not `LOWR`.

Per-map: LOWR on BF1 BES2/GEO1/HOT1/KAS1/KAS2/NAB1/NAB2/TAT1/YAV2 and BF2
DAG1/END1/GEO1/HOT1/KAS2/NAB2/POL1; absent on BF1 BES1/END1/KAM1/RHN1/RHN2/
TAT2/YAV1 and BF2 COR1/DEA1/FEL1/KAM1/MUS1/MYG1/TAN1/TAT2/TAT3/UTA1/YAV1.

### Per-map sky, sun, ambient and fog (2026-08-31, Opus session 3) — DONE
Full write-up: **`analysis/re-sky-lighting.md`**. Headlines:
- Every map LVL on both discs carries one `sky_`; only BF2 carries `lght`.
  BF1 keeps its sun and ambient in `sky_ SunInfo`/`SkyInfo`; BF2 moved them to
  `lght GlobalLights` + the `Light` block it names. BF2 authors NO
  `AmbientColor` on any of its 18 maps; BF1 authors one on all 16.
- 122 of 123 `sky_` property hashes and 23 of 23 `lght` hashes recovered as
  CONFIRMED PREIMAGES (candidates from shipped `.sky`/`.LGT` sources, the
  swbf-unmunge hash table, both PS2 ELFs and the PC exe). A hash match alone
  is only evidence: `Type` and `StaticReflectionName` genuinely collide at
  0x5127f14d.
- **ERRATUM to the config format:** a string argument's slot is
  `4 * argc + byte offset into the trailing blob`, not the constant 4. The old
  reading dropped every string after the first. Fixed in `mungefs::config`
  with a byte-verified regression on BF2 MUS1's `Effect(...)`.
- Two conventions cross-checked against each other on BF2 GEO1, the one map
  that stores its sun twice: the `lght` quaternion is (w, x, y, z) with
  forward = rotated −Z, and `SunInfo.Angle` is (elevation + 90, −azimuth).
  The two directions agree to 2.8°; no other of the 24 order/axis
  combinations lands within 20°. Executable test.
- `DomeInfo` names sky-dome models that are ALREADY BAKED in each map, and
  `DomeInfo.LowResTerrain` names the texture, max distance and fog of what the
  PS2 draws outside the level — the open question in the terrain defect.
- The reduction to one sun + hemisphere ambient + fog (`bake::lighting`) is
  OURS and PROVISIONAL, including the colour-space choice (authored 0..255 are
  gamma and are stored linearised, same rule the `_dtl` GPU test proved), the
  three heuristics deciding which maps get fog, and an ambient floor.
- HOLD FOR FABLE: what actually enables fog; what the `SunInfo` back light
  (`BackAngle`/`BackColor`/`BackDegree`, on all 16 BF1 maps) does.

### Blaster fire + damage (2026-08-30, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-blaster-damage.md (fire cycle, spread formula,
  damage chain, full ODF key/hash/offset tables, worked rifle examples both
  games, 8 test vectors, 9 open questions).
- Fire cycle (BF1 FUN_00288d10 / BF2 FUN_0032e8c0): dt-driven remainder-
  carrying accumulator (drift-free cadence, NOT fixed-tick); gates = refire ≥
  ShotDelay, fire predicate (auto/TriggerSingle/charge-release), ammo,
  not-reloading, not-overheated. BF2 made Shot/SalvoDelay min/max pairs drawn
  uniformly per trigger (inline LCG 0x19660D).
- Ordnance: bolts are simulated projectiles (Velocity×dir, range =
  Velocity×LifeSpan) with ONE predictive raycast at spawn converted to
  time-to-impact — no per-tick collision; no gravity for bolt/laser/bullet
  (only `shell` parses Gravity, ×−9.8).
- Spread: signed radian state, init −SpreadThreshold (humans), +SpreadPerShot
  per shot capped at SpreadLimit, −SpreadRecoverRate·dt; effective half-angle
  = max(state,0) + posture-lerped Still/Move base; triangular offset via 2
  LCG draws/axis.
- NEGATIVE RESULT, lead-verified independently: soldier AimFactorPosture*/
  AimFactorMove/Strafe/AimValue are consumed NOWHERE in either ELF — hash
  grep with correct constant forms: all six = 0 hits in both decomps while
  positive controls ShotDelay (−0x26CA96F8) and MaxDamage (0x47D0E794) hit
  both. PS2 accuracy is 100% weapon-side; the entc props are dead data.
  (Matters for the P4 weapon rebalance: no posture accuracy system to port —
  if openbf wants stance-dependent spread it is a NEW feature, not fidelity.)
- Damage: descriptor from ordnance class (MaxDamage + 8 HealthType scales +
  ShieldScale, charge-lerped); receiver indexes scale by victim HealthType —
  NO bodypart/locational damage. BF1 flat (rifle 60 dmg, 80 m/s, 120 m);
  BF2 adds time-based falloff (rifle 80→50 over 0.4 s+0.2 s fade ≈ 64→96 m,
  160 m/s) + ActivateBonus +25% team hook.
- Corrects re-aim-assist §5: lead-solver speed/gravity vcalls land on the
  weapon instance (aimer+0x148) which forwards to ordnance class getters.
- LEAD CORRECTIONS while porting (2026-08-30, bodies read in full: BF1
  FUN_00288d10/FUN_00288c58/FUN_002891c0/FUN_00289150 + weapon update fn
  @decomp line 208432, select/deselect FUN_00284148/FUN_002841f0; BF2
  FUN_0032e8c0/FUN_0032e758/FUN_002a3e48):
  (a) the FIRST salvo comes from the trigger-init handler (vtbl+0x74:
  accum=0, refire=ShotDelay [BF2 uniform], salvoCounter=SalvoCount,
  salvoTimer=InitialSalvoDelay), called by the weapon state machine
  (state weapon+0xcc: idle 0 → active && fire-predicate → firing); the
  refire branch of the cycle REFUSES TriggerSingle weapons — that is how
  single-shot weapons get exactly one salvo per fresh press, rate-limited
  by the refire accumulator; FUN_002891c0 is the "cycle finished → idle"
  query (salvo done && accum ≥ period && (single || !pred || reloading
  || blocked)), not a fire gate.
  (b) weapon+0xc4 (BF2 flag bit 34 of +0xb0) is the weapon ACTIVE/drawn
  flag (select sets 1, deselect clears), NOT the fire button: releasing
  the trigger does not cancel a salvo the refire branch already started —
  it fires at the next update; only holstering cancels.
  (c) BF2 does not re-roll the refire period at salvo start (FUN_0032edf8
  is called from one other site, not the cycle); the roll happens in the
  trigger init.
  (d) BF1 heat model (weapon update fn): overheated latches when heat ≥
  1.0, `heat −= dt×HeatRecoverRate` (floor 0), clears when heat <
  HeatThreshold; FUN_00289150 returns false while overheated. Closes
  report open item §10.2 for BF1 (BF2 component FUN_001e2010 still open).
  (e) Sampled-at-fire spread plateaus at SpreadLimit − RecoverRate×
  ShotDelay (growth is clamped after the shot, recovery precedes the next).
  Ported: engine/crates/core/src/{lcg.rs,weapon.rs} (GameLcg exact float
  packing verified against FUN_002a3e48; all §9 vectors as Tier-1 tests).

### Command-post capture / regions / spawn eligibility (2026-08-30, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-cp-capture.md (function tables both games, struct
  layouts, ODF key/hash/offset/default tables, algorithm, region math, 12
  reference vectors). Lead-verified directly in the decomp: BF2 CP tick
  FUN_00357440 `×0.5` presence-weight loops (lines 444/915/1018 of the body),
  capture-meter drain-then-flip on +0xa4/+0x7c, strict `T < meter` capture
  (l.503), `N <= meter` neutralize (l.1197) with `+0xa4 = 0` and charging tag
  handoff (l.1239–1244), owner-inside recovery decay (l.1000–1024); BF1
  point-in-region FUN_004c2b08 sphere `|d|² < w²` / box strict half-extents
  along the three rows / cylinder axis = row1.
- Machine: two meters per post — neutralize (owned→neutral, NeutralizeTime)
  and capture (neutral→owned, CaptureTime); `meter += w(n)·dt` with
  `w(n) = 2 − 2^(1−n)` over the charging side's counted units; a different
  lone team drains the previous charge first; contested (two affiliations)
  freezes a neutral post; defenders never slow neutralization; empty posts
  hold (capture meter both games; neutralize meter BF2 holds / BF1 resets);
  BF2 recovery decays at w(owner) only with owner units inside.
- Who counts: alive soldier component + `CapturePosts` class flag
  (0x325F7C20; 1 for soldier/walkerdroid classlabels, 0 for vehicles and BF1
  heroes), team affiliation ≠ 0, tested as a POINT at the entity origin.
- Regions: rows normalized, SIZE scaled by row lengths; sphere radius =
  |SIZE| (TAT1 authors (0.1, r, 0.1)); box half-extents; cylinder r =
  √(sx²+sz²), half-height sy; all strict. TAT1: cp2capture r 8.1499,
  cp10capture r 21.018/h 10.
- Spawning: owner == team && (SpawnPath or class SpawnPoints), or friendly
  owner + AllyPath; meters ignored; node = uniform LCG draw
  `((r1>>16)|(r2&0xFFFF0000)) % n` + ground snap.
- Shipped class values (both games): controlzone Capture 10 / Neutralize 12
  / HoloTurnOn 2.2; major 16 / 36 / 2.2 (ctor defaults 10 / 6 / 2).
- Corrections: my ticket's 10 "uncracked" CP prop hashes were decimal→hex
  transcription errors — they are Value_ATK_/Value_DEF_{Alliance,Empire,
  Republic,CIS,Locals} (AI planner weights only); BF2 FUN_00355648 is the
  destructible-CP killed notifier, not the capture handler.
- Ported: core::region (from_rows scaling + strict tests), core::cp (BF2
  machine; openbf DIVERGENCE: the neutralize meter is zeroed on capture —
  BF2 leaves it at N so an immediate counter-attack neutralizes faster; BF1's
  `== N` guard shows the intent; flagged PCSX2 item), core::lcg::index,
  Sim counting at the origin with capture_posts. Tier-1: §8 vectors 1–8 +
  10 as tests (core::cp, core::region).
- Open (report §9): BF2 leftover-meter gameplay confirmation; VO entry-2 key
  names; KillRegion/TurretPath/AISpawnWeight consumers; BF2 FUN_003e4a30
  early-out; BF1 default-CP metric FUN_002a20c0.

### P4 combat v0 implementation notes (2026-08-30, lead) — what is authentic vs provisional
- Authentic (ported from verified RE): fire cycle/spread/heat/damage
  (core::weapon), single predictive-raycast bolts (core::projectile), death
  consumes the ticket + AI admission gate `alive < tickets` (core::battle),
  BF2 CP-majority bleed 1/3 t/s clamped at alive + 20 s domination
  (core::cp), respawn timer 4.0 s, BF2 alive-AI cap 16 (NumBots), BF2
  `OnTicketCountChange: count <= 0 → MissionDefeat` = immediate defeat at 0
  tickets (Sim.outcome; `Battle::defeated` keeps the BF1 last-stand form for
  reference).
- (CP capture rule + region SIZE semantics: PROVISIONAL at first, replaced
  the same day by the verified machine — see §Command-post capture above.)
- PROVISIONAL (flagged in code): soldier collision
  radius 0.5 m + muzzle at 0.75×capsule; friendly-fire scale default 1.0
  (DAT_00517508/DAT_005fcbe0 defaults untraced); firing interrupts sprint
  (BF2 feel, untraced); spawn node = SimRng pick among owned posts' path
  nodes (no BF1 wave scheme 10 s/0.25 s yet); reload = auto on empty, full
  clip refill after ReloadTime (state machine untraced, report §10.1).
- Placeholder (NOT fidelity, PLAN deferred item #1): core::ai — nearest
  non-friendly post + engage nearest visible enemy, bounded 2.5° aim wobble.
- Tier-4: engine/artifacts/p4_combat/ (firefight video, TTK traces matching
  §8/§9: BF1 5×60 → 1.03 s @20 m; BF2 5×80 → 0.90 s @20 m, 7×65 @80 m,
  8×50 @100 m).

### Tickets / reinforcements / spawn reserve (2026-08-30, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-tickets-spawn.md (struct layouts, function tables,
  mission constants, 12 reference vectors, 7 open questions).
- THE key answer: DEATH consumes the ticket in both games; spawn consumes
  nothing. BF1 FUN_00297440 (lead-verified in decomp: team+0x2c alive −1,
  team+0x10 tickets −1 floored at 0); BF2 does it in Lua (Objective.lua
  OnCharacterDeath → AddReinforcements(team,−1); agent disassembled the PS2
  Lua 5.0 bytecode — note this build uses RK-base 128, not 250).
- NO-OVERSPAWN EXISTS IN BOTH ENGINES: AI (re)spawn blocked while tickets ≤
  alive (BF1 FUN_00295c40 — lead-verified `+0x10 <= +0x2c` comparison; BF2
  FUN_0034cea8 team+0x24 vs +0x38). Hero-flagged classes exempt. Our
  core::battle model is thus authentic; the openbf addition is only
  player-displaces-AI. BF1 extras: exhausted-reserve culling (flagged units
  instantly; 900 s then every-15 s anti-stalemate killer), ally-exhausted
  locals-ticket zeroing.
- BF2 bleed (shipping baseline, Lua ObjectiveConquest.lua; PS2 retail SP
  runs the multiplayerRules path): enemy strict CP-majority → −1 ticket/3 s
  (rate 1/3), clamped ≥ alive count; SetBleedRate (team+0x2c) is HUD/VO
  only; defeatTimerSeconds 20 (zero-CP domination). Constants added to
  core::battle (BF2_BLEED_TICKETS_PER_SEC, BF2_DOMINATION_DEFEAT_SECS).
- SURPRISE: BF1 engine bleed is provably INERT with retail data — the
  threshold scan (raw MIPS at 0x2ada14) picks the FIRST entry ≥ own CP
  value from the array as stored, and every retail mission's table order
  makes that rate 0. BF1 PS2 conquest tickets fall only on deaths + the
  20 s zero-CP wipe. (Authoring intent = tightest-match scan is a flagged
  hypothesis; PCSX2 test queued.)
- Correction to PLAN lore: BF1 PS2 unit counts are 25–32/team; BF2's "16"
  is the NumBots alive-AI default (DAT_006004fc) over 20-member pools.
  PLAN §AI updated.

### Animation banks + pinned clip durations (2026-08-30, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-anim-banks.md. zaa_ = NAME + BIN_ (SMNA header,
  version 8 + u16 animCount → MINA entries + TNJA/TADA). MINA entry = 16 B:
  {u64 boneFlagMask, u32 nameCrc, u16 frames, u16 bones}.
- Clip-name hash is NOT FNV: engine CRC-32, MSB-first poly 0x04C11DB7,
  init −1, final ~, over the lowercased name (BF1 FUN_00444bb8, table
  0x521ca8). BF1 special ids resolve via sprintf "human_rifle_%s".
- Lead-verified independently: my own MSB CRC-32 reproduces
  crc("human_rifle_diveforward") = 0xDC2308EB, and raw bytes at BF1
  COMMON/ENG/COMMON.LVL@0x20113C and BF2 INGAME.LVL@0x1ACEEC both read
  {crc DC2308EB, frames 35, bones 26}.
- PINNED durations: diveforward 35 fr → BF2 roll 0.75×34/30 = 0.85 s (BF1
  0.875 s); stand/crouch_getdown_prone 40 fr = 1.3333 s; prone_getup_stand
  45 fr = 1.5 s; prone_getup_crouch 44; stand_getupfront 58 / getupback 53;
  landsoft/landhard BF1 17/34, BF2 23/36. BF2 lacks the prone clips (cut) —
  openbf prone uses BF1's timings.
- Engine updated: SoldierConstants roll_duration = 0.85 (no longer
  provisional); prone transitions now carry BF1 hard locks (movement+turn
  frozen, 1.3333 s down / 1.5 s up) — Tier-1 tested.
- Erratum: re-stance-roll.md §4.1 lists BF2 stand_getupfront as id 0x11;
  the anim agent traced it to 0x14. Open: BF2 anim-set-0 → human_rifle
  binding inferred (not traced through FUN_00282f78); human_tool_diveforward
  = 49 fr exists if a unit binds the tool class.

### Stance machines + rolls, both games (2026-08-30, P3, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-stance-roll.md (state enums, function maps, ODF
  key/hash/default tables, BF1-vs-BF2 roll diff, 7 reference vectors).
- BF1: stance int soldier+0xbe4, update FUN_00205cc8, SetStance FUN_00209758;
  stance factors HARDCODED at integrator call sites (crouch 0.7/0.6, prone
  0.3/0.2 turn 0.5, pitch clamp ±0.4); accel = Acceleraton(!sic ODF key)
  ×1.2, brake ×0.6, gravity −18. Roll = 2 states (0xa R/0xb L), trigger
  jump+stick≥0.9 strafe-dominant, forced strafe ±1.7 through the ground
  integrator (→ MaxStrafeSpeed×1.7), fwd×0.1 turn×0.6, exit 0.75×
  dur(diveforward) to PREVIOUS stance, no fire during, no i-frames.
- BF2: stance int +0x7e4, update FUN_00240bf8. **BF2 already implements the
  locked contextual button**: crouch button + FUN_00247b88 predicate
  (fwd²+strafe²≥0.81, not straight-back ±30° cone) → roll; stationary →
  stance cycle. Stance factors moved to ControlSpeed ODF (same defaults);
  sprint = state 3 (±30° start/±60° keep hysteresis, ramp over
  SprintAccelerateTime, energy drain); roll = ONE state (5), 10 energy,
  one-shot velocity impulse to RollSpeedFactor(1.7)×MaxSpeed when speed ∈
  (0.1, target), near-zero control during, exit ALWAYS to stand + 0.3s soft
  lock. Prone action is a return-0 stub (engine support intact otherwise).
  Capsule heights DAT_0061ed50 = {1.9, 1.6, 0.55}.
- Lead-verified spot checks (decomp bodies read): FUN_00247b88 constants
  0.81/1.732 exact; FUN_002471b0 impulse formula + RollSpeedFactor≤0 disable;
  stand-case contextual dispatch + ControlSpeed offsets (+0x77c/+0x79c/
  +0x7bc); BF1 call sites 0.7/0.6, 0.3/0.2/0.5, roll 0xbfd9999a (−1.7)
  fwd×0.1 turn×0.6 brake 1.0, exit 0.75×dur(0x14) to prev stance. All match.
- Open (report §6): BF1 pad mapping of the two command paths, diveforward
  frame counts (LVL anim data — needed to pin roll duration), BF1 jump-vy key
  name (hash 0xB421860C), BF2 fire-block during roll, 1st-person view-anim
  data source.
- openbf decision (P3): adopt BF2's scheme + BF1's prone restored — BF2
  predicate for the roll branch, stand→crouch→prone cycle stationary, BF2
  impulse roll (all-direction; BF1's strafe-ramp roll kept in data comments
  for reference), BF2 sprint/energy. Recorded in PLAN.

### BF1 aim-assist magnetism (2026-08-30, P3, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-aim-assist.md. Core = BF1 FUN_0025fea8 (called from
  controller update FUN_0025e098), operating on STICK VALUES pre-rotation.
  Three effects when the weapon has a magnet target (weapon+0x128, acquired
  in FUN_00223850 by ray of length LockOnRange, mask 0x5e):
  1. sticky slowdown ×0.5 on both aim axes (UNGATED — even Auto Aim off/MP);
  2. yaw velocity-tracking `(|move|·(1−|yaw|)·0.75·latRelVel)/(fwdDist·R)`
     clamped ±1 (ungated, rate-matched, dt-free);
  3. fire magnetism: fire held → 1.0s timer; pull = (1−|yaw|)·t·0.75 ×
     linear range falloff; yaw −= pull·clamp(latAngle/(R·dt), ±1) toward the
     BALLISTIC LEAD point (FUN_0026d3f0); only this term gated (SP: profile
     +0x34 Auto Aim; MP: host DAT_00518bdc "aimassist").
- Pitch is never pulled, only slowed. LockOnRange (weaponClass+0xd4, ODF hash
  0x7F012F70) ≤0 disables everything for that weapon.
- Sign resolution (was open in report §7): f2/yaw-stick is POSITIVE-LEFT —
  forced by term-1 physics (strafing +lat makes the target drift -lat; the
  decomp ADDS the positive term). core::aim_assist documents it; callers
  with positive-right sticks negate in/out (game::walk). PCSX2 confirms later.
- Lead-verified by reading FUN_0025fea8 in full: constants 0.5/0.75/1.0,
  clamp structure, gating chain, falloff formula, timer semantics all
  confirmed against the report. Open (flagged in report §8): axis identity,
  turn-rate vcall = MaxTurnSpeed?, mask 0x5e layers, zoom divisor — PCSX2
  trace items. 6 hand-computed reference vectors in report §7 → Tier-1 tests.
- WIDESCREEN flag (the maintainer 2026-08-30, port is 16:9-native hor+): verify the
  assist/reticle chain carries no 4:3 screen-space assumptions. The core math
  (FUN_0025fea8) is stick/world-angle based — aspect-independent — but check:
  the reticle placement ray in FUN_00223850 (screen-center projection?), the
  LockOffAngle HUD ellipse (FUN_002d4b50, screen-space size), and any
  aspect-baked constants in the zoom path before porting HUD-side pieces.

### Terrain height interpolation (2026-08-30, P3 prep) — DONE, verified
- BF1 FUN_00473710 (world query) read in full: NOT bilinear — per-cell
  TRIANGLE interpolation with the split diagonal ALTERNATING by z-row parity
  (iz&1). Even row: split test xfrac<zfrac (triangles (ix,iz)(ix,iz+1)(ix+1,iz+1)
  base (ix,iz+1) / (ix,iz)(ix+1,iz)(ix+1,iz+1) base (ix+1,iz)); odd row:
  xfrac<1-zfrac (base (ix,iz) / base (ix+1,iz+1)). Missing-height sentinel
  −1000 when the height store is absent (DAT_005229a8==0).
- Globals: DAT_00564698/9c world→grid offsets, DAT_00564690 gridSize,
  DAT_00564694 unit, DAT_005647bc 1/unit, DAT_005647c0 heightScale.
- Ported: core/src/heightfield.rs (heightScale folded into stored heights —
  lerp is linear so prescaling is value-identical); built from baked terrain
  GLB via Scene::heightfield(). Tier-1: parity/spike unit tests + TAT1
  round-trip (grid vertices exact, RE_LOG worked example within 1.0).
- Open: bake's terrain GLB triangulation uses a FIXED diagonal — silhouettes
  can mismatch the game's alternating split; align terrain_glb.rs when
  polishing visuals.

### Soldier ODF props in side LVLs (2026-08-30, P3 prep) — verified, partial
- BF1 SIDE/ALL.LVL: 40 entc inside lvl_ sublevel wrappers (hashed names);
  per-unit sublevels repeat shared classes (vanguard/pilot per env variant).
- all_inf_soldier (BASE "soldier") key constants, PROP-hash-verified:
  MaxSpeed 6.5, MaxStrafeSpeed 4.5, MaxTurnSpeed 4.0, MaxHealth 300,
  AimValue 1.0, AimFactorPosture Special/Stand/Crouch/Prone = 50/100/140/160,
  AimFactorStrafe 60, AimFactorMove 70, UnitType Trooper.
- Camera per stance: CAMERASECTION (fnv1a 0xBACA6BF1) sections STAND/
  STANDZOOM/CROUCH/CROUCHZOOM/PRONE/PRONEZOOM with eye heights 1.8/1.3/0.5,
  TrackCenter 0xE85D5895, TrackOffset 0xFD3D9507, TiltValue; first key of
  each section = 0x41568C97 (value looks like eye position, name unresolved —
  NOT EyePoint=0x53F8FE8C).
- More confirmed PROP keys: WeaponName 0xFBF47DBA, WeaponAmmo 0xDC19BD13,
  DropItemClass/Probability/NextDropItem 0x406D2B75/0x26996DAE/0x5EA04066,
  GeometryName 0x47C86B4A, AnimationName, FirstPerson, HealthType, UnitType,
  AISizeType, HurtSound/DeathSound, TiltValue.
- TODO P3: resolve remaining hashes; find where engine code READS these
  fields (struct offsets). `bake classes` command now emits any LVL's entc
  with full props (bake/src/classes.rs, Tier-1 on ALL.LVL).

### BF2 soldier defaults + sprint/energy system (2026-08-30, P3 prep) — verified
- BF2 uses entc→entc inheritance: all_inf_default_rifleman → all_inf_default
  (SIDE/ALL.LVL) → com_inf_default (INGAME.LVL, BASE "soldier" = engine
  class). BF1 classes derive from engine bases directly. Runtime class store
  must implement BASE-chain property lookup for BF2 data.
- com_inf_default (INGAME.LVL) key constants, PROP-hash-verified:
  MaxSpeed 7.0, MaxStrafeSpeed 5.25, MaxTurnSpeed 5.0, MaxHealth 400,
  JumpHeight 1.78, Acceleration 70.0; SPRINT: SprintAccelerateTime 0.35,
  EnergyBar 100.0, EnergyRestore 10.0, EnergyRestoreIdle 15.0,
  EnergyDrainSprint 20.0. Aim factors identical to BF1
  (Special/Stand/Crouch/Prone 50/100/140/160, Strafe 60, Move 70).
- BF2 data still carries PRONE + PRONEZOOM camera sections (engine support
  for prone survived the button removal) — good news for the locked
  prone-returns design. SPRINT camera section: TrackOffset z 4.0 (camera
  pulls back), TiltValue 10 vs BF1 STAND 5.
- No MaxSprintSpeed key found — sprint speed derivation unknown (engine
  factor over MaxSpeed? unresolved hash?). OPEN; the stance/roll RE or a
  PCSX2 measurement should close it.
- BF1 vs BF2 trooper deltas: speed 6.5→7.0, strafe 4.5→5.25, turn 4→5,
  health 300→400.

### Instance transforms (2026-08-30, P2) — DONE, verified empirically
- XFRM 3x3 is ROW-VECTOR convention: the stored rows are the object's basis
  vectors expressed in world space (world = Y-up). In glam (column vectors)
  the stored rows become the model matrix's columns: `Mat3::from_cols(r0,r1,r2)`.
- Proof (quantitative, not eyeball): the TAT1 Tusken-camp tuskbarrier arc —
  each barrier's object-X must be tangent to the fence chain through its
  neighbors. 7/7 barriers: |dot| 0.94–1.00 under row convention vs 0.21–0.97
  transposed. (First probe — sandcrawler up-vector vs terrain normal — was
  MISLEADING: designer eyeball placement, tilts ~0.04, favored the transpose;
  don't trust single-object tilt matching for conventions.)
- Terrain GLB is world-space (identity model matrix); object Y translations
  sit at terrain height as expected.
- Viewer open items found during first renders: sarlacc pit needs terrain
  cutting (CUTR chunks / HEXP cutterQuadMask — pit ring currently sits on
  uncut sand); a few submeshes reference unbaked textures (render flat gray).

### Entity classes + geometry LOD tables (2026-08-30, P2) — DONE, verified
- Render-geometry chain: wrld inst TYPE (class) → `entc` chunk → PROP key
  fnv1a("geometryname") = 0x47C86B4A → `gmod` chunk NAME → LOD0/LOD1/LOD2/LOWD
  entries (NUL-terminated modl name + trailing f32, meaning open) → `modl`.
- entc layout: BASE (parent class string, e.g. "hover"/"prop"), TYPE (class
  name), PROP* in the same [u32 fnv1a(key)][NUL string] encoding as instances.
  Verified by direct byte dump of TAT1.LVL entc @0x1CFC4 (tat_hover_skiff:
  BASE=hover, GeometryName=all_hover_skiff + ~200 gameplay props).
- gmod INFO = 2×f32 (0.0, 1.0 observed). Single-LOD gmods exist with only
  LOD1 or LOD2 (sky dome, skiff chunks) — "best model" order LOD0>LOD1>LOD2>LOWD.
- TAT1.LVL censuses: 29 entc, 34 gmod; 137/249 instances resolve to drawable
  models. Unresolved = tat_prop_dust (effect emitter, no GeometryName) and
  com_* classes (gun turrets, recharge droids, CP markers, vehicle spawns)
  whose entc live in COMMON.LVL → bake COMMON.LVL classes when P4 needs them.
- Ported: mungefs/src/entity.rs; bake manifest gains classes+geometries
  tables; game::assets::Scene::resolve_model. Tier-1: tat1_lvl.rs entc/gmod
  census + skiff spot-check, load_scene.rs end-to-end chain (137 pinned).

### Sound banks + loading beep (2026-08-30, RE + P1) — DONE, verified
- Hash fn for sound names/tags = ELF FUN_00444b30 = FNV-1a with byte|0x20
  (so '_' hashes as 0x7F) — exactly our `fnv1a_lowercase`.
- SampleBank format: SoundBankList(0x5E262256) → SampleBank(0x9A644D15) →
  info (flat u32 hash/value pairs; records delimited by value
  hash("sample"); fields id/frequency/size/frames/padding; NO offset field —
  offsets accumulate) + data (concatenated SPU2 ADPCM, 16-B blocks → 28
  frames). Full spec: analysis/re-sound-banks.md (streams + snd_ scripts too).
- Beep = `ui_load_lp` (0xB62DC1E9) in CORE.LVL 'global' bank: 720 B ADPCM
  @3605 Hz = 1260 frames of a ~440.6 Hz tone; loading screen plays it via
  looping soundproperties load_xtracking (pitch 2.1) / load_ytracking (1.8).
- Lead-verified: independent Rust ADPCM decoder (bake/src/adpcm.rs)
  reproduces frame count, near-full-scale peak, and 440 Hz dominance
  (Goertzel >50× vs 300/600 Hz) in Tier-1 tests; `bake beep` emits WAV+OGG
  (native + both as-played renders), ffprobe-verified.
- Also mapped for later: SOUND/SHELL.LVL StreamList (shell_vo 181 segments,
  shell_music 6), CORE.LVL global bank's other 5 UI waves.

### Terrain heights (2026-08-30, RE + P1 implementation) — DONE, verified
- Formula (BF1 FUN_00382338 sample / FUN_00473710 world query / FUN_004726b8
  loader; BF2 identical: FUN_00465be8 / FUN_0055ebc0 / FUN_0055db58):
  `raw = (HGT8[(x&15)+((z&15)<<4)+(patch<<8)] << HEXP[patch].shift) +
  HEXP[patch].offset; worldY = raw * heightScale`. HGT8 is PATCH-MAJOR
  (16×16-cell patches, patch = (x>>4)+((z>>4)*patchesPerRow)).
- HEXP = 8-byte records on disk {u8 shift, u8 cutterQuadMask, i16 offset,
  i16 minHeight, i16 maxHeight} — A2's "u16 per 8×8 patch" was WRONG (erratum);
  min/max are munge metadata (offset==minHeight 256/256 on TAT1). Runtime
  repacks to 4 B {shift,_,offset} (visible in FUN_00382338's *4 indexing).
- World placement: x = cx*unit − half*unit; z = cz*unit − (half−1)*unit
  (one-cell z offset, from the height query).
- Lead-verified: decomp body read directly; empirical reproduction on TAT1
  (raw range [-10,1057] → floor −0.6 EXACT, ceiling = max(maxHeight)×0.06
  EXACT, worked example patch 17 cell (5,3) → raw 495/y 29.7/world (−856,−864));
  rendered heightmap PNG shows smooth dunes, no patch-boundary artifacts.
- Ported: mungefs/src/terrain.rs (+ Tier-1 reference-trace tests), bake emits
  terrain GLB (positions/normals/UV/LUMI vertex colors). Open: PTCH INFO
  field 2 consumer, layer splatting (LPTC/ALPH/COLR), FOLG semantics
  (512×512 4-bit, header = payload size).

### LVL textures — 32-bpp direct color (2026-08-30, P6 common-asset bake)
- A2's "always paletted 4/8-bit" is incomplete: BF1 COMMON.LVL carries five
  32-bpp `tex_` (lightning1/2, boom4/5, geo_fly_geowingsblur — effect
  sprites) with NO `pal_`; each mip BODY is RLE with 4-byte elements holding
  direct [A,B,G,R] texels (same order as palette entries). Decoded boom4 is
  a proper fireball with transparent surround → byte order confirmed. BF2
  INGAME.LVL and both TAT maps + side LVLs are all 4/8-bit. mungefs::texture
  handles 4/8/32, errors `Unsupported` otherwise; bakes now skip unsupported
  textures instead of aborting.

### LVL textures (2026-08-30, P1 implementation) — DONE, validated visually
- A2's spec held EXCEPT palette color layout: entries are stored
  **[A, B, G, R]** (u32 0xRRGGBBAA read LE), not R,G,B,A — first decode came
  out red-shifted; raw-byte inspection of sandcrawler (byte0 ≡ 255) and
  browndust/tat1_dirt_blow_in (alpha fades in byte0, constant RGB) settled it.
  Alpha is FULL-RANGE 0..255, not GS 0..0x80.
- Texels are LINEAR row-major after RLE (no GS swizzle) — A2 open Q2 answered
  by eyeballing decoded tat1_map (blue holo minimap), sandcrawler, sky, rocks.
- 4-bit high-nibble-first confirmed. **CLUTs are LINEAR at every size — see
  the erratum below; the original "CSM1 for 256-color" claim was wrong.**
  Implementation: engine/crates/mungefs/src/texture.rs; Tier-1 tests incl.
  sandcrawler palette[0] == [178,109,72,255]; `bake tex-dump` emits PNG.

### BF2 mode sublevels: the mode is in a WORLD name (2026-08-30, Opus session)
Three BF2 maps (fel1, myg1, mus1) ran conquest with zero command posts. Cause,
again ours rather than the RE: `bake::map` names a `lvl_` mode layer after
`worlds[0]` and throws the rest away, and the matcher compared that one name
against `<map>_<mode>`.
- A sublevel holds SEVERAL worlds and the mode can be in any of them. Mygeeto's
  conquest sublevel is `['myg1_turrets', 'myg1_cp-conquest']` — the bake called
  the whole layer "myg1_turrets". TAT2 only worked because its conquest
  sublevel happens to list `tat2_con` first.
- Spellings actually shipped: `_con` (TAT2 alone), `_conquest` (most maps),
  `_cp-conquest` (myg1). CTF appears as `_ctf`, `_1flag`, `_1ctf`, `_ctfcp`.
- Felucia and Mustafar ship NO conquest sublevel at all: their command posts
  sit in `fel1_design` / `mus1_design`, the map's general design pass. Falling
  back to the largest layer not named for another mode picks those up.
- The `lvl_` wrapper tags are FNV-1a hashes that resolve to none of the obvious
  `<map>_<mode>` strings, so the world names are the only handle we have.
- Result: command posts now resolve on 16/16 BF2 maps (4-7 each) and 14/14 BF1
  maps, up from 1/16 BF2 at the start of the session. Pinned by a data-gated
  test over every baked map. Which sublevels a mission REALLY loads is still
  mission-Lua data (HOLD FOR FABLE) — this is a name match over baked data.

### Material flags decide whether texture ALPHA is a mask (2026-08-30, Opus)
Found by the new per-unit/per-map golden gallery: the CIS super battle droid
rendered as nothing, and both Kamino maps came out as shredded fragments.
Cause: our shader alpha-tests every fragment at 0.5, and those textures store
alpha BELOW that everywhere (`cis_inf_sbdroid`: mean alpha 34, only 0.04 % of
texels >= 128), so every fragment was discarded. The RGB decode was fine — the
droid's texture is a correct droid skin, and its alpha channel is a darker
luminance copy, not a cutout mask.
- The deciding field is `segm MTRL`'s first u32 — the material flags word, the
  same field in both games (BF1 MTRL is 4 B, BF2 25 B). The droid's segments
  read 0x00010089, the clone trooper's 0x00010001.
- MEASURED over all 10,060 segment/texture pairs in both discs' map and side
  LVLs, joined against the baked textures' alpha histograms:
    bit 0x02 set  -> the texture is a real cutout (has both fully transparent
                     and mostly opaque regions) 77 % of the time in BF1,
                     72 % in BF2;
    bit 0x04 set  -> 36 % / 19 % (transparent/blended materials);
    neither bit   -> 6.3 % / 6.6 %.
  So 0x02 is the cutout ("hardedged") flag and 0x04 the transparent one, which
  matches how the PC tools describe SWBF material flags — but the numbers above
  are from the discs, not from any tool.
- Ported (PROVISIONAL on the bit meanings): `mungefs::model::Segment
  .material_flags`, and the bake writes a texture opaque when NO material that
  references it sets 0x02 or 0x04 (`bake::detail::ALPHA_IS_MEANINGFUL`).
  Deliberately narrow: textures used by transparent materials keep their alpha
  and stay invisible in our renderer (we have no blend path), so nothing that
  is meant to be see-through turns solid.
- ERRATUM 2026-09-01: "we have no blend path" was true for a few hours. The
  alpha-blend composite pass landed the same evening in `7a919fa` (`render`
  blend pipeline, `DrawCmd::textured_blended`, GPU referee
  `render/tests/alpha_blend.rs`); transparent-material textures are
  composited, not invisible, and water is drawn on 12 maps since session 4.
  Of the "Open for Fable" items below only the rest of the flag word
  (0x08/0x10/0x80) remains open.
- Effect: 4 of 36 goldens moved — the droid, its side's weapon sheet, and both
  Kamino maps (54 % / 38 % of pixels). Nothing else changed, i.e. no map lost a
  genuine cutout.
- Open for Fable: the rest of the flag word (0x08/0x10/0x80 appear on units and
  glass), and a real alpha-blend path in the renderer, which is what the 50
  textures with no opaque texels at all actually want (shadows, godrays,
  water, holos, HUD hints).

### Terrain layer texturing (2026-08-30, Opus session) — why every map was tan
Symptom: Hoth's snow rendered sand-coloured. Root cause was NOT a decode bug
and NOT an RE error — the terrain was never textured at all. Two gaps, both in
our own code:
1. `viewer::upload_scene` drew the terrain with a hardcoded
   `TERRAIN_TINT = [0.82, 0.70, 0.55]` and threw away the submesh's material —
   even though `bake::terrain_glb` had been naming LTEX layer 0 as that
   material since P1.
2. The baked terrain UVs were the cell fraction (one 0..1 stretch over the
   whole 2 km map), so even bound, a layer texture would have been unusable.
Fixed: UV = world metres / SCAL[0], material = LTEX[0], renderer binds it.

Measurements that back the choices (all 50 shipped PS2 terrains):
- **SCAL = world metres per texture repeat** (PROVISIONAL). Values are 8..128
  on the 8 m-cell maps and 512 on the two 40 m-cell (10 km) maps — they track
  CELL size, not map size, and the minimum is always one or two grid cells.
  The alternative reading ("repeats across the map") would tile ordinary
  ground every 64-256 m and the 10 km cloud maps every 20 m.
- **AXIS/ROTN are unused for the base layer**: AXIS is 0 on every layer 0 in
  both games (non-zero only on layers >= 2 of nine maps), ROTN is 0 everywhere
  except one layer of BF2 DAG1.
- **`PCHS -> PTCH -> INFO` field 0 is a u16 LAYER BITMASK + u16 flags.** Bit i
  = LTEX[i] is painted in that patch. Verified on all 50 terrains: every set
  bit is below `textureCount` once the high u16 is split off (that half is 0
  on every patch except one apiece on YAV1/YAV2, 0x0001_0001 / 0x0001_0005).
  Mask 0 means the patch carries no layers: plotted as an image, the non-zero
  patches of KAS1 (92 % zero) and END1 (85 %) form exactly the island and the
  forest clearing — i.e. mask 0 is the ground outside the level.
- **LTEX[0] is the base layer of every map**: it is painted in 81-100 % of the
  non-zero patches on all 26 ground maps (median 100 %), which is why a
  single-layer bake is a reasonable approximation. 98-100 % of patches carry
  no per-patch layer data at all (HOT1: 1001 of 1024).
- **LUMI's neutral value differs per map** (PROVISIONAL). BF1 terrains cluster
  at or below 128 — HOT1 stores exactly 128 on 99.2 % of its texels, BF1 KAS2
  never exceeds 128 — while bright BF2 terrains (DAG1/END1/HOT1/KAS2/TAT2) sit
  at 255 on 93-99 %. Read as the GS 128-is-1.0 convention, the bake scales the
  first group by 255/128. Classified per map by the 99th percentile (<= 204 =>
  128), not per game: BF1 BES2 uses the full range, BF2 FEL1/NAB2/YAV1 do not,
  and YAV1 ships the same lightmap in both games. Cross-check: with the scale
  applied, terrain brightness matches the rock props sharing the scene; without
  it, the ground is visibly darker than the geometry standing on it.
- NOT decoded, left for Fable: how the layers in a mask are BLENDED (per-patch
  `ALPH`/`COLR`/`LPTC`; the PCHS-level COLR/ALPH are not plain RLE arrays —
  tried elem 1/2/4, none decode to a grid size), what the flag bit on the two
  Yavin patches means, whether mask-0 patches should draw at all (`LOWR` holds
  a second, lower-resolution terrain), and the terrain's own detail texture
  (`DTLX` "hoth_detail" / "tat1_detail_1", with a `DTEX` 28-byte header that is
  all zeros on HOT1).

### All 30 ground maps bake (2026-08-30, Opus session) — two format errata
Baking every BF1/BF2 ground map with the P1 pipeline failed on 11 of 30. Both
causes were errata in formats we already parse, found by reading bytes against
the existing specs (no new format cracked):
- **`path` INFO is u16 count + u16 kind, not u32 count.** BES2/END1/GEO1/RHN2
  each carry a path literally named "boundary" whose INFO reads 131078..131082
  = 0x2_0006..0x2_000A. Checked over all 213 `path` chunks in BF1's map LVLs:
  the LOW u16 equals `PNTS len / 28` in every one, and the high u16 is 0
  everywhere except those five boundary paths, where it is 2. TAT1 (the P1
  map) has kind 0 on every path, which is why the u32 reading survived P1.
  Ported: `mungefs::path::Path.kind`; Tier-1 regression on RHN2.
- **A `tern` chunk can be a stub.** COR1/MYG1/TAN1/UTA1 ship `tern` = NAME
  only, DEA1 ships NAME + WATR (the first PS2 `WATR` we have seen — A2 said it
  was PC-only). These BF2 maps are built entirely from models (city platforms,
  interiors) and have no heightfield at all. `Terrain::is_stub` + the bake
  skipping them, rather than erroring; Tier-1 on COR1/DEA1 vs HOT1.
- Also observed (not a parser change): BF2 mode sublevels are named
  `<map>_conquest` on most maps but `<map>_con` on TAT2, and CTF appears as
  `_ctf` / `_1flag` / `_1ctf`. Matching those aliases takes BF2 maps with
  command posts from 1/16 to 13/16. Which sublevels a mission actually loads
  is mission-Lua data — HOLD FOR FABLE; fel1/myg1/mus1 still find no conquest
  layer by name.

### BF1 detail textures — `<name>_dtl` (2026-08-30, Opus session)
- A2 §4 [X] already had the rule: `tex_` INFO `detailCompressed != 0` means
  the colours are modulated **x2** against a sibling texture named
  `<name>_dtl` (unmunge `resolve_detail_compression`). Nothing consumed it
  until now, which is why BF1 units looked like 64-px blobs next to BF2's.
- Census of BF1 SIDE/ALL.LVL (84 unique textures): 22 are detailCompressed
  and every one has exactly one `_dtl` sibling — no orphans in either
  direction. The companion is ALWAYS exactly 4x the base in each dimension
  (64->256, 32->128, 128->512) and always 4-bit; the base is always 8-bit.
  Same shape in SIDE/IMP.LVL (imp_inf_trooper 64² + imp_inf_trooper_dtl 256²).
  BF2 sides do not use it at all: rep_inf_ep3trooper and cis_inf_sbdroid are
  single 256² textures with the flag clear.
- **The companion carries its signal in ALPHA** — decoded RGB is 0.0 for every
  texel of all_inf_trooper_dtl and rebel_face01_dtl; mean alpha 128.2 / 126.4,
  i.e. centred on the x2-neutral value. 16 distinct values (4-bit).
- **UV scale is 1.0 — the map is registered, not tiled.** Lead-viewed proof:
  upscale the base 4x and lay the detail alpha beside it — rebel_face01's eyes,
  nose, mouth and beard land exactly on the base face's, and all_inf_trooper's
  webbing, pouches and jacket seams land exactly on the base's. A tiling detail
  texture would neither register nor scale with each base's size. So the pair is
  "same UVs at 4x resolution", not a repeating noise layer.
- Ported: `bake::detail` writes the companion as `<base>.dtl.ktx2` (presence =
  the detailCompressed flag downstream), `assets::CpuTexture.detail`,
  `render::DetailDesc` (bind group 1 gains the detail view + a MaterialParams
  uniform) and the shader's `detail_factor`.
- **Modulation happens in GAMMA space.** The GS multiplies the stored 8-bit
  texels; our base texel arrives linearised by the sRGB sampler, so the
  equivalent factor is `pow(2*d, 2.2)`, not `2*d` (which would visibly
  under-apply the effect). Pinned by a headless GPU test
  (`render/tests/detail_map.rs`): detail alpha 64/192 must produce linear
  ratios ~0.214 / ~2.46, the bounds excluding the linear-space answer.
- Open (no citation, not guessed): whether the PS2 also LOD-fades the detail
  layer with `mipDistance` (all these pairs ship mipCount 1).

### ERRATUM — 256-color CLUTs are LINEAR, not CSM1 (2026-08-30, Opus session)
- P1 applied a CSM1 CLUT unswizzle (swap the two middle 8-colour blocks of
  every 32) to 256-entry palettes and validated it by eyeballing 8-bit sky
  and rock textures. **That validation was invalid**: a noisy rock still
  reads as "rock". Decoding the same textures BOTH ways side by side settles
  it — CSM1 speckles every 8-bit texture, linear is clean.
- Evidence (contact sheet, both decodes, lead-viewed): `rep_inf_ep3trooper`
  (BF2 SIDE/REP) clone armour plates are shot through with black/white
  pixels under CSM1, solid white with blue trim under linear;
  `tat2_bldg_set01` cantina doors are orange confetti vs clean wood/stone;
  `tat1_rocks1` is uniform noise vs a photographic rock face;
  `all_inf_trooper` likewise. 4-bit textures are unaffected (16-entry CLUTs
  were never unswizzled), which is why the P1 spot checks (sandcrawler,
  tat1_map — both 4-bit) missed it.
- Measure pinned as a Tier-1 regression (mungefs `tat1_lvl.rs`
  `tat1_256_color_clut_is_linear_not_csm1`): mean |luma step| between
  horizontally adjacent texels of tat1_rocks1 — linear 9.37, CSM1 19.78
  (2.1x). Real texture content is locally smooth; a wrong palette
  permutation is not.
- Fixed in `mungefs::texture` (unswizzle + its unit test deleted); everything
  baked before this (tat1, tat2, common_bf1/bf2, side_bf1/bf2) was rebaked and
  the tat1 goldens regenerated.

### ERRATUM — BF2 modl INFO is 72 bytes on PS2 too (2026-08-30, P5 fix)
- A2 §Models said "modl INFO 68 B (both games) — PC BF2 has 72 B with 4
  leading u32 — PS2 BF2 keeps 68 B". Wrong: every BF2 PS2 modl INFO in
  TAT2.LVL is 72 B (90/90; BF1 TAT1 51/51 are 68 B) with ONE extra leading
  u32 before the boxes (vertexBox at +16/+28, visBox +40/+52, faceCount +68).
  Reading BF2 data with the 68-B offsets shifted every dequantization box:
  tat2_bldg_cantina came out 0×65×13 m mirrored, dockingbay_02 with zero
  x-extent. With the 72-B layout: cantina 65.0×13.0×37.7 m, dockingbay_02
  39.5×15.5×41.1 m, crawler (BF1) unchanged 23.0×30.7×59.1. mungefs
  model.rs now switches on INFO length; Tier-1 test pins the three extents.
  Lesson: the first TAT2 render "looked like a city" but nobody measured a
  building — extents belong in the bake tests.

### LVL/bake formats (2026-08-30, audit A2) — DONE, verified
- Container = PC-identical chunk scheme; tags are ASCII or FNV-1a-32(lowercase)
  hashes (same hash as PC). PROP keys, lvl_ sublevel names, CORE config tags
  cracked locally. Tool: `scripts/ucfb_tree.py`.
- Textures: PS2-specific — tex_ INFO (14B: w,h,bpp,detail,mipdist,mips) +
  pal_ CLUT + RLE BODY per mip. RLE codec cracked (ctrl <0x80 literals /
  ≥0x80 run; elem 1B texels, 4B palette) — matches swbf-unmunge
  handle_texture_ps2.cpp. Lead-verified: independent decode of
  tat_skiff01_lowrez 32×32 4-bit → exact 512/128B mip sizes.
- Models: planar quantized buffers (POSI 3×u16 dequant to bbox, NORM 3×i8,
  TEX0 2×i16/2048, STRP strips w/ 0x8000 restart, _PS2 u32 unparsed).
  303 segments stride-verified. BF1 vs BF2: MTRL 4B vs 25B, RTYP str vs u32.
- Terrain: PC-identical 28B tern INFO; heights = RLE HGT8 (gridSize²) + HEXP
  u16/patch + LUMI; VU-generated geometry. HGT8↔HEXP combination = TOP OPEN
  QUESTION → decomp RE at P1.
- PC-identical: wrld/regn/inst/PROP, Locl (u32 hash + u16 size + UTF-16LE,
  2057 entries gap-free), coll, config chunks, zaa_ anim banks.
- Community: LibSWBF2 = zero PS2 support; swbf-unmunge `-platform ps2` covers
  tex/models; NOBODY parses PS2 terrain heights, _PS2, shdw/SHDI, gmod, FOLG
  → decomp is the oracle for those. P1 parse order in a2-bake-formats.md.

### Mission Lua (2026-08-30, audit A6) — DONE, verified
- Extraction: `scripts/extract_lua.py` → extracted/{bf1,bf2}/lua/ (439 files,
  local only). BF1: 147 scripts, all Lua 4.0 bytecode (44 missions = 25
  playable + 19 `_h` campaign). BF2: 273 instances / 269 unique, all Lua 5.0
  (128 unique missions + shared Objective*/setup_teams logic scripts). Both
  compiled with LUA_NUMBER = 4-byte LE float → all mission numbers are f32.
- BF2 scr_ NAMEs are PLAIN STRINGS (no hash reversal needed); `mcfg` = music
  segment config (hashed NAMEs, unresolved, low priority). BF1 lvl_ wrapper
  hash = FNV-1a-32(lowercase) verified; BF2 wrapper hash scheme unknown (open).
- PC mod-tools cross-check: Gametoast/Documentation mirrors both games' mission
  sources; 99% name coverage; BF1 string overlap 100%, BF2 ~100% after harvest
  artifacts — sole genuine delta found: PS2 tat2g_con loads `imp_walk_atst`
  (lead-verified via strings on the bytecode vs PC source). **PC .lua = the
  mission scripts; PS2 wins where they differ.**
- RON design: generate mission data from PC .lua call sequences, validate vs
  PS2 string dumps; port shared Objective* scripts as native Rust modes;
  ifs_*/ifelem_* shell scripts are a Lua UI framework → native menus in v1.
- Lead-verified: census reproduces (44 scr_ Lua 4.0), header bytes 0x40/0x50
  confirmed, tat2g_con delta confirmed.

### Call-graph reachability (2026-08-30, audit A4) — DONE, verified
- `scripts/callgraph.py` (streaming, <2s/game; lead-verified: reran BF2,
  identical reachable sets). Text-only reachability from bindings+startup is
  only ~25% — these are C++ binaries (BF1: 5,199 indirect call sites, vtables
  in .data). Adding data-section function-pointer scanning (linker tier):
  BF1 13,319 fns live (90.4%), BF2 16,197 (92.9%).
- Dropped regions carve into contiguous bands (GameSpy/EENet/PPP/DNAS/headset/
  GSM/USB; largest BF2 0x4bd830–0x4f5ce0 = 911 fns/222KB): BF1 1,260 fns/322KB,
  BF2 1,539 fns/380KB. **RE keep-set: BF1 12,326 fns/3.06MB (83.1%), BF2
  14,692 fns/3.94MB (85.0%).** Data: analysis/a4-*-{edges,reachable}.txt (local).
- Bonus IDs: BF1 memset ≈ FUN_004266d8, memcpy ≈ FUN_00426524 (size/line match
  to BF2 anchors). BF2 ScriptInit chain FUN_004241b0 → FUN_00282f78 (the 83KB
  anim/soldier setup giant) is live but referenced only via runtime-computed
  pointer — add as roots for any future graph work.
- Caveats: linker tier is a lower bound (runtime-computed pointers); ~150–200
  coincidental data words may false-root individual fns; dropped bands
  over-capture some IRX/memcard init glue (replace with platform layer, don't
  delete); VU overlays outside EE graph.

### Lua-binding subsystem map (2026-08-30, audit A3) — DONE, verified
- True counts: BF1 483 unique / BF2 768 unique (`ScriptCB_SetNumBots` bound
  twice in BF2: 00193e28 + 0019ba28); diff = 436 shared / 47 BF1-only /
  332 BF2-only. Reproduce via `scripts/bindings_diff.py` (--md for tables);
  its inputs `analysis/bf{1,2}-bindings.txt` are regenerated from a fresh ELF
  by `scripts/scan_tables.py` (scans `.data` for `{char* name, void* func}`
  binding arrays, emits `<func_addr> <name>`) — the only path back to the
  tables, kept for that reason (cleanse C2, 2026-09-08).
- Dead online plumbing = 180 net/online + 72 profile/save/memcard bindings →
  flagged do-not-RE (DNAS, GameSpy, BF2 voice, DSSessionList, journal upload).
- BF2's growth is mostly runtime mission scripting: live entity API (~40 fns),
  objectives/timers, effects, space assault, markers. BF1 mission Lua is
  pre-load setup only. Directly shapes the RON mission-data design.
- AI evidence (for deferred infantry-AI decision, lead-verified spot checks):
  goal planner exists ONLY in BF2 (`AddAIGoal` 00191d58; goal classes
  ConquestGoal/CTFGoal/DefendGoal/... — 10 goal/planning strings in BF2,
  0 in BF1); nav vocab differs (BF1 hint-node classes SnipeHint/PatrolHint/
  CoverHint... vs BF2 planning-graph arcs/barriers with runtime Lua control);
  difficulty: BF1 8 per-team fns writing clamped globals vs BF2 single
  `SetAIDifficulty` (and `ScriptCB_SetAIDifficulty` is a return-0 stub in
  BOTH); BF2-only squad-command layer (`TranslateAICommand`, `AllowAISpawn`,
  `AICanCaptureCP`...). Shared soldier-level tuning fns are identical in both
  → divergence is the command/planning layer, not the soldier layer.
- No prone/stance bindings in either game (stances are engine-side, not Lua).

### Timing / frame pacing (2026-08-30, audit A5) — DONE, verified
- Full chain traced in BOTH games (identical architecture): VBlank INTC handler
  (BF1 FUN_00341ca0 / BF2 FUN_00427cd0) → vsync wait (BF1 FUN_00341e70 /
  BF2 FUN_00427e68) with swap-interval global **default 2** (29.97 fps) →
  frame timer (BF1 FUN_00173190 / BF2 FUN_00174510) reading COP0 Count with
  exact EE clock (294,912,000 Hz; BF1 stores /128, BF2 raw 0x11940000) →
  ONE Update(gameDt, realDt) virtual per frame. Sim dt is VARIABLE measured,
  clamped at 0.1 s; hitch guard ~0.5 s → 62.5 ms. Dual game/real clocks with
  per-clock timescale; pause freezes game clock only. Shipping record/replay
  harness quantizes time to whole vblanks (Pandemic's own determinism rig).
- Lead-verified spot checks: BF1 vsync loop + `DAT_005190c0 = 2` default,
  BF2 0.1 clamp in FUN_00174510, `DAT_00661308 = 0x11940000`. All confirmed.
- Engine decision: fixed 30 Hz tick, dt = 1/30 exactly, accumulator capped at
  0.1 s (3 ticks) → `engine/crates/core/src/lib.rs` (TICK_HZ/TICK_DT/
  MAX_FRAME_ACCUM_SECS). Never scale dt.
- Open (PCSX2 rig later): sub-Update substepping, retail interval-1 paths,
  dt-arg order confirmation, dt distribution in heavy scenes.

### RNG (2026-08-30) — DONE, ported
- BF1 `FUN_0042ab40` / BF2 `FUN_005220a0` (identical): SCE libc `rand()`:
  `state = state * 0x41C64E6D + 0x3039; return state & 0x7FFFFFFF` — 31-bit
  output, NOT the ANSI `>>16 & 0x7FFF` variant. `srand` (immediately preceding
  fn in both corpora; state at reent+0x58) stores the seed verbatim.
- Ported: `engine/crates/core/src/rng.rs` (`SimRng`), Tier-1 reference-trace
  test with computed sequences (seed 1 and 0xDEADBEEF).
- Open: where does the game call srand at boot (wall-clock?) — matters for
  replay determinism defaults; find call sites when wiring the sim loop.

### Sound banks: alias records (2026-08-30) — DONE, ported
- SOUND/<planet>.LVL: five StreamList (streamed music/VO) then one `lvl_`
  wrapper per variant bank (tat1/tat2 × era, name-hashed chunk) holding
  `SampleBank {info, data}` + `snd_` property scripts. `bake sounds` walks only
  wrappers (`is_sound_wrapper`): the first cut walked a 20 MB stream `data`
  payload as chunks and hung 10 min.
- SampleBank `info` = header pairs (name/format/numsamples/size) + per-record
  pairs (marker=hash("sample"), id, frequency, size, 0x1D48FEEF frames,
  padding). 40–55 % of records carry an extra `alias` field
  (hash 0x7d268157 = FNV("alias")) naming another record's id: aliases own no
  data, so offsets accumulate over real records only. Verified on all 8 TAT
  banks (BF1+BF2): Σ real sizes == header `size` exactly, every alias resolves
  to an equal-size real sample, no chains. Before the fix 1524 records were
  "out of bank data"; after it tat1 = 614 samples, tat2 = 644.
- Ported: `mungefs::soundbank::Sample::alias_of`, aliases resolved to the
  target window; Tier-1 `tat_sound_lvl_aliases_resolve_and_fit` (bank
  64267af8: 402 records, 172 aliases).

### Cheat codes (2026-08-30) — lead-verified, ported (BF2 Morse; BF1 none)
Source: `analysis/re-cheat-codes.md` (fable subagent). Lead checks against the
raw BF2 ELF (`off = va - 0x160000 + 0x1000`) and the decomp:
- Letter table 0x5ff100: 36 × {u8 letter, pad, char* code}, ITU Morse a–z 0–9 —
  dumped, matches. Word table 0x5ff220: 7 × {fn, hash}: alias 7d268157 /
  sos cb36196c / ammo 5b9b0daf / 80s fe9ed9d2 / at 57251588 / hud e28d61c6 /
  slow 6d9542b8; all seven FNV-1a(|0x20) hashes recomputed and match, as does
  `shell_menu_accept` = 0x6a942216 (the literal in the matcher).
- Matcher `FUN_0038c1d8`: rising edge = cur ≥ 0.125 && prev < 0.125 on raw-pad
  slots 4/5/2/3 = Up/Down/Left/Right (cur at +0x0c+4i, prev at +0x5c+4i);
  priority Up > Down > Left > Right, one symbol per call; Left terminates +
  resets the letter buffer and appends the first `strncmp(code, buf, 6)` hit;
  Right terminates + resets the word, hashes it, looks up the 7 entries, plays
  0x6a942216 and calls the handler; tail `wlen &= 0x1f; mlen %= 6` every call.
  Static buffers (0x6a4930 / 0x6a4938), no timeout, no reset on resume.
- Readers: DAT_00600b1c (sos) only in FUN_001df640 / FUN_001dfb38;
  DAT_00600b1d (ammo) only in FUN_003274f8 / FUN_00327658 — human-controller
  gate `controller+0xc0 >= 0`, plus the net-game gate.
- Accepted from the report without lead re-derivation: hud/slow/80s/at/alias
  effects; BF1 verdict (no matcher, no tables; dev-only `/invincible`
  `/unlimitedammo` + hidden fake console behind gFinalBuild).
- Ported: `ui::menu::MorseCheats` replicates the matcher including both buffer
  wraps (5 unit tests: all 7 words, silent reset, dropped letter, 6-symbol
  wrap → 't', priority + persistence). Game: sos→invincible, ammo→infinite
  ammo, hud→HUD hidden, slow→positional SFX pitch ×0.3 (`SoundLib::rate_mul`),
  80s/at/alias logged no-ops (no soldier LODs / melee / post-FX yet).
  Confirmation is a HUD-feed line; `shell_menu_accept` was NOT found by hash in
  BF2 SOUND/SHELL.LVL (open — the UI sound may live elsewhere).

### Animation banks: clip directory ported (2026-08-30) — DONE
- `mungefs::anim`: `zaa_` → `AnimBank { name, version, total_tracks, clips
  (bone_mask/name_crc/frames/bones), tnja, tada }` + `crc32_lower` (engine
  CRC-32). Tier-1: BF1 COMMON.LVL human_animation_2 = 55 clips / 1430 tracks /
  61151 TADA bytes, diveforward (35 fr, 26 bones, mask 0x5); getupfront 58,
  prone_getup_stand 45; BF2 INGAME.LVL human_1 diveforward 35/26, jump 31.
- Quirk: the MINA/TNJA/TADA section lengths are rounded up to 4 but the
  enclosing `BIN_` length is not — BF2 INGAME.LVL human_1 TADA says 28844
  while 28841 real bytes remain and BIN_ ends 1 byte into the padding; clamp
  sections to the parent and slice TADA to the header's byte count.

### Soldier models + animation (2026-08-30) — lead-verified, ported
- Reports: `re-softskin.md` (model side) and `re-anim-keys.md` (keys + rig),
  both fable subagents. Lead checks: skel INFO/NAME/PRNT/XFRM and WGHT/BMAP/
  BIND bytes at the cited IMP.LVL offsets; the agent decoder rerun
  (`validate_all.py`: 39 422/39 422 streams end-aligned, 15 banks); wireframe
  renders of the decoded stormtrooper/ep3trooper = 2.09 m T-poses; Rust ports
  reproduce every §10/§7 vector (skeleton parents/M0, vertices 0/2/150, AABB ==
  vis box; diveforward keys f0/10/34; BF1 ≡ BF2 diveforward bit-identical); the
  `human` rig rest pose equals the model skel bind pose for all 25 bones.
- Facts that matter for the port: `modl INFO[0] == 1` ⇒ POSI/NORM in bone bind
  space (`p_model = p_bone · M0[bone]`, row-vector); every shipped vertex has one
  weight; `bone = BMAP[BIND[v][0] >> 2]`; BNAM segments bind to the named bone;
  skel XFRM = R(q)ᵀ of the rig's rest quat; anim keys are ABSOLUTE local quats
  (xyzw, /2047); translation only where the MINA mask bit is set; sampling
  `f = t·(F−1)` with shortest-path slerp; BF1 `t += dt·30/F`, BF2 `/(F−1)`;
  DummyRoot z = root motion (run 6.69 m/s, walk 3.87 m/s). Model +Z is forward.
- openbf: bake converts to model space and standard glTF skinning; clips baked
  in place (root motion in `<model>.anims.ron`); `game::animator` clip table
  is PROVISIONAL (blend layers untraced). Quirk: MINA/TNJA/TADA section lengths
  are padded but BIN_ is not (clamp).
- Open: weapon attach (`hp_weapons`), upper/lower body layers, `_low1` LOD
  path, BF2 `_lower` clip naming, droid-specific banks.

### Hand weapons (2026-08-30) — ported; orientation VERIFIED (no fix needed)
- Weapon models (`imp_weap_inf_rifle` etc.) are flag=0 model-space `modl`s
  with one BNAM bone at identity (re-softskin.md §6); baked static
  (`bake weapon`) and drawn at the posed `hp_weapons` bone:
  `unit × palette[hp] × bind_global[hp]`. The rifle lands in the right hand
  (chest height in `stand_shoot`). The attach ROTATION between the weapon's
  model frame and the hardpoint frame was never read from the engine, so it
  was checked visually instead (`pose_preview --weapon` at yaw 90/180/270,
  2026-08-30): the rifle already sits level in the right hand with the muzzle
  pointing the way the soldier faces — weapon local +Z is the barrel and the
  posed `hp_weapons` bone already orients it. **No PROVISIONAL rotation was
  added**; adding one would have broken a correct attach.


### Collision LAYERS — per-class subsets, `VehicleCollisionOnly`, the BF2 prim `MASK` (2026-09-03) — lead-verified, PORTED (A.4 closed; `game::collision_layers`)
- Report: `re-collision-layers.md` (Fable RE agent; the header lists what the
  lead re-ran on the raw discs and in the decomp before accepting it). Raised by
  the A.4 stall probe: units on both NAB2s stood on `nab2_bldg_b_19`'s plaza
  INSIDE its two box primitives (`p_cube`, `p_cube1`).
- [V] Neither ELF parses a primitive's NAME for flags (BF1 `FUN_001606c0`, BF2
  `FUN_001606e8` read INFO/NAME/PRNT/XFRM/DATA and hash the name). Layer
  membership is per CLASS from entc PROPs, fnv1a of the lowercased key:
  `SoldierCollision` 5DFDC07F, `VehicleCollision` DE5365A1, `BuildingCollision`
  9828F43F, `TerrainCollision` AFE693CE, `OrdnanceCollision` FB2BDF07,
  `TargetableCollision` D23DBAF8, `VehicleCollisionOnly` C3F25CBB. A value
  names one body (a prim NAME or `collisionmesh`) or the sentinels `none`
  ADA7AFDB / `clear` 5C6E1222 (empty subset). Default with no property: every
  body (mesh + prims) is in the soldier, vehicle, building, terrain and ordnance
  subsets (BF1 `FUN_001676b0`, BF2 `FUN_00167048` with MASK 0). A property
  REPLACES its own layer's subset with the accumulated named bodies (lines OR
  together in entc order) and touches no other layer; `VehicleCollisionOnly`
  adds to vehicle and removes from soldier + ordnance (BF1 `FUN_001fb6e8`
  ~109347). `FlyerCollision` is consumed by neither engine.
- [V] BF2 adds a per-prim `MASK` u8 chunk (NAME, MASK, PRNT, XFRM, DATA; absent
  on BF1 and on ~3,000 of ~6,000 BF2 prims): s 0x01, v 0x02, b 0x04, t 0x08,
  o 0x10 = the PC exporter's `p_-so-…` letters (measured on all 21 BF2 LVLs,
  zero contradictions; lead: MYG1.LVL @0xD665C 0x11, @0xD67F4 0x13, @0x10667C
  0x02). 0 or absent = all five subsets. Loader quirk: the temp is never
  cleared, so an unmasked prim after a masked one in the same chunk inherits
  its byte (209 prims on the discs).
- [V] Discs: `nab2_bldg_b_19` `SoldierCollision = collisionmesh`,
  `OrdnanceCollision = collisionmesh`, `VehicleCollision = p_cube`/`p_cube1`
  (BF1 NAB2.LVL @0x3E99B4, BF2 @0x3AAC90); `kas2_bldg_hut_round`
  `VehicleCollisionOnly = p_vehicle_3`; `kam_bldg_mushplatform1` no property
  (mesh AND `p_collision_floor` are soldier bodies); 93 classes over the 32
  maps carry a layer key (`VehicleCollisionOnly` 112 lines, Ordnance 123,
  Soldier 94, Vehicle 41).
- [V] Queries: a soldier vs a building tests the building's SOLDIER subset
  against the soldier's BUILDING subset (`FUN_00168a60`); rays with the
  trailing flag (bolts 0x48, reticle 0x4e, LOS) test the target's ORDNANCE
  subset (`FUN_00167b90`); aim-assist 0x5e tests all bodies; the terrain object
  uses the mover's TERRAIN subset. BF2 layer ids: soldier 2, vehicle 4/8,
  building 0x10, terrain 0x80, ordnance 0x400 (why BF2's ordnance mask is 0x90
  where BF1's is 0x48).
- Open (HOLD 13 keeps these): the collision RESPONSE (vtbl+0x1c: step, slope,
  push-out), the entity→layer-id constructor stores ([D] mapping), the
  `model+8/+0xc` promotion flags, the ordnance subset for the port's rays
  (deferred, recorded in `game::assets`).
- openbf: the session-7 name rule (`*vehicle*`/`*flyer*`/`*ordnance*`) is
  replaced by the class subsets + BF2 MASK (brief `brief_a4_layers.md`);
  on-disc effect 1,056 instance-prims leave the soldier world, 2 enter.

### Sky dome placement — `DomeModel` Offset / MovementScale / scale, and the sky material's GS state (2026-09-03, Fable session 11) — lead-verified, PORTED (`a937199` placement, `f01f45d` the per-material sky GS state)
Full doc: `analysis/re-sky-dome-placement.md` (Fable RE agent; the lead re-read
the three load-bearing citations in both decomps and corrected §5's model
mix-up). Raised by the maintainer's Mustafar sky-seam report.
- **Placement** [V, both ELFs]: per frame, a dome shell's X/Z are the
  camera's exactly; its Y is `cam.y + ((base + Offset) − cam.y) · (1 −
  MovementScale) · S` — BF2 `FUN_003cb918` 425830–425837 (`Offset` at slot
  +0x44 from hash 0x14c8d3ca = fnv1a("offset"), `MovementScale` +0x48,
  `base` = sky+0x2840, zeroed in the ctor at 416071); BF1 `FUN_002f2308`
  280238–280240 (slots +0x28/+0x2c, base +0xec). So MovementScale 1.0 is
  eye-locked and the authored `Offset` reaches the shell only through
  `(1 − MovementScale)`: MUS1's volcano shell (Offset 100, MS 0.995) lifts
  0.16 m at a 68 m eye, not 100 m. **The port's `eye·MS + (0, Offset, 0)`
  is wrong in both terms** (a 0.5 % horizontal lag the ELF has not, and the
  full Offset).
- **Scale** [V gate, D value]: the shell is drawn at its authored size times
  `cam+0x140` (BF2) / `cam+0x90` (BF1); the rotate+scale block is gated on
  `DAT_005ffd78`, which is 0x01 in `.data` (read from the ELF); every write
  to that camera field found is 1.0. The domes are ~20 m shells around the
  eye; angles are what matter, so the port's 1000 m normalisation stands if
  the lift is scaled by the same factor.
- **Material → GS state** [V for the mapping, D for the segment copy]:
  `FUN_0058ed70` 723411–723480 — a draw is BLENDED iff (vertex colour alpha
  ≠ 0xff) or MTRL bit 0x04 (ALPHA 0x44, standard over; bit 0x40 → 0x48
  additive), alpha-TESTED iff bit 0x02 (TEST 0x5000b, AREF from
  `DAT_0060c64c` = 0x60 — the threshold claim is [D]), else opaque with Z
  write (TEST 0x50000); blended draws take 0x51001 (no Z write). MUS1:
  `mus1_sky_dome` 0x10001 + 0x10010 → opaque; `mus1_sky_volcano` 0x10003 →
  alpha-tested (corrected by the lead from `mungefs`). **The port's sky
  pipeline composites every shell** — wrong for opaque and tested shells.
- **The seam** [D]: the volcano upper band's v = 1.0 edge (mesh y −1.78 at
  r 19.04) sits at −4.9° elevation from MUS1's ground eye on the PS2 and at
  +0.63° in the port; a chase camera cannot see it on the PS2. The port's
  gallery overview and fly camera will still see it after the fix — the
  asset's underside was never meant to be seen.
- New preimages: `Reflect` 0x92c778aa (+0x50), `Stars` 0x622b0026; BF2 also
  parses `RotationSpeed` (+0x54, axis +0x60) and `Effect` (+0x70).
- **Not closed / HOLD FOR FABLE:** the texture wrap mode (no CLAMP write in
  the traced code — baked into the munged packets [H]); the far-scene pass's
  depth range vs a 20 m shell (§6, [H]); the MTRL→segment+0x18 copy; whether
  `FUN_0058ed70`'s material rule is the GENERAL model draw path — if it is,
  it is the first ELF reading of HOLD 10(g) (bit 0x04 = blend, 0x02 = test
  at 0x60/0x80 ≈ 0.75, 0x40 additive) and disagrees with ALPHA RULE v2's
  census heuristic on the Kamino crate (0x05 → blended by the ELF's rule,
  OPAQUE by ours); reconciling that against what the PS2 actually shows is
  the next sky/alpha RE pass.

### The sky pass — camera scale, projection, Z and the far target (2026-09-03, Fable session 12) — lead-verified, port consistent
Full doc: `analysis/re-sky-pass.md` (Fable RE agent; the lead re-read every load-bearing line listed in its header).
Resolves the session-11 inconsistency ("S = 1 and a 20 m shell against an 80 m far-scene near").
- [V] `cam+0x140` (BF2) / `cam+0x90` (BF1) is the camera's ZOOM divisor: `FUN_0057b850` 711660 hands the projection
  `tan(fov/2)·2·near/S`, `FUN_00580728` 715603 stores `S/tan` as the X scale; written 1.0 by every camera mode and
  lerped toward the controlled entity's zoom by the player camera (`FUN_001bb510` 64893–64896; BF1 `FUN_001a9f20`
  52350). ONE projection per frame: near = the camera near (0.5 m on MUS1), far = `DAT_0060c288` = 10000 (`.data`).
- [V] The far-scene pass's `[NearSceneRange[1]·S, FarSceneRange]` reaches only CPU culls (`FUN_005ae940`'s record,
  the far-queue walker `FUN_005b0330` 746646 "opaque only if 80·S < d − 2r"); no VU1/GS consumer. Nothing clips a
  4–20 m shell: the shell's angular geometry on the PS2 is the authored mesh about the eye, any uniform scale.
- [V] Every sky draw takes TEST 0x51001 (Z-test GEQUAL, NO Z write), forced by the draw flag 0x20 (`FUN_0058ed70`
  723464–723465; BF1 `FUN_004b0c48` 572592–572596). The sky is the first item of far bucket 8, painted into a
  16-bit far colour target (FBP 0x180, PSMCT16S) that SHARES the main Z buffer in the shipped mode
  (`FUN_00589fa8` 720837–720843, `DAT_0060c230` = 01 with no writer); far objects overdraw it Z-tested; bucket 9
  composites the far target over the near scene with per-pixel alpha from a depth LUT (1.0 beyond 110 m or empty,
  0 nearer than 80 m; `FUN_0058ae10`/`FUN_00584838`), ALPHA 0x54 (dest alpha), TEST 0x33001 [V/D]. The sky is
  behind everything by construction; its own depth never enters Z.
- [V] **Correction to "Sky dome placement" §5:** under the sky flag, material bit 0x02 BLENDS (ALPHA 0x44, TEST
  0x51001) — it is not alpha-tested; `mus1_sky_volcano` and `myg1_sky_mountains_01` composite on the PS2. BF1's
  sky variant blends AND tests at AREF 0x40 (`FUN_004d0108` 594636–594648) but no BF1 shell carries 0x02.
- openbf: "shells scaled to 1000 m about the eye, depth off, drawn first" reproduces the PS2 angles and ordering;
  `SkyMode::Mask` is replaced by Blend (built on the `sky-wrap` branch). Not reproduced, recorded: the far target
  clears to BLACK where no shell covers (the port clears to the horizon/fog colour — PROVISIONAL, ours); the
  80–110 m band is a per-pixel depth blend of near scene into far target (the port fades per object); the PS2
  sky is a 16-bit bilinear-magnified buffer (softer). Not closed: the single-view far-target height, the VU1
  microprogram, `FUN_00581a98`'s second fill pass, sky_ key 0xc15d0860, BF1's exact frame order.

### The `segm MTRL` flag word → GS state: the general model draw path, both games (2026-09-03, Fable session 12) — lead-verified, port NOT changed (decision pending)
Full doc: `analysis/re-material-flags.md` (Fable RE agent; the lead re-read the lines listed in its header).
Answers HOLD 10(g) at the CPU-visible level; raised by the maintainer's decision 3 (reconcile before touching ALPHA RULE v2).
- [V] BF2: `FUN_0058ed70` (723257) is THE per-segment draw for every `RedModel` (vtable slot +0xc = `FUN_0057cf60`,
  36 callers: soldiers 0x80001, weapons, sky 0x20031, UI 0x4000091, shadows); the tested word is the on-disc
  `MTRL` u32 verbatim (`seg+0x18`, 724860); `RTYP` is parsed and never stored. Rule: translucent = (colour alpha
  ≠ 0xff) | 0x04 | draw-flag 0x800000 → ALPHA 0x44 (0x48 additive with 0x40; 0x64 FIX = a/2 with 0x08), TEST
  0x51001 (Z-tested, no Z write), translucent bucket; else 0x02 → TEST 0x5000b (ATST GEQUAL, AREF 0x60 = 96/255,
  AFAIL KEEP); else opaque 0x50000. 0x01 lit, 0x80 specular second pass (Cs·Ad + Cd), 0x100 env pass,
  0x0f000000 UV-animation type; 0x20/0x400/0x800/0x4000/0x8000 unread. The distance fade (712818) applies only
  to 0x04 segments.
- [V] BF1 picks one of four segment CLASSES from the flag bits, not from `RTYP` (568105–568160): 0x20 Specular,
  0x400 Scroll, 0x800|0x4000 Blur, else Normal (0x200 forces Normal). Normal: 0x02 → TEST 0x5100b (AREF 0x60,
  AFAIL FB_ONLY, no blend), 0x04 without 0x02 → blend 0x44/0x48 (additive = 0x80 in BF1) + 0x51001; Scroll: 0x02
  ignored, 0x04 blends; Specular: 0x02 and 0x04 both ignored (opaque + additive spec pass by Ad).
- [V] TEX0 TCC = 1 (RGBA) and TFX = 0 (MODULATE) for every texture in both games (717178–717188; 562030–562040):
  the texture alpha always reaches the blend/test, As = At × Av >> 7.
  **Corrected 2026-09-23 (cleanse):** too broad — the near water speckle clears TCC (BF1 551846,
  `re-water-alpha.md` §7.1) and a TCC=0 branch sits at 562037-562038 inside the cited range (queue TCC32).
- [D] Against ALPHA RULE v2's named checks: the ELF BLENDS the BF1 Kamino crate body (0x05) by its grime alpha
  (port OPAQUE); draws class D (banner/rope/deck/grate, 0x1/0x10001) OPAQUE with alpha ignored (port MASK);
  alpha-TESTS `myg1_mountains` 0x03 at 96/255 (port BLEND); cutoff 96/255 not 0.5. Agrees on leaves/bamboo 0x07,
  fountains/mist/waterfalls/shields, Scroll/Specmap strips, `kas2_prop_light`, the BF2 crate 0x10089.
  **Corrected 2026-09-23 (cleanse):**
  superseded by the entry at ~:2425 (≈ 192/255) and by queue AT (`MASK_CUTOFF` = 0.753, disc alpha ≥ 193).
  **Ported in part 2026-09-24 (queue R1U, `0c3b5c3`):** a word with neither 0x02 nor 0x04 is no longer
  composited by rule 1 (37 draws, COR1's doors and DEA1's reactor walls among them), and a class-D sheet with no
  texel past the world's test draws OPAQUE, not MASK. Class D past the test stays MASK. Counts in
  `re-material-flags.md` §6.
  **Ported in part 2026-09-24 (queue KW, `bc069b9`):** a 0x04 word whose sheet has no texel past the world's test
  composites — BF2 KAM1's `Kam_bldg_window_01`, 8 draws of glass that had drawn solid; a 0x04 sheet past the test
  keeps the histogram (the crate question stays open). Counts in `re-material-flags.md` §6.
- **Not closed, and why the rule is NOT adopted yet (lead):** read literally the CPU-side state contradicts what the
  PS2 is known to show in two places — BF1 0x5100b without ABE would draw Endor's leaves as solid cards, and
  class-D banners opaque would hang as rectangles — and the GS's 0x80-is-opaque alpha scale against the discs'
  full-range 0..255 CLUT alpha (lead: 2,156 of 2,182 baked textures max out at 255) is unreconciled. The VU1
  microprogram's use of the prim bits (0x800800, 0x2000, 0x100000) and the CLUT upload's alpha handling are the
  gaps; a GS-level ground truth (PCSX2 GS dump of END1/KAM1/NAB2 — HOLD 7, the maintainer's rig) settles them. Until then
  ALPHA RULE v2 (the eye's rule) stands; the ELF rule is recorded for the bake to A/B behind a flag.

### Texture wrap mode — GLOBAL REPEAT for every model draw, both games (2026-09-03, Fable session 12) — lead-verified, port deviates deliberately
Full doc: `analysis/re-texture-wrap.md` (Fable RE agent; the lead re-read the loader/DL lines and re-ran the
VU1 disassembly, header of the doc). Closes the "sky texture wrap mode" hold from "Sky dome placement".
- [V] The model draw path writes no GS register on the EE; it emits display-list records (opcodes 0x406
  material, 0x417/0x415 texture, 0x43a colour) that VU1 turns into A+D pairs. The register-list builder
  (BF2 `.vutext` local 0x4c4–0x4e8, BF1 0x4c8–0x4e2, same sequence) emits seven pairs — ALPHA_1, TEX0_1,
  CLAMP_1, PRIM, TEST_1, TEX1_1, MIPTBP1_1 — and CLAMP_1's data is the texture record's 4th qword
  (`texrec+0x18`), which `FUN_00582c90` (717331/717572/717616; BF1 `FUN_0049c5c8` 561920/562186/562227)
  sets to 0 unconditionally. So WMS = WMT = 0 (REPEAT) for every model texture in both games; no CLAMP_2
  is ever written; the only EE-side CLAMP writers are 2D/post paths (sprites 5|0 then restore 0, HUD/
  framebuffer copies with REGION_CLAMP).
- [V] `tex_` carries no wrap (INFO is the 14 bytes `mungefs/texture.rs` reads; NAME/INFO/pal_/BODY only —
  byte-identical INFO for `SKY_Hoth` on both discs, and the two domes that tile in V, `pol1_sky` and
  `tan1_sky`, carry the same words as Hoth's); the `segm MTRL` word selects only the ALPHA/TEST entry.
- [V]/[H] Hoth: the base dome's bottom ring at v = 1.0010 is sampled WRAPPED on the PS2 (bilinear across the
  wrap: ~63 % of row 0), the same top-row line the port showed; whether the disc shows it depends on the
  mountain shell/fog covering el −2.8° — needs a capture.
- openbf: REPEAT stays the rule for every model texture; the sky-shell V clamp (the maintainer's decision 6, built on
  `sky-wrap`) is recorded as a DELIBERATE DEVIATION for the eye, per segment where the UVs do not tile,
  not a disc behaviour. Side facts kept for the material RE: TEX1 MMAG linear, MMIN mip-nearest (0x100) or
  trilinear (0x140) by a per-texture hash; no TEXA/FOGCOL per material.

### Fog — what enables it, with which numbers, on which draws, both games (2026-09-03, Fable session 12) — lead-verified, PORTED (`1201a92`; `bake::lighting::fog_from(sky, bf2)`) (HOLD 10(a) ANSWERED)
Full doc: `analysis/re-fog.md` (Fable RE agent; the lead re-read the lines in its header and re-dumped the raw
Hoth/KAS2/KAM1 sky_ records). Closes HOLD 10(a) and the session-4/5 fog question.
- [V] PS2 fog is not GS hardware fog (no FOGCOL write, no PRIM.FGE): it is a second VU1 program named
  `fogspread` appended to a draw batch, redrawing the geometry in the fog colour with per-vertex alpha
  `f = (z_view − near)/(far − near)` from a fog matrix (BF2 `FUN_00582248` 716525, rows from `FUN_00580500`).
- [V] BF2 enable: `SkyInfo.FogRange(near, far)` with `near < far` (`FUN_00580500` 715503 sets
  `DAT_007c5764`); colour = `SkyInfo.FogColor` RGB, alpha ignored; black is legal (KAM1 400..1356, TAT3);
  `0..0` is off (GEO1/KAS2/POL1); `Enable` ignored; `WorldFogRange` stored, never read. Applies to FAR-SCENE
  draws only (models/terrain/low-res sheet with layer 0x20000, i.e. beyond `NearSceneRange[2]`), never to the
  near scene, never to sky shells or the base dome (draw flag 0x20, 723621). Maps with `FarSceneRange(0)`
  (END1, FEL1, YAV1, TAT3) draw no fog colour at all — only the near→far depth fade into the sky.
- [V] BF1: `SkyInfo.FogColor/FogRange/FogRamp` are NOT PARSED by the BF1 ELF (the hash appears only inside the
  `LowResTerrain` scope, 270546; the other two nowhere) — the ten `FogRange(0, 2700..30000)` placeholder maps
  are dead data, which is the "filter" the heuristics stood in for. The world fog is
  `DomeInfo.LowResTerrain.FogColor/FogNear/FogFar` (the `setfognear/far/color` console globals) with the 4th
  colour component a density: `far' = near + (far − near)·255/a` for a in 1..254 (270621–270623; BF1 KAM1
  400..1000 a=160 → 1356 = BF2 KAM1's authored range). Applies to every opaque model segment in BOTH scenes
  (`FUN_004d0108` gate 594590 has no layer test), terrain and the sheet in the far scene only, never sky.
  No `near < far` test and no reset: a map without a LowResTerrain fog (RHN1/RHN2) inherits the previous
  map's on the disc [D].
- [V] Hoth: both games fog (203,201,203) 100..1000 m linear — BF1 additionally on near-scene models from
  100 m; `hoth_sky_dome` and the mountain shell are fogged on NEITHER. BF1 KAS2 really fogs red:
  LowResTerrain (255,0,0,128), 1000..2000 → far' 2992, ≤ 25 % at its 1500 m far cull [V bytes, D look].
- [V] Refuted: `DAT_0060c2a8`/`FUN_00589e38` is the quad-batch program variant (far 9 / near 0xc), not a
  fog/lighting enable; BF1 `Enable` bit 0 creates the sky object (1559 and 1 both odd).
- openbf (rule to build, lead): BF2 = SkyInfo pair iff near < far, alpha ignored; BF1 = LowResTerrain triple
  with the alpha stretch, SkyInfo fog never read; linear ramp; world models + terrain fogged, sky shells NEVER
  (the port's parallax-shell fog from session 5 is a deviation to remove); black fog kept. Not reproduced,
  recorded: BF2's near-scene exemption and the b..c depth-fade band; "fog on, no far layer" maps fade into the
  sky rather than a colour. Not closed: the VU1 blend detail, BF1's cross-map persistence, the far-layer
  assignment, four preimages (0x23e10337 split-screen scope, 0x3c84cf36, 0xc15d0860, 0x6d9b436e).


### The droideka: ball jump, walk/roll state machine, both games (2026-09-07, Fable session 13) — lead-verified, port NOT built (decision for the maintainer)
Full doc: `analysis/re-droideka.md` (Fable RE agent, session 12; six load-bearing claims re-read by a verification
agent in session 13, the key lines re-read by the lead — header of the doc). Raised by the maintainer's universalisation
decision (PLAN §Movement & feel: "the droideka in ball form can jump on every map, as in BF1 — BF2's ball cannot").
- [V] **The premise is inverted by the code.** BF2's ball HAS a button jump: `FUN_001f7110` 108096–108098 takes the
  `(+0x2b4 & 3) == 3` jump edge into the Controllable vtable +0xac → `FUN_001ff238` → `FUN_001f3360`, which fires only
  in state 0xc (Rolling) with the airborne timer < 0.1 s (`DAT_005fce88`), spends `EnergyCostJump` through
  `FUN_001e2010` (92629–92636) and adds class +0xebc to vel.y (105142); the loader (112712–112716) stores
  `sqrt(2·BallJumpHeight·9.8)` there, and the ODF ships BallJumpHeight 1.27 (→ 4.99 m/s), EnergyCostJump 30,
  EnergyBar 100 (`assets_baked/classes/bf2_cis.ron` 4960/4967/4962); +25 % along facing (105149–105151, class +0xec0
  = 1.25). BF1's droideka has NO jump in any form: its Controllable jump slot is the base `jr ra` stub (ELF
  0x530048 → 0x23e8f0), no BF1 loader handles the three jump/energy hashes (0 hits corpus-wide), and the only
  upward impulse in its region is the PostCollision mirror bounce `v ← v − 2(v·n)n` for `v·n < 0` (77795–77808).
- [V] Class `walkerdroid` → `EntityDroidekaClass` in both (BF1 `FUN_001d31d8`, BF2 `FUN_001fc3a8`); EntityDroideka
  derives from EntityGeometry + Controllable + Thread + Trackable + PlayableEntitySounds — not EntitySoldier, not a
  vehicle. 14-state machine (0–10 walk, 0xb IdleToRoll, 0xc Rolling, 0xd RollToIdle); the 14×8 u32 transition
  tables at 0x517588 (BF1) / 0x5fccc8 (BF2) are byte-identical (448 bytes compared).
- [D] Ball top speed BF2 = BallMoveSpeed 6.0 (clamp 108140–108148, velocity 106790–106806); BF1 =
  BallMoveSpeed·BallRadius = 10·0.65 = 6.5 (78971–78980, 77819); walk MaxSpeed BF1 5.0 vs BF2 2.5 (ODF).
- Open [H]: whether the PS2 pad delivers the +0x2b4 edge to a droideka-driven Controllable (same field as the
  soldier's; PCSX2 closes it). Not built: the droideka is not a roster class (`is_soldier_chain` rejects
  `walkerdroid`); the PLAN bullet's attribution must flip either way — brought to the maintainer (PLAN Log 2026-09-07).

### BF2 GLOBAL.LVL, the CW/GCW streams and CORE.LVL's bank — where the VO and the shell sounds live (2026-09-07, Fable session 13) — lead-verified, port plan pending (HOLD 9 ANSWERED)
Full doc: `analysis/re-sound-global.md` (Fable RE agent, session 12; claims A–F re-measured from the raw files and
the decomp by a verification agent in session 13; the CORE.LVL bytes re-read by the lead — header of the doc).
- [V] GLOBAL.LVL (827,344,896 B) is a plain `ucfb` of 31 `StreamList` (0x5BB97F21) → `Stream` (`emo_`) {`info`,
  `data`} — no SampleBank, no `snd_`, no `lvl_`, which is why `bake sounds` (SampleBanks only) never read it.
  6,523 segment records / 6,457 unique ids; two stereo music streams `cw_music` + `gcw_music` = 630,358,016 B
  (44.1–48 kHz); 29 mono VO streams = 196,640,768 B (22 kHz); SPU2-ADPCM; every `data` payload ≡ 0 mod 16384.
- [V] Never resident: each BF2 mission luac opens streams by name (`OpenAudioStream("sound\global.lvl", …)` +
  `AudioStreamAppendSegments`, 99 of 128 mission luac); `FUN_005547b0` (687649–687660) parses `info` only and fixes
  each record's offset at `data`, never reading the payload; the parse-time handle is CLOSED and the IOP
  `multi_streamer` reopens the LVL by `cdrom0:\<path>;1` path (correction to the s12 wording); ≤ 6 stream slots
  (45029; allocator 671244); a slot holds one stream plus the MERGED segment tables of its AppendSegments.
- [V] Squad chatter: BF2 has no `*_inf_com_chatter_tac_*` (checked: 0 hits in the 20 BF2 SOUND LVLs); its command
  VO is `<side>_command_{follow,stopFollow,stopVehicle,getIn,getOut}` + `<side>_response_*` on the four
  `<side>_unit_vo_quick` streams (ODF `VOSound` events). BF1's 9-command tac chatter lives in SOUND/CW.LVL
  `cw_tac_vo` (63 segs, 1,130,496 B) and GCW.LVL `gcw_tac_vo` (62 segs, 1,081,344 B), referenced by 21 + 23 BF1
  mission luac — NOT in the per-map banks the port bakes (`samples: []` today).
- [V] **HOLD 9 answered:** `shell_menu_accept` (0x6a942216) is a `soundproperties` in BF2 `lvl/DATA/_LVL_PS2/
  CORE.LVL`'s `snd_` @0x3800 (hash at 0x8029; pitch 1.1, gain 1.0, bus `shellfx`, priority 0.5; `shell_menu_enter`
  inherits it), sample B75D27CD (11,060 Hz, 2,704 B) in CORE.LVL's 7-sample `global` bank @0x10 (record @0x78).
  The hold was a search-SCOPE miss (BF2's CORE.LVL is not under `sound/`); `re-sound-banks.md` (2026-08-30) had
  already named B75D27CD the "accept wave" for BF1.
- openbf (plan, decision for the maintainer — PLAN Log 2026-09-07): bake once (a) GLOBAL.LVL's referenced VO segments to OGG
  keyed by segment-id hash, (b) BF1 CW.LVL + GCW.LVL's four streams (the universal tac chatter), (c) CORE.LVL's
  `global` bank + `snd_`; keep the two music streams STREAMED (or a separate bake); mirror the 6-slot / merged-table
  model. Phase D's D12 is re-pointed at (b). Not closed: segment-id names (0 of 6,457 preimaged), 1,416 unnamed
  chatter-variant props, BF1 tac-chatter ODF key names, `FUN_005543e8` field by field, no emulator confirmation.

### The AI aim-error floor is a FLOAT, and the AI sniper distance table's keys and predicate, both games (2026-09-08, Fable session 14) — lead-verified, port in Phase C commit 3 (HOLD 19(e) and 19(g) ANSWERED)
Full doc: `analysis/re-ai-sniper.md` (Fable RE agent, session 14; the raw ELF words, the decomp lines, the key hashes
and the disc ODF values re-read by the lead — header of the doc).
- [V] **The error floor is a float.** `FUN_002a2df8` = `max(0, lerp(1.6, 0.4))` in single precision: Ghidra's
  *(decompiler detail omitted)* (B2:224876) is the EE `max.s $f0,$f0,$f1`, `$f1 = 0` (word `46010028` at
  0x2a2e28); `FUN_002ae7b8` takes it as a float (`mov.s $f1,$f0` at 0x2ae85c) and clamps `err` to [floor, 2.0]
  (B2:233314–233320) — floor a constant 1.5 m when there is no target (B2:233299). The cap `FUN_002a2e38` is the
  same idiom, `max(0, lerp(2.2, 1.2))`. Both are CONTINUOUS (floor 1.6 m at skill 1, 0.97 at 11, 0.4 at 20, never 0
  in 1..20), so `re-ai-difficulty.md` §4.1's "1 m below skill 12, then 0" and "cap 2 m / 1 m" were wrong (corrected
  today), as was Phase C commit 1's `int(lerp)` (addendum 4 fixes it in commit 3).
- [V] **The sniper table**, identical in both games (BF2 `FUN_0032e4b8` B2:323339–323346 / BF1 `FUN_002889e8`
  B1:211247–211254). Predicate: class `YawSpread == 0` AND fire-point owner index < 0 (an AI shooter) AND
  `TriggerSingle` AND `ZoomMax >= 6.0` (inclusive — `imp_weap_inf_sniperrifle` is exactly 6.0) AND a current target.
  Then a step table on the SQUARED distance, strict `<`, no interpolation: 400 / 1600 / 3600 / 6400 / 10000 / 14400
  → 0.0997 / 0.0499 / 0.0333 / 0.025 / 0.02 / 0.0167, else 0.0143 rad, × weapon+0x138 (BF2) / +0x158 (BF1), ADDED in
  the `YawSpread` slot: yaw = `max(s,0) + postureBase + table×mult` (`max(s,0)` NOT replaced; BD §4.3 corrected).
- [V] Keys: `ZoomMax` 0x2a96e8b4 (loader coerces 0 → 1.0, ctor default 1.0), `ZoomMin` 0x1881b1a6 / `ZoomRate`
  0x9573fb20 loaded beside it but unread by the table, `TriggerSingle` 0xed9d56db; the disc values for the four
  roster sniper primaries + two rifle controls are in `re-ai-sniper.md` §3.
- Open: BF1's own floor curve (untraced — the port uses BF2's by D1), the owner-index field's NAME, `GetPitchSpread`.
  Ported: not yet — Phase C commit 3 (`brief_phase_c.md` LEAD ADDENDUM 4 carries the fields, predicate and tests).

### The low-res terrain sheet `LOWR` — a same-footprint far LOD, the only terrain drawn past the near scene, both games (2026-09-08, Fable session 14) — lead-verified, port pending (HOLD 10(i) ANSWERED)
Full doc: `analysis/re-lowr.md` (Fable RE agent session 13, corrected and completed by a session-14 verification
agent; the census re-run and the load-bearing lines re-read by the lead — header of the doc).
- [V] Format: `tern → PCHS → LOWR` = `INFO` + `LPTC` patches (POSI u16×3 + STRP strips), planar UVs from XZ ×
  `TextureScale`, no colour, no normals (§1).
- [V] Consumer: BF1's far branch calls `FUN_00488268` and nothing else (B1:549138/549319), with the `fogspread`
  pass unconditionally (549414–549415); BF2's driver `FUN_00572458` draws the sheet then returns (B2:706497–706501),
  fogged iff the BF2 fog flag (706769–706771).
- [V] Range: `NearSceneRange[1]·zoom` → the sky scope's `LowResTerrain.MaxDistance`, default 2000 in both games
  (B1:269943 / B2:416065), which every tern load zeroes (545885 via 546002) — so a map with a sheet but no
  `LowResTerrain` scope (BF2 DAG1, NAB2) never draws it. The PS2's heightfield is drawn only to `NearSceneRange[2]`
  (B1:540932–540937, [D] — read, not traced to the camera by the lead).
- [V] Census (`lowr_census.py`, lead re-run): 16 of 32 terrains carry a sheet; `extent/W` = 1.000 on all of them and
  |dy| against the heightfield is ≤ 1.08 m median — the terrain's OWN footprint, a far LOD, not an outside fill.
- [V] The session-13 draft's "the real terrain is also drawn in the far pass (`FUN_0048b1e0`)" was WRONG — that is
  the WATER's low-res pass (`WATR`/`WMAP` → `DAT_005648dc`, B1:538734, 550903–550916); `re-fog.md` §2.2 corrected.
- Consequence: LOWR COMPLEMENTS the far cull (queue item 5), whose terrain clip becomes `NearSceneRange[2]`
  (PROVISIONAL); it is NOT the razor-horizon fix (6 of the 9 defect maps draw no sheet on the PS2 either).
  Ported: no — brief `engine/artifacts/briefs/brief_lowr.md` (queue item L, after item 5).
  **Corrected 2026-09-23 (cleanse):** PORTED — L landed 2026-09-17 (`cf7b642`), behind `OPENBF_LOWRES=1`.

### Hoth outpost rooms — the terrain cutters and the `PTCH` replacement mesh the bake ignores, both games (2026-09-08, Fable session 14) — lead-verified, port pending (HOLD 19(a) ANSWERED)
Full doc: `analysis/re-terrain-cutters.md` (Fable RE agent session 13, every cited line re-verified and the meshes
decoded by a session-14 verification agent; the decode and the cutter census re-run by the lead — header of the doc).
- [V] The manifest cutter sphere is radius-first `(r, cx, cy, cz)` (B1:536883–536909, B2:694602–694660) — the lead's
  session-13 containment script had the order wrong; no cutter on any of the 32 terrains has r ≤ 0. Both outpost
  rooms sit inside cutters on both discs, and the sim already cuts there (`collision.rs:679`, strict `<`).
- [V] The render draws the heightfield inside the rooms because the bake ignores the flagged `PTCH` records (INFO's
  u32 flag + `STRP`/`POSI`/`COLR`/`ALPH`; loader B1:546287–546333 / B2:703562–703628, builder B1:548676–548710 /
  B2:706065–706070) and emits the full procedural grid instead.
- [V, lead re-run] Decoded (`Y = (hmin + y/2^shift)·hs`, 0 mismatches over 311 cell corners): the replacement mesh is
  the heightfield clipped exactly by the 3-D cutter volume (0 surviving points inside a cutter, 0 holes outside),
  leaving holes at BF1 cp3, BF2 cp3 and BF2 cp4 and covering BF1 cp4 at −2.56 m (1.72 m below the floor); the in-room
  heightfield crest is +6.8 / +7.9 m above the floors.
- Open: the exact STRP restart rule [H] (moot — the world pass has `cull_mode: None`); `COLR`/`ALPH` undecoded.
  Ported: no — brief `engine/artifacts/briefs/brief_terrain_patch.md` (queue item H, one bake commit + [REBAKE]).

### Hint `PrimaryStance` / `SecondaryStance` are BITMASKS, not an enum — value 3 is stand-or-crouch, never prone (2026-09-08 evening, Fable session 15) — lead-verified → `engine/artifacts/lead_tmp/s15/re/primary_stance.md`
Pointer entry; the report is the record (122 lines; a Fable RE agent, every load-bearing hop re-read by the
lead: both pickers, both setters, the BF2 reader, the BF2 apply switch, the BF1 dispatch, and the vtable slot
names against `re-stance-roll.md` §4/§5). Raised by Phase C commit 4's [H] 1/2/3 → Stand/Crouch/Prone mapping
(HOLD 17) and the census that 47.8 % of takeable hints author 3.
- [V] The value is a bitmask: bit0 stand, bit1 crouch, bit2 prone. The picker (BF2 `FUN_002deed0` 267121–267133,
  BF1 `FUN_0023f600` 157215–157227, byte-identical logic) draws `r = rand(0,2)` (inclusive) until
  `(mask >> r) & 1` — a uniform choice among the SET bits, drawn once when the behaviour enters its "at hint"
  state and pushed every tick as a standing request.
- [V] BF2's setter (`FUN_002defe0` 267186–267232; keys fnv1a "primarystance" 0xbf0f1cd1, "secondarystance"
  0x92c05a59 — recomputed by the lead) stores `PrimaryStance & 3` into bits 8–9 of hint+0x50 (a value with
  no low bits is ignored; the ctor default `| 0x500` = stand/stand) and `SecondaryStance & 3` into bits 10–11
  with `(v & 0x18) << 9` as the peek-side flags. BF1's (`FUN_0023f648` 157229–157277) keeps the RAW byte at
  +0x7c when `v > 0`, `SecondaryStance & 7` at +0x7d, the peek sides at +0x7e, `Mode` (BF1-only; 3 → 0) at +0x78.
- [V] Index → action: BF2 `FUN_00249f80` 164553–164566 `case 1|2 → vt+0xa4 (crouch)`, `case 3 → vt+0xb4
  (sprint)`, `case 0|4 → vt+0xa0 (stand up)` on controllable vtable 0x61e998 (slot names `re-stance-roll.md`
  §5: +0xa8 go-prone is the STUB) — so BF2's AI never prones, three times over (the `& 3`, the Cover tick's
  `2 → 0` fold at 257788–257790, the stub); BF1 `FUN_0020aad0` 119860–119877 on `soldier+0x3c8`: 0 raise,
  1 crouch (+0x7c = `FUN_00208b00`), 2 prone (+0x84 = `FUN_00208c70`) — prone is reachable in BF1 only, and
  only from values with bit 2 (4–7, the BF1 SnipeHints).
- Answer table (`v` → BF2 / BF1): 1 stand/stand; 2 crouch/crouch; **3 stand-or-crouch 50/50 in BOTH**; 4 stand
  (masked to 0 → default) / prone; 5 stand / stand-or-prone; 6 crouch / crouch-or-prone; 7 stand-or-crouch /
  any of three; 0 or absent → stand. Adoption: only in the "at hint" state (Snipe: not moving AND within 2 m;
  held `rand(60,120)` s), pushed every tick target or not; Cover cycles primary → secondary → peek holds.
- Port rule (lead, 2026-09-08 evening): the hint's value is a MASK of allowed stances; the AI draws one uniformly
  among the set bits from the sim RNG when it takes the spot (deterministic); a BF2-sourced map applies `& 3`
  (that engine never prones), a BF1-sourced map keeps the byte. Phase D commit 0b builds it (queue item PD);
  the [H] 1/2/3 mapping shipped in Phase C commit 4 is wrong for 3 and 4–7 and stands as PROVISIONAL until then.
- **The three side findings are now LEAD-VERIFIED and CLOSED (2026-09-09, session 17, `analysis/re-stance-side.md`;
  A and C re-read by the lead, B's order-kind ladder re-read and its reachability the agent's) — they replace the
  PROVISIONAL bullet that stood here:**
- [V] `re-stance-roll.md` §3.2's labels ARE swapped (s17 `lead_tmp/s17/re/stance_side.md` A): block A (`soldier+0x3c8/+0x3cc/+0x3d0/+0x3e0`, `FUN_0020aad0`
  119860–119877) is written only by the AI soldier controller vt `0x5375e8` (+0x54 `FUN_0024ffa8` → `FUN_0023c2a8` 154691; +0x58/+0x5c roll L/R `FUN_0024fe18/fe50`);
  block B (`+0x2ee..+0x2f1`) only by `PlayerController::update` `FUN_0025e098` 179932–180038 from pad bits 2/3 (tap-release = crouch, double-tap = prone).
- [V] BF1 `FUN_002450b8` (`UnitCombatAgent` OnEnterState, vt 0x536840) does push prone directly — state 6, `ctrl vt+0x54(ctrl,2)` 161237, held 4 s, no hint read — but
  state 6 is only the ctor's initial state for order kind 3 (`FUN_00244f80` 161154–161156) and no reachable site issues kind 3 (kinds 0/1/2 at 171001/171132/171181/
  171336/171450; setter `FUN_00263b10` unreachable): DEAD on the PS2. BF1 AI prone stays hint-only (SnipeHints bit 2); port rule unchanged.
- [V] BF2 `FUN_002d53c8` (vt 0x6240e8, the sub-behaviour at `UnitTrooperAgent`+0x100, order type 9) IS coded as a third hint-stance consumer — state 5 `[0xc] =
  FUN_002def18(hint)` 260664, pushed by the agent tick 254240–254256 via `FUN_002c20e0/20a8` → `FUN_002c1f50` → `FUN_002ba970` — but its CoverHint query
  `FUN_002df858` 267643 needs `FUN_002deec0 != 0`, a `return 0` stub (0x2deec0): states 3/5 unreachable, DEAD; not "Fortification type 7". Port rule unchanged.

### Water: the PS2 surface, its fog, its level and `CPAT`, both games (2026-09-09, Fable session 17) — lead-verified on the hops named below; the VU-side claims are the agent's → `analysis/re-water.md` (HOLD 10(f) and 19(c) ANSWERED)
Full doc: `analysis/re-water.md` (Fable RE agent; the doc's header names exactly what the lead reproduced and re-read).
**Lead-reproduced:** the disc `fx__ Effect("Water")` census on KAM1/KAS2/YAV1 (`lead_tmp/s17/re/fxwater.py`, read-only;
YAV1 rerun on `YAV1.LVL` → `Tile(2,2)`, `OceanEnable(0)`, `DisableLowRes()`, `Velocity(0.1,0.02)`, `MainTexture('yav_water')`).
**Lead re-read:** `mungefs/src/water.rs:201` (`height` = the f32 after the u32); the BF1 `CPAT` tile loop 551346–551359;
the far-distance multiply `DAT_00522a7c * FarSceneRange` at 551290–551293. **NOT re-read (the agent's [V] over the decomp):**
every VU-side claim — the per-vertex diffuse LUT, the speckle pass, the FFT ocean and the GS words quoted with them.
Closes HOLD 10(f) and 19(c). BF1 `FUN_00475e08` 538702 (`WATR`) keeps only `LAYR.height` (538743) — the u32, both f32 pairs and the colour word are read and
dropped (538742–538751; BF2 696356–696364), so `LAYR`'s colour has NO byte order to find. `WMAP` bit 3 = "an `LPTC` follows in `CPAT`" (538764–538767; disc: bit-3
cells == `LPTC` count on all 14); a patch REPLACES its cell's flat quad (tile+0x24 either/or, 551350–551359) — `water_glb`'s rule is verified
(**RETRACTED 2026-09-17, queue WP `7ef36a8`: see the 2026-09-17 water-patch entry — the draw is a THREE-way and a patch
yields its OWN cell, not the box's 3×3 block**). The surface
height is `LAYR.height` + 0.06·sin(1.885 t) (539266, 538346) — `mungefs::water` reads the right field; the YAV1 pool contains the bazaar by authoring. The
water is configured by the LVL's `fx__ Effect("Water")` (keys at 538912–538988, 550347–550408; 27 names cracked, §5 table): `MainTexture`, `Tile`×INFO5,
`Velocity`/INFO5, Min/Max/BorderDiffuseColor (a 16-entry per-vertex LUT on VU1, 551904–551970, alpha 551854–551903 → blend 0x44 at 188/255 on KAS2, 80 on
KAS1, 128 on NAB2), an additive `SpeckleTextures` layer (25 `water_specularmask_NN` frames, on the discs).
**Corrected 2026-09-23 (cleanse):** see the matching errata in `re-water.md` (W4 / W5).
Fog: the NEAR water is never fogged in either game
(no `fogspread` in `FUN_0048b9c0`'s near branch / B2 708590+); the FAR water (past `NearSceneRange.b`, to 0.9·`FarSceneRange`, only without `DisableLowRes`)
is re-drawn Z-tested no-Z-write with a `fogspread` sub-pass (551405–551448); BF2 gates that sub-pass on `SpecularEnable`, which no map authors → BF2's far
water is unfogged. YAV1 authors `DisableLowRes` → no far water at all. KAM1 authors `OceanEnable(1)`: a 16×16 Tessendorf FFT ocean per 160 m cell
(`FUN_00477b98` 539609 Phillips, `FUN_004cea50` 539584 iFFT), base `kam1_water_2` opaque + foam `kam1_water` blended — the port paints the sea with the foam
texture. GS words: near main ALPHA 0x44/TEST 0x50000 (Z-written), far main 0x44/0x51001, speckle 0x48/0x51001, fog 0x144/0x51011, ocean base 0x4100/0x50000.
**Corrected 2026-09-23 (cleanse):** see the matching errata in `re-water.md` (W4 / W5).
NOT closed: the VU1 microcode (wave displacement, LUT lookup by the random word), the exact look of the fog pass (a PCSX2 GS dump), `0xa015e2ff` (mode 2).
- Ported: no — queue items W1 (the `fx__` config + texture/tiling, [REBAKE][GOLDENS]), W2 (tint/alpha/Z and the near/far fog
  rule, after W1 and 6), W3 (the additive speckle pass), WL (the port-side floor measurement at YAV1/KAS2). Item 5's open
  water-cull question is answered inside W2. Still Fable's: the VU1 microcode and KAM1's FFT ocean (HOLD 21).

### HUD bitmaps composite by CLUT alpha — source-alpha blend, no alpha test, and the loader scales CLUT alpha ×129/256 (2026-09-09, Fable session 17) — lead-verified → `analysis/re-hud-icons.md`
Pointer entry; the report is the record. Lead re-read before adoption: the BF1 `FUN_004bbe90` ALPHA 0x44 / TEST 0x31001
stores (B:580213–580427); the `pal_` loader's `(A & 0xff) * 0x81 >> 8` CLUT alpha scale (B:571177–571201); the bake's
`alpha::keep_alpha` (`bake/src/alpha.rs:336–342`) → `make_opaque` (`map.rs:1010–1021`).
Pointer entry; the report is the record. Raised by phase-d's squad marker drawing a tan square (`20260909_0035_phase-d.md` l.66–70).
- [V] Every HUD icon on both discs is a `RedBitmapElement`; its draw emits ONE material entry {ALPHA 0x44, PRIM TME|ABE, TEST 0x31001}
  (BF1 `FUN_004bbe90` 580213–580402 + init 580408–580427; BF2 `FUN_005a88c8` 741097–741286 + 741292–741311): source-over by As, no
  cutout, no Z. As = CLUT alpha × ((colour.a+1)>>1) >> 7, TFX modulate.
- [V] The `pal_` loader scales every CLUT alpha (A·0x81)>>8 at upload (BF1 `FUN_004aebf8` 571177–571201, BF2 726290/726309) — disc
  0xff IS the GS's 1.0. Lead to re-read before adopting: this reconciles `re-material-flags.md`'s open "0x80 vs full-range" item.
- [V] The disc icons ARE cutouts (`squad_cmd_highlight` 1,594/4,096 texels alpha 0); the BAKE forced them opaque — `bake_content`
  (map.rs 958–1021) kept alpha only for textures a MASK/BLEND model draw names; 211 of BF1 COMMON.LVL's 224 alpha-bearing sheets
  baked opaque, `common_bf2` likewise. The loop's "alpha 255 everywhere" measured the bake, not the disc.
  **FIXED 2026-09-17 for the COMMON bakes by queue item SM (`f21c2db`)** — the shipped `squad_cmd_highlight.ktx2` now carries
  1,594 of 4,096 texels at alpha 0 and `squad_cmd_cross.ktx2` 7,732 of 16,384, re-read off the KTX2 by the landing session;
  a MAP sheet still bakes opaque (queue **WA**). Detail in `re-hud-icons.md` under this same claim.
- [V] `squad_cmd_cross`/`_highlight` are the 2D D-pad command menu (HUD (−120,−50); cross 64², four 32² arms at ±16 rotated
  0/π/±π/2, labels at ±25; selected 0xff vs 0x7f; 0.5/s fade), same tables in BF2, which asks for the same sheets by FNV hash but
  ships neither on the PS2 disc (fallback branch; what it shows is [H]).
- Port: queue item SM — keep the alpha of every sheet no model names, blend the marker (`textured_blended`, depth off), flag on.

### Backface culling is VU1-side, per material record, and OFF for plain opaque segments (2026-09-09, Fable session 17) — GEO1's wreck is NOT a material bug → `analysis/re-vu1-cull.md`
Full note: `analysis/re-vu1-cull.md` (VU1 dumps + decoder under `lead_tmp/s17/re/vu/`). **Lead-verified on the CPU side only:**
the EE's `f&0x2` → TEST 0x5060b agrees with `re-material-flags.md` §2's [V], and the port's one `cull_mode: None` pipeline
(`render/lib.rs:1496`) already matches the two-sided result. **The VU1 microcode reading below is the AGENT's alone** — the
lead did not re-read 0x800 / 0x800000, the facing loop or the ADC bit. Answers HOLD 10(g)'s two-sided half.
Full note: `engine/artifacts/lead_tmp/s17/re/geo1_wreck.md` (VU1 dumps + decoder under `re/vu/`). Answers HOLD 10(g)'s two-sided half
and re-reads `re-material-flags.md` §2's [H] on 0x800800/0x10000 as [V].
- [V] The material qword's x word (ALPHA | `seg+0x04` prim bits) is converted by VU1 to `slot[6].w = x>>8` (BF2 0x411–0x416); the bits
  it tests are 0x4000 lit, 0x2000 vc-lit, 0x100000 COL0, 0x100 UV-matrix, **0x800 = cull enable, 0x800000 = cull-side flip**. With 0x800
  on any of a batch's materials VU1 runs a facing loop (0x640–0x663: rolling OPMULA/OPMSUB over the strip, FMAND Sz, XOR a load-time seed
  bit in the vertex w) and the material's draw (0x393–0x3a1) turns that bit into the GIF ADC bit (`w |= (w<<1) + flip`). BF1 identical
  (0x18d / 0x393 / 0x624 / 0x636). The GS never culls.
- [V] The EE sets 0x800800 only for `(f&6) && !(f&0x10000)` and for colour-faded non-0x04 segments (D:723484–723487; B:594369 twin:
  translucent && mode 0; B:549452–549460 is a debug cull-flip/off switch). So **MTRL 0x10000 = "two-sided cutout/blend"**, and every plain
  opaque world segment (0x1, 0x10001) is drawn two-sided on both consoles. Draw-flag 0x1 → opcode 0x427 → VU1 state 0x100 → the CLIP/FCAND
  path [D clipping]; draw-flag 0x10000 → `FUN_00592c38` sub-model list, not material.
- [V] GEO1 `geo_bldg_technounion_hull`: five segments carry 0x02 (0x3, 0x10003 ×4) → PS2 TEST 0x5060b (test at 96/255) = the port's
  MASK in kind ([96,127] band = 1.3 % of texels on `geo_bldg_wrecked_02`/`geo_bldg_rafters`); segs 1–7 two-sided on the PS2; the port
  draws everything two-sided (`render/lib.rs:1496`). Class D fires on one 5-triangle segment (PS2 opaque). The wreck's "bare rafters" is
  NOT explained by the material path. Port always draws LOD0 (`entity.rs:101`); the PS2 LOD switch rule is unread — the LOWD's rafters
  texture (`geo_bldg_rafters_lowrez`) is 100 % opaque, LOD0's is 28 % holes, so at the golden's ~475 m eye the two consoles may be
  drawing different models. Decision: no ALPHA RULE v3, no cull item from GEO1; PROVISIONAL capture note instead.
- Ported: nothing to port — no ALPHA RULE v3 and no cull item. Open: the PS2's `modl` LOD switch rule (HOLD FOR FABLE 20;
  `re-vu1-cull.md` §4 has the two-station PCSX2 capture note that settles the wreck).

### Alpha-test cutoff in disc terms: the PS2 keeps a texel only at disc alpha ≥ ~192, not ≥ 96 (2026-09-09, Fable session 17) — [D], lead composition of two lead-verified reports
Not a new RE: this is the lead composing two [V] facts that neither report states together, recorded because the port's
`alphaCutoff` depends on it and queue item AT builds it.
- [V] The PS2 alpha-tests a MASK segment at ATST GEQUAL with **AREF 0x60 = 96/255** in GS terms (`analysis/re-vu1-cull.md` §2
  on `f&0x2` → TEST 0x5060b; `analysis/re-material-flags.md` §2 for the same AREF on both games).
- [V] The `pal_` loader scales EVERY CLUT alpha by **×129/256** on upload — `(A & 0xff) * 0x81 >> 8`, BF1 `FUN_004aebf8`
  571177–571201, BF2 726290/726309 (`analysis/re-hud-icons.md` citation 3, lead re-read). It is the one `pal_` loader for
  every `tex_`, so it applies to model textures, not only HUD sheets.
- [D] Therefore a disc alpha `A` reaches the GS as `(A·129)>>8` and passes the test when `(A·129)>>8 >= 96`, i.e. at
  **`A` ≥ ~192/255 ≈ 0.75** — not at 96/255 ≈ 0.376. The port's `alphaCutoff` of 0.5 keeps a band of texels the PS2 discards
  (measured on GEO1's wreck: the [96,127] band is 1.3 % of texels on `geo_bldg_wrecked_02` / `geo_bldg_rafters`).
  **Corrected 2026-09-23 (cleanse):** that band
  is below 0.5, so the port discarded it too; the band 0.5 kept and the disc drops is 128..192.
- Ported: **YES, 2026-09-17 — queue item AT landed as `a55688f`** (`render/src/shader.wgsl` `MASK_CUTOFF = 0.753`,
  `bake::detail::ALPHA_TEST = 193`, the Av = 0x7f end of the ±1 LSB; 27 goldens regenerated as `565d2e0` after every
  pair was quantified and viewed). **The WORLD's cutouts only:** skinned draws keep the port's own 0.5 through
  `fs_mask_skinned`, because the side bake is not on ALPHA RULE v2 (design D6) and carries no per-draw verdict —
  `bake::skin_glb` writes no `alphaMode` at all, and at 193 BF2's `all_inf_wookie` keeps 835 of its 65,536 texels
  (4,299 at 0.5), measured on the shipped bake. That half is queue entry **SKA**.
- The landing's own measurement, worth keeping: the thinning this produces is the DISC's, not our sampling. The mip
  chain is the LVL's own (`mungefs::texture` asserts `mips.len() == info.mip_count`) and the PS2 samples it —
  `re-texture-wrap.md:67,154` reads TEX1 = MXL `(mips−1)<<2` | MMAG linear | MMIN 4/5 on the same VU1 register list
  that carries TEST_1 — and the band fraction GROWS with the mip level, so the disc at 512×448 eats more of a distant
  cutout than a 1920-wide panel does. `end_prop_tree_leaves`: of the texels kept at 0.5, mip0 loses 26 %, mip3 64 %,
  mip5 100 %. The strongest single case in the goldens is TAT2's `tat2_bldg_crashed_spaceship3` grille (mip2 loses
  56.8 %), which now reads as a few scratches.
  The blend side of the same scale is what closed `re-material-flags.md`'s "0x80 vs full-range CLUT" note.

### Water config `fx__` landed; the water floor measured; the BF2 fade maps (2026-09-09 evening, session 18) — a POINTER entry: no new PS2 claim
Three findings already recorded above became port code this window. Nothing new was read off the discs; every PS2 claim
below is the one already cited in the section it names.
- **`fx__ Effect("Water")` → the water** (queue item W1, `5758238` + `52e6b54`): the port now builds texture, tiling and velocity
  from the LVL's own `fx__` records — the census and the key table are `analysis/re-water.md` §5, which gained two
  amendments this window (BF2 DAG1 authors `DisableLowRes`; the `SpeckleTextures` prefix expands to unpadded decimal
  names). Report: `engine/artifacts/loop/reports/20260909_1534_water-fx-config.md`.
- **The port's floor under the water-side props** (queue item WL, `52d0ff5`): the level AND the floor are the disc's; YAV1's
  bazaar is an authored flooded plaza; the 18 flagged prop rows are pivots by design. HOLD 19(c)'s "level vs the authored
  floors" clause is CLOSED. Report: `engine/artifacts/loop/reports/20260909_1638_water-floor-check.md`.
- **The BF2 maps with no far layer** (queue item 6, `19148e6`): the far scene now dissolves into the sky colour instead of
  ending at a fog wall, per `analysis/re-fog.md` §2.5/§6. Report:
  `engine/artifacts/loop/reports/20260909_1522_bf2-far-fade.md`.
- Ported: all three, on main 2026-09-09 evening (11 map goldens regenerated). Open: unchanged — HOLD 21 (the water VU1
  microcode and KAM1's FFT ocean) and the look items W2/W3.

### Water tint scale verified; the water pass, the class db and the spawn rule landed (2026-09-09 night, session 19) — one lead re-read, then pointers

ONE new PS2 fact, re-read in the ELF by the lead, plus the port work that followed it.
- **The diffuse LUT is authored 0..255 and packed at the GS's 0..128 — so the port's `/255` is right, and the water is DARK.** [V]
  The LUT builder (BF1 `FUN_0048d068` 551904–551970, the entries at 551945–551960) lerps the authored Border/Min/Max colour bytes
  and stores each as `byte_lerp * 0.003921569 + 65536.0`: for a float in [65536, 131072) the mantissa's unit is 1/128, so the low
  mantissa bits hold `round(byte/255 × 128)` — the GS's own 0..0x80 scale where 255 → 128 = 1.0. The alpha table (`FUN_0048cec8`
  551854–551903, the packs at 551896–551898) packs Min.a/Max.a the same way (188 → 94 = 0.74 blend). An authored 80 therefore
  modulates the texture by 0.31. Recorded in `analysis/re-water.md` §3; **HOLD 10(f)'s tint clause is CLOSED** and the
  midpoint-of-the-LUT simplification the port ships stays PROVISIONAL (the faithful form is the per-vertex random index).
- **The water's own pass** (queue item W2, `9637294` + `5e047c2`): the authored tint at that scale, a depth-written translucent blend,
  no fog inside the near range and the far rule per game (`analysis/re-water.md` §1/§3); 7 map goldens regenerated after viewing every
  pair. The FOAM waits on the bake's `keep_alpha` rule for MAP sheets — queue **WA** since SM landed 2026-09-17 with that
  clause scoped to the COMMON bakes only — and the waves landed as queue item OC (`b8ebe59`). Report:
  `engine/artifacts/loop/reports/20260909_2255_water-tint-fog.md`.
  **Corrected 2026-09-23 (cleanse):** WA landed 2026-09-23 (`19975d0`); the foam draw itself is queue OCF.
  **Ported 2026-09-23 (night), OCF `42f85e2` + `0b203e7`:** §3a's foam byte, its crest rule and its alpha packing
  (`game::water_anim::Foam`, `foam_patch`), stepped on a 30 Hz grid of the water clock (a [D] rate from §2 135276 and
  `a5-tickrate.md`), the whole map zeroed (the disc's one-byte clear is the shipped bug in `re-water.md` §3a); the
  port's own still plane past the far range carries no foam once the sea steps. PLAN Log 2026-09-23 (night).
- **A map's classes and models come off the MAP's own disc** (queue item 4, `542d7bc`): the sweep and the tat2 arena moved by design and
  were re-baselined. Report: `engine/artifacts/loop/reports/20260909_2202_era-class-db.md`.
- **The RON twins, the Hold release and the friendly-post spawn** (items 13 `6d87a77`, HR `6c5cd4a`, WS `3f7a0f2`): no PS2 claim in any of
  the three. Reports: `20260909_2224_ron-twins.md`, `20260909_2221_hold-release.md`, `20260909_2248_walk-spawn-always-post.md`.
- Ported: all five, on main 2026-09-09 night (7 map goldens regenerated, no rebake). Open: HOLD 21's remaining half — the water VU1
  microcode and KAM1's FFT ocean — and the look items SM/OC/W3.

### The `CPAT` cull re-read: an EXACT-INDEX match, a three-way draw, and an ocean twin that ignores `+0x24` (2026-09-17, Opus loop lead lane 1) — `[O] UNAUDITED: no Fable pass`
The full text is `analysis/re-water.md` §4's **CORRECTION** block; this is the pointer. Queue item **WP** landed on main as `7ef36a8`
(golden `c754d13`; report `engine/artifacts/loop/reports/20260917_1645_water-patch-cells.md`).
- **[V][O] The patch → tile match is an exact `z * grid + x` equality, never a bounding-box test.** BF1 `FUN_004770a8`
  (header 539201) 539306–539324: *(decompiler detail omitted)* against *(decompiler detail omitted)*,
  `break` only on equality, and *(decompiler detail omitted)* — which becomes *(decompiler detail omitted)*, byte offset 0x24 — stays 0.0 otherwise. BF2 twin
  `FUN_00561b58` (header 696781) 696884–696892, same shape on `DAT_00661608`. The port had an AABB-centre test instead and
  was deleting **17 / 32 / 2** wet lattice cells where the disc marks **4 / 8 / 1** (BF1 NAB2 / BF2 NAB2 / BF2 DEA1).
- **[V][O] Each `LPTC INFO` box is its own bit-3 cell padded by half a cell on every side** — exact, on all 13 shipped
  patches (`bake/tests/water_patch_cells.rs`), which pins the i-th `LPTC` ↔ i-th bit-3 cell pairing geometrically. A patch's
  triangles cover 13–96 % of the cell they replace (= 3.4–24.0 % of that twice-as-wide box, which is where §4's "3–24 %" came from).
- **[V][O] §4's "either/or in every sub-pass" is too strong**, found by the verifier agent and re-read: the flat-water draw is a
  THREE-way (`FUN_0048b9c0` 551345–551358) and the ocean path (`FUN_0048b280` 550922–551180, dispatched by `FUN_0048b1e0` on
  `OceanEnable`) never reads `+0x24` at all. No shipped ocean map ships a `CPAT`, so the port is unaffected.
- **[D][O] `tile+0x24` is a DMA packet pointer**, not a float (Ghidra types it `float` at the store, `int` at the load; provenance
  `FUN_00488f20` 550180 → record+8 at 538772).
- Ported: `mungefs::water::Water::has_patch` + `bake::water_glb`'s lattice, main 2026-09-17 (`maps` rebaked, one golden
  regenerated). Open: unchanged — HOLD 21 (the per-vertex VU1 displacement) and queue **WA** (does the water composite by its
  TEXTURE alpha).

### KAM1's ocean is metre-scale: the inverse FFT is UNNORMALISED and the mesh carries no metres at all (2026-09-17, Opus loop lead lane 1) — `[O] UNAUDITED: no Fable pass`
The full text is `analysis/re-water.md` **§3a**; this is the pointer. Queue item **OCA**, RE only — no code changed and no gate
value moved; scratch and scripts under `engine/artifacts/lead_tmp/re/water-amplitude/`.
- **[V][O] The `1/(N·M)` divide is gated on a flag the water passes as ZERO.** BF1 `FUN_004cea50` (header 593638) divides only
  *(decompiler detail omitted)*; the direction is a hardcoded `+1` in the call's delay slot (ELF `004cea7c: addiu a3,zero,1`, against
  `FUN_004cea30`'s `004cea3c: addiu a3,zero,-1`), and the water's own call site zeroes the flag (`00477b00: daddu a3,zero,zero`
  before `00477b04: jal 0x004cea50`, inside `FUN_00477928`). BF2 twin `FUN_005c4488` / `FUN_005623d0`: `005c44b4: addiu a3,zero,1`
  and `00562598: daddu a3,zero,zero`. The 1-D butterfly (`FUN_004cece8` 593674–593762, NR `four1`) scales nothing.
  **§3's "1/NM-normalised" was wrong; §8's metre-scale [D] was right.**
- **[V][O] The mesh the VU gets is a unit cube.** `FUN_00477cd0` (header 539639) takes min/max over the transform's real parts
  (539732–539748) and writes `y = (h − min)/(max − min)`, `x = i/N`, `z = j/M` (539780–539782); `FUN_0048a6c8` (header 550630)
  packs them as u16 `(v·0.5+0.25)·65535` in x/z and `v·65535` in y (550716–550718). The only metres are the per-tile record's two
  corners — `(x0 − cell/2, base+bob+min, z0 − cell/2)` and that `+ (2·cell, max−min, 2·cell)` (`FUN_004770a8` 539327–539330,
  two PADDED vec3s: `FUN_00478bd8` stores three floats) —
  handed over as the engine's generic vertex-dequantisation box (`FUN_0039bd98`, 57 call sites, 31 of them beside a
  `FUN_00398928` mesh submission; the same convention
  `mungefs/src/model.rs:237–250` already implements for PS2 models). **[D][O]** for the dequantisation itself: that VU program is
  unread.
- **[V][O] The ocean runs no water microcode**: `FUN_0048b280` references neither VU program `0x515410`/`0x514ec0` nor the LUT
  block `0x754a30` (3 hits each, all inside the flat-water `FUN_0048b9c0`/`FUN_0048d068`). Its per-vertex `A` is the FOAM byte
  halved (550727) — half of §8's "exact per-vertex alpha", answered for the ocean; the 16-entry LUT stays the flat path's.
- **[V][O] Measured on the disc's own formulas** (`lead_tmp/re/water-amplitude/`, 320 frames, KAM1's authored cell 160 m /
  `PatchDivisions(16,16)` / `WindSpeed(25)` / `WindDirection(0.2,1)` / `PhillipsConstant(1e-05)`, re-read from both discs):
  peak-to-trough **median 19.23 m**, rms **4.145 m**, max |h| 23.5 m, `bound` 37.0 m — against the port's 0.075 m / 0.016 m /
  0.145 m. **[D][O]** the disc's foam test (second difference > 3.0, ELF `0x40400000` at `004782a4`) fires on 13.3 % of the points
  unnormalised and on **0.00 %** under the port's `1/(N·M)`: KAM1's foam layer only works at the metre scale.
- **[V][O] A shipped disc bug, found on the way:** the foam map is allocated `(N+1)·(M+1)` bytes (539462) and cleared with
  `memset(ptr, 0, **1**)` (539465; ELF `004776ec: addiu a2,zero,1`), so 288 of KAM1's 289 bytes start as heap garbage — washed
  out by the −4/frame decay within ≤64 frames. A port zeroes the map and records the deviation (queue OCF).
- Verified by one Opus verifier agent on the citations alone (§5): every load-bearing claim SUPPORTED; it falsified the WORDING
  of two absence claims (the foam map and the transform buffer each have pointer-lifecycle sites), corrected four line ranges,
  and found the three-float store and the memset bug — all folded into §3a.
- Ported: nothing yet — queue **OCB** drops the `1/(n·n)` in `Ocean::inverse` and `Ocean::bound`
  (`game/src/water_anim.rs:332, 368`) and re-derives the far-plane `drop`. Open: HOLD 21(a) — the flat-water LUT, the phase
  nibble, the far LOD and the half-cell margin, all still VU-side.
  **Corrected 2026-09-23 (cleanse):** PORTED — OCB landed 2026-09-17 (`efab925`).

### The water DOES composite by its texture alpha — and the near speckle deliberately does not (2026-09-17, Opus loop lead, lane 1) → `analysis/re-water-alpha.md`
Pointer entry; that file is the record (§0 the answer as a per-layer table, §1 the BF1 hops, §2 the
BF2 twin, §3 the absence searches, §4 the disc census, §5 the bf1_kam1 question, §6 what stays open,
§7 the verifier's one NOT SUPPORTED). Answers the FIRST clause of HOLD 21 / `re-water.md` §8, which
queue item SM could not decide and therefore scoped around. **Headline:** every blended water layer
takes `As = At x Av >> 7` — the TEX0 the water binds carries **TCC = 1 (RGBA) and TFX = 0 (MODULATE)**,
built for every non-32-bpp texture by the engine's one texture path (BF1 `FUN_0049c5c8` 562034, the
`0x400000000` byte-verified as `ori 0x8000; dsll 19` = 2^34 at va 0x49c9f0; BF2 `FUN_00582c90` 717181),
and the near/far main layers and the ocean foam all carry ALPHA 0x44. The exceptions are exact: the
ocean BASE is not blended at all (0x4100, prim 0x18 without ABE) — and its sheet `kam1_water_2` is the
ONLY water sheet on either disc authored alpha-255 — while the near cross-faded SPECKLE has its TCC
bit **explicitly cleared** (*(decompiler detail omitted)*, BF1 551846 in `FUN_0048c848`, BF2 709258
in `FUN_00577088`, gate `DAT_00522a64`/`DAT_0060c1e4` = 1 in both ELFs), which is why BF1's 25
`water_specularmask_NN` frames are authored alpha-255 on every texel. Measured on the disc with the
port's own `mungefs`: all 14 water sheets are 4/8-bpp paletted and **10 of the 12 `MainTexture`s carry
real alpha** — the other two are the KAM1s' ocean base `kam1_water_2` (first logged as "11 of 12";
corrected 2026-09-23 by WA's landing, which baked exactly 12 alpha sheets, one per wet map) — (`kas2_water` 96 flat, `yav_water` 96 flat, `nab2_water` 89..190, `fel1_water` 80..171,
`dag1_water` 138..203, `dea1_water` 137..240, `kas1_water` 142..255, BF2 `kas2_water` 158 flat, the
ocean foam `kam1_water` 0..218 mean 62). So `fs_water`'s `base.a` multiply is the disc's arithmetic and
the bake's opaquing of map water sheets is the defect. Also answered, and it is not a water path:
**BF1 KAM1's water is under the map's own terrain** — a real heightfield at y = 0.00 over all 1,618 wet
cells, 8.00 m above the -8 plane, where BF2 KAM1's `tern` is a stub — which is why its golden and all
8 walkabout panels were 0.000 when the sheet flipped. Every load-bearing claim is tagged `[O]`: one
Opus pass plus one Opus verifier agent (which falsified the speckle claim, reproduced before
acceptance), no Fable audit. Ported: nothing yet — queue **WA**'s build half, with **TCC32** for the
32-bpp TEX0 discrepancy this turned up in `re-material-flags.md` §2. **PORTED 2026-09-23 — WA landed as
`19975d0`** (lane 1): the map's own `MainTexture`/`FoamTexture` keep their disc alpha (12 sheets on 12
maps). Its landing also found that BF1 KAM1's terrain is textured with the FOAM sheet `kam1_water` (the
`tern`'s LTEX layer 0), which is why that map's golden moved its stored frame alpha and not its RGB —
`re-water-alpha.md` §5 addendum.

### BF2 doors — why MUS1's and POL1's squads stand still (2026-09-17, Opus loop lead, lane 2) → `analysis/re-bf2-doors.md`
Pointer entry; that file is the record (§0 the verdict on queue SW's three candidates, §1 the port's
measurements, §2 the six ELF hops, §3 the disc data, §4 the Lua, §5 what the port does today, §6 the
BF1 absence searches, §7 the follow-on entries DR1–DR3, §8 what stays [H], §9 the verification).
**Headline:** BF2 has an engine entity class `door` (label at VA 0x60fb68, registered by
`FUN_00173d10`) whose tick `FUN_001ebb80` — reached only through the thunk `FUN_001ec850` — opens it
while an object of collision layer 2 is inside a sphere of the ODF's `AnimationTrigger` radius around
a named hardpoint (`hp_door 4` on `mus1_prop_door_tri`, `hp_door 3.0` on `pol1_prop_door`), and the
`open` clip translates the bone the collision box is parented to by **3.965 m in 1.00 s**. The
mission Lua never opens one — it only LOCKS one (`IsLocked`); a scripted doorway (POL1's airlock) is
a plain `prop` whose named animation the mission plays. The port has no door entity and bakes that
box at frame 0 (`game/src/assets.rs:468-489`, the prim's `PRNT` node assumed identity), so **50 of 66
door instances on 5 BF2 maps (156 bodies) stand as permanent walls on authored planning arcs** — all
13 `wall` verdicts of SK's stuck census on bf2_mus1 and bf2_pol1, every one of those units standing
1.14–2.72 m from the hardpoint that would have opened the door in front of it, and a soldier-radius
flood-fill through all five doorways impassable with the baked bodies and passable without them.
BF1 has no door ENTITY class at all (three independent searches; its 6 door-named classes are
`prop`s). Every load-bearing claim is tagged `[O]`: one Opus pass plus one Opus verifier agent, no
Fable audit. Ported: nothing yet — queue **DR1** (the interim flag), **DR2** (the door entity) and
**DR3** (`plnp`).

### The ocean's LOD — the disc builds a decimated mesh every frame and never draws it (2026-09-17, Opus loop lead, lane 2) → `analysis/re-water.md` §3b
Pointer entry; §3b is the record (the hop table for both discs, (a) the two-LOD build, (b) the
byte-exhaustive absence, (c) the passes as ranges rather than resolutions, and what it means for the port).
**Headline: queue OCL's first question is answered both ways at once.** The ocean DOES take a decimated
grid — its per-frame mesh writer ends in `FUN_0048ac90` (BF1 539979 / BF2 697487), which builds the mesh
**twice**, at stride 1 and at stride `LODDecimation` (BF1 550796–550797, `DAT_005649c0` / `DAT_005649c4`;
BF2 708144–708145) — but **only the stride-1 mesh is ever issued**. The draw primitive `FUN_0039b488`
(BF1 387615–387616; BF2 `FUN_00496eb0`, same bytes) appends a DMAtag `0x50000000` — id 5, a DMA call —
to the handle it is given, and the decimated handle is read by **exactly one instruction in each ELF**
(`0048baac` / `005760dc`), both inside the FLAT-water renderer, which the dispatcher (`FUN_0048b1e0` 550906 /
`FUN_005755b0`) runs only when `OceanEnable == 0`. The ocean renderer's own 464 instructions reference only
the stride-1 handle, twice — and the decompiler drops that argument (*(decompiler detail omitted)* at 551113/551169),
so the bytes had to put it back: `0048b5d0 lw a1,0x49c0(s6)` … `0048b5dc jal 0x0039b488` with nothing
touching `a1` in between. **So the drawn PS2 ocean is ONE resolution at every distance; there is no near/far
mesh join, and no seam.** The port's two-patch step (`game/src/water_anim.rs`, `let mesh = if d <=
o.ranges.near { hn } else { hf };`) is what opened the crack, and the faithful fix is to
delete the far patch — queue **OCL**'s option (2b), now settled rather than guessed. **The crack's size is
3.229 m, not the 2.27 m written here first** (that was the pre-OCB 8.9 mm scaled by the amplitude ratio, not
re-measured): OCL's build measured it directly over 300 frames at 60 Hz — worst 3.2292 m, mean per-frame
worst 2.4262 m. §3b carries the correction. Two things re-read as
ELF bytes rather than decompiled literals and CONFIRMED where the port already had them: the ocean's far
multiplier is the inline `0x3f4ccccd` = 0.8 against the flat path's `DAT_00522a7c` = `0x3f666666` = 0.9
(`viewer.rs:31–32`), and `FarSceneRange`'s default is `0x44fa0000` = 2000.0. Both KAM1s author
`OceanEnable(1)`, `PatchDivisions(16,16)`, `LODDecimation(2)`, read off the discs here. Every load-bearing
claim is tagged `[O]`: one Opus pass plus one Opus verifier agent, no Fable audit — the verifier attacked
the three absence claims with full effective-address tracking over all 967,919 `.text` instructions plus a
pointer-formation scan, and all three survived; what it broke was four citations, all corrected. **Ported:
YES — queue OCL's build half landed 2026-09-17 as `6bf0d60`** (lane 2). `OceanDraw` keeps ONE
`Option<MeshHandle>` instead of a pair, so the crack is unrepresentable rather than merely absent; the
`LODDecimation` validation went with it (the bake still carries the key); `ranges.near` stays for the disc's
PASS split and reads nothing today. No gate value moved and no rebake was owed.

## 2026-09-17 (loop lane 1, queue L's landing) — `LOWR`'s `STRP` bit 14 is the VU's, not the disc's

`analysis/re-lowr.md` §1 (`LPTC`) amended. The section read "bit 15 = strip flag → VU word 0x8000,
and 0x4000 on every second vertex" beside "`index & 0x7fff` selects the triple", which invites a
port to look for 0x4000 in the shipped indices. It is not there: over **all 21 shipped sheets on
both discs — 2,893 `LPTC` patches, 415,277 `STRP` u16s — bit 14 is set in exactly 0**, bit 15 in
67,202, and `index & 0x7fff` overruns its patch's `POSI` count in exactly 0. Both 0x4000s in the
cited ELF range belong to the word the loader BUILDS for the VU. Measured by queue L's build
(2026-09-17, lane 2) and reproduced independently at its landing (lane 1) with
`engine/artifacts/lead_tmp/lowr-sheet/strp_bits.py`, which reads only `extracted/`. [V] — a
disc-byte count over the whole shipped corpus, not a reading. Ported: yes — `mungefs::terrain`'s
`unstitch_strips` masks with `& 0x7fff` and splits on bit 15, landed as part of L (`cf7b642`).

## 2026-09-17 (loop lane 1, queue SKA's [RE] half) — the disc alpha-tests NO soldier, and blends one segment

Full result: **`analysis/re-side-alpha.md`** (`[O] UNAUDITED`, design D6's unanswered half). The short
form: over every `modl` in all 28 SIDE LVLs on the two discs, **not one** segment of the 52 soldier models
or the 29 weapon models the port bakes carries the cutout bit `0x02` — so the disc runs no alpha test on a
soldier at all — and exactly ONE segment per game carries the transparent bit `0x04`: `all_inf_wookie`'s
fur (BF1 seg 2, word `0x5`; BF2 seg 1, word `0x10005`), which both games BLEND (`ALPHA 0x44`,
`TEST 0x51001` — Z-tested, no Z write). Everything else is drawn with `TEST 0x50000`, i.e. the sheet's
alpha IGNORED. The state comes from the segment's own `MTRL` word in both games (BF2 `FUN_0058ed70`
723411–723479, reached from the soldier's own chain `soldier` label VA 0x60fb78 → `FUN_00250488` →
`FUN_00234a68` → thunk `FUN_00255db8` → `FUN_00239bb0` → `RedModel` slot +0xc = `FUN_0057cf60`; BF1
`FUN_004cf6f0` 594347–594358 via the class-per-flag-bit ctor `FUN_004aa9f8` and the mode word
`FUN_004abde0` 568939–568954), and **the bone palette cannot reach any state word**: in both games it is
referenced twice and consumed by one DMA call (BF2 `FUN_004975c0` 724020, BF1 `FUN_0039bca0` 594499).
[V] on the lines and on the ELF bytes (both class-label strings, five vtable words, the AREF byte
`0x60c64c = 0x60`, and the TEST/ALPHA immediates straight out of the instruction stream); `[O]` —
one Opus pass plus one Opus verifier agent, no Fable audit. **Ported: YES, 2026-09-24 — queue SKA's
build half (`ff71352`, goldens `8277709`).** Until then the port alpha-tested every skinned segment at its
own `SKINNED_MASK_CUTOFF = 0.5`, deleting 93.44 % of BF2 `all_inf_wookie`'s texels and 8.46 % of BF1
`wookie`'s; now no skinned draw is tested, and the one 0x04 segment per game draws through a skinned
BLEND pass (`bake::soldier::segment_blends`, `render::SkinnedDrawCmd::layer`). The build corrected one
detail, [V] by `segdump`: BF1's wookie segments name `wookieface02` / `wookie` / `wookie`, so "all three
name one sheet" holds for BF2 only (`re-side-alpha.md` §6 item 3). The draw ORDER (§7's last [H]) was
ported by queue SKO (`662ba97`, 2026-09-24): opaque world and skinned draws first, then one far-to-near blend pass.

## 2026-09-24 (loop lane 1, queue TL) — stock SP conquest sets NO time limit → `analysis/re-time-limit.md`

Pointer entry; that file is the record (§0 the answers, §1 the nine hops, §2 the option tables, §3 the
writers, §4 BF2 has no BF1 cull, §5–§6 two finds on the way, §7 what the port owes, §8 what stays [H],
§9 the verification). **Headline:** every one of the 33 BF2 `*_con` missions passes `multiplayerRules =
true` and no `timeLimit`, so `Objective:Start` (objective.luac `main.f7` pc 29–41) takes
`ScriptCB_GetCONMaxTimeLimit()` = `int[0x5fa8c8 + 4·rules] × 60` (0x19b8b0, decomp 43020), and that table
is all 0 in the ELF (file off 0x49b8c8) — so `loseTimer` is never created (pc 108–131) and no conquest
mission runs a round timer of its own. The limit is the Instant Action option `con_timer` (Off, 5..60 min
in 5s; `ifs_instant_options` `main.f6` pc 78–90), written only through `ScriptCB_SetNetGameDefaults` and
Off at every power-on; when set, `GameOptionsTimeLimitUp` gives the win to more tickets and a tie to both
teams. Two finds on the way: the `_con` ticket percentage is a player option (10–500 %), and **Death Star
conquest takes a second ticket from team 1 per team-1 death** (`dea1{c,g}_con` `ScriptPostLoad` pc 25–28).
BF1: no conquest time limit found ([H] — searches). Every load-bearing claim is `[O]`: one Opus pass plus
one Opus verifier agent (25 of 26 claims SUPPORTED, one PARTLY — a file count, corrected), no Fable
audit. Ported: nothing needed by default (the port's stock exits are the disc's); queue **TLO** (the
opt-in timer and the % row, Off/100 % by default) and **DSD** (DEA1's second ticket, a [SWEEP]
re-baseline).

## 2026-09-24 (loop lane 1, queue SPEC's [RE] half) — the specular pass is a reflection-vector lookup into one shared table → `analysis/re-specular-pass.md`

Pointer entry; that file is the record (§0 the answers, §1 which segments, §2 GS state and colour, §3 the
texture coordinates, §4 the two camera frames, §5 the table, §6 Ad and BF2's invisible 0x04 variant, §7 when
it is drawn, §8 BF1 vs BF2, §9 what the port owes, §10 the verification). **Headline:** `FUN_00589ac0`
(BF1 `FUN_004a82f8`) builds no texture: it builds a DL opcode-1000 texture-coordinate matrix, and the pass
samples the disc's one `specular` table (CORE.LVL, 128×128 grey, identical on both discs) at
**s = power/255, t = 0.5 + 0.5·(L̂·R)**, with R = 2(N'·Î)N' − Î computed per vertex by the VU1 program
(BF2 0x0ca–0x0fe, BF1 0x0ca–0x101). The colour is flat and unlit:
- BF2: the MTRL specular RGB, halved into GS units;
- BF1: the SunInfo colour raw, or the material's own RGB.
It multiplies the texel and is added as **Cs·Ad + Cd**, where Ad = the base pass's texel alpha (the main target
is PSMCT32, FBMSK 0), so a `Specmap` sheet's alpha is its gloss mask.

Quirks, all faithful:
- the table is symmetric in t (two highlights, sun and anti-sun);
- the light is rotated by the camera's plain inverse while R lives in the negated-up/back VU view frame, so the
  effective light has its camera-right component flipped: d = 2(X·L)(X·R) − L·R;
- BF2's 0x04 variant weighs by As = 1/128 and is invisible (the pass alpha byte is `int(spec.a/255)` unscaled);
- BF2 skips the pass on far-scene models in a mission (`DAT_0060c2c7`).

Census: BF1 179 records (class 0x20), BF2 469 (bit 0x80; none with 0x100 or 0x02; 22 with 0x04). Every
load-bearing claim is `[O]`: one Opus pass plus one Opus verifier agent (23 of 30 claims SUPPORTED, 7 PARTLY —
folded in — none NOT SUPPORTED), no Fable audit. It also found that the s12/s17 VU1 decoder mislabels every
A = 0x100·k instruction and swaps `SQ` operands (the fresh decoder: `scripts/tools/vu1dis.py`). Ported:
nothing yet; queue **SPEC** (the world pass) and **SPEC2** (soldiers and weapons, skinned).

## 2026-09-25 (loop lane 1, queue WSE's [RE] half) — BF2's `SpecularEnable` and `OscillationEnable` default to 1 → `analysis/re-water-flags.md`

Pointer entry; that file is the record (§0 the answers, §1 the gate, §2 its every writer, §3 when the reset runs, §4 the
disc census and BF1, §5 where the far pass exists, §6 BF2's layer = BF1's, §7 OscillationEnable, §8 corrections, §9 what
the port owes, §10 the verification). **Headline:** BF2's flat-water renderer `FUN_00575fd8` gates its far speckle +
`fogspread` sub-pass and its near cross-faded speckle on `DAT_006615f9` (= `SpecularEnable`, 0xd4ddec5f) — and that byte
is set to **1** by the Water effect's level reset `FUN_005601a8` (ELF 0x56027c), reached through vtable 0x63a6b8+0x24 ←
the effect manager `FUN_00450cf8` ← the "mission" state's enter, before ScriptInit reads the map. Its only other writer is
the parser arm for a key that occurs in **no** disc file (0 of 186 BF2 / 137 BF1 LVLs, positive controls hit). So
`re-water.md` §1/§5's "BF2 draws no speckle and no far fog" had the default backwards: the near speckle shows on all six
BF2 flat-water maps and the far speckle on KAS2; the far fog shows nothing on any shipped map (KAS2's fog is off, the
rest have no far layer or `DisableLowRes`). BF1 has no gate (the hash is nowhere in its ELF). Same pattern: BF2
`OscillationEnable` (→ `DAT_006615f8`, reset to 1) gates the 6 cm bob, and **FEL1 authors it 0** — FEL1's water does not
bob. Corrections: `re-water.md` cit. 8's LAYR/CPAT/INFO lines are in `FUN_005609f0`, not `FUN_00560f40`; `re-fog.md`
§2.1's "fog off → rows zero" holds for the first map after boot only. Every load-bearing claim is `[O]`: one Opus pass plus
one Opus verifier run in the foreground (11 SUPPORTED, 6 PARTLY — folded in, 0 NOT SUPPORTED), no Fable audit. Ported:
nothing yet; queue **WSE** is now the build (bake both flags' disc defaults, the BF2 speckle, the fog rule, FEL1's bob).

## 2026-09-25 (loop lane 1, queue WCOL's [RE] half) — the flat water's colour is a VU1 overlay with a view-angle term → `analysis/re-water-colour.md`
The "microcode 0x515410" HOLD 21 carried is a DMA chain that MPG-uploads a 63-instruction VU1 overlay (BF1 VU 0x74e, BF2
0x758, word-identical). Its routine B colours the flat water's MAIN layer per vertex, unlit: LUT colour + `SpecularColor`·F
and LUT alpha + `SpecularColor.a`·F, F = (1 − max(0, V̂.y))³ from the vertex to the eye shifted 10 m along the light's
horizontal direction; routine A colours the speckle `SpeckleAmbient + SpeckleSpecular · lobe³²` around the sun's
azimuth. The LUT's entries 6..15 run Min → 2·Max − Min, so the vertices' mean is MaxDiffuse (the DIVU remainder makes
the index 5..15). The ocean's vertex RGB is the engine's global white halved = unity (HANDOFF What next 2(d)). Measured
on the PS2 itself (patched save states in PCSX2): zeroing `SpecularColor` removes the teal body (+13..+27 G/B levels on
BF1 KAS2, NAB2, BF2 FEL1); the port's own model run on the PS2 draws the port's grey, mottled water. The remaining look
gap on three maps is the BED under the water (KAS2 sand vs the port's grass; NAB2/FEL1 beds 2–2.5× too bright in the
port). Side finding, [V] disc/RAM/code with the lighting consumer [H]: the port's baked sun direction is (−L.z, L.y, L.x)
of BF1's `FUN_004931e0` vector and (L.x, L.y, −L.z) of BF2's global light — queue SUND. Every load-bearing claim is
`[O]`: one Opus pass plus one foreground Opus verifier (11 SUPPORTED, 2 PARTLY — folded in, 0 NOT SUPPORTED). Ported:
nothing yet; queue **WCOLB** is the build.

## 2026-09-25 (loop lane 1, queue LITE's [RE]) — why the port is brighter than the PS2: per-vertex COL0, prelit models, Static lights → `analysis/re-model-lighting.md`
The PS2 lights a static model's vertex as mat × (Top/Bottom hemisphere + Σ Cᵢ·max(0, N·Lᵢ)) in GS units (BF2 VU1 0x1e6,
0x1bb, handler 0x56b), and then applies the disc's per-vertex COL0, which the port's bake drops. On a plain lit segment
COL0 MULTIPLIES the result (VU 0x521, COL0/128). On a 0x200 "vc-lit" segment it is ADDED (0x537). A model with any 0x200
segment is "prelit" (`modl` loader, model +0x48 & 1): its light set uploads a BLACK ambient and skips every light whose
`Static()` bit (word 0 bit 8) is set (FUN_00597e98 / FUN_0057fd90). BF2 also renders `GlobalLights.Light2` (COR1's "Back"),
which the port never does. BF2's terrain base pass carries no VU lighting at all: its lightmap, plus non-Static point-light
passes. COL0 covers 24–98 % of BF2 world indices (means 52–95 of 128) and 0–91 % of BF1's; the BF2 MTRL diffuse is white
everywhere. BF2's global scale DAT_007c5754 = 0.5 is `Effect("HDR")` (Enable/DownSizeFactor/NumBloomPasses/… preimages
recomputed); its composite (ALPHA 0xff00000029) doubles the frame back, so it is neutral, apart from a bloom.
Measured both ways:
- PS2 patched save states: MYG1's world is black with ambient + sun zeroed; COR1's prelit columns ignore the Static sun
  until its bit is cleared; HOT1's snow ignores both.
- The port with COL0 + the PS2 rules at the same poses: MYG1 floor 52 → 31 (PS2 28); TAN1 frame 137 → 109 (PS2 103, its
  12–13 % clipping gone); COR1 floor 92 → 43 with Light2 (PS2 46).
BF1: the same VU paths; 0x40 segments get a black-ambient set (… its light scale already matches
the port; F15 (Theed's columns) is the sun direction (queue SUND): 97 → 59 with the PS2's vector, PS2 57.
Every load-bearing claim is `[O]` (one foreground Opus verifier: 29 SUPPORTED, 2 PARTLY — folded in, 0 NOT SUPPORTED).
Ported: nothing yet. The builds are queue **LITEB** (models), **TERL** (terrain RE + build) and **FOGN** (BF2's
far-scene-only fog).

## 2026-09-25 (loop lane 2, card VEH0) — vehicles: the census, the class code in both ELFs, the M2 plan → `analysis/re-vehicles.md`
The discs ship 53 vehicle classes on 25 maps, all placed by 336 `com_item_vehicle_spawn` pads. Hover + treaded + BF1
commandhover (21 classes) share ONE integrator — B1 FUN_001eba60 / FUN_001ee040 / FUN_001ec768, B2 FUN_0021ca80 /
FUN_0021f5d0 / FUN_0021d8b8 — with ray-cast spring points (`AddSpringBody`, ≤ 6), gravity × GravityScale and
velocity-matching thrust; its props, offsets and constructor defaults are identical in both games. BF2 differs by a
speed-based turn blend, boost and no strafe→turn fold. Walkers move by the walk animation's root motion (BF2 time-scales
it to MaxSpeed). Flyers use a 5-state machine with a Min/Mid/Max speed law. Seats are `FLYERSECTION` rings: pilot,
MountedTurret ×8, PassengerSlots ×4. Only soldiers enter: an aim cone ≤ 5 m + the target's radius, instant, exit at 9
candidate points. A vehicle has one hull pool: at 0 health the explosion and chunks fire in the same tick, no wreck.
Pads map Atk/Def to team 1/2, follow the CP, respawn after SpawnTime and decay idle vehicles; ExpireTimeEnemy/Field are
read by neither ELF. A turret building is a seat-only vehicle. The aim RE found that `re-aim-assist.md` §3's
acquisition function is the MountedTurret's (a correction note is now at its §3), so the port's soldier magnet skips the
PS2's team/kind filter (card AIMF). Built with four helper agents; one foreground Opus verifier: 38 SUPPORTED, 4 PARTLY
(folded in), 0 NOT SUPPORTED. Ported: nothing yet. Cards: HVRE, VCLS, HOVR, VSEAT (the first drivable vehicle, TAT1's
skiff), VPAD, VBAKE, VDMG, VTUR, VAI, VWLK, VFLY, AIMF; VENT amended.

## 2026-09-25 (loop lane 2, card TERL) — the terrain is UNLIT: texture × COLR at a per-game scale → `analysis/re-terrain-lighting.md`
Both games draw each terrain layer on VU1's UNLIT path with the per-vertex colour taken from `COLR` (never LUMI):
- BF1 (FUN_004828b8) sends c5 << 3 per channel with a pass colour of 0x80808080 (the copy path), so the pixel is
  tex × c5/16.
- BF2 (FUN_0056c9d8) sends c5 << 2 times a pass colour of 1.0 × HDR (FUN_0056f208 → FUN_0057fd50), so the pixel is
  tex × c5/32. The HDR 0.5 reaches the terrain; the composite that doubles the frame runs only while HDR is on, so HDR is
  neutral.
- No ambient, no sun, no Light2 in either game. That is why both Hoths clip in the port, and why the port's per-MAP LUMI
  "neutral" rule is really a per-GAME scale (BF2 FEL1 / NAB2 / YAV1 / POL1 are drawn at 0.22–0.47 of the texture).
- BF2 only: the light set's +0x94 is a PROJECTOR light (a `Texture`-carrying non-Static spot, else directional; off in
  split-screen). Up to 4 non-Static omni lights add falloff passes on the top layer. No shipped terrain map authors
  either kind, so on the discs they are runtime lights (the CP glows in the library).
- Both games: a DTLX detail pass (frame × 2·alpha) on the 3×3 patches round the eye.
- Layers: 4-bit ALPH weights → a 16-byte alpha table, ALPHA 0x44.

Measured:
- BF1 HOT1 patched states: the snow stays at 197.9 with the sun, the ambient or both zeroed; the soldier goes 156 → 24.
- BF2 FEL1 (sun not Static): the ground ignores the sun and doubles with HDR 1.0.
- The port instrument at BF2 NAB2's single-layer floor: PS2 (48,38,31) vs the rule (48,37,29).
- Hoth: 243 → 223 against the PS2's 210. The rest is the unported layer blend [H].

One foreground Opus verifier: 22 SUPPORTED, 1 PARTLY (folded in), 0 NOT SUPPORTED. Ported: nothing yet. Cards: TERL
(build), TDTL (the detail pass), TDYN (BF2's omni / projector passes); a TALPH note.

## 2026-09-25 (loop lane 1, AUDIT of VENT) — a vehicle's team follows its occupants, not its pad → `analysis/re-vehicles.md` §7.1
The audit re-derived VENT's PS2 claims from both ELFs.
- **Held:** the seat ring, the order the seat-switch walks (B1 FUN_0020b5e8), the ≤ 4 passenger clamp and the
  seat-only building turret.
- **Wrong: the team rule.**
  - A new occupant of ANY seat sets the vehicle's live team to its own (B1 FUN_0023c488 / B2 FUN_002f5478; the turret
    and passenger seats resolve to the vehicle).
  - When the last occupant leaves, the team goes back to 0 = neutral. The exception is command vehicles, whose class
    flag is clear: B1 class+0x80, B2 class+0x330 bit 34.
  - A pad gives an empty ordinary vehicle no team (B1 FUN_002b0238 / B2 FUN_003725f0). It records the pad's team apart
    for the AI instead.
  - B1 CanEnter lets anyone into a team-0 vehicle.
- **Also:** §6's HealthType is only the class default (BF1 tauntaun = animal).

One foreground Opus verifier: 10 SUPPORTED (3 qualified, folded in), 0 NOT SUPPORTED. Card VTEAM.

## 2026-09-26 (loop lane 2, card FOLG) — the terrain foliage: FOLG layer bits + the `prp_` foliage file → `analysis/re-foliage.md`
- **FOLG is a 4-bit LAYER MASK per cell** (bit L = layer L grows here), read by B1 FUN_00344d98 / B2 FUN_0042b0c0.
  BF1: always 512² at a fixed 16 m cell (FUN_00343b08). BF2: (gridSize/2)² at 2 × the grid unit (FUN_0042b1a8).
  `re-terrain-heights.md` §8's "2× terrain grid resolution" is wrong for BF1.
- **The layers are the map's `prp_` chunk** (B1 FUN_00344e80 / B2 FUN_0042b1a8). Each `Layer` has a `SpreadFactor`
  (≥ 1: one instance per s×s cells; < 1: trunc(1/v) per cell) and `Mesh()` entries picked by `Frequency`. A mesh is a
  model (`File`), a `GrassPatch` class or a `LeafPatch` class, with Scale, Stiffness (uprightness), ColorVariation, a
  near-fade, Sound, a rustle sound and a collision flag.
- **Placement is deterministic:** one 256-float table (identical in both ELFs) seeded per 32 m block and layer.
  `lead_tmp/re/folg/place.py` reproduces all 2,610 live instances in 9 PCSX2 save states.
- **Draw:** models fade from d to 1.45·d. The tint is (0.7·LUMI + 0.6, +0.35 for models, ≤ 1) × ColorVariation, the
  seed being the float bits of x·z. Grasspatch = one shared template of camera-facing sprites (a disc, sizes, grey
  tint, atlas parts, sine sway), faded over the last 20 m.
- **A path block in `prp_`** (B1 FUN_003489b0 / FUN_003483f8) puts a prop every N m along named paths: the
  tree-billboard lines and DAG1's `hell_bush` hedge.
- **BF2 split-screen** turns foliage off (FEL1 / KAS2 / YAV1) or to 0.6× range through a `sky_` block read only with
  ≥ 2 cameras.
- The Dagobah canopy is LEAF's leafpatches, not foliage.

One foreground Opus verifier: 30 SUPPORTED, 3 PARTLY (folded in), 0 NOT SUPPORTED. Ported: nothing yet. Cards: FOLG
(build: bake + models + path props), FOLGG (grasspatch sprites; also SKIPC's world-object grass), notes on LEAF and
SKIPC.

## 2026-09-26 (loop lane 2, card SUND) — the sun on models: the PS2's own vector, BF1's ((1+n·l)/2)⁴, BF1's sun-less prelit set → `analysis/re-sun-direction.md`
- **One world vector per directional light, brought into model space on VU1** (B1 0x724 / B2 0x72e).
  - BF1: L = (−sin a0·cos a1, cos a0, −sin a0·sin a1) from `SunInfo.Angle` in degrees (FUN_002e4dc8 → FUN_004931e0 →
    0x69c590). FUN_004ac0d8 uploads it unchanged through FUN_0039baa8.
  - BF2: `lght` `Rotation` is stored as q = (x, y, z, w) = (−a3, a0, −a1, a2) (FUN_00596978; `Position` is (x, y, −z)).
    The light's direction is row 2 of FUN_0053ac20's matrix, at +0x50. FUN_004973b8 negates it on upload.
  - RAM matches both formulas to ≤ 0.004° on every map with a state (16 BF1, 17 BF2, Light2 on 5).
  - The port's bake is (−L.z, L.y, L.x) on BF1 (35–76° off) and (L.x, L.y, −L.z) on BF2 (up to 105° off).
- **BF1's term is C·((1 + n·l)/2)⁴** (VU1 0x564 stores (−L/2, ½); 0x1b9 / 0x21f square d twice). BF2's is
  C·max(0, n·l) (0x1e6, MAXx).
- **BF1's 0x40 (prelit) set carries no sun** (FUN_00496248: factor 0, ambient = the black palette entry 0x69c440).
  - This holds in world rendering; a 0x2000000 preview mode lights everything with a fixed rig.
- **Side findings:**
  - Neither ELF reads SunInfo `BackAngle` / `BackColor` / `BackDegree`.
  - BF2 ignores `SunInfo` (Angle and Color).
  - BF2's `SunFlare` has its own Angle (FUN_0046e998).
  - GEO1's "2.8° agreement" was two wrong conventions agreeing.
- **Measured at all 200 PCSX2 poses** (a scratch instrument, byte-identical to main when off):
  - BF1: the PS2 vector moves 68 % of the affected blocks closer (25.4 → 14.2); dropping the prelit sun, 90 % of its
    blocks; all three changes, 82 % (22.6 → 9.3, with the GS-unit sum).
  - BF2: 72 % closer (22.5 → 13.8).
  - The GS-unit light sum is mixed on BF2 → card LSUM.

One foreground Opus verifier: 27 SUPPORTED, 2 PARTLY (folded in), 0 NOT SUPPORTED. Ported: nothing yet. Cards: SUND
(→ build), LSUM (new), notes on BACKL, SUNG, TDYN.

## 2026-09-26 (loop lane 2, AUDIT of SUND) — OK → banner in `analysis/re-sun-direction.md`
The audit re-derived SUND from the ELFs, VU1 and RAM before reading the doc.
- **Held:** every load-bearing claim.
  - Both vectors: RAM ≤ 0.0039° on BF1, ≤ 1e-5° on BF2.
  - BF1's d⁴, BF2's max(0, ·) and BF1's sun-less 0x40 set.
  - The port's offsets and §5's absences.
  - PS2 RAM axes = port axes, measured against the port's terrain on 7 maps.
- **§4's census reproduced within a few points** with the audit's own knobs and metric: BF1 68.2 %, BF2 68.1 %,
  prelit 92.8 %, base → ps2d4s 63.6 %.
- **Precisions** are folded into the banner:
  - BF1 table entries 0x1ab / 0x206 share the d⁴ loops; the table's water-overlay entries (0x74e / 0x76c) are not model
    light terms.
  - SunInfo's third key is ShadowColor.
  - No BF1 material takes the multiply `Prelit` path.

One foreground Opus verifier on the audit's claims: 7 SUPPORTED, 1 PARTLY (the table's scope, folded in).

## 2026-09-26 (loop lane 2, card HVRE) — the hover integrator, transcribed (BF2) → `analysis/re-hover.md`
Statement-level pseudo-code of BF2's hover, which drives every hover and treaded class, each statement with its B2 line:
- the tick FUN_0021ca80; the spring point FUN_0021f5d0; the inputs FUN_0021b470 and the player controller FUN_002b0548;
- the drive FUN_0021d8b8 (both branches);
- the contact FUN_0021fd98 (via the thunk at 0x224b00), its spring hook FUN_0021f508 and damage FUN_0021f978.

VEH0's five open questions are answered:
- The drive itself integrates p += v·dt and q ⊗ dq(ω·dt) and writes E through the SetTransform virtual.
- The carve is ((up·ω_yaw) × v)·dt, capped at |tS − aS|, times m.
- E+0x360 is the controller's full-stick timer (the stick held at turn² + pitch² ≥ 0.9, in s, capped at 1).
- The B2 contact twin is FUN_0021fd98.
- 30·dt·dt = dt at 1/30.

New and load-bearing:
- The visual tilt feeds back into the body (q is built from the tilted matrix; BF1 too).
- BF2's TurnFilter is a slew rate (BF1: an exponential filter).
- Springs compress only from collision contacts, c = min(d·n·up, len)/len. The spring's own query is the water surface
  (`FloatsOnWater`, default on).
- Every shipped hover has 3-6 spring points, so the no-spring branch is dead on disc data.
- Impact damage only for vehicle-vs-vehicle contacts and bottomed spring spheres. Hull contacts get μ 0.25 and
  restitution 0.125.
- The BF2 speeder-bike jump (+0xf88..f98).
- The frame order: drive (task key 0) before springs (key 15).

**PS2 RAM:** g = 0.9667 = 1 − 1/30 on 134 of 138 live hovers in 30 BF2 states. **The TAT1 skiff by hand:** rests at
1.94 m, reaches 16 m/s in 3.3 s, turns at 1.0 rad/s (a numeric model agrees).

The VCLS addendum was re-read and verified. The spring-length factor 0.5 also has an unnamed %f prop, 0x77d6d149, that
no disc class authors; an ODF-order quirk is noted. `game::vehicle_class` matches the shipped data.

One foreground Opus verifier: 46 SUPPORTED, 7 PARTLY (5 citations; 2 folded), 0 NOT SUPPORTED. Ported: nothing yet →
card HOVR.

## 2026-09-28 (loop lane 2, card KASF) — BF1 KAS2's red fog is real and live but never shows; the port's red band is its clear colour → `analysis/re-fog.md` §9
§6's KAS2 row (fog 255,0,0, 1000..2992) is exact, and not an unset default:
- The disc record is `LowResTerrain/FogColor(255, 0, 0, 128)` at KAS2.LVL 0x18f368, the same in KAS2_H.
- A census of 323 LVLs finds (255,0,0) in only 3 FogColor records. BES1's is a dead `SkyInfo` one; BF1's placeholder
  SkyInfo colour is (128,128,128).
- The ELF reads it as R, G, B, and the alpha 128 stretches the far to 2992.1875 (MIPS 0x2e335c–0x2e3480).
- PS2 RAM holds 0xff0000ff and 1000..2992.1875 in all 8 KAS2 save states. Every BF1 map with a state matches its §6 row.

It never shows: nothing drawn at the captured poses is deep enough. A fog-colour swap on the natural ramp moves nothing
beyond reload noise, while a patched 0..400 m ramp turns the far water red.

The port's pure-red band is its CLEAR colour. `clear_color` sets it to the fog colour, and every red pixel turns green
when only the clear changes. The PS2's clear packet writes black (`FUN_0049aa68`, RGBAQ 0), and its frames are dark
there. The port's fog blend is as specified (0.2346 at 1500 m, the ELF ramp × the maintainer's 0.92).

New findings, [V] on the PS2:
- The far water's blend is linear in f and caps at 0.93–0.98.
- f uses planar view depth; the port uses radial distance.
- BF1 KAS2's near models took no fog (contradicts §0.3; card FOGB).

One foreground Opus verifier: 12 SUPPORTED, 3 PARTLY (folded), 0 NOT SUPPORTED. Ported: nothing → cards CLRB (the clear
colour) and FOGB; REFL gains the band's other half.

## 2026-09-28 (loop lane 1, AUDIT of HOVR) — PARTLY → banner in `analysis/re-hover.md`, card HCON
The audit re-derived the drive, the spring point, the hook and the contact from the B2 decomp, the ELF and PS2 RAM
before reading re-hover.md. It checked the port against them.
- **re-hover.md held:**
  - Every equation, the frame conventions and the updater order.
  - RAM: the port's class numbers equal the live class K bit for bit on 138/138 hovers, and rest heights match on
    45 of 47 quiet parked hovers.
  - Corrections folded into its banner: K9 has no constrained form; the contact loop FUN_00160da8 re-queries after
    each push-out; §5.2's ±16 clamp is soldier-only; the bit-3 setter attribution.
- **The port (HOVR) is PARTLY:**
  - §6.19's g = 0 is not built.
  - §6.10's BoostSpeed turn scale is not built, although it is live on every BF2 hover.
  - The terrain contact is an unbounded half-space, and the cut is tested at the surface. POL1's hangar AAT is
    catapulted where the PS2 holds it still.
  - Push-outs are summed from pos0. The TAT1 skiff pops 5.2 m at spawn.
  - All four → card HCON.

One foreground Opus verifier: 20 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED (`engine/artifacts/lead_tmp/audit-hovr/verify/`).

## 2026-09-28 (loop lane 2, card TALPH) — the terrain's texture LAYERS: `ALPH` is a per-vertex map of 4-bit "over" alphas, and both games blend every painted layer → `analysis/re-terrain-layers.md`
The format, B2 FUN_0056bbf0 / FUN_0056e898 / FUN_0056c9d8 (B1 FUN_00481608 / FUN_004852e0 / FUN_004828b8):
- `tern → PCHS → ALPH` holds one RLE stream per multi-layer patch (the byte codec of `mungefs::rle`).
- Each stream is one (P+1)² nibble block per painted layer, ascending; row = z, column = x, low nibble first.
- Single-layer patches carry nothing and get 0xee.
- The walk consumes the chunk exactly on all 27 terrains.

The draw:
- One pass per layer, ascending LTEX index, GS ALPHA 0x44 with As = {0,9,…,118,128,0}[nibble].
- The weights are sequential "over" alphas: the lowest present layer is opaque at ≥ 97 % of vertices on most maps.
- A triangle touching a 0xf ("absent") nibble, or with all three weights 0, is not drawn. BF2 exempts each patch's top
  layer; BF1 does not.
- The quad diagonal alternates by z parity, the height query's split.
- **Mask-0 patches get no layer pass.** Examples: BF1 KAS1 951/1024, KAS2's lake bed, END1 872, RHN2 858, NAB2 855.
  This is why the PS2's KAS lakes show no bed (F16).

The layer UVs:
- SCAL is metres per repeat: 2048 ST = 1.0 via STROW 4112 and VU1 `LOI −4112`.
- AXIS: ≥ 6 mirrors S, ≥ 3 transposes, then 1 / 2 take T / S from the height.
- ROTN is read by neither ELF. BF2's DTEX is never read.
- Cutter patches take their own ALPH / COLR per `POSI` vertex, and treat AXIS ≥ 3 as top-down.

Measured against the PS2 frames, with the layer means × COLR:
- BF1 KAS2 Dock Two: the model gives (116,103,81), the PS2 (127,114,86), the port (46,45,27).
- BF1 KAS1 east harbour: the model gives (62,50,26), the PS2 (59,47,26).
- BF2 KAS2's beaches are its kas2_main_2 layer along every shore.

Side answers:
- KOC is not solved: KAM1's 1024 patches are single-layer, so the y = 0 sea terrain is drawn.
- TERL's [H] that the Hoth residual is the layers: refuted on BF2 (the LITE boxes paint only hoth_main_1). On BF1 the
  layers are part of it: hoth_main_4 shows at the spawn and closes 28–45 % of the gap.

One foreground Opus verifier: 21 SUPPORTED, 3 PARTLY (folded), 0 NOT SUPPORTED. Ported: nothing → TALPH (build) and a
new card TMSK (drop the mask-0 patches, after REFL).

## 2026-09-28 (loop lane 2, card AIMF) — the soldier's magnet is a filtered cone, not a ray → `analysis/re-aim-assist.md` §3
Where: B1 FUN_0025e098 (the PlayerController update) picks the magnet per weapon channel. It runs after the assist core
FUN_0025fea8, so the assist of frame N uses frame N−1's pick.

The pick, per channel:
1. The held target is re-scored first. Its cone widens by π/4 while that channel's fire button is held.
2. If nothing is held, an axis-aligned box query (half-side ExtremeRange, mask 0x1e) gathers eligible objects: alive
   (+0xac bit 3), killable (+0xe0) and `vtbl+0x90`.
3. Each object is scored by FUN_00241b98: `t + 16·d²` inside a small-angle cone, whose radius is r + t·LockOnAngle,
   out to ExtremeRange + r.
4. The best score with a clear LOS ray (mask 0x48, ordnance bodies) wins.
5. The winner is stored at `weapon+0x128` only if it passes the weapon's Target* team/kind bits (default: enemies,
   kinds 0-4) and lies within LockOnRange + r.

The reticle's pick and colour come from the same loop: K+0x30 / K+0x40, red/green/blue in FUN_002d4b50.

New names and numbers:
- Hash 0x8474ea2c = **ExtremeRange**. OrdnanceName overwrites it with the bolt's Velocity × LifeSpan: 120 m for the
  rifle.
- A soldier's bounds are its model box: ctr ≈ feet + 1.05 m, r = 1.39-1.70 m.
- The aim point is feet + 1.4 / 0.6 / 0.2 m by stance.
- The aim origin is 2.104 m above the feet.
- Relation: −1 / 0 / 1 (SetTeamAs*). The default table: own team friend, other nonzero teams enemy, team 0 neutral.
- BF1's infantry blasters: LockOnRange 64, LockOnAngle 1°, TargetVehicle 0.

Checked in RAM on 23 BF1 save states:
- bit 3 matches alive on 1,141/1,141 records;
- the rifle class's C+0xd0/d4/d8 read 120 / 64 / 0.01745;
- every soldier reticle pick is reproduced by the cone model, including a per-channel ExtremeRange bracket.

One foreground Opus verifier: 30 SUPPORTED, 2 PARTLY (folded: the bolt is OrdnanceLaserClass; the relation's c2
branch), 0 NOT SUPPORTED. The BF2 twin is B2 FUN_002b0548: the same loop plus a 0.25-slope HUD pick and cloak /
own-vehicle skips. The old §3 (a ray, no team filter) is the MountedTurret's, now §3.10. AIMF → build.

## 2026-09-28 (loop lane 2, card FOLGL) — the foliage draw flags: 0x80 is the static light set; FEL1's black leaves were the pre-SUND sun → `analysis/re-foliage.md` §16, `analysis/re-model-lighting.md` §12
- **0x80 = "static"** (every foliage model: no disc authors `Lighting("dynamic")`).
  - BF2: `FUN_0057cf60` → `FUN_00597e98` argument 3 skips only the light-region lookup `FUN_0059b2f8`. It is not the prelit
    rule, which keys on the model's 0x200.
  - BF1: `FUN_00493cf0` gives static draws `AmbientColor` and the full sun.
- **The other flag.** BF2 0x1000000 has no reader on the rigid path (ELF masks). BF1 0x800000 batches the segment by
  texture (`FUN_004b1688` / `FUN_004b1a20`) with the same light terms.
- **No two-sided lighting.** VU1 0x1e6 is max(0, n·l) on the stored normal, and the port does the same.
- **Measured.** The PS2's FEL1 instance rows equal the port's (504, error 1e-6); the sun equals `sun_dir`. The FEL1
  caladium is within 1.4 levels of the PS2 on main: FOLG's strip predates SUND. The port owes no foliage code.
- **Side finding (PS2 patched states, BF1 GEO1).** BF1 soldiers take SkyInfo `CharacterAmbientColor` (`DAT_0069c578`);
  static world props take `AmbientColor`. The port lights soldiers with `AmbientColor` → card CAMB. The light regions
  (BF1 `DAT_0069c548`, BF2 `FUN_0059b2f8`) that non-static models get are absent from the port → card LREG.
- One foreground Opus verifier: 17 SUPPORTED, 1 PARTLY (A10's winding counts, folded), 0 NOT SUPPORTED.

## 2026-09-28 (loop lane 2, card KOC) — BF1 Kamino's sea: the PS2 switches the terrain off (`TerrainEnable 0`); the sea is the ocean alone → `analysis/re-terrain-enable.md`
- **The key.** BF1's DomeInfo parser `FUN_002e29e0` resets `DAT_005229a8 = 1`, then sets it from `TerrainEnable` (fnv
  0xae291b6d) (B1 270060, 270354–270356). ELF 0x2e2a50 and 0x2e2d3c are its only two stores. BF2's ELF never reads the
  key.
- **At 0 there is no terrain draw of any kind.** `FUN_00487ad8`'s test at 549130 wraps the patches, the detail pass and
  the far sheet; the far-scene call tests it at 280296. The height query `FUN_00473710` answers −1000, and the terrain
  ray test misses.
- **Disc and RAM.** Exactly BF1 KAM1 and BES1 author it, both 0 (2 of 51 sky files). The PS2's RAM holds 0 on those two
  maps and 1 on the other 13 BF1 maps.
- **The PS2 taken apart** (patched states at F9's pose). With the terrain draw off, nothing changes; with the water off,
  the sea goes black. The sea is the ocean draw alone.
- **Why the port's plate is textureless.** `kam1_water` is flat grey 180, with its foam all in alpha. The port reads no
  `TerrainEnable`. Dropping its heightfield moves the F9 box from (191,214,237) to (20,33,29), against the PS2's
  (17,30,33).
- re-water-alpha §5's [H] is answered in code (the pass would take the texture alpha) and is moot.
- KOC → build (render). New card TENS (M3): no terrain floor on these two maps.
- Side finding for TDTL: the detail pass's gate `DAT_005229b0` stays 1 on retail, and `FUN_00487908` is dead.
- One foreground Opus verifier: 26 SUPPORTED, 0 PARTLY, 0 NOT SUPPORTED.

## 2026-09-28 (loop lane 2, card VDTH) — a vehicle's death: the blast's area damage, the push and the chunk bodies → `analysis/re-vehicle-death.md`
- **One blast function for every explosion** (B1 `FUN_002701a0`, B2 `FUN_0030c8f8`). It gathers up to 20 objects in a
  box of half-side max(DamageRadiusOuter, PushRadiusOuter). Only buildings and terrain block the LOS ray (B1 mask 0x48,
  B2 0x90). Each object's splash virtual gets (Inner, Outer) [V-instr].
- **Falloff** (B1 `FUN_00221530`, B2 `FUN_00268fe0`): `d' = d − w/2` to the victim's collision sphere; 1 inside
  Inner, linear to 0 at Outer. `DamageRadius` is Outer and Inner defaults to 0. Damage × f then goes through the bolt's
  TakeDamage: HealthType scale, friendly-fire scale (ELF 1.0), and AI never hurts a human teammate.
- **The victim's sphere w.** Vehicles: `max(modl SPHR r, the farthest AddSpringBody / ODF collision primitive)` about
  the SPHR centre, reproducing all 16 BF1 vehicle classes in PS2 RAM. Soldiers (RAM): 1.4626 standing, 1.1023
  crouched, 1.6221 prone.
- **Riders and push.** A soldier riding a vehicle takes no splash. Only soldiers and droids are pushed (the base push is
  `jr ra`). BF1 pushes the living; BF2's `PushDeadOnly` (fnv 0x26d8bec1, default 1) pushes only the dead.
- **Chunks.** 40-kg rigid bodies with restitution ChunkBounciness and spin friction ChunkStickiness. The vehicle launch
  is vel × ChunkVelocityFactor + an outward kick. They settle after ChunkTerrainCollisions hits and vanish 3 s (BF1) /
  8 s (BF2) after resting; explosion debris rests 5 s in both.
- Names cracked by FNV: PushDeadOnly, ArmorScale, VisibleRadius (BF2, default 60), ChunkStartDistance,
  ChunkLinear/AngularDamping, ChunkDeathSpeed, AddSpringBody.
- VDTH → build. New card KNOCK: BF1 knockback of living soldiers.
- One foreground Opus verifier: 23 SUPPORTED, 4 PARTLY (folded), 0 NOT SUPPORTED.

## 2026-09-29 (loop lane 2, card REDB) — the PS2's fog amount is a `fogspread` TEXTURE lookup at planar depth, blended on the stored bytes; BF1 KAS2's red band is the port's fog arithmetic → `analysis/re-fog-amount.md`
- **`fogspread` is a texture, not a VU program** (correcting `re-fog.md` §0.1).
  - Every fog pass in both games binds fnv1a("fogspread") = 0x6182a9d0 as its second texture (B1 549414–549415, 594618,
    551406–551407; B2 706800–706801), looked up in the global texture table.
  - `CORE.LVL` on both discs: 64×32, white, alpha 1→248. The GS VRAM holds its palette unmodified.
  - The fog matrix in RAM gives (s, t) = (f, f), with f from PLANAR view depth.
- **The PS2 at the golden's own vantage pose**, with the chase camera held by RAM writes (position + velocity):
  - it draws a thin dark-red line on BF1 KAS2's far land itself (887–1,146 red-dominant px), blue under a blue fog word;
  - the port paints it 3–10× redder. On the natural frame's sheet pixels: stored-byte lerp +1.9 levels of R, linear-light
    mix +21.9, the port today +30.3.
- **The rule, emulated:** the golden ground panel and all 21 KAS2 PXC poses have 0 fog-made red; the vantage keeps the
  PS2's thin line.
- **The curve is open:** fogged bins sit at 0.5–0.65× the table's diagonal, and patched ramps disagree at the same f →
  card FOGC.
- **The white row** is the dome's haze sliver. The PS2 covers it with the mirrored `Reflect` shells: the BF1 sky tick
  draws each mirrored, Y × −(0.85 + 0.006 sin φ) at +0.1 m, before its normal draw (280242–280255) → REFL (value 5).
- **Sweep of all 32 goldens:** only KAS2 (and 2 px of GEO1) has red on far heightfield. 13 sheet maps → new card FARS;
  the 5 sheetless → FARC.
- The sheet's live GS words are the "debug" branch (`DAT_00522a4c/50/54` = 1): DATE on, VU cull (`re-lowr.md` §2.2
  corrected).
- REDB → build: the PS2 fog amount on every fogged draw. New cards FOGC, FARS.
- One foreground Opus verifier: 11 SUPPORTED, 3 with a caveat, 2 PARTLY, 1 NOT SUPPORTED (C6, the pass words; reproduced
  and folded).

## 2026-09-29 (loop lane 2, card REFL) — `Reflect` sky shells: two mirrored copies, one per render target; the sky under BF1 Kashyyyk's water → `analysis/re-sky-reflect.md`
- **The mirror.** For each `Reflect` shell, BF1's sky tick (`FUN_002f2308`, 280242–280255) draws the whole shell twice
  before its normal draw:
  - matrix Scale(1, −(0.85 + 0.006·sin φ), 1) at the shell's own position + 0.1 m;
  - colour `ReflectionColor`: new preimage fnv 0x8a81de8b, sky+0x1c, authored only by KAS1 as (200,200,255).
  - φ += 2.0/30 per tick; measured on the PS2 at 2.0 rad/s (29.9 ticks/s).
  BF2's tick (`FUN_003cb918`) is the same. `re-sky-dome-placement.md` §3's "sun-flicker constants" were these; corrected.
- **Two copies, two targets.** 0x1000031 → segment bucket 3: the MAIN target, before the terrain (list 0 before list
  2), no Z write. 0x20031 → bucket 9: the FAR target, in submission order.
  - The near lake shows the first copy through the Z-written near water.
  - Everything past it shows the second, through the bucket-10 composite (`FUN_004a7098`, ALPHA 0x54).
  - `re-sky-pass.md` §4's "composite at 561641" is a tint; corrected.
- **Proven on the PS2.** NOP-ing each `jalr` (0x2f24f4, 0x2f251c) in a KAS1 save state removes the near-lake reflection
  or the band beyond it, not both. With no water the lake is the far copy bare, black without it; a red
  `ReflectionColor` reddens it.
- **Maps:** only BF1 KAS1, KAS2 and KAS2_H author `Reflect`. `kas_moon` is in no LVL.
- **Port prototype (scratch):** registers to the PS2 far target to −1 px; squash and lift are pinned within 4–19 px.
  It removes the KAS2 ground panel's white row and fills Beach Head's black band with the PS2's dark forest.
- REFL → build (goldens bf1_kas1/bf1_kas2, rebake maps). Notes on TMSK and FARC. New card STARS: `DomeInfo Stars()` on
  BF1 KAS1/RHN2 and BF2 COR1/NAB2, not drawn by the port.
- One foreground Opus verifier: 14 SUPPORTED, 3 with a caveat, 1 PARTLY (C13, the ordered-bucket mechanism; reproduced
  from `FUN_004a94d8` and folded), 0 NOT SUPPORTED.

## 2026-09-29 (loop lane 2, card UAUD1) — BF1 unit fidelity audit: every IA class against the disc, the PS2 and the port → `analysis/unit-audit-bf1.md`
- **Method.** 28 IA mission scripts from the PS2 bytecode; their side-LVL sublevels; EE RAM of 22 save states; the
  PCSX2 class-select screens of all four factions; the port's own roster over its 26 BF1 RONs. Tools in
  `engine/artifacts/lead_tmp/unit-audit/`, reusable for UAUD2.
- **Matches.**
  - The 26 RONs equal the bytecode's teams 1–2.
  - The PS2 fields exactly those classes and instantiates every weapon slot of each soldier.
  - Movement constants match for all 34 fielded classes: 0 differences.
- **Not fielded:** the droideka (13 CW missions; BASE walkerdroid) → DEKA; the 5 hero classes (class objects loaded,
  none spawned in 22 states) → HERO; every local (the RON keeps one class) → MSND/TEAMS; BF1 Mos Eisley (the port's
  `tat2` is BF2's) → B1ME.
- **Gaps:**
  - slots 2–4 → GREN;
  - 13 of 34 primaries are missile/shell/emitter/bullet but fly as bolts → UORD;
  - 6 classes draw another class's gun (the body cache keys on GeometryName alone) → UWPN;
  - OverrideTexture/2 and SkeletonRootScale are unread → USKN;
  - hurt/death voice, AI chatter, low-health loops, foley and jet engines are unplayed → USND;
  - jet and jump packs → JETP;
  - 10 of 34 primaries need bazooka/sbdroid/bdroid clips → ANIM;
  - drop profiles → PWUP.
- **New [V]: where an override texture goes.** BF1's draw `FUN_001be1e8` (65890–65907; vtable 0x52e088) sets model
  group fnv("override_texture") = 0xdc5803e9 to OverrideTexture, and fnv("override_texture2") = 0xb98e11c1 to
  OverrideTexture2. Each disc model segment names its group in an `MNAM` chunk (body; face or pack).
- **New gap, measured: the chase camera.** In 19 of 22 BF1 states the eye is exactly 3.50 m behind feet + 1.8 m, the
  classes' STAND TrackOffset; the other 3 are shorter, still on the view axis. The port uses one BF2 table (stand 3.2,
  crouch 3.0) for every class of both games → CAMS.
- Two sessions, two foreground Opus verifiers:
  - C1–C16: 11 SUPPORTED, 5 PARTLY;
  - F1–F10: 8 SUPPORTED, 2 PARTLY. F6's wording; F7: the camera script took a bystander as the player.

  All corrections are folded. The second session also corrected six first-draft statements (doc §5.2).

## 2026-09-29 (loop lane 2, card UAUD2) — BF2 unit fidelity audit: every conquest class against the disc, the PS2 and the port → `analysis/unit-audit-bf2.md`
- **Method.** 33 `*_con` scripts from the PS2 bytecode; INGAME.LVL and the side-LVL sublevels they read, each class
  resolved the PS2's way; EE RAM of the BF2 save states (class objects and live soldiers); the PCSX2 class-select
  screens of all four factions; the port's own roster over its 31 BF2 RONs. Tools in
  `engine/artifacts/lead_tmp/unit-audit/` (`*2.py`).
- **How BF2 builds a class [O].** The entc loader skips a name that already exists. Otherwise it derives the class
  as a byte copy of its BASE class (`FUN_00250dc8`, via vtable 0x61e8d8 slot 3), then applies each PROP in order.
  Weapon slots are per `WEAPONSECTION` (reset to 0 per class) or the numbered `WeaponName1..4`; a child overwrites
  only its own sections. The RAM class objects match this model: 633 of 633 in 22 states.
- **Matches.**
  - The 31 RONs equal the bytecode's teams 1–2.
  - The port's primary equals the PS2's slot 0 for all 76 class definitions.
  - The constants the port reads match on all 49 fielded classes.
  - 44 of 49 match the chase camera.
- **Gaps:**
  - the droideka is not fielded, though the PS2 fields 1–2 in every CW state → DEKA;
  - 8 classes draw another class's gun → UWPN;
  - 9 classes lack their skin override (the CIS assault, engineer and assassin droids on every CW map; the desert
    rebel on 7 GCW maps), plus the root scales → USKN;
  - slots 2–6 → GREN;
  - 26 of 49 primaries are not bolts → UORD;
  - the droid clip sets and the missing prone / die clips → ANIM;
  - the VOSound voice → USND;
  - jet packs → JETP;
  - camera exceptions → CAMS.
- **New:**
  - CSPD: every BF2 team class turns at 0.35 × MaxTurnSpeed sprinting, jumping and rolling (com_inf_default); the
    port uses the constructor's 0.2 / 0.5 / 0.6. Confirmed in RAM.
  - UNLK: the officer (8 points) and special class (12) are locked on the player's class select, per the PS2 frames.
  - B2JP: BF2 Jabba's Palace is not a port map.
- **Verifier:** 9 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED; every correction reproduced and folded in (§5).

## 2026-09-29 (loop lane 2, card VWLK) — the walker, transcribed (BF2) → `analysis/re-walker.md`
Statement-level pseudo-code of BF2's walker, which moves all 15 walker-family classes (the commandwalker AT-AT / AT-TE
run the same vtable functions):
- the input FUN_0025f4a0;
- the locomotion FUN_0025d5e0 with its clip helpers;
- the body task FUN_00261208 and the legs FUN_00260ba0 / FUN_002a2028;
- the jump FUN_0025b2a8, the contact FUN_0025c2b0 and the pose FUN_0025f050 / FUN_0025f210;
- the class setter's conversions and the ctor defaults.

VEH0's open question is answered: of FUN_0025d5e0's two clock branches, the `& 0x80` one is the **jump** (dead on the
discs: no walker class authors JumpHeight). **The walk is the other:** the clip clock runs at
dt·max(0.5, speed·|stick| / (walk-clip speed·SkeletonRootScale)), where speed is MaxSpeed or BoostSpeed.
- **Live PS2:** an AI one-man AT-ST ran ×0.9000 against ×0.9021 predicted, over 32.5 game seconds.
- **The disc's clips**, decoded with `mungefs::anim`: the AT-ST's walk covers 4.538 m in 0.667 s (6.806 m/s). So BF2's
  AT-ST walks 5.0 m/s and boosts to 10. BF1 plays the clip at the real dt, 6.8 m/s.

New and load-bearing:
- The body turns by a DOUBLE angle each tick, so the top yaw rate is 2·TurnSpeed rad/s (BF1 too).
- BF2 walkers turn camera-relative: the body follows the aim past ±YawLimits, and walks where the left stick points
  relative to the camera (class flags 1/3). K+0xe08/e0c/e10 are tan(87/88/89°).
- The legs trail the body by a unit-complex offset 2h. It is applied to the pose root, stepped out by the turnl/turnr
  clips at TurnThreshold, and decays 10 % a tick while walking.
- The root motion moves along the legs' forward. Only the drawn walker interpolates its clips; an undrawn one steps in
  whole keyframes.
- The feet cast 2·LegRayHitLength rays. One pair lifts to within 0.1 m and falls at 3 g; two or more also pitch (and
  with the flag, roll) a named bone. The entity stays upright.
- Terrain wall probes run ahead of the body.
- The contact: stomp, terrain support, vehicle impact damage, horizontal building push-out; the generic push-out
  follows the rest.

**PS2 RAM:** 19 walker entities in 15 BF2 states confirm the setter conversions and the anim table.

One foreground Opus verifier: 44 SUPPORTED, 4 PARTLY (folded), 0 NOT SUPPORTED. Ported: nothing yet → cards WALK (the
integrator, AT-ST first) and WSKN (the skinned walker and its clips).

## 2026-09-29 (loop lane 2, card UTAS) — the far scene draws a model's LOWD: UTA1's sinkhole far wall → `analysis/re-far-lowd.md`
`[O] UNAUDITED`. At the walkabout's con_cp2 eye, bearing 0, the PS2 shows the sinkhole's far cliff; the port shows sky
over clear-black. The wall is `uta1_bldg_cliffs`'s LOWD model: LOD0 is only the near half-cylinder, the LOWD is the
whole ring.
- A gmod with a LOWD gets LOD-record flags 3 (`FUN_001b5000`; BF1 twin `FUN_001a91e0`).
- Flag 1 queues it for the far scene whenever its sphere reaches past `NearSceneRange[1]·S` (`FUN_005af6b0`). Flag 2
  sends it down the far walker's fade branch, which draws LOD index 3, the LOWD slot (`FUN_005b0330` → `FUN_001c5610`).
- The far target shows through beyond `NearSceneRange[2]` (55 m on UTA1).
- The near scene's LOD pick (`FUN_005ace40`) is a per-level triangle BUDGET, not a distance. That answers part of card
  LOD.

**PS2:** three patched states each remove the wall and leave the port's exact picture: LOWD slot → LOD0, flags 3 → 1,
and a NOP on the fade-branch `jalr` 0x5b0640. The controls and a NOP on the opaque branch keep it. The BF2 player
teleport by RAM: E+0x150 position, E+0x4c0 velocity.

**Census:** of 515 LOWD geometries on 32 maps, only UTA1's cliffs carry large LOWD-only geometry (1.04 M m²).

One foreground Opus verifier: 13 SUPPORTED, 1 PARTLY (fixed), 1 wrong citation (fixed). Ported: nothing yet → UTAS
phase build (bake the LOWD extension, draw it beyond `NearSceneRange[1]·S`).

## 2026-09-29 (loop lane 2, card JWAL) — soldiers through walls: the wall pass's floor exemption → `analysis/re-soldier-walls.md`
`[O] UNAUDITED`. The maintainer's KAS2 jump-in is at City Hall. `kas1_bldg_hut_grand` stands on four SOLID `prim` cylinders
(r 9.25, y 13.0–17.0), with bamboo walls and a plank ceiling; the inside is terrain grass.
- **The port (M1), reproduced to the tick.** The wall pass pushes the stepped capsule (feet + 0.6) and skips a
  contact with |n.y| ≥ 0.5 as a floor. A jump lifts the lower sphere over the rim, so at tick 42 the rim normal is
  (0.82, 0.57): skipped. The ground probe (feet + 0.6) never reaches the top. The soldier falls inside onto the
  terrain.
- **General.** 8,903 of 19,440 City Hall approaches leak (all jumps; from 24 m, 4 walks up the stairs too). On flat
  ground a BF1 soldier walks through 0.5–0.75 m boxes and sheets (sprint 1.0) and jumps through ≤ 2.5 m. 1,072 /
  1,279 soldier prims on the 32 maps stand in the two bands.
- **M2.** At BF2 speed a sprint-jump (0.585 m a tick) tunnels through mesh sheets of any height.
- **M3.** The LEAK census's own KAS2 control (the cp5 hut) is a third mechanism, a 0.84 m push INTO the hut.
- **PS2 (PCSX2, 10 trials at the same lobes):** no way in. The soldier stops 0.72 m off the wall. The wall zeroes the
  into-wall velocity and keeps a slide, so a jump there goes straight up. The BF1 jump skips gravity on its takeoff
  tick, giving an apex of 1.91 m against the port's 1.65. A RAM teleport needs C+0x198 as well as C+0x228.

One foreground Opus verifier: 15 SUPPORTED, 2 PARTLY (both folded in), 0 NOT SUPPORTED. One claim checked after it
was wrong (City Hall's planning hubs DO sit inside the lobes' footprint) and is fixed. Ported: nothing yet → JWAL
phase build (the ledge rule, the velocity slide, the AI probe); cards WPSH (M2 + M3), JARC (the jump arc), SRAD (the
standoff).

## 2026-09-29 (loop lane 2, card DSHN) — chrome droids: the specular pass's gloss mask → `analysis/re-spec-gloss.md`
`[O] UNAUDITED`. The port's droids are chrome because the bake throws away the weight the PS2 puts on the specular
pass. The PS2 adds the pass as `Cs·Ad + Cd`, and Ad is the base pass's frame alpha:
- BF1 `FUN_004b2310` and BF2 `FUN_0058ed70` send the base colour halved, so its alpha is 0x80;
- TEX0 is MODULATE;
- the palettes hold (disc a·129)>>8.

So Ad = the texel's alpha, a dark gloss mask on every droid sheet (SBD mean 34/255). The port bakes those sheets at
alpha 255: its side/vehicle/map bakes keep alpha only for 0x02/0x04 draws. `fs_spec` therefore weights the add 1.0.
- **PS2 (PCSX2 patched states: the pass record's ALPHA C → FIX):** with the weight forced to 1.0, the BF1 KAS2 SBD and
  the BF2 KAM1 battle droid go chrome, exactly the port's look.
  - SBD luma p95/p99: 131/189 as is, 222/244 at weight 1.0, port 217/244.
  - The per-pixel weight is textured, mean 0.14 (BF1) / 0.13 (BF2).
- **Port prototype** (the disc alpha written back into the sheet): the pass's add matches the PS2's (+5.5 vs +5.9).
- **Scope:** 68 of the 151 sheets a pass segment draws lose their gloss — droids, the dark trooper, BF2 rebels and
  AT-AT crew, every 1st-person weapon sheet, a speeder, and 29 world sheets (BF1/BF2 KAM1 decks, DEA1, UTA1…).
- **History:** chrome since SPEC2 (`c231edb`), not CAMB.
- **Side find:** the port's un-passed droid base is darker than the PS2's (26 vs 54 mean on KAS2) → card SBLT.

One foreground Opus verifier: 11 SUPPORTED, 2 PARTLY (both folded in), 0 NOT SUPPORTED; it also verified the
palette rescale in EE RAM. Ported: nothing yet → DSHN phase build (keep the alpha of every texture a pass segment
draws, at the side, vehicle and map/common texture decisions).

## 2026-09-29 (loop lane 2, card LEAF) — leaf patches: the sprite clusters on trees' hardpoints → `analysis/re-leafpatch.md`
`[O] UNAUDITED`. The PS2's leafy canopies (BF2 YAV1, DAG1, NAB2, KAS2, COR1's planters; BF1 NAB1, NAB2, YAV1, YAV2)
are the `leafpatch` class, which the port does not draw.
- **Placement.** Props attach it: `AttachOdf` sets a current class, each `AttachToHardpoint` appends it at the
  model's hardpoint (cap 30 / 16). The current class is never reset between classes, so DAG1's hardpoints listed
  before `AttachOdf` inherit the previous class's. World instances (NAB2 vines, COR1's 37 planters), path props (DAG1
  hellbushes) and FOLG `LeafPatch` layers (YAV bushes) add the rest. Every one of 3,078 live objects in 9 PCSX2 states
  is accounted for from the port's manifest + the disc skeletons.
- **The cluster.** One sprite template per class from its `Seed`: a half-ellipsoid, a box or a branching vine; grey
  shaded by the sun; transcribed and reproduced for all 23 live templates. Each object draws it under a matrix that
  keeps only the up axis, camera-facing squares with a per-sprite twist, a wind sway on the top edge, a tint in
  [0.7, 1], faded by MaxDistance, NearSceneRange and near the camera.
- **PS2 (patched save states).** The leaf TEST register is proven (never-pass removes only the leaves); the draw
  blends, and its below-threshold texels (FB_ONLY) carry most of BF2 YAV1's fading canopy, so the port must blend. BF1
  leaves are unlit; BF2 leaves are lit (lights off: −70 %).
- **Side finds:** NAB1's far round crowns are `nab_prop_trees2LOWD`'s crown billboard, not leaves → card LOD; BF2
  YAV1 has the FOLG `LeafPatch` layers too (re-foliage §8 corrected); the grass's GS state reads the same way and
  BF2 grass uploads lights too → card FOLGB.

One foreground Opus verifier: 27 SUPPORTED, 2 PARTLY (both folded in), 0 NOT SUPPORTED; it reran the RAM checks and
wrote the true cross-class attach replay. Ported: nothing yet → LEAF phase build; cards LEAFF (falling leaves), FOLGB.

## 2026-09-30 (loop lane 2, card FXP) — particle effects: the `fx__ ParticleEmitter` format, runtime, draw and spawn keys → `analysis/re-fx.md`
`[O] UNAUDITED`. Muzzle flashes aside, every impact, explosion, trail and vehicle effect the PS2 draws is an `fx__`
`ParticleEmitter` tree the port does not read (`game::fx` draws PROVISIONAL quads).
- **Format.** NAME = fnv20(effect name) + a munged config tree; 63 keys on the discs, 56 named (11 by brute force);
  the parsers' template layout (B1 0x224 B, B2 0x1b4 B) matches the PS2's RAM field by field (BF1 NAB1 608/608, BF2
  HOT1 856/856). BF1 128 effects / 701 emitters (COMMON.LVL holds 109), BF2 292 / 1702 (INGAME.LVL 162).
- **Runtime.** Bursts until MaxParticles are emitted IN TOTAL (a lifetime budget); Spread / normalised-Circle spawn;
  Transformer chains in closed form (Accelerate is a total Δv per node; Size/Rotation `Move` ignores its value);
  FadeInTime fades the emitter's first bursts in. BF1 draws from 20 quantised variants per spawner; BF2 seeds each
  particle and caps MaxParticles at 128.
- **Draw.** PARTICLE = a screen-aligned square √2·size′ wide (VU1 overlay decoded), size′ = size·(1.2…1.6) by
  distance; BF1 ANIMATED a fixed 4×4 atlas, BF2 a FrameSize-in-texels grid with `Looping`; BILLBOARD a world-axis
  plane by ⌊rot⌋; SPARK a velocity ribbon. ALPHA 0x44 / 0x48, no alpha test, depth tested, never written, never fogged,
  vertex colour ×1. Far fade 50 / 150 / 500 m by BoundingRadius; MinLodDist / MaxLodDist are never read.
- **Spawn keys.** Bolt impacts pick `ImpactEffect{Soft,Rigid,Static,Terrain,Water,Shield}` by the hit's collision
  class and spawn with world axes; explosions spawn the class `Effect` (BF1 within 60 m) or `WaterEffect`;
  `MuzzleFlash` is NOT an effect (three hardcoded presets → card FXMF); the laser bolt is its own draw → card FXB.

One foreground Opus verifier: 34 SUPPORTED, 6 PARTLY (all folded in), 0 NOT SUPPORTED; it extended the RAM check to
every field and decoded the VU1 sprite overlay end to end. Ported: nothing yet → cards FXP1 (parse + bake), FXP2
(runtime + draw), FXP3 (spawn hooks), FXMF, FXB; FXP closes as the RE.

## 2026-09-30 (loop lane 2, card FXB) — laser bolts: the core and glow ribbons, the bolt's light, HOT1's pink wash → `analysis/re-bolt.md`
`[O] UNAUDITED`. The port draws bolts as flat boxes; the PS2 draws the ordnance class's own two ribbons, and BF2 lights
the ground under every bolt.
- **Keys.** LaserTexture / Width / Length / GlowLength / BlurLength / LaserGlowColor / FadeOutTime (+ BF2
  EndLaserGlowColor) on the laser class, LightColor / LightRadius (+ BF2 EndLightColor, FadeLightColor) on the bullet
  base; LaserLength also resets GlowLength = 3L and BlurLength = 15L (≤ 128), so PROP order matters. The layout matches
  the PS2's RAM field by field (BF1 NAB1 20 objects, BF2 HOT1 45, BF2 GEO1 40).
- **Draw.** Two camera-facing tapered ribbons built on the CPU (`FUN_0047e108` / `FUN_00569328` mode 4 latches the head,
  mode 5 builds the strip): core = LaserTexture, NORMAL blend, half-width w → w/2 over min(len, LaserLength) plus a w cap
  ahead of the head; glow = `lightglow`, ADDITIVE, 2w → 1.6w over min(len, GlowLength); w = LaserWidth·(0.9·fade + 0.1);
  U along the bolt; depth tested, never written, never fogged, no distance fade. BF2's glow fades to EndLaserGlowColor
  over the damage-falloff window (its Republic rifle bolt turns red late in flight).
- **Light.** BF1 bolts make none (the fields are dead; the terrain light pass is gated off). BF2 bolts own a non-Static
  omni light at the midpoint of the last step; the terrain adds `com_sfx_omniproj` projected down from 5 m above it — a
  disc of radius ≈ 0.98·(5 + dy) m, × max(0, 1 − dy²/r²), vertex alpha 0.25 — HOT1's pink wash. Proved on the PS2
  (zeroing LightColor removes it; zeroing the glow does not) and predicted pixel by pixel from RAM (corr 0.93).
- **Side find.** The port's 4-bit texture decode is nibble-swapped (every 4-bit texture's horizontal texel pairs).

One foreground Opus verifier: 23 SUPPORTED, 2 PARTLY (both folded in), 0 NOT SUPPORTED. Ported: nothing yet → FXB
build (the ribbons), new cards FXBL (BF2 bolt light on the terrain) and NIBL (the 4-bit texel order).

## 2026-09-30 (loop lane 2, card FXMF) — the muzzle flash: a tinted additive model at the fire point → `analysis/re-muzzle-flash.md`
`[O] UNAUDITED`. The port draws a PROVISIONAL quad; the PS2 draws a model.
- **Draw.** Each round fired restarts a timer on the weapon (end = now + FlashLength, game clock). While it runs, the
  weapon's render draws `muzzleflash` / `com_sfx_muzzleflash` (4 crossed planes, 1.056 m along +Z; a white texture whose
  alpha is the flame) at the fire point (`FirePointName`, default `hp_fire`, of the `GeometryName` model), its +Z along
  the aim, scaled by FlashRadius (cross products unnormalised: the width shrinks by cos(pitch)), tinted by FlashColor with
  alpha min(A, 2A·t). ADDITIVE, depth tested, no depth write, unlit, unfogged.
- **Keys.** `MuzzleFlash` = three presets (small / med / large: R 0.6 / 1.0 / 1.8, L 0.1 / 0.15 / 0.3); later keys override.
  Every infantry blaster sets FlashLength 0.025 s (under one 30 fps frame), so each shot's flash lives exactly the frame of
  the shot. FlashWidth and FlashLight* are parsed and never read (scans, and PCSX2 knock-ins: no light, no width).
- **Who.** Soldier-held guns only (and the first-person view): no vehicle / turret / building gun and no sabre ever draws it.
- **Port gaps.** The bake drops the flash texture's alpha (ALPHA RULE v2 rule 2) and carries no weapon fire points.

One foreground Opus verifier: 18 SUPPORTED, 2 PARTLY (both outside the flash's path, folded in), 0 NOT SUPPORTED. Checked in
PS2 RAM (fields, model pointers, fire points, t = 1 in shot frames) and on the PS2 (knock-ins). Ported: nothing yet → FXMF
build; new card ADDM (the PS2's additive model segments the port draws opaque or alpha-blended).

## 2026-09-30 (loop lane 3, card GLOW) — self-lit segments, the HDR bloom and the DomeInfo Filter → `analysis/re-glow.md`
`[O] UNAUDITED`. None of the three is in the port.
- **BF2 self-lit.** On the 13 maps whose `Effect("HDR")` is enabled, a segment with MTRL 0x10 and not 0x04 (245 on the
  port's maps, all unlit) draws with ALPHA 8 (Cs + Cs·As/128) and colour 0xff404040: ×(1 + As/128), up to ×2.98, the
  texture's alpha is the glow mask. 0x14 segments skip the HDR ×0.5 (×2 on screen). Measured exactly on the PS2 (MYG1's
  corridor strips, frame read from SW VRAM with Softness off).
- **BF2 HDR bloom.** Threshold on the half-scale frame's luma (Rec.601 fits the PS2's own masks on 99 %) at
  GlowThreshold·255, 4× downsample, NumBloomPasses × 2 five-tap passes (FIX = int(int(MaxTotalWeight·127)·0.2)), added
  after the ×255/128 composite; per-map values on the disc; split screen turns it off. The cascade reproduces the PS2's
  MYG1 bloom within ~10 %.
- **DomeInfo Filter (both games).** Every BF1 map (and BF2 TAT2) runs frame += Intensity/128 · max(0, frame↓2 − Threshold):
  BF1 TAT1 +25 frame luma on the PS2; applied to the port's frames it closes TAT1's −12.8 median luma gap (card TATL).
- Softness (a 47 % blur-lerp on every map) is SD softening, not ported. BF1 has no per-segment glow and no HDR.

One foreground Opus verifier: 15 SUPPORTED, 6 PARTLY (line numbers, two default-taken outer gates, the filter's int
offsets, a census count — folded in), 0 NOT SUPPORTED. Ported: nothing yet → GLOW build (glow + bloom); new card BFLT
(the Filter).

## 2026-09-30 (loop lane 3, card BHUD) — BF2's HUD: its layout, art, fonts and the PS2's screen mapping → `analysis/re-hud.md`
`[O] UNAUDITED`. None of it is in the port (the port's HUD is an egui placeholder).
- **Where.** The whole BF2 HUD is data: five `hud_` config chunks in BF2 INGAME.LVL (`hudtransforms`, `1playerhud` …
  `4playerhud`; DATA/SCOP; 145 of 156 property names recovered by FNV, tools in `engine/artifacts/lead_tmp/re/bhud/`). Art:
  INGAME `tex_`/`modl` (baked already) + five BF2 COMMON sheets, CORE `white`, side-LVL `hud_*` meshes; fonts: four CORE.LVL
  `font` chunks (226 glyphs, texel boxes). Weapon icons are 3D meshes keyed by the class's HUDTag, else its own
  GeometryName, else its name, through the `hudtransforms` table (a detpack switches to `hud_detpack_plunger` while charges
  are out). The minimap image is the map's `<world>_map` masked by `map_mask`. BF1 has no `hud_` chunk (its HUD is code).
- **How.** One perspective camera per viewport (60° × 46.8°, a 4:3 frustum); the HUD root scales by 1.23 / 1.3 and pushes
  to 1.24·D: a Viewport fraction lands at display x = −0.0059 + 1.0139·fx, y = −0.0242 + 1.0484·fy; a native px or glyph
  texel = H/378.7 × H/427.3; a HUD mesh unit = 0.2129 H × 0.1887 H. Checked with no fitted parameter on the HOT1 frame
  (bars and counts ±1 px, the reticle 76 × 70 vs 76.2 × 73.4). Z 0 in front; fader in / hold (default forever) / sustain /
  out; colour eases to Color and pulses to ColorChange; Shadow = a (+1, +1) black copy at half alpha.
- **Feeds.** team1 = the player's team (blue), team2 = the other (red), pulse period 1/bleed; ammo = clip + reserve as one
  number; the 1-player HUD has no CP bar (4-player only; 1-player shows capture on the minimap).

Two foreground Opus verifiers: 20 SUPPORTED + 1 PARTLY (a writer list) of 21, then the weapon-mesh key PARTLY (corrected
rule folded in) and the NameMesh fallback SUPPORTED. Ported: nothing yet → BHUD build (bake), new cards BHUD2 (draw) and
BMAP (minimap + world markers, RE first); a note on RETC (the reticle is BHUD2's Model3D group).

## 2026-09-30 (loop lane 2, card BMAP) — BF2's minimap and world markers: the Map and Target elements → `analysis/re-hud-map.md`
`[O] UNAUDITED`. None of it is in the port yet.
- **Minimap (Small view).** Heading-up and centred on the camera eye; radius = the controlled entity class's
  MapViewMin..MapViewMax by horizontal speed over MapSpeedMin..MapSpeedMax (soldiers ±50 m always; flyers 300 / 750;
  ground vehicles the defaults 125→250 m); u, v = (X − cx, Z − cz)·0.5/R on a 0.22·448-px backdrop, +X right, +Z down,
  turned by −heading, Scale 0.9, at PositionSmall; posts clamp to the rim, units outside it hide.
- **Map image.** `<world>_map` spans the Large view's square: the `mapbounds` region (all 16 BF2 maps; TAN1's yawed box
  gives R −89.43, a mirrored image) or the posts' box + 100 m (BF1's only rule, `FUN_002cc128`); the Small view samples
  the sub-square round the eye. Mission `SetMapNorthAngle` turns the Large / Spawn maps (COR1-GCW, NAB2 180° + the
  pointer flag; TAN1 180°).
- **Icons.** Posts = the CP class's MapTexture × MapScale × 6 px, friend (1,86,213) / enemy (200,100,100) / neutral
  192 grey, a 1 Hz blink toward the capturing team, the post number fading toward the rim; units = MapTexture ×
  MapScale turned by heading, enemies only when detected; Lua markers; FOV wedge; north pointer on the image's −Z edge.
- **World markers (Target).** 8 + 16 styles by on-screen × line of sight × kind; off-screen markers ride the ±0.9 NDC
  square turned toward the target; scale MinScale→MaxScale by distance; lock-on = four corner brackets. Which entities
  get markers (the tracker feed) is not traced.
- **Checked on the PS2.** Predicted from each state's own RAM, 263 of 264 unoccluded post icons in 70 walkabout states
  (16 maps) within ±3 px (mean ~1 px); the backdrop image best at the model's scale on 8 of 10 maps (NCC 0.70–0.94);
  the Large view (two new captures) = the HUD mapping × 0.948 about the screen centre (±1.7 px; cause [H]).

One foreground Opus verifier: 20 SUPPORTED, 4 PARTLY (speed is horizontal; the modeToggle / hideCPs details; the
vehicle-mode marker scale; the checker reads RAM records), 1 NOT SUPPORTED (I claimed the `mask` key unread; Ghidra
prints its hash as `-0x1888ba97`) — all folded in. Ported: nothing yet → BMAP build (after BHUD2); new card TMRK (the
marker feed RE).

## 2026-09-30 (loop lane 3, card HHUL) — which of a hover's bodies meet the terrain and buildings → `analysis/re-hover.md` §13
`[O] UNAUDITED`.
- **Only the spring spheres meet the terrain and buildings** on every shipped hover, in both eras. The one addition is
  the BF2 fighter tanks' `p_cube1` box, which also meets buildings.
- **Why.** Every `AddSpringBody` line (and `SetSpringBody`, hash 0x577b739e) names its sphere in `BuildingCollision` and
  `TerrainCollision` (B2:137561-137564, B1:102129/102037). Those handlers overwrite the subset with the bodies named
  since `GeometryName`, so the geometry's default membership is gone.
- **Independently, a coll mesh is inert on the moving side.** Its terrain slot and its pair slot return 0, and BF2 skips
  a self body with byte +0x45 set.
- **In a building, a hover meets the building's VEHICLE subset** (B1 +0x98 / B2 +0x110). The port queries the soldier
  subset. Of 22,258 base-world instance-bodies, 7,967 are soldier-only and 1,477 vehicle-only.
- **The PS2's sphere-vs-terrain** (FUN_0016c6c0 / FUN_0016b7d8): one height sample under the sphere's lowest point;
  contact when 0 ≤ d = h − c.y + r ≤ 2r; depth d·n.y. That is r(1 − n.y) shallower than the port's plane rule on slopes.

**PS2 RAM:** 18 of 18 live hover classes (343 sightings, 347 states) hold exactly the replayed subsets and body lists.
One foreground Opus verifier: 15 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED, all folded in. The PARTLYs were the
`*CollisionPrim` keys (the qualifier) and a class copy that also reads the TerrainCollision list.

Ported: nothing yet. Next: the HHUL build (the hull = `p_cube1` on two classes, and the sphere rule); new card VSUB (the
vehicle subset).

## 2026-09-30 (loop lane 2, card STALL) — why rounds stall: every stuck unit classified, set beside the PS2 → `analysis/re-round-stalls.md`
`[O] UNAUDITED`.

**The instrument.** A read-only stall-episode census over main 027dad9's sweep: 9 seeds × 32 maps, 60,000 ticks. Eight
maps miss the maintainer's line:

| map | finished /9 |
|---|---|
| KAM1 | 0 |
| POL1 | 0 |
| DEA1 | 4 |
| BF1 KAS2 | 6 |
| MUS1 | 7 |
| GEO1 | 8 (one run at 51,600) |
| MYG1 | 8 (one run at 46,920) |
| BF1 YAV1 | 8 (one run at 24,240) |

Seven causes of the port's own, none of them seen in live PCSX2 runs of BF2 KAM1, DEA1 and POL1 (16 AI a side, 420 s
each):
- **C1, falls.** KAM1: 270 of 276 deaths are the sea. The PS2: 2 of 123, and 3 of 26,529 samples below the decks.
- **C2, the wrong floor of an XZ graph.** KAM1 rims, POL1 hangar ramps, TAT2's raised post, MUS1.
- **C3, DEA1's blast-door rig.** Its bones bind at ±5 m, so the opened leaves slide into the doorway. PS2 soldiers cross
  it 21 + 18 times, all through the centre.
- **C4, a single-shot weapon fires once per trigger hold.** The PS2 taps:
  - BF2 non-sniper: `FUN_002bb1b0` → `FUN_002baf70`, one frame then re-arm at U(0.2x, 2x),
    x = max(0, 1 − lerp(0.1, 0.9));
  - BF2 sniper: `FUN_002baa38`, U(2, 3) s;
  - BF1: `FUN_0023cd40` → `FUN_0023cc20`, a fixed 4-call cadence, plus `FUN_0023c698`'s U(2, 3) s for snipers.
- **C5, doorways approached off the arc.** POL1's radar door, GEO1's bunker.
- **C6, last legs into walls.** NAB1, MYG1.
- **C8, parked AT-STs at KAS2's Empire spawn.**

Plus the rule that makes any of them fatal: **C7**, the bleed clamps at alive = tickets (re-tickets-spawn §1.3). And one
scoring artefact: **C9**, crowds on one spot.

One foreground Opus verifier: 17 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED, all folded in. It found the BF1 twin my draft
missed. Also fixed: `re-ai-difficulty.md` line 61 (bit 0x35 → 0x32) and line 207 (the tap is not a hold).

Ported: nothing. Six build cards: STTAP, STDOR, STFAL, STLVL, STARC, STEND.

## 2026-09-30 (loop lane 3, card BPNT) — BF2's local lights: the sources, the per-draw gather, the VU1 term → `analysis/re-point-lights.md`
`[O] UNAUDITED`.
- **Four sources, one light list.**
  - Every non-stub `lght` chunk: all top-level, so every mode loads them; mostly Static. `PS2Overbright` packs the
    colour ×255 instead of ×128, i.e. twice as bright on models.
  - EntityLights attached by `AttachOdf` (OmniRadius 0 = no light).
  - The CP hologram's non-Static omni: r 8, in the viewer's team-relation colour — own (1,86,213), enemy
    (200,100,100), neutral (192,192,192).
  - Runtime: bolts, explosion LightFlash (min(1, 20c), 3 × radius, +3 m), flags and mines.
- **Every model draw** (static FUN_0057cf60 and skinned FUN_001de2a8):
  - ≤ 4 omnis + ≤ 1 spot, the spot uploaded as a point with no cone;
  - chosen by lum·min(1, r²/4d²)/max(1, e²/3600) among lights whose sphere meets the model's;
  - prelit models and the terrain skip Static lights;
  - VU1 per vertex adds C·mat·((1 + N·L̂)/2)²·(1 − min(1, d²/r²))².
- **PS2 knock-outs** (code patch 0x57fed4 / 0x57fecc: no model point lights):
  - UTA1's yellow floor 103/93/55 → 58/51/46 (the port: 57/46/40);
  - TAN1's pink wall gone (the enemy CP's light);
  - a soldier's blue gone (his own CP's);
  - UTA1's red pool = the lamp's Static red spot.
- **Not light.** The lamps' red haze (lens flare + light cone) and DEA1's CP column (the hologram beam) → card LFLR.

Two helper readers; one foreground Opus verifier: 18 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED, all folded in.

Ported: nothing. BPNT → build; new card LFLR; TDYN closed into BPNT.

## 2026-09-30 (loop lane 2, card STHOP) — how a soldier follows the ground: ramps, crests, edges → `analysis/re-soldier-ground.md`
`[O] UNAUDITED`. Fills the gap `re-collision-layers.md` §4 left open (the soldier's response, "HOLD 13").
- **No snap in either game.** While grounded the integrator steers the velocity along the plane of the last floor
  contact, so a ramp is walked with a real vy.
  - BF1 `FUN_002098b8`: T1 = (s+0x200) × N, N = s+0x960.
  - BF2 `FUN_002480a0`: T1 = (s+0x120) × N, T2 = N × T1, N = s+0x4b0.
- **The grounded bit** (BF1 s+0xbfd bit 1, BF2 s+0xa74 bit 1) is cleared after the integrator every tick and set
  again by the collision response (BF1 `FUN_00202f38`, BF2 `FUN_0023cfd0`).
  - The contact is on the foot sphere `coll_body_lower`, r 0.57 at (0, 0.57, 0.15).
  - It is floor when n.y > 0.6 and the approach speed is < 0.1.
  - The push-out takes 0.02 off its upward part (BF1 0x203c48, BF2 0x23e11c), so up to 2 cm of overlap stays and
    keeps the next contact.
  - The post hook (BF1 `FUN_00203e70`, BF2 `FUN_0023e380`) grounds on the mean of the up-facing contacts.
- **Airborne:** N = up, gravity −18.
  - BF2 stand/sprint → falling (8) after 1.0 s, with full ground acceleration until then;
  - BF2 crouch stands up, and prone falls, after 0.3 s;
  - BF1 stand → falling (6) after 0.7 s, or 1.0 s for a stance ≥ 5 s old, with ×0.2 air acceleration.
- **Slopes:**
  - BF2: ×0.1 control under n.y 0.7, gravity along the plane, uphill target × cos²;
  - BF1: stick × |N.y|, ×0.01 control under 0.75.
- **PCSX2, BF2 KAM1** (live RAM, every update; `lead_tmp/sthop/kam1_{ramp,edge,air}.csv`):
  - on the card's 34.3° ramp the player is grounded on every planar-facet update (7.0 m/s along the plane), and
    loses contact for one update at the facet folds (drop ≤ 0.03 m);
  - he flies 21 updates off the rim he climbed at vy +2.1, still in stand;
  - stand → 8 exactly one update after the timer reads 1.000.
- **The port** on the same walk: grounded 3 of 39 ticks, in Fall 27.
- **The spec, prototyped** in a scratch worktree (`lead_tmp/sthop/proto_core.patch`, main b0f8fd3): 22 of 24.
  Over 8 seeds × 32 maps: rounds finished 227/256 (main 228), world deaths 2557 → 1437, KAM1 region deaths
  1370 → 324.

One foreground Opus verifier, 21 claims: 17 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED, folded in and re-read.

Ported: nothing. STHOP → build; new card SLAND (landing damage = (approach − CollisionThreshold)² × CollisionScale;
the port kills at a PROVISIONAL 25 m/s).

## 2026-09-30 (loop lane 3, card KAMR) — Kamino bring-up: the sky, the sea, the floors, the far pieces → `analysis/re-kamino.md`
`[O] UNAUDITED`. The maintainer's play session on BF2 Kamino, both games' KAM1 set beside the PS2 (70 stored poses, three live
PCSX2 experiments, a draw-versus-collision floor census over all 32 maps).
- **The sky dome is darkened by `Effect("Lightning")`** (BF1 KAM1, BF1 YAV2, BF2 KAM1): between flashes dome shell 0
  draws at 255·(1 − `SkyDomeDarkenFactor` 0.4) = 153; a flash lerps the dome AND the fog colour back to white over
  `FadeTime`. B2 parser `FUN_0045aa30`, tick `FUN_0045ae80` → `sky+0x2855`, consumer `FUN_003cb918` (shell 0 only);
  B1 `FUN_00377c40`, `FUN_00378120` → `sky+0x100`, `FUN_002f2308`. PS2 RAM holds 153 in 20 of 21 in-level states (one
  mid-flash). The port draws every shell at white: sky 1.6–2.1 × too bright; prototype parity bf1_kam1 26.62 → 21.80,
  bf2_kam1 22.49 → 19.20.
- **The sea**: the PS2 draws the ocean in whole 160 m tiles reaching 0.8·FarSceneRange (640 m) and nothing behind — the
  frame is black from the sea's edge to the horizon; the far part takes the world fog colour in BF2 too
  (`FUN_00575670` 708400–708451; live pokes of the fog colour and FarSceneRange); BF2's ocean packet carries FFT normals
  (`FUN_00574a70` 708056–708105) and the sea reads texel × sun ((8,19,19) vs the port's unlit (26–49, 38–57, 34–55)).
  The flat plane past the edge is the port's own.
- **Far pieces**: `kam_bldg_tower1` is modelled hanging; 5 of BF2's 7 instances are flipped to stand on the domes.
  Authored — the PS2 shows the same.
- **Floors**: the hidden discs / cores / wall boxes are the disc's prims; the rail is walked over on the PS2 (feet
  +0.69 m) and the hull walked glued at 34°; a PS2 soldier released on a slit between collision floor pieces stands
  (69.272 vs 69.273), the port's one-ray ground answers the hidden disc 7.9 m below.
One foreground Opus verifier, 22 claims: 21 SUPPORTED, 1 PARTLY, 0 NOT SUPPORTED.

Ported: nothing. KAMR closed, split into KSKY, KSEA, KSEAL, GRND (+ B1DK: BF1's decks 1.5–2 × dark).

## 2026-09-30 (loop lane 2, card ANNC) — the commander's voice-overs: triggers, lines, the voice queue → `analysis/re-announcer.md`
`[O] UNAUDITED`. Both ELFs, the mission bytecode, every world LVL's posts, and two live PCSX2 battles (BF1 NAB1, BF2
UTA1) sampled from RAM (`lead_tmp/annc/annc_live.py`).
- **No announcer object: five callers ask the sound handle to play a 2-D stream line with the queue flag.** The voice
  stream slot plays one line at a time; a line is queued in descending key order (key = level·(1−Bias) + Bias·Priority),
  at most 4 pending; a commander line (BF2 key 1.0) goes ahead of pending unit chatter and never cuts a line off
  (B1 `FUN_0045d638` / `FUN_0045eec8`, B2 `FUN_00550528` / `FUN_00551d08`). Live: onset one frame when the voice is
  idle, else the rest of the line then playing (BF1 0.93–1.90 s in a 25-a-side battle).
- **Command posts**: the post's own table `VO_<Listener>_<Subject><Capture|Lost|InDispute|Saved|Info>` (entry 2's name
  cracked: `InDispute`); retail sets own capture, own loss and enemy capture only — the enemy's loss is silent. BF2
  gate `ScriptCB_EnableCommandPostVO` (campaign scripts only).
- **Bleeding**: a line when a team's bleed starts; repeated every 30 s only for the listener's own team
  (`VoiceOverManager` update B1 `FUN_002b1320`, B2 `FUN_00373b80`). **Low reinforcements**: once per battle at
  trunc(0.1 × start tickets), one latch for both teams; BF1's Clone Wars scripts set no line. **Leaving the
  battlefield**: BF2 6.5 s (2.17 s for one vehicle flag), the line when the HUD warning appears; BF1 10 s. Battle end:
  music only; BF2's script sends `common_objComplete` to the winners.
- **Correction to `re-tickets-spawn.md` §2.4: BF1's conquest bleed is LIVE.** The team's vtable slot +0xc
  (`FUN_002ad2f0`) qsorts the `AddBleedThreshold` table ascending, so the scan picks the tightest threshold; live RAM:
  `[(1, 3.0), (11, 2.25), (21, 0.75), (31, 0.0)]`, a team on 2 of 5 posts lost one ticket per accumulator wrap (6 in
  32.2 s). The port plays BF2's bleed on every map (PLAN), so no sim change — the docs and a code comment were wrong.
One foreground Opus verifier, 37 claims: 32 SUPPORTED, 5 PARTLY, 0 NOT SUPPORTED, folded in.

Ported: nothing. ANNC → build; new card BNDY (the map boundary: leaving the battlefield); notes on MSND, SNDC, EVMSG,
HERO, USND.

## 2026-09-30 (loop lane 3, card EVMSG) — the on-screen event messages of both games → `analysis/re-event-messages.md`
`[O] UNAUDITED`. Both ELFs, both Locl tables, BF2's `1playerhud`, and two live PCSX2 battles (BF1 KAS2, BF2 HOT1)
sampled from RAM by the game clock (`lead_tmp/re/evmsg/evsample.py`; frames + states in `pcsx2/ref/evmsg/`).
- **Neither game has a kill feed.** A death makes a line only when the victim is a human or a hero (a hero AI victim:
  offline, one screen, human or hero killer) — B1 `FUN_00297440`, B2 `FUN_0034f650`. Killing an ordinary AI shows no
  text; the feedback is the statistics panel: BF1's `StatIconDisplay` (kills / deaths / posts, yellow flash, 3 s),
  BF2's `player1stats` (points + kills with a "+n" delta, 5 s + 1 s fade; publisher `FUN_003b5de8`).
- **BF1** (`FUN_00307a00` → `MessageDisplay`): bottom-right, right-aligned, `gamefont_tiny`, two lines, each exactly
  4.0 s, no fade; `game.infomsg.suicide / teamkill / killed / died / captured / lost / joined / left`; green own, red
  enemy, grey none. PS2: "UNIT 688 KILLED PLAYER 1", "REPUBLIC LOST THE PIER ONE", "CIS CAPTURED THE PIER ONE".
- **BF2** (`FUN_003e0320` → the layout's `player1systemMessages` MultiLineText): upper-left, four lines, fade 0.5 s /
  hold 3.0 s / fade 0.5 s (RAM 0.534 / 2.969 / 0.534), a full queue drops the message; blue own, red enemy, grey none;
  a kill line takes the VICTIM's team colour. **BF2 has no command-post line** (events 4–7 are the voice line only; the
  two Locl strings are unused) — a correction to `re-cp-capture.md` §7. The same element carries the hero lines, squad
  orders (cyan), award lines, Lua's `ShowMessageText` (yellow) and the network lines.
- AI names come from Locl `common.sides.<side>.firstnames/lastnames.NN` ("Ben Pulastra", "UNIT 688"); the composer is [H].
- BF1's HUD plane is tilted 30° about X; 1.42–1.43 px per HUD unit at 640 wide (z = 40) — the scale re-hud-icons left open.
One foreground Opus verifier, 29 claims: 25 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED, folded in.

Ported: nothing. EVMSG → build (deps BHUD2); new card TIPS (the tooltip bar); notes on AWDM, SQMK, SQACK, BHUD2.

## 2026-09-30 (loop lane 2, card SQACK) — squad orders: the player's line, who answers, what each order does → `analysis/re-squad-orders.md`
`[O] UNAUDITED`. Both ELFs (several functions as disassembly), the soldier ODFs, the sound LVLs, and 11 scripted PS2
scenes (PCSX2, BF1 NAB1 CW; `lead_tmp/sqack/sq_live.py`: pad script + per-unit controller state, voice slots, HUD).
- **BF1**: four D-pad orders — Up MOVE OUT and Left AT EASE (one case in the binary: release), Down HOLD POSITION,
  Right FOLLOW ME. The issuer (`FUN_00260f38`) plays the player's line (`<s>_inf_com_chatter_tac_{moveout,hold,follow}`,
  own stream, unqueued) and shows the HUD cross with the four order names for a 2.0 s fade BEFORE it looks for anyone;
  then every friendly AI within 30 m (45 m for the release orders; 3-D; x3 in vehicles) gets the order — no cap, no
  LOS, no nearest-first. A unit takes it at its next think (the AI manager gives each controller one full update per
  15 frames = 0.5 s) and answers `ai_<s>_inf_com_chatter_tac_yes`, queued on the voice stream; a take drawn in the
  last 1 s (2 s for all / imp) is silent, so 3-5 takers give 2-3 answers about a second apart.
- **FOLLOW ME** = `UnitFollowAgent`: at most TWO followers per leader (the first two to think); walk to him, stand at
  a point within +-3 m in his frame, follow again past 9 m, board the vehicle he boards; released 20 thinks (10 s)
  with him beyond 20 m. **HOLD** = `UnitHoldAgent`: every receiver; it does not move at all, looks around (re-roll
  1 in 18 per think); released the same way. PS2: holders moved 0.1-0.3 m in 10 s, released 9.5-9.8 s after the
  player passed 20 m; followers stood 1.3-3.8 m from a still leader.
- **Corrections to this log's 2026-09-01 squad entry**: its "UnitHoldAgent `FUN_00248cb8`" is `UnitRandomAgent`, its
  "UnitFollowAgent `FUN_00248498` / `FUN_00248828`" is `UnitHoldAgent`, its "fly agent `FUN_002474e0` / `FUN_00247b18`"
  is `UnitFollowAgent` (RTTI; the same shift in BF2: 0xe = Follow `FUN_002c9418`, 0xf = Hold `FUN_002caa40`); the
  "1-in-9" draws are 1 in 18. The 30 hashed ODF sound keys of `re-sound-global.md` §3 are `SC<Field|Driver|Passenger|
  Gunner><Action>Sound`, their `AISC…` twins and `AISCResponseYesSirSound` / `NoSirSound` (the latter never played).
- **BF2**: BF1's line + HUD function is unreferenced code (0x2e2480); the live order is contextual — press at a
  friendly AI soldier under the reticle: follow / stop (`SC_Follow` … events, `<side>_command_*`, the unit's
  `<side>_response_*` 0.5-1.2 s later); hold 1 s: all followers stop; a human leads rank + 1 units (1-4 by medals).
One foreground Opus verifier, 49 claims: 39 SUPPORTED, 8 PARTLY, 2 NOT SUPPORTED, all folded in (§10 of the doc).

Ported: nothing. SQACK → build; new card SQVEH (the vehicle-seat orders); notes on SQMK, USND.

## 2026-09-30 (loop lane 3, card GREN) — weapon slots past the primary: channels, grenades, mines, detpacks, the AI's grenade → `analysis/re-grenades.md`
`[O] UNAUDITED`. Both ELFs (class-label tables -> constructors, setters, vtables; three functions as disassembly), every
`wpnc` / `ordc` / `expc` / placed `entc` of both discs, and two live PCSX2 runs (BF1 NAB1, BF2 HOT1: one row per game
update of the thrown ordnance from RAM; `lead_tmp/gren/probe{1,2}.py`).
- **Slots**: a soldier class holds up to 8 slots (weapon, ammo in clips, channel 0 / 1); the soldier instantiates all,
  keeps one CURRENT slot per channel with both selected, and one ACTIVE channel (channel 1 wins; the other trigger is
  swallowed while a weapon fires). One switch input per channel steps to the next slot of that channel, instantly. BF2
  skips an award weapon until the unit's medal bit is set (`MedalsTypeToUnlock` / `MedalsTypeToLock`).
- **The grenade weapon** (`grenade` label: B1 `FUN_0028f1f0`, B2 `FUN_00337fd0` — NOT the block beside the RTTI string,
  which is `laser`): a fresh press only (`TriggerSingle` defaults to 1), release after `InitialSalvoDelay` (0.3 BF1,
  0.51 BF2), the next throw after + `ShotDelay` (1.7 / 1.81 s). Launch = normalise(aim + (0, `ShotElevate` 0.21, 0))
  x the ordnance `Velocity` 25 for a human, the aim itself for a bot; BF1 from the fire point, BF2 from a per-variant
  hand offset. PS2: launch velocity equal to that formula to 1e-6.
- **The thrown ordnance** (`sticky`): gravity `Gravity` x -9.8; a swept ray per frame; the bounce is restitution 0.37
  and friction min(1, 1.15 x 1.37 |vn| / |vt|) as CONSTANTS — the disc's `Rebound` / `Friction` on a sticky are read
  by nothing; `Stick*` bits per target kind; rest after 4 floor bounces (BF1) / 4 bounces slower than 1 m/s (BF2);
  **`LifeSpan` starts at REST** (or a stick, or water), not at the throw. PS2: every bounce fits to 2.5e-6 m/s; BF1's
  detonator explodes 3.64-3.70 s after release (0.9 s after rest), BF2's 2.93 s (0.6 s after rest).
- **Mines**: the `dispenser` drops an entity (per-unit `MaxItems`, the oldest destroyed); the mine triggers on an
  enemy inside `TriggerRadius` once its velocity stops pointing at the mine; BF2's sticks to world / vehicles and
  fires on enemy contact; the time bomb is a mine with `LifeSpan`; BF2's detpack `detonator` sets its packs' trigger
  flag on the next fresh press.
- **The AI**: the weapon choice walks every slot (ammo, target-kind mask, range band, "plenty of ammo", score); a
  grenade in range is forced in a 3 s window per 30 s per unit or against a GROUP of enemies (BF2: + the difficulty
  roll, always vs droidekas); the aim is the exact low-arc ballistic solution; a landed grenade warns AI units
  within 7.5 m after a difficulty-ranked delay.
One foreground Opus verifier, 42 claims: 36 SUPPORTED, 6 PARTLY, 0 NOT SUPPORTED, all folded in (§11 of the doc).

Ported: nothing. GREN → build; new cards GRAI (the AI's weapon choice and grenade), MINE (dispenser, mines, detpack).

## 2026-10-01 (Fable chat session, card KFLR) — the kill floor: what kills a soldier who leaves the world → `analysis/re-kill-floor.md`
The maintainer's ask: a kill floor on maps one can fall off, and BF2 Kamino's hidden ground under the decks. One Opus RE pass
(`[O]`, no verifier; Fable re-read the world-box lines and the death-region kill in the decomp).
- **`AddDeathRegion`'s consumer** `[O]`: one region manager per game (B1 `DAT_00518398`, B2 `DAT_005feea0`), 32
  slots of {region, type}; a per-frame update (B1 `FUN_002af1c0`, B2 `FUN_00371148`) tests soldiers and vehicles,
  AI and human alike — BF1 the FEET as a point, BF2 the collision bounding SPHERE against the region — and kills at
  once with no killer (B1 `FUN_002af0d0` → `FUN_001bbc98(E+8, 0-desc, 0)`; B2 `FUN_00370fd8` → `FUN_001df640`).
  The kill's third argument is 0, which skips the invincibility test: **a death region kills an invincible human.**
- **The world box** `[V]` (soldier tick, kills past the cheat too): BF1 `FUN_00205cc8` B1:116586-116603 — y < −500,
  y > 500, |x| or |z| > 1000, and below the `water_sink_region` plane's height (manager+0x118); BF2 `FUN_00240bf8`
  B2:159373-159398 — y < −1000, y > 1000, |x| or |z| > 2500, v.y < −60. This is what kills on BF1 Bespin
  Platforms (no death region, no terrain, no water sink): the soldier falls to −500.
- **No fall timer kills; the map boundary is 2-D** (x and z only) `[O]`.
- **BF2 Kamino's hidden collision** `[O]`: nothing kills a soldier standing on it. A landing costs (v − 13)² health
  (class threshold (10, 3, 0), scale (1, 0, 0), `FUN_00250488` B2:169419): an 8 m drop 16 HP, 15 m 80 HP.

Ported (KFLR, `core/src/sim/bounds.rs`): death regions, the kill floor and a landing under the map kill past the
`invincible` cheat. **Ours:** the kill floor's height (15 m under the lowest floor the map's graph and posts stand
on, not −500 / −1000) and the landing under the map (the maintainer's rule; the PS2 has none). **Not ported:** BF2's
sphere-against-region test (the port tests the feet in both games), BF1's water-sink plane, the ceiling and the
horizontal limits, the landing-damage law (card SLAND).

## 2026-10-01 (loop lane 1, card STDOR) — where a door's bone-bound geometry rests → `analysis/re-bf2-doors.md` §10
A door's `BNAM` segments and `PRNT` prims are stored in their bone's frame (disc data + PCSX2 frames of DEA1's blast
door and MUS1's garage / square doors; [D], no ELF hop). Built in `bake::bind_pose`. Closes card BNR and
`re-round-stalls.md` C3's [H]. Still open: prims of non-door geometries (DEA1's trash compactor).

## 2026-10-01 (loop lane 2, card KSEAL) — BF2 Kamino's lit sea, a fit → `analysis/re-kamino.md` §2.6
The PS2's ocean field read from RAM in 20 states (y range 8.8–19.6 m, foam bytes on 76–107 of 289 points) agrees with
the port's; its frames show the sea at the sun's colour, the foam faint, and a loss of colour from `FogRange`'s 400 m
that the fog pokes do not account for. Built as three PROVISIONAL constants in `viewer::look::water_look`. Still
unread [H]: the VU program that lights the sea, why the foam is faint, what darkens the far sea.

## 2026-10-01 (loop lane 3, card WAI) — how a bot drives a walker: the AI's sticks, the shared steering → `analysis/re-walker-ai.md`
`[O]`, one Opus verifier in the foreground (39 claims: 37 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED, folded in), and a
tick-by-tick replay against live PS2 RAM: 7504 ticks of AI spiders, an AT-ST and tauntauns, both sticks and the yaw
step equal on every one (`lead_tmp/wai/wai_check.py`).
- **The walker's AI** `FUN_00262248` (controllable slot +0xcc; CommandWalker shares it) writes its steering into the
  LEFT stick's X, E+0x2e0 — the p3 `re-walker.md` §4.5 C reads. The right stick carries only the aim.
- **The shared steering** `FUN_002a54d8` (seven callers: walker ×2, hover, droideka, …): outside 4 m of its goal a
  walker turns in place (throttle 0) until the goal is within 5°, then walks with throttle = cos(error); the turn
  command is `FUN_002a45a0`'s deadbeat clamp(−(n·X)/(MaxTurnSpeed·dt), ±1). With a strafe speed it returns a
  (throttle, strafe) pair toward the goal instead; the walker takes that branch only with class flag bit 0 (hash
  0x55bc9af9: BF2's three AT-STs, the one-man AT-ST, the tauntaun) and the goal within 30°, and stores E+0x2e0 = −strafe.
- **§4.5 C's negative factor is real.** In the strafe branch the negation cancels it: the walker turns toward its goal.
  In the plain branch on a class that authors e08 / e0c (BF2 spider, AT-TE, AT-AT) nothing does: the walker turns AWAY
  while it walks, passes 5°, stops, turns back — live spiders crab ~5.3° off their goal with the throttle toggling.
- **The AI's sticks are raw** (0.997, 1.0 on a walking spider; |stick| 1.41 runs the clip ×1.38).
- **The goal** is the mover's point 20.2 m ahead along the path segment (`FUN_002a5fa8`; a hover: |velocity| + 0.2);
  arrival 1.0 m at a node, 4.0 m at the last (`FUN_002f1140`).
- **Also read:** a 7.5 s stop after touching a walker / hover it should yield to (`FUN_002a8c30`); under 20 % health
  26 of 101 controllers tell every AI occupant to get out (`FUN_002a95a8`; the hover's AI has the same test); speed
  mode 3 (the boost) is set by the combat helper's state 6 and the follow navigator — its senders stay `[H]`.
- **Class ids named** from the vtables' getters (EntityWalker 0x689960, EntityHover 0x6894f8, EntityFlyer 0x689368,
  EntityDroideka 0x689300, MountedTurret 0x68c8f8, EntitySoldier 0x689828, EntityCarrier 0x688f88) — this settles
  `re-walker.md` §9's three `[H]` ids.

Ported: nothing. WAI → build (the stick law in the port's terms, §11; 145 PS2 rows for a unit test in
`lead_tmp/wai/ps2_rows.csv`). New cards VBAIL (the 20 % bail-out, hover and walker) and HAIS (the hover bots' steering).

## 2026-10-02 (loop lane 2, card AIRC) — the stick in the air, both games → `analysis/re-stance-roll.md` §4.3a
- **BF2** `FUN_00240bf8` cases 4 / 8 / 9 → `FUN_00248560`: drag `exp(−0.012·|v|·dt)`, then the stick ADDED as an
  acceleration (ControlSpeed jump 0.03 × Acceleration = 2.1 m/s²), no target, no cap. **[V][V-PS2]**
- **BF1** `FUN_00205cc8` cases 4–6 → `FUN_002098b8(fwd × 0.2, strafe × 0.2, brake 0)`: each axis raised toward 0.2 × the
  walk speed at Acceleraton × 1.2 × 0.2, never lowered. **[V][V-PS2]**
- Measured on open floor with PCSX2 (`lead_tmp/airc/ps2/`): a standing jump with the stick forward lands 0.935 m ahead
  (BF2) / 1.214 m (BF1); a running jump keeps its speed with the stick released. The "no air control" reading came
  from wall trials, where the wall takes the stick's speed off every update.

Ported: `core::soldier::Soldier::air_steer` (both rules, per `GroundRules`), `ai::brain::lost_jump` (a lost bot jumps
from where it stands). Not ported: the take-off update's missing gravity (card JARC, with BF2's numbers now on it).

## 2026-10-02 (loop lane 3, card WAI2, measure) — who pilots a walker, live from the first tick → `analysis/re-walker-ai.md` §13
- **A bot pilots every AT-AT and AT-TE** from 12-24 s of mission time and stays in (BF2 HOT1: one AI each for a 240 s
  run, 414-452 m; BF1 HOT1 with a gunner each). The command walker is `FUN_0035baa0`'s class (vtable 0x62be40, +0xcc
  0x62bde0; BF1 matrix header 0x53e1e8) — WAI's sampler filtered it out. **[V-PS2]**
- **Its team spawns at it**: 14 of BF1 Hoth's 24 Imperials and 6 of BF2's 16 are first seen 8-9 m from an AT-AT. The
  spawn rule itself is not transcribed (card CWSP). **[V-PS2]**
- Bots come 8-71 m for a walker; a BF1 tauntaun ride is 282-711 m and ends 4.5-41 m from the enemy post. **[V-PS2]**

Ported: nothing (a measure job). The port boards none of the command walkers and never reaches the snow AT-ST
(`lead_tmp/wai2/census2/`): cards WAI2 (build) and CWSP (re).

## 2026-10-02 (loop lane 3, AUDIT AIRC) — the take-off update, the roll's mover, BF1's backpedal → `analysis/re-stance-roll.md` §4.3b
- **AIRC's air rules hold** (1,152 live updates from other maps replay within 1.8e-6 / 2.7e-5 m/s); BF1 integrates the
  frame's own 33.37 ms, BF2 a fixed 1/30 s. **[V][V-PS2]**
- **The take-off update**: the ground mover runs with the update's stick, THEN the jump is applied. BF2 `FUN_00246ed0`:
  vy += the jump velocity; × JumpForwardSpeedFactor for a forward component only (backward / side take
  JumpStrafeSpeedFactor); a controller's jump request SETS v = dir × MaxSpeed × JumpForwardSpeedFactor. BF1
  `FUN_00208b78`: vy = 8.0 and v += forward × 8.0 × stick_fwd × 0.25 (± 2.0). Stick and jump pressed together from a
  stand land 3.75 m (BF2) / 4.59 m (BF1) ahead. **[V][V-PS2]**
- **BF2's roll** is moved by `FUN_00248560` too (case 5: the drag + 1.4 m/s² of stick). **BF1 walks back at MaxSpeed**
  (−6.5004 m/s live), not MaxStrafeSpeed — that asymmetry is BF2's. **[V][V-PS2]**

Ported: nothing (an audit). Cards JARC (re-scoped: the take-off update), ROLLM, BF1BK.

## 2026-10-02 (loop lane 1, card EVMSG build) — BF2's points, AI names, the message line's glyph pitch → `analysis/re-event-messages.md` §9
`[O] UNAUDITED` addendum, found while building the card (`lead_tmp/evmsg/`: `points_ram.py`, `names_ram.py`, `postnames.py`).
- **Points** come from a table the scripts fill (`ScriptCB_SetPlayerStatsPoints` → `DAT_005fedf0`, empty in the ELF,
  read from two HOT1 states): AI kill +1, human kill +2, hero +5, suicide or a death with no killer −1, team kill −2,
  a capture +5 for the player longest in the region and +2 for the others (`FUN_003616f0`, callers `FUN_00362268`,
  `FUN_00362c60`, `FUN_0034f650`, `FUN_00363a30` ← `FUN_00357440`). The 2-point kills (kinds 0x17 … 0x1e) are not read.
- **AI names** (RAM, 38 records): Rebels first + last, Imperials two letters + three digits ("DX 178"); BF1's CIS
  "UNIT 688". Locl name keys count from 01.
- **Post names**: `level.<world>.<Label>` names 227 of 248 posts (BF1 KAS2 CP5 = "PIER ONE", as the PS2 printed).
- The message line is set one texel wider per glyph than a Text element (PS2 frame, 24 glyphs, no drift); a group's
  Rect offsets its children (the statistics pop-up lands on the PS2's place only so).
One foreground Opus verifier, 8 code claims: 6 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED, folded in.

Ported: card EVMSG (`ui/src/hud_tree.rs` `MultiLine`, `game/src/event_feed.rs`).

## 2026-10-02 (loop lane 3, card CWSP) — the command walker's post: a moving spawn post → `analysis/re-command-walker-post.md`
`[O] UNAUDITED`. Both games, the discs, RAM of six states, five live runs, one live spawn-screen session (`lead_tmp/cwsp/`).
- **An AT-AT / AT-TE owns an ordinary command post** (B2 `FUN_0035baa0` → factory `FUN_00354920`, E+0x2290; B1
  `FUN_002a26e0` → `FUN_0029c788`, E+0x23e0): scheduled by the walker's init, killed by its kill handler, rebuilt with
  the walker by the pad (Hoth 120 s, Geonosis 45 s). The pad's team for life; no capture region. **[V][L]**
- **Six class spawn points in the walker's LIVE frame** (`SpawnPointLocation`: AT-AT (±8, 0, {0, ±4}), AT-TE
  (±6, 0, ±4) and (±7, 0, 0)), one drawn at random, ray-checked from the body, stood upright (B2 `FUN_00356998`, B1
  `FUN_0029df70`). BF1 Hoth: 14 opening spawns within 0.13 m of an authored point; BF2 respawns ride the AT-AT 395 m
  from its pad. **[V][L]**
- **The spawn screen offers it in both games** (the mover tests team and schedule only); live on BF2 the player
  selected an AT-AT's post and spawned beside it. Icon = the class `MapTexture` at the live position; no number in
  BF2 (0xC069B070 = `HUDIndexDisplay` 0; 0x66BCDAEA = `HUDIndex`). **[V][L]**
- **An AI member's spawn post**: one draw per death, BF1 uniform over the eligible posts (`FUN_00261e80`), BF2
  weighted by `AISpawnWeight` (`FUN_002e7240`, default 100, no conquest override on Hoth / Geonosis). **[V][L]**
- **Its death**: event 5 for its team; BF2 no ticket or bleed effect (the script's registered posts only; plain
  `ValueBleed` is not in BF2's ELF); BF1 counts it (ValueBleed 10: Hoth's Empire bleeds 0.1875 / 0.5625 tickets/s
  with one / both AT-ATs down, and a live AT-AT holds off the no-posts countdown). **[V][D]**
- BF2's conquest target pick (`FUN_002e8318`) takes a hostile walker's post as a candidate (Value_ATK 10), at the
  walker's live position. **[V]**, what the bot then does **[H]**.
Corrects `re-cp-capture.md` (activate / kill swapped, the spawn-point lists' order, flag bit 2, the AI's spawn pick).
One foreground Opus verifier, 59 claims: 56 SUPPORTED, 3 PARTLY, 0 NOT SUPPORTED, folded in.

Ported: nothing (an RE job). Cards CWSP (build), SPST (new: a bot draws a post, then a point), notes on CONT, SPSEL, BMAP.

## 2026-10-02 (loop lane 2, card CONT) — the Conquest AI goal: one random enemy post per bot, kept → `analysis/re-conquest-goal.md`
`[O] UNAUDITED`. Both games, BF2's scripts, RAM of three states, two live 9-minute battles sampled at 4 Hz (`lead_tmp/cont/`).
- **BF2 runs one Conquest goal per team** (`AddAIGoal(team, "Conquest", 100)` from `ObjectiveConquest.Start`; goal
  manager DAT_005fe7a8, 1 s tick `FUN_002e7cf8`); every AI record of the team is its member. **[V][L]**
- **A member gets a target only when it has no order or its capture behaviour finished** (`FUN_002e7110`,
  `FUN_002e7e48`), or when its target turned friendly with `Value_DEF` 0. Live: bots on foot changed target once per
  233-323 alive unit-seconds (the port: once per 9.6). **[V][L]**
- **The draw** (`FUN_002e8318`): the posts not friendly to the team, weighted by `Value_ATK` of its faction (mean of
  ATK and DEF when neutral), minus bans, value ≤ 0 and posts the planning graph cannot reach (`FUN_002ebf70`); own
  posts only when nothing else is left. **No distance, threat or crowd term.** Live: 0 of 578 draws took a friendly
  post; even over distance ranks; Polis Massa's 20-valued posts drawn 118 of 185 (125.5 by value, 95.9 uniform). **[V][L]**
- **The order is type 8**; a soldier's controller runs the capture behaviour (`FUN_002cb970`): walk to the region,
  stand in it until captured, then hints / wander at the post; it ends 5 s after arriving at a post the team can
  already spawn at ("the 5 s rule", once per start), on no path, or seated. Reactions (fight type 2, turret type 4) go
  in front of it and it resumes. 8-9 % of lives reach their target's region. **[V][L]**, the fight itself **[H]**.
- **BF1**: the same draw (`FUN_002618b8`; team 1 by ATK, team 2 by DEF whoever owns the post; no reachability test)
  and helper, but event-driven: a new draw on death, vehicle change, the target turning friendly (all its targeters
  at once), 5 s after arriving at a friendly post; a post lost to neutral calls five AI team mates within 60 m
  (`FUN_00262090`). BF2 left those hooks as empty functions. **[V][L]**
Closes `re-command-walker-post.md` section 8's two [H] (the graph test; a bot at a post with no capture region).
One foreground Opus verifier, 56 claims: 46 SUPPORTED, 10 PARTLY, 0 NOT SUPPORTED, folded in.

Ported: nothing (an RE job). Card CONT re-phased to build (section 8 of the doc is its change list).

## 2026-10-02 (loop lane 3, card SNDC) — gameplay audio census: what the PS2 plays against what the port plays → `analysis/audio-census.md`
`[O] UNAUDITED`, a measurement (no verifier). BF1 NAB1 and BF2 UTA1, 105 game seconds each on PCSX2 + a pause-menu
sample, read by a new RAM sampler (`lead_tmp/sndc/snd_live.py`); the port by a sound log of its headless battle.
- **The engine's voices**: 78 one-shot voices (B1 `*(0x5228cc)`, B2 `*(0x60be70)`, stride 0x110; state u16 +0x7c /
  +0x84, bit 8 = playing; sample record +0xa0 / +0xa8, id at record+0x10) and 39 handle voices (B1 `*(0x5220c4)`,
  B2 `*(0x60b654)`, stride 0x150) where engine loops live; delayed layers wait in a third pool (B1 0x751e40). **[V][L]**
- **The port plays 5 of 24 event rows** (fire, reload, bolt strike, commander, the squad's answer); foley, hurt /
  death voice, chatter, every vehicle sound, explosions, flight hum, music, ambience and menu sounds are silent. **[L]**
- **Sound templates are in CORE.LVL and the bake resolves `Inherit` inside one LVL only**: every port sound is gain
  1.0, 5 - 50 m; the disc says weapons gain 0.7, 5 - 400 m, rolloff 6 (BF2 3.5), explosions 30 - 450 m. 85 - 94 % of
  the PS2's audible infantry fire starts beyond 50 m. **[V disc][L]**
- Foley names are hash-only; 303 recovered as `<side>_inf_<unit>_<action>_<surface>[_left|_right]`. **[D]**
Open **[H]**: what the 39-voice set is (mixed voices or held handles), the `soundlayered` record's layout, why the
PS2 plays 2 - 12 reload sounds where the port fires 128 - 184 reloads, when the weapons' `*_mvt_*` keys play.

Ported: nothing. Cards SNDT (build), FOLY, VSND, WSND, AMBM (re); notes on USND, GREN, PWUP.

## 2026-10-02 (loop lane 2, card PWUP) — pickups: what a dead unit drops, the item, who takes it and what it gives → `analysis/re-powerups.md`
`[O] UNAUDITED`. Both games; discs, RAM of seven states, four live PS2 battles sampled at 20 Hz (`lead_tmp/pwup/`).
- **Only a soldier-class unit drops**, from a table of up to 8 (class, probability) pairs on its class
  (`DropItemClass` / `DropItemProbability` / `NextDropItem`; B1 `FUN_0020f2e0`, B2 `FUN_00252bb0`; a BF2 child
  overrides its parent's entries by position). The droideka's and the probe droid's keys are dead data. **[V]**
- **One roll per death, at most one item, none when the PowerupItem pool is full** (25; BF1 scripts 25 - 60), in the
  soldier's kill function (B1 `FUN_001ffec0`, B2 `FUN_002379f0`), which the update calls on the first update after
  health reaches zero but not while the body is thrown, tumbling or getting up. BF1: ALL / REP / CIS 0.80 (ammo
  0.40, health100 0.30, dual 0.10), IMP 0.25; BF2: 0.70 for every regular class and for heroes. Live: 0.84 / 0.91
  against 0.80, 0.23 against 0.25, 0.74 / 0.73 against 0.70. **[V][M]**
- **It spawns 1.0 m above the feet** with the corpse's velocity (BF1) or, BF2, the corpse's velocity above 3 m/s
  and otherwise 3 m/s to its left; lands in ~0.5 s; lives `LifeSpan` 30 + 1 s (BF2 fades the last second, BF1 stops
  drawing it 2 s before it goes). **[V][M]**
- **Anyone alive of either team takes it by touching it, only if it gives something**: health to the cap (BF1
  health25 +50, health100 +75, dual +100; BF2 +100 / +200 / +200), +1 or +2 clips on every weapon to its cap, BF2
  energy +150, offense x1.5 for 12 s, defense x0.5 for 20 s; a BF2 hero gets no health and wastes a buff item.
  Contact of the taker's body with the item's box (the model's bounds); live 0.3 - 1.0 m feet to item. **[V][M]**
- **BF1's bots never look for an item; BF2's detour to one within 6 m** that gives health or ammo, every 2 s, at AI
  level of detail 3 or more (`FUN_002bd628` -> `FUN_002be050`). **[V]**
Corrects `unit-audit-bf1.md` §3.4 ("the droideka included": it authors the keys, the engine never reads them).
One foreground Opus verifier, 63 claims: 60 SUPPORTED, 3 PARTLY, 0 NOT SUPPORTED, folded in with its extra finds
(BF2's create gate `DAT_006d9330`, the hero / buff waste, `DebuffDamageTimer` / `Rate`).

Ported: nothing (an RE job). Card PWUP re-phased to build (section 9 is its change list); card PWAI (BF2's detour);
a note on MINE.

## 2026-10-02 (loop lane 1, card SNDT) — a 3-D sound's level by distance → `analysis/re-sound-distance.md`
`[O] UNAUDITED`. One function, the same on both games (B1 `FUN_0044cac8`, B2 `FUN_005407e8`):
- **Level** = 1 from `rollindistance` to `mindistance`; `(R - x) / (x k + R)` beyond it (`x = d - min`,
  `R = max - min`, `k = 10 x rolloff`); 0 from `mutedistance` out; `d / ((rollin - d) k + rollin)` inside the roll-in
  distance. **[V]** The 10 is the listener's rolloff factor (constructor B1 `FUN_00449f78`, B2 `FUN_0053da70`; 10.0 in
  the RAM of all 329 reference states **[L]**; never changed in play **[H]**).
- **Keys**: `rollindistance`, `mindistance`, `mutedistance`, `maxdistance`, `rolloff` at property +0x2c .. +0x3c
  (`FUN_0044d130`); `mode3d` (0x1a8b16b5) = the 3-D bit at property +0x44, clear = 2-D, gain only. **[V]**
- **Disc**: 58,459 of 59,808 `Inherit`s name a template outside their own LVL; through CORE.LVL 0 are unresolved;
  mute != max on 95 properties, roll-in on 372. `ScaleSoundParameter` only in 12 BF2 space scripts. **[L]**
One foreground Opus verifier, 9 claims: 7 SUPPORTED, 2 PARTLY, folded in. Open **[H]**: the sound-space gains
(`obstructionhfgain` / `occlusionhfgain` scale a sound in another space than the listener), BF2's cone term.

Ported: `bake sounds` resolves templates through CORE.LVL and carries mute / roll-in / 2-D; `game::sounds::attenuation`
is the function above.

## 2026-10-02 (loop lane 3, card FOLY) — soldier foley: the `ffx_` table, the surface, what starts a step → `analysis/re-foley.md`
`[O] UNAUDITED`. Both games.
- **The table** is a config chunk `ffx_` in `SOUND/<PLANET>.LVL`, one set per sub-level `<map>` + `cw` / `gcw`:
  `FoleyFXSoldier` records (a `Class`, 13 sound slots; BF2's carry 11) listed by `FoleyFXGroup` records
  (`terrain_foley`, `stone_foley`, `metal_foley`, ...). BF1 386 records in 97 groups, BF2 592 in 140. Loader B1
  `FUN_00367558`, B2 `FUN_0044e188`. **[V]**
- **Which row**: a soldier class's ODF `FoleyFXClass` (9 values) picks the row inside the group; a prop class's
  `FoleyFXGroup` names its surface. **[V]**
- **The surface** is the last body the soldier touched (B1 `FUN_00367d08`, B2 `FUN_0044e7b0`): the terrain ->
  `terrain_foley`, the water object or a contact under the water surface -> `water_foley`, any other object -> its
  class's group. What `terrain_foley` sounds like is map data (grass on NAB1, stone on UTA1, snow on Hoth). **[V]**
- **A step** comes from the drawn animation: a foot bone that dropped and then rises from below 0.17 m (BF2:
  `MinFootHeight`) starts its side's footstep (B1 `FUN_00205538`, B2 `FUN_0023fe00`). The distance rule
  (`FootstepStride` 1.75 m; B1 `FUN_002051a0`, B2 `FUN_0023f9e0`) runs only for a human player's soldier without an
  animator or in first person. An AI soldier that is not drawn makes no footstep: RAM has the player index -1 and
  the accumulator 0.000 on every AI soldier, 43 of 43 and 34 of 34. **[V][L]**
- **Walk or run** is the movement clip: BF1 forward above 5.5 m/s (5.0 to stay), backward and strafe left ALWAYS
  run (a sign slip in `FUN_0022e990`), strafe right above 5.0 / 5.5; BF2 above 0.78 x MaxSpeed (0.71 to stay,
  `FUN_0024f350`). Live: left at 4.4 m/s plays run steps, right at 4.5 m/s walk steps; forward run steps every
  0.311 s / 2.0 m (BF1), 0.280 s (BF2) - the run clip's stride, not the 1.75 m. **[V][L]**
- **Other slots**: Jump, Land, Roll, Squat, Prone (BF1), Stand (empty in BF2's records), BodyFall from the stance
  functions; the two `Footstep*Stop` slots are read by nothing. **[V]**
- **The weapons' `*_mvt_*` keys** are started by the code on every step and stance event and are silent: 0 of
  1482 (BF1) and 0 of 494 (BF2) such properties has a sample in any bank of its disc. **[V][L]**
- Dead on both discs: `FoleyFXWalker` / `FoleyFXOrdnance` records, `foleyfx` regions, `ClothingRustleSound`,
  `RunSpeed` (no class sets it and the field has no default).
One foreground Opus verifier, 64 claims: 48 SUPPORTED, 16 PARTLY (5 substantive, 11 wording), 0 NOT SUPPORTED,
folded in with its extra finds (a writer of BF2's session flag outside the Ghidra export; BF2's records have no
`Prone` / `Stand`). Open **[H]**: BF1's first-person body; "last contact wins" against a wall.

Ported: nothing (an RE job). Card FOLY re-phased to build (section 9 is its change list); notes on ANIM (the PS2's
movement clip chooser and playback rate) and USND (the weapon movement keys are dead data).

## 2026-10-02 (loop lane 3, card AIMA) — the aim ray, the camera's tilt and where a shot goes → `analysis/re-aim-assist.md` §10
`[O] UNAUDITED`. Both games; the rule is the same, the class data differs.
- **The camera looks `TiltValue` below the aim**: BF1 5°, BF2 10° (zoom sections 3.5°, BF2 prone 5°; × 0.4 in split
  screen). The key lands in radians at section + 0x5c (setter B1 `FUN_001b6168`, B2 `FUN_001d0328`; getter B1
  `FUN_001b58c0`, B2 `FUN_001ce9b8`). The spawn pitch is −0.07 rad, so a fresh soldier's camera looks 9.01° / 14.01°
  down. PitchLimits −0.8 / +1.1 rad on every class read. **[V][V-RAM]**
- **The aim ray leaves the camera's own (ideal) position along the aim direction**, with its origin slid forward to
  be abreast of the soldier's eye point: A = cam + d·((eye − cam)·d) (the soldier's aim update, B1 `FUN_00204748`, B2
  `FUN_0023ecb8`). This is §3.2's "2.104 m above the feet". **[V][V-RAM: 132 states reproduce to 0.5 mm]**
- **The reticle** is drawn at x = 0.5, y = the projection of A + 1024·d (B1 `FUN_002d4b50`, B2 `FUN_0038e598` → the
  event `player%d.weapon%d.reticule.position`): tan(tilt) / tan(fov_y / 2) of the half height above the centre. The
  game's own stored value: line 178.48 of 480 (BF1), 153.35 (BF2), at every pitch. An angle, not a height. **[V][V-RAM]**
- **The shot** leaves a fire point from a stance table (stand: 1.7 m up, then 0.25 m along the aim, 0.10 m below it,
  0.06 m right; the same table in both games, bots included) toward H, the first thing the ray hits between
  TrackOffset.z + 2 and + 62 m from A (5.5-65.5 m BF1, 5.2-65.2 m BF2; soldiers, vehicles, buildings, terrain), else
  the ray's far end. **[V]** Live, 44 single shots: within reach the bolt lands under the reticle (≤ 2.5 lines);
  with the ground or a wall nearer than the ray's start it lands 47-64 lines below it, 1.4-3.3 m ahead of the feet
  (1.43 / 1.51 m at the steepest aim). **[M]**
- **A bot** fires from the same point along its own (tracked) aim: no tilt, no ray. **[V]**
- **The drawn camera** (B1 `FUN_001b58f0`, B2 `FUN_001cef68`): the same tilted frame, orientation blended by
  AimTension (BF1 30, BF2 none), five collision rays from feet + 1.5 m to the eye (pull in at once, recover at
  0.75 per second past a 0.15 band), MoveTension per axis. **[V]**
- Named by hash: `MoveTension`, `MoveTensionX/Y/Z`, `ZoomFirstPerson` (the scope path fires along the aim itself).
- Checks: `engine/artifacts/lead_tmp/aima/checks.txt`; one Opus verifier, 40 of 46 SUPPORTED, 6 PARTLY folded in, 0
  NOT SUPPORTED. Evidence `engine/artifacts/lead_tmp/aima/` (models, live probe, frames).

Ported: nothing (an RE job). Card AIMA re-phased to build (§10.8 is its change list); new card FPNT (the fire point
for every soldier); notes on CAMS (the tilt and the collision rule are read now) and RETC (the reticle's place).

## 2026-10-02 — Friendly fire as it runs, and the PS2's own team kills (card FFLN) → `re-blaster-damage.md` §6.3 [O]
- The friendly-fire scale is 0.99999994 (the profile byte 100 x 0.01) in all 326 battle states of the reference
  library, both games: full damage. A bot's hit on a human of its team is dropped before the scale. **[V][M]**
- BF2's bots kill their own: 11 of 91 AI kills in 515 s of KAM1 were of a teammate (12.1 %), 4 more of the killer
  himself (per-player statistics bytes, `*(0x661394) + i * 0x210`, live). **[M]** — one battle.
- One Opus verifier: 8 of 10 SUPPORTED, 2 PARTLY folded in. Evidence `engine/artifacts/lead_tmp/ffln/`.

Ported: the bot-on-human rule for bolts (`resolve_bolt`). The port's line-of-fire rule is OURS (FEEL_NOTES).

## 2026-10-02 — The PS2 foot soldier's own AI, both games (card SAIR) → `re-soldier-ai.md` [O]
Pointer entry; the note is the record (its section 0 is the one-table answer, 12 the PS2 | port | difference table,
13 the PS2's own weak points, all for card BOTD).
- **Think rate**: BF2 interval = agent factor (capture 2.0, fight 0.4, others 1.0) x {4, 3, 2, 1, 0.25} s by a level
  of detail from the nearest human (distance 7 / 20 m, the bot's 77.5 deg cone, the camera); 93-98 % of bot time at
  levels 1-2. BF1 0.5 s, 1.0 s over 45 m from the camera. **[V][L: 291,678 of 292,062 readings]**
- **The soldier's AI input** is B2 `FUN_00249f80` (not `FUN_001f02e0`, the droid's): fire controller + the shared
  steering with MaxSpeed / MaxStrafeSpeed - full stick at the move point while the body faces the aim target. **[V]**
- **Route**: weighted random draw over the authored table at every hub; the waypoint a random point on the next hub's
  sphere per unit per leg; no avoidance of other units before contact (contact -> 5 m side-step). **[V]**
- **The fight**: a per-controller number (drawn once) picks stand / circle / zig-zag / flank by the level's weights
  (Elite {1,1,5,1}) and the open space; a range band; **Cover hints in a fight** (a 50 % draw every 2-5 s: 21 % of
  attack time on Kamino, 49 % on Polis Massa); "go to the noise" within 30 m; no retreat, no own-health or ammo rule;
  the trigger a 22.5 deg cone. **[V][L]**
- **Team mates**: fire is held only for a human friend on the aim ray; the sight ray is blocked by soldiers. **[V]**
- **Turrets** (both games): boarded with a live threat inside a weapon arc, left after 10 s with no live threat - a
  remembered enemy or an enemy turret 100-180 m away resets the clock. **[V][L]**
- **The maintainer's three**: (a) the turret clock above; (b) every never-visited hub lies off every authored post-to-post
  route (Kamino 45 of 142, Polis Massa 52 of 185); (c) BF1's walk is requested once and a failed walk is never
  retried, the board agent has no timer (14 failed board bots in 37 save states, 12 heading for a turret 4-18 m
  above or below them); BF2's 30 s limit is reset by a failing walk. **[L]**
- Verifier (two passes, 84 claims): 50 SUPPORTED, 33 PARTLY folded in, 1 NOT SUPPORTED (the droid's function named
  as the soldier's; replaced). Evidence `engine/artifacts/lead_tmp/sair/`.

Ported: nothing (an RE job). Corrections to `re-conquest-goal.md` 4, `re-ai-difficulty.md` 2.4, `re-blaster-damage.md`
6.3. New cards HUBP, ENGR; notes on FGHT, GRAI, JETP; the rest is BOTD's.

## 2026-10-02 (loop lane 3, AUDIT CONT) — the Conquest goal holds; a planning hub with no arc for the mover is not a hub → `analysis/re-conquest-goal.md` banner and section 10

Audit of the landing CONT (4c1460d5) and of `re-conquest-goal.md`: PARTLY (`engine/artifacts/loop/audits.md`).
- The doc's rules re-derived from both ELFs with a fresh verifier (33 / 3 / 0 of 36); the two live battles re-read
  (value-weighted draws 368 of 556 against 373.4 / 283.9; one in-life change per 224-333 s). **[V][L]**
- New: the PS2's locate skips a hub whose mask (the OR of its arcs' masks) has no bit for the unit (BF1
  `FUN_00266678`, BF2 `FUN_002eb918` / `FUN_002ebf70`); the port's does not, and TAT1's arc-less 525 m hub makes
  its bots hunt 52 % of the time. **[V]** The planning-graph entry above carries the correction.

Ported: nothing (an audit). New card HUBM; notes on LLEG and CONT. Evidence `engine/artifacts/lead_tmp/audit-cont/`.

## 2026-10-02 — Weapon and ordnance sounds beyond fire; the `soundlayered` record (card WSND) → `re-weapon-sounds.md` [O]

- **`soundlayered`**: `Layer(layer name, sound, delay [, deviation])` (B1 `FUN_0045b7b0`, B2 `FUN_0054e6f8`); the
  sound is looked up at parse time, so a layer naming a later-defined or absent property is dropped (BF1's
  `com_weap_mine_land` keeps one of two layers; RAM agrees). A delay of 0 starts at once; any other layer waits in a
  78-entry timer pool and starts at delay + deviation x U(-1, 1) at the armed position (B1 `FUN_0045ca08` /
  `FUN_0045bc10` / `FUN_0045be28`; B2 `FUN_0054fa38`...). PS2 logs: the grenade explosion's second layer +0.100 s in
  11 of 11. **[V][L]**
- **A start** draws pitch +- `PitchDev` and gain +- `GainDev` (B1 `FUN_0045ac00`); a looping sound on a handle that
  got no voice is retried every update (`FUN_00355a28`). **[V]**
- **Explosion**: the class's `SoundProperty` once, first, unconditionally (B1 `FUN_002701a0` 193760, B2
  `FUN_0030c8f8` 300192). **Hum**: `OrdnanceSound` loops on the ordnance's handle from the first update in range to
  its death, pitch x the doppler term (B1 `FUN_0044cf80`: 344 m/s, off above 172 m/s closing). **[V]**
- **Weapon**: `WeaponChangeSound` on select (spawn and switch, every unit; BF2 also the active-channel change),
  `ReloadSound` for every unit, `FireEmptySound` human-only, `ChargeSound` with a pitch ramp capped at 0.4,
  `OverheatSound`; `ChangeModeSound` is in neither ELF. **[V][L]**
- **The reload count** was the 50 m cut-off: 12 PS2 starts are all within 48.6 m; per in-range shot 0.055 (PS2)
  against 0.046 (port). **[L]**
- Verifier (57 claims): 49 SUPPORTED, 8 PARTLY folded in, 0 NOT. Its census check found that the port's sound bake
  skips BF1 `kas1gcw` (a sub-level name hash of four printable bytes).

Ported: nothing (an RE job). Card WSND -> build; new card WHUM. Evidence `engine/artifacts/lead_tmp/wsnd/`.

## 2026-10-03 — The bot design, and where the PS2's bots stand against its barriers (card BOTD) → `bot-design.md` [O]

A design note, not an RE: one architecture for the foot soldier's AI cut from `re-soldier-ai.md` and
`bot-quality.md` (layers with one owner per output, a behaviour table with entry / exit / minimum time, scored and
reserved positions, one avoidance, tiers as competence, 16 named deviations, bars B1-B14, 16 build cards). Three
facts about the PS2 came out of its measurements and belong here:

- **The PS2's bots stay out of the barriers.** On foot they stand inside a soldier barrier in 0.3-1.2 % of their
  rows (BF2 Kamino 0.8, Polis Massa 0.3, Death Star 1.2, BF1 Kashyyyk Docks 0.3 %; the mission's switched-off
  barriers left out) and within 1.0 m of one in 23-26 %; in the fight order 13-40 % within 1.0 m. The port's bots:
  1.1-3.9 % inside. **[L]** (`lead_tmp/botd/tools/inbarr.py` on BOTM's live-RAM captures.)
- **29 % of the authored Cover hints lie inside a soldier barrier** (1,194 of 4,129 in the base layers; 43 % inside
  one grown 0.3 m). Of the hints outside, 72 % have something solid at crouch height within 3 m along the stored
  +Z row and 17 % along -Z: a hint's +Z row is "ahead", toward the cover. **[D]** from the port's collision
  (`hint_in_barr.py`, `cover_census.patch`).
- **Toward a post the authored route table has one way:** entries with 2 or more alternatives are 0-2 % on ten
  maps (0 of 580 on BF2 Mygeeto, 0 of 1,104 on Polis Massa, 17 of 846 on Kamino). **[D]** from the disc plans
  (`flank.py`; its off-route hub counts equal `re-soldier-ai.md` 10b's on Kamino and Polis Massa).

Verifier (81 claims): 65 SUPPORTED, 16 PARTLY folded in, 0 NOT; it sank the draft's "96 % of stalls cross a
barrier" (a random line does in 90 %). Ported: nothing. Evidence `engine/artifacts/lead_tmp/botd/`.

## 2026-10-03 — Vehicle sounds: the engine loop, walker steps, turret whir, boarding, collision (card VSND) → `re-vehicle-sounds.md` [O]

- **`EngineSound` is a `soundparameterized` record**: a `soundlayered` of 1 - 4 loops plus per-layer
  `ParameterGraph(parameter, graph, target)`; a graph is `ControlPoint(x, y)` points, linear between them and flat
  outside. Graphs on one target multiply; per sound update gain = clamp(property gain x factor, 0, 1), pitch =
  max(0.1, property pitch x factor) (B1 `FUN_004610b8` / `FUN_00461220`; B2 `FUN_005536c8` / `FUN_00553838`). **[V]**
- **The engine component** (B1 `FUN_00365bc0`, B2 `FUN_0044c650`; hover, flyer, walker, droideka, the jet pack) sends
  11 parameters (BF2 12); the discs use `speed` -> Pitch, `acceleration` -> Gain, `health` -> Gain, `turnontime` /
  `turnofftime` -> Gain and Pitch, BF2 `proximity` on the guided rocket. On while the pilot seat has a pilot;
  `TurnOffTime` (3.0) of run-down after he leaves. A hover's speed is |forward speed| / `ForwardSpeed` (`ReverseSpeed`
  when the throttle is not forward) with a 1 %-per-update acceleration filter; a walker sends m/s and - an
  operator-precedence slip kept in BF2 - `previous - speed / dt`. **[V]**
- **Live**: 19,374 of 19,419 layer-ticks of three BF1 hover classes equal the model within 0.002, a turn-off (stop
  at timer 3.003) and a turn-on included; a BF2 state holds the walker's values. **[L]**
- **Walker steps** come from each foot prim's height above the walker's origin (BF1 a line 0.05 m above the foot's
  lowest height, BF2 `StompThreshold`; type 1 walkers by the descent per update), `HydraulicSound` (0x069134E5) once
  per upswing. **Turret whir**: two looping voices, pitch and gain by turn rate, x0.6 per update when the angle
  stops; `TurretActivateSound` when a turret seat gets a pilot (AI too). **Collision**: hover with collision damage
  only, walker on any closing vehicle contact. **[V]** (second reader, key functions re-read) **[L]** (a boarding log,
  SNDC's step intervals 0.33 / 0.5 s).
- Names found by hash: `TurnOnSound` 0xEA603E52, `TurnOffSound` 0x042DABAE, `TurningOffSound`, `WindUpSound`,
  `WindDownSound`, `HydraulicSound`, the parameters `health`, `strain`, `revs`, `winduptime`, `winddowntime`,
  `proximity`. 0x1ADA38C1 (`WeaponChange`) is read by neither ELF.
- Verifier (62 claims): 53 SUPPORTED, 9 PARTLY folded in (seven line slips, a skipped step, a replay that chose its
  timer - the fixed-rule numbers are the ones quoted), 0 NOT.

Ported: nothing (an RE job). Found in the port: a seat gun's shot plays the rider's infantry `FireSound`. Card VSND ->
build (seat-gun fire + engine loops); new card VSN2 (steps, whir, boarding, collision, creature cries). Evidence
`engine/artifacts/lead_tmp/vsnd/`.

## 2026-10-03 (loop lane 2, card HAIS) — how a bot drives a hover; Endor's lost speeder bikes → `analysis/re-hover-ai.md`
`[O]`, one Opus verifier in the foreground (49 claims: 49 SUPPORTED, 0 PARTLY, 0 NOT SUPPORTED; refinements folded
in), and a tick-by-tick replay against live PS2 RAM in both games (`lead_tmp/hais/hai_check.py`, `hai_check1.py`):
BF2 Endor 22617 of 22621 ticks equal on throttle, strafe and turn, Kashyyyk 15815 of 15832, Geonosis 6491 of 6492;
BF1 Endor 24089 of 24222.
- **The hover's AI** `FUN_00220528` (controllable slot +0xcc; BF1 `FUN_001eedc0`) calls the shared steering
  `FUN_002a54d8(dt, ForwardSpeed, StrafeSpeed, TurnRate, PitchRate, …)` and adds five rules: an **obstacle probe** (a
  level ray 1.5 × speed ahead from 1 m up, against soldiers, vehicles and buildings, not terrain; a hit moves a copy
  of the goal 0.25 × that length to the side its normal points and halves the throttle), a ±0.5 throttle floor while
  turning for a `MovingTurnOnly` class (K+0xe75, hash 0x3c45420e; gated by mover bit 4), an **unstick manoeuvre**
  (when the path follower says stuck — not 2 m in 3 s, or no new closest distance to its node in 5 s: reverse at −1
  with turn +0.5 until the nose has swung past 90°, then +1 / +0.5 for 3 s; cooldown 15 s, BF1 4 s), turn × 0.6 under
  a throttle of 0.1 (BF2 only), and the 20 % bail-out (BF2 only). The probe is off for `AISizeType` HUGE
  (class+0x330, hash 0xaff22806).
- **The sticks.** With a StrafeSpeed: inside atan(SS / FS) of the goal throttle 1 and strafe tan(error) × FS / SS,
  outside it strafe ±1 and throttle (SS / FS) / tan(error). Without one: throttle cos(error). Turn (`FUN_002a45a0`'s
  hover path): u = clamp(s × (4.5 − clamp(TurnRate, 1, 3)), ±1), turn = u·|u| — no dt in it. Arrival and the stop
  radius are ForwardSpeed / 2; the goal is |v| + 0.2 m ahead on the path segment. Order in a frame: mover → AI (from
  the body of the tick before) → drive.
- **Endor, the PS2's own bots, 5.00 minutes each**: BF2 lost 4 bikes with a rider, **none by crashing** (19 rides);
  BF1 lost 16, **15 by crashing** (54 rides, 3.0 a minute) — two of them inside the unstick manoeuvre. The law is the
  same; BF1's bike does 40 m/s with no strafe and takes (normal speed)² of collision damage against 600 health, BF2's
  28 m/s with strafe and 0.1 × (speed − 4)² against 80.

Ported: nothing (an RE job). Card HAIS → done; the build is VDROP's, with its acceptance re-based on these numbers
(BF1 Endor's crashes are the PS2's own). Evidence `engine/artifacts/lead_tmp/hais/`.

## 2026-10-03 (loop lane 3, card BPRL) — vertex colour is blended in screen space behind a guard-band clipper; DEA1's killed post → `analysis/re-vertex-colour-interp.md` [O]

The question: UTA1's pen floor gains +0/+16/+38 under its own post in the port and +0/+1/+3 on the PS2; DEA1's cp5_cp6
corridor is washed blue; COR1's cp2 floor brightens. The card's lead (a draw flag that skips the gathered light set)
is refuted; the answer is two other things.

- **No draw skips the set.** `FUN_0057cf60` gathers unless `flags & 0x80000010`; a draw with 0x10 is UNLIT
  (`DAT_007c5768` is zeroed after every model draw and `FUN_0058ed70` then drops the lit bit), not lit by the last
  set. The pen is a prop: `FUN_0022edd8` → `FUN_001c5610` → slot +0xc with object flags | 0x80 | 0x1000000 = 0x1000083
  in RAM, model prelit; 24 of the frame's 128 light sets are prelit sets holding exactly the post's light.
- **The GS blends a triangle's vertex colour linearly in screen space** (only ST/Q is in perspective), and the VU1
  clipper (0x311–0x392, 0x666–0x72c) cuts a triangle only beyond |x| ≤ 7.994 half-widths, |y| ≤ 9.135 half-heights
  (VU[85] = (w/4096 + 0.0001, h/4096 + 0.0001, 1), `FUN_00580600`; BF1 `FUN_004997d0`) or the near/far planes, with
  one Q for position, st and colour. The stored poses stand the camera on the post; the pen floor is a fan of 6–15 m
  triangles round a vertex 0.35 m under the light. A raster model of that clip and blend, drawn from the disc, gives
  the PS2's measured gain: +1.2/+3.0 against +1.2/+2.9 (HUD-free box +1.9/+4.5 against +1.9/+4.8); a perspective
  blend gives +20/+49. COL0 goes the same way (0.80 against 0.55 in that box).
- **A GPU-safe form**, value = persp(a·wc)/persp(wc) with wc = max(w, |x|/7.994, |y|/9.1345, near), in a port
  prototype over the 954 stored poses: all maps 10.672 → 10.639, COR1 6.745 → 5.945 (its cp2 floor 103/92/68 →
  70/63/48, PS2 70/65/51), TAN1 −0.48, MYG1 −0.18, UTA1 −0.15; KAM1 +0.73 — the post's pool had hidden a deck 1.8×
  too dark (PS2 82, port 45). Plain `@interpolate(linear)` leaves a black wedge at MYG1 and a white frame at a
  broken BES2 pose.
- **DEA1**: `dea1g_con` / `dea1c_con` call `KillObject("CP6")`; the PS2's battle has six post lights and none within
  54 m of cp6. The port's battle drops CP6 too; `still_lights` (renders with no battle) lights every placed post.

Verifier, 20 claims: 16 SUPPORTED, 4 PARTLY, 0 NOT; folded in. Ported: nothing (an RE job). Card BPRL → build
(`render/src/shader.wgsl`, `game/src/point_light.rs`); a note on card B1DK (BF2 Kamino's deck is dark like BF1's).
Evidence `engine/artifacts/lead_tmp/bprl/`.

## 2026-10-03 (loop lane 2, card AMBM) — music, ambience streams, command post / prop loops, interface sounds → `analysis/re-ambience-music.md` [O]

The question: what starts the battle's music, its ambience streams, the post and prop loops and the menu sounds.

- **Ambience streams are world regions.** `soundstream <stream property> <divisor>` -> at load a looping 3-D stream
  sound at the region's centre, MinDistance = |SIZE| / divisor, Max = Mute = |SIZE| (B1 `FUN_0035c2f8`, B2
  `FUN_00441c40`), all started once at battle start (`FUN_0035bff8` <- `FUN_0033e590`). RAM: NAB1 `nab_amb_meadow`
  420.97; UTA1 `uta_amb_wind` 235.70 / 471.40, `amb_hangar` x 2. 52 (BF1) / 168 (BF2) such regions, no
  `soundstatic` region on either disc. Fixed sample emitters come from `com_snd_amb_static` world objects (296 /
  406; MYG1's 37 neon signs) and from props with `SoundProperty` (BF1: a static entity made with the prop; BF2: a
  handle on the prop, started when the prop is first drawn).
- **Music** is a `Music` record (`mus_` chunk: `SoundStream`, `FadeInTime`, `FadeOutTime`, `MinPlaybackTime`,
  `MinInactiveTime`, BF2 `MaxPlaybackTime`, `Priority`) per team and state. The state: BF1 `FUN_002b1280` = team 2's
  (the last team processed wins); BF2 `FUN_00373ad0` = the minimum of the two teams. One Music at a time; no music
  before the first human spawn. PS2: BF1 Naboo's `middle` plays a take, is silent 30.3 s, plays the next (233.024
  and 294.586 against 232.73 and 294.31 predicted); RAM: 105 / 105 BF1 and 122 / 122 BF2 states follow their rule,
  one BF2 state separates the two rules. Battle end: `soundfx` faded out, the listener's victory / defeat Music.
- **Command post**: the hum is the class `AmbientSound` of the OWNER's faction (`all` / `rep` goodie, `cis` / `imp`
  baddie), kept by a neutralised post, absent on a post that starts neutral; one capture handle (`ChargeSound` /
  `DisputeSound` on a neutral post, `DischargeSound` on an owned one) with pitch `P x progress + 1 - P`, P =
  `SoundPitchDev` 0.4; `CapturedSound` / `LostSound` one-shots; `CaptureMusic` / `LostMusic` are authored by nothing.
- **Interface**: the spawn screen (engine) plays the five `SetSoundEffect` slots; the pause menu (Lua) plays
  `shell_menu_accept` / `shell_select_change` / `shell_menu_exit`.
- Corrects `audio-census.md` (hum by friend / foe; the Utapau music sequence; "battle ended") and card AMBM's
  thresholds (0.99 / 0.1 are Naboo's script, Utapau's is 0.8 / 0.2).

Verifier, 57 claims: 56 SUPPORTED, 1 PARTLY (one `com_snd_amb_streaming` object on YAV1), 0 NOT; folded in with its
list of qualifiers (`MinInactiveTime` cracked; BF2's looping takes; `BuildingSound` silent on the PS2). Ported:
nothing (an RE job). Card AMBM -> build (music); new cards AMBS (emitters), CPSN (post sounds), UISN (interface).
Evidence `engine/artifacts/lead_tmp/ambm/`.

## 2026-10-03 (loop lane 3, AUDIT JARC) — the jump's gates, JumpHeight, the ledge a jump mounts → `analysis/re-stance-roll.md` §4.3c
- **4.3b's take-off rules hold** on 32 new PS2 trials (diagonal, strafe, back diagonal, sprint), the port within
  0.007 m/s and 1 cm on BF2; the order is in the code (the stance case's mover, then `FUN_00247528` -> slot +0xac ->
  `FUN_00246ed0`; BF1 slot +0x80 at B1:117439). **[V][V-PS2]**
- **A jump press is taken for 0.3 s off the floor** (B2:160285-160310, sprint B2:160530; BF1 by its stand timer,
  B1:117388-117414), **never from a crouch** (B2:162075-162077, B2:160394-160397: Cross stands up), and BF1's waits
  0.5 s in the stand stance. **[V][V-PS2]**
- **JumpHeight** -> sqrtf(2 h x 18) in `FUN_00252bb0` (B2:170972-170976), default 8.0; 1.78 / 2.5 / 3.5 / 4.5 on
  the disc. **[V]**
- **The ledge a jump pressed against a face mounts**: BF2 only with its apex 0.42 m or more over the top (13 of 13;
  never at 0.35 m or less, 12 trials) - 1.44 m yes, 1.51 m no; BF1 with its apex up to 0.15 m UNDER the top, not
  0.25 m (16 trials) - 2.07 m yes, 2.17 m no. The running jump is unmeasured. **[V-PS2]**

Ported: nothing (an audit). The port drops a press off the floor (1.0-5.7 % of moving ticks), jumps from a crouch,
mounts 1.75 m (BF2) / 2.16 m (BF1) and pops through decks up to 2.60 / 2.70 m. Cards JGATE, JLEDG; a note on VCOL.
Evidence `engine/artifacts/lead_tmp/audit-jarc/README.md`.

## 2026-10-03 (loop lane 3, card DEKA) — the droideka as a fielded unit: shield, walk form, collision, AI, measured motion → `analysis/re-droideka-unit.md` [O]
Two Opus readers, one verifier (37 claims: 28 SUPPORTED, 9 PARTLY folded in, 0 NOT), 450 droideka rows out of 219
stored save states and two live PCSX2 samples timed by the game clock (`lead_tmp/deka/tools`: deka_ram / deka_live /
deka_ana / deka_prims).
- **The ball moves at twice its stored velocity** in both games: the entity's two thread updates each integrate
  (B1:79913-79941 + 80289-80317; B2:109626-109653 + 110005-110033). Live ratio 1.999; top 12.96 m/s BF1, 11.99 BF2;
  free fall 39.2 m/s^2. CORRECTS `re-droideka.md` (6.5 / 6.0 m/s were the stored values). **[V][M]**
- **Fold 0.50 s, unfold 1.67 s in both** (BF1 plays every clip at rate 2.0, key 0x6b8d8679; BF2 RollTime /
  UnrollTime); the 12 clips of `cis_walk_droideka` named and counted. **[V][M]**
- **The shield** (weapon class `shield`): 3000 points, -100 /s up (`AddShield`), +300 /s down (`AddShieldOff`,
  0x729dfa7a, cracked), a press toggles, forced down in states 0xb..0xd and at 0; a sphere r 1.25 in its own layer
  that takes a bolt's damage x ShieldScale and passes none on; a bolt reaching the body skips it. BF2's update leaves
  a shield at exactly max undrained. **[V][M]**
- **The AI's form order** (BF2 ai+0x504, BF1 E+0x40c) defaults to roll; the combat helper writes deploy on enter and
  every update, roll on exit; the stuck walk the same. Shield raised only in the attack state, when down and over
  half; never lowered. Sight starts a fight only within 10 m; mode 2 (stand and shoot) only. **[V][M]**
- **Walk form**: BF1 root motion x 2 (2.26 m/s measured; MaxSpeed 5.0 only feeds the AI), BF2 the clip scaled to
  MaxSpeed 2.5; no acceleration, BF1 no strafe; no ground probe (the "probe heights" are water depths). **[V][M]**
- Collision bodies of both discs' model (BF1 by class keys incl. `BuildingCollisionPrim`, BF2 by MASK + `coll_ball`).

Ported: nothing (RE). Build cards DEKU (the unit in the sim), DEKS (the shield), DEKD (bake + draw + sounds), DEKF
(fielded: roster + bot rules). Evidence `engine/artifacts/lead_tmp/deka/` (CLAIMS.md, verify/VERDICTS.md, live/).

## 2026-10-03 (loop lane 2, card TBLD) — how a build pad becomes its turret: the building's build state, the fusion cutter, the bots that build, the hologram → `analysis/re-build-zones.md` [O]

The question: what turns BF2's (and BF1's) flat build pads into turrets, who does it, how fast, and who sees the hologram.

- **A build is a repair to full health.** A building's state is 0 built / 1 destroyed / 2 unbuilt (B2 E+0x274, B1
  E+0x2a4). Every health write goes through the handler `FUN_001e3458` (B1 `FUN_001bfa88`): health 0 in state 0 ->
  death (`FUN_001e3638`: state 2 for a class with an unbuilt model and no destroyed one, else 1; regeneration =
  MaxHealth / RespawnTime); health = max in a state other than 0 -> built (`FUN_001e3c38`). **[V][L]**
- **RespawnTime defaults to 120 s for an armed building** (`FUN_001e8860`, B1 `FUN_001c4cd0`): BF1's gun turret
  (300 HP, no key) regenerates 2.50 HP a second after a fall (8,366 live samples) and rebuilds itself; BF2's TUR
  turrets say 999999.9 and wait for a builder. **[V][L]**
- **The fusion cutter** (class `repair`; B2 `FUN_00347d48` / `FUN_00346c78` / `FUN_00346fa0`, B1 `FUN_00292f60` /
  `FUN_002929b8` / `FUN_00292ce0`): an amount per target type (the low three bits of the entity's flags: 3 vehicle,
  4 building, 5 destroyed, 6 unbuilt, 7 mine) once per ShotDelay. BF2 200 / 100 / 50 / 50 every 0.5 s; BF1 60 / 60 /
  20 / 30 (four sides: half the amount at 0.25 s). Live: 120 of 120 steps of 50.0 at 0.5 s on Mos Eisley, five
  builds of 7.5 s; 30.0 on Naboo, ten builds of 4.4-4.5 s. **[V][L]**
- **Who builds**: bots of `UnitType` repair / support / special / pilot with a cutter; every 5 s, within 75 m, a
  friendly or neutral thing lacking health - unbuilt and destroyed buildings qualify though dead; one claim per team
  per building; approach to 1.6 m, hold the trigger; give up after 20 s out of position or 8 s without gain (B2
  `FUN_002d6828` / `FUN_002d76a8`, B1 UnitRepairAgent `FUN_00249f68` / `FUN_00256fe8`). Live builders: Jawas on Mos
  Eisley (120 of 120), CIS engineers on Yavin (48), the pilots on Naboo (183 of 184). **[V][L]**
- **BF2-only quirks**: a bot under 1 m from the pad's centre is "not in position" and stands 20 s (22 of 79 spells);
  the AI sets the weapon's target with no range test, so a bot still holding the trigger builds its next pad from
  up to 59 m (28 of 120 steps). BF1 has neither. **[V][L]**
- **The hologram** (type `build`) is hidden per screen unless the screen's soldier is alive and carries a
  `WeaponRepair` (B2 `FUN_002172a8` / `FUN_0024daa0`, B1 `FUN_001e6a68` / `FUN_0020c5b8`); frame
  `lead_tmp/tbld/ps2/yav1_holo_1b8b5c0_1.png`. **[V][L]**

Ported: nothing (RE). Cards: TBLD (build: the turret's three states in the sim), FCUT (the cutter and the hologram),
BLDR (bots build and repair buildings); BZONE1 now waits for BLDR. Verifier, 37 claims: 33 SUPPORTED, 4 PARTLY
folded in, 0 NOT. Evidence `engine/artifacts/lead_tmp/tbld/` (verify/claims.md, VERDICTS.md, ps2/, tools/).

## 2026-10-05 (loop lane 1, AUDIT ANNC2) — BF1's sound sub-levels: one name, one definition per era → `analysis/re-announcer.md` banner

Audit of the landing cc4219f (card ANNC2), PARTLY. What was built holds (the conquest script's bleed rate, the lines by
the listener's army). New facts, found while asking why the maintainer heard droid lines as the Empire in a default battle:

- **A BF1 planet SOUND LVL defines the same sound name once per sub-level, with that era's own audio.** KAS.LVL has
  `kas1gcw`, `kas1cw`, `kas2gcw`, `kas2cw`; `imp_off_com_report_lost_commandpost` is stream gcw_vo, segment 1c18b3b0
  in the gcw pair and stream cw_vo, segment bdf175ac in the cw pair (the same for `imp ..captured`, `all ..captured`,
  `all ..lost`, the `rep_` / `cis_` twins and the `..3` variants). **[V]** raw bytes,
  `lead_tmp/audit-annc2/sublevel_defs.py`.
- **The PS2 holds the one sub-level its mission names**: `kas2i.luac` reads `sound\kas.lvl;kas2gcw`, `kas2c.luac`
  `kas2cw` **[V]**; in all 160 BF1 battle save states of the library (96 GCW, 64 Clone Wars) each state holds one
  era's names and the six post-line names sit beside that era's stream hash, never the other's **[L]**
  (`ram_defs.py`, `ram_defs_all_bf1.txt`).
- **The port keeps one definition per name for the whole LVL** (`game::sounds::merge_props`, marked PROVISIONAL): the
  last sub-level's, `<planet>2cw`. Of the 28 sub-levels BF1's missions load, 16 hold names the port resolves to
  another sub-level's audio: 256 names, 201 of them streamed voice; every GCW mission on a two-era planet has the 12
  post lines among them. BF2: 0-4 names a mission (`sublevel_census.py`).
- The mission scripts' commander lines are per era: 0 of 21 BF1 Clone Wars scripts set a low-reinforcement line,
  BF2's `yav1c_con` sets no line; the shell starts `<map>_con.ron` for every pairing.

Ported: nothing (audit). Cards: SNDE (the battle's sounds from its mission's sub-level), ERAM (the shell loads the
picked pair's own mission). Verifier, 18 claims: 16 SUPPORTED, 2 PARTLY in wording, 0 NOT. Evidence
`engine/artifacts/lead_tmp/audit-annc2/` (README.md, CLAIMS.md).

## 2026-10-05 (loop lane 2, card ANIM) — the soldier animator: weapon pose sets, two body layers, firing on the move → `analysis/re-soldier-anim-layers.md` [O]

Closes HANDOFF HOLD 2 (blend layers, the anim-set giants) and the droid banks / `_lower` naming of HOLD 3.

- **Two clip players per soldier, one kept pose.** The LOWER player (hips, legs) plays the rifle's movement clip
  whatever the weapon; the UPPER player (BF1: 13 named bones - spine, ribcage, neck, head, arms; BF2: every bone but
  root, pelvis, legs) plays the carried weapon's version of it, or the weapon's shoot / charge / reload clip while
  one runs. Only a soldier standing still plays the action on his legs too. **[V][M]** BF1 `FUN_0022aa10`
  (call sites 0x22b7b0 / 0x22b7dc), BF2 `FUN_0027dfe0` (0x27e84c / 0x27e940).
- **BF1 sets**: 9 fixed rows by the weapon's `AnimationBank` (rifle, bazooka, pistol, grenade, tool, lightsabre,
  sbdroid, bdroid, cannon), six tables in the global set object `DAT_00517ae8` built by `FUN_0022d828`, one
  fallback table (`DAT_00535598`). Re-built from the disc and compared with PS2 memory: 979 of 979 slots. **[M]**
- **BF2 sets**: one per (skeleton family, weapon animation class), 151 {upper, lower} pairs each, filled by
  `FUN_00282f78` from `<family>_<class>_<animation>[_upper|_lower|_full]` along the two parent chains
  (classes: rifle root, bazooka and tool under rifle, pistol and melee under tool; families under `human`).
  Re-built from the disc: 665,176 of 665,176 slots over 176 battle states, 38 distinct sets. **[M]**
- **The super battle droid**: BF1 weapon set `sbdroid` (25 `human_sbdroid_*` clips), BF2 family `sbdroid` (25
  `sbdroid_rifle_*` clips, CIS.LVL); in both over the human rifle's legs. **[M]**
- **Blend**: BF1 pulls the kept pose toward the clip by `t / T` each update (T in code: 0.15, prone 0.4, shoot
  0.01, reload 0.5); BF2 crossfades linearly over T, and its T, Loop, AimType and sync groups are disc data - the
  `sanm` chunk of INGAME.LVL (192 scopes; 258 of 258 live records equal it). **[V][M]**
- **Alert** (2.0-2.5 s after a shot or while a target is held) switches to the alert clip rows and turns the torso
  onto the aim line (spine / ribcage); otherwise only neck and head look. Limits +-63 deg pitch, +-36 deg yaw. **[V]**
- **Strafing**: neither game ships standing strafe clips; the soldier plays the forward or backward clip with the
  legs twisted (BF1 up to 72 deg, alert -45 / +31.5). Corrects `re-foley.md` 4.4's reading of BF1's "left runs".
- RAM, all reference states: the two players of every posed soldier hold the predicted clips, BF1 155 of 155, BF2
  182 of 182, five of them firing on the move. **[M]**

Ported: nothing (RE). Cards: ANIMS (bake the sets), ANIML (the two players and the action layer), ANIMA (alert and
aim), ANIMM (movement ids, rate, twist), ANIMX (what stays unread: first person, low detail, seats). Verifier, 51
claims: 41 SUPPORTED, 10 PARTLY folded in, 0 NOT. Evidence `engine/artifacts/lead_tmp/anim/` (claims.md,
VERDICTS.md, sets_bf1.py, sets_bf2.py, the `ram_*.py` readers, AGENT_*.md).

## 2026-10-05 (loop lane 2, card B1DK) — why Kamino's decks are dark: the light sum's colour space, the ambient floor, lights on a missing hardpoint, BF1's local lights → `analysis/re-kamino-decks.md`
`[O] UNAUDITED`. The deck is a lit, un-prelit fan of 13 m triangles; five terms separate the port from the PS2
(31 patched-state reloads on PCSX2 SW, a scratch port prototype scored over all 954 pairs):
- **The light sum.** The PS2 adds ambient + sun (+ points) in GS units and multiplies by the texel once (BF2 VU1
  0x1bb-0x1d2 / 0x1e6-0x20d, BF1 0x21f-0x285); on the PS2's own frames the terms add in display units to 1.5
  levels. The port linearises each colour and sums in LINEAR light: 1.37-1.40 x too dark on Kamino (ambient and sun
  of one size). **[V][V-PS2]**
- **The ambient floor.** `bake::lighting::AMBIENT_FLOOR` (0.045 linear = 60/255 display) lifts 8 manifests (COR1's
  ambient is 0 in RAM). It is why the display-unit sum read "mixed" on BF2 in 2026-09 (card LSUM): with the PS2's
  ambients the sum takes all maps 10.618 -> 9.742, no map up by more than 0.5 (COR1's floor 41.8/47.1/57.0 against
  42.7/46.3/54.2). **[V][V-RAM]**
- **BF2: an EntityLight on a hardpoint the carrier lacks is created at the carrier's ORIGIN** (`FUN_001d4db8` /
  `FUN_00451e58`; RAM 13 of 13 lights: KAM1's three bunkers x (2 kamlight + 1 kamlight_blue, r 20) + pod2, POL1 2,
  DEA1 1). The port's bake skips them. **[V][V-RAM]**
- **BF1 lights models with omnis too** (`FUN_00493cf0`, <= 4 per draw, prelit draws included; not the terrain): the
  posts' hologram omni at the building's `hp_hologram` bone (r 8; friend (100,200,100), enemy (200,100,100), neutral
  (192,192,192)), EntityLights at EXISTING bones only (8 maps; BF1's attach rule is the opposite of BF2's), and
  the point weight is ((1 + N.L)/2)^4 where BF2's is ^2. The port has none. PS2 knock-out: the green wash round a
  friendly post is the omni (94/136/112 -> 62/71/80). **[V][V-RAM][V-PS2]**
- **BF1's sun-coloured specular pass** gets the manifest's LINEAR sun colour in `SpecPass::rgb_under`: 3-5 x too weak
  under Kamino's dark sun — the deck's bearing-dependent part. **[V]**
- BF2's bearing-270 highlight is the pass (about 10) plus the HDR bloom it triggers (about 18): GLOWB.

Ported: nothing (RE). Prototype: BF1's eight deck boxes within 10-13 of the PS2 (main 21-58 under), bf1_kam1 19.29 ->
13.4, bf2_kam1 16.71 -> 11.7. Cards: LSUM (re-scoped to a build: the sum + no floor), B1DK (build: the origin lights,
the pass colour), B1LL (new: BF1's local lights). Helper reader for BF1's path, lines re-read; verifier, 30 claims:
27 SUPPORTED, 3 PARTLY folded in, 0 NOT. Evidence `engine/artifacts/lead_tmp/b1dk/` (re/CLAIMS.md,
verify/VERDICTS.md, re/AGENT_BF1.md, ps2/, par/, scratch_knobs.patch).

## 2026-10-05 (loop lane 2, card WTHR) — weather: precipitation, wind, rain shadows, lightning flashes, thunder, the sky bolt → `analysis/re-weather.md` [O]

Scope: what draws the PS2's rain, snow, ash, embers and pollen, and Kamino's lightning, in both games. Hops: the
discs' `fx__` blocks (census of 50 LVLs) → the three effect singletons and their vtables (BF2 Precipitation
`DAT_0060139c` / Wind `DAT_00601394` / Lightning `DAT_006012e8`; BF1 `DAT_00519514` / `DAT_0051950c` / `DAT_005194c4`)
→ parse, setup, update and render of each (the Wind update is a hole, read from the disassembly) → the two vertex
emitters and their VU1 paths → the region class "rainshadow" → the LightningBolt class. Checked against PS2 RAM in
32 walk states and against PS2 frames rebuilt from their own save states.

- **Where**: Precipitation on nine maps (BF1 HOT1 / KAM1 / RHN1 / YAV2, BF2 FEL1 / HOT1 / KAM1 / MUS1 / MYG1), Wind on
  the Hoths and Kaminos, Lightning on BF1 KAM1 / YAV2 and BF2 KAM1, `LightningBolt("skybolt")` on the Kaminos only.
  `Effect("ColorControl")`'s three keys are read by neither ELF. **[V][V-RAM]**
- **Precipitation** (BF2 `FUN_004630f8` / `FUN_004637f0` / `FUN_00475740`; BF1 `FUN_0037e878` / `FUN_0037eeb0` /
  `FUN_00390af8`, every field + 0xc): `ParticleDensity` particles in ONE unit cube, **tiled over the world in 5 m
  cells** (the tile size is a ctor constant); position ← frac(position + ((wind.x, −`Velocity`, wind.z) + offset)·dt/5);
  every 0.5 s offsets, alphas and spin rates step 20 % toward a random extreme. Cells are drawn out to `Range` along
  the view axis with alpha (dpx/d past dpx = `ParticleSize`·128 / tan(fov_h/2)) × (a fade over the last 34 %).
  **[V][V-RAM][V-PS2]**
- **A streak is one GS line** (`FUN_00496040`, header primitive 2 → VU 0x486 / 0x4db): head = the particle (its
  alpha), tail = head − (fall − the tracked entity's velocity (camera-scaled) + offset)·`StreakLength`/30, alpha 0;
  1 pixel of the 512; colour byte = ⌊h·h′/128⌋: **`Color` applied twice** (Kamino (91, 94, 101)). **A flake** is the
  particle sprite overlay with a ×256 size word: edge 2.83·⌊`ParticleSize`·256⌋/256, angle π·frac(2·phase − 1), **its
  own alpha only — the per-cell fade never reaches it**. Alpha blend 0x44, depth tested, no depth write, no fog.
  **[V][V-PS2]**
- **Rain shadows**: `regn` whose name begins "rainshadow" (4–23 a map; kinds 5, 7, 2 in RAM). With one in view the
  cell walk carries a pseudo-distance (2.5 × per open cell, 1 × per shut one, far × 2.5) and shuts every cell that
  overlaps a region in view (`FUN_00537208`; kind 2's test is a centre test, not an overlap) — indoor frames show no
  rain and the model draws none. **[V][V-RAM][V-PS2]**
- **Splashes** (`GroundEffect`): interval = 0.65·spread² / (rate · the view footprint's Δx·Δz); a 100 m ray from 50 m
  over the camera (the ordnance mask); dropped when a 1 m sphere at the hit touches a region in view; BF2 divides the
  rate by the view count, BF1 does not. **[V]**
- **Wind**: current = from + (target − from)·t for t < 1 (t += rate·dt), a hold to 1.5, then a new target = base ±
  `VelocityRange` along it and ± R²/(R + L) across; no reader but precipitation. **[V asm][V-RAM]**
- **Lightning** (BF2 `FUN_0045ae80`, BF1 `FUN_00378120`): groups of `NumSubFlashes` flashes, a crack per group and a
  sub-crack per further flash (unpositioned; silent while every listener is in a sound space); per flash a `FadeTime`
  fade b = B(1 − t/F) + flicker: dome f = `SkyDomeDarkenFactor`(1 − b), **the fog colour set to the dome's bytes**,
  ambients k(saved + 0.333·sun), and the sun **`Color`·(b − s) in BF1 but `Color`·(1 − b) in BF2** (BF2's sunlum is
  an int of 0..1 floats = 0). `HorizonAngleMinMax` is dead. **[V][V-RAM, three mid-flash states]**
- **The sky bolt**: one per group at 200 m (Y 200 → 0) and one every 4–7 s (ctor constants) at 800 m (Y 300 → 100)
  along the look direction ± 0.5; life `FadeTime`; a ribbon of n = len/`BreakDistance` points offset ±`SpreadFactor`/2
  from two seeded LCG streams lerped by a phase, half-width `Width` tapering, `MaxBranches` branches, a child
  re-seeded every frame; additive, depth tested, not fogged. **[V][V-RAM: four live bolts][V-PS2: one bolt rebuilt]**

Ported: nothing (RE). Cards: WTHR → build (the precipitation, value 5); WTHL new (lightning, thunder, bolt; value 4).
Two helper readers (low-level draw; lightning and bolt), their lines re-read and their two surprising claims settled
on PS2 evidence; verifier, 69 claims: 64 SUPPORTED, 5 PARTLY folded in, 0 NOT. Evidence
`engine/artifacts/lead_tmp/wthr/` (claims.md, checks.txt, VERDICTS.md, tools/, vectors/, agents/, out/).

## 2026-10-05 (loop lane 2, AUDIT JGATE) — the gate's update counts, the energy bar, the landing lock → `analysis/re-stance-roll.md` §4.3d
- **4.3c's gates hold** on 84 new PS2 trials with the game's own jump word, timer bits, lock and energy bar per
  update: the off-floor window, BF1's 5 s / touch-down rule (all three parts live), the crouch's rise (now live in
  BF1 too), the dearer cost over MaxSpeed. The button block is a PLAYER's soldier's (B2:160304). **[V][V-PS2]**
- **BF2's window is 10 updates, BF1's 9**: dt 0x3d088888 and the EE's rounding leave the timer at 0.29999998 after
  nine steps against `c.lt.s timer, 0.3f` (0x2437a0); live n = 10 jumps (2 of 2), n = 11 not (0 of 3). **[V][V-PS2]**
- **The energy bar is overdrawn**: `FUN_001e2010` clamps at a floor of minus the dearest of four class costs
  (`FUN_001e1e28` x 4 in `FUN_002517e8`, B2:170681-170684; RAM [100, -30, 20]); no restore in stances 4 and up; idle
  by horizontal speed under 1 m/s; the bit clears at EnergyMinSprint. Live: bar 10 -> -21.333 on a sprint jump,
  depleted 85 updates after the landing. **[V][V-PS2]**
- **A landing's lock**: BF2 0.3 s masks the words on L+1..L+10 and the pad routine reads the next press as held
  (`FUN_002b0548` B2:235889-235892): first jump on L+12. BF1 10.6: dead on L+1..L+18, first jump on L+20.
  **[V][V-PS2]**

Ported: nothing (an audit). The port takes 9 ticks in both games, empties an overdrawn bar (22 ticks depleted where
the PS2 has 85) and takes a press on L+10 / L+16. Card JGAT2. Verifier, 25 claims: 20 SUPPORTED, 5 PARTLY folded
in, 0 NOT. Evidence `engine/artifacts/lead_tmp/audit-jgate/README.md`.

## 2026-10-06 (loop lane 2, card ZOOM) — zoom: the button, the weapon's steps, the camera, the turn rate, the scope → `analysis/re-zoom.md` [O]
`[O] UNAUDITED`. Both games; one rule, BF2 adds a sprint cancel, a melee test and a 70 deg zoomed FOV.
- **R3 is a press toggle, humans only.** The pad's default tables give R3 logical action 4 (BF1, ELF 0x519170) /
  5 (BF2, 0x600e40); the soldier's zoom routine (B1 `FUN_00204578`, B2 `FUN_0023ead8`) returns at once for a unit
  without a player index and toggles on the press update. **[V][M]**
- **Steps** (B1 `FUN_00283c80`, B2 `FUN_003259a0`): off -> `ZoomMin` -> `ZoomMax` when it differs and `ZoomRate` is
  0 -> off. Rifles 2.5 / 2.5 (on / off), sniper rifles 2 / 8 (two steps), launchers and shotguns 2.0, repeater 3.0;
  a weapon with no zoom key (grenades, tools) and BF2's lightsaber cannot zoom. **[V][M: BF1 marksman 2, 8, off]**
- **Cancelled by** a reload of the primary, a switch of the weapon in hand, the secondary's throw, a roll, being
  thrown, death, a seat change; BF2 also a sprint. Kept through walking (same speed), a jump, a crouch, firing.
  **[V][M both games]**
- **Camera**: the projection scale S eases to the weapon's zoom, S <- z + (S - z) expf(-25 dt) (B1 `FUN_001a9f20`,
  B2 `FUN_001bb510`); the FOV base is the class's `FirstPersonFOV` while zoomed (BF1 48 deg either way, BF2 65 ->
  70 deg): rifle 37.69 -> 15.55 deg (BF1), 52.06 -> 24.24 deg (BF2). Third person takes the stance's ZOOM section at
  once (over the shoulder: (0.4, 0.05, 2.8), tilt 3.5 deg); a `ZoomFirstPerson` weapon takes the scope view (the
  eye at the fire point less 0.3 m along the aim, BF2 0.6 m; own body not drawn). **[V][M to the millimetre]**
- **Turn**: yaw and pitch rates, and the aim assist's R, are divided by the weapon's turn divisor
  (`ZoomTurnDivisorMin` / `Max`, named here by hash; no disc class authors them, so the divisor is the zoom).
  **[V][M: 1 / 2.50 on every update]**
- **Spread**: the third posture column is `StandAimSpread` / `CrouchAimSpread` / `ProneAimSpread` (named by hash);
  0.0 on every disc weapon, as the still column: zooming changes no spread. **[V]**
- **Scope display**: the `weapon_scope` quarter picture mirrored into four quadrants, up while zoomed on a scope
  weapon with S > 1; `ScopeDisplayZoomIn` / `ZoomOut` = `binocularzoomin` / `binocularzoomout` while S moves. BF2's
  per-weapon `ScopeTexture` is stored and never applied (one scope texture on the disc). **[V][D]**
- Side finds: BF2's reload is D-pad Down (live); a human's yaw rate ramps over a held stick's first second (the port
  turns flat); the PCSX2 README's "R3 = crouch" was the zoom.
- Checks: `engine/artifacts/lead_tmp/zoom/cite_check.py` (242 cites); one Opus verifier, 52 claims: 48 SUPPORTED, 4
  PARTLY folded in, 0 NOT. Live: `lead_tmp/zoom/ps2/` (six runs, 70 row files).

Ported: nothing (an RE job). Card ZOOM re-phased to build (section 10 is its change list); new card ZSCP (the scope
display and its sounds).

## 2026-10-06 (loop lane 2, card WADE) — soldiers in water: drawn before the near water, walking on the bed, drowning → `analysis/re-water.md` §9 [O]
The maintainer's playtest: a stormtrooper in BF1 Kashyyyk's water ends at the waterline. Section 9 has the hops, the live rows
and the checks.
- **[V][M][O] Draw order.** Both games kick their render buckets in ascending order (B1 `FUN_004a8f18` 567277, B2
  `FUN_0058a598` 721039). Opaque model segments, a soldier's among them, are bucket 5 (B1 `FUN_004cf6f0` 594302, also 7;
  B2 `FUN_0058ed70` 723381); the near flat water is bucket 8 in BF1 (`FUN_0048b9c0` 551306) and 7 in BF2
  (`FUN_00575fd8` 708716), blended `Cs·As + Cd·(1−As)` with Z test and Z write. So the water is laid over a wading
  soldier and his legs show through it; translucent segments (0xd / 0xc) come after and are hidden under the surface.
  No model path selects the water's bucket (callers + the verifier's ELF `jal` scan). Flat water only: the ocean's base
  is opaque in bucket 5. PS2 frames with the water's draw switched off live by one data write (B1 `DAT_00522a78`,
  B2 byte `DAT_0060c1f8`) beside the normal ones: `engine/artifacts/pcsx2/ref/{bf1_kas2,bf2_fel1,bf2_yav1}/wade/`.
- **[V][M][O] The soldier update's water block** (B1 `FUN_00205cc8` 117186–117270, B2 `FUN_00240bf8` 159994–160069):
  no float, no slow-down; triggers dead while the surface is over the fire point (feet + 1.583 m); a timer from 3.0 s
  (class constructor constant, no ODF setter) runs while the surface is at or over the aim origin (feet + 1.800 m),
  then 30 damage every 1.0 s. Live: feet on the bed at 18 depths from 0.27 to 7.28 m; BF1 dead on the 10th tick
  (300 hp), BF2 on the 14th (400 hp); 6.47–6.50 m/s in 1.0–1.4 m of water against MaxSpeed 6.5. A ripple record every
  1 / (0.3 v² + 4) s while moving (one global timer) and a splash effect on a fall faster than 5 m/s.
- **[M] The port's sim floor in water is the PS2's** (22 spots: 21 within 2.2 cm, one on a slope 5.2 cm). BF2 YAV1's bazaar floor is −72.818,
  0.32 m under the plane; the walkabout's soldier stood 0.59 m over it by the gallery's own stand rule.
- Checks: `re_check.py` fn / callers / absent / word / writers; `lead_tmp/wade/tools/lui_scan.py`; one Opus verifier,
  24 claims: 22 SUPPORTED, 2 PARTLY folded in (a line number, a third writer of the aim origin), 0 NOT.

Ported: nothing (a capture + RE job). Card WADE re-phased to build (the skinned passes before the water; the
walkabout's stand); new card WDRN (the drowning and the dead triggers); a note on FXP5 (the wader's ripple and splash).

## 2026-10-06 (loop lane 2, card SPUP) — the sprint start's speed test and a slope → `analysis/re-stance-roll.md` §4.2b
- **[V][M] `0.9 x MaxSpeed < v . the body's forward row`** (B2 `FUN_00240bf8` 160321–160329), polled while L3 is held.
  Live (BF2, one row per update, Hoth x 8 slopes + Kamino's ramp x 3): the full-stick run's number is MaxSpeed x cos^3
  of the floor's angle, so the sprint starts at 1.7, 9.8, 13.3 and 14.8 deg (Kamino's ramp, 0.904) and never at 16.1
  or 19.3; three steeper runs show one update of sprint off a terrain seam [H, not explained].
- Checks: `re_check.py fn`; no verifier (a build card, three lines of code, the live table re-states them).
- Ported: `Soldier::sprint_start_speed` (the PS2's test on a level floor; ours on a slope: 0.9 x what the slope
  allows), `input::STICK_FULL` (a left stick that peaks at 0.984 reads full - the cause on Kamino's ramp).

## 2026-10-06 (loop lane 1, card SNDX) — `FireSound` once per trigger pull → `analysis/re-weapon-sounds.md` §7.7
- **[V][O] The cannon's fire cycle starts `FireSound` once when the pull's refire test passes** (B1 `FUN_00288d10`
  211388-211389, B2 `FUN_0032e8c0` 323540-323541) **and again for a later shot only when `SalvoDelay` > 0** (211421;
  323581, BF2 its lower bound). BF2's shotgun (8 shots, no delay) is one sound.
- **[M] The port started it per bolt**: 8 starts at the ear peak at 5.1 x full scale, 30 % of the frames clipped - the
  "static" of the maintainer's 2026-10-04 playtest. The bake is not the cause (9,160 baked takes against ffmpeg's decode of the
  disc bytes: 9,035 equal frame for frame, 6 takes differ only after a saturated frame).
- Checks: `re_check.py` fn / callers / writers; one Opus verifier, 10 claims: 10 SUPPORTED, 3 qualifiers folded in.
- Ported: `game::event_sounds::fire_sound_starts`. Not found: the rocket's noise (one start each for its fire sound and
  its explosion in the log; card SNDB puts the sound starts in the black box so the next report names it).

## 2026-10-06 (loop lane 2, card GRNP) — the BF2 grenade throw beside the port's → `analysis/re-grenades.md` §13
- **[M] The port's launch and flight are the PS2 BF2's** (15 throws on Mos Eisley, aim -26 to +63.6 deg, standing
  and running: launch point and velocity equal to 4 decimals, flight rows within 2 mm; a runner adds no speed).
  **The bounces are not**: the port rests 0.13-0.23 s sooner and, thrown level or downward, 0.9-1.2 m further (3-7 %).
- **[V][O] BF2's terrain ray returns the first bisection midpoint within 0.1 m of the terrain, not the crossing**
  (`FUN_00256b58` 173311 -> `FUN_0055fb38` 695628 -> `FUN_0055f940` 695535 -> `FUN_0055f818` 695446; exact only for a
  vertical ray). A transcription with it reproduces all 15 throws row for row (2,040 rows, worst 0.9 mm).
- [M] The stick on a parked snowspeeder: stops, arms at once, gone 18 updates later — as the port; the port's hull
  is met 0.81 m further along. A BF1 map plays BF1's grenade (34.10 m / 0.9 s fuse against BF2's 35.49 m / 0.6 s).
- Checks: `re_check.py` fn / word / callers; one Opus verifier, 9 claims: 9 SUPPORTED, 5 precisions folded in.
- Not read: BF1's terrain ray; the body callback 0x162f60 (model floors). Card GRNP -> build.

## 2026-10-07 (loop lane 1, card FGHT) — the radius a soldier is heard at; where the PS2's close contact is → `analysis/re-soldier-ai.md` §15
- **[D] A BF2 soldier is heard by every enemy AI unit within 30 m when it has fired since its last think, else 5 m
  per m/s of its speed (at most 30 m), and at least 8 m** (`FUN_002de708` 266751-266759; the soldier's radius
  `FUN_0024dbf0` 167302 through the vtable word at 0x61e9d0; class+0x8d0 = 5.0 in the class constructor 169621 and on
  33 of 33 live soldier classes of two save states). **No verifier agent ran**: PROVISIONAL, card FGHT2's to verify.
- **[L] "Go to the noise" is not what brings the PS2's bots within 20 m of each other**: built on the port it runs
  at the PS2's share of time and the contact does not move (Kamino 3.5 -> 3.2 %, Polis Massa 25.3 -> 22.4 %).
  On the PS2's Kamino 38 % of the close samples are bots on their capture order that are not fighting, 24 % are in
  the attack (44 % of the attack's time is inside 20 m), and the attackers stand in the defenders' home zone for
  4.9 % of all samples against the port's 0.9 %. A 4-6 s look rate for a bot with no contact is the strongest lever
  found on the port (Kamino 3.5 -> 5.4 / 7.1 %), and not enough.
- Checks: `re_check.py` fn / word / callers (pasted in §15.1). Not read: BF1's twin (`FUN_002375e8`, unit vt+0x2c),
  the byte E+0x7e3, the object at E+0x270. Card FGHT landed the fight's styles only; card FGHT2 has the rest.

## 2026-10-07 (loop lane 2, card VPAD2) — vehicle pads at battle start: the class a pad holds, the post-load tick → `analysis/re-vehicle-pads-load.md` [O]
- **[V][O] A pad's class is one the mission loaded.** The setter looks the class name up in the loaded-class list and
  stores 0 when none answers (B2 `FUN_00371990` 369497-369503; B1 `FUN_002af778` 239878-239891). The PS2's own table,
  read from RAM in every kept state, against the port's: BF2 228 slots, 11 where the port spawns a vehicle the PS2
  never has (HOT1 `vehiclespawn_6` imp_walk_atst; GEO1 3 fighter tanks + 2 STAPs; NAB2 GCW 4 combat speeders; Mos
  Eisley CW 1 AAT); BF1 492 of 492 equal. "Listed by the script's `ReadDataFile`, or a class of the map's own LVL"
  reproduces all 720 slots.
- **[V][O] Both games tick every pad once with dt = 0 in the post-load step; BF2 does it before `ScriptPostLoad`**
  (B2 `FUN_00424e10` 482156-482157, then 482293; ELF 0x425000 `mtc1 zero,$f12`; B1 `FUN_0033ef20` 328176-328181). So a
  post the script re-owns gives its pad one spawn for the world file's owner first: BF2 Hoth's `vehiclespawn_6` holds
  "last spawn team 2, armed 0, no vehicle" beside a team-1 post (a Rebel tauntaun that decayed in about 20 s).
- **[R] The card's AT-ST (HOT1 CP6) is not there at the start on the PS2 either**: armed, 16.17 s left of 45 in the
  0-AI side-select state, alive in the walk state. The port's battle does the same (spawn at 45.03 s); the probe that
  saw nothing had skipped the mission script.
- Also read: the BF2 pad's fields, update, spawn step and decay (closes `re-vehicles.md` §7's "[H: same as B1]").
  Verifier: 42 claims, 40 SUPPORTED, 2 PARTLY (corrected), 0 NOT SUPPORTED. Left [H]: the non-roster `shield` object a
  pad follows; a 75 % health penalty at the end of BF2's spawn step that §7 says BF2 lacks.
- Tools: `engine/artifacts/lead_tmp/vpad2/` — `pads_ram2.py` (a state's pad ring), `census1.py` / `census2.py`,
  `examples/vpad2_probe.rs` / `vpad2_census.rs` (the port's pads as a battle builds them).

## 2026-10-07 (loop lane 2, card VREST) — where a parked vehicle rests: the pad's spawn point, the flyer's LandedHeight, the hover's frame order → `analysis/re-vehicle-rest.md` [O]
- **[V][O] A pad creates its vehicle at the hit of a ray from 1 m over the pad to 1 m under it** (terrain + the
  building layer: B1 `FUN_002b0238` 240466-240503, mask 0x48; B2 `FUN_003725f0` 370291-370339, mask 0x90), **else at
  the pad's own position**. 14 of 215 stock pads stand over 1 m above their ground (none a flyer pad). BF2 only: no
  spawn while a hover or walker stands in the pad's box (B2:370349-370350).
- **[V][R][O] A flyer's constructor adds the class's `LandedHeight` to that position** (B1 `FUN_001d7eb8`; B2
  `FUN_00200a98`, the function Lua's `EntityFlyerInitAsLanded` calls), and `LandedHeight` is the authored value, else
  **minus the min y of the model's `modl INFO` vis box**, stored when `GeometryName` is parsed (B1:89911-89913,
  B2:125205-125207). 18 of 18 live classes in RAM equal the disc model to the float (gunship 4.150177, X-wing
  1.778591, Y-wing 1.328592; snowspeeder 0.5 and jedi fighter 1.1 authored). Landed, the velocity is zeroed every tick
  and the body is a building-layer object. 54 parked PS2 flyers: 53 within 0.5 mm of the rule; the 54th (BF1 GEO1
  `laat3`) stands 0.249 m higher on a solid foliage rock the port's world lacks (card FOLGC).
- **[M] The port's parked flyers are low by exactly LandedHeight** (gunship 4.15 m ... geofighter 0.22 m).
- **[V][R][O] Both games run drive → collide pass → spring tick** (task keys 0 / 10 / 15: the collision world registers
  itself at prio −10, B1:1920 / B2:1806; its update slot is the collide pass `FUN_00163008` / `FUN_00163450`). This
  settles `re-hover.md` §2's [H]; 134 of 134 touching BF2 hovers hold their pending compressions at 0 in RAM. **The
  port runs the spring tick before the contacts**: its parked hailfire never comes to rest (the PS2's stands on its
  springs, origin 0.148 m under the ground, still to 3 mm) and every hover bobs about 3× the PS2's band. [M] With the
  order swapped in a scratch build the port's hovers stand within 2 cm of the PS2 on 92 of 114 pads (31 before).
- Verifier: 34 claims, 32 SUPPORTED, 0 NOT SUPPORTED, 2 qualified. Left [H]: the ±1 m window against a live vehicle;
  BF1's contact chain (by structure); the tasks beside the collision world at keys 1 and 10; walkers.
- Tools: `engine/artifacts/lead_tmp/vrest/` — `ps2veh.py` (every pad's vehicle of a state, both games, flyer fields),
  `lhmodel.py` (LandedHeight from the disc against RAM), `padrule.py`, `join.py` (PS2 against the port's census),
  `examples/vrest_census.rs` (the port's pad vehicles after N s idle, per mission).

## 2026-10-07 (loop lane 2, card VTAT) — turrets and the AI: the seat's own controller, the self-piloted turret, AutoFire, the seat's trigger → `analysis/re-turret-ai.md` [O]

- **[V][O] BF2: a turret seat with an AI pilot has its OWN controller, a UnitStationaryController** (made by the
  seat's virtual `FUN_0026fcc8`, slot +0x84, when its pilot's player index is negative; orders map to wait / combat /
  unboard only, `FUN_002b9118`). Its AI input `FUN_0026fbd0` = the shared fire controller + the shared aim with the
  section's `MaxTurnSpeed` / `MaxPitchSpeed`.
- **[V][L][O] BF2's automatic turret is `PilotType = self`**: the Controllable is its own pilot from construction
  (`FUN_002f47a0`), the armed building's spawn makes each seat's controller (`FUN_001e7e08`), and a pass every 15th AI
  step gives a self-piloted controller with no order the order type 10 = UnitWaitAgent (`FUN_002af348`). It then
  looks, sees and fights as any AI gunner (only "stand and shoot", only a target a weapon has in range) and cannot
  leave (no character). Live, BF2 Hoth 1-flag CTF: 14 of them, all so on 86,605 rows; targets held to 100 m, fought
  to MaxRange + 10 m, the trigger pressed only inside MaxRange (10-77 m for an 80 m gun).
- **[D] No conquest world on either disc places one**: 49 on BF2's ground maps, all in campaign / 1-flag / hunt
  worlds (HOT1 14, KAM1 11, MUS1 10, FEL1 5, MYG1 4, NAB2 3, UTA1 2), plus the dispensed auto turret.
- **[V][O] BF1 has no such AI**: a seat's controller is made only in SetPilot, there is no order pass; its two
  self-piloted classes (the Naboo fambaa's shield and generator) run on two class flags the turret's own tick reads,
  `Controllable = 0` (a LockOnRange ray along the fixed aim sets the weapon's target) and `AutoFire = 1` (the tick
  holds the trigger). That ray is what `re-aim-assist.md` 3.10 / `re-vehicles.md` 9 called the MountedTurret's
  acquisition; it never runs for a turret with the default `Controllable = 1`. Both corrected in place.
- **[V][O] A seat gun has no trigger rule of its own**: one fire controller per game for every controllable kind,
  dispatching on the weapon - a cannon with `MaxPressedTime` charges, the `TriggerSingle` bit taps, a cannon-class
  weapon bursts, anything else is held. `TriggerSingle` defaults to 1 in the base class and to 0 in every class
  built on the cannon constructor (labels cannon, weapon, launcher, towcableweapon, grapplinghookweapon) and in
  repair. Of the discs' distinct seat weapons 103 of 127 (BF2) and 59 of 75 (BF1) burst; Hoth's dish turret is the
  one charged gun in each game. BF1's turret stick is always the deadbeat law, BF2's the hover law over 0.5 rad/s.
- Verifier: 46 claims, 41 SUPPORTED, 5 PARTLY (folded in: the launchers' default, the instance count, two live
  numbers), 0 NOT SUPPORTED. Left [H]: what links the fambaa turret's tick and how its two weapons read the held
  trigger (card FAMB); the watched turrets' re-think storm.
- Tools: `engine/artifacts/lead_tmp/vtat/` — `pilot_census.py`, `inst_census.py`, `mission_census.py`, `seatguns.py`
  (disc), `vtat_live.py` + `live_an.py` (a live sampler of every AI-driven turret seat, BF2), `ram_turrets.py` (the
  same from a state), `ps2/hot1g_1flag_battle.p2s`.

## 2026-10-07 (loop lane 2, card WALKB1) — BF1's own walker: the two-stick turn law, the gait, and a body sphere on the terrain → `analysis/re-walker.md` §13, §16 [O]

- **[V][O] BF1's body turn** is θ = turn·dt·StoppedTurnSpeed (2θ a tick, standing or walking; ForwardTurnSpeed is
  never read), with turn = leftX + (1 − |leftX|)·clamp(−over/(MaxTurnSpeed·dt), ±1) and `over` the aim's excess past
  YawLimits; the aim yaw is clamped, and compensated for the body's turn only in the share 1 − |leftX|. On a 0 / 0
  class (tauntaun, kaadu, AT-TE) both sticks steer. The decomp drops the call's third float: read from 0x21a56c-0x21a608.
  **Replayed on the PS2: 29,241 of 29,241 steps** (six classes on three maps, one human driver), every term
  mutation-checked.
- **[V][O] BF1's gait** is z·`TEMP_AnimationSpeed`·30/N (hash 0x6b8d8679, cracked from the mod tools' ODF guide): 1.5
  on the AT-AT and AT-TE, so they walk 2.22 and 2.80 m/s (live 2.20, 2.76), not the VWLK audit's 1.48 / 1.86. BF2's ELF
  never reads the prop. An UNDRAWN walker steps whole keyframes and loses its clip's last interval at most wraps (66
  of 82 tauntaun clips): the audit's "−8 %". The player's own kaadu runs the full 19.54 m/s.
- **[V][O] A body prim in TerrainCollision** (tauntaun and kaadu, both discs) takes the unclaimed terrain contact:
  v.y = 0 and a push-out along the terrain normal, between the fall and the legs (keys 0 / 10 / 15). Parked on a slope
  it creeps at 0.98·sin φ m/s (21 windows within 0.5 %); climbing, a step d covers d·cos²φ − 29.4·dt²·sin φ·cos φ
  (kaadu 0.9882 / 0.9524 / 0.8930 at 4.8° / 11.1° / 17.7°, the model 0.9885 / 0.9531 / 0.8931; BF2's tauntaun 0.957 at
  10.4°; the feet-only AT-ST 0.998-0.9995 at any slope). The port has neither → card WCON.
- **[D] BF2's code with BF1's data** (the port until now) is neither game: a walking AT-ST, spider, AT-AT or AT-TE
  turns at most 0.06 rad/s (BF2 reads the ForwardTurnSpeed BF1 left at its default 0.03), the left stick cannot steer
  a 0 / 0 class, the tauntaun and kaadu run at half speed. **Call:** BF1-map walkers take BF2's class values for the
  walker body (e0c = 0 on the spider, AT-AT and AT-TE so bots keep BF1's convergent steering); the kaadu takes the
  tauntaun's handling changes with MaxSpeed 19.54. Card WALKB1 is now the build.
- Verifier: 47 claims, 43 SUPPORTED, 4 PARTLY (wording and scope, folded in), 0 NOT SUPPORTED; it found the
  interpolation switch (FUN_0043da00, word +0x1340, set only by the walker's draw). Left [H]: the creep's 0.4 %
  deficit; BF1's body-pitch override, zoom divisor and mouse branch (read, not exercised).
- Tools: `engine/artifacts/lead_tmp/walkb1/` — `b1_live.py` (every BF1 walker per game tick with its sticks; `sample`
  / `poke`), `b1_check.py` (the law's replay, `--variant` mutations, `--census`), `b1_clips.py`, `b1_slope.py`,
  `b2_slope.py`, `creep_fit.py`, `human_drive.py` + `run_human.sh` (mount a walker by RAM placement + Triangle, scripted
  sticks in game time), `ram_b1class.py`, `wprops.py`, `body_diff.py` (BF1 against BF2 walker props per class).

## 2026-10-07 (manual run lane 2, card KNOCK) — blast knockback and the death kick: who a blast pushes, the thrown / tumble / get-up states, the late death, the droid chunk death → `analysis/re-knockback.md` [O]

- **[V][O][M] BF1's push** (soldier vt+0x84 FUN_002009a0 → apply FUN_002008b0): every gathered soldier not thrown
  (5), tumbling (8), getting up (9) or mounted (12 / 13) - living, just killed or a corpse - gets dv = (normalize(sphere
  centre − blast) + (0, 0.6, 0)) · Push · f. v.y > 2 → thrown, else over 2 m/s along the ground → tumble. **On the PS2:
  17 of 17 living and 5 of 5 corpses pushed (0.00 m/s off the model at rest), 8 of 8 forced into the skipped states
  unpushed, 6 of 6 controls outside the radius untouched.**
- **[V][O][M] The thrown soldier**: ballistic at g 13.5 (not 18), no drag, stick at 20 %, no fire; lands after ≥ 0.3 s
  into a tumble (over 6 m/s; brake 0.12 · Acceleration; 0.5 - 2.5 s) or a get-up (brake 0.24 · Acceleration; ≤ 1 s),
  then stands under a 1.9333 s lock (`stand_getupfront`'s 58 frames; read 1.9333 in RAM). A thermal detonator 2 m
  away: 14 m of flight, at rest 18.7 m away after 2.4 s, free after 4.4 s.
- **[V][O][M] The death kick** is +5 m/s along normalize(feet − blast) for anyone dead after a blast's damage step
  (older corpses too, at every blast) and **+5 m/s along the shot for a direct-hit kill** (FUN_00200c30; 23 of 40
  deaths in the audit's battle sample). A blast-killed soldier is thrown like a living one and **dies only after the
  landing** (BF1 2.5 - 3.6 s later, 7 of 7; BF2 1.5 - 1.8 s, 4 of 4). Corpses keep a body for 4.0 s, integrate and
  are damaged, kicked and pushed again.
- **[V][O][M] Droid soldiers** (a class with chunk geometry, ChunkFrequency 0.6): the roll comes at every push call
  before the distance test - success = no push + a sticky mark (S+0xbfd bit 3), failure = pushed and a dead one's
  health set to −4.0 (the decomp drops the float; read at 0x200ad4); at the death the roll again or the mark = the
  chunk death (ExplosionName's blast, the class's chunks at 0.1 + 0.018 · overkill, no corpse). Live on Tatooine CW:
  17 of 34 unpushed, each marked; 8 of 8 pushed dead at −4.0. → new card DCHK.
- **[V][O][M] BF2**: the gate is the blast's (`PushDeadOnly`, class+0xa0 bit 0, default on): 0 of 22 living pushed by
  a thermal detonator, 15 of 15 with the bit cleared in RAM; its apply FUN_002388b8 removes half of the opposing
  velocity, throws at v·n > 3, tumbles at |v| > 8, else flinches. **Correction to `re-vehicle-death.md`: two BF2 disc
  classes author PushDeadOnly 0 - the Ewok's rock and the wampa's smash push the living** (found by both verifiers of
  the absence batch from a raw disc scan; `expc.py` reads the baked classes, which lack the EWK and SNW sides).
- Verifier workflow `knock-re-verify`: 12 agents, 55 claims, 79 verdicts - 43 claims SUPPORTED by every verifier, 11
  PARTLY (omissions and wording, folded in), 1 NOT SUPPORTED by both (the disc claim above, corrected). Left [H]: the
  seats behind state 13, the mount predicate that suppresses the × 9 throw, BF2's flinch and its other push callers.
- Tools: `engine/artifacts/lead_tmp/knock/` — `knock_probe.py` (a grenade among held subjects: living, health 1,
  corpse, forced states, control; `--double`, `--knockin`, `--enemy --droids`, `--dead-only-off`), `knock_ana.py`,
  `droid_ana.py`, `live_summary.py`, `state_pick.py` (a spawn state's player class, grenade and neighbours),
  `claims.md`, `verify/VERDICTS.md`, `out/` (rows, `push_table.txt`).

## 2026-10-07 (manual run lane 1, audit of card GRAI: PARTLY) — the AI's weapon choice read again → `analysis/re-grenades.md` §7.5

- **[V] Held:** the slot walk, band and preference of both games, the grenade gate, window and roll, the bot's number
  (0-100, once), the low-arc solver; BF2's cluster counters skip the unit itself ("over 3" = over 3 others).
- **[V] Corrections** (the audit's reading with `re_check`; no verifier pass yet - card GRAIA): "plenty" is the load a
  weapon was ISSUED (B1 weapon+0x94, B2 ammo[2]: each written once, by the constructor), not the rounds left; BF1's
  score is one shot's damage against the target's kind, not BF2's rate; BF2 does not choose a weapon in state 4
  (reload) or 5 when the choice is applied (B2:231043-231050; weapon vtable +0x5c returns 0 in 15 of 16 vtables);
  BF2's fallback skips the melee kind (vtable +0x60).
- **[M] The PS2's rate** from SAIR's four BF2 captures: a grenade episode (firing channel 0 -> 1, controllable+0x84)
  per 146 / 239 bot-seconds on Kamino, 347 / 363 on Polis Massa, median 2.5-5.0 s; slot 1 in the hands 14.0 / 11.7 %.
- Tools: `engine/artifacts/lead_tmp/audit-grai/` — `README.md`, `ps2/{slots,firech,band}.py`, `ps2/port_slots.sh`,
  `verify/re_checks.txt`, the switch patches `knob_*.patch`, `wf/RESULTS.md` (the workflow's tables).

## 2026-10-09 (loop lane 2, card FGHT2) — the radius a soldier is heard at, verified, with BF1's twin; where the bots die and at what range → `analysis/re-soldier-ai.md` §15.1, §15.4, §15.5 [O]
- **[O] BF2** (`FUN_002de708`; one Opus verifier, 23 claims: 19 supported, 3 partly, 1 not - folded in): heard within
  min(max(half the weapon's term, 5 x speed up to 50), 30) m, 30 m when the trigger was down since the last pass,
  never under 8 m. Corrections: the "object" term is the weapon in the hands; the trigger byte ai+0x80 is also set by
  the PlayerController's update `FUN_002b0548` (a human holding fire is heard at 30 m) and means "button down".
- **[O] BF1** (`FUN_002375e8` -> the soldier's vtable 0x532be0 + 0x2c -> `FUN_0020c6d8`): the same pass with a
  factor of 3.0 (`FUN_0020d940`, B1:122122), no trigger byte and no 8 m floor.
- **[L] Card FGHT2's measure** (15.5): the port's attackers get as near their target post as the PS2's; what differs
  is range and awareness - a PS2 BF2 bot dies with an enemy inside 10 m in 56-77 % of deaths (the port 18-51 %),
  45-70 % of the damage lands on a bot not in a fight order (the port 2-7 %), and BF2's level-1 think knocked into
  the port takes Kamino's close contact from 3.5 to 10.3 %. BF1 Kashyyyk Docks matches the port already.
- Tools: `engine/artifacts/lead_tmp/fght2/` — `tools/{lives,rng,aware}.py`, `tools/{observer,knob}_patch.py`,
  `tools/mini.sh`, `verify/{CLAIMS,VERDICT}.md`, `out/`, `sw/` (the sweeps).

## 2026-10-09 (audit FGHTA: PARTLY) — who the PS2's scout rule picks, BF1's twin, the think a circler's stop is drawn at → `analysis/re-soldier-ai.md` §16 [O]
- **[O] The key** (two verifiers, 14 claims: 9 supported, 5 partly - folded in -, 0 not): "stand and shoot before the
  draw" goes to a soldier whose CLASS is `UnitType scout` (BF2 class +0x8d4 = 0, BF1 +0x1914) when the chosen weapon
  is of the rifle animation type (`AnimationBank`, table 0x5fd848; not an `OffhandWeapon`) and in range - BF2
  `FUN_002abc38`, BF1 `FUN_00238d10`. BF1's draw has one range flag (a drawn stand stands anywhere in range), BF2's
  two (the middle band).
- **[V][L] Who is a scout:** 7 (class, weapon) pairs on the BF1 disc, 28 on BF2's, by two independent readers; three
  classes carry a scoped rifle with `UnitType trooper` - BF1 `cis_inf_assassindroid` (the CIS sniper of 19 of 44
  mission scripts; class +0x1914 = 5 in two live Clone Wars states) and `cis_inf_battledroid_sniper`, BF2
  `rep_inf_ep2_jettrooper_sniper`. The port's weapon key (`ai_distance_table()`) stands all three (card FGHTB).
- **[L] The stop chance's think:** 1 in 12 per think stands (`DAT_006980a0` = 2 in twelve states); the PS2's circle
  rows are at level 1-2 (a 1.2 / 0.8 s think) for 81 % on Kamino and 100 % on Polis Massa, so 0.07-0.10 stops a
  second; card FGHTA's 0.208 is level 3's.
- Tools: `engine/artifacts/lead_tmp/audit-fghta/` — `README.md`, `re/`, `verify/re_checks.txt`, `wf/RESULTS.md`.

## 2026-10-09 (loop lane 2, card CLSE) — a close fight's damage: the aim error vector the PS2 applies, and whose radius it is → `analysis/re-soldier-ai.md` §17 [O]
- **[L] The split** (two new BF2 captures with the weapon in hand every sample; the port with a read-only
  observer): inside 10 m firing time and rate are equal; a PS2 rifle shot lands 0.64 falls of health against the
  port's 0.20, and a PS2 aimer holds a rifle 52-55 % of the time against 30-31 %.
- **[O] The vector** (`FUN_002a50c0`, `FUN_002a4cc8`; one Opus verifier, 11 claims: 9 supported, 2 partly - folded
  in): aim = P + off x (d / 20 + 0.5, at most 10); off = the shooter's unit vector x the TARGET's radius x class
  +0x1e8 (shotgun 1.25, sniper 0.6 in RAM), y never negative, zero when the shooter rides with a human, **x 0.3
  when the target carries no human**. The port applies the radius whole at every range.
- **[L] Whose radius:** every controller, the player's too, carries its OWN error block - grown by its own change
  of velocity, widened by its own speed - corrected in `re-ai-difficulty.md` 4.3.
- **[L] Knock-in** (x 0.3 and k, 8 seeds x 32 maps, null = the gate baseline): Kamino inside 10 m 33 -> 52 hp per
  aimer-second (the PS2 71), Polis Massa 21 -> 31 (81); kills a minute 30.6 -> 32.5, team kills 1.9 -> 1.3 %, 256
  of 256 rounds. BF1's twin not read [H].
- Tools: `engine/artifacts/lead_tmp/clse/` — `README.md`, `tools/`, `out/`, `ps2/`, `sw/`, `re_section17.md`.

## 2026-10-09 (loop lane 1, card OBST2) — The corner search, read again: the recursion's depth, the radius a leg is tested with, where a leg starts [O] UNAUDITED
BF1 only (`SWBFRONT.ELF`); adds to "Barriers are the PS2's Obstacles" above, which has the search's shape. One Opus
verifier, 5 claims, 5 supported (it decoded the ELF words itself; `re_check.py fn` agreed on all 14 cited lines). BF2's
twin (`FUN_002f3528` / `FUN_002f3910`) was NOT opened for these three points: [H] there.
- **[O] The expansion goes one level deeper per crossed barrier.** `FUN_002691a0(parent, point, depth)`: `depth < 0x20`
  (`slti v0,s5,0x20` at 0x2691c4, s5 = a2). When `FUN_00268c98` reports a crossed barrier it calls `FUN_00268060`
  (jal at 0x2692fc; outputs at sp+16 and sp+32) and then itself twice (jal at 0x269310 and 0x269320), each with
  a0 = s3 (the SAME parent), a1 = one of the two corners and a2 = s1 = s5 + 1 (`addiu s1,s5,1` at 0x269304) - the
  decompile drops the arguments of all three calls (B1:188599-188601), the words are the evidence. `FUN_00269ac8`
  passes the popped node's chain length, itself included, as the depth (B1:188858-188861; the start node passes 1).
- **[O] A leg is tested with the unit radius alone; only the corners are 0.5 m further out.** `FUN_0026c0d0` hands its
  r unchanged to `FUN_00268f68` (B1:190572; `mov.s f12,f20` at 0x26c128), whose callback `FUN_00268ef0` hands the
  stored r (query + 0x50) to `FUN_00267d18` (B1:188228), which builds the rectangle with `FUN_002679f0(r, 0, ..)`
  (B1:187341) = centre +- axis x (SIZE + r + 0). The silhouette `FUN_00268060` takes its corners from `FUN_00267ab0`
  (B1:187498) = `FUN_002679f0(r, 0.5, ..)` (0x3f000000, B1:187134). So the PLAN Log's "tests the straight segment
  (radius r + 0.5)" is wrong by the 0.5: the port's `ai::DETOUR_RADIUS` (capsule radius + 0.5) is 0.5 m wider than
  the PS2's test (kept: card OBST2 measured the narrower test, its doc comment has the numbers).
- **[O] A follower's leg runs from the waypoint it had to the next one - not from where the unit stands.**
  `FUN_002509b0`: the current waypoint is copied (`FUN_00267538`, B1:169770-169774) BEFORE the follower is advanced
  (`FUN_00267540`, B1:169775), the next waypoint is read after it (re-rolled by `FUN_00267648` when `FUN_002686c0` says
  it is in a barrier), and `FUN_0026c0d0(r, unit, from, to, mask, 1, ..)` gets those two (B1:169809): both points
  are the follower's own (its +0x40 field before and after the advance). The first leg (`FUN_0024f3f0`, B1:168810)
  starts at the position its caller passes.
  Not checked: that the unit stands at the old waypoint when that branch runs. The port plans its corner walk from
  where the unit stands (`route::fill_detour`) and its leg line from the waypoint reached (card HUBP).
- Port findings (card OBST2, `nav::detour`): the port's search took the first 24 barriers of a leg's band in index
  order; the discs hold 30-60 a band at the median (up to 421), and 22,268 of 68,186 crossing legs in 64 rounds came
  back "clear". It is now the PS2's lazy search. Tools and tables: `engine/artifacts/lead_tmp/obst2/` (README.md).

## 2026-10-09 (loop lane 2, card CLSE2) — the aim error's block end to end, the `AIBubble*` keys, BF1's aim point without an error → `analysis/re-soldier-ai.md` §18 [O]
Both games. A workflow of five verifiers (claims and citations only): 33 claims, 32 supported, 1 partly (folded
in), 0 not supported; a second, separate verifier on the six absence / never / twin claims supported all six.
- **[O] The block is the unit's own, as a target** (`FUN_002ae010` / `FUN_002ae7b8`, B2:232993-233322): it decays at
  blend(0.1, 1.1) a second by the skill of who shoots at the unit; it GROWS by 0.455 x the change of the unit's own
  velocity (the slot +0x4c vector = E+0x550) / MaxSpeed between two thinks, and that growth is paid from a budget
  (+0x24) that fills at half the decay rate, at most 5. A bot's block thinks every 5th update at LOD 0 / 1
  (`FUN_002b7790`), the human's every frame (`FUN_002b0548`, the PlayerController's slot +0x1c).
- **[O] The radius** (`FUN_002ae4d0`) = 2 x size x (3 - 2 vis) x min(err x stance x (1 + 0.6 v / top), cap). New: the
  STANCE factor inside the cap - crouch 1.5, sprint / jump 2, roll 3 (`FUN_0024ea00` through the controllable's slot
  +0x34; RAM: crouched rows 1.000 with it, 1.500 without, 4,819 rows) - and the "state 7 / 2" factors never apply
  to a soldier (its slot +0xe0 returns 1).
- **[O] The size** (`FUN_002a76b8`) = a quarter of the HORIZONTAL diagonal of the model's bind-pose box, cached in
  class +0x370: 0.24 (super battle droid) to 0.51 (wookiee); it equals the baked GLB's POSITION bounds (RAM = GLB
  on every class read).
- **[O] The weapon class's keys** (`FUN_00328fb0`): `AIBubbleSizeMultiplier` -> +0x1e8, `AIBubbleScaleDistDivider`
  -> +0x1ec, `AIBubbleScaleClamp` -> +0x1f0; on the BF2 disc 10 classes author the first (1.25 shotgun, 0.6 sniper
  rifles and three building turrets, 0.85 bulldog, 0.4 / 0.1 auto and defense turrets), four the other two
  (100 or 50; 3.0). BF1 reads and authors none [receipt:257b62c4].
- **[O] The direction** is the shooter's: a quaternion turned 2 x dt a think about a randomly walking axis, applied
  to (0, 1, 0) - a tumble at up to 2 rad/s, not a draw per think.
- **[O] BF1's aim point has NO error vector**: `FUN_0020aad0` -> `FUN_00233500` -> `FUN_00233200` (the twin of
  `FUN_002a50c0`) -> `FUN_00232a70` = the target's posture point (feet + 1.4 / 0.6 / 0.2 m) + a frame of relative
  velocity + the weapon's lead `FUN_0026d3f0`; no draw, no radius, no factor by range; no 0.455, 2.2 or bubble key
  in the ELF [receipt:257b62c4]. RAM: a BF1 bot's view ray passes a standing target's axis at 0.06 m at every
  range (BF2: 0.20-0.32 m, growing).
- **[L] Knock-ins on the running PS2** (class +0x370 of the player's team written; Kamino, 240 s each): with the
  targets' radius at 0 their shooters' miss falls to a flat 0.060 m (411 rows) while the shooters whose OWN radius
  is 0 keep 0.29-0.46 m - the vector takes the TARGET's radius, and nothing else offsets a BF2 bot's aim. At size
  1.0 the miss over (radius x k) is 0.533 at the human and 0.174 at bots: 0.33, the code's 0.3.
- The port (`ai/gunner_foot.rs`): one err per shooter, grown by the target's displacement, no stance or size or
  class factor, a new direction every think, and BF2's whole radius on BF1 maps. Card CLSE2 builds the difference.
- Tools: `engine/artifacts/lead_tmp/clse2/` — `tools/` (`bf2_radius_replay.py`, `size_box.py`, `glb_box.py`,
  `bubble_scan.py`, `bf1_aim_off.py`, `bf1_fire_target.py`, `sair_bf2_poke.py` + `run_poke.sh`), `out/`, `ps2/`,
  `verify/claims.md`, `verify/verdicts.json`, `re_section18.md`.

## 2026-10-09 (loop lane 1, card GRAIA) — the verifier pass over the weapon choice's corrections → `analysis/re-grenades.md` §7.5
- One Opus verifier, claims and citations only: 8 claims, 7 supported, 1 partly. "Plenty" asks the load ISSUED in
  both games, BF1 scores one shot and BF2 a rate, BF2 passes over a reloading weapon: all hold.
- **[L] Partly:** BF2's ammo[2] has a second writer - `FUN_00325840` (weapon vtable +0x28) doubles it under Galactic
  Conquest's "supplies" bonus (B2:317676). Not in the port.
- New and unread [H]: BF2's `FUN_002ab430` skip when the choice is applied; three more outcomes of BF2's score
  (`FUN_002aa728`); BF1's fixed 1000 for some class pairs and its explosion falloff at 2 m.
- Built by the card: the three rules, and OURS - no grenade where a friend stands or walks to (12 m).
- Tools: `engine/artifacts/lead_tmp/graia/` — `tools/{sw.sh,sum.py,mut_all.sh,port_slots.sh}`, `sw/`, `v/`, `all/`.

## 2026-10-12 (loop lane 2, AUDIT of card CLSE) — BF2's aim-error scaling re-derived and seen on the running PS2; the landed rule beyond 30 m → `analysis/re-soldier-ai.md` §19 [O]
- **PARTLY.** 17.2 (x 0.3 against a target that carries no human, whole against one, x min(d / 20 + 0.5, 10), y never
  below the aim point, zero for a human's passenger) re-read from the export and the ELF words before the doc; two
  verifiers, claims and citations only: 13 supported, 6 partly (wording), 0 not. Every number of the landing reproduces.
- **[L] The rule on the running PS2** (`audit-clse/ps2/live_rule.py`, CLSE's two captures): the sideways aim offset of
  still pairs is 0.33 / 0.31 / 0.30 x k x (V . p) x R_T x F at 0-10 / 10-20 / 20-30 m (n 24 / 64 / 79); against the
  human 0.85 (n 8). C+0xc0 is 0 for the human and -1 for every bot - what `FUN_002a7fd8` counts.
- **[V] A third writer** of the weapon class's three `AIBubble*` fields: the copy constructor `FUN_003283d8`
  (B2:319739-319741) - the inheritance by BASE. 18.4's disc census re-read three times: the same 10 and 4 classes.
- **[L] Not measured by the card:** beyond 30 m the port went from 1.74 / 1.79 to 2.15 / 2.32 times the PS2's damage
  per aimer-second (Kamino / Polis Massa); deaths with an enemy inside 10 m 30.3 -> 25.0 % on the 17 BF2 maps (Kamino
  27 -> 18 against the PS2's 42-70), contact20 12.6 -> 10.3 %. All of it is the 0.3; the cause of the far fire is
  card CLSE4's (the bots' tight burst of card SPRD is a tenth to a fifth of it).
- Tools: `engine/artifacts/lead_tmp/audit-clse/` — `README.md`, `tools/{sw.sh,knob_patch.py,hold_patch.py,mk_mut.py,
  mut_run.sh,contact_spells.py,contact.py,census_tot.py,ps2_deaths.py}`, `ps2/live_rule.py`, `wf/RESULTS.md`.

## 2026-10-12 (loop lane 1, card MOUTH) — a soldier's leg line is the follower's segment, node reached -> node walked at; the follower in RAM → `analysis/re-soldier-ai.md` §2 [O] UNAUDITED
BF2 read, BF1 by RAM layout only. One Opus verifier (claims and citations only): 6 code claims supported; the live
check's numbers reproduced and its logic marked PARTLY (folded into the text).
- **[O] The projection's line** is follower + 0x10 -> + 0x20 at all three calls of `FUN_002a5fa8` (B2:281242, 281249,
  281386, `FUN_002f1d80`); the advance `FUN_002f1470` -> `FUN_002f0e78(follower, follower + 0x20, ..)` moves the node
  walked at into + 0x10 (B2:280791-280792, 280429-280440). After a detour corner the line starts at the corner.
- **[L] The follower in RAM**: one per bot in a fixed array (BF2 stride 0xb0, BF1 0x90): [0] node list, [1] the AI
  controller, + 0x10 / + 0x20 / + 0x30 / + 0x40 the node come from / walked at / after / last, + 0x60 mode, + 0x64
  side. A node on a hub's sphere is that hub's point; two of three BF2 nodes reached were not (corners, goals). The
  path OBJECT of `audit-hubm` is the planner's, not the walk's: its current hub sat a median 114 m from the bot.
- **[L] The PS2's own stall rate** (1.5 s of forward stick on route with under 1.5 m gained toward the bot's point
  of the next hub; the same rule on port snapshots): BF2 Kamino 67 in 153 route bot-minutes (8.7 per 20), BF1 Bespin
  Platforms 33 in 186 (3.6); the port (main 19708fa) 22.5 and 32.2. At Kamino's four walkway mouths 1 PS2 stall in
  121 passes; the port 50 in 631.
- The port (`ai/route.rs` `leg_point`) projects on `Route::from` -> waypoint with the corners behind: card MOUTH
  builds the PS2's segment. Tools and captures: `engine/artifacts/lead_tmp/mouth/` (README.md).

## 2026-10-12 (loop lane 2, AUDIT of card BSPN) — the AI's spawn-post draw read again by two verifiers; PS2 captures that tell post-first from the flat node list → `analysis/re-command-walker-post.md` §6.1 [O] UNAUDITED

- **Stands**: BF2 `FUN_002e7240` (weights summed, a float r in [0, S - 1), the first post at <= 0), BF1
  `FUN_00261e80` (uniform), the 4.0 s timer and the death's clearing of an AI's post, one ask at the timer's end
  (byte member+0x164), the node as an unsigned `random % count` (`divu` in both ELFs). Two verifiers, 17 claims: 12 /
  4 / 1 and 11 / 6 / 0 (supported / partly / not); both mark the same points partly.
- **Corrected**: BF2 does not re-test a held post while the member waits (the 18 call sites of `FUN_00356880` are
  outside the member update and the spawn function, [receipt:d719752b]); its one late test is the friend byte at the
  top of the spawn `FUN_0034e1f0` (B2:344888), which abandons the spawn and draws class and post again. BF1 re-tests
  every update (B1:221035). B2:345009 is that abandon branch: a successful spawn leaves the post in member+0x144.
  BF1 `FUN_00261f88` re-chooses the GOAL post (member+0x18), not the spawn post.
- **[L]** Six independent PS2 captures (737 AI life starts, BF2 Dagobah / Kamino, BF1 Bespin): chi2 54.5 against the
  flat node list (8 dof), 9.8 against an even draw over posts. Hoth's cited counts could not tell the two apart
  (p 0.10, 0.72). On BF2 Kamino cp2's bots start at `cp2_spawn`'s nodes, which the conquest script sets at run time
  (card SPTH). Tools: `engine/artifacts/lead_tmp/audit-bspn/` (README.md).

## 2026-10-12 (loop lane 2, card CLSE4 measure) — the far fire split by weapon; the trigger's range test and the Barrage keys, both games → `analysis/re-soldier-ai.md` §20 [O]
- **[O] The trigger's range test**: a tracked target is fired at only when the 3-D distance from the eye to the
  target's aim point is under the weapon in hand's `MaxRange` (BF2 `FUN_002bb1b0` -> `FUN_002ad228` -> `FUN_002ad1a0`
  -> `FUN_002a9c30`, class `+0x118`; BF1 `FUN_0023cd40` -> `FUN_0023a0d8` -> `FUN_0023a050` -> `FUN_00237de0`, class
  `+0xec`); x 2 at a snipe stand, capped. The port gated by the bolt's reach.
- **[O] The burst keys**: `BarrageMin` / `BarrageMax` / `BarrageDelay` (hashes 0x1aa5ec35 / 0x04bae44b / 0xffcc84b2),
  constructor defaults 3 / 6 / 2.0 s in both games; BF2 n = trunc(3 + U[0, 3) x s) with a pause of (1.5 - 0.8 s) x
  2.0 s from the burst's end; BF1 n = 3-6 uniform, pause 2.0 s flat. Only flyer weapons and the defence turret
  write them [receipt:c35231ee]. The port's 2 / 6 / 0.5 were placeholders (card AIBST, closed by this).
- **[L]** Three PS2 captures a map against 8 port seeds: Kamino beyond 30 m 14.4 against 24.5 hp per aimer-second
  (x 1.70); the excess by weapon is snipers 34 %, rifles 20 %, pistols and incinerators (fired beyond `MaxRange`)
  22 %, shotguns 19 %. Both rules in the port: x 1.24, deaths with an enemy inside 10 m 26.3 -> 28.5 % on the 17
  BF2 maps. Verifier: 21 claims, 16 SUPPORTED, 5 PARTLY, 0 NOT. Tools: `engine/artifacts/lead_tmp/clse4/` (README.md).

### 2026-10-12 - card MOUTH2: the PS2's bots at an opening door; the pod's skirt at Kamino's hub 46
- **[L]** `re-bf2-doors.md` 11: 44 PS2 passes of BF2 Kamino's pod room door at 10 Hz. Lanes up to 3.2 m off the
  centre; a bot on an outer lane walks up to the leaf (feet 1.32 m short of the plane), stands about 0.6 s (two
  read sample by sample) with the door's normal as its contact push, and goes through; 2 stalls in 51 passes against the port's 121 in 270. Built
  (main ac13ca6): `ai::steer` `StopKind::Door` (a leaf of a door that is opening is walked at, not gone round):
  0 in 157, stalls within 6 m of a door on the five door maps 613 -> 401.
- **[L] / [H]** `re-soldier-ground.md` 6: the PS2's bots walk up a 1.2 m band the port's mesh reads as n.y 0.667
  at a steady 3 m/s (BF2's full-hold uphill target), which x 0.1 under 0.7 cannot give; the port's soldier rises
  0.94 m and slides back, and its ledge rule takes 6 of his 7 m/s at a riser under the face he climbs. The PS2's N
  on that band is unread: card SKIRT. No verifier: both sections are measurements, the two decomp lines cited
  (B2:163053, 163075) checked with `re_check.py fn` / `word`. Tools: `engine/artifacts/lead_tmp/mouth2/` (README.md).

### 2026-10-12 - card SKIRT: a floor under the control limit (the pod's skirt at BF2 Kamino's hub 46)
- **[L]** `re-soldier-ground.md` 7.1: the player walked up the band 24 times in PCSX2, one row per update. N on it IS
  the face's (N.y 0.6665 / 0.6616) and the x 0.1 binds (0.69 m/s lost per update above the uphill target 3.18,
  0.21 below; no stick: +0.21 down). He is over a band 1.03-1.50 m high in 0.97-2.10 s from a 2 m run-up (13 of 15
  head-on walks) and not from a stand at its foot. The port's body (main cae9777) makes 6 of the PS2's 14 climbs:
  it stops at a band about 1.2 m high.
- **[V]** 7.2, both games (BF2 `FUN_0023cfd0` / `FUN_002480a0` / `FUN_00240bf8`, BF1 `FUN_00202f38` / `FUN_002098b8`
  / `FUN_00205cc8`): the ground normal is the FLATTEST floor contact of a pass (B2:156159-156166); only velocity
  into a face is removed (B2:156327-156341); |v.x|, |v.z| under 0.1 are zeroed after every contact
  (B2:156366-156373); the push keeps 2 cm of overlap vertically only (B2:156394-156397). **[D] + [L]:** so the
  update the foot sphere meets a steeper face runs at full hold (8 of 18 run-ups, 6.29 m/s onto the face against
  4.78), and a soldier whose speed runs out on a slope lifts off for one airborne, full-acceleration update (47 of
  56 such turns).
- Workflow `skirt-re-verify` (4 agents): 9 BF2 claims 7 / 2 / 0, 7 twin + absence claims 6 / 1 / 0 from each of two
  verifiers working apart (none overturned), 8 measured claims 4 reproduced / 4 partly (ranges, folded in).
- Prototype (scratch, 7.4-7.5): the three rules behind switches, null = main byte for byte; 14 of 14 climbs; hub 46
  52 -> 20 stalls per 100 passes with the ledge rule's height and no steep probe (the PS2 5; no probe at all costs
  BF1 Geonosis 10 recycles in 8 seeds), the maintainer's scene 3 -> 0. Left: bots on the dais (card DAIS).
  Tools: `engine/artifacts/lead_tmp/skirt/` (README.md).

## 2026-10-12 (loop lane 1, card CLSE4 build) — the trigger's range test and the Barrage keys are in the port → `analysis/re-soldier-ai.md` 20.5
- No new RE: 20.2 / 20.3 built as read (main a490579), with the named choices listed there (no x 2 at a snipe stand,
  no meter gate, the pause kept when the want drops, a seat's gun by the bolt's reach).
- Measured against main 2e12961, 8 seeds: the 17 BF2-rule maps' deaths with an enemy inside 10 m 25.8 -> 29.1 %
  (t +5.2), contact20 11.14 -> 12.24 %, Kamino beyond 30 m x 1.67 -> x 1.23 of the PS2's damage per aimer-second,
  its rifle bursts mean 2.93 shots (the PS2 2.99). Costs: team kills +0.2 points, Polis Massa's far bin unmoved
  (card CLSE5), BF1 Tatooine's shut footpath takes more lives (card TGAP).

## 2026-10-12 (loop lane 2, card DOORS, measure + build) — every door of the five door maps, censused → `analysis/re-bf2-doors.md` 12
- **[L]** 12.1: the PS2's own doors on BF2 Kamino (three captures, 153 route bot-minutes): 133 near visits, 125
  through, 1 turned back, 5 stalls (4 per 100) - not flawless, never turned away.
- **[L]** 12.2: the port after MOUTH2 (main 4dbb460, 8 seeds x 3 min a map): Kamino 1 stall in 579 near visits,
  Tantive IV 192 in 1,713. Of 358 stalls near a door 6 are at a leaf, 172 are bots standing in a firefight, 107 go
  round another body; of 784 meetings with a leaf 773 pass, 0 stuck events in them, the longest wait 2.0 s.
- **[L]** 12.3: BF2 Death Star's blast door parts on a diagonal (the bake's leaf boxes are rolled 43.9 degrees): a
  face at n.y 0.69 under the steep-floor probe, whose answer outranked the door's - one unit turned away at the
  Empire's spawn in 31 of 32 rounds. Built (main 4aaa2a2): 18 of 364 meetings ending badly -> 5 of 374, 0 of 32.
- **[L]** 12.4: Tantive IV's worst door is a path corner inside a barrier the leg ignores (card CORN; a first rule
  is no all-maps gain). Tools: `engine/artifacts/lead_tmp/doors/` (README.md).

## 2026-10-12 (loop lane 1, AUDIT of card CLSE2: PARTLY) — the aim-error block replayed on the running PS2; stance events add error; a mounted unit's block is its vehicle's → `analysis/re-soldier-ai.md` 21
- **[L]** 21.1: the port's `AimBlock::think` gives the PS2's err and budget on 20,979 of 21,272 live thinks (98.6 %)
  and 22,644 of 22,773, the human's 2,089 of 2,090; the radius with its speed term on 11,585 of 11,709 moving
  thinks; the quaternion's product order seen. The skill of a block is one level a team: 14 and 11 in a Normal
  battle, read off the blocks (the port's one level is 17; put to 14 / 11 it takes 6 % off the far fire).
- **[O]** 21.2: `FUN_002ae728` (a jump table the export did not recover; ELF words read) adds to a unit's error at
  a stance event through 18.1's budget: roll 2.0, sprint start 2.0, jump 1.0, jet 5.0. Both verifiers found it
  against the auditor's own claim. **[L]** the human's 16 jumps: +1.0000 from the budget on 10, the budget's
  remainder on 4, the cap on 1, an empty budget on 1. Not in the port (card JINK).
- **[L] [O]** 21.3: a mounted unit's controller hangs off the vehicle (267 of 267 mounted bots in 49 states, 1,813
  of 1,851 live turret thinks): the size factor is the vehicle's (turrets 2.0-4.2, walkers 3.9-12.5, flyers 7.6
  and more) with its velocity and no stance factor. The port takes the seated soldier's (card VAIM).
- Workflow `audit-clse2`: 22 agents; two verifiers 17 SUPPORTED / 7 PARTLY / 0 NOT each. Tools and captures:
  `engine/artifacts/lead_tmp/audit-clse2/` (README.md; `ps2/block_live.py`, `block_replay.py`, `bump_human.py`).

## 2026-10-12 (loop lane 1, card HVYW build, WIP) — the weapon a foot soldier may use on a vehicle: the suitability test of both games → `analysis/re-soldier-ai.md` 22
- B2 `FUN_002aaba0` / B1 `FUN_002380a8` read whole: the mask (BF2 the AITarget word, a refusing mask always
  rejects a vehicle; BF1 the Target word, soldier against soldier only), "too weak" (BF2 rate x 10 s, BF1 one
  shot x min(rounds, 50), against half the MAX health), and for a foot soldier's weak weapon the target classes
  (hover, walker, a flagged building; BF2 also a slow or landed flyer) and the ordnance classes (missile, shell,
  beam outright; grenade and sticky - haywire and beacon with it, by the class hierarchy - under 12 m/s).
- One verifier: 5 SUPPORTED, 4 PARTLY, 0 NOT; corrected 4.3 (haywire is speed-gated) and added the melee / remote
  / BF1 cannon paths ahead of the mask. Census of main and the first build's numbers in 22.3; tools
  `engine/artifacts/lead_tmp/hvyw/tools` (hvcen.py, wdump.py, sw.sh), the observer `HEAVY_CENSUS` on branch `hvyw`.

## 2026-10-12 (loop lane 2, card ROLLS, RE) — a roll is ONE clip with the body's root turned into it → `analysis/re-roll-picture.md`

- The maintainer's ask: the port's sideways roll is the forward roll played while the soldier slides sideways. The PS2, both
  games: every roll plays `diveforward` (BF1 states 10 / 0xb -> special id 0x14, `FUN_0022a0a8`; BF2 state 5 -> mode
  0x18, `FUN_0027bae0`), and the animator turns `DummyRoot` (hash 0x4446e9b8) about the up axis by an eased angle:
  BF1 toward +-pi/2 by the roll's side, 0.25 of the rest a call, multiplied onto the clip's rotation under the legs'
  blend weight (`FUN_0022aa10` case 0x14 -> `FUN_0022c7f0` -> `FUN_0022bcc0`); BF2 toward atan2f(move . left, move .
  forward) taken every update, min(1, 7.5 dt) of the rest, written outright (`FUN_0027caf0` case 0x18 ->
  `FUN_0027d9a8` -> `FUN_00280048`). The field is the walking soldier's leg twist.
- Live (BF1 Theed, BF2 Coruscant; one row per update of the soldier and his animator, video beside): BF2 left roll
  travel +90.0, twist 76.0 -> 89.9 over 26 updates; forward 0.0; forward-left +37.9 against 36.5 -> 38.1; BF1 left
  roll twist -31.2 / -0.9 / +21.8 ... +89.9 over 27 updates, each step 0.250 of the rest; an AI soldier's roll (its
  request word written) the same. Strips: `engine/artifacts/lead_tmp/rolls/sheet_ps2_bf2.png`, `sheet_ps2_bf1.png`,
  `sheet_ps2_bf1_bot.png`.
- Also: BF2's roll predicate refuses the 60 degrees either side of straight back (`re-stance-roll.md` 4.2 said 30;
  corrected); neither game has a backward roll, BF1 no forward one. The discs hold forward dives only among 20 and 26
  million names tried (`human_bazooka_stand_diveroll_forward` and `human_rifle_dive2prone` are in no table).
- Workflow `rolls-re-verify`: 7 agents, 39 claims, 30 SUPPORTED / 9 PARTLY / 0 NOT on the first reading; the six
  absence and twin claims read twice (the second reading: 2 / 4 / 0, one SUPPORTED overturned to [H]). Tools:
  `engine/artifacts/lead_tmp/rolls/ps2` (roll_strip.py, analyse.py, latency.py, sheet.py), `checks/disc_dive.py`.

## 2026-10-13 (loop lane 2, card VTURD, RE) — a building turret's posed nodes, its gunner, its views, its wreck → `analysis/re-turret-draw.md`

- [O] **Nodes** (B1 tick `FUN_00223850`, aimer `FUN_0026d748` / `FUN_0026ce10`; B2 `FUN_0026cea0`, 0x3021f8 /
  `FUN_003018d8`): the `TurretNodeName` node = a look-along matrix of (yaw x YawTurnFactor, pitch x PitchTurnFactor)
  at its bind position (every soldier-seat building turret but BF1's Bespin AA gun authors PitchTurnFactor 0.0: yaw
  only); the `AimerNodeName` node = a look-along matrix of the AIMER's own direction, up and position through its
  parent's entry; a `BarrelNodeName` node = its parent's matrix slid back by state x BarrelRecoil (state 1 at a
  shot, -4 a second). A table entry is one node's whole model-space matrix; nothing composes parents.
- [O] **Gunner**: drawn when the seat has `PilotAnimation` / `Pilot9Pose` (BF1 `FUN_0020c880`; BF2 `FUN_0024de60`
  also hides on `IsPilotExposed = 0`, authored on flyers only); at `L x E x O` (hardpoint's bind matrix relative to
  its parent, that parent node's current matrix, the building's world matrix - `FUN_0023e028` / `FUN_002f74c0`);
  posed from one clip `human_<value>`: nine-pose = frame `yf + 4` blended toward frame `yf + 7 / + 1` by `|pf|`;
  no hand weapon; drawn in every view.
- [O] **Views**: always third person on the aim frame (the MountedTurret's preference virtual returns 0, no
  building authors `ForceMode`); zoomed, the camera sits at the eye point and the whole building is left out of
  that viewport (`FUN_001c4878` / `FUN_001e81e0` early exit on `FUN_00225260` / `FUN_0026f0e0`).
- [O] **Dead**: tasks removed, angles frozen, the destroyed model drawn (state machine: `re-build-zones.md` 1); 0 of
  49 destroyed models hold a posed node.
- Census (script, raw LVL chunks): armed-building classes with a TURRET section BF1 9, BF2 69; gunner drawn /
  hidden / no soldier 6 / 1 / 2 and 15 / 7 / 47; every turret model but the fambaa and one space turret splits
  exactly into rigid per-bone parts (0 triangles across bones); the seat clips are in `human_animation_1` (BF1) and
  `human_4` (BF2), 9 / 9 / 1 frames.
- Live (PCSX2, one row per game update, a human in four turrets): the node matrix fits 758 of 758 BF1 and 1,077 of
  1,080 BF2 rows, the aim frame 758 and 1,073, the aimer node 351 of 351 and 342 of 342, the barrel slide 341 and
  342 of 342, recoil decay 4.000 and 3.996 a second; the gunner sits at his hardpoint in the turret node's frame to
  4 decimals (aimer-node seat: the node of the update before, 334 of 341); scene byte 1 / state 0x11 in the dish
  turret, 0 / no body in the laser tower.
- Workflow `vturd-re-verify`: 17 agents, 101 claims; first reading 84 SUPPORTED / 17 PARTLY / 0 NOT, the 45 twin
  and absence claims read twice (39 / 6 / 0; 3 split, none downgraded, 19 claims re-stated with their scope put
  right). Tools: `engine/artifacts/lead_tmp/vturd` (turcensus2.py, softcensus.py, clipcensus.py, replay_nodes.py,
  replay_aimer.py, ps2/turret_b1x.py, ps2/turret_b2x.py).

## 2026-10-13 (loop lane 2, card CPBAR, RE) — the PS2's capture icon: both games draw one in the 1-player HUD; BF2's layout ring is dead data → `analysis/re-capture-display.md`

- [O] **Both games draw a capture icon in every HUD** - `re-hud.md` 3 said BF2's 1-player HUD shows nothing; wrong,
  corrected. It is a code object, not layout data (BF1 `CaptureDisplay`: ctor `FUN_002bd8c8`, update
  `FUN_002bdc30`; BF2 ctor `FUN_0037e0a8`, update `FUN_0037e400`, texture by hash 0x621cb6df): the 64 x 64 symbol
  `hud_flag_timer` cut by a horizontal line into two colours, at the right edge at mid height (anchor (1.0, 0.5) in
  the scene's safe rectangle, offset (0, -30): frame x 420.99..486.40, y 194..258; the model fits the PS2 frames of
  both games to under a pixel with no fitted parameter).
- [O] **The rule** (one in both games): the first post whose capture mask holds the player. Neutral post: line at
  1 - capture / CaptureTime, below it the capture meter's team colour, above it grey; two sides inside = the top
  alternates every 0.5 s with the other side's colour. Enemy post: line at neutralize / NeutralizeTime, grey over
  the owner's colour, no blink with defenders inside. His own post: not drawn. Colours from the player's team table
  (BF2 own (1,86,213), BF1 own (100,200,100), enemy (200,100,100), neutral (192,192,192)). Off with the pause menu
  and after the battle; a network client is fed the same values by a message in 1/63 steps.
- [O] **BF2's `commandPost` events are registered and not sent**: six per player (`FUN_003b0aa8`, two players); of
  236 event-value constructions in `.text` none takes one of their handles; the 10-segment ring `playerNcpstatus`
  that listens to them is in the 3 / 4-viewport layout alone. BF2's third bitmap (`hud_notallowed`) is not drawn.
- [M] Live rows, RAM and frames (BF1 Yavin 4 Arena, BF2 Mygeeto; a player parked in a post, an enemy placed beside
  him): every state above seen in both games, with the post's two meters beside the object's fields - and BF2's
  leftover neutralize meter after a capture (`re-cp-capture.md` open question 1: closed), BF1's reset against BF2's
  hold on an empty post.
- Workflow `cpbar-re-verify`: 8 agents, 44 claims, 61 verdicts (the 17 twin and absence claims read twice):
  51 SUPPORTED / 10 PARTLY / 0 NOT; 2 split, none downgraded, 8 claims re-stated (the network driver and the gate's
  meaning, two more event send routes, the group draw's test of a child's bit). Tools:
  `engine/artifacts/lead_tmp/cpbar` (ps2/cp_probe.py, tools/rows.py, strip.py, iconbox.py, ramdisp.py, handles.py,
  constscan.py; sheets/*.png).

## 2026-10-13 (loop lane 1, AUDIT of card CLSE4: PARTLY) — the trigger's two rules read live from the PS2's burst state; a weapon with unlimited rounds is not held to n shots → `analysis/re-soldier-ai.md` 23

- [O] **Both rules of 20.2 / 20.3 hold in both games** (five verifiers, 25 claims) and every number of the landing
  reproduces from rebuilt binaries (the sample files byte for byte). New live evidence from captures already on
  disk: the burst state machine is in RAM (BF2 `ai + 0x430`: state `+0xec`, end time `+0xfc`, rounds at the start
  `+0x100`, n `+0x104`; BF1 the K block `+0x1d8` / `+0x1e0` / `+0x1e4` / `+0x1e8`). BF1: 837 draws 21.6 / 26.6 /
  25.2 / 26.5 % over 3..6, the least pause 2.00 s. BF2: the human's team draws at level 11 (64.1 / 35.9 / 0 %,
  2.17 s), the other at 14 (Kamino) and climbs to 17 inside four minutes on Polis Massa - the skill rubber band of
  re-ai-difficulty 6 seen live. The range test: 1 of 2,380 rifle shots beyond 64 m where 41 are expected with no
  test, 3 of 448 pistol shots beyond 24 m where 167.
- [O] **The n-shot test counts rounds** (B2:241999-242000 over `FUN_003037e8`, B1 `FUN_002840c0`; two verifiers):
  a weapon with unlimited rounds (the owner slot's `WeaponAmmo` under 1 or, in BF2, not written; or
  `RoundsPerClip` under 1) is not held to n - its burst runs until its heat is one salvo short of full or the want drops. Live:
  a mounted BF2 bot's rounds word is 0x7fffffff on 3,404 of 3,405 firing samples and 43 % of its firing runs last
  2.5 s or more (a rifle on foot 1-2 %); BF1's unlimited hand weapon the same. The port counts n for every seat
  and since CLSE4 pauses 1.65-2.0 s after it: mounted shots -13 % (t -4.7), mounted kills on the BF1-rule maps
  -18 %. Card BRSTU.
- [L] The forgiven pause (named not built by the landing): 42 % of the PS2's rifle bursts on foot start after a
  reset; the landing's rifle fires at 70-85 % of the PS2's rate in every range bin on Kamino (the parent 119-143
  %); knocked in, the rule closes the 0-10 m bin and not the middle ones (note on card CLSE5).
- Workflows `audit-clse4` (21 agents: 6 numeric, 8 mutations of which 6 survive, 4 controls, 3 verifiers) and
  `audit-clse4-verify2` (2 verifiers). Tools: `engine/artifacts/lead_tmp/audit-clse4/tools` (ps2_burst_bf1.py,
  ps2_burst_bf2.py, ps2_burst_mounted.py, ps2_burst_wclass.py, ps2_burst_bf1_unl.py, ps2_reset_share.py,
  wcensus.py, mounted.py, perseed.py, cmpseed.py, knob_patch.py).

## 2026-10-13 - the hologram's beam and icon, read for the build (card CPHOL) -> re-point-lights.md 15
- The "lightbeam" draw (B2 `FUN_00469628`, B1 `FUN_00386170`) is a flat 22-point fan in the plane through the beam's
  axis that faces the eye, not a cone; alphas by the view (1 - g^2, the eye inside the cone, the far fade). BF2 binds
  `white.tga` and modulates, BF1 binds sheet 0: BF1's column is half as bright per colour byte (seen in stored
  frames). BF1 draws the beam and the icon as BF2 does (`FUN_001e7d88`). The post switches its hologram off while its
  meter is within HoloTurnOnTime of its end. One verifier: 13 claims, 10 SUPPORTED / 3 PARTLY / 0 NOT; it recovered
  the cap's lost argument (g) from the ELF words.
- [H]: the beam is left out where the fan's box reaches the near plane (fitted to 12 stored PS2 poses; the VU1
  program is not read). Tools: `engine/artifacts/lead_tmp/cphol/tools` (boxpair.py, neardist.py, mut.py,
  cphol_seq.rs, sheet.py).

## 2026-10-13 - unit chatter, both games: every line a unit says and what asks for it (card USND) -> re-unit-chatter.md
- [O] **BF1**: a line is a soldier-class key played at the unit through one vtable slot each (`EntitySoldier`
  0x532d50 +0xac..+0xc4). Called: `AcquiredTargetSound` (the target tracker's setter `FUN_0024d6d0`), `FleeSound`
  (`FUN_00236be8`: the first AI soldier within 7.5 m of a live grenade-class ordnance, once a second),
  `HeardEnemySound` (the agent's fourth check `FUN_00252dc8`). **Never called** [receipt:69e3eb1c]: `HidingSound`,
  `ApproachingTargetSound`, `PreparingForDamageSound` (every virtual call through their slots is a weapon's; two
  verifiers' own ELF scans; live, their gates never armed). Called and authored by no soldier class: `ChargeSound`,
  `ShockSound`, key 0x5625fc0b.
- [O] **Hurt / death, both games**: in the damage function (B1 `FUN_001bc190`, B2 `FUN_001dfb38`) a health hit of
  more than 2 % of the health left that the unit survives plays `HurtSound`, the kill `DeathSound`, both on one
  per-entity handle (death cuts hurt); BF2 adds a damage-flag bit and `HurtFallSound` (authored by nobody). Sample
  one-shots outside the voice queue; chance 0.85 / 1.
- [O] **The gate** (B1 `FUN_0046bf10`, B2 `FUN_0055c9a0`): per PROPERTY {probability, interval, dev, next}; the
  interval is re-armed before the probability is rolled. BF1 chatter 20 +- 5 s (`all_` / `imp_` 1 to 10 s), BF2
  `VOSound` lines 40 +- 20 s (`all` 80 +- 40). BF1's take pick gives up on a locked take, BF2's steps on.
- [O] **BF2**: `VOSound = <sound> <event> [conditions]` entries on the class (a child shares its parent's chain,
  seen in RAM), event = index in the 40-hash table at 0x624ed8 (0 for a name not in it); `FUN_002e6180` picks the
  speaker (none named: the subject's nearest team-mate), requires it within `VORadius` (default 50 m, authored by no
  class) of the subject - the local player when the caller names none -, lets an enemy speak 31 times in 101, an AI
  only at detail level 3 or 4, and plays the entry with the most matching conditions. 36 call sites are the whole
  trigger list: AcquiredTarget, FriendlyFire, Grenade, Repair, GivePowerup, the slaughter counters, and for a
  HUMAN's kills HeadshotKill and the killing sprees. `MissileIncoming` (186 lines) and the eight `...VO` request
  names are never fired; KillingSpree12 / 16, Kill, NearbyTeamKillSlaughter fire with no line.
- [O] `LowHealthSound` with `LowHealthThreshold` 1.1 is the natives' idle voice (Gungans, Ewoks, Jawa; saber hums).
- [L] 12 captures of 4 minutes (`lead_tmp/usnd/ps2/usnd_live.py`: sound starts + every property's gate re-arm, the
  player held alive, optionally put beside a front-line team-mate every 20 s): BF1 32 voice lines a minute wherever
  the player stands; BF2 0.25 at the spawn post, 8 to 12 in the fight; hurt + death 30 to 175 a minute in range.
- Verifier workflow `usnd-re-verify`, 8 agents: 40 claims, 29 SUPPORTED / 11 PARTLY / 0 NOT; 13 second readings of
  the absence and twin claims, 8 / 5 / 0; 16 claims re-stated (BF2's interval was claimed 80 +- 40 for all armies;
  the human-only kill events; the shared chains; death from the kill routine). Names found by hash: `ShockSound`,
  `HurtFallSound`, `DamageRegionSound` (0x7241c3bb), `VORadius`, `VOUnitType`, KillingSpree12 / 16,
  NearbyTeamKillSlaughter, MissileLocked, CollisionDamage, Kill. Build cards UCALL, UCRY, UTALK, UVO2.
- [H]: the headshot test of event 9; `DamageRegionSound`'s reader; eight unnamed events. Tools:
  `engine/artifacts/lead_tmp/usnd/tools` (vcall.py, vtof.py, classcensus.py, vosound.py, sndcensus.py, crack.py,
  ram_vo.py, live_events.py, live_tab.py).

## 2026-10-15 (loop lane 1, card ANIMM build) — BF2's leg twist, forward bit and turn on the spot → `analysis/re-soldier-anim-layers.md` 7.1 [O]

- BF2 keeps the strafe twist and the forward bit in the soldier (`FUN_0024f350`: BF1's windows and limits, the bit
  up from a stand), the speed mode at 0.78 / 0.71 MaxSpeed; the turn unwinds over (frames - 1) / 30; one opus
  verifier, ten claims SUPPORTED (one PARTLY). [H]: the alert branch behind a weapon's vtable+0x5c (unclamped twist).

## 2026-10-15 (loop lane 2, card VRIDE) — a vehicle's riders: which seats show their soldier, where he hangs, his pose → `analysis/re-vehicle-riders.md` [O]

- One function places every rider (B1 `FUN_0023e028`, B2 `FUN_002f74c0`), called by the hover / walker / flyer /
  PassengerSlot / MountedTurret updates with the vehicle's world matrix and pose table: `L x E x O`, and
  `AnimatedPilotPosition` = the animated node's own entry (`E x O`: tauntaun, kaadu, BF2 one-man AT-ST). The census of
  every vehicle seat on both discs (bikes, STAP, BARC, fighter-tank gunners drawn; tank pilots, AT-STs hidden); a
  hover's nine-pose rider leans by the drive's smoothed throttle / turn (`a = y + 4`, `b = y + 1 | 7`, `c = |p|`),
  which also pose the hover's own `FinAnimation`; a played seat clip loops at 30 fps (BF1). Seen on the PS2: two live
  bike rides (564 rows, 12 states) and 7 stored states. Workflow verifier (6 agents): 28 / 30 supported, 1 corrected.
  [H]: BF2's played-clip loop, the constant DummyRoot of `human_ride`. Build card VRIDE; new card VPOSE.

## 2026-10-15 (loop lane 2, card SKIPC) — the instances the port skips as "no drawable model": which the PS2 draws, and why → `analysis/re-skipped-classes.md` [O]

- A census of today's main (efa2d56) over 32 maps, both render paths: a BATTLE draws the console, the three recharge
  droids, the gun turrets (the COMMON bake is merged); the STILL path (pose_render without POSE_COMMON, the walkabout
  example: the parity frames, the PXWA / PXWB pairs, the golden ground views) skips them - card STLC's cause, now this
  card's build.
- Godrays (BASE `godray`, B1 `FUN_00370fc0` / `FUN_00370d18` / manager `DAT_00519498`, B2 twins `FUN_00455760` /
  `FUN_004554f0` / `DAT_00601290`): a manager built DISABLED and switched on by the map's `Effect("Godray") Enable`
  (on for BF1 END1 GEO1 RHN1 RHN2 YAV1 YAV2, BF2 DAG1 END1 FEL1 GEO1 YAV1); NumRays rays per placed instance within
  ±SpreadRadius. Live RAM on BF1 Endor: 1,080 active rays = 135 clusters × 8; a knock-out removes vertical light shafts
  from the PS2's frame. The port has none. New card GODR (the draw stays [H]).
- BF1 Geonosis cp2's building `rep_bldg_forwardcenter` (side REP.LVL, loaded by the conquest script): drawn on the PS2
  (two frames), no model and no body in the port. New card FWDC.
- Not gaps: Hoth's hologram 157.5 m under the terrain, the `com_inv_col_*` walls (invisible both ways), BF1 Theed's
  gian speeder (campaign-only), sound / rumble / spawn markers. Workflow verifier (7 agents): 46 / 47 supported, 1
  PARTLY fixed, the second verifiers overturned none.

## 2026-10-15 (loop lane 1, AUDIT of card HVYW: PARTLY) — section 22 holds; a foot soldier attacks a flyer in the air; the building flag is "MaxHealth is finite" → `analysis/re-soldier-ai.md` 24

- [O] **22.1 / 22.2 hold** (two blind verifiers on the same 12 claims: 10 SUPPORTED by both) and every number of the
  landing reproduces from rebuilt binaries (the commit messages' base fb4a2f1 and measured 548ef04 equal the rebased
  6dc59d4 / 3fba385 byte for byte).
- [O] **4.4's "a foot soldier never attacks a flyer in the air" is wrong** (both verifiers and the auditor): the
  matrices' soldier x flyer cell chooses a weapon on both branches (B2 `FUN_002ac2b8`, B1 `FUN_002397d8` ->
  `FUN_00239260`); only BF2's too-weak path asks the flyer's state / 16 m/s. The port refuses every weapon at a flyer of
  16 m/s+ in both games: card HVYA.
- [O] 22.1 step 5's building flag (+0x26e bit 0; BF1 +0xe0) = the class's MaxHealth is finite (B2 `FUN_00267bf0`
  185257). [H] `FUN_002ab430` throttles missile ordnance per target and team as well as grenades; its capacity for a
  vehicle target is unread (HVYA step 4).
- Workflow `audit-hvyw` (22 agents: 4 numeric, 3 controls, 13 mutations of which 8 survive + 1 that did not compile and
  survives when made to, 2 verifiers). Tools: `engine/artifacts/lead_tmp/audit-hvyw/tools` (setup.sh, chain_sweeps.sh,
  sw.sh, setup_mut.sh, mut_run.sh, mk_mut.py; patches/observer.patch puts the card's HEAVY_CENSUS on a parent).

## 2026-10-16 (loop lane 1, card VTURZ) — a building turret's zoom: its camera, stick zoom and FOV → `analysis/re-turret-draw.md` 4.1 [O]

- [O] The turret's camera interface has the soldier's slots; its zoom virtual is the weapon's zoom value while the
  turret's zoom flag is set (B1 `FUN_00225c90`, B2 `FUN_002704f8`), its FOV virtual the turret class's
  `FirstPersonFOV` (+0x1bc, default 48) zoomed (B1 `FUN_00225ba0`, B2 `FUN_002703d0`). BF2's turret camera virtual is
  `FUN_00270018` (section 4 had `FUN_0026fdf8`, corrected).
- [O] BF2's turret runs the continuous zoom on the move stick's forward axis (T+0x7c = controllable +0x6c,
  B2:189181-189182); BF1's turret calls only the toggle [receipt:dbbaa9f9].
- [L] The zoomed eye = `EyePointOffset` on the aim frame: the port within 0.1 mm of the PS2's camera on 8 RAM rows.
  One opus verifier: 7 of 8 claims supported, the B2 camera slot corrected.

## 2026-10-16 (loop lane 2, card MIXH) — the PS2's mix headroom: per-ear level law, the 39-voice cap, buses → `analysis/re-mix-headroom.md` [O]

- [O] A 3-D voice is split linearly between the ears, L + R = gain x level (B1 `FUN_0044cb78`, B2 `FUN_00540b98`,
  output mode 3 in every state): a centred sound is half per ear; the port (kira's sqrt 2 constant-power pan) is +6 dB
  per ear at centre, +2.3 dB at the side. RAM: 739 / 888 BF1 and 549 / 659 BF2 bound voices within 1 %.
- [O] BF2's effects bus plays at 0.63 (slider 7/10 x `main` 0.9); BF1 1.0. The only run-time duck is the `voiceover`
  bus attenuating `main` (0.5 / 0.65, 0.5 s fades) [receipt:78f2aa0a]; no duck by voice count.
- [O] 78 virtual one-shots, at most 39 sound (B1 `FUN_0044c078`); the steal takes the lowest-value voice (BF1 by
  level, BF2 by the property's bus gain and priority). The SPU2 clamps (PCSX2 model), master 0x3fff.
- [L] PCSX2 output captures of four front-line fights (SDL disk driver): 0-4 clamp runs of 0.2 ms in 4 minutes; the PS2's
  own starts through the port's law clip like the port. Workflow verifier (6 agents): 18 / 22 supported, 4 PARTLY
  folded in, the second verifiers overturned 1 (a count).

## 2026-10-16 (loop lane 2, card FAMB) — BF1 Naboo's fambaas: the shield, the generator, the mobile post → `analysis/re-fambaa-shield.md` [O]

- [O] Only the historical `nab1c_h` loads `NAB1_H.LVL` [receipt:2bfa41c9]; two pads of the regular world
  (`perma_spawn10` / `perma_spawn13`, `ClassHisDef`, Team 2, CP1) spawn the shield and generator fambaas
  (`commandarmedanimatedbuilding` = armed animated building + the command-post component).
- [O] The shield is a 60 x 30 x 60 m ellipsoid in collision layer 0x800 that only ordnance sweeps, and only coming in
  from outside (the ray test reports entries only): units, vehicles, aim and AI rays and blast splash cross it; a bolt
  from outside dies on it and hits only the shield's 100000 points.
- [O][M] The shield weapon (2 clips, AutoFire) refills the shield per shot; the generator's repair weapon, on a fixed
  50 m ray backwards, adds one clip (`BuildingAmmo 1`) every 0.1 s. Live PCSX2: reset every 0.1 s (125 / 135 gaps),
  never under 91667; with `BuildingAmmo` knocked to 0 the shield is 0 and off at +1.001 s and stays off.
- [M] The CIS player walks through it (75 -> 14.8 m); bolts from 75 m outside leave the fambaa's health untouched,
  from 14 m inside they hurt it. Workflow verifier (8 agents): 47 / 58 supported, 11 PARTLY folded in, 0 refuted;
  the second absence verifier overturned 1 wording.

## 2026-10-16 (loop lane 2, card FXC) — the chunks mungefs skips: shdw, CLTH, bnd_ → `analysis/re-chunk-census.md` [O]

- [O] `shdw` = a per-model shadow volume inside `modl` (BF1 240 chunks / 27 models, BF2 164 / 11: vehicles, no soldier).
  Both games rebuild it along a light vector and draw it after the model when `(flags & 0x230000) == 0x10000`
  (B1 FUN_00496248 -> FUN_004ad760 / FUN_004adbc8, B2 FUN_0057cf60 -> FUN_00592c38).
- [O][M] BF1's gate DAT_00522ed0 is 1 in the ELF and never written (its setter at 0x4ae850 has no caller)
  [receipt:af3dc61d]; a PCSX2 knock-out on NAB1 removes a ground shadow over 9.5 % of the frame (on / off 0.63).
- [O] BF2's gate is the Shadow effect object's: on at level start, then `FUN_00563f78() < 2` [H: one view]; per-map
  `Effect("Shadow") Intensity` (UTA1 0.2, TAT2 0.35, FEL1 0.5, default 0.5).
- [O] `CLTH` (BF2 only, 58 chunks) = cloth meshes named by `ClothODF`: Hoth snowtroopers' skirts, clone capes, hero capes,
  CTF flags; the port draws those soldiers without them. `bnd_` = `Boundary { Path(...) }`, empty on 8 BF2 maps (UTA1,
  MYG1 among them), absent on BF1 YAV1 / YAV2 [receipt:ae8beb67] (note on BNDY). Cards VSHD (4), CLOTH (5). Workflow verifier (6 agents):
  30 / 37 supported, 6 PARTLY folded in, 1 NOT SUPPORTED (bnd_ details, recounted); the second readers overturned none.

## 2026-10-16 (loop lane 2, card CLOTH) — BF2 cloth: the CLTH mesh, who wears it, the step, the draw → `analysis/re-cloth.md` [O]

- [O] CLTH (B2 FUN_001d9ca8): texture, N positions, N UVs, F pinned = vertices 0..F-1 with their bone names, T triangles,
  three distance lists, COLL sphere / cylinder / box shapes on bones; all 58 disc chunks parse to the last byte.
- [O] A cloth is attached by the model's `gmod` INFO list (FUN_001c59e8) + a `cloth` class of that name (spawn
  FUN_00234a68 -> E+0x10a4); `ClothODF` is not read by the game [receipt:aa5571e8] (corrects `re-chunk-census.md` 4).
  RAM (HOT1): the 13 Imperial snow soldiers share one geometry object listing the skirt; the Wookiee's listed dreads
  stay unbuilt (the census of the 35 cloth classes lacks their names).
- [O] The step: Verlet in the cloth frame, gravity -9.8, the frame's acceleration x Dampening 0.5, Drag -2, wind (1,0,0) x 10
  x one random number along the normals; pins skinned to their bones; lists then COLL pushes, 1 iteration; dt <= 0.039;
  run only while drawn (RAM: 1 of 13 skirts stepped, the on-screen one). Drawn two-sided (no VU1 cull bits), front normal.
- [O] A level-start memory rule can switch cloth off for a level (HOT1: on); a Priority buffer budget (never bites on HOT1).
  Cards: CLOTH -> build (slice 1, rigid-skinned bake), CLOTH2 (M1, 4: the step). Workflow verifier (8 agents): 34 / 35
  supported, 1 PARTLY (the draw's reversed half is dead; rewritten); absence / twin set 12 / 12 by two readers, none
  overturned; measurements 5 of 6 reproduced, 1 PARTLY (only the drawn skirt is stepped, folded in).

## 2026-10-16 (loop lane 2, card RIDK) — a destroyed vehicle's riders and the kill credit → `analysis/re-vehicle-death.md` §10 [O]

- [O] At a vehicle's death (BF1 FUN_001ea190 / BF2 FUN_0021a840, after the blast) every turret, passenger and driver seat
  runs FUN_0023ac68 / FUN_002f4e00: the occupant is KILLED (FUN_001bbc98 / FUN_001df640, third argument 0: past the
  invulnerability switch) with the vehicle's last-damage record (E+0x24 / E+0x190) — the killing hit's attacker.
- [O] The member death (B1 FUN_00297440 / B2 FUN_0034f650, each with one caller [receipt:3e407ebe] [receipt:1c47752a])
  credits that attacker: a kill per rider (a team kill if friendly), the destroyer's vehicle-kill stat +1, a ticket per
  rider; a record with no attacker (a crash into the world) = deaths with no killer. Exposed riders (layer 0x10 / 0x20)
  leave a corpse (flung only by a turret seat with `PilotDeath = "fly"`), riders inside the hull are deleted.
- [O][V-live] BF1 RHN1 + NAB1, manned hovers at 1 HP (`lead_tmp/ridk/tools/ridk_probe2.py`): 6 deaths, 11 riders, every
  rider's record = the vehicle's, the 5 destroyers' kills + riders and vehicle kills + 1 within 0.1 s, the empty record
  credited nobody. Workflow verifier (7 agents): 28 claims, 24 supported, 4 PARTLY folded in (one of them a twin that
  both readers marked PARTLY); the second readers overturned none. Card RIDK -> build.

## 2026-10-16 (loop lane 2, card LOD) — the LOD switch rule: model, LOWD or nothing; soldiers' `_low1` → `analysis/re-lod.md` [O]

- [V census] A gmod is ONE model in a size slot (LOD0/1/2 by count) + an optional LOWD; 2 gmods per disc have a chain.
- [O] Far scene (FarSceneRange > 0): LOWD-bearing objects are their LOWD past NearSceneRange b..c, per pixel; objects
  without a LOWD are not drawn past c. Near scene: per-slot frame budgets (BF1 nearest first 35000/20000/20000; BF2
  largest first 100000/20000/20000) with a LOWD / nothing fallback; BF2 adds a 5-px size cull and a density switch (LOWD
  past d = r·S·√(208000/count)). Soldiers: `GeometryLowRes` IS read (re-softskin §7 corrected) — ~4 hi-res a frame.
- [V-live] PCSX2 live pokes: NAB1 two stations (tree LOWD slot → LOD0 = the port's bare far trees, 39–43 levels; a near
  head unchanged); a BF2 MYG1 threshold test (count 0.6× / 1.6× of 208000·key flips exactly the predicted instances);
  budgets → ∞ change nothing at 6 poses, → 0 turn near objects into LOWD cards. Workflow verifier (5 agents): 26 claims,
  23 supported, 3 PARTLY folded in; the 7 absence / twin claims supported by both readers, none overturned. Card LOD -> build.

## 2026-10-16 (loop lane 2, card SBLT) — why the port's BF1 soldiers are darker: the soldier ambient slot → `analysis/re-soldier-light.md` [O]

- [O] BF1 soldiers take ambient word 0x69c578. The mission reset writes (128,128,128) there; the per-frame HUD model draw
  `FUN_004b8e78` restores it through the `CharacterAmbientColor` setter, which sets that key's flag, so `AmbientColor`
  can no longer reach it. A map without `CharacterAmbientColor` (KAS2, NAB1, YAV2) lights soldiers at (128,128,128); the
  port uses `AmbientColor` there. RAM: all BF1 states agree (KAM1 lightning rows excepted).
- [V-PS2] KAS2 SBD (no pass), patched states: PS2 51.2 -> 38.8 with the port's ambient; port 28.1 -> 43.8 with 128;
  ambient-only terms equal (36.2 / 34.5). The sun's form (d⁴, bone-turned normals) matches; the PS2 takes 2.6-7 x more
  sun from overhead / in front - open [H] (card SSUN). BF2 KAM1 no longer darker (PS2 55-57, port 62).
- [O] The soldier LUMI tint min(1.2·LUMI + 0.3, 1) on maps with `DomeInfo` `TerrainColorDarkening` (0xd8a93a22): BF1
  END1 / NAB1 / TAT1 / TAT2, BF2 END1 / KAS2 (card SLUMI). Workflow verifier (5 agents): 25 claims, 23 supported, 2
  PARTLY folded in; the 6 absence / twin claims supported by both readers (X6 one PARTLY), none overturned. SBLT -> build.

## 2026-10-16 (loop lane 2, card USKN) — a class's own skeleton, SkeletonRootScale, CollisionRootScale, the override draw → `analysis/re-soldier-skeleton-skins.md` [O]

- [O] SkeletonRootScale is no mesh scale: both games' pose builders (B1 `FUN_00440928`, B2 `FUN_00533d38` + 4) multiply
  ONE bone's translation by record +0x68 (`VMULx.xyz`); the scale sits on bone_root and lifts the human clips' root to the
  class's leg length. A class with its own `SkeletonName` keeps its rig's bone LENGTHS (BF1 `FUN_0022f428`: 22 body bones;
  BF2 `FUN_00281e68`: all but bone 0, bone_root, hp_weapons). PS2 RAM of 6 states agrees (scales 1.15 / 0.5 / 1.1 / 1.24 /
  0.95 / 1.25 on bone_root, flags 3).
- [V code, measured] The port poses every soldier at the human clips' lengths: Wookiee head 1.605 vs PS2 1.852 m,
  MagnaGuard 1.643 vs 1.865, battle droid 1.605 vs 1.726 (both games' CW droids distorted); the clone trooper unchanged.
- [O] The scale also divides the velocity before the movement clip choice (B1 `FUN_0022e990`, B2 `FUN_0027ed18`); no
  camera or speed code reads it. CollisionRootScale sizes the soldier's collision primitives (B1 `FUN_0020e2e0`, B2
  `FUN_002517e8`) → card USKC. The live override draw is B1 `FUN_002013a0` / B2 `FUN_00239bb0` (UAUD1's `FUN_001be1e8`
  corrected); OverrideTexture2 only with OverrideTexture in both. Workflow verifier (5 agents): 36 verdicts on 28 claims,
  29 SUPPORTED, 7 PARTLY folded in (the low-res key scales `bone_pelvis`), none overturned. USKN -> build.

## 2026-10-16 (loop lane 2, card VSHD) — vehicle shadow volumes: BF1 counts them and darkens x 0.625, BF2 never draws them → `analysis/re-vehicle-shadow.md` [O]

- [O] `shdw` = per-node closed half-edge meshes (SHDI 4 u16 {vertex, next, twin = h ^ 1, 0xFFFF}); all 404 disc chunks
  parse. BF1 (FUN_004ad180 / FUN_004adbc8) extrudes each piece's silhouette along the SunInfo direction (DAT_0069c590) by
  len = 500 (flyer) or 150 (hover / walker) x the terrain hit fraction (FUN_00474680), caps only when the model's view
  depth < 30 m + radius (then z-fail, else z-pass).
- [O] Bucket 6 counts the volumes into a cleared 512 x 448 buffer at FBP 0x180 with COLCLAMP off (VU1 pass flag 0x100:
  +1 / 255 per triangle by screen facing, 0x544); its tail (FUN_00499f38's list, CALLed by FUN_004a7098) multiplies the
  main frame by 1 - 0x30/128 = 0.625 wherever the count is non-zero (PSMCT24 + TEXA AEM). Black, no per-map key
  (BF1 ShadowColor's setter FUN_00493120 is empty). [M] PCSX2 NAB1 TEXA poke: TA0 0x80 black, 0x00 none, 0x60 0.229,
  0x30 0.585 displayed.
- [O] BF2's piece draw slot 0x593090 is an empty function (ELF and RAM of three battles): no vehicle shadow; its
  darkening list (DAT_0060c310) and Effect("Shadow") Intensity (DAT_007c5758) are never read [receipt:02bc10f8]
  [receipt:3dc255e4] (corrects
  `re-chunk-census.md` 3). Cards VSHD (-> build, data), VSHD2 (draw), SBLOB (the separate `player_shadow` blob). Workflow
  verifier (6 agents): 27 / 27 batch claims SUPPORTED; 6 absence / twin claims by two readers each, 5 SUPPORTED by both,
  1 PARTLY by both (the gate's dead setter 0x4ae850, wording), none overturned.

## 2026-10-19 (loop lane 2, card BNDY) — the map boundary: the shapes, the half-plane test, the 10 / 6.5 s countdown → `analysis/re-boundary.md` [O]

- [O] Shapes come only from the map LVL's `bnd_` chunk (`Boundary { Path(name) }`, the first path of a name; one caller
  of the push per game [receipt:e88fecd8]); per shape a CONVEX half-plane test over its closed edges, a point inside any
  shape is inside, x and z only, no shapes = never outside (B1 FUN_002956d8 / FUN_002958f8, B2 FUN_0034c550 /
  FUN_0034c780). 22 of 32 baked maps have one; BF1 YAV1 / YAV2 and BF2 COR1 DAG1 DEA1 MUS1 MYG1 NAB2 TAN1 UTA1 none.
- [O] Every unit with a soldier (AI too) is tested each update; outside -> BF1 10.0 s / BF2 6.5 s (2.17 s for a steered
  guided rocket), the local human's team line + "Leaving Battlefield" (alpha spring), killed below 0 with what it
  drives (the invincibility cheat spares); inside -> cleared. [M] 296 / 296 PS2 test points agree on the six showcase
  maps + BF1 KAM1 / HOT1 (12 non-convex points outside on the PS2), RAM shapes = the transcription on 32 maps, death at
  10.0 / 6.5 s, AI timers run on BF1 and BF2.
- [O] Corrects the 10-16 draft: BF2's flyer +0x660 bit 7 is the fly-height flag (Lua Min/MaxPlayerFlyHeight, default
  +-200, set per mission, e.g. UTA1 29.5, MYG1 20): a human flyer above the ceiling counts as outside (card VFLY).
  Workflow verifier (6 agents, every claim to two readers): 16 / 19 SUPPORTED by both, 2 PARTLY by both (narrowed),
  1 NOT SUPPORTED by both (rewritten), none overturned.

## 2026-10-19 (loop lane 2, card GUNWT) — the AI driver's wait for a gunner, the need-gunner call's two sources, the answerers' boarding → `analysis/re-driver-wait.md` [O]

- [O] BF1: every AI pilot of a walker / hover / flyer gets a new controller whose +0x280 window is now + 4.0 s
  (`FUN_002623c8`); the 7th agent check pushes goal 4 inside it -> `UnitWaitSecondaryAgent` (vtable 0x537300, update
  `FUN_0024d0f0` - decompiled, the card's "hole" was the empty exit): the vehicle stands; done when every seat is
  occupied, or past the window with no live kind-1 team boarding claim (claims restamped each think, dead 3 s after
  the last); MOVE OUT / AT EASE sets the window to now - 2. [M] 15 live waits in 396 BF1 states, window - clock
  -1.91..+2.53 s, the ones past it held by claims aged 0.0-0.9 s.
- [O] The need-gunner call (0.3) has TWO sources: the wait's entry, and every set-out of a SEATED trooper agent
  (`FUN_002613f8` turns its FOLLOW into type 4; [M] 551 of 1,756 live seated drivers run the trooper agent). The
  issuer's only gate is "seated" ([M] 9,800 on foot / 1,755 seated, `FUN_00260f38`): no seat test, a full vehicle
  calls too. Every friendly AI on foot within 30 m answers and gets a board goal; free seats = seat count - live
  claims - human-held seats; a boarder gives up when the vehicle's +0x48 vector reaches 2 m/s [H: the velocity].
- [O] BF2 twin: the same wait (4.001 s from the agent, claims, full seats), no call, and a hold of up to 15 s while
  the human player is on foot within 12 m, or within 150 m facing the vehicle (30 deg) - before the seat test; [M]
  +0x130 >= 0 is the player alone in all 179 BF2 states. Corrects `re-squad-orders.md` 5 / 9b. Card GUNWT -> build
  (steps A / B), new card GUNWH (the BF2 hold). Workflow verifier (6 agents; absence and twin claims to two readers):
  29 claims, 24 SUPPORTED by every reader, 5 PARTLY (narrowed), 0 NOT SUPPORTED; the readers split on 4.

## 2026-10-20 (loop lane 2, card SSEL) — the start-of-level and respawn flow: side select, spawn map, class select, the death camera → `analysis/re-spawn-flow.md` [O]

- [O] BF2 PS2 order: side select (Lua `ifs_sideselect`: team 1, team 2, Auto Assign; Spectate hidden offline) -> spawn
  map -> class select -> spawn; after a death the death camera for 4.0 s, then spawn map -> class -> spawn (no side select
  again). One state machine per viewport (`FUN_00398000`, SD+0x18: 0 side, 2 map, 3 class, 4 class while alive, 5
  accepted). The side select is an Instant Action screen ("can switch sides" `DAT_00600b4e`).
- [O] Auto Assign offline keeps the player's team (team 1 at the start): no team call; [L] team 1 on Hoth, Kamino, Naboo
  and in 37 of 37 side-select states. The coin flip `FUN_0036fc40` runs only in Galactic Conquest. The highlighted
  team's first class stands in the backdrop as a preview.
- [O] The human spawns 0.10-0.13 s after X on the class, waits for no spawn wave, and never spawns without the accept
  (the port today: wave + automatic). Backdrops: the mission's `AddCameraShot` views at the level start (a new random
  one per screen), the death view at a respawn, dimmed ~0.51. [L] Death camera: last yaw kept, pulls back to ~5.2 m /
  4.1 m above the feet at -32 deg in ~0.6 s, 65 deg FOV; spawn map at +4.07 s. Killer turn not captured [H].
- Live runs: `lead_tmp/ssel/ssel_live.py` (one row per game update). Workflow verifier (7 agents, absence claims to two
  readers): 33 claims, 35 verdicts, 26 SUPPORTED, 9 PARTLY (narrowed), 0 NOT SUPPORTED; the second reader overturned one
  sub-claim. Build cards SSELA (sim gate + side choice), SSELB (the screens), SSELC (death camera).
- [O] Second pass (same day, lane 2; corrects the death-camera line above): the death camera is its own object (BF2
  vtable 0x617d78, update `FUN_001bc188`, §6.5). Phase 1 looks at the body + 0.8 m from 6.22 m along a line fixed at the
  death (1 m back / 1.5 m up, 5 m past it), eased by t = timer / 1.0; after an ENEMY kill, from 1.5 s, phase 2 moves
  onto the body and looks at the killer (`E+0x190`, his member record; his unit's or vehicle's +0xd0 = feet + 1.70 m),
  following him. Suicides (no killer) and hero classes (none passed) stay in phase 1. It sets no base FOV: tan 0.40237
  = one 65 -> 48 deg ease step of the player camera [D], matching CFOVM's reported FEL1 tangent; identifying that
  particular state's moment remains [H] pending its own JSON/save-state read (re-spawn-flow §6.5).
  [L] 10 enemy kills on Kamino with 16 AI a side
  (phase 2 in all 7 with rows), 3 suicides, 1 explosion kill. Workflow verifier: 21 verdicts, 17 SUPPORTED, 4 PARTLY
  (narrowed), 0 NOT SUPPORTED; the second reader overturned none.

## 2026-10-20 (loop lane 1, card BSHL) — the BF2 PS2 shell: screens, flow, stack look, movie, loading screen → `analysis/re-bf2-shell.md` [O]

- [O] Read from the disc's own compiled shell scripts (SHELL.LVL 92 + COMMON.LVL 42 Lua 5.0 chunks) and PCSX2 frames,
  never PC layouts. Flow: legal / logos -> Press START (only Start) -> profile -> Main Menu (Singleplayer, Multiplayer,
  Splitscreen, Options) -> Singleplayer -> Instant Action screen -> Session Options -> LAUNCH -> BF2 loading screen.
- [O] One vertical-button stack style: `interface_util`'s PS2 palette and pads, `gamefont_large`, rows font + 6, gutter
  4 (measured pitch 31.97 units); a `bDimmed` button is grey 110 and focus skips it - the PS2's own disabled style,
  which settles the port's greyed-out Multiplayer. [M] labels 1.087 x the font advances across, bar = label + 134.2,
  left-justified stacks' edge at x 34.4 units; the Lua left-justify formula does not reproduce it [H].
- [O] The background is a movie, not art: SHELL.MVS (34 MPEG-2 512x448 segments; `shell_main` 0-1, `shell_sub_left`
  2-9, silent; 10-31 carry PSS audio). The shell screen is 512 x 448, safe 0.9, aspect 0.875 (`FUN_00580b08`). The IA
  screen has no movie: `iface_instant_action`'s top 448 rows. The 30 s title idle plays the attract MOVIE: the engine
  sets `gMovieAttractMode` from a constant 1 byte, so `ScriptCB_LaunchDemo` is not reached on the PS2.
- [O] Loading: LOAD.LVL's `load` config (tips `level.common.tips.1..54`, `loadtipsbox_pieces`, `loading` dots); the
  port draws only the still and a caption. Two verifier workflows (8 agents, absence / flag claims to two readers):
  40 claims, 31 SUPPORTED, 9 PARTLY (narrowed), 0 NOT SUPPORTED; the second readers overturned none. Build cards
  BSHLA (title + stacks + bake), BSHLB (the IA screen), LDBF2 (the loading screen).

## 2026-10-20 (loop lane 2, card SSELB slice 4) — the class select's line-up ring → `analysis/re-spawn-flow.md` §4.1 [O]

- [O] Six places on a ring (`DAT_006a5080`, `FUN_0039bcb0`), place 2 the pick, place 5 unseen (colour 0 0 0 1); a
  soldier's place = lerp of two places x 150 + (100, 350, -452) (`FUN_0039a5f8`); a step slides the ring one place in
  1/3 s (`SD+0x3c += dt x 3`, `FUN_0039a980`), a step during a slide is dropped. One opus verifier: C1-C7 SUPPORTED, C8
  PARTLY (wording). The UI scene's perspective is fitted to the Hoth frame [M]; built in `game/src/spawn_preview.rs`.

## 2026-10-20 (loop lane 1, card PWST) — health, ammo and vehicle droids (`powerupstation`): reach, rate, team, animation, sounds, tips → `analysis/re-powerup-station.md` [O]

- [O] A station gives once every `PowerupDelay` s (its own clock, the remainder kept) to every unit whose feet are
  strictly inside the 3-D sphere of its instance `Radius` (5.0 m; vehicle droids 15.0): the class's amount for the
  unit's health type, to the cap - BF1 +15 health / 1.0 s, BF2 +25 / 1.0 s, ammo +1 clip to every weapon / 2.0 s, BF1
  vehicle droid +400 / +1 clip to vehicles. No team test (both games; every disc station Team 0); only while the
  building state is 0 (not destroyed, not unbuilt); BF2 refuses the SetHeroClass hero's health. B1 `FUN_002a9750`,
  B2 `FUN_003674c0`.
- [O] Measured live (`lead_tmp/pwst/ps2/heal_live.py`): BF1 NAB1 75 -> 300 in 15 steps of +15 every 1.001 s, served at
  4.56 m, not at 5.43 m; ammo 0 -> 4/3/1 (+1 / 2.0 s, caps max - 1); BF2 HOT1 100 -> 400 in 12 x +25, served at 4.60 m,
  not at 5.34 m. Census: 106 stations on the six showcase maps, 967 on all 32, every one `Radius`, none `EffectRegion`.
- [O] No bot seeks a station: the type ids and the entity kind code (B1 17, B2 18) are read only by HUD tooltips and
  network replication. Feedback: `PowerupSound` per served unit at the station; BF1 tooltip 10 "THIS IS A RE-SUPPLY
  DROID" (7 / 8 / 9 for a broken one); BF2 hints 15 / 16 / 17. The animation (idle 135-225 deg turns, gonk wobble, FX-7
  arms +-0.15 rad and socket +-pi at 1 Hz while serving) is code-driven bones; the models split into rigid parts.
- Two verifier workflows (7 agents; absence / twin claims to two readers): 28 claims, 22 SUPPORTED + 2 S/S, 4 PARTLY
  (fixed: the kind code, three BF2 differences, a citation); the second readers overturned one (B2 bots), and the
  revised bots claim then held with both. Build: card PWST phase build; tips on card TIPS.

## 2026-10-21 (loop lane 1, card SSELB slice 7) — the spawn screens' soldiers' light → `analysis/re-spawn-flow.md` §4.2 [O]
- Every UI model draw carries flag 0x4000000, and the model renderer (`FUN_0057cf60`, `FUN_001de2a8`) then lights it
  with the interface set 0x7d6160 instead of the map's lights. On the spawn screens `FUN_00398000` sets that set every
  call: one white light (GS 1.0) travelling (-1, -1, -1)/sqrt 3 and a flat ambient of 0.5. A refused class's colour word
  is black (`FUN_0039a980`). Light on, direction frame and colour scale (byte / 255) measured on the Hoth frames [M].
  One opus verifier: 8 SUPPORTED, 1 PARTLY (the light's enable bit after init, settled by measurement).

## 2026-10-21 (loop lane 1, card SSELB slice 8) — which class the PS2 refuses → `analysis/re-spawn-flow.md` §4.3 [O]
- `FUN_00364290`: a human whose points (stats+0x1fc) are under the class's `PointsToUnlock` (officers 8, the specials
  12) is refused, and so is an unlock class already at its `AddUnitClass` max; bots never (the respawn checks a human
  only, the AI pick reads min / max). One opus verifier: 9 SUPPORTED, 3 narrowed.

## 2026-10-21 (loop lane 1, card SSELB slice 9) — the class carousel's dropped step → `analysis/re-spawn-flow.md` §4.1
- `FUN_00399dd8`: the arm `SD+0x25` is set only by a step that fired, so a push held through the slide steps again at
  its end; the input runs before the slide's advance (`FUN_0039a980`); the slide is 11 updates [D] (EE add rounds
  toward zero). One opus verifier: C1, C2, C4-C6 SUPPORTED, C3 corrected (10 -> 11 updates).

## 2026-10-21 (loop lane 1, card HTGT) — the target's name and health on the HUD → `analysis/re-target-hud.md` [O]
- BF2 `FUN_003b3398` shows the reticle's pick (K+0x180+8·ch, the PlayerController's winner) at once, no hit or hold
  [receipt:c983e4eb]:
  class name (Locl `entity.<side>.<rest>`), health / max bar in friend blue / enemy red / team-0 white / neutral yellow
  (× the group's Color), a 1 s flash over the lost span, a 1 s fade on loss with the bar turned white. BF1: an
  enemy-only red bar under the player's health (2 s fade) and a name tag over other HUMANS only. Workflow verifier
  (4 agents): 10 + 4 PARTLY plain; A1 (BF1 no name) overturned by both absence verifiers — fixed.

## 2026-10-21 (loop lane 2, card HATK) — where the shot came from: the hit-direction chevrons → `analysis/re-hud-attacker.md` [O]
- Both PS2 games: a hit on the local player's soldier (and droideka / hover / walker; BF1 also flyer / carrier) shows
  the `ambush_icon` chevron at the screen edge toward the attacker — left, right, bottom (from behind, shot level or
  down) or top (from behind, travelling up), none from the front; full opacity, a linear 1.0 s fade, one timer per
  edge (B2 `FUN_003912c0` / `FUN_00391680` / `FUN_003915a8`, B1 `FUN_002d7c80` / `FUN_002d8028` / `FUN_002d7fa0`),
  fired by the TakeDamage virtual before the damage is known. The layout's Target `Attacker*` templates are dead data
  (nothing sets the record's bit 3) [receipt:9028bdc3]. Measured: 32 BF2 hits on two maps (26 exact under the rule),
  four PCSX2 frames, one BF1 hit. Workflow verifier (4 agents): 14 plain claims 11 SUPPORTED / 3 PARTLY (fixed),
  3 absence / twin claims by two readers each: 2 SUPPORTED by both, 1 PARTLY by both (an overstatement, fixed), none
  overturned.

## 2026-10-21 (loop lane 2, card HCORN) — a hover's detour corners: radius, origin, the drop inside R → `analysis/re-hover-corners.md` [O]
- Both PS2 games plan a hover's corner walk from the waypoint just reached (B1 `FUN_002509b0`, B2 `FUN_002c0220`; live
  200 / 200 BF1 Endor legs, 54 / 54 BF1 tat1 legs) at the hover's own radius: corners r + 0.5 m out, r = half the X/Z
  diagonal of the class model's box (B1 `FUN_00235ad0`, B2 `FUN_002a76b8`, kind 6 x 0.5; BF1's box = the model's
  `dummyroot` INFO min / max, read in RAM and on the disc; live BF1 bike 2.94, BF2 bike 2.45, hailfire 6.89). A corner
  already inside the node radius is left after one tick, as card HJAM built it, and 5 of the PS2's 15 BF1 Endor crash
  kills follow one. The port plans from the unit at the soldier's 1.0 m: that is tat1's deadly corner at barrier267
  (prototype, from the waypoint: rock deaths 41 -> 4, 54 -> 6 in 24 + 24 seeds). PS2 tat1 capture: 0 crashes at the
  rock in 10 min. Workflow verifiers (11 + 3 agents): 28 claims, 24 SUPPORTED, 3 PARTLY (fixed), 1 absence NOT
  SUPPORTED by both readers (the bounds ARE on the disc - fixed); no twin overturned.

## 2026-10-21 (loop lane 1, AUDIT of card BRSTU: PARTLY) — the burst's meter slots decoded; the burst state's second writer → `analysis/re-soldier-ai.md` 23.6 [O]
- BF2 cannon vtable 0x628310: slot +0x2c = `FUN_00331460` = ShotsPerSalvo x HeatPerShot, +0x30 = ShotsPerSalvo (decoded from
  the ELF bytes; +0x294 written under ShotsPerSalvo's hash 0x3b38558). The landing's [H] holds as its code reads; SalvoCount is in
  neither test. `FUN_002bbed0` (B2) / `FUN_0023d3f0` (B1) reset the burst state on a weapon switch. Workflow `audit-brstu`, two
  verifiers on 10 claims: 7 SUPPORTED by both, the auditor's own absence claim NOT SUPPORTED by both (the writer above), 2 PARTLY.

## 2026-10-21 (loop lane 1, card TCOL) — a building turret as a body: its moving prims, its gunner, its death → `analysis/re-turret-collision.md` [O]
- Both games place every collision body through its owner's pose table by its PRNT hash (`FUN_00160160`), and an
  armed building hands its turrets the SAME table its ray and target queries read (B2 E+0x2a8, B1 E+0x2d8); the
  turret / aimer / barrel nodes are entered as pointers to their live matrices, so those prims turn with the gun. The
  port takes PRNT as the identity: Geonosis's cannon's 3 gun prims sink into its base (PS2 top 3.18 m, port 1.58,
  drawn 3.21) - the maintainer's "upper half". Census of 47 placed classes: every static-node prim binds at the identity.
- Live BF2 Hoth (2 x 240 s, per update): a hit on the building never moves its gunner (38 / 38); drawn gunners keep a
  body at their seat (type 0x20, 7 / 7; BF1 type 0x10, 84 / 84) that every flying ordnance's per-update ray tests; a
  hidden gunner has none (3 / 3); a building's death kills its gunner with the building's last hit (8 / 8: corpses /
  deleted) and 10 of 24 turrets fell, 2 were rebuilt. Workflow verifiers (5 + 2 agents): 28 claims, 37 verdicts,
  32 SUPPORTED, 5 PARTLY (fixed), none refuted; the 8 twin claims and the ray chain each by two readers, none overturned.

## 2026-10-21 (loop lane 2, card KA137) — BF2 Kamino's arc 137 over the sea → `analysis/re-kam1-arc137.md` [O]
- KAM1.LVL ships three top-level `plan` chunks (0 / 124 / 143 hubs); `FUN_002ec900` frees the held graph on every one,
  so the last is live - read out of RAM in two KAM1 states (143 hubs, 172 arcs). Its arc 137 (`Connection182`,
  121 -> 123) is SOLDIER-only with flags 0 (a plain leg, `FUN_002c0220`), the sole route for 56 + 74 destinations,
  and runs ~20 m over open sea: no instance between the hubs, a PS2 soldier dropped on its line falls to y -46 and dies.
- The PS2's own bots walk it from 123 and drown (2 of 4 follower walks in 16 min; a third rail-guided to the foot of
  hub 121's deck); the neighbour jump arc 136 dropped 1 of 3 into the sea. The port's BNDS `arc_unwalkable` keeps
  soldiers off it - a named deviation, kept. Main 2198036, 24 seeds: 0 recycles, stands or falls near (54, 144).
  Workflow verifiers 5 + 2 agents, 27 verdicts, none refuted; the second verifier caught a recycle count taken with
  the observer off (re-measured).

## 2026-10-21 (loop lane 1, card HVYA step 4) — BF2's explosive claim (grenade + missile ordnance) allows one soldier of a team at a soldier, and one per hull position (hull + turrets + passenger slots) at a hover / walker / flyer → `analysis/re-soldier-ai.md` 24.5 [O]

## 2026-10-22 (loop lane 2, AUDIT of card RKHUL: PARTLY) — a walker's collision prims ride its live pose → `analysis/re-walker.md` 17 [O]
- Body = XFRM × table[PRNT] in both games (`FUN_00160160`, row vectors, XFRM first): RKHUL's order. A walker's table
  (B2 E+0x1d7c via physics vtable 0x61f7f0 slot +0x4c; B1 E+0x1ec0 via 0x533a80 slot +0x44) holds pointers to its
  skeleton instance's bone matrices, rewritten every update (B2 `FUN_00533f20` in `FUN_00261208`, B1 `FUN_00440b38` in
  `FUN_0021b3a0`): the prims follow the clip, where the port bakes them at the bind (a parked spider's legs 3.4 m off
  the drawn ones; card WHULL). Static read; workflow `audit-rkhul`, two separate readers, 4 claims each, all SUPPORTED
  by both, none overturned; the auditor's `re_check` fn / word runs on every cited line and word.

## 2026-10-22 (loop lane 2, card LOOKR) — the look stick's turn ramp, both games → `analysis/re-look-ramp.md` [O]
- A human's yaw rate blends MaxPitchSpeed -> MaxTurnSpeed by a 0..1 timer the player controller runs (+dt, cap 1)
  only while the look stick is FULL, else 0 at once: BF1 `FUN_0025e098` K+0x120 on |turn| >= 1.0, BF2 `FUN_002b0548`
  K2+0xf0 on turn^2 + pitch^2 >= 0.9; read by the yaw rules B1 `FUN_002098b8` (x the look sensitivity 0.19 n + 0.4,
  n = 5 in every state: x 1.35) and B2 `FUN_00247fa8` (no factor). Rifleman 38.7 -> 309.4 deg/s (BF1), 57.3 -> 286.5
  (BF2); pitch flat at the MaxPitchSpeed end; bots flat MaxTurnSpeed (BF1) / 4 x MaxTurnSpeed (BF2, `FUN_00238068`).
- No soldier authors MaxPitchSpeed: BF1's default is 0.5 (the port uses BF2's 1.0 for both); the aim assist's R is
  MaxPitchSpeed in both games (re-aim-assist 5's MaxTurnSpeed hypothesis corrected).
- Live: 12 padscript runs (full stick and AxisScale 0.5 .. 0.98), the RAM timer per update: 880 / 880 turning updates
  and 1380 / 1380 timer updates fit; a release resets, an instant reversal keeps it, BF2's pitch-only push builds it;
  95 % of the throw never ramps in either game. Port: `LOOK_SCALE` is applied before the sim, so no full test passes
  on a pure turn - the vehicles' timer never runs for a human (card VTRN's note). Workflow `lookr-re-verify` (5
  agents): 34 verdicts, 26 SUPPORTED, 8 PARTLY (fixed), none refuted; cards LOOKR (build), BTURN.

## 2026-10-22 (loop lane 2, card OVHT) — the weapon heat gauge on the HUD, both games → `analysis/re-heat-hud.md` [O]
- BF2: the reticle's `player1weapon1heatbar` ring (30 `white` segments, -150..150 deg, yellow -> orange by its
  NumberColor) is fed every frame by `FUN_003b3398` with clamp(1 - cur/max) of the weapon's heat component for an
  unlimited weapon (RoundsPerClip under 1), 0 for a counted one (its charge while charging); while overheated (state
  5) it also sends `player1.weaponsOverheat` ("OVERHEATED!") and `refire` = 1 - HeatThreshold/heat (two red bits).
  Heat component: spend `FUN_001e2010` (HeatPerShot), refill `FUN_001e1fc0` (HeatRecoverRate x dt), thr = 1 -
  HeatThreshold (`FUN_001e1cb8`).
- BF1: a code-drawn bar in the weapon panel (`FUN_002c0400`, sheet `hud1`, 84 x 13 at (30, 2)), width 1 - heat, blue
  (0,152,255) / red (200,100,100) while overheated, only for a weapon with no ammo per shot; latch at heat >= 1, clear
  at <= HeatThreshold (`FUN_00284318`).
- Live PCSX2, the pistol tapped to overheat in both games with the heat read every update: the rules hold to the
  update; the ring's lit arc and BF1's bar fill match the heat on 10 frames. Census: 56 BF1 / 75 BF2 heat weapon
  classes (55 / 69 unlimited). The port's sim has the same heat; its HUD sends none of the events. Workflow
  `ovht-re-verify` (5 agents): 24 verdicts, 22 SUPPORTED, 1 claim refuted by both absence verifiers (an EnergyBar
  search misread; fixed). Cards OVHT (build), VSHUD (the HUD in seats); a note on BHUD3 (the charge on the ring).

## 2026-10-22 (loop lane 2, card UORD) — ordnance that is not a bolt: missile + the lock, shell, bullet, emitter → `analysis/re-ordnance-kinds.md` [O]
- The bolt family (bolt, laser, bullet, shell, missile) shares one flight core, one per-frame sweep and one impact
  handler. The predictive spawn ray tests buildings + terrain only (B1 0x48 / B2 0x90); units, hulls, riders, water,
  foliage and shields are struck by the per-frame prev -> current sweep (0x996 / 0x132e) — corrects re-blaster-damage
  §5.1 ("cannot dodge") and the port's projectile.rs. Missile and shell skip the predictive ray (class flag 0) and
  sweep everything every frame. Life-out = ExplosionExpire for all.
- Missile (B1 `FUN_0027d6b8`, B2 `FUN_0031f2e8`): constant speed (BF2: MinSpeed -> Velocity at Acceleration); turn the
  direction toward `desired` by min(ramp x TurnRate x dt, |dir x desired|), ramp 0 -> 1 in 1 s; waver about
  sin(ph1) p1 + sin(ph2) p3; no gravity; desired = the locked target's point, else the launch line. A launcher's
  missile gets a target only after the dwell lock (LockTime: BF1 torpedo 1.5 s, BF2 rocket 1.0 s, consumed by each
  shot); bots never wait for it. BF2 adds a bot aim error and a warning to the target's occupants.
- Shell: gravity, explodes on first contact (Rebound / Friction dead). BF1 mortar: tap = shell, hold = sticky.
  Emitter: one instant cone-and-chain query per shot (BF2: hit-all, team filters, burn). BF2 shotguns: 8 bullet
  pellets in one update + 0-damage tracers.
- Live PCSX2 (both games, AI battles, the player's launcher and every bot missile, one row per update): 1,400 update
  pairs of 25 missiles replay the law to ≤ 1.2e-4 m/s; three missiles with a target knocked into RAM aim at the read
  target point to ≤ 0.009°. Four reader agents + workflow `uord-re-verify` (6 agents): 52 verdicts, 44 SUPPORTED,
  8 PARTLY (fixed), 0 refuted. Cards UORD (build: missile + shell + lock), UORDS (the bolt sweep, value 4), UORDE
  (the emitter, value 3); banners on re-blaster-damage §5.1 / §7.3 and re-vehicles' lock-on line.

## 2026-10-22 (loop lane 2, AUDIT of re-ordnance-kinds.md: PARTLY) — bolts are swept against units, measured live in both games → `analysis/re-ordnance-kinds.md` 2.4 [O]
- Live, both games (`lead_tmp/audit-uord/ps2/bolt_b1.py` / `bolt_b2.py`): a bolt's line written through a stationary
  soldier's chest after its predictive ray ran stops one update later at the chest, BF1 15 / 15 and BF2 15 / 15 (14 /
  15 and 15 / 15 hurt); the line 2.5 m aside flies on, 14 / 15 and 13 / 15. In battle 15 BF1 and 18 BF2 bolts stopped
  at a soldier with life left whose first-row life ran 10-171 m past him, and 0 of 661 BF1 / 409 BF2 predictive kept
  hits belong to a soldier. The doc's [H] on the live split is now [M]: UORDS has its PS2 check.
- Fixed in the doc: IMP's torpedo ordnance authors neither Waver key (REP / ALL only); of the 1,400 replay pairs, 442
  BF1 pairs never moved and the 582 BF2 free pairs never turn, so the steering rests on 377 pairs and BF2's waver is
  unmeasured live; 580 (not 581) of 582 BF2 pairs fit; the mortar bursts ~6.8 m up with the code's steps; two
  wordings. Workflow `audit-uord-re` (22 agents): 6 reproductions, 6 replay mutations (sign, ramp, phase order,
  speed law and target height caught where the data exercises them; the sin-vs-angle clamp is below the noise), 10
  readers on 30 claims (50 verdicts, 46 SUPPORTED, 4 PARTLY on wording, none refuted; no second reader overturned
  the first).

## 2026-10-22 (loop lane 2, card GTHRW) — the grenade throw on the soldier, both games, third and first person → `analysis/re-grenade-throw-anim.md` [O]
- BF1: the grenade's fire entry `FUN_0028eb30` raises the start flag weapon+0xc0 at the PRESS (via vtable +0x50,
  `FUN_002859f8`), so the animator driver `FUN_002013a0` passes the fire state on in the press update; the soldier's
  in-hand weapon is the grenade (channel 1 active while its state is 1-4), its set 3, and strength 1.0 (every thermal /
  crack / haywire: `MaxPressedTime` 0) gives state 3 -> `human_grenade_stand_shoot3` (51 frames) on the upper player,
  on the legs too while he stands. The grenade leaves at weapon timer 0.300 = clip time 0.267; the channel's return to
  0 at the fire state's end changes the set and cuts the action; the hands are empty (HideOnFire).
- BF2: `OffhandWeapon` (0x2ecd946f, bit 55 of class+0x1e0; every thermal, mine, detpack, buff: 82 classes; BF1 has no
  such key) makes `FUN_00239bb0` keep the PRIMARY's set and map the fire state to the secondary actions
  (`FUN_00281630`): a thermal (variant 1) plays `human_rifle_stand_shoot_secondary2_full` (41 frames; underhand
  `shoot_secondary`, 31; bazooka / tool / pistol primaries their own upper clips); the fire entry 0x337678 raises bit 33
  at the press; release at 0.533 s = clip time 0.50; the rifle stays in the right hand.
- First person, both games: an 11-column clip table per weapon row (BF1 `_1st_<set>_<action>` in `human_animation_fp`,
  `FUN_001a6880` / `FUN_001a38e0`; BF2 `humanfp_*`, 0x618440, `FUN_001beeb8` / `FUN_001bfcb0`): a thermal plays
  `_1st_grenade_shoot2` / `humanfp_grenade_shoot2` (11 frames). The port has no first-person view (card FPV).
- Live PCSX2, both games (`lead_tmp/gthrw/ps2/throw_strip.py`, a row per update of the soldier, his animator and his
  thermal): BF1 3 of 3 and BF2 2 of 2 throws hold these clips at these updates, standing and running; picture strips
  `ps2/bf1_throw_sheet.png`, `bf2_throw_sheet.png`. Workflow `gthrw-re-verify` (7 agents): 24 claims 21 SUPPORTED + 3
  PARTLY (side conditions, folded in); 8 absence / twin claims to two verifiers each: 5 both SUPPORTED, 2 both PARTLY on
  wording (fixed), 1 split (the broad "no human prone throw clip anywhere" -> [H]).

## 2026-10-22 (loop lane 2, card SPRCL) — the clip a sprinting soldier plays: BF2's sprint pair and its clock, BF1's 8 m/s idle line → `analysis/re-sprint-clip.md` [O]
- The port's freeze is BF1's: BF1 has no sprint (no sprint state, no sprint id in its animator's tables at 0x517af0 /
  0x517be0, no sprint clip in any BF1 `zaa_` bank), the port's BF1 sprint is ours, and the port keeps BF1's chooser rule
  `FUN_0022e990`: over 8 m/s (`64.0 < vx^2 + vz^2`, B1:144989, tested before the root scale's division) the legs idle.
  A census of the port's own animator over every class at 1.5 x MaxSpeed: BF1 140 / 146 db rows and 100 / 104 fielded
  rows on `*_stand_idle_emote`, BF2 0 / 324 and 0 / 104.
- BF2: every set has a sprint pair (RAM: 0 of 2,777 set rows over 181 states empty; the pass-2 alias `sprint <-
  stand_runforward` fills the pistol, melee and spear sets). The pose driver `FUN_0027caf0` case 0 plays it on BOTH
  players at `|v| / v_clip` - the speed (unscaled) over the LOWER sprint clip's root speed, a plain ratio (B2:201048,
  0.01 from the pool at 0x6206c0; `FUN_0027d9a8` B2:201453 / 201479), not the locomotion law `k + (1 - k) 0.05`
  (B2:202349); a null lower slot would return before any pose (B2:201451). Live (BF2 Geonosis, `lead_tmp/sprcl/ps2`):
  118 sprint updates fit the plain ratio to a mean 0.00005 of phase per update, the eased law misses by 0.0005, the
  port's plain clock by 0.009; the run control fits the eased law to 0.00001.
- Knock-in (both rules in a scratch binary): census BF1 140 -> 0, BF2 0; the port replays the PS2's sprint rows to a
  mean 0.00004 (main 0.00902). Workflow `sprcl-re-verify` (7 agents): 25 verdicts on 20 claims, 17 SUPPORTED, 7 PARTLY
  (folded in), 1 NOT SUPPORTED (the first-cited 0.01 was another branch's; fixed); the paired absence verifiers agreed
  on every claim.

## 2026-10-22 (loop lane 1, card AITHK) — how often a PS2 soldier bot thinks: BF2's interval by level of detail and its "think now", BF1's round → `analysis/re-ai-think-rate.md` [O]
- BF2: at most 10 UnitControllers think per step from a time-ordered list (`FUN_002af948`); interval = agent factor
  (trooper 2.0, combat 0.4, others 1.0) x {4, 3, 2, 1, 0.25} s by a level 0-4 taken from the HUMANS only (distance 7 /
  20 m, the bot's view cone, the render mark, a camera-view raise; no human = 0). Re-checked from SAIR 1.1-1.3 as one
  rule. New: `FUN_002af840` pulls the next think to now + 0.0001 from five sites (five `jal` words); a threat entry for
  a HUMAN-controlled unit (`FUN_002b9d10` -> `FUN_002b0418` IsA PlayerController) does it at ANY level - seen, hit, or
  heard by the human's own noise pass every 15 steps; an order message or a finished helper only at levels 3-4.
- Live BF2 Kamino, every update (`lead_tmp/aithk/ps2/think_live.py`): of 52,049 bot update pairs with no think between,
  all 28 with the human entering / changing in the threat list pulled the think (each to now + 0.0001), 0 of 209 with
  only an AI entry; at levels 0-2 nothing else pulled one. A life's first think is at once with no order; the order
  comes one interval later (4.0 s with nobody playing, CONT's lives).
- BF1: a 15-step round (0.5 s), a far (45 m from the first camera), slow, unseen unit skipped on alternate rounds
  (1.0 s); no level by the human, no agent factor, no think-now twin; the same camera test skips a far soldier's whole
  update on alternate frames.
- The port thinks every bot every 10 ticks in both games. Build spec in section 8 (deterministic: `Sim::players` and
  `Unit::ideal_camera`; headless = level 0); it reverses bot-design D1. Workflow `aithk-re-verify` (6 agents): 23 claims,
  18 SUPPORTED, 5 PARTLY (folded in), 0 refuted; the paired absence verifiers agreed on all four.

## 2026-10-22 (loop lane 2, card VPOSE) — a vehicle's own posed nodes: the node table, its turret and aimer nodes, the FinAnimation nine-pose → `analysis/re-vehicle-pose.md` [O]
- Every vehicle has one node table (BF1 hover E+0x1d20, BF2 E+0x17e0; walkers and flyers their own), a hash map of
  node CRC -> matrix pointer that its MountedTurrets, its own weapons' aimers and its FinAnimation step write and its
  draw reads; it is never cleared (constructors only). Task order: drive + fin fill (key 0) -> pilot seated (15) ->
  turret tick (20 + HierarchyLevel) -> aimers -> the draw refreshes aimers and turrets again.
- RAM replays over 337 stored states (254 with vehicles) + 18 bike rides: every hover turret node = re-turret-draw
  2.2's formula (362 / 362 rows), every BF1 aimer node = 2.3's (BF2 within 0.01: aimers are written at draw time);
  the FinAnimation pose - frame a and frame b each sampled by slerp between neighbouring frames, blended by slerp at
  c, the LATER of two same-CRC tracks winning, composed in the bank's own zaf rig - fits 297 / 297 hovers to 0.001 deg
  (nearest frame, floor frame, the first duplicate track and nlerp all fail).
- The clips: tank skis 45 / winglets 95 deg; bike bars, foils, flaps, pedals and bone_root's 5 deg pitch; BARC body
  lean (its rider leans with it, in RAM); skiff rudders; snowspeeder flaps (a flyer law). Frame 4 = bind in every
  clip. Found on the way: the constructors zero p and y (only a, b, c start at -1) - the port's
  `HoverNine::START` (-1, -1) is wrong; re-vehicle-riders 4.1 corrected.
- Readers AGENT_A / AGENT_B + workflow `vpose-re-verify` (7 agents): 40 claims, first reading 23 SUPPORTED / 17 PARTLY /
  0 refuted; the 15 twin and absence claims read again separately 7 / 8 / 0, none overturned. Cards: VPOSE -> build;
  new VPOSW (walkers: the AT-ST's head and guns, value 4) and VPOSF (flyers, value 3).

## 2026-10-22 (loop lane 2, card VPOSW) — a walker's posed nodes: its node table, the walk pose under its aimers and turrets, the aimers' own frame → `analysis/re-walker-nodes.md` [O]
- A walker's table (BF1 E+0x1ec0, BF2 E+0x1d7c; command walkers built on the walker) points every walk bone at the walk
  pose's own matrix block, which the walker task recomposes every update; aimers and MountedTurrets keep their own
  entries (BF1 copies the pose into them once, BF2 re-points and the first tick / draw re-points back); the draw runs
  aimers -> turrets -> the model. VPOSE's walker failures were a misread: its table reader used the hover's offset.
- RAM, 357 states, 348 walkers: every aimer, turret and barrel node = the vehicle formulas with the parent read from
  the walker's table - BF1 1,205 / 1,206 rows to 1e-3 (the one miss a barrel mid-recoil), BF2 all within 0.04 (draw-time
  writes); the parent at its bind fits 0 of 1,332. Flyers read at their own tables fit 1,501 / 1,501 (VPOSE's MAF and
  ball-turret misfits were the same misread; card VPOSF).
- A world-based aimer steers in (class bind) x (parent's entry) x (world): B1 FUN_0026d748 B1:191823-191900, BF2's
  export hole 0x3021f8 (MIPS 0x302380-0x3024bc); fits every BF1 row to 0.000 deg (the base alone: 0.22 / 2.0 / 180 deg).
  BF1's AT-AT head is the BODY aimer's matrix written over TURRET1's (24 / 24; the turret's own formula 0 / 24). BF2's
  one-man AT-ST turns only aimer_x. Bone 0's translation is 0 in every walk pose and every DummyRoot track is z-only,
  so the port's rest DummyRoot is the PS2's; the bob is bone_root's own track ([H]: the port at the same clip time).
- Workflow `vposw-re-verify` (6 agents): 21 claims, 18 SUPPORTED / 3 PARTLY (folded in) / 0 refuted; the 5 twin and
  absence claims SUPPORTED by both separate readers. Card VPOSW -> build (rows: `lead_tmp/vposw/walker_nodes_ps2.txt`).

## 2026-10-22 (loop lane 1, AUDIT of re-grenade-throw-anim.md: PARTLY) — a held L1 shows BF1's reload clip; BF2's legs take the throw only from a still, not-kneeling start → `analysis/re-grenade-throw-anim.md` §10 [O]
- Live, both games, the auditor's own sampler (`lead_tmp/audit-gthrw/tools/tsamp.py` + `dec.py`, the animator read
  through the soldier's own pointer, clip names by a CRC checked against the game's `DAT_0057f638`): 20 trials, 22
  throws. The doc's clips, sets, players, release updates and fire-state lengths hold in every trial.
- BF1: `FUN_00205cc8` (B1:117106-117161) keeps channel 1 while its fire byte is held or just released (state != 8),
  and the grenade fires only on the press edge (vtable +0xa8 0x28ee20, an export hole); so L1 held past the fire
  state's end, or pressed during the reload, plays set 3's reload slot (`rifle_<stance>_reload`) on both players to
  the release + 1 update: 6 and 21 updates live, against the doc's "never seen". BF2 shows nothing of the kind.
- BF2: `FUN_0027dfe0` (B2:201825-201880) gives the legs the action's lower slot only in the action's first update,
  movement id 0 (still, and not crouched still for over 1 s, id 2); live: crouched 0.6 s -> the legs on the stand throw,
  crouched 1.7 / 1.8 s -> the kneel stays; started still then walked -> the legs leave at once; started running ->
  never. The keep-over-later-updates mechanism is [H] (both readers read one update; the rows show the clip).
- The upper player's phase after the release update is 9/51 = 0.300 s (BF1) and 16/40 = 0.533 s (BF2) in 22 of 22
  throws, one update later than the doc's 0.267 / 0.50 (the animator counter); which phase is drawn is [H].
- Fixed: the first-person table has 59 writes, not 66; BF2 has eight thermals, not nine; "82 offhand classes" is 22
  authoring / 66 resolving (with guided rockets and force powers); the fire update 0x3377d0 is vtable slot +0xac; an
  unreferenced stub writer of BF1 weapon+0xc0 at 0x288240 [receipt:283c29c9]. Workflow `audit-gthrw` (17 agents,
  medium): 9 reproductions (2 partly on counts, the rest yes), 6 readers (BF1 channel C1-C5, BF2 legs D1-D5, the doc's
  absence / twin A1-A6: none refuted, PARTLY on details, no second reader overturned the first), 2 independent replays
  with 8 mutations, all caught (BF1 replay 867 / 922 - every miss in the two held-L1 trials; BF2 360 / 360 with the
  measured legs rule). Card GTHRW (build) carries the build changes.

## 2026-10-22 (loop lane 2, card VSHUD) — the HUD in a seat: BF2 runs the soldier's HUD code on the seat's controllable → `analysis/re-seat-hud.md` [O]
- `FUN_003b61e0` hands every HUD publisher K = record +0x150, else +0x14c, else +0x148; live, a hover, a walker and a
  BUILDING turret all put the seat's controllable at +0x14c (+0x150 = 0). So the seat's weapons fill the weapon rows
  (the AT-ST two), the heat ring / refire bits / "OVERHEATED!" work as on foot, and the reticle group stays up while
  E's vt +0x88 holds (hover / building: alive; walker: a current weapon; a building turret's E is its owner).
- `FUN_0038e598` puts the reticle at (0.5, y) of K+0x150 + 1024 x K+0xe0 through the drawn camera; the switch to the
  soldier's aim (ControlsUnit, 0x44c77c95) is authored on neither disc. Replayed on 20 BF2 states: equal to the stored
  element to 0.000 lines (tower 240.0 = centre, bike 178.5 and AT-ST 166.1: the cameras look `TiltValue` 5 / 6 deg below
  the aim); BF1's twin `FUN_002d4b50` on 6 BF1 bike states, 6 of 6.
- `FUN_003b4bb8`: entering hides the soldier's bar and shows the green vehicle bar (a building turret: the owner
  building's health) and a 4 s seating chart (new card VSEAC); `FUN_003b5708`: the energy bar is the vehicle's (none for
  a turret). Live PCSX2 frames per seat kind with the gun overheating: `lead_tmp/vshud/ps2/ps2_seat_sheet.png`.
- Workflow `vshud-re-verify` (13 agents): 22 claims, 27 verdicts, 22 SUPPORTED / 5 PARTLY (folded in) / 0 refuted; the
  5 absence / twin claims to two readers each, none overturned. Card VSHUD -> build.

## 2026-10-22 (loop lane 1, AUDIT of re-sprint-clip.md: PARTLY) — BF2's sprint turns the root toward the travel; droid sprints divide by the engine's 11.5 m/s fallback → `analysis/re-sprint-clip.md` §7 [O]
- Held, reproduced: BF1's 8 m/s idle line before the scale; BF1 has no sprint (two readers; BF1 holds none of BF2's
  sprint class keys [receipt:141fe3cb]; an independent MINA parse of every BF1 bank, 413 clips, none a sprint); case
  0's law; the RAM census (0 / 2,777); the port census on main 9193a4c byte-equal to the session's, the knock-in and
  its null, the replay 0.00902 -> 0.00004.
- Live, the auditor's own sampler (GTHRW's audit `tsamp.py`, decoder `audit-sprcl/tools/sdec.py`), BF2 Coruscant, 7
  trials: phase step = |m_xz| / 9.16 x 30 / (F - 1) on both players, mean error 0.000000 over 412 pairs each,
  including a pistol sprint whose upper clip differs (its own speed misses by 0.0215) and diagonal travel (the forward
  component misses by 0.006). The divisor is the slot's whole cm/s (9.16, not the bake's 9.1627).
- New: case 0 hands atan2f(x, z) of the local move to `FUN_0027d9a8`, which eases +0x20cc toward it by min(1, 7.5 dt)
  and yaws `dummyroot` with it (`FUN_00280048`): the sprinter's root turns toward the travel (412 live pairs fit to
  2e-7 deg; +-14.04 deg with the stick at 45 deg). The port holds the root straight in a sprint.
- New: 11.50 m/s on the droid / Aayla sprint slots is `FUN_00282900`'s id-0 fallback for a forward root speed <= 0
  (B2:204683-204695, `lui 0x4138` at 0x282a18); the port bakes those clips with zero root motion, so the doc's "1.0
  when none is baked" is wrong for droids (the PS2: |v| / 11.5).
- Fixed wording: 22 uncovered BF2 heroes, not 9; 31 slots / 30 names; "sprint in no decomp line" holds for whole
  names only; the CRC-word absence has no positive control; the ramp is the class's authored 0.35 s. Workflows
  `audit-sprcl-re` (13 agents: 6 reproductions, 43 rows 32 yes / 10 partly / 1 no - the hero count; 6 readers,
  40 verdicts 32 SUPPORTED / 8 PARTLY / 0 refuted, the paired absence readers agreed; 1 replay with 9 mutations, 6
  caught, 2 untestable on the session's flat rows, 1 equivalent there - the upper clip's speed, caught on the
  auditor's pistol rows) and `audit-sprcl-verify2` (2 separate readers of 9 new claims: 18 verdicts, 10 SUPPORTED /
  8 PARTLY on scope or line range, none refuted). Card SPRCL (build) carries the build changes.


## 2026-10-24 — BLEED: retain ticket-shortfall repair [O] UNAUDITED

See [re-bleed-reachability.md](re-bleed-reachability.md): current Rust CP bleed
preserves the live ceiling at 30 Hz, including Elite; a legal mission extra
death drain reaches `tick_bleed` repair with zero bleed. Keep the loop and its
Sim despawn consumer. Runtime admission/death/step regression and clamp matrix;
this is a port control-flow result, with original-game evidence unchanged.

## 2026-10-25 — BFC scope reconciliation; PS2 research incomplete

[re-material-flags.md §5.1](re-material-flags.md#51-bfc-scope-reconciliation-2026-10-25-research-incomplete)
records the current pipeline/strip/flags path and pose camera default. The
earlier VU1 reading remains provisional; the cause at the BFC poses stays [H].
Separate the occluding draw, near-plane difference and facing/ADC rule before
writing a culling build specification. No fresh [O] PS2 finding.
