---
id: WHUM
title: The bolt's flight hum and the weapon's looping sounds (charge, fire loop, overheat): looping voices that follow their source, with the PS2's doppler - the PS2 starts 147 hums in 105 s on Naboo, the port none
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The RE of card WSND (`analysis/re-weapon-sounds.md` sections 3, 5, 7; Docs 528fa6c): a bolt's `OrdnanceSound` and a
weapon's `ChargeSound` / `FireLoopSound` / `OverheatSound` are sounds on a HANDLE - a voice that follows its source and
is stopped by it. At filing, the port had one-shots only; VSND subsequently landed `audio::Voice` and
`play_voice` (see the build note below). Reuse that API for the remaining bolt/weapon loop runtime. The historical
PS2 105 s sample has hum starts 147 (NAB1; median 23 m from the eye) and 98 (UTA1); the bolt's whizz past the
player is the loudest thing missing from a firefight after the explosions.

## Evidence (all [O], re-weapon-sounds.md)
- **Hum**: `OrdnanceSound` (`com_weap_inf_ord_hum_sm` / `_lg` / `_rocket`: looping, 2 - 30 m; BF2 `_lg` 4 - 60 m;
  rocket 10 - 50 m; gain 0.5, PitchDev 0.15, ModeDoppler 1) is asked for when the ordnance is made and on every update
  until a voice starts (a start at level 0 is refused; a looping sound is retried), follows the ordnance's position
  and velocity, and is stopped when the ordnance dies (B1 `FUN_002728a8` 195215, `FUN_00272e98` 195357 / 195369,
  `FUN_00355a28`; B2 `FUN_0030eef8`, `FUN_0030f988`).
- **Doppler** (B1 `FUN_0044cf80`): pitch x (1 - v x f / 344), v = the rate the distance to the listener grows (m/s,
  from the source's and the listener's velocities), f = 1.0; no change while the source closes at 172 m/s or more.
- **Weapon loops**: `FireLoopSound` from the shot until the fire ends; `ChargeSound` from the charge's start, pitch =
  base x `ChargeSoundPitch` in [0.2, 4.0] at the start and ((1 - f) x `ChargeSoundPitch` + f) x base limited to
  [0.2, 0.4] during the charge (the code's own cap); BF2 a `ChargedSound` one-shot at full charge; `OverheatSound`
  with `OverheatSoundPitch` on overheating (B1 `FUN_00284c70`, `FUN_00284f48`, `FUN_00285108`, `FUN_00284a58`).

## Do
1. Reuse `audio::Voice` / `Audio::play_voice`: per-frame gain, pan and rate, stop on drop already exist. No new
   backend handle is required. Keep source-following bolt/weapon state separate from the engine layer keys.
2. `game::sounds`: a handle = (property, voice): start when the level at the source is above 0, retry each frame while
   it has no voice, set level / pan / pitch x doppler each frame, stop on request. Pitch + PitchDev at the start.
   An existing engine-loop entry with a missing voice is retained without retry (`sounds.rs::loop_voices`);
   WHUM must keep its explicit retry rule and test a source initially refused before it becomes audible.
3. The hum: one handle per live bolt / rocket inside the property's mute distance of the listener (the bolt's
   position and velocity each tick; dropped on its expiry). The weapon loops on the unit's weapon.

## Acceptance (run and paste)
- **Doppler** (unit test): closing at 100 m/s -> 1.2907; receding at 100 m/s -> 0.7093; closing at 200 m/s -> 1.0;
  mutation-check the 172 m/s test.
- **SOUND_LOG referee** (`lead_tmp/sndc/port/sound_log.patch`, `pad120.ron`, bf1_nab1): hum starts == the bolts that
  come inside the hum's mute distance of the listener while alive (counted from the same run's bolt positions); every
  hum is stopped on its bolt's expiry tick (none left playing after the last bolt); the player's own shots each start
  one (PS2: `com_weap_inf_ord_hum_sm` with every `cis_weap_inf_blaster_fire` at the player, 0.20 s apart).
- **PS2 reference** (`lead_tmp/sndc/bf1_nab1_run2.csv`): 147 `_sm` / `_lg` hum starts per 105 s against 97 infantry fire
  starts within 30 m (ratio 1.5), median start distance 23.0 m, 90 % within 29.4 m - the port's ratio and median
  beside them, with the reason for a difference.
- A charge weapon held by the player (BF1 arc caster): the charge loop starts on the press, its rate inside [0.2, 0.4]
  x base during the charge, and stops on the release (a unit test on the handle's state).
- If hums crowd the mix, port the PS2's voice cap (78 one-shot voices, steal by level x priority: SNDT's note on
  card WSND) rather than inventing one.

- From WSND's build (2026-10-03), left for this card or a small follow-up - none needs a looping voice, all were outside
  WSND's acceptance:
  - BF2's `WeaponChangeSound` on the change of the ACTIVE CHANNEL (re-weapon-sounds.md 7.1: silent when the weapon left
    is off-hand or a melee throw). The port plays the sound on a switch (`Event::WeaponSelect`), a respawn and the
    battle's first tick only; the sim has no BF2 flag and `WeaponClass` no `OffhandWeapon`.
  - A grenade's water / foliage contact (`CollisionWaterSound`, `CollisionFoliageSound`; 6): `event_sounds` plays
    `CollisionOtherSound` for a non-water contact only.
  - A take's own interval (a `Sample` with 3-4 values, doc 3.5) is not baked; `PlayInterval` / `PlayProbability` on the
    property are.
  - The channel-1 weapon (a grenade out of ammunition) gives no `FireEmptySound`; channel 0's does.
  - An effect's `SoundName` is played for effects an EXPLOSION spawns (`Fx::explode`), PROVISIONAL [H] as in the doc's
    12; the looping ones on prop / damage effects (`com_amb_fire`, `com_amb_steam`, `com_amb_spark_c`) need the looping
    voice this card builds. BF1's AAT death blast (`cis_hover_aat_exp`, effect `med_explosion`) names no sound on either
    path: what the PS2 plays for a destroyed vehicle is card VSND's.
  - Note from AMBS (2026-10-16, landed 408a8d3): the world's fixed loops (regions, `com_snd_amb_static`, props with
    `SoundProperty`) now play through `SoundLib::ambient_voices`, a looping voice per emitter. What AMBS did NOT do is
    the effect-borne loop: BF1 NAB1's fires are `com_amb_fire` on an EFFECT's `SoundName` (re-ambience-music 2.7; NAB1
    has no static entity in RAM), and the port spawns no effect on a world prop at all (AMBS found no such spawner in
    `game/src/fx.rs`), so the sound waits for whichever card brings those effects in; the voice to reuse is
    `ambient_voices`' `Loop` case.
  - `SoundLib` still has no voice cap (the PS2: 78 one-shot voices, steal by level x priority); only the waiting-layer
    pool is bounded (78).

- Note from VSND's RE (2026-10-03): card VSND's engine loop needs the same loop handle (gain, rate and pan set every
  frame, `audio::Audio`). Whichever of WHUM / VSND lands first builds it; the other reuses it.

- Note from VSND's build (2026-10-03, landed 79eac42): the loop handle is built - `audio::Voice` (`Audio::play_voice(&SoundData, db, rate, pan)`, `Voice::set(db, rate, pan, secs)`, stops when dropped). `SoundLib::loop_voices` (game/src/sounds.rs) is the engine layers' user: reuse its shape, or give it a second key space.
