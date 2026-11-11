# Space Assault: scoring, capital ships and linked systems

Written 2026-11-04 on `feature/space-assault-109`, brought up to `a3f5c192` on 2026-11-05. Evidence is the PS2 BF2 executable's decompiled
listing and the PS2 mission bytecode (`bf2/mission/*.luac`, console Lua tree `32a5d83a...6e54d`), initially read statically.
Sections 10–11 add bounded Space Hoth save-state and play comparisons; full-round original-game acceptance remains open.
`core::sim::space_assault`, `game::space_assault` and `scripts/gen_missions.py` carry it.

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

### The whole table (2026-11-05, maintainer's choice on #123)

The port now sends every score kind to team points, as the disc does; the earlier deviation (fighters and systems only)
is gone. `FUN_003616f0` line 346075 holds the table; an AI killer's award under 1 is ignored. `FUN_00361698` adds a killer
with a team and no player, positive awards only (lines 333939-333950, 185701-185716). Kinds `FUN_00362268` line 346704
and `FUN_00362c60` line 347113 add the post-defence awards; the flag list `DAT_0066138c` holds nothing in Space Assault and
hangar posts are not captured, so those branches do not fire on these maps.

What a round can see: AI soldier +1, human +2, hero +5, a kill within 20 m of a post +2, self -1, teammate -2, an armed
building kind 0xd (`FUN_001e8498` line 96214) +1. A flyer kill is +3 for the craft and +1 (AI pilot) or +2 (human pilot) for
its pilot. Kind 0x1c arrives through Lua (`FUN_0018c740`), the system points in the section above.

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

`player1.team{1,2}.points` and `.pointsText` drive the disc's own text. The interim bars are gone: `ui::hud_space` draws the
disc's own picture in `player1spaceassaultgroup` (viewport 0.738, 0.072, size 0.2007 by 0.2011).

- Layout `FUN_0036b000` line 352920, colours each frame `FUN_0036a2a8` lines 352500-352730. Own ship in the right half, the
  enemy's in the left, shields over them, a system icon 0.15 of the group wide and high, centred on the script's `u`, `v`
  inside the ship's half.
- A system or ship colour runs red (200,100,100), yellow (223,223,32), green (100,200,100) by health (`FUN_0036b900`;
  colours `FUN_0057ca38` lines 700241 onward). A shield is r=(1-f)255, g=f200, b=f150, a=f255. Either flashes to white over
  1 s when it falls. A dead system is black. A linked group shows only its first entry (`FUN_0036ad20`) and reads the
  group's fraction (`FUN_0036b600`).
- `trace:` a dead system is read from the first part's `destroyed`. The original tests object+0x228 bit 3, whose meaning is
  inferred, not traced.
- The bake dropped alpha from the five icon textures, which drew as squares. `consumer_named` in `bake/src/map/content.rs`
  now keeps it for `*_icon_ass`, `*_icon_ass_shield` and `spa_icon_critsys`.

The small minimap is off in space (`DisableSmallMapMiniMap`), and the port draws no map there except on the spawn page.
`trace:` the Large view on SELECT is not ported (card BMAP2 resume), so the disc's way of seeing every fighter on a map is
missing. `bSoldierHints` (`FUN_0017c698`, `re-hud-attacker.md` §2) defaults off and only draws on-screen rings; not ported.

## 7. What was open here, now answered

- Objective messages and voice are built; see section 13.
- `EnableLockOn` and fighter lock-on are built; see section 14.
- **`RespawnTime` on buildings.** Key 0xbd94b44e; the disc's regeneration is MaxHealth over RespawnTime (`re-build-zones.md`
  lines 74 and 83). In the space data, capital systems, generators (4500 health) and terminals (5 health) use 999999, frigate
  turrets too, so they effectively never come back on their own. Only `imp_bldg_boxturret` and
  `all_bldg_moncal_roundturret` are short: 2500 health over 120 s. The port's rule stays the maintainer's (2026-10-28,
  `building_turrets.rs` header): no self-repair, only the fusion cutter. A generator or mainframe therefore returns through
  repair, as on the disc in practice, and those two turrets do not regenerate. `measure:` confirm on the PS2 that a boxturret
  regenerates in a played round.
- Bots attacking capital ships: see section 15.

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
`+0x188` also forces it, and that field has not been read. `FUN_0024ba68` itself refuses nothing. The gunner's hold is in
the mounted-turret seat tick `FUN_0026cea0` (B2:189079-189087): when the seat's owner passes the flyer type test
(`DAT_00689368`, the same type `EnterVehicle` and the takeoff and land actions test) and its flight state at `+0x60c` is not
0, the exit is skipped. The passenger seat tick `FUN_00271b28` (B2:192404-192440) exits on use, or when the seat is disabled,
with no flight test. So in a BF2 flyer that is not landed the pilot and the gunners stay, and a passenger may jump, in every
mode. `Sim::rider_held_in_flight` follows that on ground maps. In Space Assault and space CTF it holds passengers too, a named
deviation since there is no ground under them; `measure:` a PS2 space transport passenger pressing exit in flight. BF1 was
not read, so its rule is unchanged. AI exits stay as they were: `FUN_002baa20`'s request is spent in `FUN_002bbf00`, which is
not this path. `trace:` the AI callers' `+0x68` test.

### The shield entrance

Fields say `VehicleCollision CLEAR`: `imp_cap_stardestroyer_shield` and `cis_fedcruiser_shield_blue` on all six maps. Under
`re-collision-layers.md` §2 that empties the vehicle subset, so a ship passes and a soldier and a bolt do not. The port had
no vehicle subset and flyers queried the soldier one, so to them a field was a wall. `Layers::VEHICLE` is now built from the
same rule and pilot rays use it (`ships_pass_every_hangar_field_and_soldiers_do_not`). A craft's own contact now uses `Layers::FLYER`, §11.

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

## 11. A second comparison on Space Hoth

The user compared `bf2_spa8` with the PS2 game: nose-down flight took the ship off the top of our screen, a ship could
enter and ride along the destroyer's walls where the PS2 destroys it, and hitting a hangar floor or ceiling did nothing or
landed the ship where the PS2 pushes and damages it. Save states 9 and 10 were read from copies only; lines are
`SWBFRNT2.ELF-decomp.c` unless marked.

The user's save runs the one-flag layer, not assault: RAM holds `spa8g_1flag`, `spa8_1flag`, `ctf_imp1_sp4` and
`ctf_imp1_cp`, with no `ass_imp1_*` and no trooptrans string. Its pad sp4 is an interceptor. The port's `ass` layer follows
the disc, where sp4 is `imp_fly_trooptrans` and the interceptor is on sp2, so the two pads differ for that reason.

### The camera

`FUN_0020e1b0` (lines 122921-123085, reached through the thunk `FUN_001ea1e8`) blends the view by speed. With `f =
clamp(forward speed / max(top speed, boost speed), 0, 1)`, the FOV is `(1-f) ThirdPersonFOV + f FastFOV` and the track offset
is `lerp(SlowOffset, FastOffset, f)`. Top speed comes from `FUN_00202c50`: the class maximum, capped at 60 inside a slow
region. The FOV eases at 25/s (lines 64893-64896). The three props have no known names; brute force found none. They are
`0x5dbcabb0`, `0x8d0cfc36` and `0xaaa6d95c` on the flyer's class. BF1 has none of them.

Save state 10 settles it: interceptor FOVs 38 and 90 at f = 80/170 give 62.47 degrees, and the camera struct at `0x1759720`
holds a projection of 1/1.6489. The ship sits at (-0.20, -4.50, -18.50) in view space in that state; the port's level
flight has (0, -4.25, -18.57).

The nose-down fault was in the flight step, not the camera. The pitch tilt took the y part of omega cross local velocity,
where both games take the z part (machine code `0x20680c`, line 117966; B1:85870; `re-flyer.md`). At full stick the old
term tilted the nose 0.34 rad, the velocity followed the tilted nose (line 117869 uses the drawn forward row), and the
ship left the frame. In save state 10 `omega.x` -0.056 at 80 m/s holds a tilt of 0.0002; the y part would give 0.021.

A held dive from a replay, in view space, against the half-height of the frame at that speed's FOV:

| Throttle | Speed | Ship y, z | Height in frame |
|---|---|---|---|
| Minimum | 35 | 1.68, -15.67 | 31% of half-height |
| None | 80 | 4.11, -17.49 | 50% |
| Full | 105 | 5.36, -18.53 | 54% |

Before the fix the 80 m/s row was 11.75, -13.49, off the frame.

`measure:` the PS2 shot has the ball about 92% of the way up (tan about 0.43), higher than any row above, and its speed is
unknown. One frame cannot separate speed, stick and timing. A save state taken while holding the dive would give the
numbers directly.

### Hull contact

Replaces the old `flyer_contact`, which never damaged: it passed a positive closing speed to `impact_damage`, which returns 0
for those.

Which bodies meet a craft (`FUN_00169328`, lines 7790-7870): the model's byte +3, from prop `0xF31D1478` (lines
145727-145728, B1:109208), picks the vehicle subset when set and the soldier subset otherwise. The bake carries it as
`flyer_uses_vehicle` and builds `Layers::FLYER`. A craft tests its building-subset primitives against those bodies and its
terrain-subset primitives against the terrain's height (`FUN_0016c6c0`, line 10624 on). The deepest overlap wins. Movement is
sub-stepped by the bound radius (`FUN_00163d08`, lines 3385-3440); RAM holds 4.77 for the interceptor. `measure:` the port's
`bounding_radius` against that value.

The response (`FUN_0020bff8`, lines 121489-122203), with `s` the relative velocity along the normal:

- Takeoff: another flying craft gives no response. A normal with y below -0.7 sends the craft to Landing.
- Moving apart: push only.
- Damage is `impact_damage(s, n.up, n.fwd, T, S)`. BF2's flying state adds `max(-n.fwd, 0) * 0.5 * MaxHealth`, so a nose-on
  hit costs half the health on top. Another flyer, unless this one is a commandflyer, sets it to `0.2 * MaxHealth`. Another
  vehicle caps it at half of that vehicle's maximum.
- Dead stops the response. Falling destroys the craft (lines 121913-121930).
- Bounce: `j = -1.4 s`, halved against a flying craft (`trace:` the original compares class and uses real masses). Tangent
  speed shrinks by `min(j * 0.125 / |vt|, 1)`.
- Landing only with `n.y > 0.7`, `n.up > 0.9375` and lift below 0.0001 (lines 121985-122000). Otherwise the craft jolts:
  `f = min(1, |v| * 0.025 + 0.01)`, forward bends toward the normal by `0.3 f` and toward the velocity by 0.3 (lines
  122029-122090).
- Position moves out along the normal by the depth.

Replays on spa8's `ass` layer, interceptor from pad sp2, Flying state in all of these:

- Into `spa1_prop_minidestroyer2` (`imp_frig3`) at 105 m/s: the craft is gone at tick 478, at (-1594, -33, 1158).
- Nose down in the hangar: floor at ticks 117 and 123, 1500 to 1021 to 634 health, thrown back, never Landed, and it flew on.
- Nose up in the hangar: ceiling at tick 129, 1500 to 519; a second surface at tick 137, down to 180; destroyed by the hit at tick 160.

`T [5, 2.5, 0]` and `S [1.5, 0, 0]` are the interceptor's log values in those runs.

Bots over 5400 ticks, seed 7, in `ass`, contacts by phase: spa1 none; spa3 2 Flying; spa6 1 Flying; spa7 3 Flying; spa8 1
Takeoff and 7 Landing; spa9 1 Takeoff and 8 Landing. BF2's AI curve still takes no contact (line 117417, OURS), so these
are the phases outside the curve.

The Takeoff lines on spa8 and spa9 are the same case: `imp_fly_trooptrans` on sp4, at y 15.7, meets the hangar overhead with
normal (-0.58, -0.80, -0.16), which the rule for y below -0.7 turns into a landing. So a transport on that pad goes back down.
`measure:` the user's save has no transport, so the PS2 was not seen doing this.

Not ported, each `trace:`: hit sound and sparks; rumble `FUN_00432b68`; camera jolt `FUN_001ba7e0` and the viewport `+0x90`
kick; the model bytes +0 and +1 Landing rules (props `0x6d8fe606`, `0xd9d02e03`); the `DAT_00689738` kill in Takeoff; class bit
`+0x1208` bit 1; the other body's factor from vtable `+0x40`, taken as 1.0; slot `FUN_00213370`'s `+0x160` branch. OURS: a
terrain hit needs the start of the step above the sheet, and BF1's `FUN_001dd4c8` shares the structure but has no nose term and
no flyer substitution, guarded by `FlyerRow::bf2`.

### Checks

On the working tree over `f9271f32`, after a full `scripts/pack.py` bake from the two PS2 discs.

- `cargo test --no-fail-fast --manifest-path engine/Cargo.toml`: 4327 passed, 5 failed, 196 ignored. The failures are the same
  five as §9, and `nav_walls::mygeetos_position_table_is_built_in_under_a_third_of_a_second` passes alone.
- `scripts/sim_gates.sh`: green. `--sim-hash 600` is `5c5458c8927f122b`.
- `--sim-trace ... 2400 --mode con --seed 7 --state-hashes` is unchanged from `a3f5c192` on `bf2_nab2`, `bf1_tat2`, `bf2_kam1` and
  `bf2_tat3`. `bf1_hot1` moves on purpose to `c1b2c1442827bc54`, first differing at tick 45. Putting back only the old tilt term
  moves the first difference to tick 431, so tick 45 is the tilt fix and tick 431 is the new contact.
- Replays with `dist/openbf.exe` on spa8: the crash into the frigate gives the same track and ends at the same place.

Not checked: a person flying in a window, the transport on the PS2, and how close our dive is to the PS2's in numbers.

### A second round on the same map

Corrections after the user flew it again.

**The hull still has a way in.** Straight hits on the destroyer's outside work: 60 + 120 random flights at speeds 20-130 and
angles up to near grazing, and 80 from inside the hangar, gave no crossing of a surface. But the collision has a real hole. On
top of the rear right of the Imperial destroyer, world x -36..160, z 2014..2314, y about 100, about 30,624 m2 of drawn hull has
no collision within 8 m under it. Smaller gaps, mostly riser lines, are in the top at x -432..-144 z 1938..2250, x -176..-60 z
1838..2434, x -532..-488 z 1854..2438, x -224..-196 z 1162..1242. The bottom is sealed. Flying down at (-21, 2127) from y 180
to y -1 at 30 m/s gives no contact and no damage, with a dark interior like the user's frame.

That red area is two things, measured by rastering the collision and the drawn hull at 4 m. Through: x -34..66, z 2112..2268,
about 14,900 m2, drawn hull at y 98..154 and the highest collision at y -76..-52, the hull's bottom. Sunk: x 26..162, z 2112..2316,
about 15,000 m2, with collision 13 m under the drawn plate (at (62, 2164) drawn 99.8 and 86.5, collision 86.6). The riser lines are
sunk too, 13 m at the median. So a ship can clip into those plates and then hits something, and it only flies free in the through
part.

Save state 10 of 14:10 is not a test of this: the ship is at (-141.9, 412.4, 2328.8), 2250 of 2250 health, 70 m/s, about 300 m
above the deck, beside the dome `imp_cap1_shield_ext_r`, heading within 1 degree of the through part's centre. From there, 1.5 s
straight then a 70 degree dive to (15, 112, 2191) has a clear line: the drawn plate at 314 m and the first collision at 489 m.

The PS2 agrees on the sunk part. Save state 10 of 14:15 holds a crash that started over it at 14:14: the ship ended at (99.6, 86.5,
2217.5), health 2250 to -1340.6. The drawn plate there is y 95.8 and the disc's collision y 83.1, so the ship's centre went 9.3 m under the
plate and stopped 3.5 m above the collision, with nothing to hit at the plate. The user saw it too: "just under the plate".
The port's formula on that triangle, normal (0.179, 0.982, -0.058), with the class's 5 and 1.5 and the default second threshold of
2.5, gives 3471.5 against 3590.6 measured, 3.3% low. `trace:` the difference: the normal may not be the triangle's, and the
velocity was read after the contact. The through part is still `measure:`.

This is the disc's data and not a bake loss: raw `coll` leaves equal the baked counts (118, 188, 258, 209 triangles for parts 1
to 4), and `imp_cap_stardestroyer3` and `4` hash the same in SPA1, 4, 5, 8 and 9.LVL. All instances and primitives of every
layer were included. On that data the PS2 should let a ship through too, which nobody has looked at. `measure:` fly the PS2 into
(-21, 2127) from above. No code changed for this. The port's contact takes the deepest overlap per primitive per step, at most
8, where `FUN_0016bf10` loops over several contacts; `trace:` whether that matters here.

**Instant explosion.** The death regions on spa8 are 6 x 10 x 6 cylinders in the shield rooms and the sim tests them on soldiers
only (`re-kill-floor.md`), so they do not explain the user's "instantly explode".

**Death, flash and pad camera.** `re-vehicle-death.md` §11, `re-hit-feedback.md`, `re-flyer.md` (section routing).

Replays with `OPENBF_WALK_FEET_Y=4.9`, spawn -589.3,1433.0: flying nose up into the hangar overhead, the craft dies at about
tick 160, no pilot is drawn, the view stays while the hull chunks and fire fly past it. Hitting the floor flashes red at ticks 117
and 123, fading by about 138. Boarding sp3's TIE (`imp_fly_tiefighter_sc`), the log reads center (0,5,-8), offset (0,1,10), and
the ship is framed on the pad and in flight.

Checks on the working tree over `f9271f32`, with the flyer explosion and lit chunks (`re-vehicle-death.md` §11, `re-chunks.md` §8):
`cargo test --no-fail-fast`: 4335 passed, 4 failed, 196 ignored. The failures are four of the same five as above;
`nav_walls::mygeetos_position_table_is_built_in_under_a_third_of_a_second` passed in this run. `scripts/sim_gates.sh` green.
`--sim-hash 600` and the five `--state-hashes` runs are the same as the round before: `bf1_hot1` stays at `c1b2c1442827bc54`, the others
at `a3f5c192`'s values, so neither change moved anything on those maps. Whether a flyer died in those runs was not checked, so they may not exercise
a flyer's new blast damage. Not checked: a person playing it in a window, the chunk fade on screen (the replay's pieces leave the
view first, so only a unit test covers it), and the through part of the hole against the PS2.

## 12. Pre-PR handoff (2026-11-05)

Source reviewed: `3458f82d` over `upstream/main` at `a3f5c192`. The cleanup corrects attribution of the scoring/HUD task,
removes the stale dimmed-space-map test description and repairs a crate-map path; it changes no gameplay behavior.
The two implementation commits remain intact.

Current focused checks on that source plus the cleanup:

- `cargo check --workspace --manifest-path engine/Cargo.toml`: passed.
- `cargo fmt --all --manifest-path engine/Cargo.toml -- --check` and `git diff --check`: passed.
- `cargo test --manifest-path engine/Cargo.toml -p core --lib space_assault`: 16 passed.
- The same core command with `flyer_contact`, `ai::flyer_drive` and `fmath::tests`: 12, 38 and 9 passed.
- `cargo test --manifest-path engine/Cargo.toml -p game --test suite space_assault -- --test-threads=1`: 18 passed,
  using the existing local bake and missions, with no ignored tests.
