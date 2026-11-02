# RE_LOG — reverse-engineering lab notebook

Pruned 2026-11-02: an entry whose addresses, numbers and names all appear in one dedicated note now points to that note. The full notebook is in git: `git log -1 --until=2026-11-02T00:00 -- analysis/RE_LOG.md` names its last full version.

This file holds findings: subsystem, source function addresses, what we learned, open questions. Addresses are EE virtual
addresses as they appear in the Ghidra decomps (`analysis/SWBFRONT.ELF-decomp.c` = BF1, `analysis/SWBFRNT2.ELF-decomp.c` = BF2).

## Conventions
- Named functions come from the in-binary luaL_reg-style binding tables
  (`analysis/bf1-bindings.txt` 483 fns, `analysis/bf2-bindings.txt` 769 fns),
  applied in `ghidra-proj/{bf1,bf2}`.
- Cite every ported constant/algorithm in Rust code with its source address.

(Moved: the rest of this section's facts are in bot-quality.md, plan/OPTIMISATION_AND_EXECUTION.md.)

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
- BF2 MISSION.LVL: `mcfg` chunks; NAME payload is a 4-byte hash, not a string.

(Moved: the rest of this section's facts are in a2-bake-formats.md.)

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

(Moved: the rest of this section's facts are in re-ai-difficulty.md.)

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

(Moved: the rest of this section's facts are in a2-bake-formats.md, re-fog.md, re-sky-lighting.md.)

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
- `yav_main_1`: BF1 and BF2 YAV1 ship the *same* lightmap byte for byte (mean
  103, p50 102, p99 165, max 255) and classify the same way. Consistent.
- The threshold has room: p99 is ≤ 174 on every class-128 terrain and ≥ 230 on
  every class-255 one, so 204 sits in a 56-wide gap.
- Still NOT established, still HOLD FOR FABLE: what the PS2 itself multiplies
  here (ambient, sun term, or a fixed 2× in the terrain pipeline).

(Moved: the rest of this section's facts are in re-terrain-lighting.md.)

### CORRECTION — `LOWR` lives under `tern`'s `PCHS` (2026-08-31, Opus session 3)
The 2026-08-30 caution said a `tern`-children walk "reported neither LOWR nor
LPTC while grep finds them in the same files, so that iterator is dropping
children somewhere — do not trust it as a census". **`Chunk::children()` drops
nothing.** `LOWR` is a child of `tern`'s **`PCHS`**, one level below where the
census looked. Pinned as a test
(`mungefs` `tat1_lvl::lowr_lives_under_terns_pchs_not_under_tern`).

Measured shape, NOT parsed and NOT ported (still HOLD FOR FABLE — what the PS2
draws outside the level):
- `INFO` = u32 grid, u32 resolution, f32 extent, f32 min y, f32 max y. The
  `LPTC` count is exactly grid² on all 17 maps that carry one (64, 100, 121,
  196, 256, 324), and the second u32 matches that map's
  `sky_ DomeInfo.LowResTerrain.PatchResolution` (TAT1: 5 and 5).
- `LPTC` payloads are NOT fixed size (TAT1 404/480/540 B, HOT1 fifteen distinct
  sizes), so they are compressed or variable-length per patch. Undecoded.

**This does NOT close the outside-the-level defect, and the census says why:
17 of 34 maps have no `LOWR` at all — including BF1 END1 and BF1 YAV1, which
are two of the maps that SHOW the defect.** So whatever the forest maps draw
out there, on those two it is not `LOWR`.

Per-map: LOWR on BF1 BES2/GEO1/HOT1/KAS1/KAS2/NAB1/NAB2/TAT1/YAV2 and BF2
DAG1/END1/GEO1/HOT1/KAS2/NAB2/POL1; absent on BF1 BES1/END1/KAM1/RHN1/RHN2/
TAT2/YAV1 and BF2 COR1/DEA1/FEL1/KAM1/MUS1/MYG1/TAN1/TAT2/TAT3/UTA1/YAV1.

(Moved: the rest of this section's facts are in a2-bake-formats.md, re-fog.md.)

### Per-map sky, sun, ambient and fog (2026-08-31, Opus session 3) — DONE
Full write-up: **`analysis/re-sky-lighting.md`**. Headlines:
- Every map LVL on both discs carries one `sky_`; only BF2 carries `lght`.
  BF1 keeps its sun and ambient in `sky_ SunInfo`/`SkyInfo`; BF2 moved them to
  `lght GlobalLights` + the `Light` block it names. BF2 authors NO
  `AmbientColor` on any of its 18 maps; BF1 authors one on all 16.
- **ERRATUM to the config format:** a string argument's slot is
  `4 * argc + byte offset into the trailing blob`, not the constant 4. The old
  reading dropped every string after the first. Fixed in `mungefs::config`
  with a byte-verified regression on BF2 MUS1's `Effect(...)`.
- The reduction to one sun + hemisphere ambient + fog (`bake::lighting`) is
  OURS and PROVISIONAL, including the colour-space choice (authored 0..255 are
  gamma and are stored linearised, same rule the `_dtl` GPU test proved), the
  three heuristics deciding which maps get fog, and an ambient floor.

(Moved: the rest of this section's facts are in re-fog.md, re-sky-lighting.md.)

### Blaster fire + damage (2026-08-30, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-blaster-damage.md (fire cycle, spread formula,
  damage chain, full ODF key/hash/offset tables, worked rifle examples both
  games, 8 test vectors, 9 open questions).
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

(Moved: the rest of this section's facts are in bot-design.md, re-blaster-damage.md.)

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

(Moved: the rest of this section's facts are in re-cp-capture.md.)

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
- Placeholder (NOT fidelity, PLAN deferred item #1): core::ai — nearest
  non-friendly post + engage nearest visible enemy, bounded 2.5° aim wobble.

(Moved: the rest of this section's facts are in audio-census.md, re-soldier-ai.md.)

### Tickets / reinforcements / spawn reserve (2026-08-30, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-tickets-spawn.md (struct layouts, function tables,
  mission constants, 12 reference vectors, 7 open questions).
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

(Moved: the rest of this section's facts are in re-command-walker-post.md.)

### Animation banks + pinned clip durations (2026-08-30, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-anim-banks.md. zaa_ = NAME + BIN_ (SMNA header,
  version 8 + u16 animCount → MINA entries + TNJA/TADA). MINA entry = 16 B:
  {u64 boneFlagMask, u32 nameCrc, u16 frames, u16 bones}.
- Clip-name hash is NOT FNV: engine CRC-32, MSB-first poly 0x04C11DB7,
  init −1, final ~, over the lowercased name (BF1 FUN_00444bb8, table
  0x521ca8). BF1 special ids resolve via sprintf "human_rifle_%s".

(Moved: the rest of this section's facts are in re-anim-banks.md.)

### Stance machines + rolls, both games (2026-08-30, P3, RE subagent + lead-verified) — DONE
- Full spec: analysis/re-stance-roll.md (state enums, function maps, ODF
  key/hash/default tables, BF1-vs-BF2 roll diff, 7 reference vectors).
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

(Moved: the rest of this section's facts are in re-stance-roll.md.)

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

(Moved: the rest of this section's facts are in re-aim-assist.md.)

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
- Lead-verified: independent Rust ADPCM decoder (bake/src/adpcm.rs)
  reproduces frame count, near-full-scale peak, and 440 Hz dominance
  (Goertzel >50× vs 300/600 Hz) in Tier-1 tests; `bake beep` emits WAV+OGG
  (native + both as-played renders), ffprobe-verified.
- Also mapped for later: SOUND/SHELL.LVL StreamList (shell_vo 181 segments,
  shell_music 6), CORE.LVL global bank's other 5 UI waves.

(Moved: the rest of this section's facts are in a2-bake-formats.md, re-sound-banks.md, re-stance-roll.md.)

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

(Moved: the rest of this section's facts are in a6-mission-lua.md.)

### Call-graph reachability (2026-08-30, audit A4) — DONE, verified
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

(Moved: the rest of this section's facts are in a4-callgraph.md.)

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
- No prone/stance bindings in either game (stances are engine-side, not Lua).

(Moved: the rest of this section's facts are in a3-subsystem-map.md.)

### Timing / frame pacing (2026-08-30, audit A5) — DONE, verified
- Lead-verified spot checks: BF1 vsync loop + `DAT_005190c0 = 2` default,
  BF2 0.1 clamp in FUN_00174510, `DAT_00661308 = 0x11940000`. All confirmed.
- Engine decision: fixed 30 Hz tick, dt = 1/30 exactly, accumulator capped at
  0.1 s (3 ticks) → `engine/crates/core/src/lib.rs` (TICK_HZ/TICK_DT/
  MAX_FRAME_ACCUM_SECS). Never scale dt.
- Open (PCSX2 rig later): sub-Update substepping, retail interval-1 paths,
  dt-arg order confirmation, dt distribution in heavy scenes.

(Moved: the rest of this section's facts are in a5-tickrate.md.)

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
- Ported: `mungefs::soundbank::Sample::alias_of`, aliases resolved to the
  target window; Tier-1 `tat_sound_lvl_aliases_resolve_and_fit` (bank
  64267af8: 402 records, 172 aliases).

(Moved: the rest of this section's facts are in re-sound-global.md.)

### Cheat codes (2026-08-30) — lead-verified, ported (BF2 Morse; BF1 none)
Source: `analysis/re-cheat-codes.md` (fable subagent). Lead checks against the
raw BF2 ELF (`off = va - 0x160000 + 0x1000`) and the decomp:
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
- Ported: `ui::menu::MorseCheats` replicates the matcher including both buffer
  wraps (5 unit tests: all 7 words, silent reset, dropped letter, 6-symbol
  wrap → 't', priority + persistence). Game: sos→invincible, ammo→infinite
  ammo, hud→HUD hidden, slow→positional SFX pitch ×0.3 (`SoundLib::rate_mul`),
  80s/at/alias logged no-ops (no soldier LODs / melee / post-FX yet).
  Confirmation is a HUD-feed line; `shell_menu_accept` was NOT found by hash in
  BF2 SOUND/SHELL.LVL (open — the UI sound may live elsewhere).

(Moved: the rest of this section's facts are in re-cheat-codes.md.)

### Animation banks: clip directory ported (2026-08-30) — DONE
- Quirk: the MINA/TNJA/TADA section lengths are rounded up to 4 but the
  enclosing `BIN_` length is not — BF2 INGAME.LVL human_1 TADA says 28844
  while 28841 real bytes remain and BIN_ ends 1 byte into the padding; clamp
  sections to the parent and slice TADA to the header's byte count.

(Moved: the rest of this section's facts are in re-vehicle-pose.md.)

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

(Moved: the rest of this section's facts are in re-soldier-skeleton-skins.md.)

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
- [V] **Correction to "Sky dome placement" §5:** under the sky flag, material bit 0x02 BLENDS (ALPHA 0x44, TEST
  0x51001) — it is not alpha-tested; `mus1_sky_volcano` and `myg1_sky_mountains_01` composite on the PS2. BF1's
  sky variant blends AND tests at AREF 0x40 (`FUN_004d0108` 594636–594648) but no BF1 shell carries 0x02.
- openbf: "shells scaled to 1000 m about the eye, depth off, drawn first" reproduces the PS2 angles and ordering;
  `SkyMode::Mask` is replaced by Blend (built on the `sky-wrap` branch). Not reproduced, recorded: the far target
  clears to BLACK where no shell covers (the port clears to the horizon/fog colour — PROVISIONAL, ours); the
  80–110 m band is a per-pixel depth blend of near scene into far target (the port fades per object); the PS2
  sky is a 16-bit bilinear-magnified buffer (softer). Not closed: the single-view far-target height, the VU1
  microprogram, `FUN_00581a98`'s second fill pass, sky_ key 0xc15d0860, BF1's exact frame order.

(Moved: the rest of this section's facts are in re-sky-pass.md.)

### The `segm MTRL` flag word → GS state: the general model draw path, both games (2026-09-03, Fable session 12) — lead-verified, port NOT changed (decision pending)
Full doc: `analysis/re-material-flags.md` (Fable RE agent; the lead re-read the lines listed in its header).
Answers HOLD 10(g) at the CPU-visible level; raised by the maintainer's decision 3 (reconcile before touching ALPHA RULE v2).
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

(Moved: the rest of this section's facts are in re-material-flags.md, re-water-colour.md.)

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

(Moved: the rest of this section's facts are in re-fog.md.)

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
- Open [H]: whether the PS2 pad delivers the +0x2b4 edge to a droideka-driven Controllable (same field as the
  soldier's; PCSX2 closes it). Not built: the droideka is not a roster class (`is_soldier_chain` rejects
  `walkerdroid`); the PLAN bullet's attribution must flip either way — brought to the maintainer (PLAN Log 2026-09-07).

(Moved: the rest of this section's facts are in re-droideka-unit.md, re-droideka.md.)

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
- [V] **HOLD 9 answered:** `shell_menu_accept` (0x6a942216) is a `soundproperties` in BF2 `lvl/DATA/_LVL_PS2/
  CORE.LVL`'s `snd_` @0x3800 (hash at 0x8029; pitch 1.1, gain 1.0, bus `shellfx`, priority 0.5; `shell_menu_enter`
  inherits it), sample B75D27CD (11,060 Hz, 2,704 B) in CORE.LVL's 7-sample `global` bank @0x10 (record @0x78).
  The hold was a search-SCOPE miss (BF2's CORE.LVL is not under `sound/`); `re-sound-banks.md` (2026-08-30) had
  already named B75D27CD the "accept wave" for BF1.

(Moved: the rest of this section's facts are in re-sound-global.md.)

### The AI aim-error floor is a FLOAT, and the AI sniper distance table's keys and predicate, both games (2026-09-08, Fable session 14) — lead-verified, port in Phase C commit 3 (HOLD 19(e) and 19(g) ANSWERED)
Full doc: `analysis/re-ai-sniper.md` (Fable RE agent, session 14; the raw ELF words, the decomp lines, the key hashes
and the disc ODF values re-read by the lead — header of the doc).
- [V] **The sniper table**, identical in both games (BF2 `FUN_0032e4b8` B2:323339–323346 / BF1 `FUN_002889e8`
  B1:211247–211254). Predicate: class `YawSpread == 0` AND fire-point owner index < 0 (an AI shooter) AND
  `TriggerSingle` AND `ZoomMax >= 6.0` (inclusive — `imp_weap_inf_sniperrifle` is exactly 6.0) AND a current target.
  Then a step table on the SQUARED distance, strict `<`, no interpolation: 400 / 1600 / 3600 / 6400 / 10000 / 14400
  → 0.0997 / 0.0499 / 0.0333 / 0.025 / 0.02 / 0.0167, else 0.0143 rad, × weapon+0x138 (BF2) / +0x158 (BF1), ADDED in
  the `YawSpread` slot: yaw = `max(s,0) + postureBase + table×mult` (`max(s,0)` NOT replaced; BD §4.3 corrected).
- [V] Keys: `ZoomMax` 0x2a96e8b4 (loader coerces 0 → 1.0, ctor default 1.0), `ZoomMin` 0x1881b1a6 / `ZoomRate`
  0x9573fb20 loaded beside it but unread by the table, `TriggerSingle` 0xed9d56db; the disc values for the four
  roster sniper primaries + two rifle controls are in `re-ai-sniper.md` §3.

(Moved: the rest of this section's facts are in re-ai-sniper.md.)

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

(Moved: the rest of this section's facts are in re-water.md.)

### HUD bitmaps composite by CLUT alpha — source-alpha blend, no alpha test, and the loader scales CLUT alpha ×129/256 (2026-09-09, Fable session 17) — lead-verified → `analysis/re-hud-icons.md`
- [V] The disc icons ARE cutouts (`squad_cmd_highlight` 1,594/4,096 texels alpha 0); the BAKE forced them opaque — `bake_content`
  (map.rs 958–1021) kept alpha only for textures a MASK/BLEND model draw names; 211 of BF1 COMMON.LVL's 224 alpha-bearing sheets
  baked opaque, `common_bf2` likewise. The loop's "alpha 255 everywhere" measured the bake, not the disc.
  **FIXED 2026-09-17 for the COMMON bakes by queue item SM (`f21c2db`)** — the shipped `squad_cmd_highlight.ktx2` now carries
  1,594 of 4,096 texels at alpha 0 and `squad_cmd_cross.ktx2` 7,732 of 16,384, re-read off the KTX2 by the landing session;
  a MAP sheet still bakes opaque (queue **WA**). Detail in `re-hud-icons.md` under this same claim.

(Moved: the rest of this section's facts are in re-hud-icons.md.)

### Backface culling is VU1-side, per material record, and OFF for plain opaque segments (2026-09-09, Fable session 17) — GEO1's wreck is NOT a material bug → `analysis/re-vu1-cull.md`
Full note: `analysis/re-vu1-cull.md` (VU1 dumps + decoder under `lead_tmp/s17/re/vu/`). **Lead-verified on the CPU side only:**
the EE's `f&0x2` → TEST 0x5060b agrees with `re-material-flags.md` §2's [V], and the port's one `cull_mode: None` pipeline
(`render/lib.rs:1496`) already matches the two-sided result. **The VU1 microcode reading below is the AGENT's alone** — the
lead did not re-read 0x800 / 0x800000, the facing loop or the ADC bit. Answers HOLD 10(g)'s two-sided half.
Full note: `engine/artifacts/lead_tmp/s17/re/geo1_wreck.md` (VU1 dumps + decoder under `re/vu/`). Answers HOLD 10(g)'s two-sided half
and re-reads `re-material-flags.md` §2's [H] on 0x800800/0x10000 as [V].
- Ported: nothing to port — no ALPHA RULE v3 and no cull item. Open: the PS2's `modl` LOD switch rule (HOLD FOR FABLE 20;
  `re-vu1-cull.md` §4 has the two-station PCSX2 capture note that settles the wreck).

(Moved: the rest of this section's facts are in re-vu1-cull.md.)

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
- **The port's floor under the water-side props** (queue item WL, `52d0ff5`): the level AND the floor are the disc's; YAV1's
  bazaar is an authored flooded plaza; the 18 flagged prop rows are pivots by design. HOLD 19(c)'s "level vs the authored
  floors" clause is CLOSED. Report: `engine/artifacts/loop/reports/20260909_1638_water-floor-check.md`.
- **The BF2 maps with no far layer** (queue item 6, `19148e6`): the far scene now dissolves into the sky colour instead of
  ending at a fog wall, per `analysis/re-fog.md` §2.5/§6. Report:
  `engine/artifacts/loop/reports/20260909_1522_bf2-far-fade.md`.
- Ported: all three, on main 2026-09-09 evening (11 map goldens regenerated). Open: unchanged — HOLD 21 (the water VU1
  microcode and KAM1's FFT ocean) and the look items W2/W3.

(Moved: the rest of this section's facts are in re-water.md.)

### Water tint scale verified; the water pass, the class db and the spawn rule landed (2026-09-09 night, session 19) — one lead re-read, then pointers

ONE new PS2 fact, re-read in the ELF by the lead, plus the port work that followed it.
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

(Moved: the rest of this section's facts are in re-water.md.)

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
- Ported: `mungefs::water::Water::has_patch` + `bake::water_glb`'s lattice, main 2026-09-17 (`maps` rebaked, one golden
  regenerated). Open: unchanged — HOLD 21 (the per-vertex VU1 displacement) and queue **WA** (does the water composite by its
  TEXTURE alpha).

(Moved: the rest of this section's facts are in re-water.md.)

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

(Moved: the rest of this section's facts are in re-water-colour.md.)

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

(Moved: the rest of this section's facts are in re-bf2-doors.md.)

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

Census: BF1 179 records (class 0x20), BF2 469 (bit 0x80; none with 0x100 or 0x02; 22 with 0x04). Every
load-bearing claim is `[O]`: one Opus pass plus one Opus verifier agent (23 of 30 claims SUPPORTED, 7 PARTLY —
folded in — none NOT SUPPORTED), no Fable audit. It also found that the s12/s17 VU1 decoder mislabels every
A = 0x100·k instruction and swaps `SQ` operands (the fresh decoder: `scripts/tools/vu1dis.py`). Ported:
nothing yet; queue **SPEC** (the world pass) and **SPEC2** (soldiers and weapons, skinned).

(Moved: the rest of this section's facts are in re-specular-pass.md.)

## 2026-09-25 (loop lane 1, queue WSE's [RE] half) — BF2's `SpecularEnable` and `OscillationEnable` default to 1 → `analysis/re-water-flags.md`

(Moved: the rest of this section's facts are in re-water-flags.md.)

## 2026-09-25 (loop lane 1, queue WCOL's [RE] half) — the flat water's colour is a VU1 overlay with a view-angle term → `analysis/re-water-colour.md`

(Moved: the rest of this section's facts are in re-water-colour.md.)

## 2026-09-25 (loop lane 1, queue LITE's [RE]) — why the port is brighter than the PS2: per-vertex COL0, prelit models, Static lights → `analysis/re-model-lighting.md`

(Moved: the rest of this section's facts are in re-model-lighting.md.)

## 2026-09-25 (loop lane 2, card VEH0) — vehicles: the census, the class code in both ELFs, the M2 plan → `analysis/re-vehicles.md`

(Moved: the rest of this section's facts are in re-vehicles.md.)

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

(Moved: the rest of this section's facts are in re-vehicles.md.)

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

(Moved: the rest of this section's facts are in re-sun-direction.md.)

## 2026-09-26 (loop lane 2, AUDIT of SUND) — OK → banner in `analysis/re-sun-direction.md`

(Moved: the rest of this section's facts are in re-sun-direction.md.)

## 2026-09-26 (loop lane 2, card HVRE) — the hover integrator, transcribed (BF2) → `analysis/re-hover.md`

(Moved: the rest of this section's facts are in re-hover.md.)

## 2026-09-28 (loop lane 2, card KASF) — BF1 KAS2's red fog is real and live but never shows; the port's red band is its clear colour → `analysis/re-fog.md` §9
- The disc record is `LowResTerrain/FogColor(255, 0, 0, 128)` at KAS2.LVL 0x18f368, the same in KAS2_H.

(Moved: the rest of this section's facts are in re-fog.md.)

## 2026-09-28 (loop lane 1, AUDIT of HOVR) — PARTLY → banner in `analysis/re-hover.md`, card HCON

One foreground Opus verifier: 20 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED (`engine/artifacts/lead_tmp/audit-hovr/verify/`).

(Moved: the rest of this section's facts are in re-hover.md.)

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

- A soldier's bounds are its model box: ctr ≈ feet + 1.05 m, r = 1.39-1.70 m.

(Moved: the rest of this section's facts are in re-aim-assist.md.)

## 2026-09-28 (loop lane 2, card FOLGL) — the foliage draw flags: 0x80 is the static light set; FEL1's black leaves were the pre-SUND sun → `analysis/re-foliage.md` §16, `analysis/re-model-lighting.md` §12
- **0x80 = "static"** (every foliage model: no disc authors `Lighting("dynamic")`).
  - BF2: `FUN_0057cf60` → `FUN_00597e98` argument 3 skips only the light-region lookup `FUN_0059b2f8`. It is not the prelit
    rule, which keys on the model's 0x200.
  - BF1: `FUN_00493cf0` gives static draws `AmbientColor` and the full sun.
- **Side finding (PS2 patched states, BF1 GEO1).** BF1 soldiers take SkyInfo `CharacterAmbientColor` (`DAT_0069c578`);
  static world props take `AmbientColor`. The port lights soldiers with `AmbientColor` → card CAMB. The light regions
  (BF1 `DAT_0069c548`, BF2 `FUN_0059b2f8`) that non-static models get are absent from the port → card LREG.

(Moved: the rest of this section's facts are in re-droid-death.md, re-foliage.md.)

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
- **Falloff** (B1 `FUN_00221530`, B2 `FUN_00268fe0`): `d' = d − w/2` to the victim's collision sphere; 1 inside
  Inner, linear to 0 at Outer. `DamageRadius` is Outer and Inner defaults to 0. Damage × f then goes through the bolt's
  TakeDamage: HealthType scale, friendly-fire scale (ELF 1.0), and AI never hurts a human teammate.

(Moved: the rest of this section's facts are in re-vehicle-death.md.)

## 2026-09-29 (loop lane 2, card REDB) — the PS2's fog amount is a `fogspread` TEXTURE lookup at planar depth, blended on the stored bytes; BF1 KAS2's red band is the port's fog arithmetic → `analysis/re-fog-amount.md`
- The sheet's live GS words are the "debug" branch (`DAT_00522a4c/50/54` = 1): DATE on, VU cull (`re-lowr.md` §2.2
  corrected).

(Moved: the rest of this section's facts are in bot-quality.md, re-fog-amount.md.)

## 2026-09-29 (loop lane 2, card REFL) — `Reflect` sky shells: two mirrored copies, one per render target; the sky under BF1 Kashyyyk's water → `analysis/re-sky-reflect.md`
- REFL → build (goldens bf1_kas1/bf1_kas2, rebake maps). Notes on TMSK and FARC. New card STARS: `DomeInfo Stars()` on
  BF1 KAS1/RHN2 and BF2 COR1/NAB2, not drawn by the port.

(Moved: the rest of this section's facts are in re-sky-reflect.md.)

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
- Two sessions, two foreground Opus verifiers:
  - C1–C16: 11 SUPPORTED, 5 PARTLY;
  - F1–F10: 8 SUPPORTED, 2 PARTLY. F6's wording; F7: the camera script took a bystander as the player.

  All corrections are folded. The second session also corrected six first-draft statements (doc §5.2).

(Moved: the rest of this section's facts are in unit-audit-bf1.md.)

## 2026-09-29 (loop lane 2, card UAUD2) — BF2 unit fidelity audit: every conquest class against the disc, the PS2 and the port → `analysis/unit-audit-bf2.md`
- **Method.** 33 `*_con` scripts from the PS2 bytecode; INGAME.LVL and the side-LVL sublevels they read, each class
  resolved the PS2's way; EE RAM of the BF2 save states (class objects and live soldiers); the PCSX2 class-select
  screens of all four factions; the port's own roster over its 31 BF2 RONs. Tools in
  `engine/artifacts/lead_tmp/unit-audit/` (`*2.py`).
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
- **Verifier:** 9 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED; every correction reproduced and folded in (§5).

(Moved: the rest of this section's facts are in re-foliage.md, unit-audit-bf2.md.)

## 2026-09-29 (loop lane 2, card VWLK) — the walker, transcribed (BF2) → `analysis/re-walker.md`

- **The disc's clips**, decoded with `mungefs::anim`: the AT-ST's walk covers 4.538 m in 0.667 s (6.806 m/s). So BF2's
  AT-ST walks 5.0 m/s and boosts to 10. BF1 plays the clip at the real dt, 6.8 m/s.

(Moved: the rest of this section's facts are in re-walker.md.)

## 2026-09-29 (loop lane 2, card UTAS) — the far scene draws a model's LOWD: UTA1's sinkhole far wall → `analysis/re-far-lowd.md`

(Moved: the rest of this section's facts are in re-ai-difficulty.md, re-far-lowd.md.)

## 2026-09-29 (loop lane 2, card JWAL) — soldiers through walls: the wall pass's floor exemption → `analysis/re-soldier-walls.md`
`[O] UNAUDITED`. The maintainer's KAS2 jump-in is at City Hall. `kas1_bldg_hut_grand` stands on four SOLID `prim` cylinders
(r 9.25, y 13.0–17.0), with bamboo walls and a plank ceiling; the inside is terrain grass.
- **The port (M1), reproduced to the tick.** The wall pass pushes the stepped capsule (feet + 0.6) and skips a
  contact with |n.y| ≥ 0.5 as a floor. A jump lifts the lower sphere over the rim, so at tick 42 the rim normal is
  (0.82, 0.57): skipped. The ground probe (feet + 0.6) never reaches the top. The soldier falls inside onto the
  terrain.

(Moved: the rest of this section's facts are in re-ai-difficulty.md, re-soldier-walls.md.)

## 2026-09-29 (loop lane 2, card DSHN) — chrome droids: the specular pass's gloss mask → `analysis/re-spec-gloss.md`

(Moved: the rest of this section's facts are in re-kamino-decks.md, re-spec-gloss.md.)

## 2026-09-29 (loop lane 2, card LEAF) — leaf patches: the sprite clusters on trees' hardpoints → `analysis/re-leafpatch.md`

(Moved: the rest of this section's facts are in re-leafpatch.md.)

## 2026-09-30 (loop lane 2, card FXP) — particle effects: the `fx__ ParticleEmitter` format, runtime, draw and spawn keys → `analysis/re-fx.md`

(Moved: the rest of this section's facts are in re-fx.md.)

## 2026-09-30 (loop lane 2, card FXB) — laser bolts: the core and glow ribbons, the bolt's light, HOT1's pink wash → `analysis/re-bolt.md`

(Moved: the rest of this section's facts are in re-bolt.md.)

## 2026-09-30 (loop lane 2, card FXMF) — the muzzle flash: a tinted additive model at the fire point → `analysis/re-muzzle-flash.md`

(Moved: the rest of this section's facts are in re-muzzle-flash.md.)

## 2026-09-30 (loop lane 3, card GLOW) — self-lit segments, the HDR bloom and the DomeInfo Filter → `analysis/re-glow.md`

(Moved: the rest of this section's facts are in re-glow.md.)

## 2026-09-30 (loop lane 3, card BHUD) — BF2's HUD: its layout, art, fonts and the PS2's screen mapping → `analysis/re-hud.md`
- **How.** One perspective camera per viewport (60° × 46.8°, a 4:3 frustum); the HUD root scales by 1.23 / 1.3 and pushes
  to 1.24·D: a Viewport fraction lands at display x = −0.0059 + 1.0139·fx, y = −0.0242 + 1.0484·fy; a native px or glyph
  texel = H/378.7 × H/427.3; a HUD mesh unit = 0.2129 H × 0.1887 H. Checked with no fitted parameter on the HOT1 frame
  (bars and counts ±1 px, the reticle 76 × 70 vs 76.2 × 73.4). Z 0 in front; fader in / hold (default forever) / sustain /
  out; colour eases to Color and pulses to ColorChange; Shadow = a (+1, +1) black copy at half alpha.

(Moved: the rest of this section's facts are in re-hud.md.)

## 2026-09-30 (loop lane 2, card BMAP) — BF2's minimap and world markers: the Map and Target elements → `analysis/re-hud-map.md`
- **Checked on the PS2.** Predicted from each state's own RAM, 263 of 264 unoccluded post icons in 70 walkabout states
  (16 maps) within ±3 px (mean ~1 px); the backdrop image best at the model's scale on 8 of 10 maps (NCC 0.70–0.94);
  the Large view (two new captures) = the HUD mapping × 0.948 about the screen centre (±1.7 px; cause [H]).

(Moved: the rest of this section's facts are in re-hud-map.md.)

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

(Moved: the rest of this section's facts are in re-round-stalls.md.)

## 2026-09-30 (loop lane 3, card BPNT) — BF2's local lights: the sources, the per-draw gather, the VU1 term → `analysis/re-point-lights.md`

(Moved: the rest of this section's facts are in re-point-lights.md.)

## 2026-09-30 (loop lane 2, card STHOP) — how a soldier follows the ground: ramps, crests, edges → `analysis/re-soldier-ground.md`
- **PCSX2, BF2 KAM1** (live RAM, every update; `lead_tmp/sthop/kam1_{ramp,edge,air}.csv`):
  - on the card's 34.3° ramp the player is grounded on every planar-facet update (7.0 m/s along the plane), and
    loses contact for one update at the facet folds (drop ≤ 0.03 m);
  - he flies 21 updates off the rim he climbed at vy +2.1, still in stand;
  - stand → 8 exactly one update after the timer reads 1.000.

(Moved: the rest of this section's facts are in re-aim-assist.md, re-soldier-ground.md.)

## 2026-09-30 (loop lane 3, card KAMR) — Kamino bring-up: the sky, the sea, the floors, the far pieces → `analysis/re-kamino.md`
- **The sky dome is darkened by `Effect("Lightning")`** (BF1 KAM1, BF1 YAV2, BF2 KAM1): between flashes dome shell 0
  draws at 255·(1 − `SkyDomeDarkenFactor` 0.4) = 153; a flash lerps the dome AND the fog colour back to white over
  `FadeTime`. B2 parser `FUN_0045aa30`, tick `FUN_0045ae80` → `sky+0x2855`, consumer `FUN_003cb918` (shell 0 only);
  B1 `FUN_00377c40`, `FUN_00378120` → `sky+0x100`, `FUN_002f2308`. PS2 RAM holds 153 in 20 of 21 in-level states (one
  mid-flash). The port draws every shell at white: sky 1.6–2.1 × too bright; prototype parity bf1_kam1 26.62 → 21.80,
  bf2_kam1 22.49 → 19.20.

(Moved: the rest of this section's facts are in re-kamino.md.)

## 2026-09-30 (loop lane 2, card ANNC) — the commander's voice-overs: triggers, lines, the voice queue → `analysis/re-announcer.md`
- **Command posts**: the post's own table `VO_<Listener>_<Subject><Capture|Lost|InDispute|Saved|Info>` (entry 2's name
  cracked: `InDispute`); retail sets own capture, own loss and enemy capture only — the enemy's loss is silent. BF2
  gate `ScriptCB_EnableCommandPostVO` (campaign scripts only).
- **Correction to `re-tickets-spawn.md` §2.4: BF1's conquest bleed is LIVE.** The team's vtable slot +0xc
  (`FUN_002ad2f0`) qsorts the `AddBleedThreshold` table ascending, so the scan picks the tightest threshold; live RAM:
  `[(1, 3.0), (11, 2.25), (21, 0.75), (31, 0.0)]`, a team on 2 of 5 posts lost one ticket per accumulator wrap (6 in
  32.2 s). The port plays BF2's bleed on every map (PLAN), so no sim change — the docs and a code comment were wrong.
One foreground Opus verifier, 37 claims: 32 SUPPORTED, 5 PARTLY, 0 NOT SUPPORTED, folded in.

(Moved: the rest of this section's facts are in re-announcer.md.)

## 2026-09-30 (loop lane 3, card EVMSG) — the on-screen event messages of both games → `analysis/re-event-messages.md`
`[O] UNAUDITED`. Both ELFs, both Locl tables, BF2's `1playerhud`, and two live PCSX2 battles (BF1 KAS2, BF2 HOT1)
sampled from RAM by the game clock (`lead_tmp/re/evmsg/evsample.py`; frames + states in `pcsx2/ref/evmsg/`).
- AI names come from Locl `common.sides.<side>.firstnames/lastnames.NN` ("Ben Pulastra", "UNIT 688"); the composer is [H].

(Moved: the rest of this section's facts are in re-event-messages.md.)

## 2026-09-30 (loop lane 2, card SQACK) — squad orders: the player's line, who answers, what each order does → `analysis/re-squad-orders.md`
- **BF1**: four D-pad orders — Up MOVE OUT and Left AT EASE (one case in the binary: release), Down HOLD POSITION,
  Right FOLLOW ME. The issuer (`FUN_00260f38`) plays the player's line (`<s>_inf_com_chatter_tac_{moveout,hold,follow}`,
  own stream, unqueued) and shows the HUD cross with the four order names for a 2.0 s fade BEFORE it looks for anyone;
  then every friendly AI within 30 m (45 m for the release orders; 3-D; x3 in vehicles) gets the order — no cap, no
  LOS, no nearest-first. A unit takes it at its next think (the AI manager gives each controller one full update per
  15 frames = 0.5 s) and answers `ai_<s>_inf_com_chatter_tac_yes`, queued on the voice stream; a take drawn in the
  last 1 s (2 s for all / imp) is silent, so 3-5 takers give 2-3 answers about a second apart.
- **Corrections to this log's 2026-09-01 squad entry**: its "UnitHoldAgent `FUN_00248cb8`" is `UnitRandomAgent`, its
  "UnitFollowAgent `FUN_00248498` / `FUN_00248828`" is `UnitHoldAgent`, its "fly agent `FUN_002474e0` / `FUN_00247b18`"
  is `UnitFollowAgent` (RTTI; the same shift in BF2: 0xe = Follow `FUN_002c9418`, 0xf = Hold `FUN_002caa40`); the
  "1-in-9" draws are 1 in 18. The 30 hashed ODF sound keys of `re-sound-global.md` §3 are `SC<Field|Driver|Passenger|
  Gunner><Action>Sound`, their `AISC…` twins and `AISCResponseYesSirSound` / `NoSirSound` (the latter never played).
- **BF2**: BF1's line + HUD function is unreferenced code (0x2e2480); the live order is contextual — press at a
  friendly AI soldier under the reticle: follow / stop (`SC_Follow` … events, `<side>_command_*`, the unit's
  `<side>_response_*` 0.5-1.2 s later); hold 1 s: all followers stop; a human leads rank + 1 units (1-4 by medals).
One foreground Opus verifier, 49 claims: 39 SUPPORTED, 8 PARTLY, 2 NOT SUPPORTED, all folded in (§10 of the doc).

(Moved: the rest of this section's facts are in re-squad-orders.md.)

## 2026-09-30 (loop lane 3, card GREN) — weapon slots past the primary: channels, grenades, mines, detpacks, the AI's grenade → `analysis/re-grenades.md`
`[O] UNAUDITED`. Both ELFs (class-label tables -> constructors, setters, vtables; three functions as disassembly), every
`wpnc` / `ordc` / `expc` / placed `entc` of both discs, and two live PCSX2 runs (BF1 NAB1, BF2 HOT1: one row per game
update of the thrown ordnance from RAM; `lead_tmp/gren/probe{1,2}.py`).

(Moved: the rest of this section's facts are in re-grenades.md.)

## 2026-10-01 (Fable chat session, card KFLR) — the kill floor: what kills a soldier who leaves the world → `analysis/re-kill-floor.md`
- **`AddDeathRegion`'s consumer** `[O]`: one region manager per game (B1 `DAT_00518398`, B2 `DAT_005feea0`), 32
  slots of {region, type}; a per-frame update (B1 `FUN_002af1c0`, B2 `FUN_00371148`) tests soldiers and vehicles,
  AI and human alike — BF1 the FEET as a point, BF2 the collision bounding SPHERE against the region — and kills at
  once with no killer (B1 `FUN_002af0d0` → `FUN_001bbc98(E+8, 0-desc, 0)`; B2 `FUN_00370fd8` → `FUN_001df640`).
  The kill's third argument is 0, which skips the invincibility test: **a death region kills an invincible human.**

Ported (KFLR, `core/src/sim/bounds.rs`): death regions, the kill floor and a landing under the map kill past the
`invincible` cheat. **Ours:** the kill floor's height (15 m under the lowest floor the map's graph and posts stand
on, not −500 / −1000) and the landing under the map (the maintainer's rule; the PS2 has none). **Not ported:** BF2's
sphere-against-region test (the port tests the feet in both games), BF1's water-sink plane, the ceiling and the
horizontal limits, the landing-damage law (card SLAND).

(Moved: the rest of this section's facts are in re-kill-floor.md.)

## 2026-10-01 (loop lane 1, card STDOR) — where a door's bone-bound geometry rests → `analysis/re-bf2-doors.md` §10
A door's `BNAM` segments and `PRNT` prims are stored in their bone's frame (disc data + PCSX2 frames of DEA1's blast
door and MUS1's garage / square doors; [D], no ELF hop). Built in `bake::bind_pose`. Closes card BNR and
`re-round-stalls.md` C3's [H]. Still open: prims of non-door geometries (DEA1's trash compactor).

## 2026-10-01 (loop lane 2, card KSEAL) — BF2 Kamino's lit sea, a fit → `analysis/re-kamino.md` §2.6

(Moved: the rest of this section's facts are in re-kamino.md.)

## 2026-10-01 (loop lane 3, card WAI) — how a bot drives a walker: the AI's sticks, the shared steering → `analysis/re-walker-ai.md`
`[O]`, one Opus verifier in the foreground (39 claims: 37 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED, folded in), and a
tick-by-tick replay against live PS2 RAM: 7504 ticks of AI spiders, an AT-ST and tauntauns, both sticks and the yaw
step equal on every one (`lead_tmp/wai/wai_check.py`).
- **Class ids named** from the vtables' getters (EntityWalker 0x689960, EntityHover 0x6894f8, EntityFlyer 0x689368,
  EntityDroideka 0x689300, MountedTurret 0x68c8f8, EntitySoldier 0x689828, EntityCarrier 0x688f88) — this settles
  `re-walker.md` §9's three `[H]` ids.

(Moved: the rest of this section's facts are in re-walker-ai.md.)

## 2026-10-02 (loop lane 2, card AIRC) — the stick in the air, both games → `analysis/re-stance-roll.md` §4.3a
- **BF1** `FUN_00205cc8` cases 4–6 → `FUN_002098b8(fwd × 0.2, strafe × 0.2, brake 0)`: each axis raised toward 0.2 × the
  walk speed at Acceleraton × 1.2 × 0.2, never lowered. **[V][V-PS2]**

Ported: `core::soldier::Soldier::air_steer` (both rules, per `GroundRules`), `ai::brain::lost_jump` (a lost bot jumps
from where it stands). Not ported: the take-off update's missing gravity (card JARC, with BF2's numbers now on it).

(Moved: the rest of this section's facts are in re-stance-roll.md.)

## 2026-10-02 (loop lane 3, card WAI2, measure) — who pilots a walker, live from the first tick → `analysis/re-walker-ai.md` §13

(Moved: the rest of this section's facts are in re-walker-ai.md.)

## 2026-10-02 (loop lane 3, AUDIT AIRC) — the take-off update, the roll's mover, BF1's backpedal → `analysis/re-stance-roll.md` §4.3b

(Moved: the rest of this section's facts are in re-stance-roll.md.)

## 2026-10-02 (loop lane 1, card EVMSG build) — BF2's points, AI names, the message line's glyph pitch → `analysis/re-event-messages.md` §9
- **Post names**: `level.<world>.<Label>` names 227 of 248 posts (BF1 KAS2 CP5 = "PIER ONE", as the PS2 printed).

(Moved: the rest of this section's facts are in re-event-messages.md.)

## 2026-10-02 (loop lane 3, card CWSP) — the command walker's post: a moving spawn post → `analysis/re-command-walker-post.md`

(Moved: the rest of this section's facts are in re-command-walker-post.md.)

## 2026-10-02 (loop lane 2, card CONT) — the Conquest AI goal: one random enemy post per bot, kept → `analysis/re-conquest-goal.md`
- **BF2 runs one Conquest goal per team** (`AddAIGoal(team, "Conquest", 100)` from `ObjectiveConquest.Start`; goal
  manager DAT_005fe7a8, 1 s tick `FUN_002e7cf8`); every AI record of the team is its member. **[V][L]**

(Moved: the rest of this section's facts are in re-conquest-goal.md.)

## 2026-10-02 (loop lane 3, card SNDC) — gameplay audio census: what the PS2 plays against what the port plays → `analysis/audio-census.md`
- **The port plays 5 of 24 event rows** (fire, reload, bolt strike, commander, the squad's answer); foley, hurt /
  death voice, chatter, every vehicle sound, explosions, flight hum, music, ambience and menu sounds are silent. **[L]**

Ported: nothing. Cards SNDT (build), FOLY, VSND, WSND, AMBM (re); notes on USND, GREN, PWUP.

(Moved: the rest of this section's facts are in audio-census.md.)

## 2026-10-02 (loop lane 2, card PWUP) — pickups: what a dead unit drops, the item, who takes it and what it gives → `analysis/re-powerups.md`
- **BF1's bots never look for an item; BF2's detour to one within 6 m** that gives health or ammo, every 2 s, at AI
  level of detail 3 or more (`FUN_002bd628` -> `FUN_002be050`). **[V]**
Corrects `unit-audit-bf1.md` §3.4 ("the droideka included": it authors the keys, the engine never reads them).
One foreground Opus verifier, 63 claims: 60 SUPPORTED, 3 PARTLY, 0 NOT SUPPORTED, folded in with its extra finds
(BF2's create gate `DAT_006d9330`, the hero / buff waste, `DebuffDamageTimer` / `Rate`).

(Moved: the rest of this section's facts are in re-powerups.md.)

## 2026-10-02 (loop lane 1, card SNDT) — a 3-D sound's level by distance → `analysis/re-sound-distance.md`
- **Level** = 1 from `rollindistance` to `mindistance`; `(R - x) / (x k + R)` beyond it (`x = d - min`,
  `R = max - min`, `k = 10 x rolloff`); 0 from `mutedistance` out; `d / ((rollin - d) k + rollin)` inside the roll-in
  distance. **[V]** The 10 is the listener's rolloff factor (constructor B1 `FUN_00449f78`, B2 `FUN_0053da70`; 10.0 in
  the RAM of all 329 reference states **[L]**; never changed in play **[H]**).

Ported: `bake sounds` resolves templates through CORE.LVL and carries mute / roll-in / 2-D; `game::sounds::attenuation`
is the function above.

(Moved: the rest of this section's facts are in re-sound-distance.md.)

## 2026-10-02 (loop lane 3, card FOLY) — soldier foley: the `ffx_` table, the surface, what starts a step → `analysis/re-foley.md`

(Moved: the rest of this section's facts are in re-foley.md.)

## 2026-10-02 (loop lane 3, card AIMA) — the aim ray, the camera's tilt and where a shot goes → `analysis/re-aim-assist.md` §10
- Named by hash: `MoveTension`, `MoveTensionX/Y/Z`, `ZoomFirstPerson` (the scope path fires along the aim itself).

(Moved: the rest of this section's facts are in re-aim-assist.md.)

## 2026-10-02 — Friendly fire as it runs, and the PS2's own team kills (card FFLN) → `re-blaster-damage.md` §6.3 [O]

(Moved: the rest of this section's facts are in re-blaster-damage.md.)

## 2026-10-02 — The PS2 foot soldier's own AI, both games (card SAIR) → `re-soldier-ai.md` [O]

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-02 (loop lane 3, AUDIT CONT) — the Conquest goal holds; a planning hub with no arc for the mover is not a hub → `analysis/re-conquest-goal.md` banner and section 10

(Moved: the rest of this section's facts are in re-command-walker-post.md, re-conquest-goal.md.)

## 2026-10-02 — Weapon and ordnance sounds beyond fire; the `soundlayered` record (card WSND) → `re-weapon-sounds.md` [O]

- **`soundlayered`**: `Layer(layer name, sound, delay [, deviation])` (B1 `FUN_0045b7b0`, B2 `FUN_0054e6f8`); the
  sound is looked up at parse time, so a layer naming a later-defined or absent property is dropped (BF1's
  `com_weap_mine_land` keeps one of two layers; RAM agrees). A delay of 0 starts at once; any other layer waits in a
  78-entry timer pool and starts at delay + deviation x U(-1, 1) at the armed position (B1 `FUN_0045ca08` /
  `FUN_0045bc10` / `FUN_0045be28`; B2 `FUN_0054fa38`...). PS2 logs: the grenade explosion's second layer +0.100 s in
  11 of 11. **[V][L]**

(Moved: the rest of this section's facts are in re-weapon-sounds.md.)

## 2026-10-03 — The bot design, and where the PS2's bots stand against its barriers (card BOTD) → `bot-design.md` [O]

- **The PS2's bots stay out of the barriers.** On foot they stand inside a soldier barrier in 0.3-1.2 % of their
  rows (BF2 Kamino 0.8, Polis Massa 0.3, Death Star 1.2, BF1 Kashyyyk Docks 0.3 %; the mission's switched-off
  barriers left out) and within 1.0 m of one in 23-26 %; in the fight order 13-40 % within 1.0 m. The port's bots:
  1.1-3.9 % inside. **[L]** (`lead_tmp/botd/tools/inbarr.py` on BOTM's live-RAM captures.)

Verifier (81 claims): 65 SUPPORTED, 16 PARTLY folded in, 0 NOT; it sank the draft's "96 % of stalls cross a
barrier" (a random line does in 90 %). Ported: nothing. Evidence `engine/artifacts/lead_tmp/botd/`.

(Moved: the rest of this section's facts are in bot-design.md.)

## 2026-10-03 — Vehicle sounds: the engine loop, walker steps, turret whir, boarding, collision (card VSND) → `re-vehicle-sounds.md` [O]

- Names found by hash: `TurnOnSound` 0xEA603E52, `TurnOffSound` 0x042DABAE, `TurningOffSound`, `WindUpSound`,
  `WindDownSound`, `HydraulicSound`, the parameters `health`, `strain`, `revs`, `winduptime`, `winddowntime`,
  `proximity`. 0x1ADA38C1 (`WeaponChange`) is read by neither ELF.

(Moved: the rest of this section's facts are in re-vehicle-sounds.md.)

## 2026-10-03 (loop lane 2, card HAIS) — how a bot drives a hover; Endor's lost speeder bikes → `analysis/re-hover-ai.md`
`[O]`, one Opus verifier in the foreground (49 claims: 49 SUPPORTED, 0 PARTLY, 0 NOT SUPPORTED; refinements folded
in), and a tick-by-tick replay against live PS2 RAM in both games (`lead_tmp/hais/hai_check.py`, `hai_check1.py`):
BF2 Endor 22617 of 22621 ticks equal on throttle, strafe and turn, Kashyyyk 15815 of 15832, Geonosis 6491 of 6492;
BF1 Endor 24089 of 24222.

(Moved: the rest of this section's facts are in re-hover-ai.md.)

## 2026-10-03 (loop lane 3, card BPRL) — vertex colour is blended in screen space behind a guard-band clipper; DEA1's killed post → `analysis/re-vertex-colour-interp.md` [O]

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

(Moved: the rest of this section's facts are in re-vertex-colour-interp.md.)

## 2026-10-03 (loop lane 2, card AMBM) — music, ambience streams, command post / prop loops, interface sounds → `analysis/re-ambience-music.md` [O]

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

(Moved: the rest of this section's facts are in re-ambience-music.md.)

## 2026-10-03 (loop lane 3, AUDIT JARC) — the jump's gates, JumpHeight, the ledge a jump mounts → `analysis/re-stance-roll.md` §4.3c
- **The ledge a jump pressed against a face mounts**: BF2 only with its apex 0.42 m or more over the top (13 of 13;
  never at 0.35 m or less, 12 trials) - 1.44 m yes, 1.51 m no; BF1 with its apex up to 0.15 m UNDER the top, not
  0.25 m (16 trials) - 2.07 m yes, 2.17 m no. The running jump is unmeasured. **[V-PS2]**

(Moved: the rest of this section's facts are in re-stance-roll.md.)

## 2026-10-03 (loop lane 3, card DEKA) — the droideka as a fielded unit: shield, walk form, collision, AI, measured motion → `analysis/re-droideka-unit.md` [O]

(Moved: the rest of this section's facts are in re-droideka-unit.md.)

## 2026-10-03 (loop lane 2, card TBLD) — how a build pad becomes its turret: the building's build state, the fusion cutter, the bots that build, the hologram → `analysis/re-build-zones.md` [O]

- **The fusion cutter** (class `repair`; B2 `FUN_00347d48` / `FUN_00346c78` / `FUN_00346fa0`, B1 `FUN_00292f60` /
  `FUN_002929b8` / `FUN_00292ce0`): an amount per target type (the low three bits of the entity's flags: 3 vehicle,
  4 building, 5 destroyed, 6 unbuilt, 7 mine) once per ShotDelay. BF2 200 / 100 / 50 / 50 every 0.5 s; BF1 60 / 60 /
  20 / 30 (four sides: half the amount at 0.25 s). Live: 120 of 120 steps of 50.0 at 0.5 s on Mos Eisley, five
  builds of 7.5 s; 30.0 on Naboo, ten builds of 4.4-4.5 s. **[V][L]**
- **The hologram** (type `build`) is hidden per screen unless the screen's soldier is alive and carries a
  `WeaponRepair` (B2 `FUN_002172a8` / `FUN_0024daa0`, B1 `FUN_001e6a68` / `FUN_0020c5b8`); frame
  `lead_tmp/tbld/ps2/yav1_holo_1b8b5c0_1.png`. **[V][L]**

(Moved: the rest of this section's facts are in re-build-zones.md.)

## 2026-10-05 (loop lane 1, AUDIT ANNC2) — BF1's sound sub-levels: one name, one definition per era → `analysis/re-announcer.md` banner

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

(Moved: the rest of this section's facts are in re-announcer.md.)

## 2026-10-05 (loop lane 2, card ANIM) — the soldier animator: weapon pose sets, two body layers, firing on the move → `analysis/re-soldier-anim-layers.md` [O]

- **BF2 sets**: one per (skeleton family, weapon animation class), 151 {upper, lower} pairs each, filled by
  `FUN_00282f78` from `<family>_<class>_<animation>[_upper|_lower|_full]` along the two parent chains
  (classes: rifle root, bazooka and tool under rifle, pistol and melee under tool; families under `human`).
  Re-built from the disc: 665,176 of 665,176 slots over 176 battle states, 38 distinct sets. **[M]**
- **Strafing**: neither game ships standing strafe clips; the soldier plays the forward or backward clip with the
  legs twisted (BF1 up to 72 deg, alert -45 / +31.5). Corrects `re-foley.md` 4.4's reading of BF1's "left runs".

Ported: nothing (RE). Cards: ANIMS (bake the sets), ANIML (the two players and the action layer), ANIMA (alert and
aim), ANIMM (movement ids, rate, twist), ANIMX (what stays unread: first person, low detail, seats). Verifier, 51
claims: 41 SUPPORTED, 10 PARTLY folded in, 0 NOT. Evidence `engine/artifacts/lead_tmp/anim/` (claims.md,
VERDICTS.md, sets_bf1.py, sets_bf2.py, the `ram_*.py` readers, AGENT_*.md).

(Moved: the rest of this section's facts are in re-soldier-anim-layers.md.)

## 2026-10-05 (loop lane 2, card B1DK) — why Kamino's decks are dark: the light sum's colour space, the ambient floor, lights on a missing hardpoint, BF1's local lights → `analysis/re-kamino-decks.md`
- **The ambient floor.** `bake::lighting::AMBIENT_FLOOR` (0.045 linear = 60/255 display) lifts 8 manifests (COR1's
  ambient is 0 in RAM). It is why the display-unit sum read "mixed" on BF2 in 2026-09 (card LSUM): with the PS2's
  ambients the sum takes all maps 10.618 -> 9.742, no map up by more than 0.5 (COR1's floor 41.8/47.1/57.0 against
  42.7/46.3/54.2). **[V][V-RAM]**

(Moved: the rest of this section's facts are in re-kamino-decks.md.)

## 2026-10-05 (loop lane 2, card WTHR) — weather: precipitation, wind, rain shadows, lightning flashes, thunder, the sky bolt → `analysis/re-weather.md` [O]

- **Lightning** (BF2 `FUN_0045ae80`, BF1 `FUN_00378120`): groups of `NumSubFlashes` flashes, a crack per group and a
  sub-crack per further flash (unpositioned; silent while every listener is in a sound space); per flash a `FadeTime`
  fade b = B(1 − t/F) + flicker: dome f = `SkyDomeDarkenFactor`(1 − b), **the fog colour set to the dome's bytes**,
  ambients k(saved + 0.333·sun), and the sun **`Color`·(b − s) in BF1 but `Color`·(1 − b) in BF2** (BF2's sunlum is
  an int of 0..1 floats = 0). `HorizonAngleMinMax` is dead. **[V][V-RAM, three mid-flash states]**

(Moved: the rest of this section's facts are in re-weather.md.)

## 2026-10-05 (loop lane 2, AUDIT JGATE) — the gate's update counts, the energy bar, the landing lock → `analysis/re-stance-roll.md` §4.3d
- **BF2's window is 10 updates, BF1's 9**: dt 0x3d088888 and the EE's rounding leave the timer at 0.29999998 after
  nine steps against `c.lt.s timer, 0.3f` (0x2437a0); live n = 10 jumps (2 of 2), n = 11 not (0 of 3). **[V][V-PS2]**

(Moved: the rest of this section's facts are in re-stance-roll.md.)

## 2026-10-06 (loop lane 2, card ZOOM) — zoom: the button, the weapon's steps, the camera, the turn rate, the scope → `analysis/re-zoom.md` [O]
- Checks: `engine/artifacts/lead_tmp/zoom/cite_check.py` (242 cites); one Opus verifier, 52 claims: 48 SUPPORTED, 4
  PARTLY folded in, 0 NOT. Live: `lead_tmp/zoom/ps2/` (six runs, 70 row files).

(Moved: the rest of this section's facts are in re-zoom.md.)

## 2026-10-06 (loop lane 2, card WADE) — soldiers in water: drawn before the near water, walking on the bed, drowning → `analysis/re-water.md` §9 [O]

(Moved: the rest of this section's facts are in re-water.md.)

## 2026-10-06 (loop lane 2, card SPUP) — the sprint start's speed test and a slope → `analysis/re-stance-roll.md` §4.2b
- **[V][M] `0.9 x MaxSpeed < v . the body's forward row`** (B2 `FUN_00240bf8` 160321–160329), polled while L3 is held.
  Live (BF2, one row per update, Hoth x 8 slopes + Kamino's ramp x 3): the full-stick run's number is MaxSpeed x cos^3
  of the floor's angle, so the sprint starts at 1.7, 9.8, 13.3 and 14.8 deg (Kamino's ramp, 0.904) and never at 16.1
  or 19.3; three steeper runs show one update of sprint off a terrain seam [H, not explained].

(Moved: the rest of this section's facts are in re-stance-roll.md.)

## 2026-10-06 (loop lane 1, card SNDX) — `FireSound` once per trigger pull → `analysis/re-weapon-sounds.md` §7.7

(Moved: the rest of this section's facts are in re-weapon-sounds.md.)

## 2026-10-06 (loop lane 2, card GRNP) — the BF2 grenade throw beside the port's → `analysis/re-grenades.md` §13

(Moved: the rest of this section's facts are in re-grenades.md.)

## 2026-10-07 (loop lane 1, card FGHT) — the radius a soldier is heard at; where the PS2's close contact is → `analysis/re-soldier-ai.md` §15

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

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
- Tools: `engine/artifacts/lead_tmp/vpad2/` — `pads_ram2.py` (a state's pad ring), `census1.py` / `census2.py`,
  `examples/vpad2_probe.rs` / `vpad2_census.rs` (the port's pads as a battle builds them).

(Moved: the rest of this section's facts are in re-vehicle-pads-load.md.)

## 2026-10-07 (loop lane 2, card VREST) — where a parked vehicle rests: the pad's spawn point, the flyer's LandedHeight, the hover's frame order → `analysis/re-vehicle-rest.md` [O]

(Moved: the rest of this section's facts are in re-vehicle-rest.md.)

## 2026-10-07 (loop lane 2, card VTAT) — turrets and the AI: the seat's own controller, the self-piloted turret, AutoFire, the seat's trigger → `analysis/re-turret-ai.md` [O]

- **[V][O] BF1 has no such AI**: a seat's controller is made only in SetPilot, there is no order pass; its two
  self-piloted classes (the Naboo fambaa's shield and generator) run on two class flags the turret's own tick reads,
  `Controllable = 0` (a LockOnRange ray along the fixed aim sets the weapon's target) and `AutoFire = 1` (the tick
  holds the trigger). That ray is what `re-aim-assist.md` 3.10 / `re-vehicles.md` 9 called the MountedTurret's
  acquisition; it never runs for a turret with the default `Controllable = 1`. Both corrected in place.

(Moved: the rest of this section's facts are in re-turret-ai.md.)

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

- Tools: `engine/artifacts/lead_tmp/knock/` — `knock_probe.py` (a grenade among held subjects: living, health 1,
  corpse, forced states, control; `--double`, `--knockin`, `--enemy --droids`, `--dead-only-off`), `knock_ana.py`,
  `droid_ana.py`, `live_summary.py`, `state_pick.py` (a spawn state's player class, grenade and neighbours),
  `claims.md`, `verify/VERDICTS.md`, `out/` (rows, `push_table.txt`).

(Moved: the rest of this section's facts are in re-knockback.md.)

## 2026-10-07 (manual run lane 1, audit of card GRAI: PARTLY) — the AI's weapon choice read again → `analysis/re-grenades.md` §7.5

- Tools: `engine/artifacts/lead_tmp/audit-grai/` — `README.md`, `ps2/{slots,firech,band}.py`, `ps2/port_slots.sh`,
  `verify/re_checks.txt`, the switch patches `knob_*.patch`, `wf/RESULTS.md` (the workflow's tables).

(Moved: the rest of this section's facts are in re-grenades.md.)

## 2026-10-09 (loop lane 2, card FGHT2) — the radius a soldier is heard at, verified, with BF1's twin; where the bots die and at what range → `analysis/re-soldier-ai.md` §15.1, §15.4, §15.5 [O]
- Tools: `engine/artifacts/lead_tmp/fght2/` — `tools/{lives,rng,aware}.py`, `tools/{observer,knob}_patch.py`,
  `tools/mini.sh`, `verify/{CLAIMS,VERDICT}.md`, `out/`, `sw/` (the sweeps).

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-09 (audit FGHTA: PARTLY) — who the PS2's scout rule picks, BF1's twin, the think a circler's stop is drawn at → `analysis/re-soldier-ai.md` §16 [O]
- **[O] The key** (two verifiers, 14 claims: 9 supported, 5 partly - folded in -, 0 not): "stand and shoot before the
  draw" goes to a soldier whose CLASS is `UnitType scout` (BF2 class +0x8d4 = 0, BF1 +0x1914) when the chosen weapon
  is of the rifle animation type (`AnimationBank`, table 0x5fd848; not an `OffhandWeapon`) and in range - BF2
  `FUN_002abc38`, BF1 `FUN_00238d10`. BF1's draw has one range flag (a drawn stand stands anywhere in range), BF2's
  two (the middle band).

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-09 (loop lane 2, card CLSE) — a close fight's damage: the aim error vector the PS2 applies, and whose radius it is → `analysis/re-soldier-ai.md` §17 [O]
- Tools: `engine/artifacts/lead_tmp/clse/` — `README.md`, `tools/`, `out/`, `ps2/`, `sw/`, `re_section17.md`.

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

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
- Tools: `engine/artifacts/lead_tmp/clse2/` — `tools/` (`bf2_radius_replay.py`, `size_box.py`, `glb_box.py`,
  `bubble_scan.py`, `bf1_aim_off.py`, `bf1_fire_target.py`, `sair_bf2_poke.py` + `run_poke.sh`), `out/`, `ps2/`,
  `verify/claims.md`, `verify/verdicts.json`, `re_section18.md`.

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-09 (loop lane 1, card GRAIA) — the verifier pass over the weapon choice's corrections → `analysis/re-grenades.md` §7.5
- Tools: `engine/artifacts/lead_tmp/graia/` — `tools/{sw.sh,sum.py,mut_all.sh,port_slots.sh}`, `sw/`, `v/`, `all/`.

(Moved: the rest of this section's facts are in re-grenades.md.)

## 2026-10-12 (loop lane 2, AUDIT of card CLSE) — BF2's aim-error scaling re-derived and seen on the running PS2; the landed rule beyond 30 m → `analysis/re-soldier-ai.md` §19 [O]
- **[L] The rule on the running PS2** (`audit-clse/ps2/live_rule.py`, CLSE's two captures): the sideways aim offset of
  still pairs is 0.33 / 0.31 / 0.30 x k x (V . p) x R_T x F at 0-10 / 10-20 / 20-30 m (n 24 / 64 / 79); against the
  human 0.85 (n 8). C+0xc0 is 0 for the human and -1 for every bot - what `FUN_002a7fd8` counts.
- **[V] A third writer** of the weapon class's three `AIBubble*` fields: the copy constructor `FUN_003283d8`
  (B2:319739-319741) - the inheritance by BASE. 18.4's disc census re-read three times: the same 10 and 4 classes.

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-12 (loop lane 1, card MOUTH) — a soldier's leg line is the follower's segment, node reached -> node walked at; the follower in RAM → `analysis/re-soldier-ai.md` §2 [O] UNAUDITED
- **[O] The projection's line** is follower + 0x10 -> + 0x20 at all three calls of `FUN_002a5fa8` (B2:281242, 281249,
  281386, `FUN_002f1d80`); the advance `FUN_002f1470` -> `FUN_002f0e78(follower, follower + 0x20, ..)` moves the node
  walked at into + 0x10 (B2:280791-280792, 280429-280440). After a detour corner the line starts at the corner.
- **[L] The follower in RAM**: one per bot in a fixed array (BF2 stride 0xb0, BF1 0x90): [0] node list, [1] the AI
  controller, + 0x10 / + 0x20 / + 0x30 / + 0x40 the node come from / walked at / after / last, + 0x60 mode, + 0x64
  side. A node on a hub's sphere is that hub's point; two of three BF2 nodes reached were not (corners, goals). The
  path OBJECT of `audit-hubm` is the planner's, not the walk's: its current hub sat a median 114 m from the bot.

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-12 (loop lane 2, AUDIT of card BSPN) — the AI's spawn-post draw read again by two verifiers; PS2 captures that tell post-first from the flat node list → `analysis/re-command-walker-post.md` §6.1 [O] UNAUDITED

(Moved: the rest of this section's facts are in re-command-walker-post.md.)

## 2026-10-12 (loop lane 2, card CLSE4 measure) — the far fire split by weapon; the trigger's range test and the Barrage keys, both games → `analysis/re-soldier-ai.md` §20 [O]

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

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
- Workflow `skirt-re-verify` (4 agents): 9 BF2 claims 7 / 2 / 0, 7 twin + absence claims 6 / 1 / 0 from each of two
  verifiers working apart (none overturned), 8 measured claims 4 reproduced / 4 partly (ranges, folded in).

(Moved: the rest of this section's facts are in re-soldier-ground.md.)

## 2026-10-12 (loop lane 1, card CLSE4 build) — the trigger's range test and the Barrage keys are in the port → `analysis/re-soldier-ai.md` 20.5

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-12 (loop lane 2, card DOORS, measure + build) — every door of the five door maps, censused → `analysis/re-bf2-doors.md` 12

(Moved: the rest of this section's facts are in re-bf2-doors.md.)

## 2026-10-12 (loop lane 1, AUDIT of card CLSE2: PARTLY) — the aim-error block replayed on the running PS2; stance events add error; a mounted unit's block is its vehicle's → `analysis/re-soldier-ai.md` 21
- **[L] [O]** 21.3: a mounted unit's controller hangs off the vehicle (267 of 267 mounted bots in 49 states, 1,813
  of 1,851 live turret thinks): the size factor is the vehicle's (turrets 2.0-4.2, walkers 3.9-12.5, flyers 7.6
  and more) with its velocity and no stance factor. The port takes the seated soldier's (card VAIM).
- Workflow `audit-clse2`: 22 agents; two verifiers 17 SUPPORTED / 7 PARTLY / 0 NOT each. Tools and captures:
  `engine/artifacts/lead_tmp/audit-clse2/` (README.md; `ps2/block_live.py`, `block_replay.py`, `bump_human.py`).

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-12 (loop lane 1, card HVYW build, WIP) — the weapon a foot soldier may use on a vehicle: the suitability test of both games → `analysis/re-soldier-ai.md` 22
- One verifier: 5 SUPPORTED, 4 PARTLY, 0 NOT; corrected 4.3 (haywire is speed-gated) and added the melee / remote
  / BF1 cannon paths ahead of the mask. Census of main and the first build's numbers in 22.3; tools
  `engine/artifacts/lead_tmp/hvyw/tools` (hvcen.py, wdump.py, sw.sh), the observer `HEAVY_CENSUS` on branch `hvyw`.

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-12 (loop lane 2, card ROLLS, RE) — a roll is ONE clip with the body's root turned into it → `analysis/re-roll-picture.md`

- Workflow `rolls-re-verify`: 7 agents, 39 claims, 30 SUPPORTED / 9 PARTLY / 0 NOT on the first reading; the six
  absence and twin claims read twice (the second reading: 2 / 4 / 0, one SUPPORTED overturned to [H]). Tools:
  `engine/artifacts/lead_tmp/rolls/ps2` (roll_strip.py, analyse.py, latency.py, sheet.py), `checks/disc_dive.py`.

(Moved: the rest of this section's facts are in re-roll-picture.md.)

## 2026-10-13 (loop lane 2, card VTURD, RE) — a building turret's posed nodes, its gunner, its views, its wreck → `analysis/re-turret-draw.md`

- [O] **Gunner**: drawn when the seat has `PilotAnimation` / `Pilot9Pose` (BF1 `FUN_0020c880`; BF2 `FUN_0024de60`
  also hides on `IsPilotExposed = 0`, authored on flyers only); at `L x E x O` (hardpoint's bind matrix relative to
  its parent, that parent node's current matrix, the building's world matrix - `FUN_0023e028` / `FUN_002f74c0`);
  posed from one clip `human_<value>`: nine-pose = frame `yf + 4` blended toward frame `yf + 7 / + 1` by `|pf|`;
  no hand weapon; drawn in every view.

(Moved: the rest of this section's facts are in re-turret-draw.md.)

## 2026-10-13 (loop lane 2, card CPBAR, RE) — the PS2's capture icon: both games draw one in the 1-player HUD; BF2's layout ring is dead data → `analysis/re-capture-display.md`

- [O] **Both games draw a capture icon in every HUD** - `re-hud.md` 3 said BF2's 1-player HUD shows nothing; wrong,
  corrected. It is a code object, not layout data (BF1 `CaptureDisplay`: ctor `FUN_002bd8c8`, update
  `FUN_002bdc30`; BF2 ctor `FUN_0037e0a8`, update `FUN_0037e400`, texture by hash 0x621cb6df): the 64 x 64 symbol
  `hud_flag_timer` cut by a horizontal line into two colours, at the right edge at mid height (anchor (1.0, 0.5) in
  the scene's safe rectangle, offset (0, -30): frame x 420.99..486.40, y 194..258; the model fits the PS2 frames of
  both games to under a pixel with no fitted parameter).

(Moved: the rest of this section's facts are in re-capture-display.md.)

## 2026-10-13 (loop lane 1, AUDIT of card CLSE4: PARTLY) — the trigger's two rules read live from the PS2's burst state; a weapon with unlimited rounds is not held to n shots → `analysis/re-soldier-ai.md` 23

- Workflows `audit-clse4` (21 agents: 6 numeric, 8 mutations of which 6 survive, 4 controls, 3 verifiers) and
  `audit-clse4-verify2` (2 verifiers). Tools: `engine/artifacts/lead_tmp/audit-clse4/tools` (ps2_burst_bf1.py,
  ps2_burst_bf2.py, ps2_burst_mounted.py, ps2_burst_wclass.py, ps2_burst_bf1_unl.py, ps2_reset_share.py,
  wcensus.py, mounted.py, perseed.py, cmpseed.py, knob_patch.py).

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-13 - the hologram's beam and icon, read for the build (card CPHOL) -> re-point-lights.md 15
- [H]: the beam is left out where the fan's box reaches the near plane (fitted to 12 stored PS2 poses; the VU1
  program is not read). Tools: `engine/artifacts/lead_tmp/cphol/tools` (boxpair.py, neardist.py, mut.py,
  cphol_seq.rs, sheet.py).

(Moved: the rest of this section's facts are in re-point-lights.md.)

## 2026-10-13 - unit chatter, both games: every line a unit says and what asks for it (card USND) -> re-unit-chatter.md
- [O] **BF2**: `VOSound = <sound> <event> [conditions]` entries on the class (a child shares its parent's chain,
  seen in RAM), event = index in the 40-hash table at 0x624ed8 (0 for a name not in it); `FUN_002e6180` picks the
  speaker (none named: the subject's nearest team-mate), requires it within `VORadius` (default 50 m, authored by no
  class) of the subject - the local player when the caller names none -, lets an enemy speak 31 times in 101, an AI
  only at detail level 3 or 4, and plays the entry with the most matching conditions. 36 call sites are the whole
  trigger list: AcquiredTarget, FriendlyFire, Grenade, Repair, GivePowerup, the slaughter counters, and for a
  HUMAN's kills HeadshotKill and the killing sprees. `MissileIncoming` (186 lines) and the eight `...VO` request
  names are never fired; KillingSpree12 / 16, Kill, NearbyTeamKillSlaughter fire with no line.
- [L] 12 captures of 4 minutes (`lead_tmp/usnd/ps2/usnd_live.py`: sound starts + every property's gate re-arm, the
  player held alive, optionally put beside a front-line team-mate every 20 s): BF1 32 voice lines a minute wherever
  the player stands; BF2 0.25 at the spawn post, 8 to 12 in the fight; hurt + death 30 to 175 a minute in range.

(Moved: the rest of this section's facts are in re-foley.md, re-unit-chatter.md.)

## 2026-10-15 (loop lane 1, card ANIMM build) — BF2's leg twist, forward bit and turn on the spot → `analysis/re-soldier-anim-layers.md` 7.1 [O]

(Moved: the rest of this section's facts are in re-soldier-anim-layers.md.)

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

- Workflow `audit-hvyw` (22 agents: 4 numeric, 3 controls, 13 mutations of which 8 survive + 1 that did not compile and
  survives when made to, 2 verifiers). Tools: `engine/artifacts/lead_tmp/audit-hvyw/tools` (setup.sh, chain_sweeps.sh,
  sw.sh, setup_mut.sh, mut_run.sh, mk_mut.py; patches/observer.patch puts the card's HEAVY_CENSUS on a parent).

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-16 (loop lane 1, card VTURZ) — a building turret's zoom: its camera, stick zoom and FOV → `analysis/re-turret-draw.md` 4.1 [O]

(Moved: the rest of this section's facts are in re-turret-draw.md.)

## 2026-10-16 (loop lane 2, card MIXH) — the PS2's mix headroom: per-ear level law, the 39-voice cap, buses → `analysis/re-mix-headroom.md` [O]

(Moved: the rest of this section's facts are in re-mix-headroom.md.)

## 2026-10-16 (loop lane 2, card FAMB) — BF1 Naboo's fambaas: the shield, the generator, the mobile post → `analysis/re-fambaa-shield.md` [O]

(Moved: the rest of this section's facts are in re-fambaa-shield.md.)

## 2026-10-16 (loop lane 2, card FXC) — the chunks mungefs skips: shdw, CLTH, bnd_ → `analysis/re-chunk-census.md` [O]

- [O] `CLTH` (BF2 only, 58 chunks) = cloth meshes named by `ClothODF`: Hoth snowtroopers' skirts, clone capes, hero capes,
  CTF flags; the port draws those soldiers without them. `bnd_` = `Boundary { Path(...) }`, empty on 8 BF2 maps (UTA1,
  MYG1 among them), absent on BF1 YAV1 / YAV2 [receipt:ae8beb67] (note on BNDY). Cards VSHD (4), CLOTH (5). Workflow verifier (6 agents):
  30 / 37 supported, 6 PARTLY folded in, 1 NOT SUPPORTED (bnd_ details, recounted); the second readers overturned none.

(Moved: the rest of this section's facts are in re-chunk-census.md.)

## 2026-10-16 (loop lane 2, card CLOTH) — BF2 cloth: the CLTH mesh, who wears it, the step, the draw → `analysis/re-cloth.md` [O]

(Moved: the rest of this section's facts are in re-cloth.md.)

## 2026-10-16 (loop lane 2, card RIDK) — a destroyed vehicle's riders and the kill credit → `analysis/re-vehicle-death.md` §10 [O]

- [O][V-live] BF1 RHN1 + NAB1, manned hovers at 1 HP (`lead_tmp/ridk/tools/ridk_probe2.py`): 6 deaths, 11 riders, every
  rider's record = the vehicle's, the 5 destroyers' kills + riders and vehicle kills + 1 within 0.1 s, the empty record
  credited nobody. Workflow verifier (7 agents): 28 claims, 24 supported, 4 PARTLY folded in (one of them a twin that
  both readers marked PARTLY); the second readers overturned none. Card RIDK -> build.

(Moved: the rest of this section's facts are in re-vehicle-death.md.)

## 2026-10-16 (loop lane 2, card LOD) — the LOD switch rule: model, LOWD or nothing; soldiers' `_low1` → `analysis/re-lod.md` [O]

(Moved: the rest of this section's facts are in re-lod.md.)

## 2026-10-16 (loop lane 2, card SBLT) — why the port's BF1 soldiers are darker: the soldier ambient slot → `analysis/re-soldier-light.md` [O]

(Moved: the rest of this section's facts are in re-soldier-light.md.)

## 2026-10-16 (loop lane 2, card USKN) — a class's own skeleton, SkeletonRootScale, CollisionRootScale, the override draw → `analysis/re-soldier-skeleton-skins.md` [O]

(Moved: the rest of this section's facts are in re-soldier-skeleton-skins.md.)

## 2026-10-16 (loop lane 2, card VSHD) — vehicle shadow volumes: BF1 counts them and darkens x 0.625, BF2 never draws them → `analysis/re-vehicle-shadow.md` [O]

(Moved: the rest of this section's facts are in re-vehicle-shadow.md.)

## 2026-10-19 (loop lane 2, card BNDY) — the map boundary: the shapes, the half-plane test, the 10 / 6.5 s countdown → `analysis/re-boundary.md` [O]

- [O] Shapes come only from the map LVL's `bnd_` chunk (`Boundary { Path(name) }`, the first path of a name; one caller
  of the push per game [receipt:e88fecd8]); per shape a CONVEX half-plane test over its closed edges, a point inside any
  shape is inside, x and z only, no shapes = never outside (B1 FUN_002956d8 / FUN_002958f8, B2 FUN_0034c550 /
  FUN_0034c780). 22 of 32 baked maps have one; BF1 YAV1 / YAV2 and BF2 COR1 DAG1 DEA1 MUS1 MYG1 NAB2 TAN1 UTA1 none.

(Moved: the rest of this section's facts are in re-boundary.md.)

## 2026-10-19 (loop lane 2, card GUNWT) — the AI driver's wait for a gunner, the need-gunner call's two sources, the answerers' boarding → `analysis/re-driver-wait.md` [O]

(Moved: the rest of this section's facts are in re-driver-wait.md.)

## 2026-10-20 (loop lane 2, card SSEL) — the start-of-level and respawn flow: side select, spawn map, class select, the death camera → `analysis/re-spawn-flow.md` [O]

(Moved: the rest of this section's facts are in re-spawn-flow.md.)

## 2026-10-20 (loop lane 1, card BSHL) — the BF2 PS2 shell: screens, flow, stack look, movie, loading screen → `analysis/re-bf2-shell.md` [O]

(Moved: the rest of this section's facts are in re-bf2-shell.md.)

## 2026-10-20 (loop lane 2, card SSELB slice 4) — the class select's line-up ring → `analysis/re-spawn-flow.md` §4.1 [O]

- [O] Six places on a ring (`DAT_006a5080`, `FUN_0039bcb0`), place 2 the pick, place 5 unseen (colour 0 0 0 1); a
  soldier's place = lerp of two places x 150 + (100, 350, -452) (`FUN_0039a5f8`); a step slides the ring one place in
  1/3 s (`SD+0x3c += dt x 3`, `FUN_0039a980`), a step during a slide is dropped. One opus verifier: C1-C7 SUPPORTED, C8
  PARTLY (wording). The UI scene's perspective is fitted to the Hoth frame [M]; built in `game/src/spawn_preview.rs`.

## 2026-10-20 (loop lane 1, card PWST) — health, ammo and vehicle droids (`powerupstation`): reach, rate, team, animation, sounds, tips → `analysis/re-powerup-station.md` [O]

- [O] Measured live (`lead_tmp/pwst/ps2/heal_live.py`): BF1 NAB1 75 -> 300 in 15 steps of +15 every 1.001 s, served at
  4.56 m, not at 5.43 m; ammo 0 -> 4/3/1 (+1 / 2.0 s, caps max - 1); BF2 HOT1 100 -> 400 in 12 x +25, served at 4.60 m,
  not at 5.34 m. Census: 106 stations on the six showcase maps, 967 on all 32, every one `Radius`, none `EffectRegion`.

(Moved: the rest of this section's facts are in re-powerup-station.md.)

## 2026-10-21 (loop lane 1, card SSELB slice 7) — the spawn screens' soldiers' light → `analysis/re-spawn-flow.md` §4.2 [O]

(Moved: the rest of this section's facts are in re-spawn-flow.md.)

## 2026-10-21 (loop lane 1, card SSELB slice 8) — which class the PS2 refuses → `analysis/re-spawn-flow.md` §4.3 [O]

(Moved: the rest of this section's facts are in re-spawn-flow.md.)

## 2026-10-21 (loop lane 1, card SSELB slice 9) — the class carousel's dropped step → `analysis/re-spawn-flow.md` §4.1

(Moved: the rest of this section's facts are in re-spawn-flow.md.)

## 2026-10-21 (loop lane 1, card HTGT) — the target's name and health on the HUD → `analysis/re-target-hud.md` [O]
- BF2 `FUN_003b3398` shows the reticle's pick (K+0x180+8·ch, the PlayerController's winner) at once, no hit or hold
  [receipt:c983e4eb]:
  class name (Locl `entity.<side>.<rest>`), health / max bar in friend blue / enemy red / team-0 white / neutral yellow
  (× the group's Color), a 1 s flash over the lost span, a 1 s fade on loss with the bar turned white. BF1: an
  enemy-only red bar under the player's health (2 s fade) and a name tag over other HUMANS only. Workflow verifier
  (4 agents): 10 + 4 PARTLY plain; A1 (BF1 no name) overturned by both absence verifiers — fixed.

## 2026-10-21 (loop lane 2, card HATK) — where the shot came from: the hit-direction chevrons → `analysis/re-hud-attacker.md` [O]

(Moved: the rest of this section's facts are in re-hud-attacker.md.)

## 2026-10-21 (loop lane 2, card HCORN) — a hover's detour corners: radius, origin, the drop inside R → `analysis/re-hover-corners.md` [O]

(Moved: the rest of this section's facts are in re-hover-corners.md.)

## 2026-10-21 (loop lane 1, AUDIT of card BRSTU: PARTLY) — the burst's meter slots decoded; the burst state's second writer → `analysis/re-soldier-ai.md` 23.6 [O]

(Moved: the rest of this section's facts are in re-soldier-ai.md.)

## 2026-10-21 (loop lane 1, card TCOL) — a building turret as a body: its moving prims, its gunner, its death → `analysis/re-turret-collision.md` [O]

(Moved: the rest of this section's facts are in re-turret-collision.md.)

## 2026-10-21 (loop lane 2, card KA137) — BF2 Kamino's arc 137 over the sea → `analysis/re-kam1-arc137.md` [O]

(Moved: the rest of this section's facts are in re-kam1-arc137.md.)

## 2026-10-21 (loop lane 1, card HVYA step 4) — BF2's explosive claim (grenade + missile ordnance) allows one soldier of a team at a soldier, and one per hull position (hull + turrets + passenger slots) at a hover / walker / flyer → `analysis/re-soldier-ai.md` 24.5 [O]

## 2026-10-22 (loop lane 2, AUDIT of card RKHUL: PARTLY) — a walker's collision prims ride its live pose → `analysis/re-walker.md` 17 [O]
- Body = XFRM × table[PRNT] in both games (`FUN_00160160`, row vectors, XFRM first): RKHUL's order. A walker's table
  (B2 E+0x1d7c via physics vtable 0x61f7f0 slot +0x4c; B1 E+0x1ec0 via 0x533a80 slot +0x44) holds pointers to its
  skeleton instance's bone matrices, rewritten every update (B2 `FUN_00533f20` in `FUN_00261208`, B1 `FUN_00440b38` in
  `FUN_0021b3a0`): the prims follow the clip, where the port bakes them at the bind (a parked spider's legs 3.4 m off
  the drawn ones; card WHULL). Static read; workflow `audit-rkhul`, two separate readers, 4 claims each, all SUPPORTED
  by both, none overturned; the auditor's `re_check` fn / word runs on every cited line and word.

## 2026-10-22 (loop lane 2, card LOOKR) — the look stick's turn ramp, both games → `analysis/re-look-ramp.md` [O]

(Moved: the rest of this section's facts are in re-look-ramp.md.)

## 2026-10-22 (loop lane 2, card OVHT) — the weapon heat gauge on the HUD, both games → `analysis/re-heat-hud.md` [O]

(Moved: the rest of this section's facts are in re-heat-hud.md.)

## 2026-10-22 (loop lane 2, card UORD) — ordnance that is not a bolt: missile + the lock, shell, bullet, emitter → `analysis/re-ordnance-kinds.md` [O]

(Moved: the rest of this section's facts are in re-ordnance-kinds.md.)

## 2026-10-22 (loop lane 2, AUDIT of re-ordnance-kinds.md: PARTLY) — bolts are swept against units, measured live in both games → `analysis/re-ordnance-kinds.md` 2.4 [O]
- Fixed in the doc: IMP's torpedo ordnance authors neither Waver key (REP / ALL only); of the 1,400 replay pairs, 442
  BF1 pairs never moved and the 582 BF2 free pairs never turn, so the steering rests on 377 pairs and BF2's waver is
  unmeasured live; 580 (not 581) of 582 BF2 pairs fit; the mortar bursts ~6.8 m up with the code's steps; two
  wordings. Workflow `audit-uord-re` (22 agents): 6 reproductions, 6 replay mutations (sign, ramp, phase order,
  speed law and target height caught where the data exercises them; the sin-vs-angle clamp is below the noise), 10
  readers on 30 claims (50 verdicts, 46 SUPPORTED, 4 PARTLY on wording, none refuted; no second reader overturned
  the first).

(Moved: the rest of this section's facts are in re-ordnance-kinds.md.)

## 2026-10-22 (loop lane 2, card GTHRW) — the grenade throw on the soldier, both games, third and first person → `analysis/re-grenade-throw-anim.md` [O]
- First person, both games: an 11-column clip table per weapon row (BF1 `_1st_<set>_<action>` in `human_animation_fp`,
  `FUN_001a6880` / `FUN_001a38e0`; BF2 `humanfp_*`, 0x618440, `FUN_001beeb8` / `FUN_001bfcb0`): a thermal plays
  `_1st_grenade_shoot2` / `humanfp_grenade_shoot2` (11 frames). The port has no first-person view (card FPV).

(Moved: the rest of this section's facts are in re-grenade-throw-anim.md.)

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

(Moved: the rest of this section's facts are in re-sprint-clip.md.)

## 2026-10-22 (loop lane 1, card AITHK) — how often a PS2 soldier bot thinks: BF2's interval by level of detail and its "think now", BF1's round → `analysis/re-ai-think-rate.md` [O]
- Live BF2 Kamino, every update (`lead_tmp/aithk/ps2/think_live.py`): of 52,049 bot update pairs with no think between,
  all 28 with the human entering / changing in the threat list pulled the think (each to now + 0.0001), 0 of 209 with
  only an AI entry; at levels 0-2 nothing else pulled one. A life's first think is at once with no order; the order
  comes one interval later (4.0 s with nobody playing, CONT's lives).

(Moved: the rest of this section's facts are in re-ai-think-rate.md.)

## 2026-10-22 (loop lane 2, card VPOSE) — a vehicle's own posed nodes: the node table, its turret and aimer nodes, the FinAnimation nine-pose → `analysis/re-vehicle-pose.md` [O]

(Moved: the rest of this section's facts are in re-vehicle-pose.md.)

## 2026-10-22 (loop lane 2, card VPOSW) — a walker's posed nodes: its node table, the walk pose under its aimers and turrets, the aimers' own frame → `analysis/re-walker-nodes.md` [O]

(Moved: the rest of this section's facts are in re-walker-nodes.md.)

## 2026-10-22 (loop lane 1, AUDIT of re-grenade-throw-anim.md: PARTLY) — a held L1 shows BF1's reload clip; BF2's legs take the throw only from a still, not-kneeling start → `analysis/re-grenade-throw-anim.md` §10 [O]

(Moved: the rest of this section's facts are in re-grenade-throw-anim.md.)

## 2026-10-22 (loop lane 2, card VSHUD) — the HUD in a seat: BF2 runs the soldier's HUD code on the seat's controllable → `analysis/re-seat-hud.md` [O]
- `FUN_0038e598` puts the reticle at (0.5, y) of K+0x150 + 1024 x K+0xe0 through the drawn camera; the switch to the
  soldier's aim (ControlsUnit, 0x44c77c95) is authored on neither disc. Replayed on 20 BF2 states: equal to the stored
  element to 0.000 lines (tower 240.0 = centre, bike 178.5 and AT-ST 166.1: the cameras look `TiltValue` 5 / 6 deg below
  the aim); BF1's twin `FUN_002d4b50` on 6 BF1 bike states, 6 of 6.
- `FUN_003b4bb8`: entering hides the soldier's bar and shows the green vehicle bar (a building turret: the owner
  building's health) and a 4 s seating chart (new card VSEAC); `FUN_003b5708`: the energy bar is the vehicle's (none for
  a turret). Live PCSX2 frames per seat kind with the gun overheating: `lead_tmp/vshud/ps2/ps2_seat_sheet.png`.

(Moved: the rest of this section's facts are in re-seat-hud.md.)

## 2026-10-22 (loop lane 1, AUDIT of re-sprint-clip.md: PARTLY) — BF2's sprint turns the root toward the travel; droid sprints divide by the engine's 11.5 m/s fallback → `analysis/re-sprint-clip.md` §7 [O]
- Live, the auditor's own sampler (GTHRW's audit `tsamp.py`, decoder `audit-sprcl/tools/sdec.py`), BF2 Coruscant, 7
  trials: phase step = |m_xz| / 9.16 x 30 / (F - 1) on both players, mean error 0.000000 over 412 pairs each,
  including a pistol sprint whose upper clip differs (its own speed misses by 0.0215) and diagonal travel (the forward
  component misses by 0.006). The divisor is the slot's whole cm/s (9.16, not the bake's 9.1627).
- New: 11.50 m/s on the droid / Aayla sprint slots is `FUN_00282900`'s id-0 fallback for a forward root speed <= 0
  (B2:204683-204695, `lui 0x4138` at 0x282a18); the port bakes those clips with zero root motion, so the doc's "1.0
  when none is baked" is wrong for droids (the PS2: |v| / 11.5).

(Moved: the rest of this section's facts are in re-sprint-clip.md.)

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
