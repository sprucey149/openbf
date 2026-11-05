# Space Assault: scoring, capital ships and linked systems

Written 2026-11-04 on `feature/space-assault-109`, brought up to `a3f5c192` on 2026-11-05. Evidence is the PS2 BF2 executable's decompiled
listing and the PS2 mission bytecode (`bf2/mission/*.luac`, console Lua tree `32a5d83a...6e54d`), read statically. No live
PS2 comparison is claimed. `core::sim::space_assault`, `game::space_assault` and `scripts/gen_missions.py` carry it.

## 1. The round

- **Score limit.** `FUN_0018e6e8` reads a table of five words, all 180, then a zero (`DAT_005fa898`). Instant Action's
  `iASSScoreLimit` (`session.ass_score`, default 180, 50..1000) replaces it for a shell round.
- **Winning.** `objectivespaceassault.luac` proto `main.2.0`: when team points change and the objective is not complete,
  a limit at or under the team's points completes the objective for that team and releases the watcher. Port:
  `Sim::space_add_points` sets the outcome once and pushes `TeamDefeated` for the other team.
- **Tickets.** The scripts ask for infinite reinforcements (`-1`, written `999999` in RON). `Battle::on_death` takes none
  while `unlimited_reinforcements` is set, and `update_cps` skips bleed, handicap, domination and the time limit in a
  Space Assault round. A hangar post takes no captures (`counts_in_cp`).

## 2. What scores

`FUN_003616f0` line 346075: with Space Assault enabled (`DAT_005fee38+0x621`) every score kind also goes to the team's
points through `FUN_0036ef08`, which adds without a floor (line 355520). An AI killer whose award is under 1 adds
nothing. The kinds' values come from `ingame/points.luac` (`re-event-messages.md` §9.1).

**Flyer kills.** `FUN_00269a08` line 185628, from the flyer death paths `FUN_0025f4a0` line 179236 and `FUN_0021b470` line
131331: no award when the vehicle's team is the killer's; an empty one is kind 13 (+1); else `FUN_00362ee0` line 347199
picks kind 5 (+3) for a killer whose own vehicle is not of `VehicleType` 1 to 3. Every space flyer is a fighter or a
transport, so a manned enemy flyer is +3.

**Critical systems.** `FUN_0018c848` does nothing with no killer. `FUN_0036adb8` (line 352799) returns the system's
registered points, negated when the object's team is the killer's. A bot therefore gets nothing for its own ship and a
human loses the points. `KillObject` (`FUN_0018fcd0` to `FUN_001df3d8`) has no killer, so linked deaths score nothing.

### Deviation from the disc, by the issue's specification

The issue asks for 3 a fighter kill, 18 a system, and nothing else. The disc also credits team points for infantry kills,
a pilot's own death (+1 AI, +2 human), and penalties for humans (self -1, teamkill -2). These are not built.
`replace:` send every `FUN_003616f0` kind to team points if the maintainer wants the disc's full table.
The disc's +1 for an empty flyer and -18 for a human on their own ship are built, because the issue says nothing against
them.

## 3. The point table

`gSpaceAssaultSystems_template` in `objectivespaceassault.luac` is keyed by the attacking side, so the generator reads
each mission's value from it rather than a table of ours. `SetupAllCriticalSystems` gives internal
systems the 18 in its first constant.

| attacker | engine | engines on target | total |
| --- | --- | --- | --- |
| imp | 3 | 6 (Rebel) | 18 |
| all | 6 | 3 (Empire) | 18 |
| rep | 9 | 2 (CIS) | 18 |
| cis | 18 | 1 (Republic) | 18 |

Life support, bridge, comms, sensors, each frigate and each internal console are 18. A dead system's `MaxHealth` becomes
99999997952 (closure `main.1.0`), so a repair can never complete it: `SPENT_MAX_HEALTH`.

## 4. Linked objects