- Git for Windows bash running `scripts/gpu_tests.sh --game-lib flyer_camera` and `--game-lib shell_ia`: 14 and 2 passed,
  serially. These selectors do not establish complete visual acceptance.
- `python scripts/check_provisional.py upstream/main...HEAD` and the working-tree check: zero findings.
- Native Windows execution of `scripts/size_report.sh`'s embedded Python reports backslash/forward-slash mismatches.
  A separate portable inventory finds all 808 source files mapped and zero unwaived files above 2,000 lines. Two missing
  local references (`engine/artifacts/briefs/SPLIT_RECIPE.md`, `engine/artifacts/loop/gates.toml`) also occur on the base.
  The new invalid `sim/flyer_contact.rs` reference was corrected. The standard size/map gate is not claimed green.
- `python -m unittest discover -s scripts/tests -p test_pack_limits.py`: 13 passed, 1 failed, 1 errored. `CmpBake` exits
  127 from its shell harness; `ScratchBake` passes a missing `true` executable as an environment value on Windows.
  The full workspace suite was not repeated; the four previously recorded Rust failures remain disclosed in §9/§11.

Follow-ups: [#122](https://github.com/sprucey149/openbf/issues/122) space bots;
[#123](https://github.com/sprucey149/openbf/issues/123) enemy indicators, fighter lock-on, objective feedback, icons and scoring;
[#124](https://github.com/sprucey149/openbf/issues/124) flyer crash details;
[#125](https://github.com/sprucey149/openbf/issues/125) shared destruction gaps and duplicate texture uploads;
[#126](https://github.com/sprucey149/openbf/issues/126) CTF census and space one-flag.
The hull measurements stay with #112, the content census with #107, and flight-route coordination with #103.

Played-round acceptance is blocked. The player's 2026-11-05 reply reports about 130/180 points after destroying all
available objectives, with the remaining fighter kills impractical because the AI combat loop, enemy-location feedback
and lock-on are incomplete. This is a reported gameplay blocker, not simply an unperformed win-screen check; the
automated score-limit transition does not establish a practically completable round. #109 stays open, and draft PR #127
records the dependencies on #122 and #123. The reported approximate total is not a measurement of every map's scoring.

The latest local black-box snapshot was read first: `dist/openbf_data/playtest/last.json`, Space Hoth Assault, seed 254,
exit at tick 6165, stamped `f9271f327a12+dirty`. It predates PR head `1374ca69`, so it does not independently verify the
reported score or targeting gaps on that exact head.

Still unchecked: the PS2 through-hole comparison, broader flight/crash parity and ground-vehicle presentation comparisons.
No new per-map 60 fps population or simulation timing measurement was made during this cleanup.

## 13. Objective messages and voice (#123)

`scripts/gen_missions.py` reads `ObjectiveSpaceAssault` and the map's script. `main.5` is `UpdateObjectiveMessage`, using
`level.spa.hangar.<kind>.{atk,def}.down` and the `attVO`, `defVO` keys; `main.7` plays the win and lose voice. The closures
`OnAllShieldsDown`, `OnAllShieldsUp`, `OnDisableMainframe` and `OnEnableMainframe` play `*_obj_16` to `*_obj_23`.
The generator now shares globals between the `_cmn` and `_ass` chunks, because spa6 to spa9 define the team constants in
`_cmn`. All six maps emit eight scripted entries.

`Event::SpaceObjectiveDown` fires once when the last part of a system falls and `Event::SpaceLinkChanged` once per shield or
link change. Tests: `each_shield_change_reports_once_and_a_repeat_kill_awards_nothing_new`,
`every_objective_names_real_text_and_voice_lines`.

## 14. Fighter lock-on (#123)

`core::sim::lock_on`, HUD in `ui::hud`, `ui::hud_tree`, `ui::hud_draw`.

The November 9 retail locked-shot measurements and targetable-body correction
are in `re-ordnance-kinds.md` section 4.6. The missile's dispersed shot direction
does not decide whether the launcher's completed lock is passed to the round.

- **The press.** Flyer tick lines 116130-116175: `(+0x2cc & 3) == 3` is a press, which takes the next target, and
  `(+0x2cc & 0xf) == 9` is a hold, which clears. The button is Square, the port's `buttons::RELOAD`.
  `trace:` `LOCK_CLEAR_HOLD_SECS` 0.5 s stands for the hold pattern; the input history's timing is not derived.
- **Candidates.** Scoring lines 21692-21740, ring lines 21077 and 21131. Distance squared over the aim's dot squared, dot
  floored at 0.1, a strong preference inside 0.98 and 0.998. A used target is remembered; when the list is spent it starts
  again (`LOCK_CYCLE_MEMORY` 64 is the port's ring).
- **Who.** `FUN_0017c6e8` applies the weapon's target mask. `TargetLockMaxDistance` and `TargetLockLose` come from weapon data:
  20000 and 25000 on flyers, 100 and 110 by default.
- **`EnableLockOn`.** `FUN_0018f308` writes bits 44-47 of the object's word at +0x268 (3 enabled, 8 disabled). Only spa1 and
  spa3 call it, on `OnAllShieldsDown`. The port turns a hull into a vehicle-kind target while the group's shields are down
  and drops it when they return.
- **Launching and the dwell (corrected 2026-11-07).** `FUN_0033a658` gives a human's launcher its target: the selected lock, the
  class's kind mask (class+0x120), cos at least 0.9 from the firer's aim (`LOCK_FIRE_COS`) and line of sight. A class counts
  as lock-capable when its ordnance is a missile (class+0x305, setter 319784-319797). The weapon's state machine (`FUN_00326170`)
  moves from idle (0) to state 3 as soon as `FUN_0033ab60` reports a target and a positive `LockTime`, with no trigger needed.
  In state 3 `FUN_0033a950` adds dt to +0xbc up to +0xc0 (`LockTime`, class+0x300) and sets the ready byte +0x17c when full. A
  changed target zeroes it, and `FUN_0033aa58` leaves the state when the target is gone. The trigger fires from state 3, homing
  only with a full dwell, and `FUN_0033a5e0` restarts it. The port keeps `LockOn::held` and `charging` per weapon channel.
- **The lock ring.** Each channel's `player1.weaponN.lockOnPosition` is sent only in state 3 (`FUN_003b3398`, 398065-398160). Its
  mesh `hud_main_reticule_rocketlockon_active` takes `chargeScale`, `chargeRotate` and `chargeColor`, which `hudtransforms`
  derives from `player1.weaponN.heat`. `FUN_003b3398` sends that heat as +0xbc / +0xc0 in state 3 (397707-397719), which is the
  white-to-red fill. `ui::hud_space` now sends the dwell's fraction as that heat.
- **No lock tone on a flyer.** `LockedOnSound` (ordnance +0x12c, setter 300862) is authored on no BF2 class. The
  `<side>_weap_inf_rocket_launcher_tracking` sounds are named only by on-foot launcher classes, under key 1323664648, and no
  flyer launcher carries that key. An earlier port looped that sound for the whole of a selected lock; that is removed.
- **The incoming-missile warning is silent.** The HUD's `missileLock` warning is read at `FUN_003b6050` and sends only HUD
  events; no sound path was found from it (section 27). The port's OURS beep is removed.
- **OURS `trace:`** a station target in `sa.lock_on` is treated as the vehicle kind for a launcher's mask, following `EnableLockOn`
  above. `measure:` torpedoes against a downed capital ship's systems on the PS2.
- **HUD.** Registered at `FUN_003b0aa8` lines 395768-395770 and 395922-395952. Panel `FUN_003b2cd0` line 397248: distance each
  frame, and a direction arrow while |dot(dir, up)| exceeds 0.2 (0 or 180 degrees). Reticle `FUN_003b3398` lines 398060-398170.
  Placement `FUN_003abc60` line 392409: a flyer's marker scale runs 1.5 at 200 m to 0.1 at 1700 m (0x5ff98c), the corner sprite
  is clamped to 0.9 to 1.4, 1.2 off a flyer, and pulses with white in line of sight and grey (64,64,64) out of it.

Open, with `trace:`: the played warning and audio sequence (the writer and HUD reader connection is in section 25);
`lockOnName` (`FUN_003b2b98` unread); the audio cue for a lock (the
`FUN_00394428` call is a tutorial hint, `level.geo1.hints.lockon`); the off-screen pointer's scale, which the original lerps by
the record's near and far values while the port uses the minimum.

Finding the last fighters on the PS2 is this: lock cycling across 20 km, the distance and arrow, and the off-screen pointer.

## 15. What the PS2 does about bots in a space round (#122)

### Unseen combat: `FUN_002a3c28`

This corrects `re-ai-difficulty.md` §6. In Space Assault, auto-balance mode 1 (`FUN_002a3328` line 212845) does not hand out
goals; it kills flyers and damages capital systems, picking only what no player can see. Ported as `core::sim::space_balance`.

- **When** (`FUN_002a3528` line 212948, every 15 AI steps through `FUN_002af948`). Only with humans on exactly one team
  (`FUN_002a8300` of team 1 xor team 2), so **the PS2 has none of this in a round with no human**. Nothing before 30 s.
- **The nudge.** diff = human points - enemy points (`FUN_002a3400`: margin 20, divisor 10); the human side gains 20 or 10
  when the enemy's tickets fall under 25% or 50% of their start, which cannot happen at 999999. Past the margin,
  enemy nudge = (diff -/+ 20) / 10, human nudge its negative, each held to 4.
- **The clock.** A due timer redraws with `FUN_002a3200` U(lerp(45,15), lerp(100,60)) s for a fighter kill and `FUN_002a3268`
  U(lerp(25,10), lerp(45,12)) s for a system attack. The lerp runs on the enemy bots' skill plus nudge, clamped to 1..20
  (`FUN_002a29d8` with no unit, `FUN_002a2bb8`). The period is multiplied by -1/(n-1) when the human nudge n is negative, n
  when positive: up to five times faster when the human leads, four times slower when behind.
- **Which side.** `FUN_002a3f08(0,100)` draws 0 to 100. A fighter dies on the human's team above 40 (`FUN_002a32d0` 0.4), else
  on the enemy's. A system of the human's ship is attacked above lerp(0.5,0.25) x 100 (`FUN_002a32e8`), else the enemy's.
- **A flyer** (`FUN_002a3770` line 213050). Up to 40 AI units (`FUN_002b00e8` line 222108 walks the AI controllers,
  `DAT_005fe08c`) by relation to the team: mask 4 is relation 1, friends; mask 1 is relation -1, enemies (team +0x84 bytes,
  `re-aim-assist.md`). Walked last to first, the first unit in an EntityFlyer (`DAT_00689368`), outside every player's view by
  `FUN_001ba840(100, 3000)`, with no human aboard (`FUN_002a7fd8`).
- **The kill** (`FUN_002a38a0` line 213094). The victim dies through its kill virtual with an unseen enemy flyer credited, so it
  scores as a normal kill: +3 and +1. Both flyers must exist.
- **The attack** (`FUN_002a3980` line 213130). The target comes from `FUN_0036b7c0` line 353182: systems of that team that are
  shown (`FUN_0036ac20`'s flag: an icon, and first in a linked group), alive, and outside every view by
  `FUN_001ba840(radius, 1500)`. Index = frac(U^2 + round seed) x count. Damage is the best over the attacker's weapons of
  max(rate value, 2 x per-shot value), where per-shot is ordnance damage times the target type's scale plus explosion damage
  times falloff at 2 m less half the target's radius (`FUN_002a9e58`, `FUN_00268fe0`), and rate value is the cannon form of
  `FUN_002aa548` (clip, salvo, shot delay and reload). If the target has shield left, that is x5, held to 15000, then held to
  the shield. Dealt with all scales 1 (`FUN_001e0900`), credited to the attacker, so a kill scores normally.

Where the port differs, each with `trace:`:

- The sim has no camera. `Sim::balance_seen` stands the player's aim ray in for the camera with `FlyerParams.view_fov` (the
  pilot seat's `ThirdPersonFOV`, 48 by default as in `flyer_camera.rs`), 65 on foot, and tan y = 0.7666 tan x
  (`re-zoom.md` §5). Far is the query plus 8 at zoom 1 (`DAT_005fc4f4`), ignoring the map's own far clip, and a dead player sees
  nothing. The sphere test is otherwise `FUN_0057bc50`.
- `FUN_002aa630` and `FUN_002aa6b8` (non-cannon weapons) and the charge chain walk are not ported: a thrown weapon counts 2 x
  per-shot alone, and no space flyer weapon charges.
- The first draw happens at the first evaluation with a human, not at load, so a round with no human draws nothing and hashes
  as before.
- `measure:` the nudge is used for these timers only. On the PS2 it also shifts every bot's skill by up to 4 (`FUN_002a29d8`);
  the port's bots take their skill at spawn and that shift is not built.

### Who boards: `FUN_002e2e38`

`re-soldier-ai.md` §7.1 already had this scan, and it answers the question of landing craft: every registered vehicle slower
than 3 m/s with a free seat tells friendly or neutral AI within 50 m, with **no test of the bot's order or post** and no class
filter beyond `NoEnterVehicles`. Bots do board landing craft on the PS2.

The port's `ride::claim` required a target post. A bot under a reachable Hunt has none, and enemy flyers passing over a hangar
gave one. So that side's pilots ran around their deck and boarded nothing, and its fleet died out while the other side's grew:
at 13 minutes on spa1 seed 7, 16 pilots on foot beside three empty fighters. In a space round a flyer is now claimed
within `VEHICLE_NOTIFY_RADIUS` 50 m with no post, as the scan does. Ground maps keep their rule.

`can_fly` still refuses a class no route names, which keeps `transport` classes parked on spa1, spa3, spa6 and spa7 (only
spa8 and spa9 name `transport`). The PS2 would seat the pilot; what its follower does with no route is the unported geometric
candidate (section 10). `measure:` a PS2 round on spa1, watching a landing craft.

### Not ported, and why

- **Route to the human** and **relocation near the human** are now ported: section 16. `FUN_002fbe60`, `FUN_002fa8b8`, `FUN_002fe150` and
  `FUN_002fe588` remain unread or unported.
- **Pursuit.** `FUN_002fd930`'s trigger on the PS2 appears inert: flyer+0x59c is written only by `FUN_00209220`, with a chance of 0.
  The port's "any seen enemy" trigger in `flyer_drive.rs::bf2_plan` is OURS and stays, flagged, until the maintainer decides.
- **Free fire and the Deathmatch walk** are now ported: section 17.
- **Bots shooting systems directly.** `FUN_002ac8f8` allows flyers against buildings, but whether perception ever puts one on the
  threat list is unread (`FUN_002ba088`, `FUN_002b7a40`, `FUN_002ac570`).

### Rounds to the limit

108000 ticks (60 minutes) at most, 30 Hz, 16 AI a side, `missions/bf2_<map>_ass.ron`, measured 2026-11-05. Win time, points at the win.
The counts are for the whole 60 minutes (the sim runs on after the outcome; the unseen routine stops at it), read from the last row.

| Map | Seed | Human | Winner | Time (s) | Points | Units | Flying | Shot | Crashed | Unseen kills | Systems lost | System hits |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| spa1 | 7 | idle | team 1 | 1808 | 29/181 | 17/16 | 15/15 | 77 | 8 | 16 | 0 | 47 |
| spa3 | 7 | idle | team 0 | 635 | 180/92 | 17/16 | 13/14 | 216 | 21 | 36 | 0 | 112 |
| spa6 | 7 | idle | team 1 | 1606 | 156/180 | 17/16 | 16/14 | 121 | 7 | 38 | 1 | 106 |
| spa7 | 7 | idle | team 1 | 1974 | 156/180 | 17/16 | 13/13 | 75 | 29 | 46 | 0 | 130 |
| spa8 | 7 | idle | team 1 | 2480 | 125/181 | 17/15 | 14/15 | 38 | 6 | 50 | 0 | 140 |
| spa9 | 7 | idle | team 1 | 1998 | 156/181 | 17/15 | 12/13 | 83 | 10 | 46 | 0 | 132 |
| spa1 | 11 | idle | team 1 | 1770 | 67/180 | 17/16 | 14/15 | 65 | 6 | 29 | 0 | 75 |
| spa3 | 11 | idle | team 0 | 557 | 180/102 | 17/15 | 15/12 | 193 | 26 | 44 | 1 | 140 |
| spa6 | 11 | idle | team 1 | 1940 | 140/182 | 15/16 | 14/14 | 98 | 11 | 41 | 1 | 120 |
| spa7 | 11 | idle | team 1 | 2308 | 128/180 | 17/16 | 13/14 | 82 | 22 | 38 | 0 | 110 |
| spa8 | 11 | idle | team 1 | 2648 | 140/182 | 17/16 | 14/13 | 29 | 3 | 58 | 1 | 171 |
| spa9 | 11 | idle | team 1 | 1978 | 147/181 | 17/16 | 14/15 | 78 | 4 | 44 | 0 | 133 |
| spa1 | 23 | idle | team 1 | 1621 | 37/182 | 17/16 | 15/15 | 89 | 8 | 17 | 0 | 40 |
| spa3 | 23 | idle | team 0 | 477 | 180/78 | 17/16 | 14/13 | 210 | 18 | 40 | 1 | 122 |
| spa6 | 23 | idle | team 1 | 1967 | 172/193 | 17/16 | 15/14 | 101 | 11 | 44 | 1 | 130 |
| spa7 | 23 | idle | team 1 | 1868 | 108/180 | 17/16 | 14/12 | 99 | 17 | 34 | 0 | 94 |
| spa8 | 23 | idle | team 0 | 3016 | 182/177 | 17/16 | 13/13 | 26 | 1 | 68 | 0 | 200 |
| spa9 | 23 | idle | team 1 | 1710 | 85/181 | 17/16 | 14/14 | 96 | 6 | 26 | 0 | 67 |
| spa1 | 7 | none | team 1 | 1403 | 4/183 | 17/16 | 14/14 | 112 | 4 | 0 | 0 | 0 |
| spa3 | 7 | none | team 0 | 879 | 180/0 | 17/15 | 15/13 | 202 | 21 | 0 | 0 | 0 |
| spa6 | 7 | none | team 0 | 3306 | 180/144 | 16/16 | 12/12 | 109 | 9 | 0 | 0 | 0 |
| spa7 | 7 | none | team 1 | 2816 | 100/180 | 17/16 | 14/14 | 103 | 21 | 0 | 0 | 0 |
| spa8 | 7 | none | none | over 3600 | 32/77 | 17/16 | 13/14 | 27 | 5 | 0 | 0 | 0 |
| spa9 | 7 | none | team 1 | 2076 | 41/182 | 17/16 | 14/15 | 93 | 3 | 0 | 0 | 0 |

What this shows and does not:

- All 18 idle-player rounds reach the limit, in 8 to 50 minutes. Five of six no-human rounds do; spa8 stands at 32/77 after an hour.
- Re-running spa1 seed 7 gives a byte-identical trace (`4725d3e24d4ae50e`). `--sim-hash 600` is `5c5458c8927f122b` and `scripts/sim_gates.sh` is green.
- No round wins on objectives. At most one system falls in any round, so the points are kills; the report does not split flyer kills from soldier kills. System hits are all the unseen routine's. With no human, no system falls in an hour, so in the port bots do not destroy systems by their own fire, and #122's capital-ship attacks are not established.
- `measure:` the sides are lopsided. Team 1, the side without the idle player, wins 14 of 18 idle rounds. It takes spa1 with team 0 on 29 to 67; team 0 takes spa3 in 8 to 15 minutes, with or without a human. The cause was not investigated. `Shot` is every credited flyer death except the unseen routine's, turrets included, and which guns do it was not broken down. On spa8 the unseen routine kills more flyers than the fighting does.
- Measured with seat guns reloading (`re-ammo-reload.md`, Seat guns). The spa1, spa8 and spa9 traces are byte-identical to before that change on every seed, because bots there fire only the first weapon of a seat and those primaries have no clip. On the Republic and CIS maps the V-wing's clipped cannon does change the rounds.
- Before the boarding fix (seed 7, 20 minutes) one side had no flyers in the air on every map, and only spa3 reached the limit.
- None of these numbers is a PS2 measurement, and no person played these rounds. The claim first written here, that no comparison with the
  PS2 was possible, was wrong: PCSX2 and PINE were available. Section 16 has the comparison.

`--sim-trace ... --space-report <csv>` writes the minute rows; `--no-human` empties the player slots. "Idle" keeps the trace's
player: alive, on team 0, untouched, so it is the original's condition for the simulation but not a person playing.

## 16. Compared with the running PS2 game (2026-11-06)

The maintainer reported the port as far from a PS2 frame of spa8: no red crosshair on the enemy hull, no target name or bar, no markers, a
clipped destroyer, a planet with square patches, and fewer ships. All of it was checked against PCSX2 over PINE and a frame side by side, with the
PS2's camera solved from its own marker list.

### The solved PS2 camera

Least squares on the marker NDCs and world positions, residual about 1e-3: yaw 155.57, pitch -5.115, roll -0.05, FOV x 67.60, FOV y 54.47, camera
(-1278.77, -25.68, 1117.66), 21.6 m behind the flyer at (-1270.08, -32.03, 1098.41). The TIE class has no ThirdPersonFOV (48 by default) and
FastFOV 90, so 67.6 is a speed fraction near 0.467 of the formula `flyer_camera.rs` already has. With this camera pinned in the port the geometry
lines up. The port keeps vertical FOV and widens at 16:9, so the vertical view matches. `measure:` the port's FOV at real flight speed against the PS2's.

### Reticle pick and readout

A pilot's reticle picks a hull under it with no team filter (`FUN_002b0548`, lines 224190-224340) and writes the relation (line 224405). The gun's
LockOnRange and LockOnAngle are 0, read live, and the hull is never the magnet: `W+0x10c` = 0, `W+0x114` = -1. The port's flyers had never acquired
a pick at all, and the HUD readout was on foot only. `Sim::acquire_flyer_pick` writes `reticle_pick` and never the magnet. An allied hull gives relation
+1, which the HUD draws blue. That follows from the code and is tested in the port; it was not seen on the PS2.

The ray was a real bug: `tick_lock_on` and `lock_on_visible` used `aim_ray`, whose pitch is stale in a flyer. They now use `flyer_ray`.

### Target markers

`sim/target_hints.rs` holds the marker list. Mask: weapon class +0x120. Add range +0x1d8, keep range +0x1dc. Eligibility `FUN_0017c6e8`
(line 22001) has no team filter, so friends get blue markers. The objects with lock nibble 3 on the PS2 are `all_cap1_life_ext`, `all_cap1_bridge`,
`all_cap1_comms`, `all_frig1`, `all_frig3`: the mission's critical systems, `sa.lock_on` in the port. The hull (kind 4) is excluded. Only on-screen
markers draw, since `hintoffscreen` has alpha 0. Not ported: attacker elements (bit 3), HintFlag (bit 8), node 0x3483a489's offset, and other
vehicle kinds. `trace:` which of `FUN_001f8548`, `FUN_00202d08`, `FUN_0021b470`, `FUN_00240bf8`, `FUN_0025f4a0` sets the mask for a flyer.

### Clipping and the planet

The far plane was fixed at 2000 m while the map's `far_scene_range` is far larger: `Camera::reaching` extends it for the world pass, and shadows keep
the camera's own. The planet's squares came from the bake forcing COL0 alpha opaque on sky models, losing the edge fades:
`VertexLighting::Sky` keeps it. Ground maps keep opaque skies until `scripts/rebake.sh`.

### The ridden vehicle's health

On mounting, `FUN_003b4bb8` hides the soldier's bar and feeds `player1.vehicle.health` and `.healthFraction`, the green bar through
`player1vehiclehealthcolor` (`re-seat-hud.md` section 6). The port drew the pilot's own health in blue. It now uses `HudModel::seat_health`. PS2 bar
(0,99,3), port (6,111,20), at full health. The port's bar shows red below 0.3, by the layout's table. The port's remaining
difference was the soldier's blue bar drawn over the green one while seated; section 27 measures and fixes it.

### Bots seek the human

Every soldier's agent draws a number in 0..=100 at spawn (`FUN_002a3f08(0,100)` at line 226565, stored at +0x5a0). `FUN_002faef8` (line 276064) passes
a bot only if the low nibble of that number is at least 8, so 48 values of 101, 47.5% of bots. The gates also exclude network sessions and demo mode,
and `DAT_005fed90` values 2 and 3, read as 0 live.

**Route.** When a bot picks a new route, `FUN_002fc980` (line 277194) checks, with no team test: the class's FlyerVsFlyer (follower +0x78, set in
`FUN_00300230`), that bot gate, local player 0 in a vehicle, and that player more than 400 m away. Then `FUN_002fa6b0` (line 275657) tests a chord of
150 m toward the player. If clear, the curve is `[pos - 30 forward, pos, pos + 150 dir, pos + 350 dir]`. `Sim::flyer_seek_curve` does this before
`connect`. Not ported: the node BranchProbability roll that also triggers the choice (`FUN_002f97f0`; `trace:`, the port's paths carry no chance),
and the speed profile `FUN_002fa8b8(20, 125, 300, 30, 0.707)` (`trace:`).

**Relocation.** The AI update counts flyer +0x1d8c down, and `FUN_00202780` refires it in 5.5 to 8.5 s. Each firing calls `FUN_002faf90` (line
276094). It gives up if: under 30 s of battle clock (`FUN_004240f8`); no FlyerVsFlyer; the bot gate; the human not in a flyer; the point 300 m ahead of
the human outside the `bnd_` shapes (`FUN_0034c550`); under 50 m from the human; ahead of the human (dot of forward with the unnormalised offset above
0.25) and under 500 m; a pursuit or loiter flag (+0xfd, +0x102); the craft on a player's marker list (`FUN_0017ce98`); or the same team as the human
unless a 1 in 4 roll passes. A coin picks the lane. Heads: 85 m ahead, facing the same way as the human, with a 6 s speed boost (+0xf8, the 1.5 factor at
line 277484). Tails: 700 m ahead, facing the human. Either way the spot is 85 m off to a random side, up to four tries, needing a clear sphere of 6 body
radii (`FUN_00161a98`). After that a 750 m straight curve (`FUN_002fa6b0(750, ...)`) runs along `forward * (4 or -3) - side`.

Follower +0x28 counts the class's formation names (key hash 0xe5108b93, `FUN_003012a8`). Read literally, and confirmed in the machine code at 0x2fb058, a bot
in a class with **any** formation relocates alone, while one with **none** tries to make a squad from an empty list and fails, so it never
relocates. On spa8: 2, 1, 1, 2, 0, 0 across the six classes. The port takes `formations` from the same key. Squad teleports (+0xfc, line 277385) are
not ported since `FUN_002fbe60`, which forms them, is unread: `trace:`.

OURS, each `measure:` against the PS2: the clear sphere is three chords through the spot (the port has no sphere query); `vehicle_blast_sphere`
stands for the body radius; the relocated craft keeps its speed along its new heading (`FUN_00202430` was not fully read); the agent number comes
from a hash of the handle, not an LCG. `FUN_0017ce98`'s other list at 0x66af58 is not modelled (`trace:`).

The recording below could not see any of this: in the 240 s PS2 run the human was not in a flyer, which `FUN_003d6e78(0)` +0x14c = 0 turns off. No PS2
measurement of seeking or relocation exists yet: `measure:` a recording with a live human pilot.

### Ship census

PS2: 231 frames at 1 Hz, spa8, no human flying, an unknown time into the round. Port: seed 7 and 11. Crewed flyers, team 0 / team 1.

| | Live | Crewed | Team 0 near own hull / enemy hull | Team 1 near own hull / enemy hull |
|---|---|---|---|---|
| PS2 | 22.4 | 8.5 / 9.0 | 0.76 / 0.06 | 0.49 / 0.45 |
| Port 0-120 s | 17.5 | 5.6 / 7.2 | 0.39 / 0.47 | 0.39 / 0.21 |
| Port 120-300 s | 25.8 | 11.8 / 10.5 | 0.37 / 0.44 | 0.37 / 0.31 |

"Near" is within 1500 m of the hull centre. So the port flies **more** crewed ships than that PS2 sample after two minutes, not fewer, and the
imperial shortage first suspected was only the first minute. The distribution differs: the PS2's team 0 stays by its own ship, and the port spreads
evenly. Without a human flying, that cannot come from seeking, so it is unexplained: `trace:` `FUN_002fc550` and `FUN_002ff478` against `connect`.
With seeking and a human held at the PS2's spot: ships within 1000 m rose from 3.0 to 9.3 and within 1500 m from 7.4 to 13.3, 120-300 s, seed 7,
one run; 22 relocations in 300 s.

### What still differs in the frame

- Sky and planet are too bright in the port: nebula (56,35,82) against (33,21,57), dark space (12,3,20) against (0,0,7), planet (104,120,135)
  against (74,90,109), horizon rim (74,112,140) against (34,75,101). HDR is level-neutral (`re-glow.md` section 2.2), and hull and ship are not off the
  same way. Roughly PS2 = port^1.35 on these patches. No cause was found and nothing was changed: `trace:` the sky pass colour on spa8.
- The PS2 draws thin white streaks (`trace:` space dust). Absent in the port.

## 17. Why nobody shot, and the fire in the air (2026-11-06)

The maintainer reported almost no shots anywhere in the port, against a screen full of them on the PS2, and that nothing engaged them near the
enemy destroyer. All numbers below are spa8. PS2: a private PCSX2 copy read over PINE, no human flying, about 12 minutes into the round, with only
12-14 armed turrets alive (a partial count: section 19 finds 16 that hold targets). Port: seed 7, no human, 100-300 s of a 300 s run. Neither is a measurement of play with a human pilot.

| | PS2 | Port before | Port now |
|---|---|---|---|
| Live bolts, mean (max) | 18-24 (48-53) | 0-2 | about 59 (about 80) |
| Flyer shots per second | 20.8 | 0 | 44.7 |
| Turret shots per second | about 3 (12-14 alive; section 19: 1.6-2.3) | about 0.24 | 11.7 (about 25 alive) |
| Crewed flyers | 18.9 | 0-3 | 19.4 |
| Parked and empty | 4.3 | | 3-6 |
| Soldiers on foot | 4.3 (plus 7.1 in other seats, 2.8 with no controller) | | 8-12 |
| Deaths per minute | 19.3 | | 10 |
| Share of flyer shots with no AI target | above 98% | | 98% |

The maintainer's guess was right: on the PS2 almost every bolt is fired with no target at all.

### The free burst

A flyer's AI update counts a timer at flyer +0x5b4 down (`FUN_0020b840`, lines 121345-121357). Under 0.0001 s it re-rolls to `(rand - 0.5) * 1.5 + 6`.
Under 0.6 s, and only for a class with FlyerVsFlyer (class +0x6cc), it asks the mover to fire. `FUN_002bbb60` sets mover +0xa8 bit 7 or 8. Sampled
live, bit 7 rises when the timer drops under 0.6 s and stays set until the pilot's next think. `FUN_002ce108`, called from `FUN_002cc0a0`,
`FUN_002cf458` and `FUN_002ce338`, is what clears it. Duty is 0.48 per flyer on the PS2. The first value read live is 10.00, and its setter was not
found (`trace:`).

The bit runs the burst law `FUN_002bb1b0`, which dispatches to `FUN_002bac98` (cannon: ends on heat, since RoundsPerClip 0 never reaches the count),
`FUN_002baac0` (charge) or `FUN_002baf70` (tap). The port is `flyer_drive.rs::free_burst` and `gunner.rs::free_fire`. OURS, `measure:` against the PS2: a
fixed 4 s think grid staggered by `FREE_BURST_THINK_STEP` per handle; the PS2's period changes with its think level (1.6 s fighting, nearer the
camera faster), which the port does not model.

The port's rule that a bot presses fire only at a contact it perceives (`sim/guns.rs`, `nav_stats.unperceived_shots`) has one traced exception here: a pilot
inside a free burst, the only shot that rule now lets through with no contact.

`FUN_002bb900` vetoes the fire when a human on the pilot's team is within 50 m on the nose (`human_friend_on_nose`, sphere 1 m for a soldier,
the ride's sphere for a rider). `FUN_002b8028` is a second veto, not ported (`trace:`).

### The aim test

With a target, `FUN_002ace58` and `FUN_002ad1a0` fire inside a 22.5 degree cone when under 50 m, or when the shot's miss distance is under 200 m.
`gunner.rs::flyer_on_target`. Pilots lead the target's blast-sphere centre (`led_flyer`).

### View distance

`analysis/re-ai-difficulty.md` had `FUN_002e3910` backwards: its first argument is the observer and its second the target. The reach is
`(20 x target bound radius + 30) x DAT_005fe71c x vis x 2 [target driven by a human] x (2 [flyer observer] or 10 [armed building observer])`.
So a turret sees a flyer at 500 m and a human pilot at 1 km (radius 1, multiplier 1), where the port's turrets used 100 m. The vehicle part of
the soldier scan in `perception.rs` and `self_turrets.rs::self_view` now follow it. The scan's part for targets on foot (`sight`) still has no doubling for a
human target and no flyer observer factor: `replace:` it with this formula once the ground gates are compared with the PS2. `self_view`'s cap at the weapon's MaxRange is OURS: the PS2's
turrets hold targets at a median 1093 m and up to 3.6 km, and fire at a median 559 m. Its scan radius `FUN_002a9d90` is MaxRange (doubled by bit
0x35) plus 5 x max speed. `measure:` the hold range against the PS2's.

Turrets also lead (`self_lead`), and think on 1.6 s while fighting and 4.0 s waiting in a space round (`re-turret-ai.md`, Think periods). The port
picks the nearest visible target: OURS, the likely reason the port fires about twice the PS2's rate per living turret. `measure:` the selection
against the PS2's target changes, which come 1.6 s apart and which leave a target held 38% of the time.

### The Deathmatch walk: why the ships stayed empty

This was the main fault. Every `spa*_ass.luac` adds a `Deathmatch` goal. `FUN_002e79a8` case 4 reaches `FUN_002e9418`, and `FUN_002e9458` (line 262195)
posts order 10, which is state 0xb (`FUN_002b8d60`, `FUN_002b70d0` at line 226707), the agent `FUN_002c6218` (line 238108), updated by `FUN_002c62a8`:

1. State 1: if the nav graph exists, a coin picks state 2 or 3, otherwise state 5.
2. State 2: `FUN_002a9150` (line 217205) calls `FUN_002ec380`: a random reachable hub, up to 2 x n tries. `FUN_002ea128(10, ...)` then picks a point
   inside its radius less 10, up to 40 tries, falling back to +-100 m and locating it.
3. State 3: a random member (`FUN_0036e558`) of a hostile team. `FUN_002accb0(..., 1)` asks whether this soldier can fight it, through the matrix
   `FUN_002ac8f8`. If so the goal is that target's position +-10 m on each axis, located with `FUN_002eeeb0`. If not, state 2.
4. `FUN_002c6c70`: when the mover is done (`FUN_002bc2b8` = 0), back to state 1.

So on the PS2 soldiers roam the ship, and roaming walks them past the 50 m offer (`FUN_002e2e38`) of a parked flyer. PS2 soldiers reach a flyer 14-28 s
after standing up, and an empty craft fills in about 14-30 s. The port gave every soldier one Hunt that walked them all to one spot about 100 m from the
craft, past the offer's range. `route/retarget.rs::retarget_deathmatch` is the port of the walk. OURS: a hostile's position on another deck falls back to a
point in that hub rather than the located one, and the rolls use `flyer_drive::draw`, not the LCG. Non-space Deathmatch still uses the Hunt, and `replace:`
that with this walk once a ground Deathmatch is compared.

The port's other maps, seed 7, 100-200 s: crews 16.2 (spa1), 18.8 (spa3), 19.6 (spa6), 21.8 (spa7), 13.1 (spa9); turret shots 9.9, 14.3, 13.6, 14.2,
11.7 per second; pilot shots 34.7, 59.2, 54.8, 66.6, 34.7 per second. None of these five maps has a PS2 measurement.

### A human beside the enemy destroyer

A test holds a human's ship still 300 m out from the nearest enemy turret on spa8, with no bots in the round (`a_human_ship_beside_the_enemy_destroyer_draws_its_turrets_fire`).
Of 15 enemy turrets, 4 held a target. They fired 27 shots and destroyed the 2250-health ship in 283 ticks, 9.4 s. No flying was involved, so this shows the turrets engage,
not that a moving approach feels like the PS2's. There is no PS2 figure to set beside it.

### What still differs

- **The weapon channel.** Traced and ported: section 18. An earlier version of this note said no bolt comes out on channel 1 and left it unexplained. That was
  a fault of the probe, which counted bolts only: channel 1 holds missiles and bombs.
- Per class on the PS2, shots per second (per second of burst): X-wing 0.83 (1.69), Y-wing 0.28 (0.65), A-wing 1.53 (3.2), TIE fighter 1.48 (3.27), bomber
  0.52 (0.95), interceptor 3.49 (8.72, one flyer). Port, on spa8: X-wing 1.93, Y-wing 0.64, A-wing 3.69, bomber 0.66, TIE fighter and interceptor about 3.8.
- **Deaths** are half the PS2's rate (10 against 19.3 per minute). `measure:` hit rate on free-burst bolts.
- **Other seats.** 2 PS2 soldiers sit at remote terminals (vtable +0x14 0x62b298) and 5 in seat objects (0x620050). The port has no remote terminals and about 0-2 gunners,
  so 8-12 stay on foot against 4.3.
- **Bots shooting systems directly** (`Foe::Building`): the 7% above counted bolts only, so it understated pilots with targets (section 18). Plain buildings are about 4% of
  held targets on the PS2 and are not ported as pilot targets. Bigturret beams
  (`spa1_weap_imp_shipturret`, AITargetBuilding only) need station targets the port lacks.
- **Think level** by distance to the human is now modelled for space turrets and pilots (section 19). `measure:` a PS2
  round with a live human pilot near the destroyer. Nothing here measured that.

## 18. Pilot targets, weapon channel, damage effects and the crash (2026-11-06)

Everything here is spa8, PS2 from a private PCSX2 copy over PINE with no human flying, about 12 minutes into the round. The port is seed 7. Function and line numbers are
from `SWBFRNT2.ELF-decomp.c`.

"PC" below means the original PC game. `FUN_002a3c28` is the Space Assault AI's difficulty cheat.

### Who writes the channel

`FUN_002bbf00` (lines 230493-230494) and `FUN_002bbed0` (line 230393) write K +0x84, from `FUN_002ab590` (line 218646; PC, sSelectBestWeaponType). A target
is picked first, then the weapon. `FUN_002aaba0` (PC sCauseDamage, PC) refuses a weapon whose AI target mask lacks the target's type, or that is overheated, and
otherwise accepts it when ten seconds of its rate would take half the target's maximum health, or against a soft target by ordnance kind. Heavy ordnance is reserved per target
(`FUN_002ab430`, `FUN_002e0160`, `FUN_002e0338`, `FUN_002e03e0`) for ten seconds. Inside the weapon's AI range the rules prefer plenty of ammo, then damage. Port: `core/src/ai/flyer_weapons.rs`.

PS2 by class, 90 s, 13773 sweeps, flyers in state 2:

| Class | Mean count | On channel 1 | No target | Armed or other building | Flyer | Fire bit | Ammo changes ch0/ch1 |
|---|---|---|---|---|---|---|---|
| 0x199b200 | 4 | .56 | .69 | .31 | 0 | .48 | 6/37 |
| 0x19b22c0 | 3 | .48 | .71 | .29 | 0 | .47 | 1/17 |
| 0x19e0d70 | 2.5 | .49 | .80 | .20 | 0 | .49 | 2/33 |
| 0x1a006f0 | 3 | .15 | .88 | .12 | 0 | .49 | 2/19 |
| 0x1a12160 | 3.4 | .65 | .90 | .05 | .05 | .46 | 1/37 |
| 0x1a23b50 | 1 | 0 | 1.0 | 0 | 0 | .47 | none |

Held non-flyer targets are armed buildings (vtable 0x61a470, 41125 samples) and plain buildings (0x619a78, 1657). There were 38 channel changes in 90 s. So pilots mostly hold turrets, which
selects torpedoes, missiles and bombs. The channel stays after the target goes, and the free burst of section 17 then fires the secondary at nothing.

Port with 24 bots, 150 s (`OPENBF_MEASURE_CREW=24 OPENBF_MEASURE_SECONDS=150`, `pilots_turn_on_armed_buildings_and_fire_their_secondaries`). About 10.6 ships fly in this test against 17.8 on the PS2, so it is not the full-crew round, which has about 19 flying and whose channel share is not measured:

| | PS2 | Port |
|---|---|---|
| Time on channel 1 | about .46 | .09 |
| Holding a building | about .17 | .05 |
| Holding a flyer | about .01 | .02 |

Corrected by section 19: the cause offered here, that the PS2's pilots spend more time near enemy hulls, was measured and is false, and the strafe pattern's body is in the original PC game. The gap was the missing threat law. Channel 1 is now .53 and armed targets .14 with a full crew.

Marked for replacement in `flyer_weapons.rs`:

- OURS, `measure:`: the 4 s think grid is shared with the free burst (`THINK`, `THINK_STEP`). The PS2's period follows its think level.
- OURS, `replace:`, in rounds other than space (space now has the threat table, section 19): `RAYS` 2 and the ranking by nearest. The PS2 ranks through the threat table, uniform for these turrets, before its ray checks (PC).
- `trace:` the flyer rule for threats ahead, weapon index 2 and up, and plain buildings or critical systems as pilot targets.

The first timer of section 17 is resolved: PC sets the fire timer to 10.0 in the constructor, matching the 10.00 first read live.

### Damage effects

Each flyer class holds up to ten `DamageEffect` slots, later rows dropped. `FUN_001e0c98` runs on a change of health: at or below zero it clears the alive bit, and below 0.0001 or dead it stops all
slots. Otherwise a slot is live when `fraction < start + 0.0001` and, if stop is above zero, `fraction >= stop - 0.0001`. A slot that leaves its band stops, and one entering starts, with `FUN_00376d98` giving
scale and `FUN_00376e10` inherited velocity. PC. Port: `game/src/fx/vehicle_damage.rs`, on a change of health only, with the corrected `DamageEffect::active` in `core/src/autoturret.rs`.
Effects follow the hull's hardpoint, or the hull origin when the attach name is missing (`game/src/vehicle_pads.rs`). On spa8 all 8 classes resolve all their slots (6 to 10 each).

X-wing as authored, kept slots: sparks at 85-25, 75-15, 65-5, 55-5; smoke at 75-35, 65-25, 55-15, 45-5; flame at 35-0.1 and 25-. Fractions are percent of hull.

`trace:` the PS2 reader of emitter +0x1c, where `FUN_00376e10` and `FUN_00377a10` store inherited velocity. The port adds it to the effect's own value, following PC.

### The crash

`FUN_00202d08` (lines 116891-116926; PC), once the alive bit is off:

- State 5, or state 0 (parked): pay the kill if the craft has a damager (`FUN_00269a08`), throw the chunk cluster (`FUN_00213630`), kill it, and raise class +0xedc, `ExplosionDestruct`.
- Any other state except 4: raise class +0xed8, `ExplosionCritical`, set health 0 (`FUN_001df3d8`), enter state 4. The timer is `rand*2 + 0.2` if the class has chunks (`FUN_00215f80`), else 600.

State 4's motion is lines 118135-118285 (PC 4520-4577): angular velocity filtered and scaled by lift, velocity toward cruise, gravity times `1 - lift`, a spin from forward x velocity / 64, and a random spin each step.
At timer end the state is 5. Lift falls in `case 4`, lines 117315-117320. Roll target 4 pi, with class +0xe20 `TrickRollSpeed` (default 1, lines 123680, 124866; X-wing 6), lines 119059-119132. The pilot stays seated throughout.

Contact in state 4 or by a lethal collision sets state 5 and instant death (`FUN_00200f80`; PC 2991, 5033). A craft abandoned in takeoff or flight gets health 0, state 4 and 0.5 s (lines 116237-116261; PC 3437-3462), unless class +0x1208 bit 0, `NeverCrashWhenUnpiloted` (lines 124034-124046), set only on `uta_fly_ride_gunship*`.

Port: `FlightPhase::Crashed`, `core/src/vehicle/flyer.rs`, `damage_vehicle_by` and `flyer_starts_crash` in `core/src/sim/vehicles.rs`, `fly_flyers`, `core/src/sim/flyer_contact.rs`. BF1 is unchanged.

A dying craft is left out of these, each with a PC check: lock targets (PC), reticle pick (PC), the seated crosshair (PC), boarding (PC), and vision (`perception.rs`, `self_turrets.rs`, `turrets.rs`).

`ExplosionCritical` is authored on no regular BF2 ship (guided rockets, the AAT, the snail tank and recoilless turrets only), so no flyer shows a separate blast at the start of its crash.

`trace:` or `measure:`:

- The death camera detach (0.25) during a crash is not ported.
- PC's PC has no alive test and the cannon's only stops sounds, so firing isn't blocked in the sim. A bot's trigger stops because the drive branch is for flying. `measure:` whether a dying PS2 pilot's bit stays up.

### How ships die

PS2, 180 s from one save, sampled at about 5000 sweeps per second, so no state is missed. A second run from the same save replayed the same deaths at the same times, so this is one timeline, not two samples.

| Death | Count | Per minute | How it reads |
|---|---|---|---|
| Shot down: state 2 to 4, then 5 | 3 | 1 | timers .60, 1.85, 2.11 against `rand*2 + .2` |
| Collision: 2 to 5, health below 0 | 8 | 2.7 | -64 to -17439, often near full health before |
| Instant death: 2 to 5, health exactly 0, usually full before | 8 | 2.7 | open space, far from hulls |

All eight collisions are close to enemy hull objects: four within about 100 m of `imp_cap1_comms` and the bridge, two beside `imp_frig3`, one by `imp_cap1_tur4`, one by `imp_cap1_sensor`. Most instant deaths fit the unseen kill above, which needs a human, so `--no-human` rounds have none. Three of them fall at one point near (-1570, 600, 3430), outside the `mapbounds` box and about 1500 m from any object, which a random pick would not explain. PC also kills a pilot's craft outright when its low level reports stuck. `trace:` that rule in the PS2 and whether those three are it. The port now has a stuck kill (§21, §24).

Port, seed 7, idle human, 600 s (`--space-report`): 111 shot, 0 crashed, 11 unseen; about 11, 0 and 1.1 per minute. In the 150 s suite run, 19 deaths: 6 credited to a flyer, 13 with no soldier credited (building turrets have none).

Hull lost on flying ships, 150 s, excluding the killing blow: PS2 8.2 per flyer per second (0.60 drops per second across 17.8 flyers), and 95% of flying time is spent in the top fifth of the hull. Port with 24 bots, 150 s: 42.7.

`measure:` the two rates that differ most. Shot-down is about ten times the PS2's, hull damage about five times, and collisions are zero against 2.7 per minute. A part of the first is the moment: the PS2 save has 12-14 live turrets and the port has about 25, at about twice the rate per turret
(section 17, nearest-target selection). Collisions may share a cause with the channel gap: the PS2's pilots fly close to hulls and the port's do not. Neither was tuned. Section 19 measured this and found the last two claims false, and traced the damage gap.

The four suite failures present at this point (`hud_layout::the_squad_cross_is_where_the_ps2_frames_have_it`, `squad_vehicle_maps::move_out_and_at_ease_reach_real_waiting_pilots_once_per_press`, `roster::aim_sizes_are_the_baked_models_boxes`,
`roster::fill_classes_keep_the_v0_fixtures`) also failed at commit `62c94571`. They are about HUD label position, squad orders and baked class data. They were not run on a tree without that commit.

## 19. Why the port's ships took five times the PS2's damage (2026-11-06)

Asked of the running PS2 and the code, not guessed. Everything is spa8. PS2: a private PCSX2 copy over PINE, no human flying, a save about 12 minutes into the round; runs from that save replay one timeline, so they are one sample. Port: `OPENBF_MEASURE_CREW=20 OPENBF_MEASURE_WARM=720 OPENBF_MEASURE_SECONDS=150`, `OPENBF_MEASURE_SEED` 1-6, `pilots_turn_on_armed_buildings_and_fire_their_secondaries`. "PC" is the reference of section 18. Neither side is a measurement of a human flying.

### Two wrong readings, corrected

- Section 18 says the PS2's pilots spend more time near enemy hulls, and that this may explain the channel gap and the missing collisions. Measured, it is false. Share of flying time by distance to the nearest enemy hull, under 100, 200, 400, 800, 1600 m and beyond: PS2 .021, .07, .17, .285, .421, .033; port .028, .087, .239, .26, .308, .079. The port's pilots were if anything closer. 97.8% of PS2 flying time is on splines.
- The same section says the strafe body is not in the repository. It is, in the original PC game (PS2 about line 253330). It is not what separates the two games here.
- Section 17's "12-14 armed turrets" and "turret shots about 3" were a partial count. A fuller read of the save gives 16 turrets that hold targets and 1.6-2.3 turret bolts per second.

### Where the PS2's hull damage comes from

PS2, 150 s, 17.6 flying, damage to flyers by attacker: armed buildings 81 hits, 15685; no owner (collisions) 7 hits, 6711; other flyers 6 hits, 2250. In total 9.3 per flyer per second. So turrets are about two thirds, and pilots almost nothing.

### What was missing, in the order it mattered

**Pilot hull repair.** `FUN_002f5478` (lines 271520-271625; PC) gives a seated pilot-class unit the class's `PilotSkillRepairScale` (+0x2fc, .005 live) of maximum health per second on ships above 1400 maximum health. `core/src/sim/flyers.rs::pilot_repair`, with `SoldierConstants.pilot_skill` set from the unit type (`game/src/roster.rs`). `re-vehicles.md` said no vehicle carries AddHealth, which is true of the ODF key but misses this.

**The side roll.** `FUN_0020b840` (line 121163) and lines 117385-117412 (PC): a pilot rolls aside (the conditions: `flyer_drive.rs::side_roll_wanted`), paying `EnergyTrickDrainSingleTap` and moving at `TrickSideRollStrafeSpeed`. A started roll costs that pilot's attackers aim: the AI's error takes +5, paid from its budget (PC). `vehicle/flyer.rs::{side_roll, roll_update, side_strafe, restore_energy}`, `sim/flyers.rs::start_side_roll`. 36 rolls per 150 s on average against about 28.

**The threat law for pilots and space turrets.** The port picked the nearest visible target. `FUN_002b94a0` (line 228514) scores each threat, returning 0 when a flyer observer has it behind its true nose (matrix Z, flyer +0x140). `FUN_002b9d10` updates a slot, `FUN_002b9c88` records a ray, `FUN_002b9cb0` forces. A switch needs 10 points more. PS2 live: 94% of emptied slots held a target behind the nose (n 66). Of 92 target drops in 90 s, 40 were removals for being behind and 39 were switches. Port: `core/src/sim/pilot_threats.rs` for pilots, `self_turrets.rs::threat_think` for turrets in a space round.

A flyer seen by something it can fight has that object forced into its own threats if it holds nothing (`FUN_002e43c8`, lines 258415-258418). So being fired on turns pilots on turrets, which is where the PS2's channel 1 time comes from: on channel 1 .53 (PS2 .46), holding an armed building .14 (PS2 .16-.18).

**The aim error on a flyer.** `FUN_002a4cc8` (line 213950), `FUN_002a50c0`, `FUN_002ae4d0`, `FUN_002ae7b8`; the PC agrees. The turn law aims at the target plus a bubble; the fire test's point has no bubble (`FUN_002ace58`). Radius: the target's horizontal box diagonal x (3 - 2 vis) x min(err x (1 + .6 speed/top), the flyer cap `lerp(4, 1.2)`), the weapon's `AIBubbleSizeMultiplier`, .3 when no human flies it, then `min(d / AIBubbleScaleDistDivider + .5, AIBubbleScaleClamp)`. Y is held above zero unless the target is flying. Live radii per class 9.54, 12.08, 12.36, 16.2, 9.61, 19.04 equal the model boxes for four of six. `self_turrets.rs::flyer_aim_error`, `vehicle_pads::FLYER_AIM_BOXES`. The flyer's own decay is halved (PC). This alone did little against bots, because the bubble is 1.6-10 m.

**The turret's fire line. This was the largest single fault.** The port and the PS2 share a turret's weapon timing: inside a burst .27/.37/.47 s, 1.00 s between bursts, on both. They differ in what the fire test reads. `FUN_002ace58` (PC `sIsTargetLinedUpPos`) takes C +0xd0 and C +0xe0, the eye point and **eye direction**: the turret's own axis (`mAimMatrix` Z, PC), driven by the turn law. It requires:

1. eye direction within 22.5 degrees of the led target;
2. the aimer's direction within 22.5 degrees of the eye direction (melee and droidekas exempt);
3. under 50 m, or a miss distance along the eye direction under 20 m (200 for a flyer, 50 for a hover, walker or flyer gun; a building turret gets 20, `FUN_002a8ef0`).

The port tested the aimer. The aimer reaches its target almost at once, while the turret itself has momentum: its turn rate may be 90 rad/s at full stick but its filter accepts only 10 rad/s per second, and the stick law `(1.5 sin e)^2` (PC sCalculateTurnControls, port `hover_drive::shaped`) does not brake in time. So a PS2 barrel swings through its target and well past it. One hold, in degrees off target: start 91, best 12, last 104. Another: start 157, best 10, last 122. Many of its 1.6 s holds never come on line and never fire. The port's law already matched (`Turn::step`, TurnRate 90.5, PitchRate 90, filters 10, limits equal once negated). Only the test was wrong, so the port fired while the turret was still swinging. `self_turret` now tests the eye direction in a space round.

| Turret, per hold | PS2 | Port before | Port now |
|---|---|---|---|
| Start angle median | 45 | 45 | 41-61 |
| Reach 5 degrees, p50 and p90 s | .37, 2.0 | .33, .90 | .33-.43, 1.3-6.2 |
| Holds that never reach 5 degrees | .46 | .15 | .35 (.22-.50) |
| Time within 5 degrees | .30 | .52 | .29-.53 |
| Holds with no shot | .59 | .34 | .54 |
| Shots per held second | .49 | .97 | .53 |
| First shot p50 and p90 s | .87, 2.81 | .43, 1.27 | .60-.83, 1.3-5.4 |

The port's "before" column measured the aimer for the angles, since that was its fire line.

### Result

Port, six seeds, 150 s each, against the one PS2 timeline. Seed 1, 2, 3, 4, 5, 6 then the mean:

| | Seeds | Mean | PS2 |
|---|---|---|---|
| Hull lost per flyer per s | 9.2, 12.4, 13.3, 14.2, 7.8, 7.1 | 10.7 (median 10.8) | 9.3 |
| Turret hits | 144, 170, 148, 76, 102, 124 | 127 | 81 |
| Turret damage | 24698, 30226, 25220, 12656, 17313, 20573 | 21700 | 15685 |
| Collision hits | 9, 2, 15, 31, 16, 0 | 12 | 7 |
| Collision damage | 4441, 477, 7803, 28314, 7814, 0 | 8200 | 6711 |
| Pilot hits | 8, 24, 78, 7, 3, 3 | 21 | 6 |
| Pilot damage | 6500, 7150, 13368, 11650, 2050, 880 | 6900 | 2250 |
| Turrets holding a flyer | 3.91, 3.70, 3.96, 2.82, 3.05, 3.03 | 3.5 | 3.2-4.4 |
| Turret shots per s | 2.0, 2.4, 2.2, 1.0, 1.1, 2.2 | 1.8 | 1.6-2.3 |
| All flyer shots per s | 29.2, 29.6, 34.2, 22.9, 28.0, 23.0 | 28 | 20.3 |
| Ships destroyed in 150 s | 19, 17, 15, 11, 10, 7 | 13 | about 16 |

Hull lost was about 32 per flyer per second when this work began, and 18.9 with everything above but the fire line.

### What still differs

- `measure:` the PS2 column is one timeline. The port's own spread (7.1 to 14.2) is wider than the remaining gap, so these rows cannot say whether turret hits at 1.6 times the PS2's, collisions at 1.7 times and pilot hits at 3.5 times are real. The PS2 needs a second save, from another moment of a round.
- `measure:` pilot hits come in a few close episodes: one pilot, on the nose, not its target, closing 58 to 17 m, on channel 0. That is the free burst into a ship ahead on the same spline. `trace:` whether anything on the PS2 spares that ship. PS2 hit rate .2% of bolts, port about .6%.
- `trace:` target changes. PS2 pilots hold about 147 targets per 150 s for 1.6/3.2/4.0 s; the port 82 for 4.0/4.0/8.8 s. PS2 turrets change about 1.8 times as often as the port's, for about the same time holding. No cause is known, and damage does not seem to depend on it.
- OURS, `trace:`: a pilot's stick stays neutral during a roll, without PS2 evidence. Flyer candidates must have an occupant (`IsSuitableEnemy` not read). Stations score relative 15 whether armed or plain, where PC `GetRelativePriority` has 10 for a plain building. The awing and xwing rows of `FLYER_AIM_BOXES` are the live PS2's 12.08 and 16.2, because the bake gives 10.94 and 15.97 (`trace:` why). Turret FORCEDSEEN is not ported (2% of live turret slots). Neither are the camera clause `FUN_001ba840`, the flag carrier clause, the medal repair clause `FUN_00360018`, the PC's fake ray hits, soldiers as fallback pilot candidates, or `IsTargetLOSPossiblyBlocked` (`FUN_002b8028`), which needs last-shot distance kept per turret.
- `replace:` ground rounds keep the old model: self-piloted turrets and pilots outside space still pick the nearest target, and ground self-turrets still test the aimer. All of this applies on the PS2 (nothing in these functions asks whether it is space), but no ground PS2 comparison has been made, and the gate hashes hold because of the split.
- `measure:` a human near the enemy destroyer, which is the maintainer's complaint and is still not answered. See below.
- A human pilot on the PS2 has still not been measured, and that is what the maintainer flew against. The law takes a human through its own terms (view doubled, think level, facing score, full aim bubble), but those are unmeasured against the game.

### A human ship near the enemy destroyer

The threat law takes a turret's view from the eye direction: inside 10 m always, otherwise within half-angle 77.5 degrees (PC `CanPotentiallySee`), then a clear line. The one skip of the cone is for the human's reticle target at the highest think level (PC, `UpdatePotentiallyVisible`). `self_turrets.rs` passes the turret's axis as the view. The old nearest-target rule saw all round, so this is a change the maintainer will feel, and it follows the code without being measured against a human on the PS2.

Port, with no other ships, a human ship parked at (-332, 247, -496): 14 of 15 enemy turrets had it outside their cone or range, and the nearest had it in cone behind a hull. None held it. Parked inside a turret's view with a clear line, 300 m along its axis, it draws fire and takes hits (`a_human_ship_in_a_turrets_view_draws_its_fire`).

Port, a full round (`OPENBF_MEASURE_HUMAN=1`, crew 20, warm 720), a human ship held still 300 m outward of the enemy turret nearest the hull's centre, seeds 1-6:

| Seed | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| Alive, s (the run stops at 120) | 18.0 | 120 | 25.3 | 33.1 | 3.3 | 13.3 |
| First damage, s | 10.8 | 15.5 | 20.8 | 31.6 | 2.7 | 4.3 |
| Turrets holding it, mean | .93 | 0 | 0 | 0 | 0 | 2.7 |

In four of the six the mean rounds to zero, so almost no turret held it and the damage there came from something else. The probe does not record the attacker. A ship held still is not a ship being flown, and these numbers have no PS2 column.

`measure:` the way to get that column: the maintainer flies at the enemy destroyer on the PS2 while its turret slots, its bolts and the human ship's health are read over PINE (read only). Compare time to first hit, turrets holding the human, and time to death with the same flight path in the port.

Paths that force a threat on the PS2 and are not ported for turrets (`trace:`): `AudibleManager::UpdateAudience` (radius capped at 30 m; over 3 m it gives `AudibleResult` an unknown status, closer it forces; firing sets the radius to 100 before the cap), PC (a seen target's handler forces the seer), PC `AddFakeRayhitResult` (both ways, when hit by a soft collision and the bolt has lived under .4 s). Any of these could put a turret on a human the cone would miss.

`FLYER_AIM_BOXES` also holds `all_la_at_gunship` and `rep_la_at_gunship`, 29.257, from the baked model's box like most rows. No gunship was among the six classes read live. `measure:` one when a save has one flying.

## 20. System repair, shield impact FX, and radio chatter verification (Issue #123)

### System repair and restore rules
- **Decomp evidence:**
  - `FUN_00268c78` (line 185043): `EntityBuilding::SetProperty` handles property `0x19971f1b` (`fnv1a("maxhealth")`), setting *(decompiler detail omitted)*.
  - `FUN_0026a7b8` / `FUN_0026afe4` (line 186122): `EntityDestructibleBuilding::SetProperty` handles `0x19971f1b` (`max_health`), setting *(decompiler detail omitted)*.
  - `FUN_001df3d8` (lines 90273–90321): `HealthProperty::SetHealth` applies health changes and calls virtual `OnHealthChanged` at offset `+0x28`.
  - `FUN_001df4e0` (lines 90324–90345): `HealthProperty::SetShield` updates current shield pool.
  - `FUN_001df560` (lines 90351–90390): updates regeneration toward `max_health` and `max_shield`.
  - `FUN_002f5478` (lines 271520–271625; the PC agrees): pilot repair skill adds `PilotSkillRepairScale` (0.005) of max health per second.
  - `objectivespaceassault.luac` (closure `main.1.0`): `OnCriticalSystemDestroyed` invokes `SetProperty(obj, 'MaxHealth', 99999997952.0)` (`SPENT_MAX_HEALTH`).
- **Can destroyed critical systems be rebuilt on PS2?**
  - **No.** Setting `MaxHealth` to `SPENT_MAX_HEALTH` (`99999997952.0`) prevents any repair tool (`FUN_00346fa0` line 339872, `core::sim::repair::repair_target`) from ever completing the repair to restore the station.
  - **Before 0 HP:** damaged critical systems can be repaired normally by fusion cutter or pilot repair up to their authored max health.
  - **Non-critical linked systems:** `shieldgenerator` (`linkedshields.luac`) and `mainframe` (`linkedturrets.luac`) do not receive `SPENT_MAX_HEALTH` on death. When repaired back to 100% health, they restore (`s.destroyed = false`, live collision restored, shields and auto-turrets reactivated).
  - **Port alignment:** `core::sim::space_assault` (lines 377–382) and `core::sim::repair` (lines 136, 362–366) match PS2 rules.

### Shield impact visual feedback
- `FUN_003148f0` (lines 292899-292900) gives every bullet two shield effects: `com_sfx_shieldimpact_sm` at +0xfc and `com_sfx_shieldimpact_xl` at +0x100. The `ImpactEffectShieldSmall` and `ImpactEffectShieldLarge` setters (lines 293108-293110, 293139-293141) both write +0xfc, so authoring either replaces the small effect and the large one stays at its default. `ImpactLook::apply` does the same.
- The hit handler (lines 292458-292470) reads the struck primitive's bounding-sphere radius through `FUN_00164ff0`. Below 16.0 (`DAT_005fea68`, read from the ELF) it plays the small effect, otherwise the large one. The port's `CollisionWorld::instance_radius_sq_at` stands in for that radius: the smallest body of the station's instance containing the hit, by AABB half-diagonal. OURS, `measure:` the struck primitive's sphere on PS2 for a Star Destroyer prim under 16 m.
- Measured on spa1, spa3 and spa8: nearly every shield-group body is over 16 m, so these hits use the large effect, and a few Star Destroyer prims (12-14 m) use the small one.
- `imp_cap_stardestroyer_shield`, `sensors_spin` and `sensors_spin0` have a `prop` base and stay out of the shield groups. Hull classes are `armedbuilding` with MaxHealth 999999 and AddHealth 50000, and were `Station`s before this work.
- Several shield objects have `Team` 0 on the instance (every hull on spa3, `all_cap_rebelcruiser3` and `shield_r`/`shield_l` on spa1). They stay teamless in the port, so any side's shots drain the pool through them and pilots do not rank them as threats. `measure:` friendly fire on such a hull on PS2.
- `SetParticleLODBias`: spa3, spa6-spa9 Assault use 15000; `spa1g_ass.luac` has no such call (`spa1g_c` and `spa1g_1flag` use 3000). `FUN_0037a008` resets the bias to 0 from `FUN_00174b28`, so spa1 Assault runs at 0 in the port. `measure:` the bias tiers `DAT_0068226c`, `DAT_0068228c` and `DAT_006822ac` live in spa1 Assault.
- Once shields collapse (`shield == 0.0`), bolt and torpedo impacts route to `Surface::Building`, emitting static hull debris/explosions. The hit that exhausts the shield follows the post-damage rule measured in section 26.

### Shield ring orientation (playtest fix, 2026-11-07)
- The hit handler passes the hit normal to `FUN_00313c40`, which offsets the effect by 0.1 along it and hands it to `FUN_00452330` as the frame's z axis. The port now builds the same frame (`game/src/fx/ingest.rs`, `shield_normal`).
- `com_sfx_shieldimpact_xl`'s Rings are BILLBOARD with `NoRegisterStep`, and re-fx.md (BF2 differences) says those axes follow the emitter node. `emitter_draws` applies the node rotation for that case only, so BF1 and other BF2 billboards keep their world axes.
- OURS `measure:` `Event::Impact` carries no normal, so the normal comes from a 2 m static ray either side of the hit (`SHIELD_NORMAL_PROBE`), falling back to the reversed bolt direction. `replace:` with the struck primitive's normal if the event ever carries one.
- `measure:` the rings' darkness against the PS2. The coplanar world-axis quads overlapped before this fix, so overlap is the suspected cause of the extra brightness. No gain factor was added; re-glow says HDR is level-neutral.

### Bomber gunner camera (playtest fix, 2026-11-07)
- `SWITCH_WEAPON` while mounted switches seat (`core/src/sim/guns/seat_switch.rs`), so the wheel moves a TIE Bomber pilot to `FLYERSECTION TURRET1`. The fallback `vehicle_camera` flattens the view to the heading and world up, which is wrong on a pitched flyer.
- A flyer gunner now takes `turret_seat_camera`: `TrackCenter` and `TrackOffset` on the aim frame, per re-turret-draw.md section 4. That section is about building turrets, and re-seat-hud.md marks the flyer gunner's seat evidence [H]. `measure:` a PS2 bomber gunner's view through the oracle.

### Spawn briefing, lock audio, and flight radio chatter
- `FUN_0018f7f0` (line 34252): Lua `BroadcastVoiceOver(name, teamMask)` invokes `FUN_003e2648(hash, mask)`.
- **No opening spawn briefing:** an earlier port queued `<P>OSMP_obj_01`/`02` at first spawn. No mission script on the disc names those lines (a search of `extracted/bf2/lua` finds only `obj_03` and later), so the rule was invented and is removed. `measure:` the PS2 at a space round's first spawn, to confirm no line plays.
- **Ship loudspeakers:** see `re-ambience-music.md`, "Sound trigger regions".
- **Flight radio chatter:** `FUN_0044c650` (lines 497625-497632) requests `Cockpit3rdPersonSound` (+0x44) or `Cockpit1stPersonSound` (+0x3c) on every engine update when *(decompiler detail omitted)* is set. Its flyer caller (118865-118878, 118925) sets that only when a local player controls the craft. All 19 flyer classes author both keys with `<side>_inf_com_space_chatter`, and the record's `play_interval (12.0, 3.0)` is what spaces the lines.
- The port asks each tick while the player holds a flyer's pilot seat (`Commander::cockpit`, `Call::Cockpit`) through `gate_calls`, 2-D at the ears. This replaces an invented 10 s timer that played on foot too and went through the one-shot path, which does not play a stream record.
- `measure:` whether the PS2 stream voice overlaps commander lines. The port shares the one voice queue, so a commander line delays chatter.
- Not covered: on-foot unit lines inside a capital ship. They belong to `crate::vo`, and spa1's names table has no `inf_chatter`, `_command_` or `ai_` entries.
- **Lock-on & incoming missile audio:** corrected in section 14. A flyer's lock plays nothing, and the incoming beep is untraced.

## 21. Flyer impact presentation, transport fallback, stuck-kill, and 6-map Space Assault sweep (Issues #122 & #124)

### Flyer impact FX & controller rumble (`#124`)
- `Event::FlyerImpact { vehicle, pos, normal, damage, lethal }` spawns `flyerspray` (with `com_sfx_ord_exp` fallback when unbaked) at the contact point (`game/src/fx/ingest.rs`) and triggers heavy/light controller rumble (`Rumble::blast` on lethal impact, scaled jolt when occupied by the local player in `game/src/rumble.rs`).
- Crash death camera (`WalkSim::camera` in `game/src/walk.rs`) detaches at `0.25x` follow stiffness during `FlightPhase::Falling` and holds the post-crash view for `seat_view_timer` after `FlightPhase::Crashed` destruction.

### Transport spline fallback & wedged stuck-kill (`#122`)
- **Transport spline fallback (`core/src/ai/flyer_drive/admission.rs`):** On maps that omit explicit `transport` (`0xd32f6312`) flyer paths (`spa6`, `spa7`), transport AI pilots fall back to friendly fighter spline classes discovered from live friendly flyers and vehicle pads (`fallback_transport_classes`), and request takeoff/landing transitions around enemy hangar landing regions.
- **Wedged BF2 stuck-kill (`core/src/sim/seats.rs`, `core/src/sim/flyers.rs`, `core/src/ai/flyer_drive.rs`):** Matching PC (§18), a BF2 flyer in flight whose wedge counter reaches `WEDGED_GIVEUP` (`3`) is destroyed outright along with its seated bot pilot instead of ejecting a pilot into empty space.
- **Hangar mouth departure (`core/src/ai/flyer_drive.rs`):** When spline connection chords and loiter circles are blocked by hangar walls, `connect` probes `clearest` and only emits an exit curve if a full `WEDGED_RAY` (`200 m`) ray into open space is unobstructed, preserving `curve == None` inside enclosed boxes.

### Verified 6-map Space Assault sweep (`examples/space_sweep.rs`)
Command: `cargo run --release --manifest-path engine/Cargo.toml -p game --example space_sweep -- --max-ticks 3600` (seed `42`, score limit `180`, 120.0 s simulated time per map):

| Map | Seed | Outcome | Sim Duration (s) | Points | Stations Destroyed | Flyers Shot | Flyers Crashed |
|---|---|---|---|---|---|---|---|
| `bf2_spa1` | 42 | Time Limit | 120.0 | 31 - 11 | 0 | 13 | 9 |
| `bf2_spa3` | 42 | Time Limit | 120.0 | 5 - 3 | 0 | 2 | 52 |
| `bf2_spa6` | 42 | Time Limit | 120.0 | 4 - 2 | 0 | 1 | 8 |
| `bf2_spa7` | 42 | Time Limit | 120.0 | 0 - 0 | 0 | 0 | 17 |
| `bf2_spa8` | 42 | Time Limit | 120.0 | 2 - 4 | 0 | 4 | 3 |
| `bf2_spa9` | 42 | Time Limit | 120.0 | 12 - 13 | 0 | 6 | 5 |

## 22. Seat-launched guided rockets (2026-11-07)

`imp_fly_tiebomber_sc`'s `TURRET1` holds `imp_weap_fly_tiebomber_guided_rocket`, a `remote` weapon whose ordnance is a flyer. The gunship and troop transport have the same kind of weapon. The on-foot path in `re-guided-rockets.md` ends a rocket when its owner is mounted, and the seat path fired a plain round.

- The port now launches a player's seat rocket from the seat's muzzle and aim (`Sim::launch_seat_guided`) and records the seat on the rocket. The pad routes to the rocket while the owner stays in that seat, and the camera follows it. ACTION returns control and SECONDARY_FIRE ends the flight, as on foot. The class's `0 -3 20` offset is applied in the aim frame.
- OURS `measure:` the offset's reference frame and whether the craft keeps flying while its gunner steers, both against a PS2 bomber.
- OURS `replace:` a bot gunner still fires the old plain round. Bots steering seat rockets are not traced.

## 23. Frigate destruction (#137, measured 2026-11-11)

One native death was recorded on the PS2 (USA v2.01, `SLUS-21240`, CRC `02F4B541`) from save state `.10`, which is `spa8g_ass` (the mission name is in RAM and the HUD reads 0/180 on both sides). Dumps and states are under `engine/artifacts/lead_tmp/issue-137/`; the Tracking run is `01M756MJWQ35AQCYF05V638W1K` [M]. The managed launch record identifies emucap server build `8587968`; emulator status reports PCSX2 build string `PCSX2 fd9d310` and host commit `fd9d310ccbb6b8b62c976da8886a3c8fd3a10ff3`. The executable's PE version fields report `0.0.0.0`, so no release version is available from this capture record. The port comparison is at source `39914a2` plus the working-tree parser change.

### Census

A frigate is a `destructablebuilding` with `MaxHealth 90000`, `ExplosionName spa_prop_frigate_destruct_exp`, no destroyed or unbuilt model, and its hull cut into chunk slots. Every slot is `ChunkPhysics FULL`, `ChunkGravity 0 0 0`, `ChunkTerrainCollisions 0`, with a small `ChunkOmega` and the key `0xc74f3df1` set to 1.

| Class | Slots | Trails | Maps (Assault frigates) |
|---|---|---|---|
| `all_prop_blockaderunner` | 2 | `spa_sfx_frigate_exp_1`, `_2` | spa1, spa9 |
| `spa1_prop_minidestroyer2` | 3 | `_3`, `_4`, `_4` | spa1, spa8, spa9 |
| `spa1_prop_correliancruiser` | 4 | `_1` on three, none on the fourth | spa1, spa8 |
| `spa2_prop_minifedcruiser` | 4 | `_5`, `_5`, none, `_2` | spa3, spa6, spa7 |
| `spa2_prop_minirepassaultship` | 5 | `_5`, `_1`, `_5`, none, none | spa3, spa6, spa7 |

| Comparison | PS2 [M] | OpenBF | Coverage |
|---|---|---|---|
| Death and lifetime | `spa8g_ass`, Corellian cruiser: hull hidden and collision removed on death; four pieces remain at 7.5 s and are gone by 8.5 s | Eight-second space lifetime is covered by the frigate scenario for all 22 scoring ships across six maps | Native runtime sample covers one class/map; the shared reader/lifecycle path and per-class authored fields cover the rest statically |
| Chunk body | Template byte `+0xdd` registers the piece collision body; the first cruiser piece's recorded body frame is rolled about 9 degrees | Map collision prims and local transforms now load from each map manifest; a real-data test checks every frigate class/map appearance, all authored Omega values, and the spa8 cruiser box dimensions/offset | Template and asset parsing are checked across all appearances |
| Visual sequence | `f000`, `f060`, `f240`, `f450`, `f480`, `f510`: visible fire and hull fragment at 1 and 4 s, fading fragment at 7.5 and 8 s, no fragment at 8.5 s; residual smoke remains [M]. Replayed from the live TIE-fighter viewpoint in Tracking run `01M76CZ7N26MRSRCC4D2YQP1K5` [M] | A focused renderer capture asserted `StationDestroyed` for the same class/map and sampled ticks 0, 30, 120, 225, 255 (0, 1, 4, 7.5, 8.5 s). At 60 m, blast/smoke is visible at 1 s and chunk trails remain visible at 4 s; at approximately 430 m, the trail effect is beyond its 90 m draw range [M] | Native and port captures show the same death-to-fade timing. Their camera poses still differ, so image scale, particle brightness and exact visual alignment remain unmeasured |
| Live piece collision | Four active pieces at 4 s; after placement through each piece, the flyer path converged to the no-piece control by frame 30 (`A0` and `C0` both `[-1065.421, 8.820, -273.759]`) [M] | Port pieces are visual effects and do not block flyers | No lasting flyer stop or deflection in this native control; the template body still registers |

The missions list 2 scoring frigates on spa1 and spa3, 6 on spa6 and 4 on spa7, spa8 and spa9, 18 points each. The capital hulls (`*_cap_*`, `cis_fly_fedcruiser*`, `rep_cap_assultship*`) are separate objects and are not destroyed by this path. The `*_destruct*` and `*_damaged*` classes are the critical systems' own props and their pre-authored broken states; none of them is a frigate stage.

### The recorded death

Setup: state `.10` loaded and stepped. Two other frigates had their health written down to 300 (logged interventions) and were not hit in the window, so they play no part. The frigate that died, a Corellian cruiser at (-1079.07, 0, -240.32), was never written: it fell from 90000 to 15708 in the first 60 s under ordinary fire and died 7.8 s later. The break on `FUN_001e3638` showed the call chain ordnance hit `0x313c40` -> damage `FUN_001dfb38` -> handler `FUN_001e3458` -> death `FUN_001e3638`. Frames below are 60 Hz video frames after that entry; the game ticks at 30 Hz.

| Frame | Seconds | Observed in RAM |
|---|---|---|
| 0 | 0 | health 0, state word 0, hull body and visible flag still set |
| 1 | 0.02 | state word 1, alive bit clear, repair type 5, collision body pointer (entity +0x10) cleared, visible byte (entity +0x118) cleared |
| 3 | 0.05 | chunk cluster `0x16f16d0` holds four pieces with timer 0.033; the team's score word `0x1dc7ba0` reads 18, up from 0 |
| 60 | 1.0 | timers 0.967; pieces turning in place |
| 240 | 4.0 | timers 3.983 |
| 450 | 7.5 | timers 7.483 |
| 510 | 8.5 | timers 8.016, all four pieces cleared |

The native frames and RAM timeline are in `engine/artifacts/lead_tmp/ps2-death-fixed/` [M]; the collision control is in `engine/artifacts/lead_tmp/ps2-chunk-block/block.json` [M]. In `f060`, the released hull section is surrounded by bright fire and smoke. At `f240`, the same section has rotated, with smaller flame patches remaining along it. At `f450` and `f480`, its mesh is translucent beneath smoke and fire; at `f510`, the mesh is gone while a small smoke/fire patch remains. A temporary OpenBF renderer harness in `engine/artifacts/lead_tmp/issue-137/port_exact_death_capture_harness.rs.txt` asserted the death event and rendered the same class/map at ticks 0, 30, 120, 225 and 255; the paired close and far output sets are in `port_exact_death_close_2026-11-11/` and `port_exact_death_far_2026-11-11/` [M]. The close-camera run shows trail smoke at 4 s. The far-camera run does not: the authored effect's `BoundingRadius` is 5, and `fx/draw.rs::far_distance_with_bias` gives that effect a 90 m draw range. This accounts for the apparent gap in the far view; the PS2 camera distance was not measured, so this is not a direct effect-range parity comparison.

**Replayed native image sequence (2026-11-11).** After the PCSX2 Control server was reconfigured, I loaded `engine/artifacts/lead_tmp/ps2-death-fixed/fixed_entry.p2s` into the bound managed session and captured frame-stable 596x447 screenshots at offsets 0, 60, 240, 450 and 510 with PINE. The images and per-frame SHA-256 values are listed in `engine/artifacts/lead_tmp/issue-137/native-visual-replay-2026-11-11.txt` [M], registered under Tracking run `01M76CZ7N26MRSRCC4D2YQP1K5` [M]. Bright fire/smoke is visible by 1 s; multiple burning hull sections remain at 4 s; a translucent piece is still visible at 7.5 s; it is gone at 8.5 s with a smoke/fire tail. This confirms the visual lifetime from a live combat viewpoint. The same state replayed through OpenBF with camera offsets of 60, 150 and 300 m; those are still not a camera match, so no pixel-scale or brightness conclusion is claimed.

The frigate stays in state 1, hidden, for the rest of the window. Its class's `RespawnTime` reads as the largest float, so the regeneration `FUN_001e3638` sets is zero and it never rebuilds.

So the sequence is one event with a tail, not a staged script. On the death tick the hull is hidden, its collision is removed, the explosion ordnance is fired, the points are scored and the hull's chunk pieces are launched. The pieces then turn slowly for 8.0 s and are removed. `DeathAnimation` takes no part: `FUN_001e3638` (B2:93584-93789) has no animation call, and nothing moved the hull entity.

What moves each piece:
- **Speed.** The body centres barely move: about 0.002 m in 0.45 s on the first slot (`ChunkSpeed 0.03`). The model origins travel 1 to 10 m/s only because each piece turns about a body centre 70 to 120 m from the frigate's origin.
- **Spin.** The template keeps `ChunkOmega` as authored (first slot (0.03, 0.01, 0.01)). The piece's world angular velocity is that vector carried through the piece's body frame, with no sign change: the first slot's body frame is rolled 9 degrees and its world value reads (0.0312, 0.0052, 0.0100). After one tick the model's X axis reads (1, 0.000333, -0.000173), which is that rate for 1/30 s.
- **Life.** 8.0 s from launch. A map without terrain runs the life timer from the first tick (`re-chunks.md` §4).
- **Body.** The key `0xc74f3df1` sets the template's byte +0xdd. `FUN_001d5b00` then registers the piece's body even though the map has no terrain (B2:82835) and leaves out the exclusion it otherwise adds for one collision layer (B2:83026).
- **Numeric property parsing.** `spa2_prop_minifedcruiser` authors `ChunkSpeed = 4 .00` on its first slot. The native property-number reader `FUN_005200b0`, case 4, converts the leading numeric token, so this spelling yields 4.0. OpenBF's full-token `f32` parse previously rejected it; the reader now accepts the leading value. The focused parser test covers this spelling and signed/exponent forms. This is a reader trace, not a live launch measurement.

### What the port did, and the changes

The port already hid the hull, emptied its collision, scored and launched the authored pieces on the death tick (`core/src/sim/stations.rs` `station_dies`, `install_station_wreck`; `game/src/fx/chunks.rs` `StationDestroyed`). Three things differed, and made the frigate seem to vanish or turn about the wrong point:

1. Pieces lived 3 s everywhere, the `[ours]` rest life chosen for ground wrecks. In a space round the timer starts at launch, so a hull piece was fading after one second. A space round now uses the PS2's 8 s (`SPACE_LIFE`, `Piece::life`). Ground rounds keep 3 s.
2. `ChunkOmega` had its X and Y negated and was used as a world vector. It is now kept as authored and turned by the launch frame (`Chunks::launch_posed`). This is the one init function for every chunk, so it changes the turning sense of all pieces, not their rates.
3. The map bake already carried a collision prim for each frigate chunk, but `SideData::load` read only side vehicle and droid manifests. Map chunks therefore used a sphere around their GLB bounds. The loader now reads map `manifest.ron` collision prims before side data. The spa8 cruiser regression check confirms its first chunk uses the authored box half-extents `(64.03713, 10.072614, 108.157265)` and local translation `(5.107916, 19.034204, 72.342255)`; the multi-map check confirms non-sphere prims and unchanged authored Omega for all 22 scoring frigates' class/map appearances.

### Checks

- `cargo test -p game --lib fx::chunks`: 23 pass, 4 ignored. New: `a_space_round_keeps_a_piece_eight_seconds_and_fades_its_last_two`, `authored_omega_turns_with_the_launch_frame_as_the_ps2_cruiser_piece_does` (the recorded frame and rate), `a_whitespace_split_chunk_number_reads_its_leading_float_as_the_ps2_does`, and `map_frigate_chunk_specs_keep_authored_prims_and_omega_across_all_maps`.
- `cargo test -p game --test space_destruction_scenarios`: `baked_frigates_break_into_their_hull_pieces_for_eight_seconds_on_all_six_maps` kills all 22 scoring frigates of the six Assault missions (all five classes). Each one survives a hit one point short, then dies once: one `StationDestroyed`, 18 points once, no live collision layers on its bodies that tick, the authored piece count, pieces gone after exactly the 8 s tick count, and the score unchanged afterwards.
- Temporary serial GPU capture: `scripts/gpu_tests.sh --game capture_staged_frigate_death_at_native_comparison_times` passed after asserting the `StationDestroyed` event and four launched pieces. It rendered exact 0, 1, 4, 7.5 and 8.5 s frames in both a close and a far camera; harness source and captures are in `engine/artifacts/lead_tmp/issue-137/` [M]. The far camera demonstrates the trail's 90 m distance cull; it is not a visual mismatch against the PS2 sample.

Playtest pack: `cargo build --release -p game`, followed by `game.exe --write-pack` of the current `assets_baked` and `missions` (28,729 files). The staged binary loaded embedded `assets_baked/bf2_spa8` in Assault mode and reported 790 draws, 178 instances and zero untextured draws. `dist/openbf.exe` was replaced after that smoke check; SHA-256 `ac21d797b4a3cd2d727a11f32abe3801820af1fef8a1aa8365f166fffe190ab6`. `dist/openbf_data/profile.ron` and its `playtest` directory were preserved.

### Not done

- The native frames show the death and its burning chunk. The port capture confirms trail smoke at 4 s only from within its effect draw range, but the PS2 camera position is not known. Exact explosion scale, trail density/brightness and image-space alignment of the prim-centered drift/rotation remain unmeasured.
- Native live-death captures cover one class/map. Other classes share the same native lifetime and property reader, and the real-data scenario covers all 22 scoring frigates, but a per-class native launch sequence was not captured.
- The controlled native flyer path shows no lasting deflection from active pieces, matching the port's non-blocking effects in this probe.
- The `[ours]` 3 s ground life and the failure of `real_sbd_and_magnaguard_profiles_launch_five_pieces_at_the_authored_frequency` (345/1000, the same without this change) are untouched.

## 24. Ground maps after the space merge (2026-11-10)

A review of the 11-08 merge for side effects on Conquest and other ground modes.

- **Stuck kill.** Both PS2 fly agents kill the craft a bot flies when its path follower reports stuck (value 2), before any
  state logic and with no mode test: BF2 `FUN_002c87c8` (B2:251620-251627, the stuck word read by `FUN_002ba940` and a full
  instant-death hit through the health object) and BF1 `FUN_00246d80` (B1:162418-162426, `FUN_001bbc98` with the instant
  flag). So the port's kill of a BF2 craft wedged `WEDGED_GIVEUP` times in flight stays in every mode, and costs a ticket in
  Conquest as the originals do. A craft that never died would also hold its pad. `trace:` whether the PS2 flyer's follower
  reports stuck in flight at all; if it does after one 3 s window, `WEDGED_GIVEUP` falls to 1.
- **Riders of a destroyed vehicle.** The PS2 kills each rider in his seat (re-vehicle-death §10). The merge stood every rider
  at the hull origin before the kill, which moved a building turret's drawn gunner off his seat into the turret. Each rider
  now takes his carried pose (`rider_pose`): the origin for flyers, hovers and walkers, as before, and his seat for a turret
  gunner.
- **Leaving in flight.** §10: pilot and gunners held, passengers free, except in space modes.
- **Mesh-only sphere and terrain fallback** (`flyer_contact.rs`). Every baked flyer and command-flyer class has at least one
  non-mesh prim in its building subset and one in its terrain subset, so neither fallback runs on any shipped craft. Separate
  and older: the PS2 puts the `coll` mesh in the building subset of several BF2 craft (gunship, V-wing, A-wing, droid
  fighters, droid gunship, ARC-170), and the port's contact skips mesh parts. `measure:` a landing in a hangar by one of those.
- **Probe layer.** `Layers::FLYER` is given to every map body and every vehicle hull by the same class rule, so capital and
  ground-vehicle hulls carry it; the probes already see all other hulls. Probing `FLYER | VEHICLE` would widen the probe to
  bodies a craft flies through. Only the comments were stale. Older mismatch, left alone: contact with a flyer- or
  walker-owned body uses its building subset while the probes use FLYER.

### Issue 145 investigation checkpoint (2026-11-10)

[O] Source under inspection: `55d7445` (briefing checkpoint `2b9cec49d634`). The issue has no comments. The later report adds a hangar takeoff particle plume and missing red ship lighting; these are separate observations, not proof that they share the flight-streak system.

The PS2 executable contains a dedicated candidate for the flight effect. `FUN_00460418` (current BF2 decompilation lines 510793–510840) allocates two banks of 80 entries and clears their lifetimes. `FUN_00460928` (511061–511367) decreases lifetimes, rejects entries behind the view, advances surviving positions, and creates entries at a rate dependent on the magnitude of a vector obtained from the view-associated object. It uses a separate random sequence in the original; that does not authorize consuming the port's gameplay RNG. `FUN_00461080` (511372–511520) selects the `spacedust1` texture and submits two geometry pieces per active entry. Its line length interpolates between two globals using the same associated-vector magnitude; the draw also uses a view-depth-dependent alpha. This is a strong lead, not yet a demonstrated identity for every streak in the screenshot: the globals, object velocity, update timing and live entries need to be measured before implementation. `FUN_00460548` (510855–510987) reads configuration into those globals. The texture name alone is not distinctive: the CTF delivery marker also uses it (§ related note in `re-ctf.md`).

The managed PCSX2 run `01M73RJKGVF1K5VXR2E3KZJT8C` loaded the supplied states with no memory writes. Frozen-boundary EE reads found `spa8g_ass` and `spa8_ass` in `.10`, and `spa8g_1flag` and `spa8_1flag` in `.09`; the latter must not be treated as Assault. In `.10`, eight frames were captured at five-frame intervals after loading, with no injected flight input, under `engine/artifacts/lead_tmp/issue-145/flight-*.png`. The view contains small pale streaks, but the craft speed and camera pose were not measured, so this is not a matched motion-law sequence. `.09` was loaded separately; cross, triangle and square were tried and recorded in Tracking, but did not board the craft. The adapter advertised digital inputs only. The managed generation was stopped by its exact launch ID. Raw images and the local checklist remain untracked.

The latest available playtest black box at investigation time was `dist/openbf_data/playtest/last.json`: `cargo run -p game --example blackbox_read -- ../dist/openbf_data/playtest/last.json` identified build `d7211719a9cc +dirty`, `bf2_spa6`, `1flag`, seed 309, 184 ticks. It is a later snapshot than the report's seed 292 and does not establish flight behavior. Its 75 fps and 13.3 ms mean are a whole-frame measurement, not the cost of the missing effect.

For the added lighting report, the existing BF2 shadow-region investigation is in `re-model-lighting.md` § BF2 interior shadow regions. The port gathers regions for skinned character draws in `engine/crates/render/src/frame/submit.rs`, while the rigid vehicle commands in `engine/crates/game/src/viewer/vehicles.rs` currently do not provide a lighting sphere. This is an actionable lead, but the appropriate original vehicle sphere, region selection and resulting image still need verification. The takeoff plume's native system and inputs also remain untraced.

Unchecked: matched rest/cruise/boost/turn sequences and both cameras; native configuration and all six map/mode inputs; pause and repeated submission; targeted motion, loading and boundary tests; serial headless GPU comparisons; rendering cost and simulation hashes; the takeoff plume and ship-lighting correction. No effect implementation or parity claim is made at this checkpoint.

### Follow-up measurements and rigid lighting

[O] At source `3ae7c545`, the `SpaceDust` effect identifier hashes to `0x3757a565`, the identifier registered by `FUN_00460348` (510754–510765). The six local PS2 space map LVLs (`SPA1`, `SPA3`, `SPA6`, `SPA7`, `SPA8`, `SPA9`) contain byte-identical 344-byte `SpaceDust` configuration scopes. The local scope's SHA-256 is `63fe9fe89e4cc6f0f776c0b6414e87d134f8844a345f50747c29866fcfb9df59`; it enables the system and names `spacedust1`. A frozen `.10` Assault read matched the 12 numeric values in that scope to the live globals: 150, 70, 20, 0.02, 0.1, 0.5, 35, 100, 2, 12, 35 and 170. These are configuration values, not yet a completed interpretation of their units or a mode-by-mode runtime census. Tracking run `01M73SEXSTJGWY3Q18JK3PRYB4` records the load and observations; its exact managed generation was stopped. The next step is to bake these properties from the player's LVLs and verify the spawn geometry, lifetimes and presentation clock against the live bank before drawing them.

[O] For the added hangar-lighting report, BF2 `FUN_00597e98` calls the region gatherer `FUN_0059b2f8` with a model's bounds (717154–717163). The port now submits each vehicle's side-baked model sphere to the existing BF2 shadow-region blend, including its specular pass. The rigid shader receives region terms separately from culling and fade inputs. This is a targeted correction supported by the supplied comparison and the model lighting path; a matched original/port capture is still needed for visual sign-off. The takeoff image remains a separate investigation: BF2 parses `ThrustEffect` in `FUN_00211a40` (124203–124223), but that alone does not establish that it produces the depicted ground plume.

Validation of the rigid-lighting change: `cargo check -p game`, `cargo clippy --workspace -- -D warnings`, `cargo clippy -p render --tests -- -D warnings`, and `cargo clippy -p game --lib --bin game --test space_playtest_fixes --test space_secondary_weapons -- -D warnings` passed. `cargo test -p game --test space_playtest_fixes` passed 9/9; `OPENBF_REQUIRE_SPACE_ASSETS=1 cargo test -p game --test space_secondary_weapons` passed its matrix test. The serial `scripts/gpu_tests.sh --render` renderer suite passed 84/84, including the new rigid-region/cull test; the subsequent full golden-gallery phase was interrupted and is not claimed as passed. `scripts/size_report.sh --check-map` failed on pre-existing map drift (3 missing entries, 2 missing paths, 139 stale counts) and existing oversized files; touched rows were updated. The alignment gate was not run and cross-source acceptance is not claimed.

On 2026-11-10, a 60-frame, 60-tick `--bench --bench-player-cam` run at 1280×720 on the RTX 3060 with `bf2_spa6` one-flag measured frame mean 1.13 ms for the staged build versus 0.98 ms for the prior packed binary. These short sequential whole-frame samples are noisy, not an isolated region-lighting cost. The respective `--sim-hash 100` values matched at `5573f2b603ed95f4`. A packed `--headless --scene-report` loaded the embedded one-flag map, reporting 791 draws, 141 instances and zero untextured draws. No bake inputs changed. This does not verify the missing streak or plume and does not establish native visual acceptance.

### SpaceDust and takeoff plume implementation (2026-11-10)

[O] Source under test: `26a12a0` plus this space-effect change. The six local PS2 space LVLs share the byte-identical `SpaceDust` scope cited above. `bake/src/map/space_dust.rs` writes its texture and 12 named properties to each map's `space_dust.ron`; `game/src/fx/space_dust.rs` loads the same file through `pak`, so a packed build reads the baked scope and a map without one stays unchanged. The scope enables `spacedust1` with values 150, 70, 20, 0.02, 0.1, 0.5, 35, 100, 2, 12, 35 and 170.

The port's mapping follows the routines already cited. `FUN_00460928` (BF2 decompilation lines 511061–511367) places new samples on the plane across the flight direction, 150 m ahead, spread over ±70 m, with a 20 m central hole; it adds at most 20 samples per update into the 80-entry bank, drifts the bank backward at 100 m/s along the view, and drops samples behind the camera. The update interval is `0.02 * 35 / speed`, so the sample rate rises with the flyer's speed. Sample size is 0.1–0.5. The bank keeps the native 30000 s lifetime; it is presentation-only and uses an independent `Lcg(42)`, never the gameplay sequence. `Fx::advance_space_dust` runs it only while the player's mount is a flyer, and pauses (clearing the bank and the fractional accumulator) when the player leaves the craft or stops.

`FUN_00461080` (511372–511520) draws each live sample as an additive capped ribbon from `spacedust1`, with the tail along the view's backward axis. The ribbon length interpolates from 2 to 12 m over speed 35–170 m/s. Alpha is `1 + 254 * clamp((depth - 50) / 30, 0, 1)`, so a sample fades in across the near 30 m. The port draws at most 80 ribbons and only when the map's effect library supplies the texture.

The takeoff plume comes from `FUN_00201c68` (BF2 decompilation lines 114974–115041). It casts a 1024-unit downward ray from one metre above the craft, ignores the craft itself, uses mask `0x90` (BF2 terrain and building), looks up the effect hash `0xd63ee91a` (`flyerspray`), and starts that effect at the ground hit. `game/src/fx/flyer_spray.rs` triggers once when a flyer's phase changes from `Landed` to `Takeoff`, casts the same ray through the port's ordnance collision layer plus `terrain_ray`, keeps the nearest hit, and spawns the baked `FlyerSpray` effect there.

The maintainer observed a landing plume in PCSX2. BF2 `FUN_00202d08` (117241–117274) calls the same `FUN_00201c68` as takeoff when the descending craft's clearance crosses from above to at or below its landed height plus its collision-sphere radius. The port now triggers the BF2 effect at that crossing, using the sphere-center downward ray to find clearance and the existing effect ray to place the plume. BF1 retains the previous provisional touchdown trigger; trace the BF1 path. A matched PCSX2 capture is still needed to compare the plume's appearance and timing.

At revision 1a7c75da plus this change, `cargo test --manifest-path engine/Cargo.toml -p game --lib flyer_spray` passed two tests, including a threshold crossing before touchdown and no second plume on landing. This does not establish visual parity.

The local BF2 tri-fighter cannon inherits a secondary tracer and authors a continuous chaingun firing sound. The existing walker tracer and firing-loop paths now accept flyer weapons too. The playtest snapshot `dist/openbf_data/playtest/last.json` was recorded from binary 26a12a0aa82e plus edits; it records the tri-fighter missile sound three times per salvo. The maintainer subsequently reported that the cannon effect and sound seem to work. The landing plume still needs an original-game comparison.

Flyer pilot weapons now use their individual authored aimer or fire-point positions. The tri-fighter has three missile positions and the A-wing has two positions for each weapon. PROVISIONAL: the port cycles through those points in authored order for consecutive projectiles; trace the original's aimer selection and compare firing order in PCSX2. These positions are bind-pose positions until flyer animation is implemented.

At c52ec491a35e plus these edits, `cargo test --manifest-path engine/Cargo.toml -p core --lib flyer_salvo_uses_each_authored_muzzle` passed one test, including the muzzle-selection state hash. The `every_space_secondary_loadout_fires_and_hits` integration test passed with baked tri-fighter and A-wing point checks. These checks do not establish original-game firing order or missile trail smoothness.

BF2 `FUN_0020fd58` (lines 124203–124222, 124332–124340 and 124360–124371) reads up to eight exhaust effect names, attachment names and offsets. `FUN_00201da0` (115075–115185) creates each configured effect and attaches it using the authored node and offset. The port now loads those entries for flyers and keeps their effects on the rendered vehicle transform. PROVISIONAL: exhaust is active from takeoff through landing and uses bind-pose attachment nodes; trace the original's complete start/stop transitions and compare animated attachments and appearance in PCSX2.

At revision 621263ea plus the exhaust edits, `cargo test --manifest-path engine/Cargo.toml -p game --lib flyer_exhaust_follows_the_craft_and_stops_on_landing` passed one test. The `every_space_secondary_loadout_fires_and_hits` integration test also passed, checking the three tri-fighter exhaust entries against the local baked classes. Neither test is a rendered comparison with the original game.

The maintainer rejected the first shield-edge change: its origin-based touchdown veto made a ship take off again and did not match the original. That change was removed. The follow-up reads BF2 `FUN_00371148` (356790–356837), which calculates the flyer's region overlap from its collision sphere. In `FUN_00202d08` (117193–117218), the flying-state landing request is refused when registered landing regions exist and that overlap is at most 0.99. The same update blocks seat weapons whenever the overlap is nonzero (118290–118318). `FUN_005adc48` and its box helper `FUN_005ad8e0` (732152–732215, 732281 onward) show that greater than 0.99 requires the sphere to be inside the box by more than 0.98 of its radius. The port uses the baked class sphere for both decisions. It does not add a new touchdown-time veto. The exact ship sphere selection and behavior across overlapping regions remain to be compared against the original. Snapshot `dist/openbf_data/playtest/last.json` recorded the rejected build on `bf2_spa8`, with the player's TIE changing from landing to takeoff near (-600, 9, 1387). An original-game controlled comparison is still required.

At 144bbe8d plus the revised change, `cargo test --manifest-path engine/Cargo.toml -p core --lib landing_region` passed three tests; the individual `flyer_region_overlap_distinguishes_firing_from_landing` and `landing_requires_more_than_99_percent_sphere_overlap` tests passed as well. These exercise the two thresholds and ground-map fallback, not a matched PCSX2 flight.

At source revision c52ec491a35e with these edits, `cargo test --manifest-path engine/Cargo.toml -p game flyer_spray --lib` passed one test, and `cargo test --manifest-path engine/Cargo.toml -p game --test space_secondary_weapons every_space_secondary_loadout_fires_and_hits` passed one test. Neither check establishes the cannon's visual or audio parity, landing plume timing, or projectile origin placement.

Validation of the staged change: `cargo clippy --workspace -- -D warnings`, `cargo fmt --all -- --check`, `cargo test -p bake --lib` (366 passed, 12 ignored), `cargo test -p game --lib` (905 passed, 28 ignored), `OPENBF_REQUIRE_SPACE_ASSETS=1 cargo test -p game --test space_playtest_fixes` (9/9), and the full 12-map/86-mount `space_secondary_weapons` matrix passed. The serial `scripts/gpu_tests.sh --render` suite passed 84/84. A 60-frame, 60-tick `--bench --bench-player-cam` run on `bf2_spa6` one-flag at 1280×720 with the rebuilt release binary measured frame mean 0.90 ms and sim tick mean 0.073 ms; that bench stays in the hangar and therefore prices the map frame, not the dust ribbons. `--sim-hash 100` remained `5573f2b603ed95f4`. The regenerated pack's `--scene-report` loaded the embedded `bf2_spa6` one-flag scene (791 draws, 0 untextured), and a one-tick headless walk replay loaded the map's effects from the archive. A matched original/port cruise capture and a count of visible ribbons are still unchecked, so this records the native law and asset path, not visual parity.

### Correction after playtest

The previous shadow-region-only change did not address the reported `bf2_spa6` hangar: that map's baked manifest has no shadow regions. Its carrier classes supply local lights, and vehicle draws did not set `DrawCmd::lit`, so the existing `FUN_00597e98`-based per-model local-light gather never ran for them. Vehicle draws now provide the model sphere to both the local-light gather and the shadow-region gather. The headless serial `scripts/gpu_tests.sh --game-lib vehicle_draws_gather_local_lights_from_the_model_sphere` test renders the actual vehicle submission path with a static red point light and confirms that it affects the draw. This controlled test is not a matched hangar capture.

## 25. Issue 123 investigation checkpoint (2026-11-11)

At source `2cb064d4dcaa`, the latest exit snapshot `dist/openbf_data/playtest/last.json` is an Assault round on `bf2_spa8`, build `2cb064d4dcaa`, team 2, tick 5703. It contains no framebuffer evidence for the reported health-bar color or shield effect. `cargo run -p game --example blackbox_read -- ../dist/openbf_data/playtest/last.json` read it from `engine/`.

A managed PCSX2 session with disc serial SLUS-21240 and CRC 02F4B541 loaded `.10` and advanced 60 frames. Its screen shows an Assault 0/180 HUD and a green ridden-vehicle bar. This supports the earlier full-health measurement in section 16, but is not a matched port capture. The port feeds vehicle health through `game/src/main/battle.rs` and `ui/src/hud.rs`; section 16 records the previous measured RGB comparison. Loading `.09` and advancing 60 frames shows flag indicators and a minimap; it is not an Assault baseline. Both state loads are recorded as interventions in Tracking run `01M7590FJQGFVM9KM88XP67VHK`; no pose or health writes were made. Private screenshots and the pre-edit checklist are under `engine/artifacts/lead_tmp/issue-123/`. This adapter advertised digital buttons but no analog axes; its debug catalog did not expose `record_window`. Neither screenshot is a native gameplay sequence.

`cargo test -p game --test space_assault_scenarios` passed 5/5, including synthetic shield routing, incoming-warning lifecycle and linked-system restore/spent scoring. These tests do not prove original-game behavior. No demonstrated discrepancy was corrected in this checkpoint.

The six Assault mission/side pairs requiring played captures are `bf2_spa1` (both sides), `bf2_spa3` (both), `bf2_spa6` (both), `bf2_spa7` (both), `bf2_spa8` (both) and `bf2_spa9` (both). Shared simulation and HUD wiring are exercised by the synthetic scenario test; map-specific native and port captures for those twelve cases remain unchecked. Also unchecked: matched shield-up/down paths and struck-body classification; fresh-spawn briefing and cockpit/chase chatter with commander interruption; missile warning onset, sound, cadence and cancellation through impact, loss and exit; native repair and repeated scoring; matched current-port health color; and a normal played Assault flow. Alignment across sources was not run. Do not treat this checkpoint as issue acceptance.

Follow-up at `0681df8b`: `FUN_0031ed70` (current B2 lines 300125–300287) and `FUN_0031f2e8` (300527–300577) update a targeted vehicle's occupants' warning fields and nearest missile distance. `FUN_0031f178` (300290–300340) clears the warning on missile destruction when its value exceeds 0.2. The HUD caller `FUN_003b61e0` passes the local controllable to `FUN_003b6050` (399203–399251, 399480–399485), which enables the warning for a positive value and sends the distance as an integer. This connects the writer and reader previously left open in section 14; it does not establish the audio path, timing or multi-missile behavior.

Tracking run `01M7597VMFH1QBHEVKFB4HHDEJ` loaded `.10`, held R1 for 120 frames and retained twelve screenshots at ten-frame intervals in `engine/artifacts/lead_tmp/issue-123/`. A later R1 attempt was interrupted after five frames at the general impact handler `FUN_00313c40`; the hit was not attributed to the player or classified. No shield comparison is claimed. No position or health was written. The adapter's digital-only input and lack of the recording operation limit this setup, but do not establish that those samples cannot be obtained by another controlled route.

A second watch used the local controllable identified at the live `FUN_003b6050` call, not an address reused from another state. A frozen-boundary read established its warning fields at zero. In 1,540 frames the first watched write was the controllable reset at `FUN_002f4a50` (`0x002f4bc0`), with no preceding warning write. Tracking run `01M759TN6R3AZ9H05HDNB10828` records that bounded negative result and the state-load intervention; the 20-second screen is private. It is not a locked-missile sample and cannot establish warning timing or sound.

Follow-up checks from `engine/`: `cargo test -p core` passed 2,170 unit tests (2 ignored), 5 battle-outcome tests and 10 drowning tests. `cargo clippy --workspace -- -D warnings`, `cargo clippy -p core --tests -- -D warnings`, `cargo clippy -p game --test space_assault_scenarios -- -D warnings`, and `cargo fmt --all -- --check` passed. With `OPENBF_REQUIRE_SPACE_ASSETS=1`, `cargo test -p game --test space_assault_scenarios --test space_playtest_fixes --test space_secondary_weapons` passed 5, 9 and 1 tests respectively. The secondary matrix covers its 12 map/modes, 86 mounts and 16 pilot secondaries; it is not guided-camera acceptance. The added scenarios exercise warning cleanup on exit and actual vehicle-destruction death, and partial/full mainframe repair changing a linked turret's team. They verify port transitions, not native played-flow acceptance. No production behavior changed; no GPU comparison or alignment gate was run.

Playtest build: `cargo build --release -p game` built source `8460771b`; no assets needed rebaking for these test and record changes. From `engine/`, `target/release/game.exe --write-pack ../dist/openbf-issue-123.exe --pak ../dist/openbf-issue-123.tmp.pak assets_baked=assets_baked missions=missions` packed 28,729 files into one executable. From `dist/`, the staged executable's `--map assets_baked/bf2_spa8 --mode ass --headless --scene-report` loaded the embedded map and reported 790 draws, 178 instances and no untextured draws. The staged executable replaced `dist/openbf.exe`; both SHA-256 values were `850f5d0998a1ea7197ea4e0c8e3032272d653145b52c8bc7ed1b7b691a9aa25b`. `dist/openbf_data` file paths, sizes and modification times matched before and after. This smoke test is not a visual comparison or played-flow acceptance.

## 26. The shield-depleting impact (2026-11-11, #123)

`FUN_00313c40` applies direct damage through the struck owner's health dispatch (current B2 lines 292300–292333, native call at `0x00313e28`), then chooses the effect class from the struck body and the owner's resulting shield (292356–292398, `0x00313ef0–0x00313f4c`). A shielded owner with shield remaining selects `0x1000`; otherwise the handler uses the actual body's layer. The size choice for `0x1000` is still the small/large rule in section 20.

PCSX2 Tracking run `01M75A84WX1FMSS5923R2VRFK2` captured a pending ordinary shot on an Assault capital-ship body whose underlying class was `0x10`. At the native damage-call breakpoint its shield was 200000. Replaying that same pending shot unchanged produced 199950 shield and effect class `0x1000`. For the other run, a logged memory intervention changed that shield to 1.0 before the native damage call; the hit reduced shield to zero and the handler selected `0x10`. Both used the same saved instruction boundary, struck body and camera. A separate post-damage zero write confirmed the classifier branch, but is not the depletion experiment. Four-frame screenshots from the two pre-damage runs are identical because this impact was outside the camera; they do not establish effect placement, orientation, size or brightness. State and screenshots remain private under `engine/artifacts/lead_tmp/issue-123/`.

The port selected the surface before calling `damage_station`, so the shield-depleting hit incorrectly emitted a shield effect. `core/src/sim/combat.rs` now preserves the struck body's radius before damage and chooses the impact surface after damage. `station_impact_uses_shield_remaining_after_the_hit` reproduces the old error with one shield point and checks a surviving shield and an already down shield. This is a bounded dispatch correction, not visual acceptance or a complete native Assault flow.

## 27. The silent missile warning and the seated health bar (2026-11-11, #123)

| Check | PS2 evidence | OpenBF evidence | Boundary |
|---|---|---|---|
| Shield-depleting impact surface | Same pending hit at native `FUN_00313f4c`: class `0x1000` with 200000 shield, then class `0x10` after the logged 1-point intervention depleted it; dispatch comparison `01M75A84WX1FMSS5923R2VRFK2` [M]. New paired breakpoint samples in `01M75TMRWMVQ7YCSCCB6FTHYEK` [M] record the same hit point `(-195.1306, 188.2851, 1950.3097)` game units and normal `(0.093757, 0, -0.995595)` in both cases; handler offsets effect origin 0.5 along the normal to `(-195.0837, 188.2851, 1949.8119)`. In a same-frame saved-state screenshot pair in `01M76CZ7N26MRSRCC4D2YQP1K5` [M], 123 pixels differ by more than 40 RGB levels within x=273..298, y=184..212 (26x29 pixels), maximum channel delta 149. The localized flash is partly obscured below the cruiser hull. | Surface selection now reads shield remaining after damage; focused combat test covers active and depleted shields | Native position, orientation and a visible hit-associated change are measured. Particle silhouette, scale/brightness and matched OpenBF appearance remain unmeasured |
| Missile warning presentation | Controlled PINE retarget plus matching generation to player flyer: warning 2.0 by +2 video frames; the distance field refreshed every 2 frames (0.033 s). Native HUD showed faint `INCOMING MISSILE!` at 20 frames and clear text at 30–60 frames. The missile destructor cleared warning to 0 and distance to FLT_MAX at 5.03 s; HUD text was gone 1 s later [M]. `1playerhud` authors pulse 0.7, fade-in 0.5 s, fade-out 1.0 s; `FUN_003b6050` supplies integer distance each positive-warning frame | Port warning lifecycle tests assert text/distance updates and no sound | Controlled retarget, not a natural enemy launch; HUD number was about 17–20 m below the sampled distance field; multi-missile selection remains unmeasured |
| Warning audio | Reader calls no sound function; the only missile lifecycle sound is unauthored `LockedOnSound` | Warning path emits no beep | The PCSX2 adapter rejects `sound:true`; default output is muted, so the warning-tone absence is a static trace, not a measured audio sample |
| Repair and repeated scoring | Static ODF/script trace: `com_weap_inf_fusioncutter` authors `BuildingHealth 100` per shot and `ShotDelay 0.5 s`; critical systems receive `SPENT_MAX_HEALTH` on death, while linked shield generators/mainframes can restore. Pilot repair is `0.005 × max health / s` on eligible vehicles, not buildings. A controlled `.03` spawn menu lists Imperial Pilot with Fusion Cutter [M] | Scenario tests cover linked-system restore and spent-critical scoring transitions | No native tool-hit or score sample. This setup's status advertises digital buttons only, with no analog movement/aim axes; the pilot could be spawned but the system repair cycle was not reachable through advertised controls |
| Spawn briefing and NPC chatter | First-spawn UI capture from state `.03` shows no subtitle or portrait in `ps2-spawn-audio-2026-11-11/02-after-character-confirm.png` [M]; this state is after map load. Static Lua search finds no `OSMP_obj_01/02` script reference. | The port no longer queues the unsupported objective lines; cockpit chatter is requested only while piloting | Voice playback and commander interruption are unmeasured; the adapter audio contract prevents host capture |

The native runs use the US Battlefront II disc, serial `SLUS-21240`, CRC `02F4B541`, PCSX2 build string `PCSX2 fd9d310`, emucap host build `8587968`, and host commit `fd9d310ccbb6b8b62c976da8886a3c8fd3a10ff3`. The executable's PE version fields report `0.0.0.0`, so a release version is unavailable. New captures use the active Tracking run `01M75TMRWMVQ7YCSCCB6FTHYEK` [M]; the shield dispatch comparison remains in `01M75A84WX1FMSS5923R2VRFK2` [M]. The paired vector record is `engine/artifacts/lead_tmp/ps2-shield-impact-world-2026-11-11/shield-impact-pair.json` [M]. The BIOS at `engine/artifacts/cloak_ps2/bios/SCPH-70012_BIOS_V12_USA_200.BIN` and the local adapter are available.

**Shield impact position and visible flash.** At the native handler `FUN_00313f4c`, the same pending hit uses point `(-195.130615, 188.285065, 1950.309692)` game units and unit normal `(0.0937572, 0, -0.9955950)` for active shield and the shield-depleting hit. With 200000 shield, the handler class is `0x1000`; writing 1.0 before the same hit lets native damage reach zero and selects class `0x10`. The handler offsets the effect handoff point by 0.5 along the normal, giving `(-195.083737, 188.285065, 1949.811895)`. The paired samples use the same handler object and saved instruction boundary [M]. A separate visual replay loaded `shield-before-hit.p2s` and `shield-impact.p2s` and captured both at stable PINE frame 7907. The impact image has 123 pixels above 40 RGB delta in the inclusive box x=273..298, y=184..212, with maximum channel delta 149; the changed area is a small flash partly obscured under the cruiser hull [M], recorded in Tracking run `01M76CZ7N26MRSRCC4D2YQP1K5`. This confirms a localized hit-associated image change but does not isolate the full particle silhouette or establish its scale/brightness against OpenBF. Those visual checks remain open.

The 2026-11-11 fresh-spawn capture loaded state `.03`, selected the command post and character, then captured the first playable infantry frame and 20/40-second follow-ups under `engine/artifacts/lead_tmp/ps2-spawn-audio-2026-11-11/` [M]. It shows no subtitle or portrait, but state `.03` is already at the spawn menu after the map has loaded, so it cannot rule out an earlier briefing line. A 45-second host loopback capture contained only zeros: the launch used `OutputMuted=true`. A retry with `sound:true` was rejected with `sound:true is not supported by the selected adapter`. Do not treat the silent WAV as evidence that voice or radio was absent; direct audio measurement is unavailable through this adapter.

A separate controlled load of the same `.03` state reached command-post selection, then the Imperial Pilot menu and a live pilot spawn. The menu lists Fusion Cutter, but the PCSX2 adapter status exposes only digital buttons and no analog axes; this run made no health write and did not fire at a system. The native repair rate and repeat-scoring path remain unmeasured [M] (`ps2-repair-2026-11-11-pilot-spawn/`).

**Incoming-missile warning.** Three paths were checked in the current B2 decompile:

- The writers. The missile's constructor `FUN_0031ed70` (300125–300287) and its homing update `FUN_0031f2e8` (300520–300577, only while the homing factor computed from ordnance +0x11c/+0x120 is positive) set each occupant of the targeted vehicle to warning 2.0 (+0x10c), a timestamp from `FUN_004240f8` (+0x110), the nearest missile's distance (+0x118) and that missile (+0x114). The destructor `FUN_0031f178` (300290–300340) zeroes the warning and sets the distance to FLT_MAX for occupants whose value exceeds 0.2. The controllable's reset `FUN_002f4a50` (270995–271025) clears the same fields. The controlled PINE sample [M] confirmed that missile destruction clears the warning and distance; the behavior when homing stops while the missile remains alive is unmeasured.
- The reader. `FUN_003b6050` (399203–399251) sends `missileLock` (HUD +0x310) when the value turns positive, `missileLockDisable` (+0x314) when it falls to zero, and the integer distance (+0x318) every frame it is positive. It calls no sound function. The authored `player1missilelock` and `player1missilelockdistance` elements in `1playerhud` are red text pulsing at 0.7 with 0.5 s fade-in and 1.0 s fade-out; the HUD tree has no sound node.
- The sounds. The only sound in the missile's lifecycle is the missile's own `LockedOnSound` handle at ordnance +0x12c (hash `0x9371aecd`, setter 300862), started at the missile by `FUN_0043d688` from the constructor when it has a target and stopped by `FUN_0043d8d0` in the destructor. Section 14 found it authored on no BF2 class, and a text search of `extracted/` finds no `LockedOnSound`. The chatter index 5 `MissileIncoming` is passed by no call site (`re-unit-chatter.md` 3.5).

The controlled native sample loaded the existing homing missile state, then wrote its target pointer and generation to the player's flyer. After two video frames the flyer occupant had warning 2.0, the missile pointer, and a distance of 24.67 m. At 60 video frames the warning was still 2.0 and the distance had advanced to 276.58 m; the HUD displayed `INCOMING MISSILE! 260`. The number was about 17–20 m below the sampled field over this window, so the exact display conversion remains open. The warning fields updated every two video frames (one 30 Hz game update). The text was faint at 20 frames, visible at 30–60 frames, and the HUD had faded away one second after the missile destructor cleared the field at 5.03 s. This is a controlled target intervention, not a naturally launched enemy missile. Captures and field samples are registered in Tracking run `01M75TMRWMVQ7YCSCCB6FTHYEK` [M].

The port played the infantry `*_weap_inf_rocket_launcher_tracking` sound at OURS intervals of 4, 8 or 15 ticks by distance; `game/src/event_sounds.rs` no longer does, and `incoming_missile_warning_lifecycle_and_hud_events` checks that 80, 200 and 400 m warnings make no sound. The native reader has no sound call, and the warning HUD has no sound node. A direct audio check is unavailable here: PCSX2 launched with output muted, and the adapter rejected `sound:true`. This is a call-graph trace, not a recorded audio result. Natural enemy-launch timing and multi-missile selection remain unmeasured.

The host capture check [M] found the active PCSX2 configuration has `OutputMuted = true`. The installed FFmpeg 8.1.2 build rejects WASAPI input as an unknown format, DirectShow lists only video inputs, and `Get-PnpDevice -Class AudioEndpoint` returned no rows. The diagnostic is in `engine/artifacts/lead_tmp/issue-123/audio-capture-capabilities-2026-11-11.txt`. This confirms that this host/session has no usable loopback capture route; it does not establish that voice or chatter did not play.

**Natural missile opportunity checks.** The Y-wing save `starfighter-parity/full-lock.p2s` was allowed to run without controller input or RAM writes. The first pass found no warning at screenshots from 0–2 s, 12 s, or each second from 13–20 s. A follow-up filled 2.5–11.5 s at 0.5 s intervals (19 screenshots); no warning was visible in those frames [M]. The manifest, review sheet and source images are under `engine/artifacts/lead_tmp/issue-123/starfighter_natural_warning_gap_2026-11-11/` and are registered in Tracking run `01M76CZ7N26MRSRCC4D2YQP1K5` [M]. This covers 0–20 s at 0.5–1 s screenshot intervals, not continuously; warning onset between frames remains possible. The X-wing save `starfighter-parity/xwing-full-lock.p2s` had no warning at one-second checkpoints from 0–10 s. A second X-wing state was then advanced using ordinary R3 thruster input; target range moved from about 502 m to 246 m and back to 441 m, with no warning visible in the sampled frames [M]. Screenshots and checkpoint details are under `engine/artifacts/lead_tmp/issue-123/`; Tracking run `01M75TMRWMVQ7YCSCCB6FTHYEK` contains the earlier image artifacts and input/state interventions. These bounded negative windows do not prove that native enemy missiles never target the player and do not satisfy the requested positive sample; natural-launch onset remains unmeasured.

**Seated health bar.** The PS2 `.10` frame (`state10.png`, an Assault 0/180 HUD, full health) samples (0,99,2–4) over black at x 40–120, rows 378–384, with an olive bar (93,74,20) at rows 391–393. A throwaway probe rendered the port's 1-player HUD tree headless at 596x447 with `seat_health` full, clear colours black and 50 % grey, and the PS2 texture tier. It drew (0,107,128) over black: the blue is the soldier's bar (0,150,213) at alpha 0.6 (213 x 0.6 = 128) over the green one. `ui::hud::feed` published `player1.health` every frame while alive, and the seated gate sent `player1.healthDisable` only on its falling edge, then forgot the value; the next frame's publish re-enabled the soldier's bar. The feed now publishes the soldier's health only when not seated (heroes unchanged). The same probe then drew (0,100,0) over black and (66,166,66) over grey, with the olive bar at (94,76,18). `a_seat_keeps_the_soldier_bar_hidden_until_the_rider_leaves` covers mounting from foot and starting in a seat, three seated frames and dismounting. Port PNGs are private under `engine/artifacts/lead_tmp/issue-123/`. This is a matched-layout comparison over flat backgrounds, not a played frame.

Still open for #123: a positive unmodified enemy-launched missile sample against the player and exact HUD-distance conversion; matched rendered shield-impact placement/normal against OpenBF, particle scale/brightness and port appearance; confirmed map-start voice playback and cockpit radio priority (direct host audio capture is unavailable through this adapter); native repair/re-destruction scoring; and played Assault captures for both sides of `bf2_spa1`, `bf2_spa3`, `bf2_spa6`, `bf2_spa7`, `bf2_spa8` and `bf2_spa9`.

Checks at `aebf87a`, from `engine/`: `cargo test -p core` passed 2,171 unit tests (2 ignored), 5 battle-outcome and 10 drowning tests; `cargo test -p ui` passed 170. With `OPENBF_REQUIRE_SPACE_ASSETS=1`, `space_assault_scenarios`, `space_playtest_fixes` and `space_secondary_weapons` passed 5, 9 and 1. `cargo clippy --workspace -- -D warnings`, strict Clippy for `core --tests`, `ui --tests` and `game --test space_assault_scenarios`, and `cargo fmt --all -- --check` passed. `scripts/gpu_tests.sh --goldens hud` passed its 2 non-golden tests; the 2 golden comparisons could not run because `engine/goldens/` is absent from this checkout. No alignment gate was run.

Playtest build: `cargo build --release -p game` at `aebf87a`, then `target/release/game.exe --write-pack ../dist/openbf-issue-123.exe --pak ../dist/openbf-issue-123.tmp.pak assets_baked=assets_baked missions=missions` packed 28,729 files. From `dist/`, `--map assets_baked/bf2_spa8 --mode ass --headless --scene-report` loaded the embedded map with 790 draws, 178 instances and no untextured draws. The staged executable replaced `dist/openbf.exe` (SHA-256 `725ed6b8c09fb1e436ed7080f0ab02cbce40c81fbe646f3039453d4d68170066` for both); `dist/openbf_data` paths, sizes and times were unchanged. This is not played-flow acceptance.

### Continuation verification (2026-11-11; source `39914a2` plus the shared working tree)

`cargo check --workspace` passed without warnings. `cargo test -p core -p ui --quiet` passed: 2,179 core unit tests (2 ignored), 5 battle-outcome tests, 10 drowning tests, and 170 UI tests. The full `cargo test -p core -p ui -p game` run passed the task-focused `space_assault_scenarios` (5/5) and `space_destruction_scenarios` (7/7), plus 934 game library tests (28 ignored), but the combined `game/tests/suite` result was 499 passed, 2 failed, 29 ignored. `nav_walls::mygeetos_position_table_is_built_in_under_a_third_of_a_second` missed its 0.333 s budget under the combined run and passed on a serial rerun. An isolated rerun of `cargo test -p game --test suite squad_vehicle_maps::move_out_and_at_ease_reach_real_waiting_pilots_once_per_press -- --exact --nocapture --test-threads=1` reproduced the remaining failure on `bf2_geo1`: at 8 m it logged an `OrderTaken` at tick 19 but `pickup.released` remained `None`. This test touches the separate uncommitted movement/deck work in the shared tree; that work was not changed here. The temporary frigate GPU capture passed through `scripts/gpu_tests.sh --game capture_staged_frigate_death_at_native_comparison_times`; its source and frames are retained under `engine/artifacts/lead_tmp/issue-137/`.

Further native measurements stopped at the adapter boundary. Control status reports the active launch `launch-01m756j1w8bd1r4n4stw1szy4x` and bridge as exited; `launch_plan` now reports the PCSX2 adapter and BIOS unavailable to the server, so no new PCSX2 capture can start in this session. At that point this left #137's camera-matched scale/brightness and multi-class live samples, and #123's audio, repair/re-destruction and matched shield visual measurements unchecked, and neither issue was ready to claim closed. No runtime code changed in that step; `dist/openbf.exe` was not updated (SHA-256 then `B2BF4681180453B736F6C3F8C1B0351D760C581AD2077F3BF362A8ED8CE74BDB`).

**Launcher recheck.** The local pinned `pcsx2-qt.exe`, `emucap-pcsx2-bridge.exe`, and 4 MiB BIOS file are present. The emucap server's `launch_plan` reports `kind=bios-not-configured`, `bios_available=false`, and `ready_to_launch=false`; a managed launch request returned `launched=false` with `adapter binary is unavailable`. The adapter README requires `EMUCAP_PCSX2_BIOS` to be set before the control server starts, and the launch tool has no per-call BIOS input. Recovering native measurement therefore requires restarting/reconfiguring that server with the absolute BIOS path; this turn did not launch an unmanaged emulator.

**Recovery after server reconfiguration (2026-11-11).** A new `launch_plan` resolves the PCSX2 binary and bridge and reports the BIOS at `engine/artifacts/cloak_ps2/bios/SCPH-70012_BIOS_V12_USA_200.BIN` as available, with `launch_blockers=[]` and `local_preconditions_ready=true`. It returns `ready_to_launch=false` with transition `inspect_existing` because a managed generation is already live. Control status confirms bound launch `launch-01m7681pjq4stxfegna8ka84a3`, connected and frozen; `get_rom_info` confirms `SLUS-21240`, CRC `02f4b541`, and SHA-1 `b385edc89d053ca8238b47c6549e78f45eb88ca4`. The run uses emucap build `8587968`, PCSX2 upstream commit/build `fd9d310ccbb6b8b62c976da8886a3c8fd3a10ff3`, and tracking run `01M75TMRWMVQ7YCSCCB6FTHYEK` [M]. A current screenshot at frame 3777 shows the command-post menu (artifact `01M7689NS6PDGJ1PM6J2HPEBS3`, SHA-256 `9ef1eda8ebcd5388ac55729dcbd247ea1a379196d0f500926178db1961aae4eb`) [M]. This confirms the server and PINE screenshot path work again. It does not establish first-spawn audio or gameplay parity: the adapter exposes no analog axes, `sound_contract` is null, and its previous `sound:true` request was rejected. The saved state is post-map-load, so the menu screenshot cannot rule out an earlier briefing line.

**Pre-isolation shared-tree build blocker (2026-11-11; HEAD `95d17f5`).** `cargo check --workspace` passed without warnings, and `cargo test -p game --lib a_whitespace_split_chunk_number_reads_its_leading_float_as_the_ps2_does` passed 1/1. `OPENBF_REQUIRE_SPACE_ASSETS=1 cargo test -p game --test space_destruction_scenarios` did not reach its tests: the current shared guided-flight edit calls missing `Sim::landing_coverage` at `core/src/sim/guided.rs:248`. `cargo build --release -p game` failed before packing because `game/src/render_blend.rs` references missing `GuidedRocket.bank`, while `game/src/sounds.rs` references missing `GuidedClass.boost_speed` and `.speeds`. Those files are outside this Space Assault diff and were left untouched. The exact command results are in `engine/artifacts/lead_tmp/issue-137/build-acceptance-check-2026-11-11.txt` [M]. This was the status before switching to an isolated worktree; the successful build and package follow-up is recorded below.

### Isolated build and visual replay follow-up (2026-11-11)

The `emulator/sstates` directory is absent from this checkout and was not used. The managed Control MCP session ran the pinned PCSX2 binary from `tools/emucap` with the BIOS at `engine/artifacts/cloak_ps2/bios/SCPH-70012_BIOS_V12_USA_200.BIN`; captured states are under `engine/artifacts/lead_tmp/`. The bound live launch is `launch-01m7681pjq4stxfegna8ka84a3`, ROM `SLUS-21240`, CRC `02f4b541`, PCSX2 `fd9d310`, emucap build `8587968` [M].

The native screenshot replay used `ps2-death-fixed/fixed_entry.p2s`, not the spawn-menu state `issue-137/death_entry_f4633.p2s`. It captured frame offsets 0, 60, 240, 450 and 510 (0, 1, 4, 7.5 and 8.5 s) at 596x447. The 1 s image shows bright fire/smoke around the hull; at 4 s several burning sections remain; at 7.5 s a translucent section is still visible; at 8.5 s that section is gone with residual smoke/fire. Screenshots and hashes are in `engine/artifacts/lead_tmp/issue-137/native-visual-2026-11-11/`; the run record and gate are Tracking run `01M76CZ7N26MRSRCC4D2YQP1K5` [M].

OpenBF was replayed headlessly at the same simulation timestamps with three camera offsets. This confirmed the 8 s disappearance timing but did not produce a camera match: the original 60 m view placed the camera inside the wreck geometry, while the farther views culled the authored trail effect. The 150 m and 300 m views show the outside geometry but still differ in framing and particle scale from the TIE-fighter view. Exact visual alignment remains unchecked; these captures are diagnostic and do not establish a scale/brightness mismatch.

To get a task-focused build without overwriting other work, a detached worktree at `95d17f5` received only `engine/crates/game/src/fx/chunks.rs` and `engine/crates/game/src/fx/chunks/tests.rs` from the shared tree. `cargo check --workspace` passed without warnings; `cargo build --release -p game` passed. `OPENBF_REQUIRE_SPACE_ASSETS=1 cargo test -p core -p ui -p game` ran the task scenarios successfully: `space_assault_scenarios` 5/5, `space_destruction_scenarios` 7/7, `space_playtest_fixes` 9/9 and `space_secondary_weapons` 1/1. The full combined run had 508 passed, 1 failed and 33 ignored; the remaining failure was the unrelated sniper-assist count expectation (15 found, 7 expected). The result and output hash are recorded in `engine/artifacts/lead_tmp/issue-137/isolated-pack-acceptance-2026-11-11.txt` [M].

The isolated release executable packed 28,729 files and replaced `dist/openbf.exe` only after packing succeeded. Its SHA-256 is `BDCAC2B6BB3B4B263384F0BF30299368B3FF590351FA267AB589A58EB67E9A6B`; the preserved previous executable backup is `349C419CE70F43C94FC0047785E56FF953E9F447F0969FB8185143400B8726F5`. At pack completion, `dist/openbf_data` matched its before-manifest: 122 visible files, 24,610,089 bytes, delta 0. The subsequent scene-report launch from `dist` was followed by a changed inventory, recorded below. This updates the distribution with the task-focused parity snapshot; unrelated uncommitted guided-flight edits in the shared tree remain outside that package.

**Distribution data after smoke check.** The later inventory had 125 files including hidden files and 17,704,691 bytes. `profile.ron` had a newer modification time and the playtest `last.json`/`prev*.json` chain had rotated. A SHA-256 verified copy of the current tree is at `engine/artifacts/lead_tmp/dist-openbf_data-after-smoke-2026-11-11/`, with its manifest at `engine/artifacts/lead_tmp/dist-openbf_data-after-smoke-2026-11-11.json`; the check record is `engine/artifacts/lead_tmp/issue-137/dist-data-smoke-side-effect-2026-11-11.txt` [M]. The earlier manifest captured sizes and timestamps only, and no exact backup of the older rolling files or profile contents was available. I left the current data tree untouched rather than restore from incomplete metadata and risk discarding newer playtest state. No further `dist/openbf.exe` process was launched.

**Closure decision (2026-11-11).** The maintainer asked the parity PR to close #123 and #137 on merge. #137 closes on the measured native death, the replayed 0/1/4/7.5/8.5 s image sequence and timer tail, plus the port's six-map/22-frigate lifecycle test and the map-prim/Omega checks. The six-map native comparison and a camera-matched scale/brightness comparison were not completed; they are recorded as unmeasured above and are not promoted to evidence by this decision. #123 closes on the traced dispatch and warning paths, the controlled native warning/dispatch samples, the localized shield-impact image change and the focused port tests. A positive natural enemy-launch missile sample and exact HUD-distance conversion, host audio capture, native repair/re-destruction scoring, and the twelve played map/side captures were not completed. Those gaps remain visible in this record and in the issue comments; closing the issues does not claim them as measured.
