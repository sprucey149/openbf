---
id: WSND
title: Weapon and ordnance sounds beyond fire: explosions and the layered record's delays, per-start pitch and gain variation, weapon change and empty click - and the sound bake's skipped sub-level (BF1 Kashyyyk Islands GCW has no map sounds)
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/mungefs/src/sound_props.rs,engine/crates/mungefs/src/soundbank.rs,engine/crates/bake/src/sounds.rs,engine/crates/manifest/src/lib.rs,engine/crates/game/src/sounds.rs,engine/crates/game/src/event_sounds.rs,engine/crates/game/src/fx.rs,engine/crates/game/src/main/headless.rs,engine/crates/game/src/main/windowed.rs,engine/crates/core/src/impact.rs,engine/crates/core/src/weapon.rs,engine/crates/core/src/sim/state.rs,engine/crates/core/src/sim/ordnance.rs,engine/crates/core/src/sim/step.rs
declares: rebake=sounds
needs:
effort:
branch: wsnd
resume: landed 13f3da9c 2026-10-03
created: 2026-10-02
alpha: yes
---
## Why (the card as filed)
SNDC (`analysis/audio-census.md`): after fire, reload and the bolt's strike, the port plays nothing a weapon or its
ordnance makes. PS2, 105 s (NAB1 / UTA1): explosions 990 / 392 starts, the bolt's flight hum 174 / 107, grenade and
mine bounce 36 / 89, weapon change 31 / 38.

## Evidence
- Explosions: the explosion class's `SoundProperty` (68 classes BF1, 118 BF2) - `com_weap_thermaldetonator_exp`,
  `com_weap_inf_haywire_exp`, the layered `com_weap_inf_ordnance_exp` (low / high layers), `com_weap_obj_*_exp`
  with `_near` / `_distant` / `_frequent` variants, `imp_weap_ord_exp_lg`. The port's `Sim::explode` starts no sound.
- `soundlayered` records are not decoded: the list interleaves layer hashes with floats (delays?) and a recurring
  0x77075624; the bake stores every value as a layer (audio-census.md note 4). PS2 delayed layers: pool 0x751e40.
- `OrdnanceSound` (`com_weap_inf_ord_hum_sm` / `_lg` / `_rocket`): starts with every bolt at the muzzle and follows
  it (the port reads the key and plays nothing). `CollisionOtherSound` (`com_weap_inf_grenade_bounce`,
  `com_weap_mine_land`). `WeaponChangeSound` / `ChangeModeSound` (`*_weap_inf_equip_sm|med|lg`, heard on Square / R2).
  `FireEmptySound`, `ChargeSound` (`rep_weap_inf_arccaster_charge`, chaingun spin), `FireLoopSound`.
- Reload: the PS2 started 12 / 2 reload sounds in 105 s; the port fired 184 / 128 `Reload` events in 120 s. Whether
  the PS2's AI reloads without the sound, or rarely, is open [H].

## Note from SNDT (2026-10-02)
- Distances are now the disc's (weapons 5 - 400 m), so about 20x more one-shots are in range (bf2_uta1, 120 s:
  3,700 fire sounds played against 270). The port has no voice cap or priority; the PS2 has 78 one-shot voices and
  steals by level x priority (B1 `FUN_004599e0` 518990 `FUN_0044e4e8`). If explosions / hums crowd the mix, port that.
- A `soundlayered` record's `layers` list holds non-hash words (0x0, 0x3e4ccccd = 0.2): its per-layer delays / gains
  are parsed as layer names today (`imp_weap_ord_exp_lg` on UTA). BF2's cone term (`analysis/re-sound-distance.md`
  claim 6) is not ported. Referee: `lead_tmp/sndt/referee.py <SOUND_LOG csv> <map dir> [<old map dir>]`.

- Planner 2026-10-02 14:45: value 4 -> 5. The census's loudest silent row a player meets on foot: the PS2 starts 990 (NAB1) / 392 (UTA1) explosion sounds in 105 s and the port none, and GREN (value 5, build) is about to land grenades that would go off in silence. Bound the RE to explosions (the layered record), the bolt's flight hum and the grenade's bounce; weapon change / empty / charge and the reload count are second. SNDT's notes here (the cone term, the 78-voice steal) stand. Undo: `loopctl.py set WSND value=4`.

- Planner 2026-10-02 14:45: `deps: AIMA` is an ORDERING dep, not a technical one. Lane 2 prefers an RE card at equal value; without it lane 2 would leave the builds (GREN, PARR) for this RE the moment UWPN lands, while lane 3 (the research lane) is on AIMA. With it the card opens when AIMA's RE is done and the research lane is free. Undo: `loopctl.py set WSND deps=`.
- AIMA's RE session 2026-10-02 16:25: the ordering dep is cleared - AIMA's RE is done (Docs 2aac586) and the card went on to build, so `deps: AIMA` would now have held this RE behind a build. Undo: `loopctl.py set WSND deps=AIMA`.

- RE session 2026-10-02 (lane 3): also for step 4 - a grenade's water contact plays `CollisionWaterSound` (sticky key
  0x2297E47D) and a foliage contact `CollisionFoliageSound` (0xB8C524F1); `event_sounds` plays `CollisionOtherSound`
  only (`StickyContact { water: false }`). BF2's ordnance has `CollisionWaterSound` / `CollisionShieldSound` of its own
  (doc 5.5). Small; fold in if the event carries the surface.
- The same session's verifier found the kas1gcw skip and the parse-order drop; both are in Do 1-2 with their checks.

## Built and landed (2026-10-03, 13f3da9; report `reports/20261003_0040_card_WSND.md`)
- Layer parse test ok: BF1 1185 entries / 30 late, BF2 1819 / 11 absent (= `census_layers.py`); `com_weap_mine_land`
  one layer. `bf1_kas1/sounds` has 4 banks (`2a396768` new). Runtime delay / deviation / pitch / interval tests ok,
  each mutation-checked.
- Referee (`lead_tmp/wsnd/referee/summary.py`): explosion `SoundProperty` rows 140 (NAB1) / 165 (UTA1) from 0; two
  equip sounds at the player on the first tick, one on his switch tick; a BF1 reload's layer +9 ticks at 30 Hz =
  0.300 s; sim hash equal to main's binary on both replays.
- Left for WHUM (noted there): BF2's active-channel equip sound, grenade water / foliage contact, a take's own
  interval, looping effect sounds, a one-shot voice cap.
