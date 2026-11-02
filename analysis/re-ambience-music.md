# OPUS RE (2026-10-03) - [O] UNAUDITED

# Music, ambience, command post / prop sounds and interface sounds (card AMBM)

What starts the battle's music, its ambience streams, the loops of command posts, props and droids, and the 2-D
interface sounds on the PS2 - both games. Tags: [V] read (lines cited; function checked with `re_check.py fn`), [L]
measured (disc, PS2 RAM in the reference save states, SNDC's sound-start logs), [D] derived, [H] hypothesis. Corpora:
B1 `analysis/SWBFRONT.ELF-decomp.c`, B2 `analysis/SWBFRNT2.ELF-decomp.c`. Evidence, tools and pasted checks:
`engine/artifacts/lead_tmp/ambm/` (`claims.md`, `checks/`, `tools/`, `ram/`, `verify/VERDICTS.md`, the two readers'
`AGENT_CP.md` / `AGENT_UI.md`). Three sessions read: the lead (regions, streams, music), a reader for command posts /
props / droids and a reader for the interface; every load-bearing line was re-checked by the lead and then by one
independent verifier (section 10).

## 0. Result

1. **Ambience streams come from the world file** `[O]`: a region whose name is `soundstream <stream property>
   <divisor>` becomes, at load, a looping 3-D sound at the region's centre with MinDistance = w / divisor and
   MaxDistance = MuteDistance = w, w = |SIZE| of the region. All of them are started once, at battle start. No
   mission script and no trigger is involved. `soundstatic` regions work the same way with a sample sound, and no
   disc has one.
2. **Two more sources of fixed emitters** `[O]`: world objects of class `com_snd_amb_static` (`Sound`, `MinDistance`,
   `MaxDistance` per object: Mygeeto's 37 neon signs, Tatooine's sandcrawler) and props whose class has a
   `SoundProperty` (fires, vaporators, control panels) - on BF1 a static emitter made with the prop, on BF2 a handle
   on the prop that starts the first time the prop is drawn.
3. **Music is a small state machine over `Music` records** `[O]`: `SetAmbientMusic(team, x, name, state, 1)` stores
   a threshold (x times the team's reinforcements at the call) and a Music per team and state. Every frame, once a
   human has spawned, the manager plays the listener team's Music of the current state. **The state rule differs per
   game:** BF1 takes the state of team 2 (team 1's only if team 2 is above its first threshold); BF2 takes the less
   advanced of the two teams. One Music plays at a time; a Music is held at least `MinPlaybackTime`, fades out over
   `FadeOutTime`, and may start again only `MinInactiveTime` after it stopped (BF1 Naboo: 90 s for `start`, 30 s
   for `middle` / `end`). A BF1 take plays once; a BF2 take loops until `MaxPlaybackTime`.
4. **Battle end** `[O]`: the `soundfx` bus is faded to 0, every sound is stopped and the listener team's
   `SetVictoryMusic` / `SetDefeatMusic` Music is played.
5. **A command post hums by its owner's faction, not by friend or foe** `[O]`: the class's four `AmbientSound`
   values are `<faction> <sound>`; `all` and `rep` name `com_blg_commandpost_goodie`, `cis` and `imp`
   `com_blg_commandpost_baddie`. The owner faction's sound loops at the post; a neutralised post keeps the previous
   owner's; a post that starts neutral is silent.
6. **Capture sounds** `[O]`: one handle per post. Neutral post: `ChargeSound` while one side captures,
   `DisputeSound` while two sides are inside. Owned post: `DischargeSound` while an enemy is inside. The voice's
   pitch is written every tick: charge `P x capture / T + 1 - P`, discharge `P x (1 - neutral / N) + 1 - P`, P =
   `SoundPitchDev` = 0.4 on the disc (0.6 .. 1.0). `CapturedSound` / `LostSound` are one-shots at the post.
7. **Interface sounds** `[O]`: the spawn screen is the engine's state machine and plays the five `SetSoundEffect`
   slots (`SpawnDisplayUnitChange` -> `shell_select_unit`, `...UnitAccept` and `...SpawnPointAccept` ->
   `shell_menu_enter`, `...SpawnPointChange` -> `shell_select_change`, `...Back` -> `shell_menu_exit`); the pause
   menu is Lua and plays `shell_menu_accept` on opening and on every accept, `shell_select_change` on every cursor
   move, `shell_menu_exit` on back. All 2-D.
8. **Earlier records corrected** (section 7): the census's "hum by friend / foe", its Utapau music sequence and
   "battle ended", and the card's thresholds (0.99 / 0.1 are Naboo's; Utapau's script says 0.8 / 0.2).

## 1. Sound regions

1. **[V] Four region types.** B1 `FUN_00173a98` registers factories named "soundstatic" (16107), "soundstream"
   (16119), "soundspace" (16132) and, through `FUN_00359670`, "soundtrigger" (16144 -> 345547). BF2: the same names
   at 0x60f8f0 .. (`FUN_00174b28`), 0x6386f8.
2. **[V] Region -> entity** `[O]`. The static and stream factories (B1 `FUN_001753b0` 16902-16912, `FUN_001754e8`
   16950-16960) build a default class and call `FUN_0035c2f8(region, string, type, class)` (347673-347750): the
   string behind the type word is read with `" %s %f"` (0x547278); the entity is made at the region's transform;
   entity+0x30 = hash(name); if the float > 0, entity+4 = w / float and flag 1; entity+0xc = w and entity+8 = w,
   flags 6 (w = region word [0x13] = sqrt(sx^2 + sy^2 + sz^2), `re-cp-capture.md` 5). B2 `FUN_00441c40`
   (502442-502524) is the same code.
3. **[V] The entity** (B1 base constructors `FUN_0035bcd0` 347379, `FUN_0035bd70` 347407): +4 MinDistance (1.0),
   +8 MaxDistance (10.0), +0xc MuteDistance (10.0), +0x10 flags (1 min, 2 max, 4 mute), +0x20 position, +0x30 the
   sound's name hash; linked into list 0x647560 (B2 0x71f588). A flag that is clear means "use the sound's own
   distance". Keys (instance `FUN_0035bf08` 347490, class `FUN_0035c200` 347630; B2 `FUN_00441b48` 502399):
   `Sound` 0x0e0d9594 and `SoundStream` 0x2e93ef4c -> the name, `MinDistance` -> +4, `MaxDistance` -> +0xc AND +8.
4. **[V] Start** `[O]`: B1 `FUN_0035bff8` (347528) calls slot +0x38 of every entity of the list; its only caller is
   the battle-start function `FUN_0033e590` (328148), a few lines after `… the game-over
   flag). B2 `FUN_00441948` (502297) <- `FUN_004241b0` (482059, after `DAT_00600b3c = 0` 482054).
5. **[V] Static entity** (vtable 0x5474b8): start `FUN_0035ceb8` (348189): resolve the name and play it on the
   entity's handle at its position. Update `FUN_0035cf10` (348202): the handle update (a looping sound out of range
   is retried every update, `re-weapon-sounds.md` 3.4); with a voice, the flagged distances overwrite the voice's
   mindistance (+0x60), mutedistance (+0x64), maxdistance (+0x68); and whenever the handle is not active the sound
   is played again - so a non-looping sound repeats as fast as its own `PlayInterval` / `PlayProbability` allow.
   Nothing stops it. B2 `FUN_00442498` / `FUN_004424f8` (502819-502864).
6. **[V] Stream entity** (vtable 0x5475f8) `[O]`: start `FUN_0035d7c0` (348527-348577): not when the game is over
   in a mission (it starts regardless in the "shell" state, `DAT_00515ce4` == 0x11e1fc01); the stream property by
   name (`FUN_0045dc50`); the property is copied into the entity's OWN voice instance (entity+0x40,
   `FUN_0045e0c8`), 3-D, position = the entity's, the flagged distances as above; `FUN_0045ea08(instance, 1)` plays
   it. B2 `FUN_00442b68` (503086-503136), instance at entity+0x60.
7. **[V] BF2 restarts a stopped stream entity every update** (`FUN_00442ca0` 503139-503151, component vtable
   0x638b20: voice state 4 and a property -> play again). BF1's stream entity has no update (the constructor in
   use, `FUN_0035d668` 348475, makes no update component); its only per-frame pass is the stream system's gain
   bookkeeping (`FUN_0045eec8` 522657-522668).
8. **[V] The play** (`FUN_0045ea08`, B1 522415-522469): the property's `PlayInterval` / `PlayProbability` gate, a
   take, the OPEN stream of the property's `Stream` name (`FUN_0018e770`, list 0x675fc8) and the take's segment in
   it, pitch and gain draws, a random start offset when property+0x64 == 1, then the instance is put into the open
   stream's group list ordered by its priority value (`FUN_0044b478` -> `FUN_0044c468`). **[H]** how many of a
   group's instances sound at once - presumably one per `OpenAudioStream` of that stream (the scripts open the map
   stream twice: `nab1`, `uta1`); the assignment code was not read.
9. **[L] RAM** (`tools/ram_regions.py`): BF1 NAB1 one stream entity `nab_amb_meadow` at (-15.1, -107.6, 102.7), min
   = max = mute = 420.97, flags 7, playing; world record: sphere, SIZE (420.73, 10, 10), "soundstream nab_amb_meadow
   1.0". BF2 UTA1 three: `uta_amb_wind` 235.70 / 471.40 ("... 2"), `amb_hangar` 145.95 / 189.74 and 126.11 / 163.94
   ("... 1.3") - all three with the playing bit while SNDC's log shows the two `uta1` slots (3 and 4) carrying
   `amb_hangar` and `uta_amb_wind`. Over 151 BF1 + 178 BF2 states: 442 / 998 stream entities, every one flags 7 and
   max == mute; 416 / 967 playing (the rest: 26 / 28 with a property and not playing, 3 BF2 without a property).
10. **[D] What is heard**: by `re-sound-distance.md` the level is 1 inside MinDistance, falls as
    `(R - x) / (x k + R)` (x = d - min, R = max - min, k = 10 x `RollOff`) and is 0 from MuteDistance on. Naboo's
    meadow (divisor 1.0: min = max) is on / off at 420.97 m of the centre - the whole map. Utapau's wind (`RollOff`
    0) falls linearly from 235.7 m to 0 at 471.4 m; the two hangars from 146 / 126 m to 190 / 164 m. The properties:
    `nab_amb_meadow` Gain 0.3, `uta_amb_wind` 0.5, `amb_hangar` 0.6, all Looping 1, Mode3D 1, bus `ambience`, one
    take of 90 - 98 s (`wsnd/prop.py`, `tools/seglen.py`).
11. **[L] Disc** (`tools/region_census.py`): BF1 52 `soundstream`, 0 `soundstatic`, 190 `soundspace`, 6
    `soundtrigger` (all HOT1, `hot_loudspeaker`); BF2 168 / 0 / 297 / 16 (all SPA*). Showcase maps:

    | map | soundstream regions (name divisor: w) | soundspace |
    |---|---|---|
    | BF1 NAB1 | `nab_amb_meadow` 1.0: 420.97 | 5 `stonehead` |
    | BF1 TAT1 | `tat_amb_desertwind` 1.0: 601.54; `tat_amb_canyon` 2.0: 144.97 | 0 |
    | BF1 YAV2 | `yav_amb_rain` 2.0: 575.86 | 16 |
    | BF2 UTA1 | `uta_amb_wind` 2: 471.40; `amb_hangar` 1.3: 189.74, 163.94 | 1 `exhaust` |
    | BF2 MYG1 | `myg_amb_wind` 1.0: 989.81 | 12 |
    | BF2 HOT1 | `hot_amb_wind` 1.0: 865.54; `hot_amb_tunnel` 1.3: 60.77, 1.2: 67.91; `hot_amb_hangar` 1.2: 64.97, 64.13 | 11 |
12. **Not read here**: `soundspace` (the listener's sound space; `re-sound-distance.md` claim 7) and
    `soundtrigger`. **[H]** the trigger's rule from one function the verifier did not see (B1 `FUN_00359448`
    345452): for each trigger region holding the given point, a sound of the region's `TriggerSoundRegion` record
    is played as a one-shot at the region's centre, a stream sound only while its stream slot is idle.

## 2. World objects and props that emit sound

1. **[L] Two classes on these labels** on each disc: `com_snd_amb_static` (BASE `SoundAmbienceStatic`) and
   `com_snd_amb_streaming` (BASE `SoundAmbienceStreaming`), both `MinDistance` 1.0, `MaxDistance` 10.0 (BF1
   COMMON.LVL, BF2 INGAME.LVL). They are the entities of section 1, placed as world objects with instance keys
   `Sound`, `MinDistance`, `MaxDistance` (flags 7, so the object's distances replace the sound's own).
2. **[L] World objects** (`tools/snd_objects.py`): `com_snd_amb_static` BF1 296 in 8 world files (KAS1 66, KAS2 /
   KAS2_H 68, NAB2 / NAB2_H 36, TAT1 / TAT1H 1, YAV1 20), BF2 406 in 9 (DAG1 1, DEA1 19, KAM1 80, KAS2 80, MUS1 116,
   MYG1 37, NAB2 36, TAN1 17, YAV1 20); `com_snd_amb_streaming`: one on each disc (YAV1 `waterfall`: `waterfall1`,
   25 / 200 - the verifier's find). Showcase maps: **MYG1 37 x `Neon_buzz_low`** (23 with 2.0 / 30, 14 with 1.5 /
   30), **TAT1 1 x `sandcrawler`** (50 / 400); NAB1, YAV2, UTA1, HOT1 none.
3. **[V] BF1 prop** `[O]`: the prop class (labels `prop` and `building` share it, B1 15553-15557) embeds a sound
   class at class+0x13c (`FUN_001fb390` 108881); `SoundProperty` 0xf350b4e5 is forwarded to it as `Sound`
   (`FUN_001fb6e8` 109496-109497); the prop instance constructor makes a static entity at the prop's transform when
   that name is set (`FUN_001fa400` 108431-108433 -> `FUN_0035d0f0`). It then lives as 1.5: started at battle
   start, re-played whenever inactive, never stopped. No distance flag is set, so the sound's OWN distances apply
   ([L] RAM: flags 0 on every such entity, e.g. END1 95 x `com_amb_fire`, TAT2 42 x `vaporatorhum`; TAT1,
   `tat1_cp0_cp10_postA_spawn.p2s`: 11 x `vaporatorhum`, 5 x `com_amb_fire`, 1 x `sarlacc_amb`, beside the
   `sandcrawler` object and the two stream regions).
4. **[V] BF2 prop** `[O]`: `FUN_0022ef70` (145523-145669) runs from the prop's RENDER method (145518) and from the
   `animatedprop` update (147342): the sound position moves half way per call to prop matrix x `SoundOffset`
   (0xe6dedb8f, class+0x320); `SoundProperty` (class+0x304) is played on the prop's handle when inactive, else
   updated (145626-145636). So a BF2 prop's loop starts the first time the prop is drawn and is never stopped.
   BF2 adds one-shots `StartMovementSound` / `StopMovementSound` when the sound position starts / stops moving
   (145639-145658), and `SoundWhenMoving` (0xba67f8d2, bit 2 of class+0x330): the loop is then silent until a first
   move, stopped when a move starts and played when it stops - the reverse of its name. Only the two Death Star
   bridge parts author it.
5. **[L] Authors of `SoundProperty` on a prop**: BF1 12 classes (`control_panel_a` / `_b`, `com_amb_fire`,
   `com_amb_fire_large`, `minifall`, `vaporatorhum`, `sarlacc_amb`); BF2 33 read by the prop setter (5 more sit on
   `armedbuilding` / `destructablebuilding` classes, whose setters never see the key).
6. **[V] `BuildingSound`** (0xeba16fe4, `destructablebuilding`): silent on both PS2 discs. BF1's code plays it from
   the draw function (`FUN_001c0710` 67179-67181) and no BF1 class authors it; BF2 authors it in 132 chunks and
   `FUN_001e4168` (94033-94059; instructions 0x1e4198-0x1e41d4) plays only on an already active handle, which
   nothing activates ([L], the verifier's walk: 195 alive instances in the BF2 states, 0 active handles).
7. **[V] An effect's `SoundName`** (0x7a5ff18f): played once on the emitter's own handle the first update the
   emitter is alive, at the emitter's frame with its velocity, then updated - a looping sound follows the emitter
   and is retried until in range; stopped when the emitter is stopped (B1 `FUN_002b26a0` 241937-241954,
   `FUN_002b25c8` 241867; B2 `FUN_00374de0` 371753-371766, `FUN_00374d08` 371682). **[D]** the census's Naboo
   `com_amb_fire` is this path (NAB1 has no static entity in RAM).

## 3. Music

1. **[V] The script calls** `[O]`. `SetAmbientMusic(team, x, name, state[, flag])` (B1 32420; B2 45451): with a
   5th argument that is not 0, x is multiplied by the team's reinforcement count at the call; `FUN_002b16d0`
   (241377) stores (int) of it and the `Music` found by name in entry[team-1][state] of the VoiceOverManager (B1
   manager + 0x98 + team x 0x30 + state x 0x10: Music pointer, a byte, the threshold at +8; B2 `FUN_00373f58`
   371274, manager + 0x90). [L] RAM: Naboo 200 / 198 / 20 (script 1.0 / 0.99 / 0.1), Utapau 150 / 120 / 30 (1.0 /
   0.8 / 0.2). `SetVictoryMusic` / `SetDefeatMusic` (B1 32469 / 32486) fill entries of a second object
   (`DAT_005644a0` + team x 0x10 + 0x260 / 0x268; B2 `DAT_00661340` + 0x2a0 / 0x2a8).
2. **[V] The state, BF1** `[O]` (`FUN_002b1280` 241193-241225, instructions 0x2b1280-0x2b131c): for team index 0
   then 1, with T its reinforcements: T <= thr[2] -> 2; else T <= thr[1] -> 1; else T <= thr[0] -> 0; else no
   change. The value left by the last team that set it is returned - **team 2's state, whatever team 1's is**,
   unless team 2 has more reinforcements than its first threshold. No comparison between the teams exists.
3. **[V] The state, BF2** `[O]` (`FUN_00373ad0` 371082-371120): the same per-team state, and the MINIMUM over the
   teams that have one (371108-371110); 0 when none has.
4. **[L] RAM over every reference state** (`tools/ram_music.py`; `ram/music_all.txt`): with the manager enabled
   exactly one entry is asked in every state; BF1 105 of 105 follow rule 2 (and rule 3: no BF1 state has the teams
   in different states, so BF1's rule stands on the instructions alone); BF2 122 of 122 follow rule 3, one of them
   (`bf2_yav1_cp3_landingzone_post1_spawn.p2s`: 134 and 119 against 150 / 120 / 30 -> `start`) contradicting rule
   2. The ten unprefixed `tat1_*` / `tat2_*` states are not in these counts.
5. **[V] The manager's update** `[O]` (B1 `FUN_002b1320` 241305-241322; B2 `FUN_00373b80` 371201-371219): play
   entry[listener][state]; if the listener team changed since the last update, stop the old listener's entry of
   this state; if state >= 1, stop both teams' entries of state - 1. It runs only while the game is not over and
   the manager is enabled - enabled is 0 until a human-controlled unit spawns (`re-announcer.md` 2.0; [L] 99
   states before the first spawn: no entry asked, no Music playing). Listener = `re-announcer.md` 2.0.
6. **[V] An entry**: play (`FUN_00359b30` 345756) asks the Music when the entry is idle or the Music has stopped
   (state 4) - so a Music that ended is asked again on the next frame; stop (`FUN_00359ba8` 345783) sets the
   Music's target to "stopped". Play is refused only when the game is over AND the game state is "shell".
7. **[V] `Music` records** `[O]`: chunk `mus_` of the SOUND LVL -> `FUN_0035ad68` (17441) -> `FUN_0035ab68`
   (346509-346590): `Name`; `SoundStream` (the stream property it plays); `FadeInTime` 0x5083ecd9; `FadeOutTime`
   0x798dc484; `MinPlaybackTime` 0x6813be83; `MinInactiveTime` 0x35724887 (stored twice: the limit, and the idle
   timer's first value - so the first start is not delayed); `Priority`. BF2 (`FUN_00440788` 501444-501533) adds
   `MaxPlaybackTime` 0x319a9661 and clamps Min / Max against each other.
8. **[V] One Music's update** `[O]` (B1 `FUN_0035a208` 346117-346428; states 1 fading in, 2 playing, 3 fading out,
   4 stopped; BF2 `FUN_0043fde0`):
   - playing: when the stream slot's voice has stopped (the take ended) -> fade-out state, target stopped. BF2
     also stops it when the elapsed time reaches `MaxPlaybackTime` (501076-501078).
   - asked to play while stopped: only when idle time >= `MinInactiveTime` (or the options-screen override,
     below); it takes the slot that has the property's stream open, waits while another Music holds that slot,
     then starts the property on the slot (a take is drawn), fades the gain from 0 to 1 over `FadeInTime`.
   - asked to stop while playing: nothing until elapsed >= `MinPlaybackTime`; then a fade to 0 over `FadeOutTime`;
     when the fade is done the slot is stopped and released and the idle time restarts from 0.
   - A pause path (target 6, states 5 / 6: fade out, keep the play position, resume there) exists (346231-346272);
     what requests it was not read [H].
9. **[V] All Musics** (`FUN_0035b018` 346721-346817 <- `FUN_003560e8` 343492): the asked Musics are kept sorted by
   `Priority` + 0.0001 (0 while `MinInactiveTime` has not passed); only the FIRST may be started, and only while no
   Music is playing; every other asked Music that plays is told to stop. So the ambient music, a capture music and
   the victory music never overlap.
10. **[V] The override** `DAT_0051944c` (ignore `MinInactiveTime`): 0 in the ELF, one writer, reached only from
    `ScriptCB_SetShellMusicInterval`, which only the sound options screen calls ((0) on Enter, (10) on Exit) - in a
    battle the interval holds.
11. **[L] Disc** (`tools/music.py`, `wsnd/prop.py`, `tools/seglen.py`):

    | | FadeIn | FadeOut | MinPlayback | MinInactive | MaxPlayback | Priority | stream property |
    |---|---|---|---|---|---|---|---|
    | BF1 `cis_nab_amb_start` | 1 | 1 | 5 | 90 | - | 0 | 1 take 53.9 s, Looping 0 |
    | BF1 `..._middle` | 1 | 1 | 5 | 30 | - | 0 | 4 takes 26.1 - 31.3 s, CyclePlayback 1 |
    | BF1 `..._end` | 1 | 1 | 5 | 30 | - | 0 | 1 take 29.1 s |
    | BF1 `..._victory` / `_defeat` | 0 | 1 | 5 | 60 | - | 1 | 1 take 8.4 / 4.9 s |
    | BF2 `cis_uta_amb_start` / `_middle` / `_end` | 0 | 1.5 | 20 | 15 | 180 | 1 | 7 / 7 / 4 takes of 109 - 123 s, Looping 1, CyclePlayback 3 |
    | BF2 `..._victory` / `_defeat` | 0 | 5 | 3 | 35 | 360 | 9 | 1 take 8.4 / 4.9 s, Looping 0 |

    All 2-D, bus `ingamemusic` (0x4d9942af), stream `nabcw_music` (BF1, per planet and era) / `cw_music` (BF2,
    GLOBAL.LVL). BF2's three ambient properties share takes (D4A0C155 is in `_start` and `_middle`, 9295185E in
    `_start` and `_end`). Each faction also has a sixth, unnamed Music (BF1 Priority 0.5) that no script names [H].
12. **[L] PS2 timing, BF1 Naboo** (`sndc/bf1_nab1_run2.csv` + the state's RAM): `cis_nab_amb_middle`'s take AF338BD0
    (26.12 s) started at clock 176.61; the next take, B63396D5 (31.29 s), at 233.024 = 176.61 + 26.12 + 30.0 + 0.29;
    the next, 82338421, at 294.586 = 233.024 + 31.29 + 30.0 + 0.27. So: a take, 30.3 s of silence, the next take in
    the list's order. **[D]** BF2: a looping take for up to 180 s, a 1.5 s fade, 15 s of silence, the next.
13. **[V] Battle end** `[O]`: B1 `FUN_002be838` (header 249713) <- `FUN_0033f718` (328746), once, right after the
    game-over flag is set (2 s after a team is out, `re-announcer.md` 2.5): bus `soundfx` (0xeceb308a) faded to 0,
    `FUN_00356000` stops the sounds, and the listener team's entry is played: defeat when its bit is in the mask of
    the teams that are out, else victory (249775-249776). B2 `FUN_00380c30` (379625-379706): the mask is the
    WINNERS' (`MissionVictory` 37078; `MissionDefeat` 37122 writes its complement): bit set -> victory.
14. **[V] The other callers of the Music player**: the command post's capture / lost music (4.6: never authored),
    BF2 `ScriptCB_PlayInGameMusic` (45886) and the hero music (150302-150305) - not read.

## 4. Command post

1. **[V] `AmbientSound`** (0x65a920ba) `[O]`: the setter (B1 `FUN_002a1138` 229524-229533; B2 `FUN_0035a3a0`
   353500-353506) needs two tokens; the first is a faction (`neu` 0, `all` 1, `imp` 2, `rep` 3, `cis` 4, else 5:
   `FUN_002ad1d0` 238349), `" defer"` is appended to the second, the reference goes to class + 0x28 + faction x 8
   (B2 + 0x30).
2. **[V] The hum** `[O]`: on a team change (B1 `FUN_002a0738` 228958-229056; B2 `FUN_00359c08` 352997-353060) the
   handle (cp+0x678; B2 cp+0xb24) is stopped; for a new team >= 1 the OWNER team's faction entry is copied into the
   post's reference and played on the handle at the post. Every tick (B1 `FUN_0029f0d8` 228014-228041; B2
   `FUN_00357440` 351591-351622) the handle is updated and, when it is not active, the reference is played again.
   No pitch or gain is written.
3. **[V]+[L] Neutral posts**: the reference is written only by the constructor and by that copy, so a neutralised
   post plays the previous owner's hum again one tick later, and a post that starts neutral is silent. RAM (the
   verifier's walk, `verify/`; not re-run by the lead): 4 neutral BF1 posts with a set reference hum, 127 with an
   empty one are silent. Utapau log: a CIS listener's own post plays `baddie`, restarting at the instant of each
   capture / loss line.
4. **[L] Disc**: `com_bldg_controlzone` (BF1 COMMON.LVL, BF2 INGAME.LVL): `all` and `rep` -> `..._goodie`, `cis` and
   `imp` -> `..._baddie`; both Looping 1, 3-D, gain 0.5, 6 - 100 m, rolloff 6. Other authors: BF1
   `end_bldg_bunker_controlpanel` (`control_panel_b`), `hoth_bldg_shieldgenerator` (`all hoth_shield_amb` only),
   `nab_bldg_fambaa_generator` / `_shield` (`famba_moan`, a one-shot with `PlayInterval` 25 +- 8, re-asked every
   tick); a faction with no entry is silent.
5. **[V] Capture sounds** `[O]`: one handle (B1 cp+0x684; B2 cp+0xb30), updated every tick.
   - neutral post, exactly one side capturing -> `ChargeSound` 0x4ee58508 (B1 228246-228276, B2 351961-351993);
     two sides inside -> `DisputeSound` 0x12caacaa (228184-228217, 352284-352318); nobody -> stopped; the capture
     stops it.
   - owned post, an enemy side inside (alone or contested) -> `DischargeSound` 0x16e4a90e (228457-228488,
     352625-352656); neutralised -> stopped. `DisputeSound` never plays on an owned post.
   - owned post, enemy gone, progress left: BF1 stops the loop on the next tick; when the owner alone is inside,
     `ChargeSound` is started for one tick with the "saved" event (228651-228690). BF2 keeps the discharge loop
     while the progress holds or decays and starts the one-tick charge when it reaches 0 (352442-352609).
   - pitch, every tick while the handle has a voice (it replaces the start's pitch): charge `P x capture / T +
     (1 - P)` (B1 228278-228284, B2 351995-352001), discharge `P x (1 - neutral / N) + (1 - P)` (228490-228496,
     352658-352664); P = `SoundPitchDev` 0xa33d3401 (class+0x80, B2 +0x98; default 0.1, disc 0.4). Dispute and the
     saved charge get none. Records: Looping 1, gain 0.3, 12 - 75 m, rolloff 4.
6. **[V] `CapturedSound`** 0xf17a02e6 / **`LostSound`** 0x8e39006a: one-shots at the post (3-D, 12 - 75 m, gain 0.5)
   from the line function on event 4 (captured) / event 5 (neutralised, or a destructible post killed): B1
   `FUN_0029edd0` 227873 / 227908, B2 `FUN_003570b8` 351431 / 351467. BF1 skips them when the game is over or the
   VoiceOverManager is not enabled; BF2 only when the game is over. The same function would play a `CaptureMusic`
   0x6b9b4728 / `LostMusic` 0x42cb8c86 of the listener's faction for a credited player - no class on either disc
   authors either key (0 PROP chunks), so it never plays.

## 5. Droids, stations, shield

1. **[V] Recon droid** (`droid`): `AmbientSound` and `Ambient2Sound` 0x9a108b4a on two handles from spawn,
   re-played whenever inactive, stopped by the kill (B1 72794, 74153-74159, 72960; B2 100430, 102220-102230,
   100772). Disc: `remotedroid_engine_lp` (loop) and `remotedroid_radio` (`PlayInterval` 5).
2. **[V] `powerupstation`** (the FX-7): `AmbientSound` (`droid_fx7_idle_chatter`: Looping 0, `PlayProbability`
   0.05, 1 - 20 m) is asked whenever its handle is inactive while the building is intact (B1 235629-235634, B2
   362445-362449); `PowerupSound` 0xa2a034ea is a one-shot per unit served (235669, 362494) and per item pickup
   (234860, 361741). The gonk authors no `AmbientSound`.
3. **[V] `ShieldSound`** 0xd85503ff is a key of the weapon class `shield` (BF1's droideka only: `droid_fx7_hum`):
   a loop while the shield is up with gain x (1 - (1 - shield / max)^2), `ShieldOffSound` 0x5df6c1a4 when it drops
   (B1 `FUN_00293920` 219191-219256). Card DEKA / USND material.
4. **[V] Soldier, item, flag `AmbientSound`**: asked whenever the handle is inactive (B1 116840, 234804; B2 159534,
   361675, 356009); the only author is BF1 `imp_inf_darthvader` (`com_inf_saber_ambient`).

## 6. Interface sounds

1. **[V] `SetSoundEffect(name, sound)`** (B1 32259-32419; B2 45261-45441, with a third `index` argument for the
   hero slots) resolves the sound at the call into a slot picked by the name's hash. The five spawn-screen names
   write both viewports' spawn display: `SpawnDisplayUnitChange` -> SD+0x7c, `...UnitAccept` +0x84,
   `...SpawnPointChange` +0x8c, `...SpawnPointAccept` +0x94, `...Back` +0x9c (B2 +0x5c .. +0x7c). Also
   `ScopeDisplayZoomIn` / `ZoomOut` / `Ambient`, `WeaponUnableSelect`, bird and grass sounds, BF2 `LockOn` and
   three hero names. [L] every mission script that calls it (44 BF1, 102 BF2) sets the five to `shell_select_unit`,
   `shell_menu_enter`, `shell_select_change`, `shell_menu_enter`, `shell_menu_exit`, and the two zoom names to
   `binocularzoomin` / `binocularzoomout`; no script sets any other name.
2. **[V] Spawn screen, BF1** `[O]` (`FUN_002df2b0` 268075; state SD+0x18: 0 team, 2 spawn point, 3 unit, 4 spawn
   asked): CROSS in state 0 -> +0x94; in state 2 -> +0x94 only when a post is selected; in state 3 -> +0x84 (the
   spawn) (268226-268254). TRIANGLE in state 3 or 4 -> +0x9c; nothing in states 0 and 2 (268283, 268288). Unit pick
   (`FUN_002e0180` 268507): left stick X beyond 0.8 (x^2 > 0.64, re-armed under 0.2) or d-pad left / right, not
   while the carousel slides: next / previous class with wrap (left = +1) and +0x7c. Spawn-point pick
   (`FUN_002e03b0` 268594): the nearest own post in the stick's direction, +0x8c only when the selected post
   changed. The team toggle (`FUN_002e0880` 268874) also plays +0x7c. No input while the pause menu is open. A held
   CROSS fires every second update on BF1 (the latch clears on the next update).
3. **[V] Spawn screen, BF2** (`FUN_00398000` 392765): as BF1 with slots +0x74 (accept in states 0 and 2), +0x64
   (class accepted -> state 5), +0x5c (unit change, `FUN_00399dd8` 393708), +0x6c (post changed, `FUN_0039a058`
   393966); a class the game refuses plays the BACK slot +0x7c and stays (393066-393074); TRIANGLE: state 3 -> 2,
   4 -> 5, 5 -> 3 each +0x7c (393172-393183). Side select is a Lua button screen.
4. **[V] Lua menus** play through `ScriptCB_SndPlaySound(name)` (B1 31641, B2 44588), 2-D. Cursor: `FollowButtonLink`
   plays `shell_select_change` whenever it finds a button (BF1 `interface_util.luac` proto 1 pc 49-53; BF2
   `ifelem_button.luac` proto 3 pc 36-40); the engine posts up / down / left / right from the d-pad or the left
   stick beyond 0.5 with auto-repeat, first after 0.65 s, then every 0.15 s (B1 `FUN_0033d530` 327413-327490).
5. **[V] Pause menu** `[O]` (`ifs_pausemenu.luac`, opened by START when not paused): entering plays `acceptSound`
   (`shell_menu_accept`); CROSS plays it first for every button, Resume included; TRIANGLE plays `exitSound`
   (`shell_menu_exit`) and unpauses (BF1 protos 8, 9, 10; BF2 protos 7, 9, 14). The yes / no popup: accept, then
   accept (yes) or `shell_menu_cancel` (no). A screen re-entered after a pop plays its `exitSound`.
6. **[L] The records**: `shell_menu_enter` inherits `shell_menu_accept` (one sample, pitch 1.1) - the census's
   "shell_menu_accept" rows on the spawn screen are `shell_menu_enter`; `shell_select_unit` is a record of every
   battle SOUND LVL (not CORE): 2-D, bus shellfx, pitch 0.3 +- 0.1, gain 0.3 (BF1 NAB) / 0.4 (BF2 UTA). [L] RAM:
   all five slots hold 2-D records in both games' battle states.
7. **Not found**: what starts BF1's `shell_menu_exit` at the suicide confirm (one start in `bf1_nab1_ui.csv`; the
   scripts give two accepts only) - needs a live look.

## 7. Corrections to earlier records

- `analysis/audio-census.md` rows "command post hum loop (friend / foe)": the pick is the OWNER's faction (4.2).
- Same doc, section 1.3 and the BF2 music row: the Utapau run's takes D4A0C155 and 9295185E belong to
  `cis_uta_amb_start` as well as to `_middle` / `_end`, so the log does not show a start -> middle -> end sequence;
  and `cis_off_victory_im` is the low-reinforcement line (`re-announcer.md` T5): command post lines follow it 15 and
  18 s later, the battle had not ended, and no victory music is in the log.
- Card AMBM: "start / middle / end at 1.0 / 0.99 / 0.1" is Naboo's script; Utapau's is 1.0 / 0.8 / 0.2 (per map).
- `analysis/re-cp-capture.md` 4 step 5 and 7: "ambient: VO entry 4 (Info)" is the announcer's `VO_*Info` line; the
  loop is the class `AmbientSound`. The ramp key 0xA33D3401 is `SoundPitchDev`; `DisputeSound` plays on a neutral
  post only.
- `analysis/re-announcer.md` 2.1 "the faction's capture / lost music": the keys exist, no class authors them.
- `analysis/re-weapon-sounds.md` 4.3: the prop sound's off-switch (bit 2) is `SoundWhenMoving`.

## 8. PS2 | port | card

| PS2 | port (main b073094) | card |
|---|---|---|
| ambient music: Music records, per-game state rule, one at a time, min playback / inactive times, takes; victory / defeat | none; the music streams are indexed, not decoded | **AMBM** (build) |
| `soundstream` regions, `com_snd_amb_static` / `_streaming` objects, prop `SoundProperty` loops, effect `SoundName` | none (the ambience OGGs are baked) | **AMBS** (new) |
| post hum by owner faction; charge / discharge / dispute with the pitch ramp; captured / lost one-shots | none | **CPSN** (new) |
| spawn-screen slots, pause-menu sounds | none | **UISN** (new) |
| FX-7 chatter, serve sound, recon droid loops, droideka shield hum | none | notes on USND / PWUP / DEKA |
| `soundspace` (occlusion), `soundtrigger` (BF1 Hoth, BF2 space), `BuildingSound` (silent on the PS2) | none | not carded |

## 9. Checks run

- `re_check.py fn` on every cited line: `checks/fn_b1.txt`, `fn_b2.txt`, `fn_b1_cp.txt`, `fn_b2_cp.txt`,
  `fn_b1_ui.txt`, `fn_b2_ui.txt` (all in the named functions).
- `re_check.py callers`: `FUN_0035bff8` 1, `FUN_0035ad68` 1, `FUN_0035b018` 1, `FUN_00359b30` 4, B2 `FUN_0043f708`
  8, `FUN_002be838` 1, `FUN_0035d0f0` 1 (+ its class vtable word, `scripts/tools/refs.py`), `FUN_0035cd40` 0.
- `re_check.py writers`: `DAT_0051944c` 1, `DAT_00519444` 2 (both 0), `DAT_00515ce4` 1, B2 `DAT_0066133c` 7.
- `re_check.py word`: 0x547278 `" %s %f"`, 0x51944c 0, 0x516ec4 10.0, the vtables 0x5474b8 / 0x5475f8 / 0x638b20 /
  0x638b40; hashes with `foly/h.py`.
- Instructions (`scripts/tools/eedis.py`): `FUN_002b1280` whole; the readers' and the verifier's for the post's
  pitch, `BuildingSound`, the BF1 accept latch.
- Twins opened for every BF2 claim (regions, entities, the state rule, the Music parser / update, battle end, the
  post, the prop, the spawn screen); the two games differ in: the music state rule, `MaxPlaybackTime`, the stream
  entity's restart, the prop sound's owner (static entity / handle on the prop), the post's discharge hold, the
  spawn screen's states.

## 10. Verifier

One Opus verifier, claims and citations only (`claims.md`, 57 claims): **56 SUPPORTED, 1 PARTLY, 0 NOT** (312 tool
uses; `verify/VERDICTS.md`, its scripts `verify/v_*.py`). PARTLY: "no object of class `com_snd_amb_streaming`" -
YAV1 has one on each disc (folded into 2.2). Six citation slips corrected here (a function's first line twice, the
BF1 stream constructor in use, `LostMusic`'s line, the `index` argument being BF2's only). From its closing list,
folded in: the key name `MinInactiveTime`; BF2's looping takes and the Min / Max clamp; `building` being the prop
class and `BuildingSound` the destructable building's; the saved-charge branch; the line function's gates per game;
the BF1 accept latch; the unit pick's direction; RAM counts for neutral posts and silent `BuildingSound`.

## 11. Not settled

- [H] How many stream entities of one opened stream sound at once and which win (1.8); on the showcase maps at most
  two regions of one stream overlap anywhere, the number the scripts open.
- [H] BF1's music rule has no PS2 state that separates it from BF2's (3.4); a live check: hold team 1's
  reinforcements under its `end` threshold with team 2 above `middle` and see `start` keep playing.
- [H] `CyclePlayback` 1 against 3, the Music pause path's requester, the sixth Music per faction, a Music whose
  stream is not open (the playing counter then goes negative, B1 346193-346199, 346291-346293).
- [H] `soundspace`, `soundtrigger`, the options / lobby screens' sounds, BF1's `shell_menu_exit` at the suicide
  confirm, the one-tick charge's audible length, a killed post's voices.

## 12. Reproduction

```
T=engine/artifacts/lead_tmp/ambm/tools
python3 $T/regions.py <world LVL>            # sound regions of one world;  region_census.py bf1|bf2 = all
python3 $T/snd_objects.py bf1|bf2 [--list MAP]   # com_snd_amb_static / _streaming world objects
python3 $T/music.py <SOUND LVL> --names engine/artifacts/lead_tmp/ambm/music_names.txt   # Music records
python3 $T/seglen.py engine/assets_baked/<map> <take id>...                              # take lengths
python3 $T/ram_music.py [--verbose] [state.p2s]    # music table, thresholds, both rules against every state
python3 $T/ram_regions.py [--all] [state.p2s]      # sound entities in RAM
python3 engine/artifacts/lead_tmp/annc/luacalls.py bf1|bf2 <script>   # the script's sound calls
```
