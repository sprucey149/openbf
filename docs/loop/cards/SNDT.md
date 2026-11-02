---
id: SNDT
title: Sound distances and loudness: bake resolves a property's Inherit template inside its own SOUND LVL only, the templates are in CORE.LVL - every port sound is gain 1.0 and silent beyond 50 m (the disc: weapons 5-400 m gain 0.7, explosions 30-450 m)
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/bake/src/sounds.rs,engine/crates/bake/src/main.rs,engine/crates/mungefs/src/sound_props.rs,engine/crates/game/src/sounds.rs,engine/crates/manifest/src/lib.rs,scripts/rebake.sh
declares: rebake=sounds
needs:
effort:
branch: sndt
resume: landed 3aefd6f9 2026-10-02
created: 2026-10-02
alpha: yes
---
## Why
SNDC's census (`analysis/audio-census.md` section 0): every 3-D sound the port plays is cut off at 50 m and has gain
1.0. `bake sounds` flattens a property's `Inherit` chain only inside its own SOUND LVL (`bake/src/sounds.rs` 82-96,
`by_hash` is one LVL's records) and the templates live in CORE.LVL.

## Evidence
- Baked `rep_weap_inf_rifle_fire` (bf1_nab1 and bf2_uta1 `sounds/sounds.ron`): gain 1.0, 5 - 50 m, rolloff 1.0.
  Disc template 0xB1AD507C, present in `assets_baked/core_bf1/sounds/sounds.ron`: gain 0.7, 5 - 400 m, rolloff 6.0
  (BF2 CORE: 3.5), priority 0.85. Others: explosions 0xBFA71AD3 30 - 450 m rolloff 1; engines 0x964673B0 /
  0x6350B2DC 45 / 25 - 450 m; foley 0xFE30AE3A gain 0.7, 10 - 50 m, rolloff 3; chatter 0xCFFF9324 gain 0.6, 16 - 150 m.
- PS2 (live, 105 s each): 85 % (NAB1) and 94 % (UTA1) of the infantry fire the player hears starts beyond 50 m
  (median 153 m / 77 m). Port, UTA1, 120 s: the player hears 0 of 760 CIS shotgun shots and 11 of 540 Republic
  rifle shots (`lead_tmp/sndc/port/bf2_uta1_sounds.csv`, distance < baked max).
