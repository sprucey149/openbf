# OPUS RE (2026-09-30) - [O] UNAUDITED

# How a PS2 soldier follows the ground: ramps, crests, edges (card STHOP)

Question (card STHOP): walking down a steep ramp, does the PS2's soldier snap to the floor under the new position, and
within what height? At what slope does he leave the ground? The port's `Soldier::step` holds `vel.y = 0` while grounded,
compares the new feet with the floor under the OLD feet (`sim/step.rs` `ground_under(pos)` before the step) and drops
contact past 0.05 m. It then switches to Fall 0.5 m above the floor with ~2 % air control. On BF2 KAM1 that makes a
34° ramp into a string of 16-tick hops.

## 0. The answer

- **There is no snap.** Neither game moves a grounded soldier down onto the floor. **[V]**, §1.6. What keeps him on a
  ramp is the velocity: while grounded, both integrators steer it along the **plane of the last floor contact** (axes
  T1 = right × N, T2). So walking down a ramp is walking along it, with a real vy. **[V]** §1.1
- **Contact is re-found every tick, after the move, by the collision pass.** The grounded bit is cleared after the
  integrator, and the soldier's collision response sets it again for a contact on his foot sphere. That sphere is
  `coll_body_lower`: r 0.57, centre (0, 0.57, 0.15). The contact counts as floor when its normal has **n.y > 0.6**
  (slopes < 53.13°) and the approach speed is < 0.1 m/s. N is then the contact normal. **[V]** §1.2–§1.4
- **Up to 2 cm of penetration is kept.** The push-out takes 0.02 off its upward part, so a soldier stays in contact
  over folds that fall away less than that. **[V]** §1.3
- **Airborne means only gravity.** With no floor contact, N resets to up (0, 1, 0), gravity is −18 m/s² and the
  soldier flies on until he touches again. **[V]** §1.1
- **In the air he stays in the stand stance.** BF2 switches stand/sprint to falling (state 8) only after **1.0 s**
  airborne. BF1 does so after 0.7 s (1.0 s if the stance is ≥ 5 s old). Until then:
  - BF2 keeps the FULL ground acceleration;
  - BF1 keeps ×0.2 of it.

  **[V]** §1.5, and measured on the PS2: 1.0 s + one update, §2.3.
- **The slope limits.** n.y > 0.6 is floor.
  - BF2: control drops to ×0.1 when n.y < 0.7 (45.6°), and gravity along the plane then out-pulls the stick.
  - BF1: control drops to ×0.01 and braking stops when n.y < 0.75 (41.4°).

  **[V]** §1.1
- **PS2 trace on the card's KAM1 ramp (§2.1).** It is a 34.3° dome surface, n.y 0.826.
  - On the planar facets the player is **grounded every update** at 7.0 m/s along the plane (vy −3.9).
  - At the folds between facets: 4 of 24 updates without contact, the feet never more than 0.028 m over the floor.
  - Off the rim at its top, which he had climbed at vy +2.1, he flew **21 updates** (0.7 s), still in stand.
  - The port on the same walk: **grounded 3 of 39 ticks, in Fall 27**, the feet 0.14–0.85 m over the ramp (§2.4).
- **The spec of §3, prototyped on main b0f8fd3 (§2.5):** grounded 22 of 24 ramp ticks with the PS2's velocities; over
  8 seeds × 32 maps the rounds finished stay 227/256 (main 228) and world deaths fall 2557 → 1437 (KAM1 region deaths
  1370 → 324).

## 1. The mechanism [V]

Evidence: `engine/artifacts/lead_tmp/sthop/evidence/` (`fn_b1.txt`, `fn_b2.txt`, `words.txt`), claims `claims.md`.
"s+X" = byte offset in the soldier entity.

### 1.1 The integrators

