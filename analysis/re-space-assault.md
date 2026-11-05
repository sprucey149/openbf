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
`+0x188` also forces it, and that field has not been read. `FUN_0024ba68` itself refuses nothing, and a gunner's update
(`FUN_001ee668`) has no flight gate. `Sim::pilot_held_in_flight` holds a BF2 pilot in a flyer that is not landed. BF1 was not
read, so its rule is unchanged. AI exits stay as they were: `FUN_002baa20`'s request is spent in `FUN_002bbf00`, which is not
this path. `trace:` the AI callers' `+0x68` test.

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
- **Launching.** `FUN_0033a658`: cos at least 0.9 to the target (`LOCK_FIRE_COS`), line of sight, and the class's dwell
  (class+0x120).
- **HUD.** Registered at `FUN_003b0aa8` lines 395768-395770 and 395922-395952. Panel `FUN_003b2cd0` line 397248: distance each
  frame, and a direction arrow while |dot(dir, up)| exceeds 0.2 (0 or 180 degrees). Reticle `FUN_003b3398` lines 398060-398170.
  Placement `FUN_003abc60` line 392409: a flyer's marker scale runs 1.5 at 200 m to 0.1 at 1700 m (0x5ff98c), the corner sprite
  is clamped to 0.9 to 1.4, 1.2 off a flyer, and pulses with white in line of sight and grey (64,64,64) out of it.

Open, with `trace:`: who writes `player%d.missileLock*` (`FUN_003b6050` line 399203 reads entity +0x10c and +0x118, no writer
found), so there is no incoming-missile warning; `lockOnName` (`FUN_003b2b98` unread); the audio cue for a lock (the
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

- **Route to the human** (`FUN_002fc980` line 277194) and **relocation near the human** (`FUN_002faf90` line 276094): read, helpers
  `FUN_002faef8`, `FUN_00161a98`, `FUN_002fbe60`, `FUN_002fa8b8`, `FUN_002fe150`, `FUN_002fe588` not. Relocation teleports a squad of
  int((rand-.5)x3+4.5) to 85 or 700 m ahead of a human flying a flyer, unseen, after 30 s.
- **Pursuit.** `FUN_002fd930`'s trigger on the PS2 appears inert: flyer+0x59c is written only by `FUN_00209220`, with a chance of 0.
  The port's "any seen enemy" trigger in `flyer_drive.rs::bf2_plan` is OURS and stays, flagged, until the maintainer decides.
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
- No comparison with the PS2 was possible here; none of these numbers is a PS2 measurement. Nor did a person play these rounds.

`--sim-trace ... --space-report <csv>` writes the minute rows; `--no-human` empties the player slots. "Idle" keeps the trace's
player: alive, on team 0, untouched, so it is the original's condition for the simulation but not a person playing.