**`linkedshields.luac`.** Defaults: `maxShield` 200000, `addShield` 1000. Only Yavin overrides, with 500000. Damage to any
listed object copies its shield to all of them; at zero the refill stops, and the first time it also kills the controller
object. Killing the controller zeroes all and stops the refill. A controller that returns restores the full pool and the
refill. Port: `ShieldGroup`, `Sim::space_station_damaged`, `tick_space_assault`. The bytecode divides nothing by the object
count, which corrects `space-state.md`.

**Refill law.** `FUN_001df560` line 90351: health and shield each move by rate times seconds toward their maximum. The
hull classes' `AddHealth` (50000) goes through it, as `SpaceAssault::regen`. Not refilling a destroyed object is ours:
`trace:` whether that update runs on a dead object.

**`linkeddestroyables.luac`.** When every object of one set is dead, each live object of the other sets is killed.

**`linkedturrets.luac`.** A dead mainframe puts its turrets on team 0; a returned mainframe gives them back.

**Frigates.** `OnObjectKillName` callbacks kill the frigate's turrets and hide them. Yavin names them; the `_cmn` scripts of
spa6 to spa9 build `<frigate>_tur<i>` in a loop. The generator keeps these callbacks out of the mission's `killed`, which
wrongly listed them as dead at the start.

## 5. Objects in the port

An audit of all six maps found every declared critical system, controller and mainframe as a placed instance:
`destructablebuilding` (consoles, generators, frigates) or seatless `armedbuilding` (bridge, engines, comms, sensors,
mainframes, hulls). The first were already `Station`s. `game::space_assault` gives the second a `Station` too, only for
names a mission declares and only in Space Assault, so no ground map changes. Three names have a `prop` base and no
health (`sensors_spin`, `sensors_spin0`, `imp_cap_stardestroyer_shield1`) and are left out. `Sim::building_down` also
answers for a destroyed station, so `FallenDraw` shows the armed building's wreck.

## 6. HUD

`player1.team{1,2}.points` and `.pointsText` drive the disc's own text. The disc draws ship bitmaps and system icons in
`player1spaceassaultgroup` (viewport 0.738, 0.072; `FUN_0036b000`). In its place, `ui::hud::draw_space_meters` draws shield,
hull and points bars under the minimap, whose corner that viewport rectangle would cover. OURS, asked for by the issue. `replace:` the disc's procedural icons. Hull has no
disc equivalent: it is the summed health of a ship's critical systems over their full health.

## 7. Not built

- Objective messages and voice (`level.spa.hangar.<system>.atk.down`, the `attVO` and `defVO` keys), and `EnableLockOn`
  on a dead shield. `trace:` `UpdateObjectiveMessage`.
- `RespawnTime` on buildings: the port's building rule is no self-repair (`building_turrets.rs` header), so a generator or
  mainframe returns only through a fusion cutter.
- Bots attacking capital ships, and bot gunnery in general. See section 8.

## 8. Measurements

```
target/release/game.exe --sim-trace <out> 36000 --map assets_baked/bf2_<map> --mode ass \
  --mission missions/bf2_<map>_ass.ron --seed 7
```

Twenty minutes each, seed 7, the player idle, on the working tree over `a3f5c192`:

| map | points | outcome |
| --- | --- | --- |
| spa1 | 0, 18 | none |
| spa3 | 0, 0 | none |
| spa6 | 15, 0 | none |
| spa7 | 0, 0 | none |
| spa8 | 3, 6 | none |
| spa9 | 0, 9 | none |

A round of bots alone does not reach 180. The objective itself ends a round (`game` test suite, `space_assault::`), and a
human can reach the limit: one enemy ship and its frigates hold 108 to 144 points once linked deaths are counted once (spa1 and spa3 108, spa7 to spa9 126, spa6 144), and fighter kills cover the rest. This
is the path followers' combat, which `re-flyer-ai.md` lists as unconfirmed (`PathFollowerFlyerVsFlyer`, `0x2fd930`).
`measure:` these rows again once that is traced.

## 9. Checks

On the working tree over `a3f5c192`, after a full `scripts/pack.py` bake from the two PS2 discs. The comparison build is
`a3f5c192` from a clean worktree, on the same bake.

