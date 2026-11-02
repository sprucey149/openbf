# OPUS RE (2026-10-02) - [O] UNAUDITED

# Weapon and ordnance sounds beyond fire (card WSND)

What starts an explosion's sound, the bolt's flight hum, a grenade's bounce, the weapon-change, empty, charge and
reload sounds on the PS2, and what a `soundlayered` record is. Both games. Tags: [V] read (lines cited, function
checked with `re_check.py fn`), [L] measured (disc, RAM, a PCSX2 sound-start log), [D] derived, [H] hypothesis.
Corpora: B1 `analysis/SWBFRONT.ELF-decomp.c`, B2 `analysis/SWBFRNT2.ELF-decomp.c`. Evidence, tools and the pasted
checks: `engine/artifacts/lead_tmp/wsnd/` (`claims.md`, `checks/`, `VERDICTS.md`).

## 0. Result

1. **`soundlayered`** `[O]`: each entry is `Layer(layer name, sound property, delay [, delay deviation])`. The name is
   a label (`near`, `medium`, `far`, `high`, `high_delay`, `low`, `null`) that only a `soundparameterized` record
   uses; the delay is seconds. The sound is looked up while the record is parsed: a layer whose sound is no
   `soundproperties`, or is defined later in the script than the record, is dropped (BF1's `com_weap_mine_land` loses
   one of its two layers this way). BF2's parser takes two more values (a stop time and its deviation) that no record
   on the disc has.
2. **Playing it** `[O]`: every layer gets the position of the event. A layer with delay exactly 0 starts at once. Any
   other layer waits in a pool of 78 timers and starts at the first sound update at or after
   `delay + deviation x U(-1, 1)` seconds, at the position captured when it was armed. With the pool full, the new
   layer replaces the waiting layer of the lowest priority below its own, or is dropped.
3. **Every start draws its own pitch and gain** `[O]`: pitch + `PitchDev` x U(-1, 1), gain + `GainDev` x U(-1, 1)
   clamped to 0..1 (explosions and the hum: `PitchDev` 0.15). A start is refused by the property's `PlayInterval` /
   `PlayProbability`, by a take's own interval, and when its level by distance is 0.
4. **Explosion** `[O]`: the explosion function plays the class's `SoundProperty` once, at the blast, before anything
   else, with no condition. An explosion's `Effect` can carry a sound of its own (`SoundName`).
