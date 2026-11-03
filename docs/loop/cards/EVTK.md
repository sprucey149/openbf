---
id: EVTK
title: The windowed game reads a sim tick's events once: effects and event sounds are re-spawned on every render frame that steps no tick (twice per event at 60 Hz), and dropped when a frame steps two
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Found by SQMK (2026-10-02) while fixing the maintainer's doubled 'Follow me — 3 units' line. `Sim::events` holds the LAST stepped
tick's events until the next step (`core/src/sim/step.rs:73` clears at step start). The windowed frame
(`game/src/main/windowed.rs` ~1129) calls `walk.advance(dt)` and then, whatever the number of ticks it ran,
`self.fx.ingest(walk)`, `play_event_sounds` and the commander's `v.frame`. On a vsynced 60 Hz display every other frame
steps 0 ticks, so each impact effect and each event sound (shot, reload, hit, order chatter) is started twice ~16 ms apart;
on a faster display more often; a frame that steps 2 ticks loses the first tick's events (the comment there says so).
`Fx`'s own doc says 'ingest once per sim tick' (`game/src/fx.rs:360`). READ from the code, not yet measured — the
headless paths step tick by tick, so no golden or gate sees it. SQMK fixed the message box only (`game::event_feed`
gates on the tick).

## Result (2026-10-02, build, branch evtk 0b99471)
MEASURED, the claim holds. `cargo test -p game --lib event_sounds` (`game/src/event_sounds.rs`,
`a_ticks_events_start_their_sounds_and_effects_once`): a soldier firing at the ground for 8 s = 240 ticks, 40 shots,
40 struck bolts, so K = 80 sounds / 40 effects / 40 bolt remnants.
| frame rate | read once per render frame (before) | read in the tick hook (after) |
|---|---|---|
| 144 Hz | 384 / 192 / 192 | 80 / 40 / 40 |
| 60 Hz | 160 / 80 / 80 | 80 / 40 / 40 |
| 30 Hz | 80 / 40 / 40 | 80 / 40 / 40 |
| 15 Hz | 0 / 0 / 0 | 80 / 40 / 40 |
The test drives the same functions the window calls (`WalkSim::advance_with`, `Fx::ingest`, `BoltVis::track`,
`event_sounds::each`), not the winit `App` itself: the 20 lines of glue in `main/windowed.rs` are read, not run.
Gates: every gate PASS, goldens unchanged, ci 1099 / 0 / 43 (+1 = the new test).
