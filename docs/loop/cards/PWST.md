---
id: PWST
title: Health and ammo droids (powerupstation): a soldier by one is healed and re-supplied as on the PS2 - RE the station's reach, rate and team rule, then build it
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/sim/stations.rs, engine/crates/core/src/sim/stations/tests.rs, engine/crates/core/src/sim/state.rs, engine/crates/core/src/sim/step.rs, engine/crates/core/src/sim/pickups.rs, engine/crates/game/src/roster.rs, engine/crates/game/src/classdb.rs, engine/crates/game/src/assets.rs, engine/crates/bake/src/turret_rig.rs, engine/crates/manifest/src/lib.rs, engine/crates/game/src/viewer/stations.rs, engine/crates/game/src/viewer/scene.rs, engine/crates/game/src/event_sounds.rs, engine/crates/game/src/fx.rs, engine/CRATES.md, engine/crates/game/src/sounds.rs, engine/crates/game/src/main/windowed.rs
declares:
needs:
effort:
branch: pwst
resume: DONE: slices 1 (sim be94db1), 2 (draw d79666c) and 3 (chatter 3890c34) landed. Left for the planner: the vehicle droid's +1 clip waits for vehicle-gun reserves (the port's seat guns keep none; core/src/sim/stations.rs header); open [H]s: idle arm / socket pose, the 60 m viewport gate; tips -> TIPS, broken stations -> PWSTD. Independent re-powerup-station.md audit input hold: restore the report's 32 paths before retry (reports/20261025_0454_audit_re-powerup-station.md); no verdict earned.
created: 2026-10-19
alpha: yes
first: 10
---
## Why
(the maintainer's INBOX 2026-10-19, the menus / gameplay list) The health and ammo droids (`powerupstation` objects at command posts and bases) are drawn but do nothing in the port: no code reads the class (`grep -i powerupstation engine/crates` is empty). On the PS2 a soldier who stands by one is healed and re-supplied; that is a player-visible rule on every map that places them.

## Evidence
`analysis/re-powerups.md` 7.3: the station uses the same give functions as a pickup (callers in `lead_tmp/pwup/check_absent.txt`); 5.3 / 5.4 hold the health and ammo give rules; the station's own reach, rate, team rule and cool-down are NOT read yet.

## Notes
Take mechanisms and layouts from any outside material, never constants: every number comes from the PS2 corpus or a PCSX2 measurement.
- 2026-10-20 (the maintainer): he plays with invincibility and infinite ammo, so he cannot tell whether the droids work: today they do NOT (drawn only). The PS2 droids also animate - they rotate round - and make noises / movements while healing and giving ammo; he wants all of it. Scope: the station's give (health, ammo, BF2 energy; reach, rate, team rule), its idle and active animation, its sounds and effects, the HUD feedback (the BF2 HUD's own); RE first (powerupstation FUN_002aa5d8 B1, FUN_00368898 B2).
- 2026-10-20 RE DONE (lane 1; `analysis/re-powerup-station.md` [O], report `reports/20261020_2045_card_PWST.md`). Acceptance 1: reach = the instance `Radius` sphere (5.0 m, strict 3-D, feet); rate = once per `PowerupDelay` (BF1 +15 / 1.0 s, BF2 +25 / 1.0 s, ammo +1 clip / 2.0 s, BF1 vehicle droid +400 / +1 clip); no team rule (any team; all stations Team 0); bots never seek one (served only when they stand inside); sound / effects / tips / animation in §5-§6. Measured live on both games (`lead_tmp/pwst/ps2/out/`). Acceptance 2: census §2.2 (NAB1 15, TAT1 11, YAV2 18, UTA1 17, MYG1 12, HOT1 33 / 18 in conquest). Acceptance 3 is the build (resume:). The tips (BF1 tooltip 10, BF2 hints 15-17) are a note on card TIPS; broken stations are card PWSTD.
- 2026-10-21 slice 3 LANDED 3890c34 (lane 1; report `reports/20261021_1520_card_PWST.md`): the FX-7's `AmbientSound` on one handle per station (`SoundLib::station_ambient`, `event_sounds::station_asks`), asked every tick while idle through the property's draw. Headless SOUND_LOG 60 s beside a droid: BF1 NAB1 37 starts, mean gap 1.65 s (the rule predicts ~1.57 s: 0.90 s take / pitch + 1/0.05 ticks); BF2 HOT1 39 starts; 100 m away 0. Logs `lead_tmp/pwst/s3/`.