5. **Flight hum** `[O]`: `OrdnanceSound` is a looping sound on a handle the ordnance owns. It is asked for when the
   ordnance is made and again on every update until a voice starts (a start out of range is refused, and a looping
   sound is retried); the voice follows the ordnance's position and velocity and is stopped when the ordnance dies.
   Range 2 - 30 m (`_sm`, BF1 `_lg`), BF2 `_lg` 4 - 60 m, rocket 10 - 50 m. Its pitch is multiplied by the doppler
   term `1 - v x f / 344` (v = the rate the distance grows, m/s; f = the listener's doppler factor, 1.0) while the
   closing speed is under 172 m/s.
6. **Grenade contact** `[O]` (`re-grenades.md` 4.2): water -> `CollisionWaterSound`, foliage ->
   `CollisionFoliageSound`, any other contact (each bounce, the stick) -> `CollisionOtherSound`, at the contact.
7. **Weapon** `[O]`: `WeaponChangeSound` is a one-shot at the unit when a weapon is selected - the switch and the
   spawn, for every unit; BF2 also on the change of the active channel (L1), except when leaving an off-hand weapon.
   `ReloadSound` plays for every unit when the reload starts. `FireEmptySound` plays for a human only, on the trigger
   press while the weapon cannot fire. `ChargeSound` loops with a pitch that follows the charge; `FireLoopSound` loops
   while the weapon fires; `OverheatSound` on overheating. `ChangeModeSound` is read by neither game.
8. **The reload count** `[L]`: the PS2's 12 / 2 are starts inside the sound's 50 m; the port's 184 / 128 are every
   reload on the map. Inside 50 m the port had 27 / 1, and per infantry shot inside 50 m 0.046 against the PS2's
   0.055 on Naboo. No rule silences a bot's reload.
9. **A port defect found on the way** `[L]`: the sound bake skips a sub-level whose name hash is four printable
   bytes. One exists on the two discs - BF1 `kas1gcw` (0x2A396768, "hg9*"; 1.56 MB) - so BF1 Kashyyyk Islands in the
   Galactic Civil War has no baked map sounds (`assets_baked/bf1_kas1/sounds` holds 3 of the LVL's 4 sub-levels).

## 1. The `soundlayered` record

1. **[V] Loader.** B1 `FUN_004490a0` (507566; the `snd_` dispatcher) calls `FUN_0045ba78` for kind 0xa54a2138
   (507634-507635); that takes a record from pool 0x6755c0 and parses it with `FUN_0045b7b0` (520295-520297). The
   card's `FUN_00356a50` is not a loader of this record.
2. **[V] An entry.** `FUN_0045b7b0` (520137), key 0x7a640f6 (`layer`): the value count (520162); value 1, a hash, kept
   as the layer's name (520163, node[3] 520179); value 2, a hash, looked up among the sound properties with
   `FUN_0045a798` (520164-520165) - no node when it is not found (520166, 520189), and the lookup runs at parse time,
   so a property defined after the record is not found; value 3 a float, node[1] (520167, 520177); value 4 a float,
   node[2], only when the count is 4, else 0 (520168-520173). Node = {property, delay, deviation, name}. Key
   0x8d39bde6 (`name`) -> record+8 (520191-520194).
3. **[V] BF2** `FUN_0053cf00` 671157 -> `FUN_0054e9f0` 683209 -> `FUN_0054e6f8` (683032): the same, with count
   tests `< 4` (deviation 0), `< 5` (node[3] = FLT_MAX) and `< 6` (node[4] = 0), node = {property, delay, deviation,
   v5, v6, name} (683074-683108). v5 / v6 are read only by the controllable handle `FUN_0054f5f8` (683847: a stop
   time = start + v5 + v6 x U(-1, 1)).
4. **[L] Disc** (`census_layers.py` in the evidence directory, every SOUND LVL + CORE.LVL, records in file order): BF1
   488 records (51 names), 1185 entries - 749 with 3 values, 436 with 4; BF2 649 records (89 names), 1819 entries -
   1057 / 762. No entry has 5 or 6 values, so BF2's stop time is never set. The delay is not 0 on 596 (BF1) and 866
   (BF2) entries (values 0.05 - 13.5 s; 0.3, 0.6, 0.1, 0.5, 0.2 the common ones). What the parser keeps: BF1 1155
   entries; 30 are dropped as defined too late - all of them `com_weap_mine_land`'s second layer (0x9D66E891, 0.3 s),
   in each of the 30 sub-levels that hold the record; BF2 1808 kept, 11 dropped as naming no `soundproperties` of the
   sub-level or CORE (5 flyer records: `rep_fly_gunship_land`, `cis_fly_gunship_land`, `rep_fly_vwing_land`,
   `_take_off`, `rep_fly_arc170_turn_on`), none too late.
5. **[L] RAM** (`wsnd/ram_weap.py --layers`, `wsnd/ram_lay2.py --layers`): the 19 records of BF1 NAB1 and the 22 of
   BF2 UTA1 read as {property, delay, deviation, name} hold the disc's values, e.g. `com_weap_obj_med_exp` = near 0 /
   near 0.3 +- 0.1 / medium 0.3 +- 0.1 / far 0.5 +- 0.1 - with the two drops the census predicts for them:
   `com_weap_mine_land` holds one node (0x4E176B01, 0.6 s) on BF1 and `rep_fly_gunship_land` one of two on BF2. No BF2
   node has a stop time.
6. **[L] Names**: `null` 0x77074BA4 (the census's 0x77075624 was a misprint), `near` 0x54F3570F, `medium` 0x35AFD776,
   `far` 0xBCF819EE, `high` 0x3DDDB9FD, `high_delay` 0x07E89EE9, `low` 0x4F516A81. The name is read by
   `soundparameterized` only (`FUN_0045b958`, called from B1 `FUN_00460978` 523889) [V].

Examples (disc, `wsnd/prop.py`): `com_weap_inf_ordnance_exp` = high 0, high_delay 0.1, (0x5FC3D415) 0.2 - all three
`..._exp_high` - and low 0; `com_weap_inf_conc_grenade_exp` = high 0, high_delay 0.1, low 0; `imp_weap_ord_exp_lg` =
near `..._sub` 0, medium 0 (BF2 0.2); `cis_weap_inf_reload_med` = null 0xE6A296B7 0.3; `com_weap_mine_land` = two
layers at 0.6 and 0.3; `com_weap_throw` = `com_weap_throw_fire` 0.3.

## 2. Playing a layered sound

1. **[V] One-shot.** B1 `FUN_0045ca08(instance, record)` (521006): per node, `delay == 0.0` (521035; ELF 0x45ca6c
   `c.eq.s`) -> `FUN_004599e0` now, with the 3-D instance only when the property is 3-D (521036-521040). Else: the
   first free timer of ring 0x6755f0 (521044-521050); with none, the value `FUN_0045bec0(instance, property)` and
   `FUN_0045c060(value)` = the waiting timer with the smallest value below it (520587-520593), dropped when there is
   none (521057-521060); `FUN_0045bc10` arms it (521064). Two callers: the one-shot play and the handle play
   (342712, 343122).
2. **[V] The timer.** `FUN_0045bc10` (520335): the instance is copied into the timer and its position / velocity
   pointers re-pointed at the copies (520357-520410); elapsed = 0; fire time = delay + 2 x deviation x (r - 0.5), r
   uniform in [0, 1) (520415-520419; ELF 0x45bd6c-0x45bd80). `FUN_0045be28(dt, timer)` (520454): when fire time <=
   elapsed, start the property and free the timer; then elapsed += dt (520463-520473). Pool: 0x4e x 0x80 bytes at
   0x751e40 (`FUN_0045c130` 520612-520625). Run once per sound update with dt > 0: `FUN_0045cb58` 521092-521096 <-
   `FUN_00448ed8` 507530 (inside `if (0.0 < dt)`, 507492). The value is what a one-shot start steals a voice by
   (`FUN_0045bec0` -> `FUN_0044e570`, level x the priority term); computing it also draws a take (`FUN_0046b290`,
   520497), so a take is drawn when the timer is armed and again when it fires.
3. **[V] BF2** line for line: `FUN_0054fa38` (683934, 683939, 683958, 683963), `FUN_0054eb30` (683319-683321),
   `FUN_0054ed48` (683365-683375), pool 0x8380a0 (`FUN_0054f050` 683516-683527), update `FUN_0054fb88` 683991-683995
   <- `FUN_0053cce0` 671022 (only when dt > 0).
4. **[V] On a handle** a layered sound takes the same one-shot path and the handle gets no voice (B1 343117-343123,
   343163; B2 499313-499316): it cannot be moved or stopped. A `soundparameterized` (engines) uses a second player
   that holds one voice per layer and starts each when its delay has passed (B1 `FUN_0045c700` 520974-521000, while
   the handle plays and a layer is still to start, the random redrawn every update; B2 `FUN_0054f5f8` draws each
   layer's start and stop once, 683836-683848) - card VSND's.
5. **[L] The PS2's own starts** (`wsnd/layer_times.py` over SNDC's logs): BF1 NAB1, the grenade explosion's second
   `high` layer starts 0.100 s after the first in 11 of 11 events (record: 0.1, deviation 0); BF2 UTA1 0.100, 0.100,
   0.133. `com_weap_obj_med_exp` (NAB1): near again at 0.234 - 0.400 s (0.3 +- 0.1), distant 0.234 - 0.401 (0.3 +-
   0.1), far 0.467 - 0.600 (0.5 +- 0.1); `com_weap_obj_small_exp`: 0.100 - 0.167 (0.1 +- 0.05), 0.167 - 0.233 (0.2 +-
   0.1). The sample of that `high` layer is shared with `com_weap_inf_ordnance_exp`, whose third `high` at 0.2 s
   is in neither log: the events are grenades. A reload's layer starts 0.300 - 0.367 s after the unit's rocket shot
   (delay 0.3 + the state change) for the 7 units within 24 m of the eye, and 0.834 - 0.867 s after it for the 4 at
   29 - 49 m [H: an AI weapon's skipped updates, B1 208374-208382]. BF2 RAM: one armed timer with fire time 0.325
   for a layer of 0.5 +- 0.3.

## 3. The sound reference, its handle, and a start

1. **[V] A reference** is {pointer, kind}: `FUN_00355328` (B1 342823; B2 `FUN_0043d230`) looks the hash up as a
   sound property (kind 1), a layered record (2), a stream property (3). A value `"<name> defer"` (second token
   0x1A01FB3B) is resolved at first use (kind 4, B1 342811-342816) - the baked effects carry
   `"com_weap_obj_med_exp defer"`.
2. **[V] One-shot** `FUN_00354fe0(ref, pos, vel, ..)` (B1 342677; B2 `FUN_0043ceb0`): kind 2 -> section 2; kind 1
   -> `FUN_004599e0`.
3. **[V] Handle** `FUN_00355768(handle, ref, pos, vel, 1)` (B1 343079; B2 `FUN_0043d688`): a held voice is let go;
   the property is started with a callback; the handle's flag is 3 for a looping property (property+0x1c bit 4), 1
   otherwise; its id is the voice's, or 0 when the start gave none (343129-343147). With no position the instance
   points at the zero vector 0x66f150 [L: (0,0,0,0) in 40 states; one writer, 508145].
4. **[V] Handle update** `FUN_00355a28(handle, ref, pos, vel)` (B1 343217; B2 `FUN_0043d948`): active with no voice
   -> a looping sound is played again (every update until it starts), any other is ended (343230-343239); with a
   voice, its position and velocity pointers are set (343244-343252). The callback (`FUN_00355b10`, 343259) ends the
   handle when the voice finished and only clears the id otherwise - so a looping sound that loses its voice starts
   again when it can.
5. **[V] A start** (`FUN_004599e0`, 518898; B2 `FUN_0054cba8`): refused by `FUN_0046bf10(property+0x54)` (the
   `PlayInterval` clock, then a `PlayProbability` draw; 532415-532435) and by the take draw `FUN_0046b290`
   (518936-518944); refused at level 0 (518974-518976, `re-sound-distance.md` claim 7) and when no voice is free or
   can be stolen (519001-519003); it returns the voice only to a caller that passed a callback (519070-519074); then
   `FUN_0045ac00` (519061) sets pitch = `FUN_0044e288(pitch, ..)` = pitch + 2 x (r - 0.5) x `PitchDev` and gain =
   `FUN_0044e348` = gain + 2 x (r - 0.5) x `GainDev`, clamped 0..1 (519742-519746). [L] gated properties on NAB1 CW
   (`wsnd/gates.py`): 13 of 396, among them the explosions' far layers 0x87502C03 / 0xEC44D0B6 (`PlayInterval` 0.5 s)
   and `com_veh_collision_lg` (0.1 s).

## 4. Explosion

1. **[V]** `SoundProperty` 0xf350b4e5 -> explosion class+0x9c (B1 `FUN_0026f528` 193495-193496), BF2 class+200
   (`FUN_0030bb88` 299789). The explosion function - B1 `FUN_002701a0` (193655), B2 `FUN_0030c8f8` (300083), the one
   `re-vehicle-death.md` 2 describes - calls the one-shot play of it at the blast position as its first act (B1
   193760, B2 300192): no test, once per explosion.
2. **[V]** The effect's own sound: when the blast is in no camera's view (a per-camera sphere test, B1 `FUN_0019fc10`
   with 60.0, 193756; BF2 400.0, 300188) and the class has an `Effect`, the effect's `SoundName` reference is played
   instead of the effect (B1 effect+0x21c 193771-193777; B2 effect+0x1ac 300203-300208). Inside the range the spawned
   effect plays it itself [H: not read]. Of the 613 baked effect entries of BF1's common, side and Naboo sets 4 carry
   one (`com_weap_obj_med_exp`, `_large_exp`, `_huge_exp`, `com_amb_fire`).
3. **[V]** The other class with a `SoundProperty` key (B1 `FUN_001fb6e8`, B2 `FUN_0022f6e8`) is the prop class: a
   looping sound on the object's handle (B2 `FUN_0022ef70` 145627-145628) - card AMBM's.

## 5. The ordnance: flight hum, doppler, impact

1. **[V]** `OrdnanceSound` 0xccf9be5c -> class+0x74, `CollisionSound` 0xc28f0c96 -> class+0x7c (B1 `FUN_002738a8`
   195888-195897); BF2 class+0x9c / +0xa4 (`FUN_00310620` 302507, 302513).
2. **[V] Hum.** The constructor plays `OrdnanceSound` on the handle at ordnance+0x80 with no position (B1
   `FUN_002728a8` 195215; B2 `FUN_0030eef8` 301500, handle +0x9c). The update calls the handle update with the
   ordnance's position and velocity on every call (B1 `FUN_00272e98` 195357; B2 `FUN_0030f988` 301815) and stops
   the handle when the life runs out (195362-195370; 301827); the destructor stops it (195262; 301589). With 3.3-3.4
   that is: no voice until the ordnance is inside the sound's range of a listener, then a looping voice on it.
3. **[L]** `com_weap_inf_ord_hum_sm`: looping, 2 - 30 m, mute 30, gain 0.5, `PitchDev` 0.15, `ModeDoppler` 1,
   rolloff 4; BF1 `_lg` the same; BF2 `_lg` 4 - 60 m, pitch 0.8, rolloff 5; `_rocket` 10 - 50 m (BF1 mute 30), gain
   1, rolloff 1.5. PS2 starts: NAB1 147 `_sm` / `_lg` (median 23.0 m, 90 % within 29.4 m) against 97 infantry fire
   starts within 30 m - bolts fired farther away fly into range; UTA1 98 (median 53.3 m).
4. **[V] Doppler.** B1 `FUN_0044cf80` (510467), for a property with `modedoppler` (P+0x18 bit 6): d1 = |(pos - vel) -
   (listener pos - listener vel)|, d0 = |pos - listener pos|; when d1 - d0 < 172.0 the factor is (c - (d0 - d1) x
   listener[+0x18]) / c, c = listener[+0x20] x 344.0; else 1.0 (510480-510520; ELF `lui 0x432c`, `lui 0x43ac`). The
   voice update multiplies the pitch by it (`FUN_0044c8f8` 510176-510183 <- `FUN_004573e8` 517431). Listener +0x18 /
   +0x20 are 1.0 / 1.0 (`re-sound-distance.md` claim 5; BF2 +0x18 = 0.4 in 3 states). [H] the vectors are per
   second (344 is the speed of sound in m/s); no lower clamp was found.
5. **[V] Impact.** B1 `FUN_00272d70` (195286): nothing on layer 0x100 (foliage), else `CollisionSound` at the hit
   (195309-195327). BF2 `FUN_0030f550` (301614): layer 0x200 nothing; layer 0x1000 with a `CollisionShieldSound`
   (class+0xb4) -> that alone; layer 0x100 -> `CollisionWaterSound` (class+0xac); `CollisionSound` only when the
   hit is above the water height of its cell (301771-301784) - where the cell has no water the compared value is an
   unwritten stack word [H: what it does in play].

## 6. Grenade contact

**[V]** B1 `FUN_0027f1e8` 204266-204286 / B2 `FUN_00320fe8` 314008-314029: water (B1 layer 0x80, B2 0x100) ->
`CollisionWaterSound` (sticky class+0xe0 / +0x120) and stop; foliage (0x100 / 0x200) -> `CollisionFoliageSound`
(+0xe8 / +0x128) and nothing else; otherwise `CollisionOtherSound` (+0xf0 / +0x130) at the contact point, before the
stick / bounce code - so once per bounce (at most 4) and once for a stick. `com_weap_inf_grenade_bounce`: 20 - 70 m,
6 takes, pitch 0.7 +- 0.15. The port plays it since GREN (`Event::StickyContact`).

## 7. The weapon's sounds

Slots (B1 `FUN_002864f0` 209793, tests at 210364, 210324, 209969, 210511, 210189, 210003, 210526 / B2 `FUN_00328fb0`
319917): `FireSound` class+0xfc / +0x128, `FireLoopSound` +0x104 / +0x130, `FireEmptySound` +0x10c / +0x138,
`ReloadSound` +0x114 / +0x140, `ChargeSound` +0x11c / +0x148, `ChargeSoundPitch` +0x124 / +0x150, `WeaponChangeSound`
+0x128 / +0x15c, `OverheatSound` (0xf4a517d4) +0x130 / +0x164, BF2 only `ChargedSound` (0xd8468816) +0x154. A weapon
holds three handles (B1 +0xf0 / +0xf4 / +0xf8; B2 +0xd4 / +0xd8 / +0xdc), moved to the owner's position every update
(B1 208425-208433) - an AI's weapon skips updates (B1 208374-208382, B2 318068-318077), the handles with it.

1. **[V] Weapon change.** B1 `FUN_00284148` (208254) = the weapon's "selected": a one-shot of `WeaponChangeSound` at
   the owner, no owner test (208271). Called by the switch for the new weapon when the old one was selected
   (`FUN_00205cc8` 117827-117831; `re-grenades.md` 2.4), by the soldier's set-up for its first weapon (`FUN_0020b2c8`
   120181, `FUN_0020b5e8` 120317), for both channels' weapons when not yet selected (`FUN_0020cbd0` 121397) and by the
   soldier's state reader (`FUN_0032ed90` 317611-317614); 16 weapon vtables hold it and a 17th reaches it through
   `FUN_0028c780`. The same slot again plays the error sound (0x5893f8) for a human only (117816-117820). **[L]**
   NAB1: two equip sounds at one instant at the player when he spawns (his primary's and his secondary's); 25 starts
   in 105 s, 4 of them the player's; the others are units within 50 m [D: spawns and switches]. BF2 `FUN_00325f80`
   (317971): the one-shot only when its third argument is 0 (317987-317992); the cycle passes 0 (161172, 161175; only
   when the old weapon was selected, 161167); the change of the active channel passes 1 when the weapon being left has
   the class flag `OffhandWeapon` or is a melee throw (159913-159917, handed to the new weapon's select at 159946).
   **[L]** UTA1: L1 is followed by equip sounds at +0.07, +0.30 and +0.33 s; BF1's L1 by none.
2. **[V] Reload.** B1 `FUN_00285398` (209043): the reload starts, `ReloadSound` is played on handle +0xf0
   (209055-209058); the owner is tested only afterwards, for the ammunition (209060). B2 `FUN_003274f8`
   318912-318920. **[L]** RAM, 151 BF1 states: 46 AI weapons in the reload state, one with a live voice on that
   handle; the log: other units' reload layers after their rocket shots at 12 - 49 m (section 2.5).
3. **[V] Empty.** `FireEmptySound` on handle +0xf4 only when the owner's player index (owner+0xe8; BF2 +0xc0) is >=
   0, the weapon is selected and the trigger was just pressed, on every route to it: while it reloads, is overheated
   or is in state 8, and in a fire state only more than 1.0 s into it (B1 `FUN_00284318` 208523-208526, 208575-208583,
   208598-208604; B2 `FUN_00326170` states 1-2, 4, 5, 6: 318216-318222, 318285-318294, trigger `(*t & 3) == 3`).
   **[L]** the owner pointer is soldier+0x2a0 on all 10,235 weapons of 151 states, so owner+0xe8 is soldier+0x388,
   the player index (-1 = AI).
4. **[V] Charge.** `ChargeSound` on handle +0xf0 at the charge's start (B1 `FUN_00284f48` 208899-208902; skipped
   for a launcher whose owner is not a local human, 208890-208896), pitch = base x `ChargeSoundPitch` in [0.2, 4.0]
   (208906-208914); each update pitch = ((1 - f) x `ChargeSoundPitch` + f) x base, f = charge / full, limited to
   [0.2, 0.4] (`FUN_00285108` 209016-209027; ELF 0x285304-0x285340) - the 0.4 is what the code holds. BF2
   `FUN_00326f88` / `FUN_00327168`, plus the `ChargedSound` one-shot when the charge is full (318863-318876).
5. **[V] Fire loop, overheat.** `FireSound` on handle +0xf4 (a class with `FireSoundStop`, class+0x13c, stops the
   voice still playing there first, 208765-208770) and `FireLoopSound` on handle +0xf0 at the shot (B1
   `FUN_00284c70` 208774-208779), the loop stopped by `FUN_00284d80`; `OverheatSound` on handle +0xf0 with pitch x
   `OverheatSoundPitch` when the weapon overheats (`FUN_00284a58` 208675-208689) - the census's unnamed loop
   0x0FB06B34 (74 starts) [D]. The ten per-weapon movement keys (class+0x140.., `FUN_00284e50`) have no banked
   sample (`re-foley.md`).
6. **[V] `ChangeModeSound`** (0xc77178c1) is in neither ELF (`constscan.py`: no raw word, no `lui`, no immediate; the
   control hash is found). The sound on Square and R2 is `WeaponChangeSound`.
7. **[V] `FireSound` is the trigger pull's, not each bolt's** (card SNDX, 2026-10-06 `[O]`). The cannon's fire cycle,
   B1 `FUN_00288d10` (211354) / B2 `FUN_0032e8c0` (323477), has two branches inside its time loop:
   - no salvo pending (weapon+0x144 < 1; B2 +0x124): when the refire test passes it stops the loop handle
     (`FUN_00284f28` / `FUN_00326f68`) and calls `FUN_00284da0` / `FUN_00326d78` ONCE (211388-211389; 323540-323541),
     then salvo counter = `SalvoCount` (class+0x22c / +0x290), salvo timer = `InitialSalvoDelay` (+0x238 / +0x2a0).
     `FUN_00284da0` (208794) / `FUN_00326d78` (318566) call the fire sound function with 0 when a session flag is 0
     (B1 `DAT_00518be4`, or `DAT_00518be8` when `DAT_005e9a70` != 0; B2 `DAT_00600548`, or `DAT_00600549` when
     `DAT_0060054f` != 0). The flags are 0 in the ELF images and set to 1 only by the two arms of
     `ScriptCB_SetAmHost` (B1 `FUN_00322f20` / `FUN_00322f48`; B2 `FUN_00400030` / `FUN_00400058`) and by one B2
     destructor (`FUN_004066c0`) [D: a network session; never on the offline disc game].
   - a salvo pending: per salvo *(decompiler detail omitted)* (211398; 323550), then `ShotsPerSalvo`
     (class+0x230 / +0x294) shots; a shot that leaves calls the fire sound function only when
     *(decompiler detail omitted)* (class+0x234, 211421-211422; B2 class+0x298, the LOWER bound of its drawn range,
     323581-323582), and *(decompiler detail omitted)* after every shot that leaves (211425; 323585).

   So: `SalvoDelay` 0 -> one `FireSound` for the whole pull, however many shots and salvos (BF2's shotgun, 8 shots:
   one sound; BF1's shotgun 5, the bowcaster 7, every twin cannon with `SalvoCount` 2 and no delay); `SalvoDelay` > 0
   -> the pull's sound, then one more for every shot but the pull's first. The fire sound function
   (`FUN_00284c70` 208753 / `FUN_00326c38` 318518): `FireSound` (class+0xfc / +0x128) on handle +0xf4 / +0xd8
   (208778, 208780; 318545, 318547) and, argument 0, `FireLoopSound` on +0xf0 / +0xd4. Its other callers (B1
   `FUN_002859f8` with argument 1 and `FUN_00293920`; B2 `FUN_00327d78` with 1, `FUN_00335920`, `FUN_00337b80`,
   `FUN_00348818`) are other weapon classes' or vtable-only and were not read [H: none is on the cannon's path].
   Checks: `re_check.py fn` on 211389 / 211399 / 211421 / 211443 / 208776 / 208806 (B1) and 323543 / 323549 /
   323581 / 323582 / 323605 / 318547 / 318578 (B2), each in the function named; `callers b1 FUN_00284c70` -> 4,
   `callers b2 FUN_00326c38` -> 6; `writers` of the six flags. Verifier (one foreground Opus, 10 claims): 10
   SUPPORTED, with three qualifiers folded in (the pull's call is itself flag-gated; B2's gate reads only the lower
   bound, so a class with `SalvoDelay 0 x` has gaps and still one sound; B2 `DAT_0060054f` starts at 1, so B2 reads
   the pending flag at boot - still 0).
   **Port before SNDX:** one `FireSound` per `Event::Shot`, i.e. per bolt - 8 starts in one tick for a BF2 shotgun.
   [M] One blast at the ear rendered from the baked take (`lead_tmp/sndx`): 1 start peaks at 0.98 x full scale with
   no frame clipped; 8 starts (each its own +-5 % pitch and gain) peak at 5.1 x full scale and 30 % of the blast's
   frames are over full scale - the "static". **Port since SNDX:** `game::event_sounds::fire_sound_starts`.
   [M] The bake is not the cause: 9,160 baked takes of 34 map directories against ffmpeg's `adpcm_psx` decode of the
   disc's bytes - rate and frame count equal on all, 9,035 equal frame for frame, 125 files (6 takes) differ only
   after a frame that saturates (the bake keeps the clamped value as the filter's history, ffmpeg the unclamped one;
   which the SPU2 does was not checked on hardware [H]).

## 8. The reload count [L]

| | PS2 NAB1 | port NAB1 | PS2 UTA1 | port UTA1 |
|---|---|---|---|---|
| reload sound starts / `Reload` events | 12 (run 1: 3) | 184 | 1 | 128 |
| of them within 50 m | 12 (farthest 48.6 m) | 27 | 1 | 1 |
| infantry fire starts within 50 m | 220 (run 1: 166) | 581 | 22 | 273 |
| reloads per shot within 50 m | 0.055 (0.018) | 0.046 | 0.045 | 0.004 |

The reload sound is cut off at 50 m (its layer inherits the foley template: 10 - 50 m, mute 50) and the PS2 refuses a
start at level 0, so its count is the reloads near the player; the port's 184 / 128 counted the map. Near the player
the port's rate per shot is the PS2's on Naboo; its battle put 2.3 times the gunfire per second inside 50 m (581 in
120 s against 220 in 105 s). BF1's reload sound is a layer with delay 0.3 s (the port starts it at once), BF2's a
plain property.

## 9. PS2 | port | difference

| | PS2 | port (main e336f2e) | card |
|---|---|---|---|
| `Layer` entry | name, sound, delay, deviation; dropped when the sound is not yet defined | every value kept as a layer hash; all start at once; 20 (BF1) / 57 (BF2) entries whose name is also a sound start twice; the mine's dropped layer plays | WSND |
| sub-level `kas1gcw` | loaded | not baked (the wrapper's hash reads as an ASCII tag, `mungefs::soundbank::is_sound_wrapper`) | WSND |
| pitch / gain per start | +- `PitchDev` / `GainDev` | the property's pitch and gain | WSND |
| `PlayInterval` / `PlayProbability` on a sample property | gate the start | not read | WSND |
| explosion `SoundProperty` | once per explosion, at the blast | nothing (`Fx::explode`, 3 call sites) | WSND |
| effect `SoundName` | by the effect / the explosion | baked (`"... defer"`), not played | WSND |
| `WeaponChangeSound` | select: spawn + switch, every unit | nothing | WSND |
| `FireEmptySound` | human, trigger press, empty; cuts its handle | both channels, the cutter and a flown droid's blaster: `WeaponState::press_clicks`, `SoundLib::weapon_voices` | WSND |
| `ReloadSound` | every unit; BF1 +0.3 s | at once | WSND |
| grenade contact | three keys | `CollisionOtherSound` (GREN) | - |
| `OrdnanceSound` | looping voice on the bolt, doppler | key read, not played | WHUM |
| `ChargeSound`, `FireLoopSound`, `OverheatSound` | loops on the weapon's handle | nothing | WHUM |

## 10. Checks run

`engine/artifacts/lead_tmp/wsnd/checks/`: `fn_b1.txt` (89 cited B1 lines, each in the function named),
`fn_b2.txt` (77 B2 lines), `callers_b2.txt`. `re_check.py callers b1 FUN_0045ca08` -> 2 (`FUN_00354fe0` 342712,
`FUN_00355768` 343122); `FUN_0045cb58` -> 1 (`FUN_00448ed8` 507530); `FUN_002701a0` -> 17; `FUN_0045ac00` -> 3;
`FUN_0044cf80` -> 2. `re_check.py writers b1 DAT_0066f150` -> 1 (508145, `= 0`). `constscan.py b1|b2 0xc77178c1` ->
`raw 0 | lui 0 | imm16 lo 0`; control `b1 0x352a4db8` -> lui 0x286694. Disassembly (`scripts/tools/eedis.py`):
0x45ca08-0x45cb58 (the delay test and the two paths), 0x45bd30-0x45bde0 (the fire time), 0x44cf80-0x44d098 (172.0,
344.0), 0x2852f0-0x285378 and 0x285014-0x285060 (the charge clamps 0.2 / 0.4 and 0.2 / 4.0). The BF2 twins were
found by a reader agent (`AGENT_B2.md`); every line of it cited here was opened again.

## 11. Verifier

One foreground Opus verifier, the claims and citations only (`claims.md`, 57 claims): 49 SUPPORTED, 8 PARTLY, 0 NOT
SUPPORTED (`VERDICTS.md`). Every constant was re-read in the ELF (0.5 and the factor 2, FLT_MAX, 0.2 / 4.0 / 0.4,
172.0 / 344.0) and every absence survived its own search. Folded in from the PARTLY verdicts and its own findings:
- the disc census skipped the sub-level `kas1gcw` and collapsed duplicates (the numbers in 1.4 are the re-run's, equal
  to the verifier's own walker) - and the port's bake has the same skip (0.9);
- a layer is dropped when its property is defined later than the record (1.2, 1.4): RAM's one-node
  `com_weap_mine_land`, which the author's RAM read had printed and not noticed;
- the empty sound's extra condition in a fire state (7.3); BF2's cycle lines and its "old weapon selected" condition,
  and the off-hand flag being the weapon left (7.1); a fifth caller of the select and a 17th vtable (7.1);
- the reload log: two of the four cited rows are beyond 11 m, not three; and the 0.83 - 0.87 s lag of the far units
  (2.5);
- the pool update's dt > 0 condition, the take drawn at arm time, the ways a start gives no voice (2.2, 3.5), the
  per-camera view test (4.2), `FireSoundStop` (7.5). Reproduced by the author afterwards: the census re-run (488 /
  1185 / 30 late; 649 / 1819 / 11 absent), the one printable sub-level hash of 69 (`wsnd/printable_sublevels.py`), the
  reload lags by distance, BF2's cycle lines.

## 12. Not settled

- Inside the camera range, who plays an effect's `SoundName` (the effect's own update was not read) [H].
- The doppler term's units and whether a pitch near 0 is clamped (the bolt leaving at more than 344 m/s) [H].
- BF2's `CollisionSound` test against an unwritten stack word away from water (5.5): a live count of impact sounds
  per hit would settle it; BF2's 231 `imp_weap_ord_exp` starts against BF1's 88 do not suggest a silent rule.
- The gate before every play (`FUN_00354ee8` / `FUN_0043cd88`: sound on, the shell, network session flags) is
  described by use only [H]; offline it passes.
- Weapon subclasses that override the base functions (beam, launcher, melee) were not read beyond their calls into
  the base.
- The charge pitch's upper limit of 0.4 during the charge is what the code holds in both games; not heard live.
- Why the reload layer of a unit 29 - 49 m away starts 0.5 s later than a near unit's (2.5).