- `cargo test --no-fail-fast --manifest-path engine/Cargo.toml`: 4307 passed, 5 failed, 196 ignored. `game::space_assault`
  holds 18 tests, `core::sim::space_assault` 16, `game::walk::flyer_camera` 11.
- Four failures also fail on the comparison build: `roster::aim_sizes_are_the_baked_models_boxes`,
  `roster::fill_classes_keep_the_v0_fixtures`,
  `squad_vehicle_maps::move_out_and_at_ease_reach_real_waiting_pilots_once_per_press`,
  `hud_layout::the_squad_cross_is_where_the_ps2_frames_have_it`. The fifth,
  `nav_walls::mygeetos_position_table_is_built_in_under_a_third_of_a_second`, misses its CPU limit under a loaded run and
  passes alone.
- `python -m unittest discover -s scripts/tests`, Windows: 47 failures, 33 errors and 6 skipped on both trees.
- Ground: `--sim-hash 600` is `5c5458c8927f122b` on both. `--sim-trace ... 2400 --seed 7 --state-hashes` is identical at
  every tick on `bf2_nab2` (`0dd9fc1622b97d3b`), `bf1_tat2` (`b38c43f7dbc59005`), `bf2_kam1` (`8f0a983dfffd25f9`) and
  `bf2_tat3` (`22a208d7dcec0130`). `bf1_hot1` differs on purpose, `9926ba86b3f90c05` against `3e5c4a4f50c4285e`, from
  tick 697: its flyers now use `Layers::VEHICLE` (§10). `scripts/sim_gates.sh`: green.
- Generated ground and Hero Assault missions are byte-identical on both; only the six `bf2_spa*_ass.ron` differ.
- `scripts/size_report.sh`: no file this work grew passes 1,500 lines, except `bake/src/map/content.rs` (1,699, waived).
  `vehicle_class.rs` and `manifest/src/tests.rs` had their tests moved out to stay under.
- `dist/openbf.exe --headless --hud --walk-replay` on spa1 and spa6 draws the meters, a pilot and the minimap.
- The field: idle replay on spa1, frames 1 s apart. The field's pixels change by 5 to 11 grey levels on average while a
  wall in the same frames changes by under 0.05, so it scrolls.
