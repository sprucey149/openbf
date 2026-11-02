---
id: AMBS
title: Ambience: the world's soundstream regions, sound objects and prop loops - each map's ambience streams (looping 3-D at the region's centre, distances from the region's size), com_snd_amb_static objects (Mygeeto's neon, Tatooine's sandcrawler), props with SoundProperty (fires, vaporators) and effect sounds
milestone: M1
value: 4
phase: build
status: done
deps:
files: engine/crates/game/src/ambience.rs, engine/crates/game/src/sounds.rs, engine/crates/game/src/event_sounds.rs, engine/crates/game/src/lib.rs, engine/crates/game/src/main/windowed.rs, engine/crates/game/src/main/headless.rs, engine/crates/game/tests/ambience_ps2.rs
declares:
needs:
effort:
branch: ambs
resume: landed 408a8d35 2026-10-16
created: 2026-10-03
alpha: yes
---
## Why
The port's battle has no ambience. On the PS2 every map has one or more ambience streams all battle long and fixed
loops on props and world objects (fires, vaporators, neon signs, control panels). RE: `analysis/re-ambience-music.md`
sections 1-2 (8a35a92).

## Evidence
- A world region named `soundstream <stream property> <divisor>` is a looping 3-D stream sound at the region's
  centre, MinDistance = |SIZE| / divisor, MaxDistance = MuteDistance = |SIZE|, started once at battle start (BF2
  restarts a stopped one every update). The level by distance is `analysis/re-sound-distance.md`'s law with those
  three distances replacing the property's own.
- World objects of class `com_snd_amb_static` carry `Sound`, `MinDistance`, `MaxDistance` (MuteDistance = Max);
  one `com_snd_amb_streaming` object exists (YAV1 `waterfall`).
- A prop class with `SoundProperty` loops that sound at the prop with the sound's OWN distances: BF1 from battle
  start, BF2 from the first time the prop is drawn, at prop matrix x `SoundOffset`. Never stopped. A non-looping
  sound is re-asked whenever its handle is idle (its `PlayInterval` / `PlayProbability` pace it).
- An effect's `SoundName` plays once on the emitter (a looping one follows it): Naboo's `com_amb_fire`.
- Tools: `engine/artifacts/lead_tmp/ambm/tools/` (`regions.py`, `region_census.py`, `snd_objects.py`,
  `ram_regions.py`).
