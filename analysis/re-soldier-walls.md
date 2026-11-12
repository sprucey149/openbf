# OPUS RE (2026-09-29) - [O] UNAUDITED — soldiers through walls: the wall pass's floor exemption (BF1 KAS2 City Hall's lobes) and thin-wall tunnelling at BF2 speeds

> **AUDIT 2026-09-29 (loop lane 2, PARTLY — `engine/artifacts/loop/audits.md`, report
> `engine/artifacts/loop/reports/20260929_2240_audit_JWAL.md`).**
> - **Method.** Re-derived before reading this doc, from:
>   - the B1/B2 decomp and ELF instructions;
>   - the disc's BF2 class data;
>   - the port, rebuilt at 816a9e1 (pre-JWAL), a8da834 (JWAL, not WPSH) and main, with the audit's own probes.
>
>   Evidence: `engine/artifacts/lead_tmp/audit-jwal/`. One Opus verifier: 9 of 11 SUPPORTED; V3 PARTLY, corrected below.
> - **Held.**
>   - **M1, to the tick.** At tick 42 (feet 16.0667) there is no push; at 43 he is inside body 109; at 61 he lands
>     inside, y 14.6967. The audit's own City Hall sweep: 2,294 of 3,888 approaches end inside before JWAL, all jumps;
>     0 on main.
>   - **M2:** 46/51 at MaxSpeed 9 before JWAL.
>   - **M3:** the tick-20 push (0.254, −0.799).
>   - **Census stdouts** are byte-identical to the WALK / VHULL / WPSH baselines.
>   - **§5.1's PS2 numbers** follow from the PS2's own soldier collision (next item):
>     - the head-on standoff 0.72 = 0.57 + 0.15;
>     - the into-wall velocity zeroed in one frame;
>     - an apex creep of 0.045 m;
>     - no lift at the rim (n.y 0.39 ≤ 0.6).
> - **New [V] (answers SRAD's question; built by card SRAD).**
>   - **Shape.** The soldier's terrain / building / vehicle parts are two spheres of radius 0.57 (the per-stance table
>     0x533090's 0.72 − 0.15), set 0.15 m FORWARD of the feet, with centres 0.57 and s·1.9 − 0.42 above them. B1
>     `FUN_0020e2e0`; B2 `FUN_002517e8`.
>   - **Walls.** A wall is a lower-sphere contact with n.y ≤ 0.6, or an upper-sphere contact with n.y > 0. It clips
>     the VELOCITY against the horizontal normal (then |v.x| and |v.z| under 0.1 snap to 0). It pushes the POSITION
>     along the FULL normal × depth, with the y part less 0.02, floored at 0. B1 `FUN_00202f38`; B2 `FUN_0023cfd0`.
> - **Wrong or overstated.**
>   - **§4's "BF2 MaxSpeed 9"** is the constructor default. Every fielded BF2 infantry class inherits
>     `com_inf_default`'s 7.0 (the snipers 8.0), so fielded sprint-jumps peak at 13.58 / 15.53 m/s (0.453 / 0.518 m a
>     tick). The sniper did cross 45/51 before JWAL.
>   - **§4.2's "0 of 51 (was 46)".** The 46 predates JWAL's build. At a8da834 the run crosses 1/51, and no fielded
>     class crosses at all.
>   - **§4.2's census numbers.** WPSH's own census on the pre-WPSH engine counts push 1,972. So the new start skip
>     makes 48,415 → 1,972, and the engine change makes 1,972 → 1,212.
>   - **§4.2's M3.** On main the tick-20 crossing is unpushed and is now filed as `ceiling`. A ~0.97 m sideways push
>     follows at tick 21. The soldier ends inside the ring (r 6.66 m) → WTUN.
>   - **§5.3's [H] swept test** is false for meshes. B1 `FUN_00160e60` is discrete and iterative. C+0x198 is the
>     collision object's previous position, and only the primitive-pair collide `FUN_001605d0` reads it.
>   - **§8's step height** stays [H]: the full-normal push can creep a soldier up an edge.

Card JWAL (lane 2). The maintainer's play session (INBOX 2026-09-29): on BF1 Kashyyyk Docks (KAS2), a round wooden platform's
bamboo side wall stops a walking soldier, but a run at it plus a jump forward carries him INSIDE, onto the ground
under the deck (`lead_tmp/playtest_0929/kas2_platform_wall_outside.png`, `kas2_platform_inside.png`). Question: what
lets him in, does the PS2 let him in too, and what should the port do?

## 0. Answer

- **The platforms are City Hall's four lobes** [V]. `kas1_bldg_hut_grand` (one instance, cp3 at (108, 16, -128), yaw
  180°) stands on four SOLID `prim` cylinders: p_collision1/4/5/6, r 9.25, h 4.0. The port bakes them as bodies
  103/106/107/109, centres (108 ± 6.17, 15.007, -128 ± 6.17), y 13.007–17.007, in the soldier and ordnance layers (§1).
  The render model's bamboo wall with arched frames is `kas1_wookiee_hut_walls`. The PS2 frames at the same wall
  show the same arches as the maintainer's screenshot (§5).