| | BF1 `FUN_002098b8` | BF2 `FUN_002480a0` (twin) |
|---|---|---|
| grounded bit | bit 1 of s+0xbfd | bit 1 of s+0xa74 |
| ground normal N | s+0x960 | s+0x4b0 |
| velocity / feet | s+0xa20 / s+0x230 | s+0x550 / s+0x150 |
| axes | T1 = (s+0x200) × N, T2 = (s+0x220) × N, not normalised (119116–119119; 0x209cdc–0x209ce8) | T1 = (s+0x120) × N, T2 = N × T1, normalised when N.y < 0.99 (163010–163030; 0x248158–0x248164, 0.99 at 0x24813c) |
| grounded only | removes the into-N part of v (119155); adds the in-plane part of (0, −0.4·Acceleraton·dt, 0) (119124–119150, 0.4 = DAT_005178a8) | removes the into-N part of v (162984–163000); the per-axis clamp window shifted by 18·dt·(−T1.y, −T2.y) (… an uphill target is scaled by 1 − (uphill part)²/\|target\|² (163059–163066) |
| slope scaling of the stick | both inputs × \|N.y\| (119113/119115; abs.s + mul.s at 0x209cbc–0x209cd4) | — |
| acceleration | Acceleraton × 1.2; ×0.2 when not grounded; ×0.01 when N.y < 0.75 (119223–119232; stance 7: ×0.3) | Acceleration (× the caller's factor); ×0.1 when grounded and N.y < 0.7 (163053–163054); **no reduction when airborne** |
| braking | only when grounded and N.y > 0.75, scaled by the caller's brake (119196–119212) | — |
| gravity | −18 (−13.5 in stances 5/9) only when not grounded, and only with no platform (s+0x950 == 0) (119280–119300) | −18 only when not grounded, no platform (s+0x4a0 == 0) (163098–163100) |
| move | s+0x230 + v·dt → vtable +0x10 of the collision sub-object (vtable 0x532cb8, slot word 0x532cc8 = FUN_00210ff8) (119316) | s+0x150 + v·dt → vtable +0x10 at s+0xcc (163121) |

The update resets N to (0, 1, 0) when the bit is clear:

- BF1 FUN_00205cc8, 116535–116538;
- BF2 FUN_00240bf8, 159161–159164.

It runs the stance switch (the integrator), then **clears the grounded bit**:

- BF1 117980 `& 0xfd`;
- BF2 161722.

The measured PS2 values match the table on the KAM1 ramp:

- 7.0 m/s along the plane going down (§2.1);
- 4.87 m/s going up a 34° slope = 7·(1 − sin²θ) (§2.2).

### 1.2 Who sets the grounded bit

- **BF1:**
  - FUN_00202f38, the collision response (114582, 114713, 114841);
  - FUN_00203e70, the post hook (115048; 115243 is a prone-only ray);
  - the network state read FUN_0032ed90 (317359).
- **BF2:**
  - FUN_0023cfd0 (156032, 156162, 156452);
  - FUN_0023e380 (156587; 156763 prone-only);
  - the stream reads FUN_0040eae0 / FUN_004194e8.

The spawn/reset `| 3` writes are overwritten by `= 0` in the same functions:

- BF1 111679 → 111751, 317720 → 317741;
- BF2 149590 → 149813, 469461 → 469484.

So a soldier starts airborne with N = up. That is a search of every `+0xbfd` / `+0xa74` / `(p + 0x29d)` store with
`| 2`/`| 3`, repeated by the verifier with the int-index spellings. **[V]**

### 1.3 The collision response (BF1 FUN_00202f38 via thunk FUN_00211148 = vtable 0x532cb8 +0x1c; BF2 FUN_0023cfd0)

Terrain contacts arrive through EntityTerrain's collide `FUN_00212530`, body contacts through `FUN_00168a60`; both call
the mover's +0x1c.

For a contact on the soldier's **`coll_body_lower`**:

- the body is BF1 class+0x1e80 per posture, BF2 class+0x1248 = fnv `0x9458fb2e`;
- **normal.y > 0.6** (BF1 … BF2 literal) with **approach dot(v, n) < 0.1** is a **floor**
  (BF1 114813, 114838; BF2 156134, 156159);
- it sets the bit and N = n unless N is already as flat (BF1 114839–114846; BF2 156161–156166).

Then, for every contact:

- the into-normal part of v is removed (114856 / 156333);
- |vx|, |vz| < 0.1 → 0;
- the push-out = direction × depth, with **its y reduced by 0.02, clamped at 0**. The ELF bytes are
  `lui 0x3ca3 / ori 0xd70a / sub.s / max.s`: BF1 0x203c48, BF2 0x23e11c (114902 / 156396).
- the soldier moves to pos + push (114913 / 156407).

So a floor contact up to 2 cm deep is not pushed out vertically. That residual overlap is what keeps the next update's
contact alive.

Contacts with n.y ≤ 0.6 are walls, and this is a nuance for the wall pass, not STHOP:

- only the velocity's copy of the normal is flattened, so only horizontal velocity is removed;
- the push keeps the true normal, so a steep face lifts the soldier by n.y·depth − 0.02 (verifier, re-read at
  114813–114830).

BF2 extras:

- classes with class+0x778 & 0x20 (s+0x4ac ≠ 0) take every contact as floor (156048);
- a floor contact records the object stood on (s+0x480), whose motion carries the soldier (158580–158757).

### 1.4 The post hook

BF1 FUN_00203e70 (main vtable word 0x532e54) and BF2 FUN_0023e380 run after the collision pass. BF1's order, per the
verifier: update key 0 → collision key 10 → hook key 15.

- The response keeps a running mean of the lower body's up-facing contact normals: BF1 s+0x990, count s+0x981,
  114795–114807; BF2 s+0x4d0.
- If that mean has **y > 0.6** and the bit is still clear, the hook sets N = the mean and the bit (115022–115048 /
  156554–156590).
- So a soldier wedged between two steep faces stands.

### 1.5 The foot and the pass

- **The foot** (BF1 FUN_0020e2e0 122404, 122477–122491; BF2 FUN_002517e8 170220–170237):
  - `coll_body_lower` is a sphere (shape type 0) of **r 0.57 at (0, 0.57, 0.15)** for stand and crouch;
  - the terrain, building and vehicle layers test lower / upper / medium / medium2 (122756–122795).
- **The pass** (BF1 FUN_00163658, BF2 twin FUN_00163d08):
  - each soldier is walked from his last resolved position to the integrated one in `(int)(L / min(0.5·radius,
    bound) + 1)` sub-steps (3172–3191; the stand soldier's radius is 1.14);
  - each sub-step runs SetPosition → the terrain collide → each gathered body's collide, then deflects the rest of
    the step along the contact normals (3326, 3327, 3352, 3374).
  - So the floor is found under the **new** position, sub-step by sub-step.

### 1.6 No snap, no probe

The soldier updates do not touch the soldier's position except through the integrator, the response and platform
carry. Their ray casts are:

- aim (60 m);
- headroom;
- BF1's vehicle-exit placement (not per tick);
- BF2's 2 m down-ray at 159032, which only tilts the up axis for the s+0x4ac classes;
- the prone-only ground rays.

None moves a walking soldier down. (Verifier search; B3 in `claims.md`.)

### 1.6a The stance when airborne

- **BF2** (FUN_00240bf8):
  - **Stand** (case 0): on the first update without the bit, set latch bit 0x80 of s+0xa74 and zero the stance timer
    s+0x7ec (it grows by dt at 160119). Once the timer is **> 1.0 → SetStance(8)** (160284–160296; 0x243740
    `lui 0x3f80`, 0x24375c `jal 0x247bf8, a1 = 8`). Contact clears 0x80. The buttons act only while the timer is
    < 0.3 (0x243790).
  - **Sprint** (case 3): the same 1.0 s rule (160506–160521).
  - **Crouch** (case 1): stands up after 0.3 s (160363–160376).
  - **Prone** (case 2): → 8 after 0.3 s (160456–160457).
  - The falling state 8 moves with FUN_00248560: the stick × ControlSpeed[jump] × Acceleration added as acceleration,
    drag exp(−0.012·|v|·dt).
- **BF1** (FUN_00205cc8 case 0):
  - The timer s+0xbec grows by dt (117352).
  - When not grounded: timer < 5 → 10.3, 5 ≤ timer < 10 → 10.0; **timer > 11.0 → SetStance(6)** (117387–117401;
    0x20742c–0x2074c4). So falling starts 0.7 s after stepping off, or 1.0 s if the stance began ≥ 5 s earlier.
  - Contact with timer ≥ 10 → 5.0 (117405–117409).
  - Air acceleration ×0.2 (§1.1).

## 2. PCSX2 (BF2 KAM1, 0 AI, SW renderer) — `lead_tmp/sthop/ramp_bf2.py`, `kam1_*.csv`

Every game update was sampled from live RAM (1 ms polling, one row per game-clock change): feet s+0x150, v s+0x550,
N s+0x4b0, flags s+0xa74, stance s+0x7e4, timer s+0x7ec.

A 0x00 flag row followed by 0x02 with the timer not reset is a read inside an update (the bit is cleared before the
pass). It is not counted.

### 2.1 The card's ramp (`kam1_ramp.csv`; the port's floor under every sample: `ps2_walk_floor.txt`)

The ramp is a planar-faceted dome side of 34.3°, n.y 0.826, running from about (−46, 60.9, 287) down to about
(−33.2, 50.5, 279.7). The PS2's contact normals equal the port's ray normals, e.g. (0.34, 0.83, −0.45) and
(0.45, 0.83, −0.34). So the collision mesh is the same data.

The player started on the flat top at 60.300, yaw 307.8°, and held the stick forward 1.8 s:

- **Up the rim, then a flight.** The rim's face has n (−0.48, 0.61, 0.63) and counts as floor. He climbed it at
  vy +1.1…+2.2, lost contact at 60.83 moving up at 2.06 m/s, and flew **21 updates** in stand (latch 0x80, timer
  0 → 0.63) with full air acceleration: v_h 5.3 → 7.0 m/s in one update. He landed on the ramp at (−42.46, 58.09,
  284.58).
- **Down the ramp.** On each planar facet he was **grounded every update**: v (4.6, −3.9, −3.6) = 7.0 m/s along
  the plane, y −0.13 per update, the feet +0.017 m over the floor. That is what a 0.57 sphere 0.15 ahead, facing
  downhill and touching the plane, gives: +0.0177.
- **The folds.** Between facets whose normals turn from (0.34, 0.83, −0.45) to (0.45, 0.83, −0.34), he lost contact
  for one update each time: N → up, gravity, a drop ≤ 0.03 m, contact again.
- **In all:** 24 updates on the ramp, 4 without contact, feet +0.001…+0.028 m over the floor (`summ.py`).
- **At rest on the slope** he creeps: the push drifts him downhill ~1 mm/update until contact lapses, then gravity
  drops him 0.02 m (one update at −0.6 m/s), every ~15 updates.

### 2.2 Off the ramp's foot (`kam1_edge.csv`)

- He walked from (−38.8, 55.06, 281.87) down the ramp and past its foot at (−34.0, 50.95, 278.2) into the gap, and
  flew 22 updates (0.73 s, stance 0).
- He landed 8.4 m lower on the far up-slope, n (−0.52, 0.83, 0.22), at vy −17.15, and walked up it at 4.87 m/s along
  the plane (vy +2.69) — BF2's uphill scaling.

### 2.3 The stand → falling rule (`kam1_air.csv`)

- Grounded at the spawn (timer 4.13), then held 20 m up with v = 0.
- The first airborne update set 0x80 and timer 0; at **timer 1.000 the next update was stance 8**, timer 0.
- Released, he fell in state 8 (dv/update 0.6 → 0.37 at 24 m/s, the drag) and landed back to stance 0.

### 2.4 The port on the same walk (`port_kam1_ramp_main.txt`, main 6c3c8ea, scratch probe `sthop_probe.rs walk`)

From the same start and heading (all_inf/imp_inf rifleman, 2.33 m/s per tick of acceleration):

- The port climbs the rim by floor snaps with vel.y = 0 (+0.30, +0.28), then leaves it level. At t12 it is Fall
  (0.74 m over the floor).
- It lands at t21, is airborne again at t22 (the ramp dropped 0.15 m under the old feet), Fall at t26, lands t37,
  and so on.
- 39 ticks on the ramp: grounded 3, in Fall 27, the feet 0.14–0.85 m over the floor.

### 2.5 The spec, prototyped (scratch, not landed: `lead_tmp/sthop/proto_core.patch` on main b0f8fd3)

- **The same walk** (`port_kam1_ramp_proto.txt`):
  - 24 ticks on the ramp, **22 grounded, 0 in Fall**, the feet at most 0.012 m over the floor;
  - v (4.60, −3.86, −3.60) and (4.55, −3.95, −3.56) on the two facets, the PS2's to two decimals.
  - Before the ramp the point foot differs from the PS2's sphere. It crawls up the rim face (n.y 0.6065, ×0.1 control,
    as the PS2's rule says) instead of rolling over it. It also drops one tick through a **crack in the collision**
    at the rim–ramp seam (−44.6, 286.3), which the 0.57 sphere bridges.
- **The round sweep, 8 seeds × 32 maps** (`sweep4_main`, `sweep4_limits`, `sweep4_nolimit`; `cmp.py`):

  | | rounds finished | maps ≥ 7/8 | world deaths | KAM1 region deaths | KAM1 no-progress | BES1 falls |
  |---|---|---|---|---|---|---|
  | main b0f8fd3 (with STFAL) | 228/256 | 28/32 | 2557 | 1370 | 74 | 494 |
  | prototype, PS2 slope rules | 227/256 | 28/32 | **1437** | **324** | 196 | 469 |
  | prototype, no slope rules | 220/256 | 26/32 | 1214 | 495 | 170 | 56 (22 stuck) |

  - KAM1 still finishes 0/8, and its no-progress count at the round's end rises 74 → 196. That column also counts
    idle holders. Whether these are units alive on a wrong level is STLVL's question. [H]
  - Without the slope rules BES1's falls drop to 56, but 22 units end without progress and a round is lost. [H: a
    unit that grounds on any up-facing face stays on it.] The rules stay.
- **Three prototype bugs worth keeping:**
  - `rising` (probe from the feet, not feet + step) must mean the Jump stance. Plane-following gives a walker
    vel.y > 0 uphill, and probing from the feet then drops him through the ramp: GEO1 went 8/8 → 0/8.
  - The floor must resolve AFTER the wall pass. Before it, a unit pressed on an edge railing is judged over the pit
    and sinks through the edge.
  - A face too steep to stand on must still hold the feet out (slide, airborne), or units sink into steep terrain.

  A first comparison against a baseline that already had STFAL, with a prototype base that had not, read as
  "+50 % world deaths". Compare on one base.

## 3. What the port should do (build spec, card STHOP)

All in `core::soldier` / `core::sim::step`:

1. **Carry a ground normal N** on the soldier, from the floor contact under the NEW feet: `ray_down`'s normal, or the
   heightfield's slope normal. Reset it to up when airborne.
2. **Grounded integrator = BF2 FUN_002480a0:**
   - T1 = right × N, T2 = N × T1 (normalised);
   - drive dot(v, T1) / dot(v, T2) to the stick targets;
   - remove v's into-N part;
   - the gravity-along-slope shift of the clamp window;
   - the uphill target scaling;
   - acceleration ×0.1 when N.y < 0.7;
   - no gravity while grounded.

   BF1 maps differ in three places: stick × |N.y|, the 0.4·Acceleraton slope push, and ×0.2 air control. Decide per
   game with the existing `consts` split.
3. **Contact after the move.** `pos += v·dt`, then the floor under the new feet. If the feet are ≤ floor + 0.02 and
   the floor's n.y > 0.6, that is contact:
   - grounded, N = its normal;
   - feet = floor;
   - v loses its into-N part.

   The 0.02 is the point-foot analogue of the PS2's allowance. The PS2 keeps up to 2 cm of overlap after the push-out
   and loses contact only when a fold falls away by more than that. A point foot set onto the floor keeps no overlap,
   so it gets the same 2 cm above. It also absorbs float error along a plane.

   Otherwise the soldier is airborne, with N = up and gravity. The 0.05 m tolerance and the old-feet sample go.

   Details the prototype needed (§2.5):
   - resolve it after the wall pass, at the feet the push left;
   - probe from feet + step height unless the stance is Jump and vel.y > 0;
   - a floor with n.y ≤ 0.6 under penetrating feet still lifts them to the surface and takes the into-N velocity. It
     does not ground: he slides, airborne.
4. **Fall by airtime, not height.** Stand/sprint → Fall after 1.0 s airborne (BF2) or 0.7/1.0 s (BF1, §1.6a);
   crouch → stand after 0.3 s. Stand stays at full (BF2) / ×0.2 (BF1) control until then.
   - The `> 0.5 m above the floor` switch goes.
   - FEEL_NOTES: none; this is the PS2's measured rule.
5. **The acceptance run:** the scratch walk of §2.4 (`sthop_probe walk bf2_kam1 -46.708 60.4 287.820 -23.0 269.4 60 1`).
   Grounded on every planar-facet tick, as the PS2's is (the prototype: 22 of 24). Then the 8-seed sweep: finishes
   not below main's, world deaths down (the prototype: 2557 → 1437, KAM1 region deaths 1370 → 324). KAM1's ≤ 5 %
   needs STLVL as well.

**Left out on purpose** (point foot vs the PS2's sphere):

- the +0.02 to +0.22 m foot height on slopes (the 0.15 m forward offset);
- the sphere rolling over sharp crests;
- the 2 cm creep at rest.

A sphere-cast foot is a later refinement if a golden or a trace demands it. **[D]**
(§7, card SKIRT: a trace does ask for three rules of the sphere's contact - the flattest normal, the dead zone and
the overlap that dies on a slope - though not for the sphere itself.)

## 3a. What the build did (STHOP landed 2026-09-30, main eb90b56)

`core::soldier` (`GroundRules`, `Soldier::step`, `resolve_ground`), `core::sim::step` (3a'), `Sim::ground_under_n`.
The spec above, with BF1's rules behind `Sim::ground_rules`, plus four things the spec did not have:

- **The probe covers the tick's drop.** The contact is asked under the NEW feet, so a fall of more than a step in one
  tick started its ray under the floor it had crossed. The ray now starts at max(feet + step, the feet's height before
  the move). (Leak census `floor`: 8069 → 9359 without, 8124 with.)
- **Wedged = standing** (PROVISIONAL, ours; `WEDGE_MAX_DY` 5 mm). A soldier held on a face of n.y ≤ 0.6 whose feet did
  not change height over the tick stands, level. It is the point foot's stand-in for the post hook of §1.4. Without
  it 1,410 census runs ended airborne for good (67 before, 193 with). (Corrected 2026-09-30 by cleanse CLNC: this
  said 1,370, KAM1's region-death count; the census files `lead_tmp/sthop/gates/leak_1.txt` and `leak_v2.txt` sum
  `float` to 1410.)
- **Too steep to climb = a wall for the AI** (`ai::steer::steep_ahead`). The port's wall pass calls a face a wall only
  under n.y 0.5; with the PS2's limits bots pressed on steeper floors for a whole round (TAT1 (−137, 95), GEO1
  (−72, 268)). Raising the wall pass's own limit to 0.6 was measured and dropped: 220/256 rounds.
- **BF1 as far as it went:** stick × |N.y|, the 0.4·Acceleraton pull, ×0.01 under N.y 0.75, ×0.2 air control, the
  0.7 / 1.0 s timer. NOT ported: BF1's ×1.2 acceleration, its one-sided approach with a separate brake, its
  un-normalised axes. Still code-only [H] for the timings.

Measured (8 seeds × 32 maps, main 5c73167 vs the landed code, `lead_tmp/sthop/sweep5_main`, `sweep8_final`):

| | rounds | maps ≥ 7/8 | world deaths | KAM1 region deaths | KAM1 kills |
|---|---|---|---|---|---|
| main | 228/256 | 28/32 | 2557 | 1370 | 57 |
| landed | 226/256 | 28/32 | 1468 | 367 | 130 |

The KAM1 ramp walk of §2.4 on the landed code: 22 of 24 ramp ticks grounded, 0 in Fall (the PS2 20 of 24).

## 4. Found on the way (not STHOP's fix)

- **Landing damage.** A contact whose approach speed exceeds the class's **CollisionThreshold** deals
  (excess)² × **CollisionScale** (BF1 FUN_00202f38 114478–114540, class+0x1850.. / +0x185c..; BF2 FUN_0023cfd0
  155769–155866, class+0x7dc / +0x7e8).
  - The threshold = T0 + T1·(up·n) + T2·(fwd·n); the BF1 defaults are (10, 3, 0), FUN_0020d238 121824–121839. So a
    flat landing starts to hurt at 13 m/s, a ~4.7 m drop.
  - The port uses a PROVISIONAL 25 m/s cut-off (`sim/state.rs` `LETHAL_LANDING_SPEED`) for classes with nonzero collision damage. Authored zero-scale immunity is retained (§8); the positive-scale damage law remains SLAND debt.
- **Steep faces lift.** 0 ≤ n.y ≤ 0.6 contacts push out along the true normal (§1.3); the port's wall pass pushes
  horizontally only. Noted on SLAND; no card.
- **BF1 AI half rate.** An unimportant soldier (< 10 m/s, > 45 m from the camera, off screen: FUN_00236508) skips an
  update and runs the next with 2·dt (FUN_0020aad0 119745–119772, FUN_00205cc8 116566). Longer steps lift off crests
  more. Not ported. [V, verifier; not re-read]

## 5. Checks and what stays open

- **Checks:**
  - `re_check.py fn` on every cited line (`evidence/fn_b1.txt`, `fn_b2.txt`);
  - `word` on every constant and vtable slot (`evidence/words.txt`);
  - disassembly for the cross-product order, the 0.02, 0.99, 1.0, 10.3/10.0/5.0/11.0 literals;
  - fnv of the body names;
  - the grounded-bit writer census (both games).
- **Twins:** BF2 FUN_002480a0 / FUN_0023cfd0 / FUN_0023e380 / FUN_00163d08 / FUN_002517e8, matched by data flow and
  the same constants.
- **One foreground Opus verifier** (`claims.md`, 21 claims): **17 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED**.
  - A1: the stick × |N.y|;
  - A3: platform, brake and stance-7 gates;
  - B3: the spawn `| 3` is cleared, and the prone rays;
  - B5: steep-face push;
  - all folded in above and re-read by me (119113/119115 and 0x209cbc; 111751, 317741, 149813, 469484; 160456;
    114813–114830).
- **[H]:**
  - the exact caller that runs BF1 FUN_00203e70 after the pass (the verifier's key order; not re-read);
  - BF1's timings are code-only: no BF1 PCSX2 trace this session (STFAL's BES1 acceptance will take one).
- **The prototype** ran BF2's rules on all 32 maps. BF1's three differences (§3.2) are not prototyped. Its sweep
  numbers are measured once, on one base (main b0f8fd3), and were not seen by the verifier.

## 6. A band the port reads as n.y 0.667 that the PS2's bots walk up (card MOUTH2, 2026-10-12) [L], cause [H]

BF2 Kamino, the pod at hub 46 (centre (-15.3, 102.1)): a dais at 61.6 ringed by four rail walls, each with a
skirt outside it whose lowest 1.2-1.3 m (60.30 up to about 61.6) is ONE planar face of the baked collision mesh
with normal (0.728, 0.667, -0.161) on the east side and (-0.149, 0.667, 0.731) on the north
(`FLOOR_LINE=-6.2,101,-10.2,101,0.2,63` on the scratch `round_to_outcome` of `engine/artifacts/lead_tmp/mouth2/`:
the port's ray). By §1.1 that is under BF2's 0.7: control x 0.1 (FUN_002480a0, B2:163053-163054), and the clamp
window that follows (B2:163057-163075; `DAT_005fd0ac` = 1.0 in the ELF) then takes at least
18 dt |T.y| - 0.1 a dt = 0.447 - 0.233 = 0.21 m/s off the uphill speed every update: a climb under it can only slow.

- **[L] The PS2's bots climb it steadily.** 30 ascents from the ground to over 61.9 in two captures
  (`engine/artifacts/lead_tmp/mouth/ps2/bf2_kam1_m{2,3}_units.csv`, 8 min each at 10 Hz; `re-bf2-doors.md` 11).
  `m2` k 27 starts from a STAND at (-17.51, 60.30, 111.90), 1.7 m from the skirt's foot, and reads at 10 Hz
  60.46, 60.61, 60.84, 61.03, 61.32, 61.50, 61.86, 62.04: 0.6 s on the band at 1.3-3.2 m/s over the ground and
  +1.4 to +2.9 m/s up, as fast at its top as at its foot; k 16, k 22 and k 29 the same from other sides. About
  3 m/s along the slope is BF2's own uphill target for a 48 degree face at full hold: 7.0 x (1 - sin^2 48) = 3.1.
- **[L] The port's soldier does not.** Head on at 7.0 m/s he keeps the projected 5.3 m/s, rises 0.94 m and slides
  back (main 5fbbbd4, the steering's steep probe and the ledge rule's sideways push both switched off in a scratch
  binary; with them on he does not start up it - card MOUTH2's section "Measured").
- **[H] Why.** Either the PS2's N on that band is not the face's (the foot sphere's contact - r 0.57, 0.15 m
  ahead - on a fold or on a different body than the baked mesh), or the x 0.1 does not bind there. One PCSX2 run
  settles it: the player walked up the skirt with S+0x4b0 (N), S+0x550 (v) and the flags sampled per update
  (`engine/artifacts/lead_tmp/sthop/ramp_bf2.py`). Card SKIRT.
  **Settled in §7 (2026-10-12): neither.** N is the face's and the x 0.1 binds; he is carried over by momentum and
  three rules of the contact code. The "steady 3 m/s" above is 10 Hz samples of a climb that slows and is pushed on
  again (§7.1), and the port's body takes the band where it is under about 1.2 m high.
- **[L] Found on the way, the port's own:** the wall pass asks its LEDGE rule with the full capsule at the height
  the feet had before this tick's floor is found. On a face being walked up the feet are up to 0.25 m under it, so
  a riser whose top edge IS that face's edge (here two vertical triangles, normal (0.896, 0, 0.445), from
  (-18.36, 60.30, 109.76) up to (-17.44, 62.16, 107.90)) stands over the step, pushes him 0.10 m sideways and takes
  the into-push part of his velocity: 7.0 -> 2.6 m/s in one tick (seed 60, unit 18, tick 1057). Asked with the
  feet lifted to the floor under the axis the push is gone and he keeps (-2.90, 3.01, -3.28), the plane's
  projection. Scratch switch `MOUTH2=32` in `engine/artifacts/lead_tmp/mouth2/scratch_knobs.patch`; not built
  (card SKIRT).

## 7. A floor under the control limit: momentum, the flattest normal and two ways off the floor (card SKIRT, 2026-10-12) - [O] UNAUDITED

§6 asked how the PS2's bots get up a band that the port's mesh reads as n.y 0.667, when BF2's x 0.1 under 0.7
allows no steady climb. Answered with the player walked up it 24 times in PCSX2 (every update sampled), the port's
soldier on the same 24 walks tick by tick, the decomp for what the trace shows (a workflow of three verifiers and
one reproducer, §7.7) and a scratch prototype. Evidence: `engine/artifacts/lead_tmp/skirt/` (README.md).

### 7.0 The answer

- **N on the band IS the face's normal** - (-0.1487, 0.6665, 0.7305) on the north side, 0.6616 on the east: the
  port's mesh, the PS2's data. **The x 0.1 binds**: he loses 0.69 m/s per update above the uphill target and 0.21
  below it. **[L]**, and the integrator's own arithmetic **[V]** (§7.2).
- **He gets up it all the same, on momentum.** The band is 1.03-1.50 m high (48 degrees, then a 37 degree face
  the stick holds on). From a run-up of 2 m the PS2's soldier is over it in 0.97-2.10 s: 13 of the 15 head-on
  walks on its four sides, and the other two are a hold that ended at 2.0 s on a line he takes in 2.10 and a top
  3 mm under the mark. From a stand at its foot he is not: he climbs 0.3-0.7 m and hangs there (two lines, 6 and 8
  s watched; a third, on the east face, was over after 3.2 s). **[L]** §7.1
- **Three things in his contact code carry him, none of them in the port [V] + [L]:** the ground normal is the
  FLATTEST floor contact of the pass, so the update his foot sphere meets the band with the floor still under it
  runs at full hold (8 of 18 run-ups: 6.29 m/s onto the face instead of 4.78); after every contact |v.x| and |v.z|
  under 0.1 m/s are zeroed and only velocity INTO the face is removed, so when his speed runs out he leaves the
  face for one update, and that update is airborne - N up, full acceleration; and the push keeps 2 cm of overlap
  only vertically, so on a slope the overlap dies away and a hair of out-of-plane velocity sheds him the same way.
- **The port's body (main cae9777) makes 6 of the PS2's 14 climbs** on those 24 walks, misses 8 and makes 2 the
  PS2 does not. Its limit is a band about 1.2 m high where the PS2's takes 1.5 m: card SKIRT's "invisible wall",
  on most of the skirt but not all of it (card MOUTH2 traced a bot; the player with a run-up was not tried).
- **Prototyped behind switches** (null = main byte for byte): the flattest-normal rule alone makes 14 of 14; with
  the other two 14 of 14 and the maintainer's Kamino scene reads 0 trails over 6 reversals (main 3). §7.4
- **The bots at hub 46 are held by the steering first**: of main's 228 stalls there (52 per 100 passes, the PS2
  5), 94 push at the band from a stand at its foot and 60 slide along it because the steep probe calls it a wall.
  Body rules + the ledge rule's height + no steep probe: 87 (20 per 100). What is left is bots on the dais walking
  at points through its rail (43 of the 87; main 38): the route, not the body. §7.5

### 7.1 Measured on the PS2 [L] - `lead_tmp/skirt/ps2/skirt_probe.py`, `ps2/out/{a,b,c}_*.csv` (+ `.raw`)

BF2 Kamino, 0 AI, the imp rifleman (Acceleration 70, MaxSpeed 7.0, read from his class). Per walk the player is
held at the start while the camera turns, released, stands 1.5 s, then the stick is held forward. One row per game
update, the LAST consistent read before the clock moves: feet, velocity, N (s+0x4b0), the flag byte (s+0xa74: 0x02
grounded, 0x80 the airborne latch), stance, timer; the whole block s+0x120..0xad0 is kept raw. Every number below
was recomputed from the files by a separate agent (§7.7).

- **Meeting the band.** `a_n172`: v goes (0, 0, -6.998) -> (-0.760, 3.408, -3.264), the row before minus its part
  along the band's normal to 0.0002 m/s, with N still (0, 1, 0); the next row has N = the band's and |v| 6.29. On 10
  other walks the first row on the band already has N.y 0.6616-0.6668 and |v| 4.77-4.80 (7.0 x the projection).
  Head-on run-ups that got the full-hold update: 8 of 18 (`a_n172`, `b_r176`, all six of batch c); the same start
  and heading gave it in `c_e104long` and not in `b_east104`, so it is the update's phase, not the place.
- **On the band, stick held** (rows with N.y 0.66-0.67, both grounded): |v| falls 0.693-0.697 per update while
  above the uphill target (0.677-0.682 in the boosted `a_n172`), then 0.2089-0.2117 (0.22-0.25 for an update or two
  after a re-contact, while a sideways part is shed). No stick: he slides down, +0.2099-0.2100 per update. On the
  37 degree face above (N.y 0.7951) he holds 4.5389-4.5390 m/s (4.5429 on the south side, 4.5382 east).
- **Losing the floor.** 63 walk rows have no floor contact right after a row with N.y under 0.7. The next row has
  it again in 57 (the others ran off an end of the band). When he was going UP, that row is 0.48-1.61 m/s faster
  (42 rows); when he was sliding down it is not (15).
- **At the turning point.** Where the climb runs out (`a_east` t 1.2-2.0, `b_eastlong` 1.2-1.7, `b_foot` 0.5-1.1)
  all 10 rows without contact follow a row whose v.x and v.z are EXACTLY 0 and whose v.y is +0.010..+0.109; the
  raw rows show the feet 0.2-1.9 mm off the face along its normal on the row that lost it (v.n +0.007..+0.058 m/s).
  `b_foot` t 1.2027 has v.x = v.z = 0 with v.y -0.028: he stays grounded and slides. Over all 24 walks the climb
  ends 71 times: on 56 the last climbing row has v.x = v.z = 0 and v.y positive and 47 of those lose the floor on
  the next update; on the other 15, one does. On the east face the cycle (0.2 s: lift off, +0.8 m/s, four updates
  of climb) gains 0.034-0.041 m of height each time, 0.19 m/s.
- **In mid climb.** Where the face leans to the walker's LEFT (N . his right axis -0.15, 16 walks) the first loss
  comes 4-9 updates after the band is met on 14 and after 13 and 21 on two; where it leans right (+0.16, 8 walks)
  after 12-19, at the turn or the band's top. The raw rows: v.n is +0.0007..+0.0018 m/s before a loss (`a_nstand`,
  the feet +4.8e-5 m off the face) and -0.0001..-0.004 where the contact holds (`a_n122`, `a_east`).
- **The walks** (`tools/table.py`; seconds of stick when the feet first reach y 62.0, the top of the skirt; `--` =
  never while the stick is held, with the highest feet):

  | walk (start x, z; yaw; hold) | band high | PS2 | port main | proto 1+2 | proto 8 | proto 1+2+8 |
  |---|---|---|---|---|---|---|
  | n172 (-17.2, 112.6; 0; 2 s) | 1.12 | 0.97 | 0.97 | 0.97 | 0.83 | 0.83 |
  | nstand (-17.5, 111.9; 0; 2 s) | 1.07 | 1.07 | 0.90 | 0.90 | 0.80 | 0.80 |
  | n122 (-12.2, 112.6; 0; 2 s) | 1.16 | 1.70 | 1.00 | 0.97 | 0.83 | 0.83 |
  | south (-12.4, 92.0; 180; 2 s) | 1.17 | 1.30 | 1.00 | 1.00 | 0.83 | 0.83 |
  | r176 (-17.6, 112.6; 0; 2 s) | 1.05 | 1.00 | 1.00 | 1.00 | 0.87 | 0.87 |
  | east104 (-3.5, 104.0; 90; 2 s) | 1.25 | 1.24 | -- (61.15) | -- (61.15) | 0.97 | 0.97 |
  | eastlong (-3.5, 101.0; 90; 6 s) | 1.36 | 2.10 | -- (61.25) | 1.07 | 0.97 | 0.97 |
  | e104long (-3.5, 104.0; 90; 8 s) | 1.25 | 1.10 | -- (61.15) | -- (61.15) | 0.97 | 0.97 |
  | e102long (-3.5, 102.5; 90; 8 s) | 1.49 | 1.50 | -- (61.23) | 1.60 | 0.97 | 0.97 |
  | w101long (-27.0, 101.0; 270; 8 s) | 1.33 | 1.27 | -- (61.16) | 1.27 | 1.20 | 1.10 |
  | w104long (-27.0, 104.0; 270; 8 s) | 1.28 | 1.34 | 1.30 | 1.17 | 1.10 | 1.07 |
  | n150long (-15.0, 112.6; 0; 8 s) | 1.48 | 0.97 | -- (61.29) | 1.40 | 0.83 | 0.83 |
  | s150long (-15.0, 92.0; 180; 8 s) | 1.50 | 1.04 | -- (61.15) | 1.07 | 0.90 | 0.90 |
  | footE (-6.2, 101.0; 90; 8 s), 0.4 m off | 1.36 | 3.17 | -- (61.14) | 0.73 | 0.63 | 0.53 |
  | east (-3.5, 101.0; 90; 2 s) | 1.36 | -- (61.41) | -- (61.25) | 1.07 | 0.97 | 0.97 |
  | east99 (-3.5, 99.0; 90; 2 s) | 1.03 | -- (61.9966) | 1.17 | 1.17 | 1.10 | 1.10 |
  | foot (-12.2, 110.6; 0; 6 s), 0.6 m off | 1.16 | -- (60.98) | 0.83 | 0.73 | 0.60 | 0.60 |
  | footN (-15.0, 110.5; 0; 8 s), 0.1 m off | 1.48 | -- (60.59) | -- (60.45) | -- (60.85) | -- (61.40) | -- (61.05) |
  | nrest (dropped on the band, then 1.5 s) | 1.12 | -- (61.15) | -- (60.96) | -- (60.96) | 0.60 | 0.60 |
  | nhalf (as n172, stick 0.75 s) | 1.12 | -- (61.04) | -- (61.51) | -- (61.51) | -- (61.91) | -- (61.91) |
  | r179 (-17.9, 112.6; 0; 2 s), at a riser | - | -- (61.84) | -- (60.65) | 1.73 | -- (61.62) | 1.23 |
  | r182 (-18.2, 112.6; 0; 2 s), corner ramp | - | -- (61.68) | -- (61.66) | -- (61.66) | -- (61.68) | -- (61.68) |
  | obl (-14.5, 112.6; 45; 2 s), skims along | - | -- (60.68) | -- (60.71) | -- (60.71) | -- (60.88) | -- (60.88) |
  | bot41 (-15.5, 112.4; 41; 2 s), skims along | - | -- (60.60) | -- (60.63) | -- (60.63) | -- (60.66) | -- (60.66) |
  | **of the PS2's 14 climbs / misses / climbs the PS2 does not** | | 14 | 6 / 8 / 2 | 12 / 2 / 4 | 14 / 0 / 4 | 14 / 0 / 5 |

  The band's height is the port's mesh under each line (`FLOOR_LINE`, the foot to the first face of n.y 0.7 or
  more). `b_foot` ends in a cycle of 0.834 s between y 60.51 and 60.95 that repeats for the rest of the hold.
- **The PS2's feet ride 0.45 m over the band**: at z 109.96 on x -17.2 the port's ray reads 60.33 and card MOUTH's
  bot stood at 60.78. It is the foot sphere (r 0.57, 0.15 m ahead: §1.5): 0.57 / n.y - 0.57 + 0.15 tan 48.2 = 0.453.
  The sphere meets the band 0.40 m before its foot line and its contact point runs 0.57 m ahead of the feet. **[D]**

### 7.2 The mechanism [V] (BF2 `FUN_0023cfd0`, `FUN_002480a0`, `FUN_00240bf8`; BF1 twins `FUN_00202f38`, `FUN_002098b8`, `FUN_00205cc8`)

1. **The flattest floor contact stays.** A lower-body contact with normal y over 0.6 and approach under 0.1 sets
   the grounded bit and stores its normal ONLY IF the bit was clear or the stored N.y is less than the contact's
   (B2:156134, 156159-156166; BF1 B1:114813, 114838-114845, 0.6 = `DAT_0051788c`). Scope, from the verifiers: this is
   the path of the ordinary postures. Under posture 2 (BF2 bits 22-23 of the word at s+0x7e0, B2:155950; BF1
   s+0xbe0 == 2, B1:114631) the same test stores the normalised MIDPOINT of the stored and the new normal
   (B2:156004-156032; B1:114685-114713), and one contact kind stores (0, 1, 0) outright (B2:156447-156452;
   B1:114578-114582). Posture 2 = prone is **[H]**; the standing player's N equalled the face's to four places **[L]**.
2. **Only velocity INTO the face goes.** v -= (n.v) n when n.v < 0, else nothing (B2:156327-156341; B1:114851-114863).
   n here is the working copy: the contact normal for a floor (y over 0.6), its flattened and renormalised form for
   a wall (B2:156155, 155984-155995; B1:114834, 114666-114676) - §1.3's wall nuance.
3. **The dead zone.** Right after, |v.x| under 0.1 -> 0 and |v.z| under 0.1 -> 0, on the world axes; v.y is not
   touched (B2:156366-156373; B1:114883-114888).
4. **The push** keeps 2 cm of overlap only vertically: a positive push y becomes max(y - 0.02, 0) (B2:156394-156397;
   B1:114900-114902); position + push goes to the mover's SetPosition (B2:156398-156407).
5. **The integrator** (BF2): acceleration x 0.1 when grounded AND N.y under 0.7 (B2:163053-163054), nothing of the
   kind when airborne; grounded only, the uphill part of the stick target scales it by 1 - up^2 / |target|^2 and
   each axis' change is clamped to a window 18 dt x -(axis y) either way by the acceleration (B2:163059-163079,
   `DAT_005fd0ac` = 1.0, no writer [receipt:7c8b2ae8]); gravity only with the bit clear and no platform
   (B2:163098-163100). The update sets N to (0, 1, 0) when the bit is clear (B2:159161-159164) and clears the bit
   after the stance switch (B2:161722). BF1: B1:116535-116538, 117980; its factors are x 0.2 airborne and x 0.01
   under N.y 0.75 (`DAT_005178ac`), independent tests that stack, the second not gated on the bit
   (B1:119223-119233; neither constant has a writer [receipt:b0ea8e35]).

**The measured numbers follow from 5 [D]:** on N (-0.1487, 0.6665, 0.7305) heading -z the forward axis is
(0, 0.7387, -0.6740); the target is 7 x (1 - 0.7387^2) = 3.18 m/s and the window gives 0.443 +- 0.233 per update
down the face: 0.68 lost above the target, 0.21 below - the trace's 0.69 and 0.21. On N.y 0.7951 the same lines
give 4.541 m/s; the trace holds 4.539, the port 4.540.

**And the two ways off the floor [D] from 2-4, [L] in the rows:**
- *The turning point.* With the speed nearly gone both horizontal parts fall in the dead zone; what is left is
  (0, +v.y, 0), which points OUT of a sloped face (n.v = v.y n.y). Nothing removes it (2), the integrator adds only
  in-plane change, the move carries the sphere off the face, the pass finds no contact, and the next update is
  airborne: N up, full acceleration (BF2), gravity. He meets the face again about 1 m/s up it. It needs a row with
  the horizontal parts under 0.1 while v.y is still positive; the horizontal part steps 0.14 per update, so about 7
  turns in 10 (measured: 56 of 71, and the floor lost after 47 of the 56).
- *The overlap dies on a slope.* After a push the sphere keeps 0.02 n.y of overlap along the normal; on later
  updates the push is horizontal only and each leaves n.y^2 of it (0.444 on this band: 13 mm, 5.9, 2.6, ... 0.02
  mm after 8). Any out-of-plane velocity then decides: +0.002 m/s is 6e-5 m in an update and sheds him at the
  8th-9th update; a part into the face feeds the overlap and holds him. Which sign he carries is set by rounding
  in the dead zone and the axes, 1e-3 m/s: **not ported** (§7.6).

### 7.3 The port on the same walks (main cae9777) - `tools/skirt_walk.rs`, `out/port_k0_{a,b,c}.txt`

The port's rules are the PS2's where §3 ported them: the entry (4.78), the 0.695 and 0.21 per tick and the 4.540
on the upper face are the PS2's to two places. It differs in the three rules of §7.0 and in the foot:

- a point with a ring of rays 0.3 m out (cards GRND, RAIL): it meets the band at the foot line, 0.40 m later than
  the sphere, and finds the face above when the feet are 0.3 m short of the band's top, where the sphere's contact
  runs 0.57 m ahead of feet that ride 0.45 m high. Net, the port needs 0.14 m LESS travel on the band;
- no full-hold update on meeting the band, no loss of the floor on it: its contact holds within 2 cm, constant.

So it climbs the band where it is 1.03-1.17 m high (six lines, and `w104long` at 1.28) and runs out short of the
ring's reach where it is 1.25-1.50 (five lines, seven walks: feet no higher than 61.15-61.29), then slides to the
foot and repeats from a stand for good. At the riser (`r179`) it stops at 60.65: the ledge rule's height, §6's
last point - with that rule asked at the floor under the axis (scratch `MOUTH2=32`) its feet reach 61.80, the
PS2's 61.84. That switch changes no other walk's outcome (the rows of eight others move; two times by 0.03 and
0.07 s). The 0.14 m is **[D]** from the sphere's geometry, not measured.

### 7.4 Prototype (scratch, not landed) - `lead_tmp/skirt/scratch.patch`, `bin/skirt_walk_k2`, `bin/rto_k2`

`SKIRT=<bits>` in `Soldier::resolve_ground`: **1** the dead zone after a floor contact; **2** the overlap (0.02 n.y
on touch-down, x n.y^2 a tick, less the tick's move off the face; with none left and a move off the face over
1e-5 m the contact is lost for that tick; only on floors under the climb limit); **8** the flattest normal: on the
tick a floor steeper than last tick's N is first met, N stays the old floor's. The null equals main byte for byte
on all 16 walks of batches a and b, on Kamino's 485 P-stalls and on the maintainer's scene (3 of 403).
Read off the table: 1 alone changes nothing (the 2 cm tolerance hides the lift-off); 1+2 makes 12 of 14 and is
caught on one line (`e104long`: 3 of its first 6 turns step over the dead zone, and each slide to the foot costs
what the lift-offs gained - the PS2's own `b_foot`); 8 makes all
14, 28 % sooner than the PS2 on average (the PS2 gets that update on 8 of 18 run-ups, the port every time) and
climbs from a stand 0.5 m off where the PS2 hangs (`nrest`, `foot`): the port's foot meets the band 0.4 m later
and so has more run-up. A named deviation in the direction the maintainer asks of the bots.

### 7.5 The bots at hub 46 (BF2 Kamino, 8 seeds x 3 min; `tools/snapv2.sh`, `out/<tag>.txt`, `mouth/tools/site.py`)

Main's 228 stalls by the steering's answer and the feet's height (`out/hub46_n0_spots.txt`): `open fwd` at the
band's foot 94 (73 of them standing, v under 1 m/s), `steep slide` at the foot 60, `wall slide` against
`kam_bldg_pod2` on the dais or the skirt 38, the rest 36.

| stack | body | ledge | steep probe | hub 46 stalls / passes | per 100 | Kamino P-stalls | the maintainer's scene |
|---|---|---|---|---|---|---|---|
| main (null) | - | - | on | 228 / 436 | 52 | 485 | 3 of 403 |
| W | - | at the floor | on | 177 / 444 | 40 | 432 | 4 of 433 |
| WP | - | at the floor | off | 132 / 424 | 31 | 426 | 6 of 423 |
| L | 1+2 | - | on | 232 / 441 | 53 | 501 | 4 of 392 |
| LW | 1+2 | at the floor | on | 168 / 408 | 41 | 472 | 2 of 445 |
| LWS | 1+2 | at the floor | open for a short band | 128 / 404 | 32 | 439 | 3 of 387 |
| LWP | 1+2 | at the floor | off | 89 / 388 | 23 | 349 | 2 of 424 |
| K8W | 8 | at the floor | on | 153 / 386 | 40 | 435 | 5 of 424 |
| K8WS | 8 | at the floor | open for a short band | 124 / 421 | 29 | 405 | 5 of 440 |
| K8WP | 8 | at the floor | off | 88 / 415 | 21 | 356 | 7 of 452 |
| K11WP | 1+2+8 | at the floor | off | 87 / 425 | 20 | 378 | 0 of 444 |

(The PS2: 5 per 100. "Ledge at the floor" is card MOUTH2's scratch bit 32; "open for a short band" = the probe is
no wall when a floor that holds lies within 1.5 m up and 2.4 m on, `hold_above`.)

- The body rules do nothing for the bots while the probe keeps them off the band (L 53, K8 49).
- With the best stack the standing pushes fall 94 -> 21 and the slides 60 -> 0; the `wall slide` stalls on the
  dais and the skirt stay (43 of the 87): bots inside the rail ring walking at a point outside it. That is
  card OBST2's note (hub 46's centre lies inside the ring), now the largest part - card DAIS.
  **[L]** The PS2's bots cross the dais and leave it by its four corner ramps: 146 visits in card MOUTH's three
  captures (24 min at 10 Hz; `lead_tmp/skirt/tools/dais_ps2.py`), a median 1.5 s on it (25 over 5 s, the longest
  16.4 s); 134 leave down at a corner (SW 54, SE 35, NW 25, NE 20), 3 go up over the rail, 9 end there.
- **No probe at all is not safe**: tat1 + BF1 Geonosis + BF2 Geonosis, 8 seeds, recycles 9 -> 26 (BF1 Geonosis 0
  -> 10, its median finish tick 14324 -> 6862) with rounds 24 / 24 both ways; with the short-band test 12. The
  test as prototyped opens too little at hub 46 (31 `steep slide` stalls left of 60).

### 7.6 What the port should do (build spec, card SKIRT)

1. **`Soldier::resolve_ground`: the PS2's three rules**, each behind a switch in one binary first (null = parent),
   judged by `tools/table.py`: all 14 of the PS2's climbs, none of `r182` / `obl` / `bot41` / `nhalf` / `footN`.
   (a) the flattest normal on the tick a steeper floor is met (rule 1; every time, the PS2 8 of 18: named
   deviation); (b) the dead zone (rule 3); (c) the overlap as a soldier field (rule 4), lost only under the climb
   limit and only for a move off the face over 1e-5 m. Both new fields are hashed. BF1 takes the same three
   (twins [V]); its airborne update has x 0.2 and its limit is 0.75 - no BF1 trace, **[H]** for what that does.
2. **`collision::walls`: the ledge rule asked at the floor under the axis** (§6; `r179`).
3. **`ai::steer::steep_ahead`: a band with a floor that holds above it, within the rise the soldier's speed
   carries, is no wall.** Size the test from the body (a 48 degree band 1.5 m high is climbed at 7 m/s head on)
   and count `steep slide` at hub 46 to 0; TAT1 (-137, 95) and both Geonosis maps keep their recycles.
4. Not in this card: the dais (card DAIS); the sphere foot itself (the 0.45 m riding height, the early touch).

**Not ported, on purpose:** the mid-climb loss of the floor (rule 4's sign of a 1e-3 m/s residue decides it; the
port would need the EE's rounding in the axes) and the PS2's 8-in-18 phase of rule 1.

### 7.7 Checks and what stays open

- `re_check.py fn` on every cited line (`lead_tmp/skirt/evidence/fn_b1.txt`, `fn_b2.txt`), `word` on 0x51788c
  (0.6), 0x5178ac (0.75), 0x5fd0ac (1.0), `writers` on the three (0 each) [receipt:7c8b2ae8] [receipt:b0ea8e35].
- **Workflow `skirt-re-verify`, 4 agents** (claims and citations only, `lead_tmp/skirt/claims.md`; verdicts
  `evidence/workflow_verdicts.json`). BF2 set, one verifier, 9 claims: 7 SUPPORTED, 2 PARTLY (rule 1's scope, rule
  2's working copy - both folded in above). BF1 twins + the absence claim, two verifiers apart, 7 claims each: 6
  SUPPORTED and 1 PARTLY from both (the same scope note on rule 1's twin); neither could falsify the absence claim
  (one added an ELF instruction scan: one or two loads of each constant, no store). No claim was overturned by the
  second reader. Measured set, one reproducer, 8 claims: 4 reproduced, 4 partly - ranges I had drawn too tight or
  worded loosely; §7.1 carries its numbers.
- **[H] / open:** posture 2 = prone; BF1 on such a face (code only); why batch c's six run-ups all got the
  full-hold update; whether the PS2's AI has any slope test of its own (unread - its bots walk at the band at
  speed, 30 ascents in 16 min, §6); the arithmetic behind the mid-climb residue's sign.

## 8. Zero collision damage exposed by bot jets (issue #113, 2026-11-03)

**[O] New primary-code reading; no fresh original-game capture.** Both games
multiply the squared excess contact speed by the class's three-term
`CollisionScale`, evaluated against the contact normal. A positive result is
required before calling damage: BF1 `FUN_00202f38:114508–114538`, BF2
`FUN_0023cfd0:155830–155865`. When all three scale terms are zero, neither a
flat landing nor an inclined contact can produce collision damage.

The defaults are `(1, 0, 0)`: BF1 `FUN_0020d238:121824–121832`, BF2
`FUN_00250488:169421–169427`. The soldier property setters read successive
float components into those existing fields: BF1 `FUN_0020f2e0:122899–122901`,
BF2 `FUN_00252bb0:171547–171549`. The referenced formats in the owned ELFs are
three float conversions (BF1 `0x532af8`, BF2 `0x61e8c8`). Thus a partial
subclass value replaces only supplied terms; earlier components not supplied
by that override retain their inherited values.

BF1's `imp_inf_dark_trooper` and `imp_inf_dark_troopersnow` author all three
terms as zero. The PS2 gameplay table and the PC correction table retain these
values. In the same-input port sweep at `e5dd0e03`, Bespin Cloud City has
24 fall deaths across seeds 1–8, and the jet census attributes all 24 to jet
units. The corresponding October source `85dc9fa8`, before bot jets, has none.
The census-only reruns preserve every original outcome/hash row. This exposes
an older landing-rule error rather than proving that every new jet death is
wrong: death-region, kill-floor and lost-unit deaths remain separate causes.

The port now retains the class-chain zero-scale result as
`SoldierConstants::collision_damage_immune` and checks it before the existing
lethal-landing shortcut. The active unit and its respawn specification both
contribute this future-relevant state to the hash; the default format stays
unchanged. Under-map landings are checked first, and death regions, water,
blast damage and the kill floor receive no new exemption. This is a bounded
correction for authored immunity, not the full positive-scale collision law.

Focused controls cover both ground-rule sets, humans and bots, ordinary
lethal landings, preserved health for immune landings, death regions, active
and respawn-state hashes, partial/ordered class overrides and owned class
loads. Original-game landing traces and the nonzero-scale collision-damage
model remain unchecked; the old 25 m/s policy must not be described as the
PS2 damage formula.
