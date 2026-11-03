---
id: AMBM
title: Music: the ambient music's start / middle / end by reinforcements (BF1: team 2's state; BF2: the less advanced team), Music records (fade, min playback, min inactive time, takes), victory / defeat at battle end, a streaming player for the stereo music streams
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
SNDC (`analysis/audio-census.md`): the port's battle has no music, no ambience and no interface sound. PS2: the
ambient music plays from the first second (`cis_nab_amb_middle`; UTA1 `cis_uta_amb_start` -> `_middle` -> `_end`,
then `cis_off_victory_im`), one or two ambience streams run all battle, every command post hums, and every menu move
clicks.

## Evidence (live, `lead_tmp/sndc/`)
- Music: stream slot 0 (BF1) / 2 (BF2); `SetAmbientMusic(team, fraction, name, state)` start / middle / end at ticket
  fraction 1.0 / 0.99 / 0.1 and the Victory / Defeat music are specified in `analysis/re-announcer.md` 2.5 and section
  1 (the VoiceOverManager owns them). The stereo music streams are indexed and not decoded by the bake (the maintainer's
  decision: music stays streamed) - the runtime needs a streaming decoder for them.
- Ambience streams: BF1 NAB1 slot 3, property 0xEB51145B on stream `nab1`; BF2 UTA1 slots 3 and 4, 0x4FEEB00F and
  0x7C8CA2CF (unnamed `<map>_emt` properties; the bake has their OGGs under `sounds/streams/`). What starts them (a
  sound region, the mission script) is not read [H].
- World loops (sample voices): `com_blg_commandpost_goodie` / `_baddie` (a post's hum by friend / foe, 99 audible
  starts NAB1), `com_blg_commandpost_charge` / `_discharge` on a capture (keys 0xF17A02E6, 0x16E4A90E, 0x8E39006A,
  0x12CAACAA also name `_capture`, `_lost`, `_dispute`), prop emitters (`windmill`, `neon_buzz`, `com_amb_fire`),
  `AmbientSound` of the droids (`droid_fx7_idle_chatter`, `com_item_gonkrecharge`).
- Interface (CORE.LVL bank, 2-D): `shell_select_change` on every pause-menu and spawn-map move, `shell_menu_accept`
  on accept, `shell_menu_exit`, `shell_select_unit` on the class pick.

## Note from SNDT (2026-10-02)
- Sound spaces scale 3-D sounds: when a sound's space is not the listener's, the PS2 multiplies its level by the mean
  of that space's `obstructionhfgain` / `occlusionhfgain` (B1 `FUN_004599e0` 518970-518972, `FUN_0046a2f8`,
  `FUN_0044d698`; the default space holds 0.5 / 0.5 or 0.1 / 0.1 in 38 BF1 states, 0-9 spaces per map).
  Not ported; `analysis/re-sound-distance.md` claim 7. Distances and gains are now the disc's
  (`game::sounds::attenuation`); referee `lead_tmp/sndt/referee.py <SOUND_LOG csv> <map dir>`.

- Note from WSND's RE (2026-10-02, `analysis/re-weapon-sounds.md` 4.3): the prop class's `SoundProperty` (B1
  `FUN_001fb6e8`, B2 `FUN_0022f6e8` -> class+0x304) is a sound on the object's own handle at the object's position,
  started and updated by B2 `FUN_0022ef70` (145627-145628) unless bit 2 of class+0x330; BF2 adds `StartMovementSound`
  (+0x30c) / `StopMovementSound` (+0x314) one-shots. A looping sound on a handle is retried every update until it is
  in range (3.4). Not the explosion class's key of the same name.