- **The port's mechanism (M1), reproduced to the tick** [V, port] [O]. The wall pass pushes the STEPPED capsule
  (base = feet + 0.6) out of what it touches, and skips any contact whose normal has |n.y| ≥ 0.5 as "a floor, the
  ground query's job". A jump lifts the capsule's lower sphere above the lobe's top edge, so the rim contact tilts past
  |n.y| 0.5 and is skipped. The ground probe starts at the feet (rising) or feet + 0.6 (falling). It never reaches the
  top, because the port's apex puts the feet at 16.41 and the top is at 17.007. So nothing holds him out, and he falls
  inside the solid onto the terrain. There, the shortest way out is straight up, which the same filter skips again (§2).
- **It is general** [V, port]. At City Hall, 8,903 of 19,440 scripted approaches end inside a lobe or the front
  block, all of them jumps (walk-jump 4,249, sprint-jump 4,654; walk 0, sprint 0). From 24 m out, 4 walk and 4
  sprint runs leak too: up the south stairs into the front block, over a 0.63 m ledge (§3.1). On flat ground with one
  obstacle H tall, a BF1 soldier walks THROUGH a solid box or a mesh sheet of H 0.5–0.75 m (sprint to 1.0 m). A jump
  passes H ≤ 2.5 m, and H ≥ 2.75 m blocks (§3.2). An indicative census counts 1,072 soldier-layer primitives on the
  32 maps standing 0.6–1.1 m above the terrain and 1,279 standing 1.1–2.75 m (§3.3).
- **A second mechanism (M2), BF2 speeds only** [V, port]. A tick's step longer than the capsule radius (0.5 m)
  crosses a zero-thickness `coll` mesh wall. The push-out then shoves the soldier out on the far side. At BF2's
  MaxSpeed 9 a sprint-jump moves 17.55 m/s, 0.585 m a tick, and passes a mesh sheet of every height tested up to 4 m
  in 46 of 51 jump timings. Solid boxes of H ≥ 3 m still block (§4).
- **The PS2 has neither hole** [V, PCSX2] [O]. Same map, same lobes, no-AI battle, clean RAM teleport, ten trials:
  eight jump at a lobe (run and jump at the wall ×2, jump 1 m and 2 m short, 60°, the west and north walls, a long
  hold), one runs without a jump, and one (120°) never reaches the wall. Every one ends OUTSIDE (§5.1):
  - the PS2 soldier stops 0.72 m from the lobe's surface head-on (0.62–0.64 in an oblique slide);
  - the wall zeroes the into-wall velocity in one frame (−8.50 → 0.00 m/s) and keeps the along-wall part (a slide);
  - so a jump against the wall goes straight up;
  - the apex puts the feet 0.35 m below the top, and he creeps in 0.05 m at most, then back out.
- **Two more PS2 facts from the same traces** [V, PCSX2]:
  - the BF1 jump applies no gravity on its takeoff tick, a tick later than the port, so its apex is 1.91 m against
    the port's 1.65 m (§5.2);
  - a RAM teleport needs the collision copy C+0x198 written with the position C+0x228 (§5.3).
- **AI** [V, port]. No AI path presses JUMP, so no bot uses the jump way in. The authored route over City Hall
  climbs the stairs' centre line onto the deck; its hubs sit inside the lobes' footprint at terrain height. The
  walk-through band (M1 walking, 0.6–1.0 m obstacles) does move bots (§6).
- **Fix** (§7, card JWAL phase build): a floor-like contact on the stepped capsule is a LEDGE, re-asked with the full
  capsule; a wall pushes the velocity's into-wall part off (PS2-measured); the AI's `wall_sweep` gets the same ledge
  rule. M2, and M3 (the census's own KAS2 control, an inward push at the cp5 hut, §4.1), go to card WPSH. New finding
  cards: JARC (the jump arc and the forward boost) and SRAD (the 0.72 m standoff).

## 1. The spot, in the baked data [V]

- `engine/assets_baked/bf1_kas2/manifest.ron`: `kas1_bldg_hut_grand` (class base `prop`), collision entry: glb
  `collision/kas1_bldg_hut_grand.glb`, whose mesh spans local y 1.0–24.66 (the hut above the deck only). The prims:
  p_collision1/4/5/6 are cylinders `dims (9.25, 4.0)` at local (±6.15, −0.993, ±6.17). p_collision2 is a box
  half (6.16, 2.0, 4.35) at local (0, −1, 10.93), the front block. p_collision3/7/8 are boxes tilted 26.6° about x
  (the stairs). p_vehicle_1/2 are cylinders.
- The class's collision props: `VehicleCollisionOnly` for p_vehicle_1/6/7/8 only. By the class rule in
  `game/src/collision_layers.rs` (header: no SoldierCollision property means every body is in the soldier subset;
  `VehicleCollisionOnly` removes the named body), the lobes and the front block are SOLDIER bodies.
- The port's world at City Hall (`lead_tmp/jwal/bodies_cityhall.txt`, the probe's `bodies 108 -128 22`):
  - body 100: the mesh (422 tris), y ≥ 17.0;
  - bodies 103/106/107/109: `cyl(r 9.250 hh 2.000)`, y 13.01–17.01, `Layers(3)` (soldier | ordnance);
  - body 105: the front box;
  - bodies 102/104/108: the stairs;
  - body 101: p_vehicle_2 (cyl r 6, hh 8, y 24.6–40.6).
- The terrain at City Hall's centre is 14.04; at lobe 109's east wall the soldier stands at 14.76 (port) and 14.74
  (PS2, §5.1).