- Bots leaving: 5400 ticks, seed 7, a segment test per tick for each crewed craft in the flying state. Every craft that took
  off left 200 m from where it took off: 16, 17, 17, 19, 13 and 13 craft on spa1, 3, 6, 7, 8 and 9. The segments that met a
  static vehicle body number 4, 95, 4, 18, 0 and 4, out of 55,000 to 72,000. All were on a ship's outside, none in a hangar:
  the nearest to a pad was 150 m over spa7's own hangar. These are BF2's AI curve, which takes no contact (B2:117417,
  OURS, in `Sim`'s flyer step). `measure:` whether the disc's routes clear those hulls once the original's
  avoidance is read.

The white frame round a hangar opening and the bright floor plates were first flagged here as a possible fault. A PS2
frame of the same hangar shows the same white frame, so they are not listed as a defect.

Not checked: a person flying a round in a window, or a round won through the shell.

## 10. What a first play of the build found

Six reports on the packed build, 2026-11-04. Lines are `SWBFRNT2.ELF-decomp.c`.

### Doors

Every map holds ten `door` instances, all unlocked in Assault. All of them open for a walker and let it cross
(`every_door_opens_for_a_walker_and_lets_it_through`). One side of each ship's shield generator door stops a walker 4.3 to
4.7 m short, in the room's own geometry, so the test asks for one crossing side per door.

The arches seen from the server room are not doors. They are part of the static model `imp_cap_stardestroyer_server`,
which uses the material `spa1_prop_impdoor`, and no door instance stands there, so they cannot open on any build reading
this data. The server room's way out is `Impdoor03` at (766.7, 217, -96.6) on spa1.

The mission scripts of spa6 to spa9 start `LockHangarWarsDoors(false)`, a function in the map's `_cmn` script. With false
it locks nothing and disables six barriers (`all_cap1_bar1`..`3` and `imp_cap1_bar1`..`3` on spa8 and spa9). The generator
followed only calls with no arguments, so those barriers stayed on for bots. `gen_missions.py hangar_war_calls50` now runs
the arm the literal selects. spa1 and spa3 name their barriers directly in `ScriptPostLoad`.

### The opening to space

Two separate faults.

The field's scroll was read from the wrong byte. `FUN_00590328` (lines 711824, 711958-711959) reads the two rates with two
four-byte reads, each kept as a byte, so they sit at MTRL offsets 16 and 20. The port read 16 and 17. Every material with
a second rate lost it. `FUN_0058e930` (line 710885) turns them into `-rate * time / 255`. `spa2_bldg_shield` holds 0 and 60,
so its V scroll is now -60/255 per second, and its low-resolution sheet 89/255. `re-material-flags.md` corrects its note.
This reaches every BF2 map that has such a material, so all of them are rebaked.

The sky was missing, which left the translucent field in front of a clear colour. `SPA/SPA_SKY.LVL` holds one `lvl_`
sublevel per planet, and the missions load it after the map (spa1 `yav`, spa3 `kas`, spa6 `myg`, spa7 `fel`, spa8 `hot`,
spa9 `tat`). `FUN_00177058` hands each `sky_` chunk to `FUN_003be920` (line 404931) in load order: it sets `SkyInfo` fields one at a
time, appends `SkyObject`s and passes `DomeInfo` to `FUN_003bcd50` (line 403853), where each `DomeModel` takes the next slot
(`+0x38`, line 404143). So the second chunk adds to the first. `Sky::absorb` does the same, and `bake map ... --sky-sublevel
<lvl> <name>` adds the sublevel's chunks as a layer. A model named `spa1_sky_ nebula22` has that space in the disc's data
on both ends.

`every_space_map_has_its_sublevels_sky` holds this. Which part of the sky fills a given opening has been compared with one
PS2 frame only, and that frame's map is not known.

### Stacked ships

`FUN_003725f0` (line 357813) builds a box from the class geometry bounds at pad `+0x110`, and before it arms or counts down
it tests each live vehicle in the pad's list against that box with `FUN_00536f10` case 2 (`FUN_005381a8`: per-axis excess of
the sphere centre over the half extents, summed as squares against the radius). An overlap returns from the step. The port
had no such test, so a SpawnCount 7 pad put a new ship on the old one every 5 s. `Sim::pad_box_taken` is that test, with the
class's model box (`ManifestVehicle::vis_box`) and the vehicle's bound sphere. `re-vehicle-rest.md` records the separate
second gate, `FUN_00161b50` for hovers and walkers, which remains unported.

### Leaving a flying ship

`FUN_00202d08` (line 115783, exit test lines 116984-117024) sends the exit button to the exit logic only in flight state 0,
landed, or for a class with bit 1 of `+0x1208`, which property `0xc6e9c80b` sets on guided rockets only. A further float at
`+0x188` also forces it, and that field has not been read. `FUN_0024ba68` itself refuses nothing, and a gunner's update
(`FUN_001ee668`) has no flight gate. `Sim::pilot_held_in_flight` holds a BF2 pilot in a flyer that is not landed. BF1 was not
read, so its rule is unchanged. AI exits stay as they were: `FUN_002baa20`'s request is spent in `FUN_002bbf00`, which is not
this path. `trace:` the AI callers' `+0x68` test.

### The shield entrance

Fields say `VehicleCollision CLEAR`: `imp_cap_stardestroyer_shield` and `cis_fedcruiser_shield_blue` on all six maps. Under
`re-collision-layers.md` §2 that empties the vehicle subset, so a ship passes and a soldier and a bolt do not. The port had
no vehicle subset and flyers queried the soldier one, so to them a field was a wall. `Layers::VEHICLE` is now built from the
same rule and flyer contact and pilot rays use it (`ships_pass_every_hangar_field_and_soldiers_do_not`).

### Bots flying through the wall