### 1.1 Which platform the maintainer was at [V, textures + frames]
The maintainer's inside screenshot has three features:
- a grass floor, which is the terrain (City Hall stands on terrain at 14.04);
- a bamboo wall with arched frames on a large-radius curve;
- a ceiling of radial wooden planks with a ring.

The ceiling's texture is `kas2_bldg_platform_small` (planks, radial straw and a metal ring; decoded from
`assets_baked/bf1_kas2/textures/kas2_bldg_platform_small.ktx2`). `kas1_bldg_hut_grand` combines it with the arched
bamboo `kas1_wookiee_hut_walls`. `kas2_bldg_hut_round`, `hut_square` and `hut_base_low` carry it too. Their
interiors were not tested; their soldier bodies are `coll` meshes, plus 0.45 m posts on `hut_base_low`.
`kas2_bldg_platform_roof`, the hut on the cp5/cp4 decks (y 16.5–17.0 over water), has NO `kas2_bldg_platform_small`
material (its render materials: `kas1_wookiee_hut_walls`, `kas2_bldg_hut_walls` ×2, `kas2_platform_pole`,
`kas2_metals`). The floor inside it is `kas2_bldg_platform_1`'s wooden deck, not grass. The PS2 frames at lobe 109
(§5.1) match the maintainer's outside screenshot: the arches, the dark band and rail above them, the thatch hut beyond. So
The maintainer's hollow is a City Hall lobe.

The LEAK census (13c7843) names a different site as "the maintainer's jump-in": the `kas2_bldg_platform_roof` hut on the deck
at (4.886, 16.518, −7.062). It is a real leak with its own mechanism (§4.1, M3), not the one the maintainer reported.

## 2. The port's mechanism, M1 [V, port code; reproduction]

### 2.1 The code path
- `Sim::step` stage 3 (`core/src/sim/step.rs`):
  - reads the ground at the PRE-step feet, `self.ground_under(pos, vel.y > 0)` (:245);
  - steps the soldier (`Soldier::step`, :253);
  - runs the wall pass `self.collision.push_out(&self.units[i].capsule().stepped())` (:280);
  - moves ONLY the position, `soldier.pos += push` (:281). The velocity is untouched.
- `Capsule::stepped` (`core/src/collision/shape.rs:66–73`): base + `STEP_HEIGHT.min(height/2)`, height − that.
  `STEP_HEIGHT` = 0.6 (`core/src/collision.rs`), so a standing soldier's stepped capsule spans feet + 0.6 … feet + 1.9,
  radius `SOLDIER_RADIUS` 0.5 (`core/src/unit.rs:37`). Its lower sphere's centre is at feet + 1.1.
- `CollisionWorld::push_out` (`core/src/collision/walls.rs:109`), per body and per pass (2 passes, `WALL_ITERATIONS`):
  - takes the deepest contact (meshes per triangle via `mesh_contact`, primitives via parry `contact`);
  - drops a touching one (`dist >= 0`);
  - **drops one whose normal has |n.y| ≥ `WALKABLE_MIN_NY` (0.5) (:149–151)**;
  - else pushes along the normal's XZ part by the depth.
- The ground query `Sim::ground_under` (`core/src/sim/state.rs:562–586`):
  - the probe starts at `feet.y + if rising { 0 } else { STEP_HEIGHT }` (:575);
  - it casts `ray_down` to the terrain floor;
  - `ray_down` (`core/src/collision/rays.rs:55–86`) is NOT solid (`cast_local_ray_and_get_normal(…, false)`,
    `shape.rs:309–`). From inside a primitive it hits the face it leaves through, here the lobe's bottom at 13.007,
    below the terrain and out of the ray's length. So inside a lobe the ground is the terrain.
- `Soldier::step` (`core/src/soldier.rs`):
  - the jump sets `vel.y = jump_vy` and scales the planar velocity by `jump_forward_factor` along the facing (:363–383);
  - in the air the stick steers by `acceleration × ControlSpeed[jump]` (0.03), which is near-ballistic
    (2026-10-02, card AIRC: the port then took that row as a target SPEED too; the PS2's air rules, measured on open
    floor, are `re-stance-roll.md` §4.3a and the port's are `Soldier::air_steer`);
  - gravity: `vel.y -= GRAVITY*dt` (:455) BEFORE `pos += vel*dt` (:457), `GRAVITY` 18.0 (:149).

### 2.2 The reproduction (`lead_tmp/jwal/tools/jwal_probe.rs`, a scratch example on the real bake)
BF1 `imp_inf_storm_trooper` through `game::roster::ClassRoster::build` (MaxSpeed 6.5, MaxStrafeSpeed 4.5,
Acceleration 70, jump_vy 8, JumpForwardSpeedFactor 1.3, sprint ×1.5). Spawn (130.146, 14.649, −121.827), yaw 90°
(heading −x, at lobe 109's east wall x = 123.396), stick full forward every tick, L3 held for sprint, JUMP pressed at
tick 36. `lead_tmp/jwal/trace_sprintjump.txt`, `trace_walkjump.txt`:

| tick | sprint-jump | walk-jump |
|---|---|---|
| 23 / 29 | first push; held at x 123.8959 (0.5 m off the wall), push 0.325 / 0.2167 m every tick, vel unchanged −9.75 / −6.50 | same |
| 36 | JUMP: vel (−12.605, 7.4, 0) | vel (−8.380, 7.4, 0) |
| 36–41 | still pushed, 0.42 / 0.27 m a tick | same |
| 42 | feet 16.0667: **push 0**; x 123.4886 / 123.6283 | same |
| 43 | feet + 0.1 inside body 109 | same |
| 61 | lands on the terrain INSIDE, y 14.697 | lands inside, y 14.738 |

The wall pass's own view of the two poses (`lead_tmp/jwal/contact_t41_t42.txt`, the probe's `contact` mode, walk-jump
feet):
- tick 41 (feet 15.94): the stepped capsule's lower sphere (centre 17.04) touches body 109, depth 0.2654, normal
  (0.9903, 0.1392, 0), so it is pushed back 0.2666 (the sim's own tick-41 push is 0.2654; the second pass
  moves by 2e-4 m between the rounded and the exact pose);
- tick 42 (feet 16.0667, lower-sphere centre 17.1667, 0.16 above the lobe's top): depth 0.2182, normal
  (0.8247, 0.5655, 0). |n.y| 0.5655 ≥ 0.5, so `push_out` returns **0**. The FULL capsule at the same pose would be
  pushed 0.2676 m.

Control: sprinting at the wall without a jump holds at x 123.8959 to the end (`trace_sprint_nojump.txt`,
`entered None`). That is what the maintainer saw: "the bamboo walls stop a walking unit".

### 2.3 Why the ground query cannot rescue it
The rim contact is only a floor if the soldier can stand on the top. The probe starts at feet + 0.6 (falling) and
the feet peak at 16.4067 (the port's discrete apex, 1.647 m above the takeoff 14.76). So the probe reaches
17.0067 < 17.007 and never sees the top. The band where the rim is skipped but not stood on:
- starts when the lower sphere's centre clears the top (feet > top − 1.1);
- ends where the probe would reach the top (feet > top − 0.6).

At this spot the feet are in the band from tick 42 to the landing.

## 3. How general M1 is [V, port]

### 3.1 City Hall (`lead_tmp/jwal/sweep_cityhall.txt`, `jwal_probe sweep 108 -128 26`)
The scripted approaches:
- 72 start bearings round City Hall at 26 m;
- 5 headings each (toward the centre, ±10°, ±20°);
- walk, sprint, walk-jump and sprint-jump, the jumps pressed at ticks 10…60 in steps of 2;
- 110 ticks each.

8,903 of 19,440 runs are LEAK lines: the run ENDS with feet + 0.3 inside a soldier primitive and entered one on
the way (`jwal_probe.rs`, `end_in` and `entered` both set):
- by mode: walk-jump 4,249, sprint-jump 4,654, walk 0, sprint 0;
- by body: lobes 103/106/107/109 with 2,000 / 2,452 / 1,899 / 2,498; the front block 105 with 51; the stairs 104
  with 3 (standing on the ramp, not a leak).

The count is a property of this sample, not of the site. The verifier re-ran other start radii: 24 m gives 10,960,
28 m 7,037, 30 m 5,064. At 24 m (`lead_tmp/jwal/sweep_cityhall_r24.txt`) 4 walk and 4 sprint runs also leak, all up
the south stairs. The traced one (`trace_walk_stairs_leak.txt`: `trace 97.857 -149.751 -145.0 walk 0 110`) is M1
WALKING:
- the soldier climbs diagonally up the stairs ramp (box 104, over the handrail box 102);
- at tick 38 his feet are at 16.367, 0.63 m under the front block 105's top (17.0), just over `STEP_HEIGHT`;
- the stepped capsule's rim contact is skipped; the ground probe (16.967) misses the top by 0.03 m;
- so he falls INTO block 105, lands on the terrain inside it (14.48), and walks on into lobe 107, ending at y 14.04.

(The first version of the sweep counted runs that ever touched a prim, which also caught soldiers walking up the
stairs. The end-state count above is the right one.)

### 3.2 Synthetic band (`lead_tmp/jwal/band_before.txt`, `jwal_probe band`)
Flat ground and one obstacle, near face at x = 0: a solid box 4 m deep, or a zero-thickness mesh sheet. The soldier
runs from x = 8 heading −x, jumps pressed at ticks 10…60. BF1 stormtrooper speeds. The outcome per obstacle height:
- **walk passes** H ≤ 0.75 m and **sprint passes** H ≤ 1.0 m, with no jump, through the box and through the sheet;
- **jumps pass** H ≤ 2.5 m (a few jump timings are blocked from 1.75 m up);
- H ≥ 2.75 m blocks every run.

"Pass" through the box means the soldier ends beyond it after walking inside it on the terrain. None ever ends on
its top.

### 3.3 Census, indicative (`lead_tmp/jwal/prim_band_census.txt`, `jwal_probe census`)
Soldier-layer PRIMITIVES whose bottom is at or under the terrain at their centre, by how far their top stands above
that terrain:
- 1,072 stand 0.6–1.1 m high (the walk-through band);
- 1,279 stand 1.1–2.75 m high (the jump-through band).

These are counted over 32 maps. Examples: tat2 282 / 134, bf1_nab1 135 / 35, bf2_fel1 86 / 349. BF2 COR1, DEA1,
KAM1, MYG1, MUS1, TAN1 and UTA1 have none. The ground beside a primitive may be a deck, not terrain, so this counts
candidates, not leaks. LEAK's census is the measure.

## 4. M2 — tunnelling through a zero-thickness mesh wall [V, port]

A `coll` mesh wall is a sheet. The push-out takes the capsule's deepest contact with it. Once the capsule's axis is
past the sheet, the normal points to the FAR side and the push completes the crossing (`walls.rs` A.4 notes: "a
capsule 0.1 … 0.45 m inside one, stepping 1.0 m further in, ends CLEAN THROUGH it"). From a pose touching the wall,
the axis crosses when a tick's step exceeds the radius 0.5 m, i.e. above 15 m/s:
- BF1 stormtrooper: the fastest is a sprint-jump, 9.75 × 1.3 = 12.7 m/s (0.42 m a tick). No crossing.
- BF2 MaxSpeed 9 (`SoldierConstants::default`, BF2 ctor): a sprint-jump is 13.5 × 1.3 = 17.55 m/s (0.585 m a tick).
  `lead_tmp/jwal/band_before_bf2speed.txt` (`JWAL_MAXSPEED=9 jwal_probe band`): sprint-jumps pass mesh sheets of
  2.75, 3.0 … 4.0 m in 46 of 51 timings. Solid boxes of H ≥ 3.0 m block every run.
- [D] A BF2 roll (RollSpeedFactor 1.7 × 9 = 15.3 m/s, 0.51 m a tick) is just over the line; not run.

The port also keeps the full velocity while pressed against a wall (§2.1), so a pressed soldier who jumps carries
it into the wall. The PS2 does not (§5.1).

### 4.1 M3 — an inward push: the LEAK census's control at the cp5 hut [V, port trace]
The census's own replay is `LEAK_NEAR=4.9,-7.06,14 LEAK_TRACE=1.28,4.66,jump leak_census bf1_kas2` at 13c7843
(`lead_tmp/jwal/leak_hut_trace.txt`). The BF1 soldier walks on the cp5 platform's deck (y 16.705) at 6.5 m/s and
jumps at tick 18. At tick 20 (feet 17.385) the wall pass pushes (0.254, −0.799), 0.838 m in one tick:
- the pre-push position is (2.657, 17.385, 0.343);
- the direction to the hut's axis (4.886, −7.062) from it is (0.288, −0.958);
- the push's direction is (0.303, −0.953), 1° from the axis direction, i.e. INTO the hut.

The census's crossing test sees the soldier's axis cross a face with normal (−0.289, −0.305, 0.908) on that tick.

This is not M1: the push is wall-like and is applied. The deepest contact of the hut's mesh body points inward.
`kas2_bldg_platform_roof`'s collision mesh is a double-skinned flared wall: at local r ≈ 7–10, y 0–5, it has triangles
facing out and down and triangles facing in and up (`lead_tmp/jwal/glbread.py` classification). [H] The capsule
reaches a skin whose contact normal points in. The triangle-level cause is not traced.

The census counts this class as `push`, and M2 falls in it too (the flip past a sheet is a push ≥ 0.25 m). Its
accepted line reads "`* push  # JWAL`", but M1's fix does not touch a wall-like push, so `push` needs its own card
(§7).

### 4.2 The fix — card WPSH (2026-09-29) [V, port]
The triangle behind §4.1, traced with `lead_tmp/wpsh/tools/wpsh_probe.rs` (every penetrating triangle of a pose;
`lead_tmp/wpsh/hut_contacts.txt`) at tick 20's pre-push pose. Body 386 has 16 triangles in contact:
- The deepest is **triangle 5**, the upper flare's INNER skin: depth 0.544, face (0.257, 0.531, −0.808), contact
  normal (0.303, 0, −0.953).
- The stepped axis was 0.205 m behind that face at tick 19 and is 0.027 m in front of it at tick 20, so the push takes
  it further through.
- The outer skin (triangles 9, 10, 16, 22: face n.y −0.54, −0.44, −0.31) leans out over the soldier. Its contacts
  are ceiling-like or shallower, so the capsule rose into the flare unopposed.

`CollisionWorld::push_out_soldier_from(from, full)` (walls.rs), called from `Sim::step`'s wall pass, adds two rules:
- **Sub-steps (M2).** The tick's move is cut into XZ steps of at most `SUBSTEP_OF_RADIUS` = 0.5 radii (0.25 m), each
  followed by the pass. A BF1 walk is one step, as before. There is no cap on the count: the PS2's collision update
  `FUN_00163d08` cuts a move into `int(d / step + 1)` pieces and only snaps a move over 100 m (B2:3393,
  `SWEEP_SNAP`). The cap of 8 it had until 2026-11-11 let a rider carried out of a hangar at a ship's boost speed
  through the shield (`re-flyer-deck.md` §3a).
- **No far-side push (M3), `pushes_past`.** A mesh triangle's contact is dropped when its horizontal push points to
  the far side of the face from where the start pose's axis segment lay. It applies only when the whole segment lay on
  one side, and only when the segment's point at the triangle's mid height projects INSIDE the triangle. Without that
  prism test, BF2 MYG1 soldiers were trapped in crevices (census `float` 43 → 150). A prism margin of 0.25 or 0.5 m
  brings the trap back, so there is none.

Measured:
- **M2.** `JWAL_MAXSPEED=9 jwal_probe band` (`lead_tmp/wpsh/band_after_bf2speed.txt`): sprint-jumps pass mesh sheets
  of 2.75–4.0 m in 0 of 51 timings (was 46). A sheet and a solid box now give identical rows at every height.
- **M3.** The hut replay pushes 0 at tick 20. The axis then crosses triangle 5 unpushed: the body's deepest remaining
  contact is triangle 13's ceiling-like one, and a ceiling-deepest body is skipped (WTUN's suspect).
- **The census's own starts.** The bare pass's "already in contact" start test sees only wall-like contacts. Most of
  the `push` class were starts INSIDE a solid, which the pass then pushed out:
  - TAT2's 7.8 m box primitive at (24.9, −82.0): from inside, the way out is down, a skipped ceiling, until a side is
    nearer;
  - BF2 YAV1's 1 m cylinder platform: a ledge;
  - BF1 BES1's 1.5 m open mesh object at (88.5, 180.5).

  The census now also skips a start whose axis is inside a primitive, or whose stepped capsule touches any SOLDIER
  body by more than 1 cm.
- **Census totals (32 maps).** `push` 48,415 → 1,212, `tunnel` 14,326 → 9,587, `float` 59 → 63. Runs 1,962,285 →
  1,834,290: the skipped starts. The residue is card WPSR (an eave's lower edge, a sloped step, a sliver triangle).

### 4.3 The census's `tunnel` class — card WTUN (2026-09-29) [V, port]
Traced with `lead_tmp/wtun/tools/wtun_probe.rs` (WPSH's probe, plus a box primitive's contact), on the census's
largest tunnel sites after WPSH. Two port mechanisms, both "the pass skips a body because of a non-wall contact":
- **A deeper overhang hides the wall under it (meshes).** BF1 BES1 (94, 198), 95 runs, a walk at body 26: its deepest
  triangle is tri 0, an underside at y 100.65 over the 1.9 m capsule (face n.y −1, 0.236 m). The wall under it, tri 2
  (n (−0.38, 0, −0.92)), is only 0.067 m deep. `push_out` took one contact per body, the deepest, and skipped the
  body as a ceiling. Fix: `mesh_contact(…, walls_first)` returns the deepest WALL-like contact when there is one.
- **A slab leaning over the soldier (primitives).** BF2 DAG1 (−2, −38), 59 runs: a tilted box (body 345, half-depth
  0.43 m) leans over the path. Its convex contact with the capsule is at the lower edge, n (−0.36, −0.66, −0.66),
  ceiling-like, so it was skipped and the axis crossed the face (n.y −0.27). Fix: a ceiling-like contact pushes by
  its normal's HORIZONTAL part times the depth, not renormalised. A flat ceiling (n ≈ −Y) pushes ≈ 0, so heads
  under low decks are not shoved; there is still no vertical response (VCOL). The AI's wall probe
  (`penetration_block`) keeps skipping a primitive's overhang.

Census totals (32 maps, `lead_tmp/wtun/leak_main.txt` → `leak_v3.txt`): `tunnel` 9,587 → 1,427, `ceiling`
75,127 → 37,736, `floor` 10,758 → 7,990, `push` 1,212 → 785; `died` 42,634 → 44,278; `closed` 26,110 → 28,131
(runs that no longer cross; a `closed` run crosses no face at the axis, so these are ray-parity readings, LEAK2 —
tat1 (−238, −2): now held under a rock's edge instead of passing through it); starts 122,286 → 122,271 (the census's
start test uses the bare pass, which now sees the ceilings' horizontal part).

The residue (card WTHIN), sampled: geometry thinner or narrower than the soldier, where one contact per body and two
passes fight each other. BF1 NAB2 (−13, 17): a 0.54 m slot between a hip-high slab (body 28) and a 16 m inward-faced
box (body 836). BF2 POL1 (−118, −146): a prop of 0.2–0.4 m parts (body 837) with 14 triangles in contact, pushing
+Z 0.37 and −Z 0.17 at once.

## 5. The PS2 [V, PCSX2]

### 5.1 The lobes (`lead_tmp/jwal/ps2exp/lobe_jump.py`, `out/results.json`, `out/<trial>.csv`, `out/<trial>.png`)
The setup, one scripted foreground session under ci.lock:
- `ref/states/bf1_kas2_units1_gcw_sideselect.p2s` (walk_bf1.py's no-AI battle: SetUnitCount patched to 1; mission
  `KAS2i`), the two bots parked 300 m up;
- the player (C 0x1e50db8, team 1) held at a start point: C+0x228 and C+0x198 written every 2 ms, §5.3;
- the camera yaw set by right-stick taps read back from RAM, then released for 1.2 s to settle on the PS2's ground;
- then W (run) held and K (Cross = jump) tapped at the trial's time, C+0x228 / C+0xa18 sampled every ~8 ms.

| trial | start → end | outcome |
|---|---|---|
| t1 east, run 1.5 s then jump | (128.44, 14.73) → (124.1157, 14.74, −121.88) | OUTSIDE, 0.720 m off the surface; apex feet 16.660 |
| t2 east, run, no jump | → x 124.1157 | OUTSIDE 0.720 |
| t3 east, jump at 0.65 s (~1 m short) | → x 124.1158 | OUTSIDE 0.720; apex 16.666; max speed 8.5 |
| t4 east, jump at 0.45 s (~2 m short) | → x 124.1158 | OUTSIDE 0.720; apex 16.666 |
| t5 east, repeat of t1 | → x 124.1157 | OUTSIDE 0.720 |
| t6 east, heading 60° | slides along the wall, stops at (121.914, −127.924) | OUTSIDE (0.625 from 109, 0.72 from 103: the corner) |
| t7 east, heading 120° | ran off along the wall to (106.58, 13.81, −106.09) | OUTSIDE (never jumped at the wall) |
| t8 west, lobe 106 | → x 91.8114 | OUTSIDE 0.751; apex 16.497 |
| t9 north, lobe 109 | → z −111.857 | OUTSIDE 0.720; apex 16.559 |
| t10 east, run + jump, 6 s | → x 124.1135 | OUTSIDE 0.720 |

The trace details:
- **Velocity.** t1: pressed against the wall with W held, vx reads 0.0000. The jump starts (0, 8.0, 0), straight up.
  t3: the jump starts at x 124.202 with v (−8.5002, 8.0, 0). The first sample of the next game frame (730.0 ms; the
  721.9 ms row is the same frame) is x 124.1159 with v (0.0, 8.0, 0): the wall zeroed the into-wall velocity in one
  frame. t6: the velocity turns along the wall, e.g. (−2.92, −4.87) after
  (−5.63, −3.24): a slide, until the corner between lobes 109 and 103 stops it.
- **Near the apex.** The soldier creeps 0.045–0.05 m nearer (t1 min x 124.0678, t3 124.0652) at feet 16.52–16.67, then
  back to 124.1159 as he falls. He never gets on top: the apex feet are 0.35 m below 17.007.
- **The frames** (`out/ps2_trials_sheet.png`: t1, t3, t8, t9) show the stormtrooper outside the arched bamboo wall,
  the same wall as the maintainer's port screenshot.

### 5.2 The jump arc (the same CSVs)
- t1: y 14.748 → 15.0148 on the first sample after takeoff (+0.2668 = 8.0/30). After that each step is that
  tick's own vy·dt, and vy falls 0.6006 a tick (g 18.0 at dt ≈ 1/29.97), which is the port's order. The one
  difference is the takeoff tick: the PS2's first airborne step moves at the full 8.0 with no gravity. Gravity
  starts one tick later than in the port, whose jump tick already subtracts it (`grounded = false` is set before
  the gravity line, §2.1).
- The PS2's apex is 14.748 → 16.660 (t1's highest sample), **1.912 m**. Σ(8 − 0.6k)/30 for k = 0…13 gives 1.913 m.
- The port's apex is 1.647 m (trace: 14.76 → 16.4067). Σ(8 − 0.6k)/30 for k = 1…13 gives 1.647 m.
- The forward boost: t3's run speed 6.5003 becomes 8.5002 at takeoff (+2.0, or ×1.3077). The port's ×1.3 gives 8.45.

### 5.3 The RAM teleport (`lead_tmp/jwal/ps2exp/tele_diag.py`, `out/tele_diag.txt`)
At the spawn, the player's position appears twice in C+0x100…0x2c0: C+0x198 and C+0x228.
- A hold that writes only C+0x228 (walk_bf1.py's) at City Hall leaves C+0x198 at the spawn (97.014, −0.02, 303.224).
  Released, the soldier FREE-FALLS through the world (y 15.1 → −2.996 in 1.4 s, vy −25.2).
- The same hold writing C+0x198 too lands him on the ground at 14.722 within 0.2 s.
- A 3 m hop near the spawn with C+0x228 alone also lands, because C+0x198 is updated from it.

[H] The PS2's soldier-vs-world test runs from the previous position (C+0x198) to the new one (C+0x228), a SWEPT
test, which would explain why the PS2 has no M2. Not read in the ELF.

## 6. The AI [V, port]
- `grep -rn "buttons::JUMP" engine/crates/core/src` hits only `soldier.rs:263` (the jump edge) and `soldier.rs:658` (a
  test) outside `sim/input.rs`'s definition, plus `sim/seats.rs:133` (the hover's jump). No `ai/` file presses JUMP, so
  no bot takes M1's jump way in. (The verifier's search over every crate adds only player pads: `input/src/lib.rs`
  and two examples. `ai::think` ORs only FIRE and ROLL_STANCE into a bot's pad.)
- The authored planning graph (`manifest.ron` nav.plan, read with `ps2live.Nav`) HAS hubs inside the lobes' footprint
  at terrain height: Hub167 (107.74, 14.51, −141.18) r 0.99, Hub170 (107.94, 14.06, −136.19) r 1.16 and Hub171
  (107.54, 14.04, −127.39) r 6.41 (City Hall's centre). Soldier arcs 101–102–103 join them to the stairs' foot,
  Hub166 (108.03, 15.19, −147.84). [D] In XZ the route runs up the stairs' centre line (x ≈ 108), where the ramp's top
  (17.04) meets the front block's top (17.0) flush, and the port's XZ arrival puts a bot on the deck. The hubs' y is
  the terrain's, not the deck's. So the route is not a way in, but the graph cannot be read as "no hub inside the
  lobes".
- The WALK band (§3.2: 0.6–0.75 m walking, to 1.0 m sprinting) applies to bots. `ai::steer`'s probe is `wall_sweep`
  of the same stepped capsule (`ai/steer.rs:50`). Its primitive path skips a floor-like cast hit exactly as
  `push_out` does (`walls.rs:411`, and the documented limitation "a graze along a box's TOP EDGE can still report a
  floor-like normal and skip the box"). So today a bot walks through a 0.75 m crate, and its probe calls the way
  clear. A bot that leaves the stairs' centre line (unit separation, a dodge) can fall into the front block from
  the ramp's side, as §3.1's walking trace does.

## 7. The fix — card JWAL, phase build

1. **Ledges (M1).** `push_out` takes the FULL capsule and derives the stepped one itself. Per body, the stepped
   capsule's contact decides as now, with one new case: an UPWARD floor-like contact (n.y ≥ `WALKABLE_MIN_NY`) is a
   LEDGE. Its surface is above feet + `STEP_HEIGHT`, which the ground query can never stand on. For a ledge, the body
   is re-asked with the FULL capsule:
   - for a primitive, the convex contact;
   - for a mesh, the deepest contact over the triangles whose FACE normal is a wall (the pattern
     `penetration_block` and the A.3 cast already use).

   A wall-like answer pushes as usual. A floor-like answer is a real step or landing (the feet are within the
   penetration of the top), and the ground query lifts him. Downward contacts (ceilings) stay skipped.
   - Kerbs under 0.6 m never touch the stepped capsule, so they stay climbable.
   - At §2.2's tick 42 the full capsule pushes 0.2676 m.
2. **The velocity (PS2 §5.1).** After a non-zero push, remove the velocity's component INTO the push direction
   (horizontal, only when v·n̂ < 0); the tangential part stays (the PS2's slide). A jump against a wall then goes up,
   as on the PS2.
3. **The AI's probe.** `wall_sweep`'s primitive cast and `penetration_block`'s primitive branch get the same ledge
   rule. The probe then agrees with the new push-out and a bot no longer sees a 0.75 m crate as open.
4. **Tests.** Core synthetic tests:
   - a 0.75 m box and a 0.75 m sheet stop a walk and a sprint;
   - a 2.25 m box stops every jump timing;
   - a 0.4 m kerb is still climbed;
   - a jump onto a 1.2 m box lands on top;
   - a pressed jump is vertical.

   A game test replays the maintainer's approach on the KAS2 bake (walk, sprint, walk-jump, sprint-jump, several headings
   round each lobe; skipped when the bake is absent) and asserts every end is outside. LEAK's census is the check on
   the 32 maps.
5. **Declares:** `sim` (every unit's wall pass changes near ledges and walls) and `sweep` (bots stop walking through
   0.6–1.0 m primitives). No golden should move; the picture tests do not step a sim. The pinned `--sim-hash 100`
   (`sim/state.rs`, `Sim::new(42)` idle) touches no wall.

The census's classes after this fix:
- `tunnel` (WTUN's class; its suspect is M1's mesh form, a body dropped because its chosen contact is sloped) should
  fall to a residue;
- `push` (M2, M3) should not move.

JWAL's landing re-points the accepted list: `tunnel` goes to the residue's card, and `push` to WPSH.

**Not in JWAL:**
- **WPSH (M2 + M3, the census's `push`):** the wall pass must never move the soldier's axis across a face:
  - a swept horizontal step, or sub-steps when a tick's XZ step exceeds half the radius, so BF2 sprint-jumps and
    rolls cannot cross a mesh sheet (M2; [H] the PS2 tests swept, §5.3);
  - no push toward the far side of a face the axis has not crossed (M3, the cp5 hut).
- **JARC:** the PS2 BF1 jump rises 1.91 m (no gravity on the takeoff tick) with a +2.0 m/s forward boost
  (6.50 → 8.50); the port's rises 1.65 m, ×1.3. It moves which ledges a jump can reach (§5.2).
- **SRAD:** the PS2 soldier stops 0.62–0.75 m from a primitive's surface; the port's radius is 0.5 (§5.1). [H] Its
  shape is not a plain 0.5 m capsule.

## 8. Checks run, and what stayed [H]
- No claim here rests on a decomp reading. The port claims cite port source (read at `9274979`). The PS2 claims are
  measured on PCSX2: CSVs, the kept frames, `results.json`, `tele_diag.txt`. So `re_check.py` fn/absent/word/writers
  have nothing to check. The absence claim (§6, no AI JUMP) is a whole-tree `grep`.
- One foreground Opus verifier on the claims and their evidence files (§9).
- Stayed [H]:
  - the PS2's swept soldier test (§5.3);
  - the PS2 soldier's collision shape (§7 SRAD);
  - the PS2's walking step height: not measured; no 0.6–1.0 m primitive was walked at on the PS2;
  - the BF2 roll's tunnelling (§4).

## 9. Verifier
One foreground Opus verifier had the eighteen claims and their evidence files only. It re-ran every probe command,
leak_census's trace and the GLB reads, and searched every crate for JUMP. Result: **15 SUPPORTED, 2 PARTLY,
0 NOT SUPPORTED.** Folded in:
- C9 (the City Hall sweep): the count depends on the start radius (24 m 10,960 with 4 walk + 4 sprint leaks, 26 m
  8,903, 28 m 7,037, 30 m 5,064). §3.1 now quotes it as a sample, and the walking leak it found is traced there.
- C13 (the jump arc): "explicit Euler" was wrong. After takeoff the PS2 steps in the port's order; only its takeoff
  tick skips gravity. The apex numbers stand (§5.2).
- Minor, fixed:
  - the tick-41 push is the sim's 0.2654 (0.2666 at the rounded pose);
  - t3's zeroed velocity is first seen at 730.0 ms;
  - of the ten PCSX2 trials, eight jump at a lobe (t2 has no jump; t7 never reached the wall);
  - the two landing heights are labelled by mode.
- After the verifier, §6's "the planning graph cannot route through a lobe" was checked and found WRONG: three hubs
  sit inside the lobes' footprint at terrain height. §6 now gives them, and the route up the stairs.