Three causes, measured with a per-tick segment test against the vehicle subset for every flying craft.

- Stacked hulls refused every route out, so `connect` took its fallback.
- That fallback was `.or(ranked.first())`, which flew the best route even with a chord blocked. `FUN_002ff478` (line 279157)
  has nothing like it. Failed candidates leave the list, up to four tries; then it returns its separate best geometric
  candidate, which passed its own two chords, or a null path, and `FUN_002fc550` resets the follower through `FUN_002fa478`.
  The port's fallback is gone. `measure:` the geometric candidate is still not ported, and the craft now holds and retries.
- `transport` follower classes have no route on spa1, spa3, spa6 or spa7 (the disc's route classes: `imp`, `rep`, `all`,
  `cis` and their `_flag` forms; only spa8 and spa9 name `transport`). The port flew those pilots on its own loiter circle
  through the hull. `can_fly` now refuses a BF2 class no route names, so they stay parked. OURS, with no PS2 read behind it:
  `measure:` a PS2 round on spa1, whether a bot boards a landing craft. `FUN_002faf90` holds a relocation path that was not
  followed to its callers and may belong here.

The loiter circle is flown only when its own chords are clear, otherwise the craft holds. That is OURS, and
`re-flyer-ai.md` keeps its `measure:`.

### The pilot's camera (BF2 `FUN_0020d450`, BF1 `FUN_001de278`)

Lines are `SWBFRNT2.ELF-decomp.c` unless marked. The port's old seat camera used a heading frame, which dropped the ship's pitch and roll.

- Frame: the drawn matrix with the visual tilt taken off (lines 122518-122519), the same as `flyer::untilted`. Aim direction is that frame's forward (line 120058 on).
- `FUN_001cef68` (line 77650): back is minus the aim, right is up cross back, so the view keeps the frame's up and rolls with the ship. The eye is frame times `TrackCenter` plus view times `TrackOffset` (`FUN_001ce948`).
- The eye's step from last frame's eye is taken into the view's axes and each axis is scaled by 1 - exp(-tension dt), with its own tension per sign (lines 78033-78090). A centre over 250 m from the eye snaps (62500 squared).
- BF2 replaces two section values on a copy (lines 122639-122666): no aim lag, and tilt 0.087665 rad. BF1 passes the class section as it is, so its `AimTension` and `TiltValue` apply. BF1 was read only by structure. OURS, `measure:`: BF1's aim lag blends with a normalised lerp, not a true slerp, so the camera stays within the deterministic math the `game` crate's gate requires.
- Disc values for `imp_fly_tiefighter_sc`: `TrackCenter` 0 5 -8, `TrackOffset` 0 0 0, `MoveTensionX` 2.7, `MoveTensionY` 2.7 10.0, `MoveTensionZ` 4.25.

Not ported, each `trace:`: the look-at blend while `+0x660` bit 1 is set (`+0x638`, a manoeuvre flag), the target view set by `FUN_00201b08`, the viewport `+0x90` value, the `FUN_00563f78` == 2 scaling, and the horizontal reticle shift. OURS, `measure:` a PS2 boarding: the camera starts at its settled place on boarding, because whether the PS2 keeps the soldier's camera matrix there was not followed. The port also keeps its own collision pull in front of the lag.

Hovers and walkers go through the same PS2 function and keep the provisional `vehicle_camera`.

Seen in frames: `dist/openbf.exe` on spa1, a replay boarding `imp_fly_tiefighter_sc` from 4 m, taking off, flying out through the field, then banking at minimum throttle. The ship sits under the reticle and the horizon rolls with it, as in the PS2 frame the report supplied. At full throttle the ship falls about 30 m back, which is the `MoveTensionZ` lag at 105 m/s. `measure:` that distance against a PS2 frame at a known throttle; the report's frame is at an unknown one.

`Layers::VEHICLE` moves flyer rays, so `bf1_hot1` state hashes change from tick 697: reverting only that restores 9d019cd7a87ac27b. Bodies only in the vehicle subset also grew `building_sphere`; that now counts soldier and ordnance bodies only.
